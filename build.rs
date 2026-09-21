fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/theme.slint");
    println!("cargo:rerun-if-changed=ui/emojis/");
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    
    slint_build::compile("ui/main.slint").unwrap();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app_icon.ico");
        res.compile().unwrap();
    }
}
