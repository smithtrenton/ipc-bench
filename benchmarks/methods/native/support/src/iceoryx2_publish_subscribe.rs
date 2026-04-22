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
use iceoryx2::port::{
    publisher::Publisher, subscriber::Subscriber, update_connections::UpdateConnections,
};
use iceoryx2::prelude::*;

use crate::util::{configure_iceoryx2_logging, unique_name};

const ENV_REQUEST_SERVICE_NAME: &str = "IPC_BENCH_ICEORYX2_REQUEST_SERVICE";
const ENV_RESPONSE_SERVICE_NAME: &str = "IPC_BENCH_ICEORYX2_RESPONSE_SERVICE";

type Iceoryx2PublishSubscribeFactory =
    iceoryx2::service::port_factory::publish_subscribe::PortFactory<ipc::Service, [u8], ()>;
type Iceoryx2Publisher = Publisher<ipc::Service, [u8], ()>;
type Iceoryx2Subscriber = Subscriber<ipc::Service, [u8], ()>;

const PARTICIPANT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const PARTICIPANT_POLL_INTERVAL: Duration = Duration::from_millis(1);
const RESPONSE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn run_iceoryx2_publish_subscribe() -> Result<(), Box<dyn Error>> {
    configure_iceoryx2_logging();
    let config = BenchmarkConfig::from_env()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    match config.role {
        ProcessRole::Parent => run_parent(config),
        ProcessRole::Child => run_child(config),
    }
}

fn run_parent(config: BenchmarkConfig) -> Result<(), Box<dyn Error>> {
    let request_service_name = format!(
        "ipc-bench/{}/{}",
        method_name(),
        unique_name("request-service")
    );
    let response_service_name = format!(
        "ipc-bench/{}/{}",
        method_name(),
        unique_name("response-service")
    );
    let node = NodeBuilder::new().create::<ipc::Service>()?;
    let request_service_id: ServiceName = request_service_name.as_str().try_into()?;
    let response_service_id: ServiceName = response_service_name.as_str().try_into()?;
    let request_service = create_service(&node, &request_service_id)?;
    let response_service = create_service(&node, &response_service_id)?;
    let request_publisher = request_service
        .publisher_builder()
        .initial_max_slice_len(config.message_size.max(1))
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()?;
    let response_subscriber = response_service.subscriber_builder().create()?;

    let mut child = ManagedChild::spawn_self_with_env(
        &config.child_args(),
        &[
            (ENV_REQUEST_SERVICE_NAME, request_service_name.as_str()),
            (ENV_RESPONSE_SERVICE_NAME, response_service_name.as_str()),
        ],
    )?;
    let readiness = child.wait_for_ready()?;
    if readiness != "ready" {
        return Err(format!("unexpected child readiness message `{readiness}`").into());
    }
    wait_for_participants(&request_service, 1, 1, "request")?;
    wait_for_participants(&response_service, 1, 1, "response")?;
    request_publisher.update_connections()?;
    response_subscriber.update_connections()?;

    let mut outbound = vec![0_u8; config.message_size];
    let mut inbound = vec![0_u8; config.message_size];
    for (index, byte) in outbound.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    run_round_trip(
        &request_publisher,
        &response_subscriber,
        config.message_size,
        &mut outbound,
        &mut inbound,
    )?;

    let report = run_benchmark(method_name(), &config, true, || {
        run_round_trip(
            &request_publisher,
            &response_subscriber,
            config.message_size,
            &mut outbound,
            &mut inbound,
        )
        .expect("loaned publish/subscribe round trip should succeed");
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
    let request_service_name = std::env::var(ENV_REQUEST_SERVICE_NAME)?;
    let response_service_name = std::env::var(ENV_RESPONSE_SERVICE_NAME)?;
    let node = NodeBuilder::new().create::<ipc::Service>()?;
    let request_service_id: ServiceName = request_service_name.as_str().try_into()?;
    let response_service_id: ServiceName = response_service_name.as_str().try_into()?;
    let request_service = open_service(&node, &request_service_id)?;
    let response_service = open_service(&node, &response_service_id)?;
    let request_subscriber = request_service.subscriber_builder().create()?;
    let response_publisher = response_service
        .publisher_builder()
        .initial_max_slice_len(config.message_size.max(1))
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()?;
    wait_for_participants(&request_service, 1, 1, "request")?;
    wait_for_participants(&response_service, 1, 1, "response")?;
    request_subscriber.update_connections()?;
    response_publisher.update_connections()?;

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
        if let Some(request) = request_subscriber.receive()? {
            if request.payload().len() != config.message_size {
                return Err(format!(
                    "received request payload with {} bytes, expected {}",
                    request.payload().len(),
                    config.message_size
                )
                .into());
            }

            scratch.copy_from_slice(request.payload());
            if !scratch.is_empty() {
                scratch[0] = scratch[0].wrapping_add(1);
            }

            let response = response_publisher
                .loan_slice_uninit(config.message_size)
                .expect("response publish allocation should succeed");
            let response = response.write_from_slice(scratch.as_slice());
            response.send().expect("response publish should succeed");
        } else if stop_requested.load(Ordering::Acquire) {
            return Ok(());
        } else {
            std::hint::spin_loop();
        }
    }
}

fn run_round_trip(
    request_publisher: &Iceoryx2Publisher,
    response_subscriber: &Iceoryx2Subscriber,
    message_size: usize,
    outbound: &mut [u8],
    inbound: &mut [u8],
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + RESPONSE_WAIT_TIMEOUT;
    let sample = request_publisher.loan_slice_uninit(message_size)?;
    let sample = sample.write_from_slice(outbound);
    sample.send()?;

    loop {
        if let Some(response) = response_subscriber.receive()? {
            if response.payload().len() != inbound.len() {
                return Err(format!(
                    "received response payload with {} bytes, expected {}",
                    response.payload().len(),
                    inbound.len()
                )
                .into());
            }
            inbound.copy_from_slice(response.payload());
            if !outbound.is_empty() {
                outbound.copy_from_slice(inbound);
                outbound[0] = outbound[0].wrapping_add(1);
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for publish/subscribe response".into());
        }
        std::hint::spin_loop();
    }
}

fn wait_for_participants(
    service: &Iceoryx2PublishSubscribeFactory,
    expected_publishers: usize,
    expected_subscribers: usize,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + PARTICIPANT_WAIT_TIMEOUT;
    loop {
        let dynamic_config = service.dynamic_config();
        if dynamic_config.number_of_publishers() >= expected_publishers
            && dynamic_config.number_of_subscribers() >= expected_subscribers
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for {label} pub/sub participants (publishers={}, subscribers={})",
                dynamic_config.number_of_publishers(),
                dynamic_config.number_of_subscribers()
            )
            .into());
        }
        thread::sleep(PARTICIPANT_POLL_INTERVAL);
    }
}

fn create_service(
    node: &Node<ipc::Service>,
    service_id: &ServiceName,
) -> Result<Iceoryx2PublishSubscribeFactory, Box<dyn Error>> {
    Ok(node
        .service_builder(service_id)
        .publish_subscribe::<[u8]>()
        .max_publishers(1)
        .max_subscribers(1)
        .history_size(0)
        .subscriber_max_buffer_size(1)
        .enable_safe_overflow(false)
        .create()?)
}

fn open_service(
    node: &Node<ipc::Service>,
    service_id: &ServiceName,
) -> Result<Iceoryx2PublishSubscribeFactory, Box<dyn Error>> {
    Ok(node
        .service_builder(service_id)
        .publish_subscribe::<[u8]>()
        .open()?)
}

fn method_name() -> &'static str {
    "iceoryx2-publish-subscribe-loan"
}
