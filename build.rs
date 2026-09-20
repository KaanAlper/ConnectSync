fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/emojis/");
    slint_build::compile("ui/main.slint").unwrap();
}
