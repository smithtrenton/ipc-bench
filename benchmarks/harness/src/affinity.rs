use std::{env, io, process::Child};

use crate::ProcessRole;

const STABLE_AFFINITY_ENV: &str = "IPC_BENCH_STABLE_AFFINITY";

#[derive(Clone, Copy, Debug)]
struct StableAffinityPair {
    parent_mask: usize,
    child_mask: usize,
}

pub(crate) fn apply_child_affinity_if_configured(role: ProcessRole) -> io::Result<()> {
    if role != ProcessRole::Child {
        return Ok(());
    }

    if let Some(pair) = stable_affinity_pair()? {
        apply_current_process_affinity(pair.child_mask)?;
    }

    Ok(())
}

pub(crate) fn apply_parent_and_child_affinity_if_configured(child: &Child) -> io::Result<()> {
    if let Some(pair) = stable_affinity_pair()? {
        apply_child_process_affinity(child, pair.child_mask)?;
        apply_current_process_affinity(pair.parent_mask)?;
    }

    Ok(())
}

fn stable_affinity_enabled() -> bool {
    env::var(STABLE_AFFINITY_ENV)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "" | "0" | "false" | "no" | "off")
        })
        .unwrap_or(false)
}

fn stable_affinity_pair() -> io::Result<Option<StableAffinityPair>> {
    if !stable_affinity_enabled() {
        return Ok(None);
    }

    let core_masks = processor_core_masks()?;
    let parent_mask = core_masks
        .first()
        .and_then(|mask| first_logical_processor_mask(*mask))
        .ok_or_else(|| io::Error::other("failed to resolve parent CPU affinity"))?;
    let child_mask = core_masks
        .iter()
        .skip(1)
        .find_map(|mask| first_logical_processor_mask(*mask))
        .ok_or_else(|| {
            io::Error::other(
                "stable affinity requires at least two physical CPU cores with addressable logical processors",
            )
        })?;

    Ok(Some(StableAffinityPair {
        parent_mask,
        child_mask,
    }))
}

fn first_logical_processor_mask(core_mask: usize) -> Option<usize> {
    (core_mask != 0).then(|| 1usize << core_mask.trailing_zeros())
}

#[cfg(windows)]
fn processor_core_masks() -> io::Result<Vec<usize>> {
    use std::mem::size_of;

    use windows_sys::Win32::System::SystemInformation::{
        GetLogicalProcessorInformation, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
    };

    let mut bytes = 0u32;
    unsafe {
        let _ = GetLogicalProcessorInformation(std::ptr::null_mut(), &mut bytes);
    }
    if bytes == 0 {
        return Err(io::Error::last_os_error());
    }

    let entry_size = size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
    let capacity = (bytes as usize).div_ceil(entry_size);
    let mut entries = vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); capacity];
    let success = unsafe { GetLogicalProcessorInformation(entries.as_mut_ptr(), &mut bytes) };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }

    let entry_count = bytes as usize / entry_size;
    entries.truncate(entry_count);

    Ok(entries
        .into_iter()
        .filter(|entry| entry.Relationship == RelationProcessorCore)
        .map(|entry| entry.ProcessorMask)
        .filter(|mask| *mask != 0)
        .collect())
}

#[cfg(not(windows))]
fn processor_core_masks() -> io::Result<Vec<usize>> {
    Ok(Vec::new())
}

#[cfg(windows)]
fn apply_current_process_affinity(mask: usize) -> io::Result<()> {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessAffinityMask};

    let current_process = unsafe { GetCurrentProcess() };
    let success = unsafe { SetProcessAffinityMask(current_process, mask) };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(not(windows))]
fn apply_current_process_affinity(_mask: usize) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn apply_child_process_affinity(child: &Child, mask: usize) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Threading::SetProcessAffinityMask;

    let handle = child.as_raw_handle() as HANDLE;
    let success = unsafe { SetProcessAffinityMask(handle, mask) };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(not(windows))]
fn apply_child_process_affinity(_child: &Child, _mask: usize) -> io::Result<()> {
    Ok(())
}
