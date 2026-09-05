//! Concurrent message-pipe operations. Every pending buffer and OVERLAPPED is owned
//! until its completion packet is drained, including cancellation after an error.
use crate::{
    named_pipe::{ENV_PIPE_NAME, NamedPipeKind, open_client, run_child},
    util::{OwnedHandle, unique_name},
};
use harness::{BenchmarkConfig, ManagedChild, ProcessRole};
use std::{error::Error, io, mem::zeroed, ptr};
use windows_sys::Win32::{
    Foundation::ERROR_IO_PENDING,
    Storage::FileSystem::{ReadFile, WriteFile},
    System::{
        IO::{CancelIoEx, CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED},
        Pipes::{PIPE_READMODE_MESSAGE, SetNamedPipeHandleState},
    },
};

pub fn run_pipe_iocp() -> Result<(), Box<dyn Error>> {
    let config =
        BenchmarkConfig::from_env().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if config.workload != "windowed" {
        return Err("named-pipe-iocp requires --workload windowed".into());
    }
    if config.role == ProcessRole::Child {
        return run_child(config, NamedPipeKind::MessageSync);
    }
    let name = format!(r"\\.\pipe\{}", unique_name("ipc-bench-iocp"));
    let mut child =
        ManagedChild::spawn_self_with_env(&config.child_args(), &[(ENV_PIPE_NAME, name.clone())])?;
    if child.wait_for_ready()? != "ready" {
        return Err("invalid pipe readiness".into());
    }
    let pipe = open_client(&name, NamedPipeKind::Overlapped)?;
    let mode = PIPE_READMODE_MESSAGE;
    if unsafe { SetNamedPipeHandleState(pipe.raw(), &mode, ptr::null(), ptr::null()) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    let completion = OwnedHandle::from_handle(unsafe {
        CreateIoCompletionPort(pipe.raw(), ptr::null_mut(), 0, 1)
    })?;
    let mut session = Session::new(pipe, completion, config.queue_depth, config.wire_size());
    let report =
        harness::run_throughput("named-pipe-iocp", &config, |count| session.deliver(count))?;
    drop(session);
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("IOCP worker exited with {status}").into());
    }
    print!("{}", report.render(config.output_format)?);
    Ok(())
}

#[repr(C)]
struct Operation {
    overlapped: OVERLAPPED,
    buffer: Vec<u8>,
    index: usize,
    read: bool,
    pending: bool,
}

struct Credit {
    started: Option<std::time::Instant>,
    sequence: usize,
    write_done: bool,
    response_done: bool,
}

struct Session {
    pipe: OwnedHandle,
    completion: OwnedHandle,
    // Boxes are required: completion pointers must survive Vec movement/reallocation.
    #[allow(clippy::vec_box)]
    operations: Vec<Box<Operation>>,
    credits: Vec<Credit>,
    pending: usize,
    wire_size: usize,
    expected: Vec<u8>,
    next_sequence: usize,
}

impl Session {
    fn new(pipe: OwnedHandle, completion: OwnedHandle, depth: usize, wire_size: usize) -> Self {
        let operations = (0..depth * 2)
            .map(|index| {
                Box::new(Operation {
                    overlapped: unsafe { zeroed() },
                    buffer: vec![0xA5; wire_size],
                    index,
                    read: index >= depth,
                    pending: false,
                })
            })
            .collect();
        let credits = (0..depth)
            .map(|_| Credit {
                started: None,
                sequence: 0,
                write_done: true,
                response_done: true,
            })
            .collect();
        let mut expected = vec![0; wire_size];
        harness::initialize_payload(&mut expected);
        Self {
            pipe,
            completion,
            operations,
            credits,
            pending: 0,
            wire_size,
            expected,
            next_sequence: 0,
        }
    }

    fn submit(&mut self, index: usize) -> io::Result<()> {
        let operation = &mut self.operations[index];
        if operation.pending {
            return Err(io::Error::other("attempt to reuse pending IOCP operation"));
        }
        operation.overlapped = unsafe { zeroed() };
        let length = u32::try_from(operation.buffer.len())
            .map_err(|_| io::Error::other("IOCP length overflow"))?;
        let ok = unsafe {
            if operation.read {
                ReadFile(
                    self.pipe.raw(),
                    operation.buffer.as_mut_ptr(),
                    length,
                    ptr::null_mut(),
                    &mut operation.overlapped,
                )
            } else {
                WriteFile(
                    self.pipe.raw(),
                    operation.buffer.as_ptr(),
                    length,
                    ptr::null_mut(),
                    &mut operation.overlapped,
                )
            }
        };
        if ok == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(error);
            }
        }
        // Synchronous completion also queues a packet; no skip-completion modes are used.
        operation.pending = true;
        self.pending += 1;
        Ok(())
    }

    fn complete(&mut self) -> io::Result<(usize, usize)> {
        let mut bytes = 0;
        let mut key = 0;
        let mut overlapped = ptr::null_mut();
        let ok = unsafe {
            GetQueuedCompletionStatus(
                self.completion.raw(),
                &mut bytes,
                &mut key,
                &mut overlapped,
                5000,
            )
        };
        if overlapped.is_null() {
            return Err(io::Error::last_os_error());
        }
        // This port is private; only boxed Operation pointers have ever been submitted.
        let operation = unsafe { &mut *overlapped.cast::<Operation>() };
        if !operation.pending {
            return Err(io::Error::other("duplicate IOCP completion"));
        }
        operation.pending = false;
        self.pending -= 1;
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((operation.index, bytes as usize))
    }

    fn deliver(&mut self, count: usize) -> Result<(), Box<dyn Error>> {
        let depth = self.credits.len();
        let first_sequence = self.next_sequence;
        let mut sent = 0;
        let mut delivered = 0;
        let mut reads_issued = 0;
        for index in 0..count.min(depth) {
            self.submit(depth + index)?;
            reads_issued += 1;
        }
        while delivered < count || self.pending > 0 {
            while sent < count {
                let sequence = first_sequence + sent;
                let index = sequence % depth;
                let credit = &mut self.credits[index];
                if !credit.write_done || !credit.response_done {
                    break;
                }
                *credit = Credit {
                    started: sequence.is_multiple_of(16).then(std::time::Instant::now),
                    sequence,
                    write_done: false,
                    response_done: false,
                };
                let buffer = &mut self.operations[index].buffer;
                harness::initialize_payload(buffer);
                buffer[..8].copy_from_slice(&(sequence as u64).to_le_bytes());
                self.submit(index)?;
                sent += 1;
            }
            let (index, bytes) = self.complete()?;
            if bytes != self.wire_size {
                return Err(format!("short IOCP completion: {bytes} of {}", self.wire_size).into());
            }
            if index < depth {
                self.credits[index].write_done = true;
            } else {
                let response = &self.operations[index].buffer;
                let mut header: [u8; 8] = response[..8].try_into()?;
                header[0] = header[0].wrapping_sub(1);
                let sequence = usize::try_from(u64::from_le_bytes(header))?;
                let credit = &mut self.credits[sequence % depth];
                if sequence < first_sequence
                    || sequence >= first_sequence + sent
                    || sequence != credit.sequence
                    || credit.response_done
                {
                    return Err("duplicate/stale IOCP response".into());
                }
                self.expected[..8].copy_from_slice(&(sequence as u64).to_le_bytes());
                harness::check_response_and_advance(&mut self.expected, response)?;
                credit.response_done = true;
                harness::record_delivery_latency(credit.started.take());
                delivered += 1;
                if reads_issued < count {
                    self.submit(index)?;
                    reads_issued += 1;
                }
            }
        }
        self.next_sequence += count;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.pending != 0 {
            unsafe {
                CancelIoEx(self.pipe.raw(), ptr::null());
            }
            while self.pending != 0 {
                let before = self.pending;
                let _ = self.complete();
                if self.pending == before {
                    // A non-cancelling driver must not cause buffers still owned by the
                    // kernel to be freed. Abort closes the job and destroys the whole tree.
                    std::process::abort();
                }
            }
        }
    }
}
