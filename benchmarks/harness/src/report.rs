use std::fmt::Write as _;

use serde::Serialize;

use crate::{
    BenchmarkConfig, OutputFormat,
    stats::{AggregateSummary, TrialSummary, aggregate_trials, measure_trial},
};

#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub workload: String,
    pub queue_depth: usize,
    pub wire_size: usize,
    pub validation_policy: String,
    pub sampling_unit: String,
    pub measurement_batch_size: usize,
    pub timed_operation_count: usize,
    pub preflight_operations: usize,
    pub final_check_operations: usize,
    pub timer_pair_micros: f64,
    pub effective_parent_affinity: Option<usize>,
    pub method: String,
    pub child_ready: bool,
    pub config: BenchmarkConfig,
    pub trials: Vec<TrialSummary>,
    pub summary: AggregateSummary,
}

pub fn run_benchmark<F>(
    method: &str,
    config: &BenchmarkConfig,
    child_ready: bool,
    mut operation: F,
) -> Result<BenchmarkReport, Box<dyn std::error::Error>>
where
    F: FnMut() -> Result<(), Box<dyn std::error::Error>>,
{
    let contextual = |phase: &str, iteration: usize, error: Box<dyn std::error::Error>| {
        format!("method={method} phase={phase} trial=0 iteration={iteration}: {error}")
    };
    crate::payload::set_full_validation(true);
    operation().map_err(|e| contextual("preflight", 1, e))?;
    for iteration in 0..config.warmup_count {
        operation().map_err(|e| contextual("warmup", iteration + 1, e))?;
    }
    let calibration = std::time::Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(std::time::Instant::now().elapsed());
    }
    let timer_pair_micros = calibration.elapsed().as_secs_f64() * 1000.0;
    crate::payload::set_full_validation(config.validation == "full");
    let trials = (0..config.trials)
        .map(|index| {
            crate::payload::set_full_validation(config.validation == "full");
            measure_trial(
                index + 1,
                config.message_count,
                config.measurement == "latency",
                &mut operation,
            )
            .map_err(|e| format!("method={method} {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::payload::set_full_validation(true);
    operation().map_err(|e| contextual("final", 1, e))?;
    let summary = aggregate_trials(&trials, config.message_count);
    if ![
        summary.total_micros,
        summary.average_micros,
        summary.min_micros,
        summary.max_micros,
        summary.stddev_micros,
        summary.round_trip_rate,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        return Err("non-finite measurement invalidates report".into());
    }
    Ok(BenchmarkReport {
        schema_version: 2,
        workload: if method == "placeholder" {
            "harness-overhead"
        } else {
            "round-trip"
        }
        .into(),
        queue_depth: 1,
        wire_size: config.wire_size(),
        validation_policy: if method == "placeholder" {
            "none"
        } else if config.validation == "full" {
            "full-payload-every-operation"
        } else {
            "sequence-every-operation;full-every-1024;full-preflight-and-final;full-last-batch-each-trial"
        }
        .into(),
        sampling_unit: if config.measurement == "latency" {
            "individual-round-trip"
        } else {
            "batch-average-round-trip"
        }
        .into(),
        measurement_batch_size: if config.measurement == "latency" {
            1
        } else {
            crate::stats::measurement_batch_size(config.message_count)
        },
        timed_operation_count: config.message_count * config.trials,
        preflight_operations: 1,
        final_check_operations: 1,
        timer_pair_micros,
        effective_parent_affinity: crate::affinity::effective_mask()?,
        method: method.into(),
        child_ready,
        config: config.clone(),
        trials,
        summary,
    })
}

impl BenchmarkReport {
    pub fn render(&self, format: OutputFormat) -> Result<String, serde_json::Error> {
        match format {
            OutputFormat::Text => Ok(self.render_text()),
            OutputFormat::Json => crate::resources::render_json(self),
        }
    }

    fn render_text(&self) -> String {
        let mut output = String::new();

        writeln!(output, "============ RESULTS ================").expect("write to string");
        writeln!(output, "Method:             {}", self.method).expect("write to string");
        writeln!(output, "Sampling unit:      {}", self.sampling_unit).expect("write to string");
        writeln!(
            output,
            "Child bootstrap:    {}",
            if self.child_ready { "ok" } else { "not used" }
        )
        .expect("write to string");
        writeln!(output, "Message size:       {}", self.config.message_size)
            .expect("write to string");
        writeln!(output, "Message count:      {}", self.config.message_count)
            .expect("write to string");
        writeln!(output, "Warmup count:       {}", self.config.warmup_count)
            .expect("write to string");
        writeln!(output, "Trial count:        {}", self.config.trials).expect("write to string");
        writeln!(
            output,
            "Total duration:     {:.3}\tms",
            self.summary.total_micros / 1_000.0
        )
        .expect("write to string");
        writeln!(
            output,
            "Average duration:   {:.3}\tus",
            self.summary.average_micros
        )
        .expect("write to string");
        writeln!(
            output,
            "Minimum sample:      {:.3}\tus",
            self.summary.min_micros
        )
        .expect("write to string");
        writeln!(
            output,
            "Maximum sample:      {:.3}\tus",
            self.summary.max_micros
        )
        .expect("write to string");
        writeln!(
            output,
            "Sample stddev:       {:.3}\tus",
            self.summary.stddev_micros
        )
        .expect("write to string");
        writeln!(
            output,
            "Round-trip rate:       {:.0}\tmsg/s",
            self.summary.message_rate
        )
        .expect("write to string");

        for trial in &self.trials {
            writeln!(
                output,
                "Trial {:>2}: total {:.3} us | avg {:.3} us | rate {:.0} msg/s",
                trial.trial_index, trial.total_micros, trial.average_micros, trial.message_rate
            )
            .expect("write to string");
        }

        writeln!(output, "=====================================").expect("write to string");

        output
    }
}

#[cfg(test)]
mod tests {
    use crate::{BenchmarkConfig, OutputFormat};

    use super::run_benchmark;

    #[test]
    fn renders_json_report() {
        let config = BenchmarkConfig {
            trials: 1,
            warmup_count: 0,
            output_format: OutputFormat::Json,
            ..BenchmarkConfig::default()
        };
        let report = run_benchmark("placeholder", &config, true, || Ok(())).unwrap();
        let rendered = report
            .render(OutputFormat::Json)
            .expect("json rendering should succeed");

        assert!(rendered.contains("\"method\": \"placeholder\""));
    }
}
