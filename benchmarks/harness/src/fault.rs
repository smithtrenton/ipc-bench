//! Opt-in fault injection for the cross-process contract gate, never enabled by runners.
use std::{sync::OnceLock, time::Duration};

fn fault() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    VALUE.get_or_init(|| std::env::var("IPC_BENCH_TEST_FAULT").unwrap_or_default())
}

pub(crate) fn worker_started() {
    match fault() {
        "before-ready" => std::process::exit(42),
        "mid-request" => {
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                std::process::exit(42);
            });
        }
        _ => {}
    }
}

pub fn worker_finished() {
    if fault() == "shutdown" && std::env::args().any(|arg| arg == "child") {
        loop {
            std::thread::park();
        }
    }
}

pub fn transform_response(payload: &mut [u8]) {
    payload[0] = payload[0].wrapping_add(1);
    if fault() == "corrupt" {
        *payload.last_mut().unwrap() ^= 1;
    }
}
