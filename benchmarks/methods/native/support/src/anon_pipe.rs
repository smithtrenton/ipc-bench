use std::{
    error::Error,
    io::{self, Read, Write},
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, run_benchmark};

pub fn run_anon_pipe() -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let mut child = ManagedChild::spawn_self(&config.child_args())?;
    let (mut stdin, mut stdout) = child.take_pipes()?;

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(
        "anon-pipe",
        &config,
        false,
        || -> Result<(), Box<dyn Error>> {
            stdin.write_all(&outbound)?;
            stdin.flush()?;
            stdout.read_exact(&mut inbound)?;
            harness::check_response_and_advance(&mut outbound, &inbound)?;
            Ok(())
        },
    )?;

    drop(stdin);
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut buf = vec![0_u8; config.wire_size()];

    loop {
        match reader.read_exact(&mut buf) {
            Ok(()) => {
                if !buf.is_empty() {
                    harness::transform_response(&mut buf);
                }
                writer.write_all(&buf)?;
                writer.flush()?;
            }
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
}
