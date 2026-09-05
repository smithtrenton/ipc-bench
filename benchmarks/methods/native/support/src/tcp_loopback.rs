use std::{
    error::Error,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};

pub fn run_tcp_loopback() -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let mut child = ManagedChild::spawn_self(&config.child_args())?;
    let readiness = child.wait_for_ready()?;
    let port = parse_ready_port(&readiness)?;
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_nodelay(true)?;

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(
        "tcp-loopback",
        &config,
        true,
        || -> Result<(), Box<dyn Error>> {
            stream.write_all(&outbound)?;
            stream.flush()?;
            stream.read_exact(&mut inbound)?;
            harness::check_response_and_advance(&mut outbound, &inbound)?;
            Ok(())
        },
    )?;

    drop(stream);
    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    println!("ready:{port}");
    io::stdout().flush()?;

    let (mut stream, _) = listener.accept()?;
    stream.set_nodelay(true)?;
    let mut buf = vec![0_u8; config.wire_size()];

    loop {
        match stream.read_exact(&mut buf) {
            Ok(()) => {
                if !buf.is_empty() {
                    harness::transform_response(&mut buf);
                }
                stream.write_all(&buf)?;
                stream.flush()?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::ConnectionReset
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
