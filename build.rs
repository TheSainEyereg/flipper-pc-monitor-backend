// Embeds the qFlipper icon into the Windows executable so it is shown in the
// taskbar / Explorer. No-op when building for other targets (e.g. Linux CI).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os == "windows")
        .unwrap_or(false)
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("qFlipper.ico");
        res.compile().expect("Failed to compile the Windows resources");
    }
}