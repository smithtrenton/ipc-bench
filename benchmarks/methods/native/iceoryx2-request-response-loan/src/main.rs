fn main() {
    if let Err(error) = support::run_iceoryx2_request_response() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
