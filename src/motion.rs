//! Motion sensors (6-axis: 3-axis accelerometer + 3-axis gyroscope) on PlayStation 4 / 5
//! controllers (DualShock 4, DualSense). Xbox controllers have none; Switch controllers need
//! a start-up command and aren't read yet.
//!
//! gilrs doesn't expose motion, so a background thread reads it directly:
//! - Linux: the kernel's driver adds a separate "… Motion Sensors" input device with
//!   calibrated values; we read its events.
//! - Windows: the controller's raw HID input reports (uncalibrated, close enough for a test).

use std::sync::{Arc, Mutex};

/// One reading: acceleration in g (about 1 g of gravity at rest) and rotation in degrees/second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
}

impl Sample {
    pub fn gravity(&self) -> f32 {
        len(self.accel)
    }
    pub fn turning(&self) -> f32 {
        len(self.gyro)
    }
}

fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// No controller with motion sensors found (yet).
    Searching,
    /// Reading this controller.
    Live(String),
    /// Found one, but the system doesn't let us read it (Linux device permissions).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    NeedsPermission(String),
}

struct Shared {
    sample: Option<Sample>,
    source: Source,
}

/// What the test has seen so far.
#[derive(Clone, Copy, Debug, Default)]
pub struct Checks {
    /// Furthest each accelerometer axis has read (absolute g).
    pub tilt: [f32; 3],
    /// Fastest turn seen around each axis (absolute °/s).
    pub turn: [f32; 3],
    /// Gravity while held still (should be about 1 g).
    pub rest_gravity: Option<f32>,
    /// Gyro reading while held still (should be near 0 °/s).
    pub rest_gyro: Option<f32>,
}

/// A tilt counts once an axis reads this much of gravity; a turn once it's this fast.
pub const TILT_G: f32 = 0.6;
pub const TURN_DPS: f32 = 150.0;

impl Checks {
    pub fn update(&mut self, s: &Sample) {
        for i in 0..3 {
            self.tilt[i] = self.tilt[i].max(s.accel[i].abs());
            self.turn[i] = self.turn[i].max(s.gyro[i].abs());
        }
        // Held still: barely turning. Smooth the readings so one jolt doesn't count.
        if s.turning() < 12.0 {
            let ema = |old: Option<f32>, new: f32| Some(old.map_or(new, |o| o * 0.95 + new * 0.05));
            self.rest_gravity = ema(self.rest_gravity, s.gravity());
            self.rest_gyro = ema(self.rest_gyro, s.turning());
        }
    }

    pub fn tilts_done(&self) -> usize {
        self.tilt.iter().filter(|t| **t >= TILT_G).count()
    }

    pub fn turns_done(&self) -> usize {
        self.turn.iter().filter(|t| **t >= TURN_DPS).count()
    }

    /// The accelerometer reads clearly wrong at rest (gravity should be about 1 g).
    pub fn gravity_wrong(&self) -> bool {
        self.rest_gravity.is_some_and(|g| !(0.7..=1.3).contains(&g))
    }

    /// The gyro reports turning while the controller is still.
    pub fn gyro_drifts(&self) -> bool {
        self.rest_gyro.is_some_and(|d| d > 8.0)
    }

    pub fn tested(&self) -> bool {
        self.rest_gravity.is_some() || self.turns_done() > 0
    }
}

pub struct Motion {
    shared: Arc<Mutex<Shared>>,
    started: bool,
    pub checks: Checks,
    pub last: Option<Sample>,
}

impl Default for Motion {
    fn default() -> Self {
        Self { shared: Arc::new(Mutex::new(Shared { sample: None, source: Source::Searching })), started: false, checks: Checks::default(), last: None }
    }
}

impl Motion {
    /// Start looking for a controller (once; the thread keeps looking and reconnects).
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        let shared = self.shared.clone();
        std::thread::Builder::new().name("motion".into()).spawn(move || run(shared)).ok();
    }

    /// Take the latest reading into the checks. Called every frame on the Gamepad page.
    pub fn update(&mut self) {
        let sample = self.shared.lock().ok().and_then(|s| s.sample);
        if let Some(s) = sample {
            if self.last != Some(s) {
                self.checks.update(&s);
            }
            self.last = Some(s);
        }
    }

    pub fn source(&self) -> Source {
        self.shared.lock().map_or(Source::Searching, |s| s.source.clone())
    }

    pub fn reset(&mut self) {
        self.checks = Checks::default();
    }
}

/// Hand the UI thread the current source and, when there is one, the newest reading.
/// Losing the controller clears the reading.
fn publish(shared: &Arc<Mutex<Shared>>, source: Source, sample: Option<Sample>) {
    if let Ok(mut s) = shared.lock() {
        if sample.is_some() || !matches!(source, Source::Live(_)) {
            s.sample = sample;
        }
        s.source = source;
    }
}

fn run(shared: Arc<Mutex<Shared>>) {
    if std::env::var("NYANSPECT_MOTION_DEMO").is_ok() {
        demo(shared);
        return;
    }
    loop {
        if let Err(source) = read_device(&shared) {
            publish(&shared, source, None);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// Dev: NYANSPECT_MOTION_DEMO fakes a controller being tilted and turned.
fn demo(shared: Arc<Mutex<Shared>>) {
    let t0 = std::time::Instant::now();
    loop {
        let t = t0.elapsed().as_secs_f32();
        let roll = (t * 0.9).sin() * 0.9;
        let pitch = (t * 0.6).cos() * 0.5;
        let sample = Sample {
            accel: [roll.sin(), roll.cos() * pitch.cos(), pitch.sin()],
            gyro: [(t * 2.1).sin() * 220.0, (t * 1.3).cos() * 90.0, (t * 3.0).sin() * 180.0],
        };
        publish(&shared, Source::Live("Demo DualSense".into()), Some(sample));
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

// ---------------------------------------------------------------- Linux: evdev

#[cfg(target_os = "linux")]
fn read_device(shared: &Arc<Mutex<Shared>>) -> Result<(), Source> {
    use std::io::Read;
    let (path, name) = find_linux().ok_or(Source::Searching)?;
    let mut file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Err(Source::NeedsPermission(name)),
        Err(_) => return Err(Source::Searching),
    };
    // Units per g / per °/s, as the driver reports them (fall back to the PlayStation driver's).
    let res = |code: u16, fallback: f32| abs_resolution(&file, code).filter(|r| *r > 0).map_or(fallback, |r| r as f32);
    let (acc_res, gyro_res) = ([res(0, 8192.0), res(1, 8192.0), res(2, 8192.0)], [res(3, 1024.0), res(4, 1024.0), res(5, 1024.0)]);
    let size = std::mem::size_of::<libc::input_event>();
    let mut buf = vec![0u8; size * 64];
    let mut raw = [0i32; 6];
    publish(shared, Source::Live(name.clone()), None);
    loop {
        let n = file.read(&mut buf).map_err(|_| Source::Searching)?;
        for ev in buf[..n].chunks_exact(size) {
            // struct input_event { timeval time; u16 type; u16 code; i32 value; }
            let off = size - 8;
            let kind = u16::from_ne_bytes([ev[off], ev[off + 1]]);
            let code = u16::from_ne_bytes([ev[off + 2], ev[off + 3]]);
            let value = i32::from_ne_bytes([ev[off + 4], ev[off + 5], ev[off + 6], ev[off + 7]]);
            match (kind, code) {
                (3, 0..=5) => raw[code as usize] = value, // EV_ABS: X/Y/Z accel, RX/RY/RZ gyro
                (0, 0) => {
                    // EV_SYN: one complete reading.
                    let sample = Sample {
                        accel: [raw[0] as f32 / acc_res[0], raw[1] as f32 / acc_res[1], raw[2] as f32 / acc_res[2]],
                        gyro: [raw[3] as f32 / gyro_res[0], raw[4] as f32 / gyro_res[1], raw[5] as f32 / gyro_res[2]],
                    };
                    publish(shared, Source::Live(name.clone()), Some(sample));
                }
                _ => {}
            }
        }
    }
}

/// The kernel's motion device for a PlayStation (or Switch) controller: "… Motion Sensors" / "… IMU".
#[cfg(target_os = "linux")]
fn find_linux() -> Option<(std::path::PathBuf, String)> {
    let mut found: Vec<(String, String)> = std::fs::read_dir("/sys/class/input")
        .ok()?
        .flatten()
        .filter_map(|e| {
            let dev = e.file_name().to_string_lossy().into_owned();
            if !dev.starts_with("event") {
                return None;
            }
            let name = std::fs::read_to_string(e.path().join("device/name")).ok()?.trim().to_string();
            (name.ends_with("Motion Sensors") || name.ends_with(" IMU")).then_some((dev, name))
        })
        .collect();
    found.sort();
    found.into_iter().next().map(|(dev, name)| (std::path::PathBuf::from(format!("/dev/input/{dev}")), name))
}

/// EVIOCGABS: an axis's range info; we want its resolution (units per g or per °/s).
#[cfg(target_os = "linux")]
fn abs_resolution(file: &std::fs::File, code: u16) -> Option<i32> {
    use std::os::fd::AsRawFd;
    let mut info = [0i32; 6]; // value, minimum, maximum, fuzz, flat, resolution
    let request = (2u64 << 30) | ((std::mem::size_of_val(&info) as u64) << 16) | (0x45u64 << 8) | (0x40 + code as u64);
    // SAFETY: EVIOCGABS writes one struct input_absinfo (six i32) into `info`.
    let r = unsafe { libc::ioctl(file.as_raw_fd(), request as _, info.as_mut_ptr()) };
    (r >= 0).then_some(info[5])
}

// ---------------------------------------------------------------- Windows: raw HID reports

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Pad {
    DualShock4,
    DualSense,
}

#[cfg_attr(not(windows), allow(dead_code))]
const SONY: u16 = 0x054C;

#[cfg_attr(not(windows), allow(dead_code))]
pub fn pad_kind(vendor: u16, product: u16) -> Option<Pad> {
    match (vendor, product) {
        (SONY, 0x05C4 | 0x09CC | 0x0BA0) => Some(Pad::DualShock4),
        (SONY, 0x0CE6 | 0x0DF2) => Some(Pad::DualSense),
        _ => None,
    }
}

/// Raw sensor units: the gyro spans ±2000 °/s and the accelerometer about ±4 g over an i16.
const GYRO_PER_DPS: f32 = 16.384;
const ACCEL_PER_G: f32 = 8192.0;

/// Pull the sensors out of an input report (the first byte is the report ID).
/// USB and Bluetooth reports put them at different offsets.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_report(pad: Pad, data: &[u8]) -> Option<Sample> {
    let (gyro, accel) = match (pad, *data.first()?) {
        (Pad::DualShock4, 0x01) if data.len() >= 25 => (13, 19),
        (Pad::DualShock4, 0x11) if data.len() >= 27 => (15, 21),
        (Pad::DualSense, 0x01) if data.len() >= 28 => (16, 22),
        (Pad::DualSense, 0x31) if data.len() >= 29 => (17, 23),
        _ => return None,
    };
    let at = |o: usize| i16::from_le_bytes([data[o], data[o + 1]]) as f32;
    Some(Sample {
        accel: [at(accel) / ACCEL_PER_G, at(accel + 2) / ACCEL_PER_G, at(accel + 4) / ACCEL_PER_G],
        gyro: [at(gyro) / GYRO_PER_DPS, at(gyro + 2) / GYRO_PER_DPS, at(gyro + 4) / GYRO_PER_DPS],
    })
}

#[cfg(windows)]
fn read_device(shared: &Arc<Mutex<Shared>>) -> Result<(), Source> {
    let api = hidapi::HidApi::new().map_err(|_| Source::Searching)?;
    let info = api.device_list().find(|d| pad_kind(d.vendor_id(), d.product_id()).is_some()).ok_or(Source::Searching)?;
    let pad = pad_kind(info.vendor_id(), info.product_id()).ok_or(Source::Searching)?;
    let name = info.product_string().filter(|s| !s.trim().is_empty()).map_or_else(
        || if pad == Pad::DualSense { "DualSense".to_string() } else { "DualShock 4".to_string() },
        |s| s.trim().to_string(),
    );
    let device = info.open_device(&api).map_err(|_| Source::Searching)?;
    // Over Bluetooth the controllers only send motion after this feature report is read
    // (DualShock 4: 0x02, DualSense: 0x05, both calibration data). Over USB it's harmless.
    let mut feature = [0u8; 64];
    feature[0] = if pad == Pad::DualSense { 0x05 } else { 0x02 };
    let _ = device.get_feature_report(&mut feature);
    publish(shared, Source::Live(name.clone()), None);
    let mut buf = [0u8; 128];
    loop {
        let n = device.read_timeout(&mut buf, 1000).map_err(|_| Source::Searching)?;
        if let Some(sample) = parse_report(pad, &buf[..n]) {
            publish(shared, Source::Live(name.clone()), Some(sample));
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn read_device(_shared: &Arc<Mutex<Shared>>) -> Result<(), Source> {
    Err(Source::Searching)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(report: &mut [u8], at: usize, v: i16) {
        report[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn dualsense_usb_and_bluetooth_reports() {
        let mut usb = [0u8; 64];
        usb[0] = 0x01;
        put(&mut usb, 16, 1638); // gyro X: 100 °/s
        put(&mut usb, 24, 8192); // accel Y: 1 g
        let s = parse_report(Pad::DualSense, &usb).unwrap();
        assert!((s.gyro[0] - 100.0).abs() < 0.1 && (s.accel[1] - 1.0).abs() < 0.001);

        let mut bt = [0u8; 78];
        bt[0] = 0x31;
        put(&mut bt, 17, -1638);
        put(&mut bt, 25, 8192);
        let s = parse_report(Pad::DualSense, &bt).unwrap();
        assert!((s.gyro[0] + 100.0).abs() < 0.1 && (s.accel[1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn dualshock4_reports_and_short_ones() {
        let mut usb = [0u8; 64];
        usb[0] = 0x01;
        put(&mut usb, 17, 3277); // gyro Z: 200 °/s
        put(&mut usb, 19, -8192); // accel X: -1 g
        let s = parse_report(Pad::DualShock4, &usb).unwrap();
        assert!((s.gyro[2] - 200.0).abs() < 0.1 && (s.accel[0] + 1.0).abs() < 0.001);
        // Bluetooth's basic report (before motion is switched on) has no sensors.
        assert!(parse_report(Pad::DualShock4, &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_none());
        assert_eq!(pad_kind(0x054C, 0x09CC), Some(Pad::DualShock4));
        assert_eq!(pad_kind(0x045E, 0x0B12), None, "Xbox controllers have no motion sensors");
    }

    #[test]
    fn checks_count_tilts_turns_and_rest() {
        let mut c = Checks::default();
        for _ in 0..200 {
            c.update(&Sample { accel: [0.0, 1.0, 0.0], gyro: [0.5, -0.3, 0.2] });
        }
        assert_eq!(c.tilts_done(), 1);
        assert!(!c.gravity_wrong() && !c.gyro_drifts());
        c.update(&Sample { accel: [0.8, 0.5, 0.7], gyro: [200.0, 0.0, -180.0] });
        assert_eq!((c.tilts_done(), c.turns_done()), (3, 2));
        let mut broken = Checks::default();
        for _ in 0..200 {
            broken.update(&Sample { accel: [0.0, 0.1, 0.0], gyro: [0.0, 10.0, 0.0] });
        }
        assert!(broken.gravity_wrong() && broken.gyro_drifts());
    }
}
