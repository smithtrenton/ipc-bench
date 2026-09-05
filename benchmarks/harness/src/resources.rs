use serde::Serialize;
use std::sync::Mutex;
static CHILD_CPU: Mutex<Option<f64>> = Mutex::new(None);
static CHILD_AFFINITY: Mutex<Option<usize>> = Mutex::new(None);
thread_local! {
    static SYNC_COUNTS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}
pub fn record_signal() {
    let (signals, waits) = SYNC_COUNTS.get();
    SYNC_COUNTS.set((signals + 1, waits));
}
pub fn record_wait() {
    let (signals, waits) = SYNC_COUNTS.get();
    SYNC_COUNTS.set((signals, waits + 1));
}

#[cfg(windows)]
pub(crate) fn cpu_seconds(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<f64> {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetProcessTimes};
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let ticks = |time: FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    Some((ticks(kernel) + ticks(user)) as f64 / 10_000_000.0)
}

pub(crate) fn record_child(child: &std::process::Child) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        *CHILD_CPU.lock().unwrap() = cpu_seconds(child.as_raw_handle());
        let mut mask = 0;
        let mut system = 0;
        if unsafe {
            windows_sys::Win32::System::Threading::GetProcessAffinityMask(
                child.as_raw_handle(),
                &mut mask,
                &mut system,
            )
        } != 0
        {
            *CHILD_AFFINITY.lock().unwrap() = Some(mask);
        }
    }
    #[cfg(not(windows))]
    let _ = child;
}

pub(crate) fn render_json(report: &impl Serialize) -> Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(report)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        value["parent_cpu_seconds"] =
            serde_json::json!(cpu_seconds(unsafe { GetCurrentProcess() }));
    }
    value["child_cpu_seconds"] = serde_json::json!(*CHILD_CPU.lock().unwrap());
    let (signals, waits) = SYNC_COUNTS.get();
    value["parent_explicit_signal_calls"] = serde_json::json!(signals);
    value["parent_explicit_wait_calls"] = serde_json::json!(waits);
    value["processor_topology"] = serde_json::json!(crate::affinity::topology().ok());
    value["effective_processor_group"] = serde_json::json!(crate::affinity::effective_group());
    value["effective_child_affinity"] = serde_json::json!(*CHILD_AFFINITY.lock().unwrap());
    value["cpu_time_scope"] = serde_json::json!("process-lifetime-through-shutdown");
    value["build_features"] = serde_json::json!({
        "padded_layout": cfg!(feature="padded-layout"),
        "copy_elision": cfg!(feature="copy-elision"),
        "borrowed_response": cfg!(feature="borrowed-response"),
        "conditional_wake": cfg!(feature="conditional-wake"),
        "cached_cursors": cfg!(feature="cached-cursors"),
    });
    serde_json::to_string_pretty(&value)
}
