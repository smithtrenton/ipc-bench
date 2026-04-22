fn main() {
    if let Err(error) = support::run_raw_sync_shared_memory(support::RawSyncMethod::Event) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
