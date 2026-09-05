param(
    [string]$OutputDir = $(Join-Path "results" (Get-Date -Format "yyyyMMdd-HHmmss")),
    [int[]]$MessageSizes = @(64, 1024, 4096, 16384, 32704),
    [int]$MessageCount = 1000,
    [int]$WarmupCount = 100,
    [int]$Trials = 3,
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
$arguments = @("run", "--output", $OutputDir, "--count", $MessageCount.ToString(),
    "--warmup", $WarmupCount.ToString(), "--trials", $Trials.ToString(),
    "--launches", $LaunchCount.ToString(), "--seed", $Seed.ToString(),
    "--timeout", $TimeoutSeconds.ToString(), "--validation", $Validation,
    "--measurement", $Measurement, "--sizes") + @($MessageSizes | ForEach-Object { $_.ToString() })
if ($StableAffinity) { $arguments += "--stable-affinity" }
if ($SkipPython) { $arguments += "--skip-python" }
Push-Location $repoRoot
try {
    uv run --locked --group build python scripts/benchmark_suite.py @arguments
    if ($LASTEXITCODE -ne 0) { throw "benchmark suite failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }
