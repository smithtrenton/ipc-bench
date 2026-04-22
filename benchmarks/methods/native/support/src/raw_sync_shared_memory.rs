use std::{
    error::Error,
    io::{self, Write},
    mem::{align_of, size_of},
    sync::atomic::{AtomicBool, Ordering, fence},
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};
use raw_sync::{
    Timeout,
    events::{BusyEvent, Event, EventInit, EventState},
};
use shared_memory::{Shmem, ShmemConf};

use crate::util::{slice_from_raw_parts, slice_from_raw_parts_mut, unique_name};

const ENV_MAPPING_NAME: &str = "IPC_BENCH_RAW_SYNC_MAPPING";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawSyncMethod {
    Event,
    Busy,
}

#[repr(C)]
struct RawSyncHeader {
    stop: AtomicBool,
}

struct RawSyncLayout {
    request_event_offset: usize,
    response_event_offset: usize,
    request_offset: usize,
    response_offset: usize,
    total_size: usize,
}

pub fn run_raw_sync_shared_memory(method: RawSyncMethod) -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    match config.role {
        ProcessRole::Parent => match method {
            RawSyncMethod::Event => run_parent::<Event>(config, method_name(method)),
            RawSyncMethod::Busy => run_parent::<BusyEvent>(config, method_name(method)),
        },
        ProcessRole::Child => match method {
            RawSyncMethod::Event => run_child::<Event>(config),
            RawSyncMethod::Busy => run_child::<BusyEvent>(config),
        },
    }
}

fn run_parent<E>(config: BenchmarkConfig, method: &'static str) -> Result<(), Box<dyn Error>>
where
    E: EventInit,
{
    let mapping_name = unique_name(method);
    let layout = raw_sync_layout::<E>(config.message_size);
    let mut mapping = ShmemConf::new()
        .os_id(mapping_name.as_str())
        .size(layout.total_size)
        .create()?;

    header_mut(&mut mapping)
        .stop
        .store(false, Ordering::Release);
    let request_event =
        unsafe { E::new(mapping.as_ptr().add(layout.request_event_offset), true)? }.0;
    let response_event =
        unsafe { E::new(mapping.as_ptr().add(layout.response_event_offset), true)? }.0;

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[(ENV_MAPPING_NAME, mapping_name.as_str())],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }

    let mut outbound = vec![0_u8; config.message_size];
    let mut inbound = vec![0_u8; config.message_size];
    for (index, byte) in outbound.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }

    let report = run_benchmark(method, &config, true, || {
        request_slice_mut(&mut mapping, &layout).copy_from_slice(outbound.as_slice());
        fence(Ordering::Release);
        request_event
            .set(EventState::Signaled)
            .expect("request event should signal");
        response_event
            .wait(Timeout::Infinite)
            .expect("response wait should succeed");
        fence(Ordering::Acquire);
        inbound.copy_from_slice(response_slice(&mapping, &layout));
        if !outbound.is_empty() {
            outbound.copy_from_slice(inbound.as_slice());
            outbound[0] = outbound[0].wrapping_add(1);
        }
    });

    header_mut(&mut mapping).stop.store(true, Ordering::Release);
    request_event.set(EventState::Signaled)?;
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child<E>(config: BenchmarkConfig) -> Result<(), Box<dyn Error>>
where
    E: EventInit,
{
    let mapping_name = std::env::var(ENV_MAPPING_NAME)?;
    let mut mapping = ShmemConf::new().os_id(mapping_name.as_str()).open()?;
    let layout = raw_sync_layout::<E>(config.message_size);
    let request_event =
        unsafe { E::from_existing(mapping.as_ptr().add(layout.request_event_offset))? }.0;
    let response_event =
        unsafe { E::from_existing(mapping.as_ptr().add(layout.response_event_offset))? }.0;

    println!("ready");
    io::stdout().flush()?;

    let mut scratch = vec![0_u8; config.message_size];
    loop {
        request_event.wait(Timeout::Infinite)?;
        fence(Ordering::Acquire);
        if header(&mapping).stop.load(Ordering::Acquire) {
            return Ok(());
        }

        scratch.copy_from_slice(request_slice(&mapping, &layout));
        if !scratch.is_empty() {
            scratch[0] = scratch[0].wrapping_add(1);
        }
        response_slice_mut(&mut mapping, &layout).copy_from_slice(scratch.as_slice());
        fence(Ordering::Release);
        response_event.set(EventState::Signaled)?;
    }
}

fn raw_sync_layout<E>(message_size: usize) -> RawSyncLayout
where
    E: EventInit,
{
    let header_size = size_of::<RawSyncHeader>();
    let request_event_offset = align_up(header_size, align_of::<usize>());
    let request_event_size = E::size_of(None);
    let response_event_offset = align_up(
        request_event_offset + request_event_size,
        align_of::<usize>(),
    );
    let response_event_size = E::size_of(None);
    let request_offset = response_event_offset + response_event_size;
    let response_offset = request_offset + message_size;
    let total_size = response_offset + message_size;

    RawSyncLayout {
        request_event_offset,
        response_event_offset,
        request_offset,
        response_offset,
        total_size,
    }
}

fn align_up(offset: usize, alignment: usize) -> usize {
    (offset + (alignment - 1)) & !(alignment - 1)
}

fn header(mapping: &Shmem) -> &RawSyncHeader {
    unsafe { &*(mapping.as_ptr().cast::<RawSyncHeader>()) }
}

fn header_mut(mapping: &mut Shmem) -> &mut RawSyncHeader {
    unsafe { &mut *(mapping.as_ptr().cast::<RawSyncHeader>()) }
}

fn request_slice<'a>(mapping: &'a Shmem, layout: &RawSyncLayout) -> &'a [u8] {
    unsafe {
        slice_from_raw_parts(
            mapping.as_ptr().add(layout.request_offset),
            layout.response_offset - layout.request_offset,
        )
    }
}

fn request_slice_mut<'a>(mapping: &'a mut Shmem, layout: &RawSyncLayout) -> &'a mut [u8] {
    unsafe {
        slice_from_raw_parts_mut(
            mapping.as_ptr().add(layout.request_offset),
            layout.response_offset - layout.request_offset,
        )
    }
}

fn response_slice<'a>(mapping: &'a Shmem, layout: &RawSyncLayout) -> &'a [u8] {
    unsafe {
        slice_from_raw_parts(
            mapping.as_ptr().add(layout.response_offset),
            layout.total_size - layout.response_offset,
        )
    }
}

fn response_slice_mut<'a>(mapping: &'a mut Shmem, layout: &RawSyncLayout) -> &'a mut [u8] {
    unsafe {
        slice_from_raw_parts_mut(
            mapping.as_ptr().add(layout.response_offset),
            layout.total_size - layout.response_offset,
        )
    }
}

fn method_name(method: RawSyncMethod) -> &'static str {
    match method {
        RawSyncMethod::Event => "shm-raw-sync-event",
        RawSyncMethod::Busy => "shm-raw-sync-busy",
    }
}
