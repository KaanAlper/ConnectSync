slint::slint! {
    export component Dummy inherits Window {}
}
fn main() {
    let dummy = Dummy::new().unwrap();
    // dummy.show().unwrap(); // Not shown
    
    // Slint will quit immediately if there are no visible windows!
    slint::run_event_loop_until_quit().unwrap();
    println!("Exited");
}
