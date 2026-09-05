use crate::{BenchmarkConfig, OutputFormat};
use serde::Serialize;
use std::{error::Error, time::Instant};

thread_local! {
    static LOAD_SAMPLES: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
}
pub fn record_delivery_latency(start: Option<Instant>) {
    if let Some(start) = start {
        LOAD_SAMPLES.with_borrow_mut(|samples| {
            if samples.len() < 65_536 {
                samples.push(start.elapsed().as_secs_f64() * 1_000_000.0);
            }
        });
    }
}

#[derive(Serialize)]
pub struct DeliveryTrial {
    pub trial_index: usize,
    pub elapsed_seconds: f64,
    pub delivered_messages: usize,
    pub delivered_payload_bytes: usize,
    pub delivered_messages_per_second: f64,
    pub payload_bytes_per_second: f64,
    pub latency_samples_micros: Vec<f64>,
    pub latency_percentiles_micros: Option<[f64; 3]>,
}

#[derive(Serialize)]
pub struct ThroughputReport {
    pub schema_version: u32,
    pub method: String,
    pub workload: String,
    pub queue_depth: usize,
    pub ring_capacity: usize,
    pub wire_size: usize,
    pub config: BenchmarkConfig,
    pub validation_policy: String,
    pub byte_count_direction: String,
    pub timed_operation_count: usize,
    pub delivery_errors: usize,
    pub effective_parent_affinity: Option<usize>,
    pub trials: Vec<DeliveryTrial>,
    pub latency_sampling_policy: String,
}

pub fn run_throughput<F>(
    method: &str,
    config: &BenchmarkConfig,
    mut deliver: F,
) -> Result<ThroughputReport, Box<dyn Error>>
where
    F: FnMut(usize) -> Result<(), Box<dyn Error>>,
{
    deliver(1).map_err(|e| format!("method={method} phase=preflight: {e}"))?;
    deliver(config.warmup_count).map_err(|e| format!("method={method} phase=warmup: {e}"))?;
    let mut trials = Vec::with_capacity(config.trials);
    for trial_index in 1..=config.trials {
        LOAD_SAMPLES.with_borrow_mut(|samples| *samples = Vec::with_capacity(65_536));
        let start = Instant::now();
        deliver(config.message_count)
            .map_err(|e| format!("method={method} phase=timed trial={trial_index}: {e}"))?;
        let elapsed_seconds = start.elapsed().as_secs_f64();
        let samples = LOAD_SAMPLES.with_borrow_mut(std::mem::take);
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        let percentiles = (!sorted.is_empty()).then(|| {
            [0.5, 0.95, 0.99].map(|p| sorted[(p * sorted.len() as f64).ceil() as usize - 1])
        });
        if elapsed_seconds <= 0.0 {
            return Err("timer resolution insufficient".into());
        }
        let bytes = config
            .message_count
            .checked_mul(config.message_size)
            .ok_or("payload byte count overflow")?;
        trials.push(DeliveryTrial {
            trial_index,
            elapsed_seconds,
            delivered_messages: config.message_count,
            delivered_payload_bytes: bytes,
            delivered_messages_per_second: config.message_count as f64 / elapsed_seconds,
            payload_bytes_per_second: bytes as f64 / elapsed_seconds,
            latency_samples_micros: samples,
            latency_percentiles_micros: percentiles,
        });
    }
    deliver(1).map_err(|e| format!("method={method} phase=final: {e}"))?;
    Ok(ThroughputReport {
        schema_version: 2,
        method: method.into(),
        workload: config.workload.clone(),
        queue_depth: config.queue_depth,
        ring_capacity: config.ring_capacity,
        wire_size: config.wire_size(),
        config: config.clone(),
        validation_policy: "full-payload-every-delivery;exact-sequence".into(),
        byte_count_direction: if config.workload == "streaming" {
            "requests"
        } else {
            "responses"
        }
        .into(),
        timed_operation_count: config.message_count * config.trials,
        delivery_errors: 0,
        effective_parent_affinity: crate::affinity::effective_mask()?,
        trials,
        latency_sampling_policy: "send-to-validated-delivery-observation;every-16th-sequence;first-65536-samples-per-trial".into(),
    })
}

impl ThroughputReport {
    pub fn render(&self, format: OutputFormat) -> Result<String, serde_json::Error> {
        match format {
            OutputFormat::Json => crate::resources::render_json(self),
            OutputFormat::Text => Ok(format!(
                "{} {}: {} verified deliveries at queue depth {}\n",
                self.method, self.workload, self.timed_operation_count, self.queue_depth
            )),
        }
    }
}
