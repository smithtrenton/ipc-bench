fn main() {
    if let Err(error) = support::run_iceoryx2_publish_subscribe() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
