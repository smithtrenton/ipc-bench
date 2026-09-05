use std::{
    error::Error,
    io::{self, Write},
    time::Duration,
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};
use windows_sys::Win32::{
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_READ, OPEN_EXISTING,
        ReadFile,
    },
    System::Mailslots::CreateMailslotW,
};

use crate::util::{OwnedHandle, retry_with_backoff, unique_name, wide_string, write_all_handle};

const ENV_REQUEST_SLOT: &str = "IPC_BENCH_MAILSLOT_REQUEST";
const ENV_RESPONSE_SLOT: &str = "IPC_BENCH_MAILSLOT_RESPONSE";

pub fn run_mailslot() -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let request_name = format!(r"\\.\mailslot\{}", unique_name("ipc-bench-request"));
    let response_name = format!(r"\\.\mailslot\{}", unique_name("ipc-bench-response"));
    let response_slot = create_mailslot(&response_name, config.wire_size())?;

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[
            (ENV_REQUEST_SLOT, request_name.clone()),
            (ENV_RESPONSE_SLOT, response_name.clone()),
        ],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }

    let request_writer = open_writer(&request_name)?;
    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(
        "mailslot",
        &config,
        true,
        || -> Result<(), Box<dyn Error>> {
            write_all_handle(request_writer.raw(), &outbound)?;
            read_exact_message(response_slot.raw(), &mut inbound)?;
            harness::check_response_and_advance(&mut outbound, &inbound)?;
            Ok(())
        },
    )?;

    write_all_handle(request_writer.raw(), &[0xFF])?;
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let request_name = std::env::var(ENV_REQUEST_SLOT)?;
    let response_name = std::env::var(ENV_RESPONSE_SLOT)?;

    let request_slot = create_mailslot(&request_name, config.wire_size())?;
    let response_writer = open_writer(&response_name)?;

    println!("ready");
    io::stdout().flush()?;

    let mut message = vec![0_u8; config.wire_size()];
    loop {
        let bytes_read = read_next_message(request_slot.raw(), &mut message)?;
        if bytes_read == 1 && message[0] == 0xFF {
            return Ok(());
        }
        if bytes_read != config.wire_size() {
            return Err("incorrect mailslot request length".into());
        }
        if !message.is_empty() {
            harness::transform_response(&mut message);
        }
        write_all_handle(response_writer.raw(), &message)?;
    }
}

fn create_mailslot(name: &str, message_size: usize) -> io::Result<OwnedHandle> {
    let name = wide_string(name);
    let handle = unsafe {
        CreateMailslotW(
            name.as_ptr(),
            u32::try_from(message_size).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "mailslot length overflow")
            })?,
            5000,
            std::ptr::null(),
        )
    };
    OwnedHandle::from_file_handle(handle)
}

fn open_writer(name: &str) -> io::Result<OwnedHandle> {
    let name = name.to_owned();
    retry_with_backoff(200, Duration::from_millis(10), || {
        let name = wide_string(&name);
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        OwnedHandle::from_file_handle(handle)
    })
}

fn read_next_message(
    handle: windows_sys::Win32::Foundation::HANDLE,
    buffer: &mut [u8],
) -> io::Result<usize> {
    let buffer_len = u32::try_from(buffer.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "buffer too large"))?;
    let mut bytes_read = 0_u32;
    let ok = unsafe {
        ReadFile(
            handle,
            buffer.as_mut_ptr().cast(),
            buffer_len,
            &mut bytes_read,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if bytes_read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "mailslot closed while reading",
        ));
    }
    Ok(bytes_read as usize)
}

fn read_exact_message(
    handle: windows_sys::Win32::Foundation::HANDLE,
    buffer: &mut [u8],
) -> io::Result<()> {
    let bytes_read = read_next_message(handle, buffer)?;
    if bytes_read != buffer.len() {
        return Err(io::Error::other(format!(
            "expected message length {}, got {bytes_read}",
            buffer.len()
        )));
    }
    Ok(())
}
