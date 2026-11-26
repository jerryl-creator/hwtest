// Embeds the app icon and version info into the Windows .exe (Explorer, taskbar, Alt-Tab).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Nyanspect");
        res.set("FileDescription", "Nyanspect - keyboard, dead pixel and speaker tester");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
