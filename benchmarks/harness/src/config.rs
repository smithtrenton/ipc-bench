use std::{env, ffi::OsString};

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessRole {
    Parent,
    Child,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BenchmarkConfig {
    pub message_count: usize,
    pub message_size: usize,
    pub warmup_count: usize,
    pub trials: usize,
    pub output_format: OutputFormat,
    pub role: ProcessRole,
    pub validation: String,
    pub measurement: String,
    pub timeout_seconds: usize,
    pub workload: String,
    pub queue_depth: usize,
    pub ring_capacity: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            message_count: 1_000,
            message_size: 1_000,
            warmup_count: 100,
            trials: 3,
            output_format: OutputFormat::Text,
            role: ProcessRole::Parent,
            validation: "full".into(),
            measurement: "batch".into(),
            timeout_seconds: 120,
            workload: "round-trip".into(),
            queue_depth: 1,
            ring_capacity: 64,
        }
    }
}

impl BenchmarkConfig {
    pub fn from_env() -> Result<Self, String> {
        let config = Self::from_args(env::args().skip(1))?;
        if config.role == ProcessRole::Child {
            crate::fault::worker_started();
        }
        if config.workload != "round-trip" {
            let executable = env::current_exe().map_err(|e| e.to_string())?;
            let is_iocp = executable.file_stem().unwrap_or_default() == "named-pipe-iocp";
            if !is_iocp
                && !executable
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .starts_with("shm-ring-")
            {
                return Err(
                    "streaming/windowed workloads require shm-ring methods or named-pipe-iocp"
                        .into(),
                );
            }
        }
        crate::process::supervise(&config).map_err(|error| error.to_string())?;
        crate::affinity::apply_child_affinity_if_configured(config.role)
            .map_err(|error| format!("failed to apply child CPU affinity: {error}"))?;
        Ok(config)
    }

    pub fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Self::default();
        let args = args.into_iter().collect::<Vec<_>>();
        let mut index = 0;

        while index < args.len() {
            let current = &args[index];

            match current.as_str() {
                "--validation" | "--measurement" | "--workload" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| format!("missing value for {current}"))?;
                    match (current.as_str(), value.as_str()) {
                        ("--workload", "round-trip" | "streaming" | "windowed") => {
                            config.workload = value.clone()
                        }
                        ("--validation", "full" | "sampled") => config.validation = value.clone(),
                        ("--measurement", "batch" | "latency") => {
                            config.measurement = value.clone()
                        }
                        _ => return Err(format!("invalid value `{value}` for {current}")),
                    }
                }
                "--timeout-seconds" => {
                    config.timeout_seconds =
                        Self::parse_usize(&args, &mut index, current, "timeout")?;
                }
                "--queue-depth" => {
                    config.queue_depth =
                        Self::parse_usize(&args, &mut index, current, "queue depth")?
                }
                "--ring-capacity" => {
                    config.ring_capacity =
                        Self::parse_usize(&args, &mut index, current, "ring capacity")?
                }
                "-c" | "--message-count" => {
                    config.message_count =
                        Self::parse_usize(&args, &mut index, current, "message count")?;
                }
                "-s" | "--message-size" => {
                    config.message_size =
                        Self::parse_usize(&args, &mut index, current, "message size")?;
                }
                "-w" | "--warmup-count" => {
                    config.warmup_count =
                        Self::parse_usize(&args, &mut index, current, "warmup count")?;
                }
                "-t" | "--trials" => {
                    config.trials = Self::parse_usize(&args, &mut index, current, "trials")?;
                }
                "--format" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "missing value for --format".to_owned())?;
                    config.output_format = match value.as_str() {
                        "text" => OutputFormat::Text,
                        "json" => OutputFormat::Json,
                        _ => {
                            return Err(format!(
                                "invalid output format `{value}`; expected `text` or `json`"
                            ));
                        }
                    };
                }
                "--role" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "missing value for --role".to_owned())?;
                    config.role = match value.as_str() {
                        "parent" => ProcessRole::Parent,
                        "child" => ProcessRole::Child,
                        _ => {
                            return Err(format!(
                                "invalid process role `{value}`; expected `parent` or `child`"
                            ));
                        }
                    };
                }
                "--help" | "-h" => {
                    return Err(Self::usage());
                }
                _ => {
                    return Err(format!(
                        "unrecognized argument `{current}`\n\n{}",
                        Self::usage()
                    ));
                }
            }

            index += 1;
        }

        if config.message_size == 0 || config.message_size > 1024 * 1024 {
            return Err("message size must be between 1 and 1048576 payload bytes".into());
        }
        if config.timeout_seconds == 0 || config.timeout_seconds > 86400 {
            return Err("timeout must be between 1 and 86400 seconds".into());
        }
        if config.queue_depth == 0
            || config.queue_depth > 256
            || !config.ring_capacity.is_power_of_two()
            || config.ring_capacity > 256
        {
            return Err(
                "queue depth must be 1..256; ring capacity must be a power of two in 1..256".into(),
            );
        }
        if config.workload == "round-trip" && config.queue_depth != 1 {
            return Err("round-trip workload requires queue depth one".into());
        }
        if config.workload != "round-trip" && config.validation != "full" {
            return Err("throughput workloads require full delivery validation".into());
        }
        if config
            .message_count
            .checked_mul(config.trials)
            .and_then(|n| n.checked_add(config.warmup_count))
            .filter(|n| *n <= 1_000_000_000)
            .is_none()
        {
            return Err("total operation count exceeds 1000000000".into());
        }
        if config.message_count == 0 {
            return Err("message count must be greater than zero".to_owned());
        }
        let batch = if config.measurement == "latency" {
            1
        } else {
            crate::stats::measurement_batch_size(config.message_count)
        };
        if config
            .message_count
            .div_ceil(batch)
            .checked_mul(config.trials)
            .is_none_or(|n| n > 1_000_000)
        {
            return Err(
                "retained measurement samples exceed 1000000; reduce count/trials or use batches"
                    .into(),
            );
        }

        if config.trials == 0 {
            return Err("trials must be greater than zero".to_owned());
        }

        Ok(config)
    }

    pub fn child_args(&self) -> Vec<OsString> {
        self.args_for_role(ProcessRole::Child)
    }

    /// Eight sequence bytes are protocol overhead, excluded from payload byte rates.
    pub fn wire_size(&self) -> usize {
        self.message_size + 8
    }

    pub fn args_for_role(&self, role: ProcessRole) -> Vec<OsString> {
        let mut args = Vec::with_capacity(12);
        args.extend([
            OsString::from("--workload"),
            OsString::from(&self.workload),
            OsString::from("--queue-depth"),
            OsString::from(self.queue_depth.to_string()),
            OsString::from("--ring-capacity"),
            OsString::from(self.ring_capacity.to_string()),
            OsString::from("--validation"),
            OsString::from(&self.validation),
            OsString::from("--measurement"),
            OsString::from(&self.measurement),
            OsString::from("--timeout-seconds"),
            OsString::from(self.timeout_seconds.to_string()),
            OsString::from("--message-count"),
            OsString::from(self.message_count.to_string()),
            OsString::from("--message-size"),
            OsString::from(self.message_size.to_string()),
            OsString::from("--warmup-count"),
            OsString::from(self.warmup_count.to_string()),
            OsString::from("--trials"),
            OsString::from(self.trials.to_string()),
            OsString::from("--format"),
            OsString::from(match self.output_format {
                OutputFormat::Text => "text",
                OutputFormat::Json => "json",
            }),
            OsString::from("--role"),
            OsString::from(match role {
                ProcessRole::Parent => "parent",
                ProcessRole::Child => "child",
            }),
        ]);

        args
    }

    pub fn usage() -> String {
        let program_name = env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_stem()
                    .map(|value| value.to_string_lossy().into_owned())
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "ipc-bench".to_owned());

        [
            &format!("Usage: {program_name} [options]"),
            "",
            "Options:",
            "  -c, --message-count <N>  Number of measured round trips (default: 1000)",
            "  -s, --message-size <N>   Payload size in bytes (default: 1000)",
            "  -w, --warmup-count <N>   Warmup iterations before timing (default: 100)",
            "  -t, --trials <N>         Number of benchmark trials (default: 3)",
            "      --format <FORMAT>    Output format: text | json (default: text)",
            "      --role <ROLE>        Internal process role: parent | child",
            "      --validation <MODE>  full (default) | sampled (every 1024, plus preflight/final)",
            "      --measurement <MODE> batch (default) | latency (every operation)",
            "      --timeout-seconds <N> Process-tree deadline (default: 120)",
            "      --workload <MODE>    round-trip | streaming | windowed (rings; IOCP windowed)",
            "      --queue-depth <N>    Maximum outstanding requests, 1..256",
            "      --ring-capacity <N>  Power-of-two slots per direction, 1..256",
        ]
        .join("\n")
    }

    fn parse_usize(
        args: &[String],
        index: &mut usize,
        flag: &str,
        label: &str,
    ) -> Result<usize, String> {
        *index += 1;
        let value = args
            .get(*index)
            .ok_or_else(|| format!("missing value for {flag}"))?;

        value
            .parse::<usize>()
            .map_err(|_| format!("invalid {label} `{value}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::{BenchmarkConfig, OutputFormat, ProcessRole};

    #[test]
    fn parses_short_and_long_flags() {
        let config = BenchmarkConfig::from_args([
            "-c".to_owned(),
            "25".to_owned(),
            "--message-size".to_owned(),
            "512".to_owned(),
            "-w".to_owned(),
            "5".to_owned(),
            "--trials".to_owned(),
            "2".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
            "--role".to_owned(),
            "child".to_owned(),
        ])
        .expect("config should parse");

        assert_eq!(config.message_count, 25);
        assert_eq!(config.message_size, 512);
        assert_eq!(config.warmup_count, 5);
        assert_eq!(config.trials, 2);
        assert_eq!(config.output_format, OutputFormat::Json);
        assert_eq!(config.role, ProcessRole::Child);
    }

    #[test]
    fn child_args_preserve_parent_configuration() {
        let config = BenchmarkConfig {
            message_count: 7,
            message_size: 128,
            warmup_count: 2,
            trials: 4,
            output_format: OutputFormat::Json,
            role: ProcessRole::Parent,
            ..BenchmarkConfig::default()
        };

        let child = BenchmarkConfig::from_args(
            config
                .child_args()
                .into_iter()
                .map(|value| value.to_string_lossy().into_owned()),
        )
        .expect("child args should round-trip");

        assert_eq!(child.message_count, 7);
        assert_eq!(child.message_size, 128);
        assert_eq!(child.warmup_count, 2);
        assert_eq!(child.trials, 4);
        assert_eq!(child.output_format, OutputFormat::Json);
        assert_eq!(child.role, ProcessRole::Child);
    }

    #[test]
    fn rejects_zero_message_count() {
        let error = BenchmarkConfig::from_args(["--message-count".to_owned(), "0".to_owned()])
            .expect_err("zero message count should fail");

        assert!(error.contains("message count"));
    }

    #[test]
    fn rejects_invalid_bounds_before_allocation() {
        for args in [
            vec!["--message-size", "0"],
            vec!["--message-size", "18446744073709551615"],
            vec!["--message-count", "18446744073709551615", "--trials", "2"],
            vec!["--trials", "0"],
            vec!["--timeout-seconds", "0"],
            vec!["--ring-capacity", "3"],
            vec!["--queue-depth", "0"],
        ] {
            assert!(BenchmarkConfig::from_args(args.into_iter().map(str::to_owned)).is_err());
        }
    }
}
