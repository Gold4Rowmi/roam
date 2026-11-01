fn main() {
    // the exe icon: logo/app-icons style, Rommel on the Mint shell at whole-pixel sizes
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("roam.ico");
        r.compile().unwrap();
    }
    println!("cargo:rerun-if-changed=roam.ico");
}
