use std::{
    error::Error,
    io::{self, Write},
    mem::size_of,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Memory::{
            CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
            OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
        },
        Threading::{
            CreateEventW, CreateSemaphoreW, EVENT_ALL_ACCESS, OpenEventW, OpenSemaphoreW,
            SEMAPHORE_ALL_ACCESS,
        },
    },
};

use crate::util::{
    LayoutHeader, OwnedHandle, mapping_size, release_semaphore, set_event, slice_from_raw_parts,
    slice_from_raw_parts_mut, unique_name, wait_for_signal, wide_string,
};

const ENV_MAPPING: &str = "IPC_BENCH_MAPPING";
const ENV_REQ_A: &str = "IPC_BENCH_REQ_A";
const ENV_REQ_B: &str = "IPC_BENCH_REQ_B";
const ENV_REQ_C: &str = "IPC_BENCH_REQ_C";
const ENV_RESP_A: &str = "IPC_BENCH_RESP_A";
const ENV_RESP_B: &str = "IPC_BENCH_RESP_B";
const ENV_RESP_C: &str = "IPC_BENCH_RESP_C";

#[derive(Clone, Copy)]
pub enum WaitStrategy {
    Spin,
    Hybrid,
}

pub fn run_shm_events() -> Result<(), Box<dyn Error>> {
    run_mailbox("shm-events", MailboxMode::Events)
}

pub fn run_shm_semaphores() -> Result<(), Box<dyn Error>> {
    run_mailbox("shm-semaphores", MailboxMode::Semaphores)
}

pub fn run_shm_mailbox(strategy: WaitStrategy) -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_mailbox_wait_parent(config, strategy),
        ProcessRole::Child => run_mailbox_wait_child(config, strategy),
    }
}

pub fn run_shm_ring(strategy: WaitStrategy) -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_ring_parent(config, strategy),
        ProcessRole::Child => run_ring_child(config, strategy),
    }
}

#[derive(Clone, Copy)]
enum MailboxMode {
    Events,
    Semaphores,
}

fn run_mailbox(method: &str, mode: MailboxMode) -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_mailbox_parent(config, mode, method),
        ProcessRole::Child => run_mailbox_child(config, mode),
    }
}

fn run_mailbox_parent(
    config: BenchmarkConfig,
    mode: MailboxMode,
    method: &str,
) -> Result<(), Box<dyn Error>> {
    let mapping_name = format!(r"Local\{}", unique_name("ipc-bench-map"));
    let req_name = format!(r"Local\{}", unique_name("ipc-bench-req"));
    let resp_name = format!(r"Local\{}", unique_name("ipc-bench-resp"));

    let mut mapping = SharedMailbox::create(&mapping_name, config.wire_size())?;
    mapping.stop_flag().store(false, Ordering::Release);
    let request_sync = create_sync_object(mode, &req_name)?;
    let response_sync = create_sync_object(mode, &resp_name)?;

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[
            (ENV_MAPPING, mapping_name.clone()),
            (ENV_REQ_A, req_name.clone()),
            (ENV_RESP_A, resp_name.clone()),
        ],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(method, &config, true, || -> Result<(), Box<dyn Error>> {
        mapping.request_mut().copy_from_slice(&outbound);
        signal(mode, request_sync.raw())?;
        wait(mode, response_sync.raw())?;
        inbound.copy_from_slice(mapping.response());
        harness::check_response_and_advance(&mut outbound, &inbound)?;
        Ok(())
    })?;

    mapping.stop_flag().store(true, Ordering::Release);
    signal(mode, request_sync.raw())?;
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_mailbox_child(config: BenchmarkConfig, mode: MailboxMode) -> Result<(), Box<dyn Error>> {
    let mapping_name = std::env::var(ENV_MAPPING)?;
    let req_name = std::env::var(ENV_REQ_A)?;
    let resp_name = std::env::var(ENV_RESP_A)?;

    let mut mapping = SharedMailbox::open(&mapping_name, config.wire_size())?;
    let request_sync = open_sync_object(mode, &req_name)?;
    let response_sync = open_sync_object(mode, &resp_name)?;

    println!("ready");
    io::stdout().flush()?;

    let mut scratch = vec![0_u8; config.wire_size()];
    loop {
        wait(mode, request_sync.raw())?;
        if mapping.stop_flag().load(Ordering::Acquire) {
            return Ok(());
        }
        if cfg!(feature = "borrowed-response") {
            mapping.respond_directly();
        } else {
            scratch.copy_from_slice(mapping.request());
            if !scratch.is_empty() {
                harness::transform_response(&mut scratch);
            }
            mapping.response_mut().copy_from_slice(&scratch);
        }
        signal(mode, response_sync.raw())?;
    }
}

fn run_mailbox_wait_parent(
    config: BenchmarkConfig,
    strategy: WaitStrategy,
) -> Result<(), Box<dyn Error>> {
    let mapping_name = format!(r"Local\{}", unique_name("ipc-bench-mailbox"));
    let req_event = format!(r"Local\{}", unique_name("ipc-bench-mailbox-req"));
    let resp_event = format!(r"Local\{}", unique_name("ipc-bench-mailbox-resp"));

    let mut mapping = SharedMailbox::create(&mapping_name, config.wire_size())?;
    mapping.stop_flag().store(false, Ordering::Release);
    mapping.request_ready().store(false, Ordering::Release);
    mapping.response_ready().store(false, Ordering::Release);

    let request_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(create_event(&req_event)?)
    } else {
        None
    };
    let response_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(create_event(&resp_event)?)
    } else {
        None
    };

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[
            (ENV_MAPPING, mapping_name.clone()),
            (ENV_REQ_C, req_event.clone()),
            (ENV_RESP_C, resp_event.clone()),
        ],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let method = match strategy {
        WaitStrategy::Spin => "shm-mailbox-spin",
        WaitStrategy::Hybrid => "shm-mailbox-hybrid",
    };
    let report = run_benchmark(method, &config, true, || -> Result<(), Box<dyn Error>> {
        mapping.request_mut().copy_from_slice(&outbound);
        mapping.request_ready().store(true, Ordering::Release);
        if let Some(event) = &request_event {
            notify_receiver(event, &mapping.header().request_sleeping)?;
        }
        wait_for_mailbox_value(
            mapping.response_ready(),
            true,
            strategy,
            response_event.as_ref(),
            &mapping.header().response_sleeping,
        )?;
        inbound.copy_from_slice(mapping.response());
        mapping.response_ready().store(false, Ordering::Release);
        harness::check_response_and_advance(&mut outbound, &inbound)?;
        Ok(())
    })?;

    mapping.stop_flag().store(true, Ordering::Release);
    if let Some(event) = &request_event {
        set_event(event.raw())?;
    }
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_mailbox_wait_child(
    config: BenchmarkConfig,
    strategy: WaitStrategy,
) -> Result<(), Box<dyn Error>> {
    let mapping_name = std::env::var(ENV_MAPPING)?;
    let mut mapping = SharedMailbox::open(&mapping_name, config.wire_size())?;
    let request_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(open_event(&std::env::var(ENV_REQ_C)?)?)
    } else {
        None
    };
    let response_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(open_event(&std::env::var(ENV_RESP_C)?)?)
    } else {
        None
    };

    println!("ready");
    io::stdout().flush()?;

    let mut scratch = vec![0_u8; config.wire_size()];
    loop {
        if !wait_for_mailbox_value_or_stop(
            mapping.request_ready(),
            true,
            mapping.stop_flag(),
            strategy,
            request_event.as_ref(),
            &mapping.header().request_sleeping,
        )? {
            return Ok(());
        }
        if cfg!(feature = "borrowed-response") {
            mapping.respond_directly();
        } else {
            scratch.copy_from_slice(mapping.request());
            mapping.request_ready().store(false, Ordering::Release);
            if !scratch.is_empty() {
                harness::transform_response(&mut scratch);
            }
            mapping.response_mut().copy_from_slice(&scratch);
        }
        mapping.request_ready().store(false, Ordering::Release);
        mapping.response_ready().store(true, Ordering::Release);
        if let Some(event) = &response_event {
            notify_receiver(event, &mapping.header().response_sleeping)?;
        }
    }
}

fn run_ring_parent(config: BenchmarkConfig, strategy: WaitStrategy) -> Result<(), Box<dyn Error>> {
    let mapping_name = format!(r"Local\{}", unique_name("ipc-bench-ring"));
    let req_event = format!(r"Local\{}", unique_name("ipc-bench-ring-req"));
    let resp_event = format!(r"Local\{}", unique_name("ipc-bench-ring-resp"));

    let mut ring = SharedRing::create(&mapping_name, config.wire_size(), config.ring_capacity)?;
    let request_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(create_event(&req_event)?)
    } else {
        None
    };
    let response_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(create_event(&resp_event)?)
    } else {
        None
    };

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[
            (ENV_MAPPING, mapping_name.clone()),
            (ENV_REQ_B, req_event.clone()),
            (ENV_RESP_B, resp_event.clone()),
        ],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let method = match strategy {
        WaitStrategy::Spin => "shm-ring-spin",
        WaitStrategy::Hybrid => "shm-ring-hybrid",
    };
    if config.workload != "round-trip" {
        let mut sequence = 0u64;
        let mut buffers = WindowBuffers::new(config.wire_size(), config.queue_depth);
        let report = harness::run_throughput(method, &config, |count| {
            deliver_window(
                &mut ring,
                &config,
                count,
                &mut sequence,
                request_event.as_ref(),
                &mut buffers,
            )
        })?;
        ring.stop_flag().store(true, Ordering::Release);
        if let Some(event) = &request_event {
            set_event(event.raw())?;
        }
        child.request_shutdown();
        let status = child.wait()?;
        if !status.success() {
            return Err(format!("delivery worker exited with {status}").into());
        }
        print!("{}", report.render(config.output_format)?);
        return Ok(());
    }
    let report = run_benchmark(method, &config, true, || -> Result<(), Box<dyn Error>> {
        ring.push_request(&outbound);
        if let Some(event) = &request_event {
            notify_receiver(event, &ring.header().request_sleeping)?;
        }
        ring.pop_response(&mut inbound, strategy, response_event.as_ref())?;
        harness::check_response_and_advance(&mut outbound, &inbound)?;
        Ok(())
    })?;

    ring.stop_flag().store(true, Ordering::Release);
    if let Some(event) = &request_event {
        set_event(event.raw())?;
    }
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_ring_child(config: BenchmarkConfig, strategy: WaitStrategy) -> Result<(), Box<dyn Error>> {
    let mapping_name = std::env::var(ENV_MAPPING)?;
    let mut ring = SharedRing::open(&mapping_name, config.wire_size(), config.ring_capacity)?;
    let request_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(open_event(&std::env::var(ENV_REQ_B)?)?)
    } else {
        None
    };
    let response_event = if matches!(strategy, WaitStrategy::Hybrid) {
        Some(open_event(&std::env::var(ENV_RESP_B)?)?)
    } else {
        None
    };

    println!("ready");
    io::stdout().flush()?;

    let mut scratch = vec![0_u8; config.wire_size()];
    let mut sequence = 0u64;
    loop {
        if !ring.pop_request(&mut scratch, strategy, request_event.as_ref())? {
            return Ok(());
        }
        if config.workload == "streaming" {
            validate_stream_request(&scratch, sequence)?;
            sequence += 1;
            // Release delivery acknowledgement only after validation, independent of
            // the ring's read cursor (which acknowledges slot reuse, not correctness).
            ring.header()
                .delivered
                .store(sequence as usize, Ordering::Release);
            continue;
        }
        if !scratch.is_empty() {
            harness::transform_response(&mut scratch);
        }
        ring.push_response(&scratch);
        if let Some(event) = &response_event {
            notify_receiver(event, &ring.header().response_sleeping)?;
        }
    }
}

// Feature-gated layout experiment: compact is the control; padded aligns each
// owner/publication field and each payload/slot start to 64 bytes.
#[repr(C)]
#[cfg_attr(feature = "padded-layout", repr(align(64)))]
struct Control<T>(T);
impl<T> std::ops::Deref for Control<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

fn slot_stride(size: usize) -> io::Result<usize> {
    if cfg!(feature = "padded-layout") {
        size.checked_add(63)
            .map(|n| n & !63)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "slot stride overflow"))
    } else {
        Ok(size)
    }
}

#[repr(C)]
struct MailboxHeader {
    request_sleeping: Control<AtomicBool>,
    response_sleeping: Control<AtomicBool>,
    layout: LayoutHeader,
    stop: Control<AtomicBool>,
    request_ready: Control<AtomicBool>,
    response_ready: Control<AtomicBool>,
}

struct SharedMailbox {
    mapping: OwnedHandle,
    view: *mut u8,
    message_size: usize,
    stride: usize,
}

impl SharedMailbox {
    fn create(name: &str, message_size: usize) -> io::Result<Self> {
        let stride = slot_stride(message_size)?;
        let mapping_size = mapping_size(size_of::<MailboxHeader>(), stride, 1)?;
        let handle = create_mapping(name, mapping_size)?;
        let view = map_view(handle.raw(), mapping_size)?;
        unsafe {
            std::ptr::write_bytes(
                view.add(size_of::<MailboxHeader>()),
                0xA5,
                mapping_size - size_of::<MailboxHeader>(),
            );
        }
        let header = unsafe { &mut *(view.cast::<MailboxHeader>()) };
        header.layout = LayoutHeader::new(mapping_size, 1, message_size);
        header.stop.store(false, Ordering::Release);
        header.request_sleeping.store(false, Ordering::Relaxed);
        header.response_sleeping.store(false, Ordering::Relaxed);
        header.request_ready.store(false, Ordering::Release);
        header.response_ready.store(false, Ordering::Release);
        Ok(Self {
            mapping: handle,
            view,
            message_size,
            stride,
        })
    }

    fn open(name: &str, message_size: usize) -> io::Result<Self> {
        let stride = slot_stride(message_size)?;
        let mapping_size = mapping_size(size_of::<MailboxHeader>(), stride, 1)?;
        let handle = open_mapping(name)?;
        let view = map_view(handle.raw(), mapping_size)?;
        let owned = Self {
            mapping: handle,
            view,
            message_size,
            stride,
        };
        owned
            .header()
            .layout
            .validate(LayoutHeader::new(mapping_size, 1, message_size))?;
        Ok(owned)
    }

    fn respond_directly(&mut self) {
        // Parent owns request until publication; child owns response until its reply
        // publication. The two payload regions are disjoint and attachment was validated.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.view.add(size_of::<MailboxHeader>()),
                self.view.add(size_of::<MailboxHeader>() + self.stride),
                self.message_size,
            );
        }
        harness::transform_response(self.response_mut());
    }

    fn header(&self) -> &MailboxHeader {
        unsafe { &*(self.view.cast::<MailboxHeader>()) }
    }

    fn stop_flag(&self) -> &AtomicBool {
        &self.header().stop
    }

    fn request_ready(&self) -> &AtomicBool {
        &self.header().request_ready
    }

    fn response_ready(&self) -> &AtomicBool {
        &self.header().response_ready
    }

    fn request(&self) -> &[u8] {
        unsafe {
            slice_from_raw_parts(self.view.add(size_of::<MailboxHeader>()), self.message_size)
        }
    }

    fn request_mut(&mut self) -> &mut [u8] {
        unsafe {
            slice_from_raw_parts_mut(self.view.add(size_of::<MailboxHeader>()), self.message_size)
        }
    }

    fn response(&self) -> &[u8] {
        unsafe {
            slice_from_raw_parts(
                self.view.add(size_of::<MailboxHeader>() + self.stride),
                self.message_size,
            )
        }
    }

    fn response_mut(&mut self) -> &mut [u8] {
        unsafe {
            slice_from_raw_parts_mut(
                self.view.add(size_of::<MailboxHeader>() + self.stride),
                self.message_size,
            )
        }
    }
}

impl Drop for SharedMailbox {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.cast(),
            });
        }
        let _ = self.mapping.raw();
    }
}

#[repr(C)]
struct RingHeader {
    request_sleeping: Control<AtomicBool>,
    response_sleeping: Control<AtomicBool>,
    delivered: Control<AtomicUsize>,
    layout: LayoutHeader,
    stop: Control<AtomicBool>,
    request_write: Control<AtomicUsize>,
    request_read: Control<AtomicUsize>,
    response_write: Control<AtomicUsize>,
    response_read: Control<AtomicUsize>,
    capacity: usize,
    message_size: usize,
    stride: usize,
}

#[derive(Default)]
struct Cursor {
    owned: usize,
    peer: usize,
}

struct SharedRing {
    request_cursor: Cursor,
    response_cursor: Cursor,
    mapping: OwnedHandle,
    view: *mut u8,
}

struct RingQueue<'a> {
    cursor: &'a mut Cursor,
    stop: &'a AtomicBool,
    sleeping: &'a AtomicBool,
    write_index: &'a AtomicUsize,
    read_index: &'a AtomicUsize,
    capacity: usize,
    message_size: usize,
    base: *mut u8,
    stride: usize,
}

impl SharedRing {
    fn create(name: &str, message_size: usize, capacity: usize) -> io::Result<Self> {
        let stride = slot_stride(message_size)?;
        let total_size = mapping_size(size_of::<RingHeader>(), stride, capacity)?;
        let handle = create_mapping(name, total_size)?;
        let view = map_view(handle.raw(), total_size)?;
        unsafe {
            std::ptr::write_bytes(
                view.add(size_of::<RingHeader>()),
                0xA5,
                total_size - size_of::<RingHeader>(),
            );
        }
        let header = unsafe { &mut *(view.cast::<RingHeader>()) };
        header.layout = LayoutHeader::new(total_size, capacity, message_size);
        header.stop.store(false, Ordering::Release);
        header.request_sleeping.store(false, Ordering::Relaxed);
        header.response_sleeping.store(false, Ordering::Relaxed);
        header.request_write.store(0, Ordering::Release);
        header.request_read.store(0, Ordering::Release);
        header.response_write.store(0, Ordering::Release);
        header.response_read.store(0, Ordering::Release);
        header.delivered.store(0, Ordering::Release);
        header.capacity = capacity;
        header.message_size = message_size;
        header.stride = stride;
        Ok(Self {
            request_cursor: Cursor::default(),
            response_cursor: Cursor::default(),
            mapping: handle,
            view,
        })
    }

    fn open(name: &str, message_size: usize, capacity: usize) -> io::Result<Self> {
        let stride = slot_stride(message_size)?;
        let total_size = mapping_size(size_of::<RingHeader>(), stride, capacity)?;
        let handle = open_mapping(name)?;
        let view = map_view(handle.raw(), total_size)?;
        let owned = Self {
            request_cursor: Cursor::default(),
            response_cursor: Cursor::default(),
            mapping: handle,
            view,
        };
        owned
            .header()
            .layout
            .validate(LayoutHeader::new(total_size, capacity, message_size))?;
        if owned.header().capacity != capacity
            || owned.header().message_size != message_size
            || owned.header().stride != stride
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ring metadata mismatch",
            ));
        }
        Ok(owned)
    }

    fn header(&self) -> &RingHeader {
        unsafe { &*(self.view.cast::<RingHeader>()) }
    }

    fn stop_flag(&self) -> &AtomicBool {
        &self.header().stop
    }

    fn request_base(&self) -> *mut u8 {
        unsafe { self.view.add(size_of::<RingHeader>()) }
    }

    fn response_base(&self) -> *mut u8 {
        unsafe {
            self.request_base()
                .add(self.header().capacity * self.header().stride)
        }
    }

    fn request_queue(&mut self) -> RingQueue<'_> {
        let base = self.request_base();
        // Mapping header and local cursors are disjoint. Only this process owns cursors.
        let header = unsafe { &*self.view.cast::<RingHeader>() };
        RingQueue {
            cursor: &mut self.request_cursor,
            stop: &header.stop,
            sleeping: &header.request_sleeping,
            write_index: &header.request_write,
            read_index: &header.request_read,
            capacity: header.capacity,
            message_size: header.message_size,
            stride: header.stride,
            base,
        }
    }
    fn response_queue(&mut self) -> RingQueue<'_> {
        let base = self.response_base();
        let header = unsafe { &*self.view.cast::<RingHeader>() };
        RingQueue {
            cursor: &mut self.response_cursor,
            stop: &header.stop,
            sleeping: &header.response_sleeping,
            write_index: &header.response_write,
            read_index: &header.response_read,
            capacity: header.capacity,
            message_size: header.message_size,
            stride: header.stride,
            base,
        }
    }
    fn push_request(&mut self, payload: &[u8]) {
        push_ring(self.request_queue(), payload);
    }
    fn push_response(&mut self, payload: &[u8]) {
        push_ring(self.response_queue(), payload);
    }
    fn pop_request(
        &mut self,
        buffer: &mut [u8],
        strategy: WaitStrategy,
        event: Option<&OwnedHandle>,
    ) -> io::Result<bool> {
        pop_ring(self.request_queue(), buffer, strategy, event)
    }
    fn pop_response(
        &mut self,
        buffer: &mut [u8],
        strategy: WaitStrategy,
        event: Option<&OwnedHandle>,
    ) -> io::Result<bool> {
        pop_ring(self.response_queue(), buffer, strategy, event)
    }
}

impl Drop for SharedRing {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.cast(),
            });
        }
        let _ = self.mapping.raw();
    }
}

fn push_ring(queue: RingQueue<'_>, payload: &[u8]) {
    loop {
        let write = if cfg!(feature = "cached-cursors") {
            queue.cursor.owned
        } else {
            queue.write_index.load(Ordering::Acquire)
        };
        let read = if cfg!(feature = "cached-cursors") {
            if !ring_has_space(write, queue.cursor.peer, queue.capacity) {
                queue.cursor.peer = queue.read_index.load(Ordering::Acquire);
            }
            queue.cursor.peer
        } else {
            queue.read_index.load(Ordering::Acquire)
        };
        if ring_has_space(write, read, queue.capacity) {
            let slot = ring_slot(write, queue.capacity);
            unsafe {
                slice_from_raw_parts_mut(queue.base.add(slot * queue.stride), queue.message_size)
                    .copy_from_slice(payload);
            }
            queue
                .write_index
                .store(write.wrapping_add(1), Ordering::Release);
            if cfg!(feature = "cached-cursors") {
                queue.cursor.owned = write.wrapping_add(1);
            }
            return;
        }
        std::hint::spin_loop();
    }
}

fn ring_slot(index: usize, capacity: usize) -> usize {
    if cfg!(feature = "cached-cursors") {
        index & (capacity - 1)
    } else {
        index % capacity
    }
}

fn pop_ring(
    queue: RingQueue<'_>,
    buffer: &mut [u8],
    strategy: WaitStrategy,
    event: Option<&OwnedHandle>,
) -> io::Result<bool> {
    let mut spins = 0_usize;
    loop {
        let read = if cfg!(feature = "cached-cursors") {
            queue.cursor.owned
        } else {
            queue.read_index.load(Ordering::Acquire)
        };
        let write = if cfg!(feature = "cached-cursors") {
            if queue.cursor.peer == read {
                queue.cursor.peer = queue.write_index.load(Ordering::Acquire);
            }
            queue.cursor.peer
        } else {
            queue.write_index.load(Ordering::Acquire)
        };
        if write != read {
            disarm_receiver(queue.sleeping);
            let slot = ring_slot(read, queue.capacity);
            unsafe {
                let slot_ptr = queue.base.add(slot * queue.stride);
                buffer.copy_from_slice(slice_from_raw_parts(slot_ptr, queue.message_size));
            }
            queue
                .read_index
                .store(read.wrapping_add(1), Ordering::Release);
            if cfg!(feature = "cached-cursors") {
                queue.cursor.owned = read.wrapping_add(1);
            }
            return Ok(true);
        }
        if queue.stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        wait_with_strategy(strategy, event, &mut spins, queue.sleeping)?;
    }
}

fn wait_for_mailbox_value(
    flag: &AtomicBool,
    target: bool,
    strategy: WaitStrategy,
    event: Option<&OwnedHandle>,
    sleeping: &AtomicBool,
) -> io::Result<()> {
    let mut spins = 0_usize;
    loop {
        if flag.load(Ordering::Acquire) == target {
            disarm_receiver(sleeping);
            return Ok(());
        }
        wait_with_strategy(strategy, event, &mut spins, sleeping)?;
    }
}

fn wait_for_mailbox_value_or_stop(
    flag: &AtomicBool,
    target: bool,
    stop_flag: &AtomicBool,
    strategy: WaitStrategy,
    event: Option<&OwnedHandle>,
    sleeping: &AtomicBool,
) -> io::Result<bool> {
    let mut spins = 0_usize;
    loop {
        if flag.load(Ordering::Acquire) == target {
            disarm_receiver(sleeping);
            return Ok(true);
        }
        if stop_flag.load(Ordering::Acquire) {
            return Ok(false);
        }
        wait_with_strategy(strategy, event, &mut spins, sleeping)?;
    }
}

fn wait_with_strategy(
    strategy: WaitStrategy,
    event: Option<&OwnedHandle>,
    spins: &mut usize,
    sleeping: &AtomicBool,
) -> io::Result<()> {
    match strategy {
        WaitStrategy::Spin => std::hint::spin_loop(),
        WaitStrategy::Hybrid => {
            let budget = spin_budget();
            if *spins < budget {
                *spins += 1;
                std::hint::spin_loop();
            } else if cfg!(feature = "conditional-wake") && *spins == budget {
                // SeqCst arm pairs with the producer's SeqCst exchange. Return to the
                // caller to recheck publication/stop before actually sleeping.
                // An exchange (not just a store) also acquires a preceding producer
                // exchange, so either we observe publication or the producer sees us armed.
                sleeping.swap(true, Ordering::SeqCst);
                *spins += 1;
                if force_yield() {
                    std::thread::yield_now();
                }
            } else if let Some(event) = event {
                wait_for_signal(event.raw())?;
                *spins = 0;
            } else {
                std::hint::spin_loop();
            }
        }
    }
    Ok(())
}

fn create_mapping(name: &str, size: usize) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            std::ptr::null(),
            PAGE_READWRITE,
            ((size as u64) >> 32) as u32,
            size as u32,
            name.as_ptr(),
        )
    };
    OwnedHandle::from_handle(handle)
}

fn open_mapping(name: &str) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, name.as_ptr()) };
    OwnedHandle::from_handle(handle)
}

fn map_view(mapping: HANDLE, size: usize) -> io::Result<*mut u8> {
    let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, size) };
    if view.Value.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(view.Value.cast())
    }
}

fn create_event(name: &str) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
    OwnedHandle::from_handle(handle)
}

fn open_event(name: &str) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe { OpenEventW(EVENT_ALL_ACCESS, 0, name.as_ptr()) };
    OwnedHandle::from_handle(handle)
}

fn create_semaphore(name: &str) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe { CreateSemaphoreW(std::ptr::null(), 0, 1, name.as_ptr()) };
    OwnedHandle::from_handle(handle)
}

fn open_semaphore(name: &str) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe { OpenSemaphoreW(SEMAPHORE_ALL_ACCESS, 0, name.as_ptr()) };
    OwnedHandle::from_handle(handle)
}

fn create_sync_object(mode: MailboxMode, name: &str) -> io::Result<OwnedHandle> {
    match mode {
        MailboxMode::Events => create_event(name),
        MailboxMode::Semaphores => create_semaphore(name),
    }
}

fn open_sync_object(mode: MailboxMode, name: &str) -> io::Result<OwnedHandle> {
    match mode {
        MailboxMode::Events => open_event(name),
        MailboxMode::Semaphores => open_semaphore(name),
    }
}

fn signal(mode: MailboxMode, handle: HANDLE) -> io::Result<()> {
    match mode {
        MailboxMode::Events => set_event(handle),
        MailboxMode::Semaphores => release_semaphore(handle),
    }
}

fn wait(mode: MailboxMode, handle: HANDLE) -> io::Result<()> {
    match mode {
        MailboxMode::Events | MailboxMode::Semaphores => wait_for_signal(handle),
    }
}

// SPSC ownership: only the producer writes an available slot; release of write_index
// publishes its bytes. The consumer reads only after acquire, then releases read_index
// before the producer may reuse that slot. References never outlive those operations.
fn ring_has_space(write: usize, read: usize, capacity: usize) -> bool {
    write.wrapping_sub(read) < capacity
}

fn validate_stream_request(request: &[u8], sequence: u64) -> io::Result<()> {
    if request.len() <= 8
        || request[..8] != sequence.to_le_bytes()
        || request[8..]
            .iter()
            .enumerate()
            .any(|(i, byte)| *byte != (i % 251) as u8)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("incorrect streaming delivery sequence={sequence}"),
        ));
    }
    Ok(())
}

struct WindowBuffers {
    started: Vec<Option<std::time::Instant>>,
    outbound: Vec<u8>,
    inbound: Vec<u8>,
    expected: Vec<u8>,
}
impl WindowBuffers {
    fn new(size: usize, depth: usize) -> Self {
        let mut outbound = vec![0; size];
        let mut expected = vec![0; size];
        harness::initialize_payload(&mut outbound);
        harness::initialize_payload(&mut expected);
        Self {
            started: vec![None; depth],
            outbound,
            inbound: vec![0; size],
            expected,
        }
    }
}

fn deliver_window(
    ring: &mut SharedRing,
    config: &BenchmarkConfig,
    count: usize,
    sequence: &mut u64,
    request_event: Option<&OwnedHandle>,
    buffers: &mut WindowBuffers,
) -> Result<(), Box<dyn Error>> {
    // All buffers are allocated and touched by preflight before the timed trial.
    let WindowBuffers {
        started,
        outbound,
        inbound,
        expected,
    } = buffers;
    let first_sequence = *sequence;
    let mut sent = 0;
    let mut received = 0;
    while received < count {
        if config.workload == "streaming" {
            let previous = received;
            received = ring
                .header()
                .delivered
                .load(Ordering::Acquire)
                .wrapping_sub(first_sequence as usize);
            if received > sent {
                return Err("invalid delivery acknowledgement".into());
            }
            for index in previous..received {
                harness::record_delivery_latency(started[index % config.queue_depth].take());
            }
        } else if ring.header().response_write.load(Ordering::Acquire)
            != ring.header().response_read.load(Ordering::Relaxed)
        {
            ring.pop_response(inbound, WaitStrategy::Spin, None)?;
            expected[..8].copy_from_slice(&(first_sequence + received as u64).to_le_bytes());
            harness::check_response_and_advance(expected, inbound)?;
            harness::record_delivery_latency(started[received % config.queue_depth].take());
            received += 1;
        }
        // Never block a producer while the opposite direction needs draining.
        if sent < count
            && sent - received < config.queue_depth
            && ring_has_space(
                ring.header().request_write.load(Ordering::Relaxed),
                ring.header().request_read.load(Ordering::Acquire),
                ring.header().capacity,
            )
        {
            outbound[..8].copy_from_slice(&(first_sequence + sent as u64).to_le_bytes());
            started[sent % config.queue_depth] = (first_sequence + sent as u64)
                .is_multiple_of(16)
                .then(std::time::Instant::now);
            ring.push_request(outbound);
            if let Some(event) = request_event {
                notify_receiver(event, &ring.header().request_sleeping)?;
            }
            sent += 1;
        } else {
            std::hint::spin_loop();
        }
    }
    *sequence += count as u64;
    Ok(())
}

fn disarm_receiver(sleeping: &AtomicBool) {
    if cfg!(feature = "conditional-wake") {
        sleeping.store(false, Ordering::SeqCst);
    }
}

fn notify_receiver(event: &OwnedHandle, sleeping: &AtomicBool) -> io::Result<()> {
    if !cfg!(feature = "conditional-wake") || sleeping.swap(false, Ordering::SeqCst) {
        set_event(event.raw())?;
    }
    Ok(())
}

fn spin_budget() -> usize {
    static VALUE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("IPC_BENCH_SPIN_BUDGET")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256)
            .min(1_000_000)
    })
}

fn force_yield() -> bool {
    static VALUE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| std::env::var_os("IPC_BENCH_TEST_YIELD").is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_capacity_one_and_wraparound() {
        for start in [0, usize::MAX - 1, usize::MAX] {
            assert!(ring_has_space(start, start, 1));
            assert!(!ring_has_space(start.wrapping_add(1), start, 1));
            assert!(ring_has_space(
                start.wrapping_add(1),
                start.wrapping_add(1),
                1
            ));
        }
    }

    #[test]
    fn actual_ring_slot_publication_wraps_at_counter_maximum() {
        let stop = AtomicBool::new(false);
        let sleeping = AtomicBool::new(false);
        for start in [usize::MAX - 1, usize::MAX] {
            let write = AtomicUsize::new(start);
            let read = AtomicUsize::new(start);
            let mut producer = Cursor {
                owned: start,
                peer: start,
            };
            let mut consumer = Cursor {
                owned: start,
                peer: start,
            };
            let mut storage = [0u8; 1];
            for byte in 1..=5 {
                push_ring(
                    RingQueue {
                        cursor: &mut producer,
                        stop: &stop,
                        sleeping: &sleeping,
                        write_index: &write,
                        read_index: &read,
                        capacity: 1,
                        message_size: 1,
                        stride: 1,
                        base: storage.as_mut_ptr(),
                    },
                    &[byte],
                );
                let mut output = [0];
                assert!(
                    pop_ring(
                        RingQueue {
                            cursor: &mut consumer,
                            stop: &stop,
                            sleeping: &sleeping,
                            write_index: &write,
                            read_index: &read,
                            capacity: 1,
                            message_size: 1,
                            stride: 1,
                            base: storage.as_mut_ptr()
                        },
                        &mut output,
                        WaitStrategy::Spin,
                        None
                    )
                    .unwrap()
                );
                assert_eq!(output, [byte]);
            }
        }
    }

    #[test]
    fn wake_handshake_interleavings_have_no_lost_notification() {
        // Enumerate all order-preserving merges of publish/notify and arm/recheck.
        // Arm and notify are acquire/release RMWs on the same atomic: if notify
        // precedes arm, its publication happens-before the recheck; otherwise notify
        // observes the armed receiver. This model tests that state protocol under SC.
        for order in [
            [0, 0, 1, 1],
            [0, 1, 0, 1],
            [0, 1, 1, 0],
            [1, 0, 0, 1],
            [1, 0, 1, 0],
            [1, 1, 0, 0],
        ] {
            let (mut p, mut c) = (0, 0);
            let (mut ready, mut armed, mut event, mut blocked) = (false, false, false, false);
            for actor in order {
                if actor == 0 {
                    if p == 0 {
                        ready = true;
                    } else {
                        event |= armed;
                        armed = false;
                    }
                    p += 1;
                } else {
                    if c == 0 {
                        armed = true;
                    } else {
                        blocked = !ready;
                    }
                    c += 1;
                }
            }
            assert!(!blocked || event);
        }
    }
    #[test]
    fn rejects_overflow_and_bad_attachment() {
        assert!(mapping_size(64, usize::MAX, 64).is_err());
        let header = LayoutHeader::new(256, 1, 64);
        assert!(header.validate(LayoutHeader::new(256, 1, 65)).is_err());
        let name = format!(r"Local\{}", unique_name("layout-test"));
        let _owner = SharedMailbox::create(&name, 65).unwrap();
        assert!(SharedMailbox::open(&name, 64).is_err());
    }
}
