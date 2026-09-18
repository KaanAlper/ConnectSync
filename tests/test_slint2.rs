slint::slint! {
    export component Dummy inherits Window {}
}
fn main() {
    let dummy = Dummy::new().unwrap();
    drop(dummy);
    
    // Slint will quit immediately if there are no visible windows?
    slint::run_event_loop_until_quit().unwrap();
    println!("Exited");
}
