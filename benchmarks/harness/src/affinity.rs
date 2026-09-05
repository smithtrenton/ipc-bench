use std::{env, io, process::Child};

use crate::ProcessRole;

const STABLE_AFFINITY_ENV: &str = "IPC_BENCH_STABLE_AFFINITY";

#[derive(Clone, Copy, Debug)]
struct StableAffinityPair {
    parent_mask: usize,
    child_mask: usize,
}

pub(crate) fn apply_child_affinity_if_configured(role: ProcessRole) -> io::Result<()> {
    if let Some(pair) = stable_affinity_pair()? {
        apply_current_process_affinity(if role == ProcessRole::Child {
            pair.child_mask
        } else {
            pair.parent_mask
        })?;
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
    if env::var("IPC_BENCH_TOPOLOGY").as_deref() == Ok("unpinned")
        && env::var_os("IPC_BENCH_CPU_PAIR").is_none()
    {
        return Ok(None);
    }
    if !stable_affinity_enabled()
        && env::var_os("IPC_BENCH_CPU_PAIR").is_none()
        && env::var_os("IPC_BENCH_TOPOLOGY").is_none()
    {
        return Ok(None);
    }

    let topology = topology()?;
    if topology.iter().any(|entry| entry.group != 0) {
        return Err(io::Error::other(
            "controlled process affinity currently supports processor group 0 only; use unpinned on multi-group hosts",
        ));
    }
    let cores: Vec<_> = topology
        .iter()
        .filter(|entry| entry.kind == "core")
        .map(|entry| entry.mask)
        .collect();
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "requested CPU pair/topology is unavailable",
        )
    };
    let (parent_mask, child_mask) = if let Ok(value) = env::var("IPC_BENCH_CPU_PAIR") {
        let bits = value
            .split(',')
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid())?;
        if bits.len() != 2 || bits[0] == bits[1] || bits.iter().any(|bit| *bit >= usize::BITS) {
            return Err(invalid());
        }
        let masks = (1usize << bits[0], 1usize << bits[1]);
        let active = cores.iter().fold(0, |a, b| a | b);
        if masks.0 & active == 0 || masks.1 & active == 0 {
            return Err(invalid());
        }
        masks
    } else {
        match env::var("IPC_BENCH_TOPOLOGY")
            .as_deref()
            .unwrap_or("separate-core")
        {
            "unpinned" => return Ok(None),
            "smt" => {
                let core = cores
                    .iter()
                    .find(|mask| mask.count_ones() >= 2)
                    .ok_or_else(invalid)?;
                let first = first_logical_processor_mask(*core).ok_or_else(invalid)?;
                (
                    first,
                    first_logical_processor_mask(*core & !first).ok_or_else(invalid)?,
                )
            }
            "separate-cache" => {
                let level = topology
                    .iter()
                    .filter_map(|e| e.cache_level)
                    .max()
                    .ok_or_else(invalid)?;
                let caches: Vec<_> = topology
                    .iter()
                    .filter(|e| e.cache_level == Some(level))
                    .collect();
                let first = caches.first().ok_or_else(invalid)?.mask;
                let second = caches
                    .iter()
                    .find(|e| e.mask & first == 0)
                    .ok_or_else(invalid)?
                    .mask;
                (
                    first_logical_processor_mask(first).ok_or_else(invalid)?,
                    first_logical_processor_mask(second).ok_or_else(invalid)?,
                )
            }
            "separate-core" => {
                let level = topology
                    .iter()
                    .filter_map(|e| e.cache_level)
                    .max()
                    .ok_or_else(invalid)?;
                topology
                    .iter()
                    .filter(|e| e.cache_level == Some(level))
                    .find_map(|cache| {
                        let mut sharing = cores
                            .iter()
                            .map(|core| core & cache.mask)
                            .filter(|mask| *mask != 0);
                        Some((
                            first_logical_processor_mask(sharing.next()?)?,
                            first_logical_processor_mask(sharing.next()?)?,
                        ))
                    })
                    .ok_or_else(invalid)?
            }
            _ => return Err(invalid()),
        }
    };

    Ok(Some(StableAffinityPair {
        parent_mask,
        child_mask,
    }))
}

fn first_logical_processor_mask(core_mask: usize) -> Option<usize> {
    (core_mask != 0).then(|| 1usize << core_mask.trailing_zeros())
}

#[derive(serde::Serialize)]
pub(crate) struct CpuTopology {
    kind: String,
    group: u16,
    mask: usize,
    cache_level: Option<u8>,
}

#[cfg(windows)]
pub(crate) fn topology() -> io::Result<Vec<CpuTopology>> {
    use windows_sys::Win32::System::SystemInformation::{
        GetLogicalProcessorInformationEx, RelationCache, RelationProcessorCore,
    };
    let mut result = Vec::new();
    for relation in [RelationProcessorCore, RelationCache] {
        let mut bytes = 0u32;
        unsafe {
            GetLogicalProcessorInformationEx(relation, std::ptr::null_mut(), &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut storage = vec![0u64; (bytes as usize).div_ceil(8)];
        if unsafe {
            GetLogicalProcessorInformationEx(relation, storage.as_mut_ptr().cast(), &mut bytes)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let data =
            unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), bytes as usize) };
        let mut offset = 0;
        while offset < data.len() {
            let bad = || {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Windows CPU topology record",
                )
            };
            if data.len() - offset < 8 {
                return Err(bad());
            }
            let size =
                u32::from_ne_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
            if size < 8 || size > data.len() - offset {
                return Err(bad());
            }
            let record = &data[offset..offset + size];
            let (count_offset, masks_offset) = if relation == RelationProcessorCore {
                (30, 32)
            } else {
                (38, 40)
            };
            if record.len() < masks_offset {
                return Err(bad());
            }
            let count =
                u16::from_ne_bytes(record[count_offset..count_offset + 2].try_into().unwrap())
                    as usize;
            if count == 0 || masks_offset + count * 16 > record.len() {
                return Err(bad());
            }
            for index in 0..count {
                let start = masks_offset + index * 16;
                let mask =
                    u64::from_ne_bytes(record[start..start + 8].try_into().unwrap()) as usize;
                let group = u16::from_ne_bytes(record[start + 8..start + 10].try_into().unwrap());
                result.push(CpuTopology {
                    kind: if relation == RelationProcessorCore {
                        "core"
                    } else {
                        "cache"
                    }
                    .into(),
                    group,
                    mask,
                    cache_level: (relation == RelationCache).then_some(record[8]),
                });
            }
            offset += size;
        }
    }
    Ok(result)
}
#[cfg(not(windows))]
pub(crate) fn topology() -> io::Result<Vec<CpuTopology>> {
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

#[cfg(windows)]
pub(crate) fn effective_mask() -> io::Result<Option<usize>> {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessAffinityMask};
    let mut process = 0;
    let mut system = 0;
    if unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut process, &mut system) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Some(process))
}

#[cfg(windows)]
pub(crate) fn effective_group() -> Option<u16> {
    use windows_sys::Win32::System::{
        SystemInformation::GROUP_AFFINITY,
        Threading::{GetCurrentThread, GetThreadGroupAffinity},
    };
    let mut affinity = GROUP_AFFINITY::default();
    (unsafe { GetThreadGroupAffinity(GetCurrentThread(), &mut affinity) } != 0)
        .then_some(affinity.Group)
}
#[cfg(not(windows))]
pub(crate) fn effective_group() -> Option<u16> {
    None
}
#[cfg(not(windows))]
pub(crate) fn effective_mask() -> io::Result<Option<usize>> {
    Ok(None)
}
