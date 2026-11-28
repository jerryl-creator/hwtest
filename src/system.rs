//! System page: which computer this is (model, serial number, specs) and how healthy its
//! battery is. Everything is read in a background thread, so the window never stalls.
//!
//! Model and serial number come from the firmware (SMBIOS/DMI): sysfs on Linux, the raw
//! SMBIOS table on Windows. CPU, memory, drives and OS come from `sysinfo` (drives from
//! sysfs on Linux, which names the physical disks). Batteries come from `starship-battery`.

use crate::i18n::t;
use crate::report::{Section, Status};
use crate::theme::{self, alpha, Mascot};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Below this share of its original capacity a battery counts as worn (the usual threshold
/// for "consider replacing it").
pub const GOOD_HEALTH: f32 = 0.80;

#[derive(Clone, Debug, PartialEq)]
pub enum Serial {
    Known(String),
    /// Linux only lets administrators read it.
    NeedsAdmin,
    NotSet,
}

#[derive(Clone, Debug)]
pub struct Drive {
    pub name: String,
    pub bytes: u64,
    /// "NVMe SSD", "SSD", "HDD", "USB" or "".
    pub kind: &'static str,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub model: Option<String>,
    pub serial: Serial,
    pub bios: Option<String>,
    pub os: String,
    pub cpu: Option<String>,
    pub cores: Option<usize>,
    pub threads: usize,
    pub ram_bytes: u64,
    pub drives: Vec<Drive>,
}

#[derive(Clone, Debug)]
pub struct Battery {
    pub name: String,
    /// Full-charge capacity ÷ design capacity (None if the battery doesn't say).
    pub health: Option<f32>,
    pub full_wh: f32,
    pub design_wh: f32,
    /// 0..1
    pub charge: f32,
    pub state: &'static str,
    pub cycles: Option<u32>,
    pub vendor: Option<String>,
    /// The model name: on many laptops this is the battery's part number (e.g. "L19M3PF4").
    pub model: Option<String>,
    pub serial: Option<String>,
    /// "Li-ion", "Li-polymer"…
    pub chemistry: Option<&'static str>,
    pub voltage: Option<f32>,
    /// Watts flowing in (charging) or out (on battery).
    pub power_w: Option<f32>,
    pub temperature_c: Option<f32>,
    /// Minutes until full (charging) or empty (on battery).
    pub minutes_left: Option<u32>,
    /// The laptop stops charging at this percentage (a battery-care setting).
    pub charge_limit: Option<u32>,
}

impl Battery {
    pub fn worn(&self) -> bool {
        self.health.is_some_and(|h| h < GOOD_HEALTH)
    }
}

enum Msg {
    Info(Info),
    Batteries(Result<Vec<Battery>, String>),
}

pub struct SystemPage {
    info: Option<Info>,
    batteries: Option<Result<Vec<Battery>, String>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// When the battery was last asked for (it's re-read every few seconds on this page).
    battery_asked: f64,
    battery_pending: bool,
    /// The page has been opened: until then it doesn't count as checked.
    viewed: bool,
    copied_at: Option<f64>,
}

impl SystemPage {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let worker = tx.clone();
        std::thread::Builder::new()
            .name("system-info".into())
            .spawn(move || {
                let _ = worker.send(Msg::Batteries(read_batteries()));
                let _ = worker.send(Msg::Info(collect()));
            })
            .ok();
        Self { info: None, batteries: None, tx, rx, battery_asked: 0.0, battery_pending: true, viewed: false, copied_at: None }
    }

    fn pull(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Info(i) => self.info = Some(i),
                Msg::Batteries(b) => {
                    self.batteries = Some(b);
                    self.battery_pending = false;
                }
            }
        }
    }

    fn refresh_battery(&mut self, now: f64) {
        if self.battery_pending {
            return;
        }
        self.battery_pending = true;
        self.battery_asked = now;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Batteries(read_batteries()));
        });
    }

    pub fn clear_results(&mut self) {
        self.viewed = false;
    }

    fn batteries(&self) -> &[Battery] {
        match &self.batteries {
            Some(Ok(b)) => b,
            _ => &[],
        }
    }

    /// The model name for the Home page, once known.
    pub fn model(&mut self) -> Option<String> {
        self.pull();
        self.info.as_ref().and_then(|i| i.model.clone())
    }

    pub fn report(&self) -> Section {
        let worn = self.batteries().iter().any(Battery::worn);
        let status = if !self.viewed {
            Status::NotTested
        } else if self.info.is_none() {
            Status::InProgress
        } else if worn {
            Status::Issue
        } else {
            Status::Pass
        };
        let mut lines = Vec::new();
        if let Some(i) = &self.info {
            lines.push(tr!("Model: {0}", i.model.as_deref().unwrap_or(t("unknown"))));
            lines.push(serial_line(&i.serial));
            lines.push(tr!("Processor: {0}", cpu_text(i)));
            lines.push(tr!("Memory: {0}", ram_text(i.ram_bytes)));
            for d in &i.drives {
                lines.push(tr!("Drive: {0}", drive_text(d)));
            }
            lines.push(tr!("Operating system: {0}", i.os));
            if let Some(b) = &i.bios {
                lines.push(tr!("BIOS: {0}", b));
            }
        }
        match &self.batteries {
            Some(Ok(bs)) if bs.is_empty() => lines.push(t("Battery: none found (desktop PC?)").into()),
            Some(Ok(bs)) => {
                for b in bs {
                    lines.push(match b.health {
                        Some(h) => tr!("Battery health: {0}% ({1} of {2} Wh when new)", pct(h), wh(b.full_wh), wh(b.design_wh)),
                        None => t("Battery health: not reported by the battery").into(),
                    });
                    if let Some(c) = b.cycles {
                        lines.push(tr!("Charge cycles: {0}", c));
                    }
                    let id: Vec<String> = [b.vendor.clone(), b.model.clone(), b.chemistry.map(str::to_string)].into_iter().flatten().collect();
                    if !id.is_empty() {
                        lines.push(tr!("Battery: {0}", id.join(" · ")));
                    }
                    if let Some(sn) = &b.serial {
                        lines.push(tr!("Battery serial number: {0}", sn));
                    }
                    if b.worn() {
                        lines.push(t("The battery holds under 80% of its original charge: expect noticeably shorter battery life.").into());
                    }
                }
            }
            Some(Err(e)) => lines.push(tr!("Battery: couldn't read it ({0})", e)),
            None => {}
        }
        Section { title: "System", jp: "システム", status, lines }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool, renderer: &str) {
        let now = ui.input(|i| i.time);
        self.viewed = true;
        self.pull();
        if now - self.battery_asked > 10.0 {
            self.refresh_battery(now);
        }
        // Battery charge changes slowly: a repaint every couple of seconds keeps it current.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));

        let worn = self.batteries().iter().any(Battery::worn);
        if self.info.is_none() {
            mascot.say(t("Peeking inside this computer…"));
        } else if worn {
            mascot.say(t("This battery is getting tired… (；ω；)"));
        } else {
            mascot.say(t("Here's what's inside this computer~ ♪"));
        }

        let Some(info) = self.info.clone() else {
            theme::card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t("Reading the computer's details…"));
                });
            });
            return;
        };

        // Big model name.
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(t("This computer")).font(theme::bold(13.0)).color(theme::TEXT_DIM));
            ui.label(egui::RichText::new(info.model.as_deref().unwrap_or(t("Unknown model"))).font(theme::bold(26.0)).color(Color32::WHITE));
            ui.add_space(2.0);
            ui.label(egui::RichText::new(&info.os).size(14.0).color(theme::TEXT));
        });
        ui.add_space(12.0);

        let screen = ui.ctx().input(|i| i.viewport().monitor_size).map(|s| s * ui.ctx().pixels_per_point());
        ui.columns(2, |cols| {
            theme::card().show(&mut cols[0], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Identity", "本体情報", cjk);
                let mut rows: Vec<(&str, String)> = vec![(t("Model"), info.model.clone().unwrap_or_else(|| t("unknown").into()))];
                let serial = match &info.serial {
                    Serial::Known(s) => s.clone(),
                    Serial::NeedsAdmin => t("needs administrator rights to read").into(),
                    Serial::NotSet => t("not set by the maker").into(),
                };
                rows.push((t("Serial number"), serial));
                if let Some(b) = &info.bios {
                    rows.push((t("BIOS"), b.clone()));
                }
                rows.push((t("Operating system"), info.os.clone()));
                grid(ui, "sys-id", &rows);
                if let Serial::Known(s) = &info.serial {
                    ui.add_space(6.0);
                    if ui.button(t("Copy serial number")).clicked() {
                        ui.ctx().copy_text(s.clone());
                        self.copied_at = Some(now);
                    }
                    if self.copied_at.is_some_and(|at| now - at < 3.0) {
                        ui.label(egui::RichText::new(t("Copied!")).color(Color32::WHITE));
                    }
                }
            });
            theme::card().show(&mut cols[1], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Specs", "スペック", cjk);
                let mut rows = vec![
                    (t("Processor"), cpu_text(&info)),
                    (t("Memory"), ram_text(info.ram_bytes)),
                    (t("Graphics"), renderer.to_string()),
                    (t("Screen"), screen.map_or("?".into(), |s| format!("{:.0} × {:.0}", s.x, s.y))),
                ];
                if info.drives.is_empty() {
                    rows.push((t("Drives"), t("none found").into()));
                }
                for (i, d) in info.drives.iter().enumerate() {
                    rows.push((if i == 0 { t("Drives") } else { "" }, drive_text(d)));
                }
                grid(ui, "sys-specs", &rows);
            });
        });
        ui.add_space(12.0);

        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::card_title(ui, "Battery", "バッテリー", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::fancy_button(ui, t("↻ Refresh"), Vec2::new(110.0, 32.0), theme::SILVER, theme::STEEL, !self.battery_pending).clicked() {
                        self.refresh_battery(now);
                    }
                });
            });
            match self.batteries.clone() {
                None => {
                    ui.spinner();
                }
                Some(Err(e)) => {
                    ui.label(tr!("Couldn't read the battery: {0}", e));
                }
                Some(Ok(bs)) if bs.is_empty() => {
                    ui.label(egui::RichText::new(t("No battery found: this looks like a desktop PC.")).size(14.0).color(theme::TEXT));
                }
                Some(Ok(bs)) => {
                    for (i, b) in bs.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(14.0);
                        }
                        battery_ui(ui, b);
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(t(
                            "Health compares how much charge the battery holds now with when it was new. Under 80% means noticeably shorter battery life.",
                        ))
                        .size(12.5)
                        .color(theme::TEXT_DIM),
                    );
                }
            }
        });
    }
}

fn battery_ui(ui: &mut egui::Ui, b: &Battery) {
    ui.horizontal(|ui| {
        let (verdict, col) = match b.health {
            Some(h) if h >= GOOD_HEALTH => (t("Good"), Color32::WHITE),
            Some(h) if h >= 0.6 => (t("Worn"), theme::GLINT),
            Some(_) => (t("Replace soon"), theme::GLINT),
            None => (t("Not reported"), theme::TEXT_DIM),
        };
        let big = b.health.map_or("?".to_string(), |h| format!("{}%", pct(h)));
        ui.label(egui::RichText::new(big).font(theme::bold(40.0)).color(Color32::WHITE));
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(t("Battery health")).font(theme::bold(14.0)).color(theme::TEXT_DIM));
            egui::Frame::new()
                .fill(if b.worn() { theme::GLINT } else { alpha(theme::SILVER, 0.2) })
                .corner_radius(10)
                .inner_margin(egui::Margin::symmetric(8, 1))
                .show(ui, |ui| {
                    let ink = if b.worn() { theme::INK } else { col };
                    ui.label(egui::RichText::new(if b.worn() { format!("⚠ {verdict}") } else { verdict.to_string() }).font(theme::bold(13.0)).color(ink));
                });
        });
    });
    // Gauge: capacity now against capacity when new, with the 80% line marked.
    if let Some(h) = b.health {
        ui.add_space(4.0);
        let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(bar, 7.0, Color32::from_rgb(34, 34, 38));
        let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * h.clamp(0.0, 1.0), bar.height()));
        if fill.width() > 2.0 {
            theme::hgradient(p, fill, 7.0, theme::STEEL, if b.worn() { theme::SILVER } else { Color32::WHITE });
        }
        let x = bar.left() + bar.width() * GOOD_HEALTH;
        p.line_segment([Pos2::new(x, bar.top() - 3.0), Pos2::new(x, bar.bottom() + 3.0)], Stroke::new(1.5, alpha(theme::LIGHT, 0.7)));
        p.text(Pos2::new(x, bar.bottom() + 4.0), egui::Align2::CENTER_TOP, "80%", egui::FontId::proportional(11.0), theme::TEXT_DIM);
        ui.add_space(14.0);
    }
    let not_reported = || t("not reported").to_string();
    let mut now_rows = Vec::new();
    if b.design_wh > 0.0 {
        now_rows.push((t("Holds now"), format!("{} Wh", wh(b.full_wh))));
        now_rows.push((t("When new"), format!("{} Wh", wh(b.design_wh))));
    }
    now_rows.push((t("Charge cycles"), b.cycles.map_or_else(not_reported, |c| c.to_string())));
    now_rows.push((t("Charge now"), format!("{}% · {}", pct(b.charge), t(b.state))));
    if let Some(w) = b.power_w {
        let text = if b.state == "Charging" { tr!("charging at {0} W", format!("{w:.1}")) } else { tr!("using {0} W", format!("{w:.1}")) };
        now_rows.push((t("Power"), text));
    }
    if let Some(m) = b.minutes_left {
        let time = duration_text(m);
        now_rows.push((t("Time left"), if b.state == "Charging" { tr!("{0} until full", time) } else { tr!("{0} until empty", time) }));
    }
    if let Some(l) = b.charge_limit {
        now_rows.push((t("Charge limit"), tr!("stops at {0}% (battery-care setting)", l)));
    }

    let mut id_rows = vec![
        (t("Maker"), b.vendor.clone().unwrap_or_else(not_reported)),
        (t("Model / part number"), b.model.clone().unwrap_or_else(not_reported)),
        (t("Serial number"), b.serial.clone().unwrap_or_else(not_reported)),
        (t("Chemistry"), b.chemistry.map_or_else(not_reported, str::to_string)),
    ];
    if let Some(v) = b.voltage {
        id_rows.push((t("Voltage"), format!("{v:.2} V")));
    }
    if let Some(c) = b.temperature_c {
        id_rows.push((t("Temperature"), format!("{c:.0} °C")));
    }
    ui.columns(2, |cols| {
        grid(&mut cols[0], ("sys-bat-now", b.name.as_str()), &now_rows);
        grid(&mut cols[1], ("sys-bat-id", b.name.as_str()), &id_rows);
    });
}

/// "2 h 15 min" / "48 min".
fn duration_text(minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    if h == 0 { tr!("{0} min", m) } else { tr!("{0} h {1} min", h, m) }
}

fn grid(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, rows: &[(&str, String)]) {
    egui::Grid::new(id).num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
        for (k, v) in rows {
            ui.label(egui::RichText::new(*k).size(13.5).color(theme::TEXT_DIM));
            ui.add(egui::Label::new(egui::RichText::new(v).size(13.5).color(Color32::WHITE)).wrap());
            ui.end_row();
        }
    });
}

fn serial_line(s: &Serial) -> String {
    match s {
        Serial::Known(s) => tr!("Serial number: {0}", s),
        Serial::NeedsAdmin => t("Serial number: needs administrator rights to read").into(),
        Serial::NotSet => t("Serial number: not set by the maker").into(),
    }
}

fn cpu_text(i: &Info) -> String {
    let name = i.cpu.clone().unwrap_or_else(|| t("unknown").into());
    match i.cores {
        Some(c) if c != i.threads => tr!("{0} ({1} cores, {2} threads)", name, c, i.threads),
        Some(c) => tr!("{0} ({1} cores)", name, c),
        None => name,
    }
}

/// Installed memory, rounded up to a size you can actually buy (some is reserved for
/// graphics and firmware), plus what the OS can use.
fn ram_text(bytes: u64) -> String {
    let gib = bytes as f64 / (1u64 << 30) as f64;
    tr!("{0} GB ({1} GB usable)", installed_gb(gib), format!("{gib:.1}"))
}

fn installed_gb(gib: f64) -> u32 {
    const SIZES: [u32; 14] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128];
    SIZES.iter().copied().find(|s| f64::from(*s) >= gib - 0.05).unwrap_or_else(|| gib.ceil() as u32)
}

fn drive_text(d: &Drive) -> String {
    let size = size_text(d.bytes);
    match d.kind {
        "" => format!("{} · {size}", d.name),
        k => format!("{} · {size} {}", d.name, k),
    }
}

/// Drive sizes the way makers label them (decimal GB/TB).
fn size_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1000.0 { format!("{:.1} TB", gb / 1000.0) } else { format!("{gb:.0} GB") }
}

fn pct(x: f32) -> u32 {
    (x * 100.0).round().max(0.0) as u32
}

fn wh(x: f32) -> String {
    format!("{x:.1}")
}

// ---------------------------------------------------------------- reading the details

/// Firmware (SMBIOS/DMI) strings.
#[derive(Default, Debug, PartialEq)]
struct Dmi {
    vendor: Option<String>,
    product: Option<String>,
    version: Option<String>,
    serial: Option<String>,
    board_vendor: Option<String>,
    board_name: Option<String>,
    board_serial: Option<String>,
    bios_version: Option<String>,
    bios_date: Option<String>,
    /// The serial couldn't be read (Linux without root).
    serial_denied: bool,
}

/// Placeholder text makers leave in the firmware.
fn clean(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(char::from(0)).trim();
    let l = s.to_ascii_lowercase();
    const JUNK: [&str; 16] = [
        "",
        "to be filled by o.e.m.",
        "default string",
        "system product name",
        "system manufacturer",
        "system serial number",
        "system version",
        "not applicable",
        "not specified",
        "none",
        "n/a",
        "invalid",
        "0123456789",
        "123456789",
        "o.e.m.",
        "oem",
    ];
    if JUNK.contains(&l.as_str()) || l.chars().all(|c| c == '0' || c == ' ' || c == '.' || c == 'x') {
        None
    } else {
        Some(s.to_string())
    }
}

fn short_vendor(v: &str) -> String {
    let l = v.to_ascii_lowercase();
    let known = [
        ("asustek", "ASUS"),
        ("hewlett-packard", "HP"),
        ("hp", "HP"),
        ("lenovo", "Lenovo"),
        ("dell", "Dell"),
        ("micro-star", "MSI"),
        ("acer", "Acer"),
        ("gigabyte", "Gigabyte"),
        ("microsoft", "Microsoft"),
        ("samsung", "Samsung"),
        ("apple", "Apple"),
        ("huawei", "Huawei"),
        ("xiaomi", "Xiaomi"),
        ("toshiba", "Toshiba"),
        ("dynabook", "Dynabook"),
        ("fujitsu", "Fujitsu"),
        ("razer", "Razer"),
        ("asrock", "ASRock"),
        ("framework", "Framework"),
    ];
    known.iter().find(|(k, _)| l.starts_with(k)).map_or_else(|| v.to_string(), |(_, n)| n.to_string())
}

impl Dmi {
    fn model(&self) -> Option<String> {
        let vendor = self.vendor.as_deref().or(self.board_vendor.as_deref()).map(short_vendor);
        let with_vendor = |name: &str| match &vendor {
            Some(v) if !name.to_ascii_lowercase().starts_with(&v.to_ascii_lowercase()) => format!("{v} {name}"),
            _ => name.to_string(),
        };
        // Lenovo puts the machine type in the product name and the real name in the version.
        if vendor.as_deref() == Some("Lenovo") {
            if let (Some(p), Some(v)) = (&self.product, &self.version) {
                return Some(format!("{} ({p})", with_vendor(v)));
            }
        }
        if let Some(p) = &self.product {
            return Some(with_vendor(&p.replace('_', " ")));
        }
        // Self-built desktops: name the motherboard.
        self.board_name.as_ref().map(|b| tr!("{0} (motherboard)", with_vendor(b)))
    }

    fn serial(&self) -> Serial {
        match (&self.serial, &self.board_serial) {
            (Some(s), _) => Serial::Known(s.clone()),
            _ if self.serial_denied => Serial::NeedsAdmin,
            (None, Some(b)) => Serial::Known(b.clone()),
            (None, None) => Serial::NotSet,
        }
    }

    fn bios(&self) -> Option<String> {
        match (&self.bios_version, &self.bios_date) {
            (Some(v), Some(d)) => Some(format!("{v} ({d})")),
            (Some(v), None) => Some(v.clone()),
            _ => None,
        }
    }
}

#[cfg(target_os = "linux")]
fn read_dmi() -> Dmi {
    let read = |f: &str| std::fs::read_to_string(format!("/sys/class/dmi/id/{f}")).ok().and_then(|s| clean(&s));
    let serial = std::fs::read_to_string("/sys/class/dmi/id/product_serial");
    let board_serial = std::fs::read_to_string("/sys/class/dmi/id/board_serial");
    let denied = |r: &std::io::Result<String>| matches!(r, Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
    Dmi {
        vendor: read("sys_vendor"),
        product: read("product_name"),
        version: read("product_version"),
        serial_denied: denied(&serial),
        serial: serial.ok().and_then(|s| clean(&s)),
        board_vendor: read("board_vendor"),
        board_name: read("board_name"),
        board_serial: board_serial.ok().and_then(|s| clean(&s)),
        bios_version: read("bios_version"),
        bios_date: read("bios_date"),
    }
}

#[cfg(windows)]
fn read_dmi() -> Dmi {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;
    let rsmb = u32::from_be_bytes(*b"RSMB");
    // SAFETY: a null buffer of size 0 only asks for the size; then the buffer has that size.
    let size = unsafe { GetSystemFirmwareTable(rsmb, 0, std::ptr::null_mut(), 0) };
    if size == 0 {
        return Dmi::default();
    }
    let mut buf = vec![0u8; size as usize];
    let got = unsafe { GetSystemFirmwareTable(rsmb, 0, buf.as_mut_ptr().cast(), size) };
    if got == 0 || got > size {
        return Dmi::default();
    }
    // RawSMBIOSData: 8 header bytes (version, table length), then the structures.
    parse_smbios(buf.get(8..).unwrap_or(&[]))
}

#[cfg(not(any(target_os = "linux", windows)))]
fn read_dmi() -> Dmi {
    Dmi::default()
}

/// Read the BIOS (type 0), system (type 1) and baseboard (type 2) strings from a raw SMBIOS table.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_smbios(data: &[u8]) -> Dmi {
    let mut dmi = Dmi::default();
    let mut i = 0;
    while i + 4 <= data.len() {
        let kind = data[i];
        let len = data[i + 1] as usize;
        if len < 4 || i + len > data.len() {
            break;
        }
        let formatted = &data[i..i + len];
        // The strings follow the formatted area, each null-terminated, ending with an extra null.
        let mut strings = Vec::new();
        let mut j = i + len;
        if data.get(j) == Some(&0) && data.get(j + 1) == Some(&0) {
            j += 2;
        } else {
            while j < data.len() {
                let end = data[j..].iter().position(|b| *b == 0).map_or(data.len(), |p| j + p);
                strings.push(String::from_utf8_lossy(&data[j..end]).into_owned());
                j = end + 1;
                if data.get(j) == Some(&0) {
                    j += 1;
                    break;
                }
            }
        }
        let s = |offset: usize| -> Option<String> {
            let idx = *formatted.get(offset)? as usize;
            if idx == 0 { None } else { strings.get(idx - 1).and_then(|v| clean(v)) }
        };
        match kind {
            0 => {
                dmi.bios_version = dmi.bios_version.take().or_else(|| s(0x05));
                dmi.bios_date = dmi.bios_date.take().or_else(|| s(0x08));
            }
            1 => {
                dmi.vendor = dmi.vendor.take().or_else(|| s(0x04));
                dmi.product = dmi.product.take().or_else(|| s(0x05));
                dmi.version = dmi.version.take().or_else(|| s(0x06));
                dmi.serial = dmi.serial.take().or_else(|| s(0x07));
            }
            2 => {
                dmi.board_vendor = dmi.board_vendor.take().or_else(|| s(0x04));
                dmi.board_name = dmi.board_name.take().or_else(|| s(0x05));
                dmi.board_serial = dmi.board_serial.take().or_else(|| s(0x07));
            }
            127 => break,
            _ => {}
        }
        i = j;
    }
    dmi
}

#[cfg(target_os = "linux")]
fn read_drives() -> Vec<Drive> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir("/sys/block") else { return out };
    let mut names: Vec<String> = dir.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        if ["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd"].iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let read = |f: &str| std::fs::read_to_string(format!("/sys/block/{name}/{f}")).map(|s| s.trim().to_string()).unwrap_or_default();
        let bytes = read("size").parse::<u64>().unwrap_or(0) * 512;
        if bytes == 0 {
            continue;
        }
        let model = read("device/model");
        let kind = if read("removable") == "1" || std::fs::canonicalize(format!("/sys/block/{name}")).is_ok_and(|p| p.to_string_lossy().contains("/usb")) {
            "USB"
        } else if name.starts_with("nvme") {
            "NVMe SSD"
        } else if name.starts_with("mmcblk") {
            "eMMC"
        } else if read("queue/rotational") == "1" {
            "HDD"
        } else {
            "SSD"
        };
        out.push(Drive { name: if model.is_empty() { name } else { model }, bytes, kind });
    }
    out
}

#[cfg(not(target_os = "linux"))]
fn read_drives() -> Vec<Drive> {
    use sysinfo::{DiskKind, Disks};
    let Ok(disks) = Disks::new_with_refreshed_list() else { return Vec::new() };
    disks
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| Drive {
            name: d.mount_point().display().to_string(),
            bytes: d.total_space(),
            kind: if d.is_removable() {
                "USB"
            } else {
                match d.kind() {
                    DiskKind::SSD => "SSD",
                    DiskKind::HDD => "HDD",
                    _ => "",
                }
            },
        })
        .collect()
}

fn collect() -> Info {
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()).with_memory(MemoryRefreshKind::nothing().with_ram()),
    )
    .ok();
    let cpus = sys.as_ref().map_or(&[][..], |s| s.cpus());
    let dmi = read_dmi();
    let os = match (System::long_os_version().ok(), System::kernel_version().ok()) {
        (Some(l), Some(k)) => format!("{l} · {k}"),
        (Some(l), None) => l,
        (None, _) => std::env::consts::OS.to_string(),
    };
    Info {
        model: dmi.model(),
        serial: dmi.serial(),
        bios: dmi.bios(),
        os,
        cpu: cpus.first().map(|c| c.brand().replace("(R)", "").replace("(TM)", "").replace("(tm)", "").split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty()),
        cores: System::physical_core_count().ok(),
        threads: cpus.len(),
        ram_bytes: sys.as_ref().map_or(0, |s| s.total_memory()),
        drives: read_drives(),
    }
}

fn read_batteries() -> Result<Vec<Battery>, String> {
    if let Ok(demo) = std::env::var("NYANSPECT_BATTERY_DEMO") {
        return Ok(demo_batteries(&demo));
    }
    use starship_battery::units::electric_potential::volt;
    use starship_battery::units::energy::watt_hour;
    use starship_battery::units::power::watt;
    use starship_battery::units::ratio::ratio;
    use starship_battery::units::thermodynamic_temperature::degree_celsius;
    use starship_battery::units::time::second;
    use starship_battery::State;
    let manager = starship_battery::Manager::new().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for b in manager.batteries().map_err(|e| e.to_string())? {
        let Ok(b) = b else { continue };
        let design = b.energy_full_design().get::<watt_hour>();
        let full = b.energy_full().get::<watt_hour>();
        // Wireless mice and headsets show up too, without any capacity: skip them.
        if design <= 0.0 && full <= 0.0 {
            continue;
        }
        let name = [b.vendor(), b.model()].iter().flatten().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        let text = |v: Option<&str>| v.and_then(clean);
        let state = b.state();
        let power = b.energy_rate().get::<watt>().abs();
        let minutes = match state {
            State::Charging => b.time_to_full(),
            State::Discharging => b.time_to_empty(),
            _ => None,
        }
        .map(|t| (t.get::<second>() / 60.0).round() as u32)
        .filter(|m| *m > 0 && *m < 24 * 60);
        let volts = b.voltage().get::<volt>();
        let temp = b.temperature().map(|t| t.get::<degree_celsius>()).filter(|c| *c > -30.0 && *c < 120.0);
        out.push(Battery {
            vendor: text(b.vendor()),
            model: text(b.model()),
            serial: text(b.serial_number()),
            chemistry: chemistry(b.technology()),
            voltage: (volts > 0.5).then_some(volts),
            power_w: (power > 0.05 && matches!(state, State::Charging | State::Discharging)).then_some(power),
            temperature_c: temp,
            minutes_left: minutes,
            charge_limit: None,
            name,
            health: (design > 0.1).then(|| full / design),
            full_wh: full,
            design_wh: design,
            charge: b.state_of_charge().get::<ratio>(),
            state: match state {
                State::Charging => "Charging",
                State::Discharging => "On battery",
                State::Full => "Fully charged",
                State::Empty => "Empty",
                // Plugged in and topped up, or held at a charge limit.
                _ if b.state_of_charge().get::<ratio>() >= 0.99 => "Fully charged",
                _ => "Plugged in, not charging",
            },
            cycles: b.cycle_count(),
        });
    }
    // The charge limit isn't in the battery's own data; Linux shows it in sysfs (only
    // matched up when there's exactly one battery).
    if out.len() == 1 {
        out[0].charge_limit = linux_charge_limit();
    }
    Ok(out)
}

fn chemistry(t: starship_battery::Technology) -> Option<&'static str> {
    use starship_battery::Technology as T;
    Some(match t {
        T::LithiumIon => "Li-ion",
        T::LithiumPolymer => "Li-polymer",
        T::LithiumIronPhosphate => "LiFePO4",
        T::NickelMetalHydride => "NiMH",
        T::NickelCadmium => "NiCd",
        T::NickelZinc => "NiZn",
        T::LeadAcid => "Lead-acid",
        T::RechargeableAlkalineManganese => "Alkaline",
        _ => return None,
    })
}

#[cfg(target_os = "linux")]
fn linux_charge_limit() -> Option<u32> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    dir.flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("BAT"))
        .find_map(|e| std::fs::read_to_string(e.path().join("charge_control_end_threshold")).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|l| *l > 0 && *l < 100)
}

#[cfg(not(target_os = "linux"))]
fn linux_charge_limit() -> Option<u32> {
    None
}

/// Dev: NYANSPECT_BATTERY_DEMO=good|worn|none for screenshots.
fn demo_batteries(kind: &str) -> Vec<Battery> {
    let (full, cycles) = match kind {
        "none" => return Vec::new(),
        "worn" => (34.1, 812),
        _ => (47.6, 213),
    };
    vec![Battery {
        name: "Demo Battery A32".into(),
        health: Some(full / 50.0),
        full_wh: full,
        design_wh: 50.0,
        charge: 0.64,
        state: "Charging",
        cycles: Some(cycles),
        vendor: Some("Demo Co.".into()),
        model: Some("A32N1A07".into()),
        serial: Some("0C2F-9917".into()),
        chemistry: Some("Li-polymer"),
        voltage: Some(11.55),
        power_w: Some(21.4),
        temperature_c: Some(31.0),
        minutes_left: Some(48),
        charge_limit: Some(80),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny SMBIOS table: BIOS, system and baseboard structures, then the end marker.
    fn table() -> Vec<u8> {
        let mut t = Vec::new();
        // Type 0: vendor=1, version=2, date=3.
        t.extend([0, 0x12, 0, 0, 1, 2, 0, 0, 3]);
        t.extend([0u8; 0x12 - 9]);
        t.extend(b"Acme BIOS\0X515JP.309\009/05/2022\0\0");
        // Type 1: manufacturer=1, product=2, version=3, serial=4.
        t.extend([1, 0x08, 1, 0, 1, 2, 3, 4]);
        t.extend(b"LENOVO\021AH00ABCD\0ThinkPad T14 Gen 3\0PF3ABCDE\0\0");
        // Type 2: placeholder board strings.
        t.extend([2, 0x08, 2, 0, 1, 2, 0, 3]);
        t.extend(b"LENOVO\021AH00ABCD\0Default string\0\0");
        // Type 3 with no strings, then the end.
        t.extend([3, 0x04, 3, 0, 0, 0]);
        t.extend([127, 0x04, 4, 0, 0, 0]);
        t
    }

    #[test]
    fn smbios_strings_are_read() {
        let d = parse_smbios(&table());
        assert_eq!(d.bios_version.as_deref(), Some("X515JP.309"));
        assert_eq!(d.bios_date.as_deref(), Some("09/05/2022"));
        assert_eq!(d.serial.as_deref(), Some("PF3ABCDE"));
        assert_eq!(d.board_serial, None, "placeholder text is dropped");
        assert_eq!(d.model().as_deref(), Some("Lenovo ThinkPad T14 Gen 3 (21AH00ABCD)"));
        assert_eq!(d.serial(), Serial::Known("PF3ABCDE".into()));
    }

    #[test]
    fn broken_tables_do_not_panic() {
        let t = table();
        for n in 0..t.len() {
            parse_smbios(&t[..n]);
        }
        parse_smbios(&[1, 0, 0, 0, 0]);
        parse_smbios(&[1, 200, 0, 0]);
    }

    #[test]
    fn model_names() {
        let asus = Dmi {
            vendor: clean("ASUSTeK COMPUTER INC."),
            product: clean("VivoBook_ASUSLaptop X515JP_A516JP"),
            version: clean("1.0       "),
            ..Dmi::default()
        };
        assert_eq!(asus.model().as_deref(), Some("ASUS VivoBook ASUSLaptop X515JP A516JP"));
        let desktop = Dmi {
            vendor: clean("System manufacturer"),
            product: clean("System Product Name"),
            board_vendor: clean("ASUSTeK COMPUTER INC."),
            board_name: clean("TUF GAMING B660-PLUS WIFI D4"),
            ..Dmi::default()
        };
        assert_eq!(desktop.model().as_deref(), Some("ASUS TUF GAMING B660-PLUS WIFI D4 (motherboard)"));
        let hp = Dmi { vendor: clean("HP"), product: clean("HP EliteBook 840 G8"), ..Dmi::default() };
        assert_eq!(hp.model().as_deref(), Some("HP EliteBook 840 G8"));
    }

    #[test]
    fn serial_fallbacks() {
        let denied = Dmi { serial_denied: true, board_serial: Some("B1".into()), ..Dmi::default() };
        assert_eq!(denied.serial(), Serial::NeedsAdmin);
        let board = Dmi { board_serial: Some("B1".into()), ..Dmi::default() };
        assert_eq!(board.serial(), Serial::Known("B1".into()));
        assert_eq!(Dmi::default().serial(), Serial::NotSet);
    }

    #[test]
    fn memory_rounds_to_a_real_size() {
        assert_eq!(installed_gb(15.5), 16);
        assert_eq!(installed_gb(7.7), 8);
        assert_eq!(installed_gb(16.0), 16);
        assert_eq!(installed_gb(9.6), 12);
        assert_eq!(installed_gb(31.2), 32);
    }

    #[test]
    fn durations() {
        assert_eq!(duration_text(48), "48 min");
        assert_eq!(duration_text(135), "2 h 15 min");
    }

    #[test]
    fn worn_below_eighty_percent() {
        let worn = &demo_batteries("worn")[0];
        let good = &demo_batteries("good")[0];
        assert!(worn.worn() && !good.worn());
    }
}
