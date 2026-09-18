use keyring::Entry;
fn main() {
    let entry = Entry::new("ConnectSync", "google_token").unwrap();
    entry.set_password("test").unwrap();
    println!("Set: {:?}", entry.get_password());
    entry.delete_credential().unwrap();
    println!("Deleted: {:?}", entry.get_password());
}
