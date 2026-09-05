fn main() {
    if let Err(error) = support::run_pipe_iocp() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    support::worker_finished();
}
