use serde::Serialize;
use std::time::Instant;

const TARGET_BATCHES_PER_TRIAL: usize = 100;
const MAX_BATCH_SIZE: usize = 100;

#[derive(Clone, Debug, Serialize)]
pub struct TrialSummary {
    pub trial_index: usize,
    pub samples: Vec<BatchMeasurement>,
    pub latency_percentiles_micros: Option<[f64; 3]>,
    pub total_micros: f64,
    pub average_micros: f64,
    pub min_micros: f64,
    pub max_micros: f64,
    pub stddev_micros: f64,
    pub message_rate: f64,
    pub round_trip_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AggregateSummary {
    pub total_micros: f64,
    pub average_micros: f64,
    pub min_micros: f64,
    pub max_micros: f64,
    pub stddev_micros: f64,
    pub message_rate: f64,
    pub round_trip_rate: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BatchMeasurement {
    pub average_micros: f64,
    pub operations: usize,
}

pub fn measure_trial<F>(
    trial_index: usize,
    message_count: usize,
    latency: bool,
    operation: &mut F,
) -> Result<TrialSummary, Box<dyn std::error::Error>>
where
    F: FnMut() -> Result<(), Box<dyn std::error::Error>>,
{
    let batch_size = if latency {
        1
    } else {
        measurement_batch_size(message_count)
    };
    let mut batches = Vec::with_capacity(message_count.div_ceil(batch_size));
    let mut remaining = message_count;
    while remaining > 0 {
        let operations = remaining.min(batch_size);
        if remaining <= batch_size {
            crate::payload::set_full_validation(true);
        }
        let start = Instant::now();
        for index in 0..operations {
            operation().map_err(|error| {
                format!(
                    "phase=timed trial={trial_index} iteration={}: {error}",
                    message_count - remaining + index + 1
                )
            })?;
        }
        let elapsed = start.elapsed().as_secs_f64() * 1_000_000.0;
        batches.push(BatchMeasurement {
            average_micros: elapsed / operations as f64,
            operations,
        });
        remaining -= operations;
    }
    let mut summary = summarize_trial(trial_index, &batches);
    if !summary.round_trip_rate.is_finite() {
        return Err(
            format!("phase=timed trial={trial_index}: timer resolution insufficient").into(),
        );
    }
    if latency {
        let mut samples: Vec<_> = batches.iter().map(|b| b.average_micros).collect();
        samples.sort_by(f64::total_cmp);
        summary.latency_percentiles_micros = Some(
            [0.50, 0.95, 0.99]
                .map(|p| samples[((p * samples.len() as f64).ceil() as usize).saturating_sub(1)]),
        );
    }
    Ok(summary)
}

pub fn aggregate_trials(trials: &[TrialSummary], message_count: usize) -> AggregateSummary {
    debug_assert!(!trials.is_empty(), "trials cannot be empty");

    let total_messages = (trials.len() * message_count) as f64;
    let total_micros = trials.iter().map(|trial| trial.total_micros).sum::<f64>();
    let average_micros = total_micros / total_messages;
    let min_micros = trials
        .iter()
        .map(|trial| trial.min_micros)
        .fold(f64::INFINITY, f64::min);
    let max_micros = trials
        .iter()
        .map(|trial| trial.max_micros)
        .fold(f64::NEG_INFINITY, f64::max);
    let per_trial_messages = message_count as f64;
    let variance = trials
        .iter()
        .map(|trial| {
            per_trial_messages
                * (trial.stddev_micros.powi(2) + (trial.average_micros - average_micros).powi(2))
        })
        .sum::<f64>()
        / total_messages;
    let stddev_micros = variance.sqrt();
    let message_rate = if total_micros == 0.0 {
        f64::INFINITY
    } else {
        total_messages / (total_micros / 1_000_000.0)
    };

    AggregateSummary {
        total_micros,
        average_micros,
        min_micros,
        max_micros,
        stddev_micros,
        message_rate,
        round_trip_rate: message_rate,
    }
}

pub fn measurement_batch_size(message_count: usize) -> usize {
    message_count
        .div_ceil(TARGET_BATCHES_PER_TRIAL)
        .clamp(1, MAX_BATCH_SIZE)
}

fn summarize_trial(trial_index: usize, batches: &[BatchMeasurement]) -> TrialSummary {
    let count = batches.iter().map(|batch| batch.operations).sum::<usize>() as f64;
    let total_micros = batches
        .iter()
        .map(|batch| batch.average_micros * batch.operations as f64)
        .sum::<f64>();
    let average_micros = total_micros / count;
    let min_micros = batches
        .iter()
        .map(|batch| batch.average_micros)
        .fold(f64::INFINITY, f64::min);
    let max_micros = batches
        .iter()
        .map(|batch| batch.average_micros)
        .fold(f64::NEG_INFINITY, f64::max);
    let variance = batches
        .iter()
        .map(|batch| {
            let delta = batch.average_micros - average_micros;
            delta * delta * batch.operations as f64
        })
        .sum::<f64>()
        / count;
    let stddev_micros = variance.sqrt();
    let message_rate = if total_micros == 0.0 {
        f64::INFINITY
    } else {
        count / (total_micros / 1_000_000.0)
    };

    TrialSummary {
        trial_index,
        samples: batches.to_vec(),
        latency_percentiles_micros: None,
        total_micros,
        average_micros,
        min_micros,
        max_micros,
        stddev_micros,
        message_rate,
        round_trip_rate: message_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::{TrialSummary, aggregate_trials};

    fn approx_equal(left: f64, right: f64) {
        let delta = (left - right).abs();
        assert!(delta < 0.000_1, "left={left}, right={right}, delta={delta}");
    }

    #[test]
    fn shared_uneven_batch_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/statistics.json")).unwrap();
        let batches: Vec<_> = fixture["batches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| super::BatchMeasurement {
                average_micros: b["average_micros"].as_f64().unwrap(),
                operations: b["operations"].as_u64().unwrap() as usize,
            })
            .collect();
        let trial = serde_json::to_value(super::summarize_trial(1, &batches)).unwrap();
        for key in [
            "total_micros",
            "average_micros",
            "min_micros",
            "max_micros",
            "stddev_micros",
            "round_trip_rate",
        ] {
            approx_equal(trial[key].as_f64().unwrap(), fixture[key].as_f64().unwrap());
        }
    }

    #[test]
    fn retains_uneven_final_batch_and_contextual_failure() {
        let trial = super::measure_trial(2, 205, false, &mut || Ok(())).unwrap();
        assert_eq!(trial.samples.len(), 69);
        assert_eq!(trial.samples.last().unwrap().operations, 1);
        let error =
            super::measure_trial(7, 10, false, &mut || Err("peer died".into())).unwrap_err();
        assert!(error.to_string().contains("trial=7 iteration=1"));
        let single = super::summarize_trial(
            1,
            &[super::BatchMeasurement {
                average_micros: 2.0,
                operations: 1,
            }],
        );
        assert_eq!(single.stddev_micros, 0.0);
    }

    #[test]
    fn aggregates_trial_summaries() {
        let trials = vec![
            TrialSummary {
                trial_index: 1,
                samples: vec![],
                latency_percentiles_micros: None,
                round_trip_rate: 100.0,
                total_micros: 10.0,
                average_micros: 1.0,
                min_micros: 0.5,
                max_micros: 1.5,
                stddev_micros: 0.2,
                message_rate: 100.0,
            },
            TrialSummary {
                trial_index: 2,
                samples: vec![],
                latency_percentiles_micros: None,
                round_trip_rate: 140.0,
                total_micros: 14.0,
                average_micros: 1.4,
                min_micros: 0.4,
                max_micros: 1.8,
                stddev_micros: 0.4,
                message_rate: 140.0,
            },
        ];

        let aggregate = aggregate_trials(&trials, 10);

        approx_equal(aggregate.total_micros, 24.0);
        approx_equal(aggregate.average_micros, 1.2);
        approx_equal(aggregate.min_micros, 0.4);
        approx_equal(aggregate.max_micros, 1.8);
        approx_equal(aggregate.stddev_micros, 0.374_165_738_677_394_17);
        approx_equal(aggregate.message_rate, 833_333.333_333_333_4);
    }
}
