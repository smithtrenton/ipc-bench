use std::{error::Error, io};

use harness::{BenchmarkConfig, ProcessRole, run_benchmark};

pub fn run_copy_roundtrip() -> Result<(), Box<dyn Error>> {
    let config = BenchmarkConfig::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    if config.role != ProcessRole::Parent {
        return Err("copy-roundtrip does not use a child role".into());
    }

    let mut outbound = vec![0_u8; config.wire_size()];
    let mut request = vec![0_u8; config.wire_size()];
    let mut scratch = vec![0_u8; config.wire_size()];
    let mut response = vec![0_u8; config.wire_size()];
    let mut inbound = vec![0_u8; config.wire_size()];
    harness::initialize_payload(&mut outbound);

    let report = run_benchmark(
        "copy-roundtrip",
        &config,
        false,
        || -> Result<(), Box<dyn Error>> {
            request.copy_from_slice(&outbound);
            std::hint::black_box(&mut request);
            scratch.copy_from_slice(&request);
            std::hint::black_box(&mut scratch);
            if !scratch.is_empty() {
                harness::transform_response(&mut scratch);
            }
            response.copy_from_slice(&scratch);
            std::hint::black_box(&mut response);
            inbound.copy_from_slice(&response);
            std::hint::black_box(&mut inbound);
            harness::check_response_and_advance(&mut outbound, &inbound)?;
            Ok(())
        },
    )?;

    print!("{}", report.render(config.output_format)?);
    Ok(())
}
