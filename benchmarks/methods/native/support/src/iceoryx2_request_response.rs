use std::{
    error::Error,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use harness::{BenchmarkConfig, ManagedChild, ProcessRole, hold_until_stdin_closes, run_benchmark};
use iceoryx2::pending_response::PendingResponse;
use iceoryx2::port::{client::Client, update_connections::UpdateConnections};
use iceoryx2::prelude::*;

use crate::util::{configure_iceoryx2_logging, unique_name};

const ENV_SERVICE_NAME: &str = "IPC_BENCH_ICEORYX2_SERVICE";

type Iceoryx2RequestResponseFactory =
    iceoryx2::service::port_factory::request_response::PortFactory<
        ipc::Service,
        [u8],
        (),
        [u8],
        (),
    >;
type Iceoryx2Client = Client<ipc::Service, [u8], (), [u8], ()>;

const PARTICIPANT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const PARTICIPANT_POLL_INTERVAL: Duration = Duration::from_millis(1);
const RESPONSE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn run_iceoryx2_request_response() -> Result<(), Box<dyn Error>> {
    configure_iceoryx2_logging();
    let config = BenchmarkConfig::from_env()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let service_name = format!(
        "ipc-bench/{}/{}",
        method_name(),
        unique_name("request-response")
    );
    let node = NodeBuilder::new().create::<ipc::Service>()?;
    let service_id: ServiceName = service_name.as_str().try_into()?;
    let service = create_service(&node, &service_id)?;
    let client = service
        .client_builder()
        .initial_max_slice_len(config.message_size.max(1))
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()?;

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[(ENV_SERVICE_NAME, service_name.as_str())],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }
    wait_for_participants(&service)?;
    client.update_connections()?;

    let mut outbound = vec![0_u8; config.message_size];
    let mut inbound = vec![0_u8; config.message_size];
    for (index, byte) in outbound.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    run_round_trip(&client, config.message_size, &mut outbound, &mut inbound)?;

    let report = run_benchmark(method_name(), &config, true, || {
        run_round_trip(&client, config.message_size, &mut outbound, &mut inbound)
            .expect("loaned request/response round trip should succeed");
    });

    child.request_shutdown();
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("child exited with status {status}").into());
    }

    print!("{}", report.render(config.output_format)?);
    Ok(())
}

fn run_child(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let service_name = std::env::var(ENV_SERVICE_NAME)?;
    let node = NodeBuilder::new().create::<ipc::Service>()?;
    let service_id: ServiceName = service_name.as_str().try_into()?;
    let service = open_service(&node, &service_id)?;
    let server = service
        .server_builder()
        .initial_max_slice_len(config.message_size.max(1))
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()?;
    wait_for_participants(&service)?;
    server.update_connections()?;

    let stop_requested = Arc::new(AtomicBool::new(false));
    let stop_signal = Arc::clone(&stop_requested);
    let _stdin_monitor = std::thread::spawn(move || {
        let _ = hold_until_stdin_closes();
        stop_signal.store(true, Ordering::Release);
    });

    println!("ready");
    io::stdout().flush()?;

    let mut scratch = vec![0_u8; config.message_size];
    loop {
        if let Some(active_request) = server.receive()? {
            if active_request.payload().len() != config.message_size {
                return Err(format!(
                    "received request payload with {} bytes, expected {}",
                    active_request.payload().len(),
                    config.message_size
                )
                .into());
            }

            scratch.copy_from_slice(active_request.payload());
            if !scratch.is_empty() {
                scratch[0] = scratch[0].wrapping_add(1);
            }

            let response = active_request
                .loan_slice_uninit(config.message_size)
                .expect("loaned response allocation should succeed");
            let response = response.write_from_slice(scratch.as_slice());
            response
                .send()
                .expect("loaned response send should succeed");
        } else if stop_requested.load(Ordering::Acquire) {
            return Ok(());
        } else {
            std::hint::spin_loop();
        }
    }
}

fn wait_for_response(
    pending_response: &PendingResponse<ipc::Service, [u8], (), [u8], ()>,
    inbound: &mut [u8],
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + RESPONSE_WAIT_TIMEOUT;
    loop {
        if let Some(response) = pending_response.receive()? {
            if response.payload().len() != inbound.len() {
                return Err(format!(
                    "received response payload with {} bytes, expected {}",
                    response.payload().len(),
                    inbound.len()
                )
                .into());
            }
            inbound.copy_from_slice(response.payload());
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for response".into());
        }

        std::hint::spin_loop();
    }
}

fn run_round_trip(
    client: &Iceoryx2Client,
    message_size: usize,
    outbound: &mut [u8],
    inbound: &mut [u8],
) -> Result<(), Box<dyn Error>> {
    let request = client.loan_slice_uninit(message_size)?;
    let request = request.write_from_slice(outbound);
    let pending_response = request.send()?;
    wait_for_response(&pending_response, inbound)?;
    if !outbound.is_empty() {
        outbound.copy_from_slice(inbound);
        outbound[0] = outbound[0].wrapping_add(1);
    }
    Ok(())
}

fn wait_for_participants(service: &Iceoryx2RequestResponseFactory) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + PARTICIPANT_WAIT_TIMEOUT;
    loop {
        let dynamic_config = service.dynamic_config();
        if dynamic_config.number_of_clients() >= 1 && dynamic_config.number_of_servers() >= 1 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for request/response participants (clients={}, servers={})",
                dynamic_config.number_of_clients(),
                dynamic_config.number_of_servers()
            )
            .into());
        }
        thread::sleep(PARTICIPANT_POLL_INTERVAL);
    }
}

fn create_service(
    node: &Node<ipc::Service>,
    service_id: &ServiceName,
) -> Result<Iceoryx2RequestResponseFactory, Box<dyn Error>> {
    Ok(node
        .service_builder(service_id)
        .request_response::<[u8], [u8]>()
        .max_active_requests_per_client(1)
        .max_clients(1)
        .max_servers(1)
        .max_response_buffer_size(1)
        .enable_safe_overflow_for_requests(false)
        .enable_safe_overflow_for_responses(false)
        .create()?)
}

fn open_service(
    node: &Node<ipc::Service>,
    service_id: &ServiceName,
) -> Result<Iceoryx2RequestResponseFactory, Box<dyn Error>> {
    Ok(node
        .service_builder(service_id)
        .request_response::<[u8], [u8]>()
        .open()?)
}

fn method_name() -> &'static str {
    "iceoryx2-request-response-loan"
}
