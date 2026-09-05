use std::{
    error::Error,
    io::{self, Write},
    net::UdpSocket,
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};

const ENV_PARENT_PORT: &str = "IPC_BENCH_UDP_PARENT_PORT";
const MAX_UDP_PAYLOAD: usize = 65_507;

pub fn run_udp_loopback() -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    validate_udp_message_size(config.wire_size())?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn validate_udp_message_size(message_size: usize) -> io::Result<()> {
    if message_size > MAX_UDP_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "udp-loopback supports message sizes up to {MAX_UDP_PAYLOAD} bytes, got {message_size}"
            ),
        ));
    }
    Ok(())
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let parent_socket = UdpSocket::bind(("127.0.0.1", 0))?;
    let parent_port = parent_socket.local_addr()?.port();

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[(ENV_PARENT_PORT, parent_port.to_string())],
    )?;
    let readiness = child.wait_for_ready()?;
    let child_port = parse_ready_port(&readiness)?;

    parent_socket.connect(("127.0.0.1", child_port))?;

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(
        "udp-loopback",
        &config,
        true,
        || -> Result<(), Box<dyn Error>> {
            if parent_socket.send(&outbound)? != outbound.len() {
                return Err("short UDP send".into());
            }
            if parent_socket.recv(&mut inbound)? != inbound.len() {
                return Err("short UDP response".into());
            }
            harness::check_response_and_advance(&mut outbound, &inbound)?;
            Ok(())
        },
    )?;

    parent_socket.send(&[])?;
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let parent_port = std::env::var(ENV_PARENT_PORT)?.parse::<u16>()?;
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    let child_port = socket.local_addr()?.port();
    socket.connect(("127.0.0.1", parent_port))?;

    println!("ready:{child_port}");
    io::stdout().flush()?;

    let mut buf = vec![0_u8; config.wire_size()];
    loop {
        match socket.recv(&mut buf) {
            Ok(read) => {
                if read == 0 {
                    return Ok(());
                }
                if read != config.wire_size() {
                    return Err("incorrect UDP request length".into());
                }
                if !buf.is_empty() {
                    harness::transform_response(&mut buf);
                }
                if socket.send(&buf)? != buf.len() {
                    return Err("short UDP reply send".into());
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn parse_ready_port(readiness: &str) -> Result<u16, Box<dyn Error>> {
    let Some(port) = readiness.strip_prefix("ready:") else {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    };
    Ok(port.parse::<u16>()?)
}
