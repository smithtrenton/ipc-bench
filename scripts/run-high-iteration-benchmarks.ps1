param(
    [string]$OutputDir = $(Join-Path "results" (Get-Date -Format "yyyyMMdd-HHmmss-high")),
    [int[]]$MessageSizes = @(64, 1024, 4096, 16384, 32704),
    [int]$DefaultMessageCount = 100000,
    [int]$DefaultWarmupCount = 10000,
    [int]$DefaultTrials = 7,
    [int]$MailslotMessageCount = 5000,
    [int]$MailslotWarmupCount = 200,
    [int]$MailslotTrials = 5,
    [ValidateRange(1, 99)]
    [int]$LaunchCount = 5,
    [switch]$StableAffinity,
    [switch]$SkipPython,
    [int]$Seed = 20260904,
    [int]$TimeoutSeconds = 120,
    [ValidateSet("full", "sampled")]
    [string]$Validation = "full",
    [ValidateSet("batch", "latency")]
    [string]$Measurement = "batch"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
$arguments = @("run", "--output", $OutputDir, "--count", $DefaultMessageCount.ToString(),
    "--warmup", $DefaultWarmupCount.ToString(), "--trials", $DefaultTrials.ToString(),
    "--launches", $LaunchCount.ToString(), "--seed", $Seed.ToString(),
    "--timeout", $TimeoutSeconds.ToString(), "--validation", $Validation,
    "--measurement", $Measurement, "--sizes") + @($MessageSizes | ForEach-Object { $_.ToString() })
$arguments += @("--mailslot-count", $MailslotMessageCount.ToString(), "--mailslot-warmup", $MailslotWarmupCount.ToString(), "--mailslot-trials", $MailslotTrials.ToString())
if ($StableAffinity) { $arguments += "--stable-affinity" }
if ($SkipPython) { $arguments += "--skip-python" }
Push-Location $repoRoot
try {
    uv run --locked --group build python scripts/benchmark_suite.py @arguments
    if ($LASTEXITCODE -ne 0) { throw "benchmark suite failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }
