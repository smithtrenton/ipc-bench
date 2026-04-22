param(
    [string]$OutputDir = "results\published\windows11-high-iterations",
    [int[]]$MessageSizes = @(64, 1024, 4096, 16384, 32704),
    [int]$DefaultMessageCount = 100000,
    [int]$DefaultWarmupCount = 10000,
    [int]$DefaultTrials = 7,
    [int]$MailslotMessageCount = 5000,
    [int]$MailslotWarmupCount = 200,
    [int]$MailslotTrials = 5,
    [ValidateRange(1, 99)]
    [int]$LaunchCount = 1,
    [switch]$StableAffinity,
    [switch]$SkipPython
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$outputPath = Join-Path $repoRoot $OutputDir
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

function Resolve-UvCommand {
    Get-Command uv -ErrorAction SilentlyContinue
}

function Invoke-UvPythonCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    if (-not $script:uvCommand) {
        throw "uv is required for Python benchmark execution"
    }

    & $script:uvCommand.Path "run" "--python" "3.14" "python" @Arguments
}

function Get-BenchmarkParams {
    param([string]$Method)

    if ($Method -eq "mailslot") {
        return @{
            message_count = $MailslotMessageCount
            warmup_count = $MailslotWarmupCount
            trials = $MailslotTrials
        }
    }

    return @{
        message_count = $DefaultMessageCount
        warmup_count = $DefaultWarmupCount
        trials = $DefaultTrials
    }
}

function Write-Manifest {
    $script:manifest | ConvertTo-Json -Depth 6 | Set-Content -Path $script:manifestPath
}

function Write-RunStatus {
    param(
        [string]$Status,
        [string]$ErrorMessage = $null
    )

    $completedCount = 0
    foreach ($entry in $script:manifest) {
        if ($entry.status -eq "completed") {
            $completedCount += 1
        }
    }

    $runState = New-Object PSObject
    $runState | Add-Member -NotePropertyName started_at -NotePropertyValue $script:startedAt
    $runState | Add-Member -NotePropertyName updated_at -NotePropertyValue ((Get-Date).ToString("o"))
    $runState | Add-Member -NotePropertyName status -NotePropertyValue $Status
    $runState | Add-Member -NotePropertyName build_profile -NotePropertyValue "release"
    $runState | Add-Member -NotePropertyName completed -NotePropertyValue $completedCount
    $runState | Add-Member -NotePropertyName failed -NotePropertyValue $script:failures.Count
    $runState | Add-Member -NotePropertyName error -NotePropertyValue $ErrorMessage
    $runState | Add-Member -NotePropertyName failures -NotePropertyValue ([object[]]$script:failures.ToArray())
    $runState | ConvertTo-Json -Depth 8 | Set-Content -Path $script:runStatusPath
}

function Invoke-BenchmarkCommand {
    param(
        [scriptblock]$Command,
        [bool]$EnableStableAffinity
    )

    $hadNativePreference = $null -ne (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hadNativePreference) {
        $previousNativePreference = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }

    $hadStableAffinity = Test-Path Env:IPC_BENCH_STABLE_AFFINITY
    $previousStableAffinity = if ($hadStableAffinity) { $env:IPC_BENCH_STABLE_AFFINITY } else { $null }
    if ($EnableStableAffinity) {
        $env:IPC_BENCH_STABLE_AFFINITY = "1"
    }
    elseif ($hadStableAffinity) {
        Remove-Item Env:IPC_BENCH_STABLE_AFFINITY
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"

    try {
        $output = & $Command 2>&1
        $exitCode = if ($null -ne $LASTEXITCODE) { $LASTEXITCODE } else { 0 }
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        if ($EnableStableAffinity) {
            if ($hadStableAffinity) {
                $env:IPC_BENCH_STABLE_AFFINITY = $previousStableAffinity
            }
            else {
                Remove-Item Env:IPC_BENCH_STABLE_AFFINITY -ErrorAction SilentlyContinue
            }
        }
        elseif ($hadStableAffinity) {
            $env:IPC_BENCH_STABLE_AFFINITY = $previousStableAffinity
        }

        if ($hadNativePreference) {
            $PSNativeCommandUseErrorActionPreference = $previousNativePreference
        }
    }

    $outputText = (($output | ForEach-Object { "$_" }) -join "`n").Trim()

    if ($exitCode -ne 0) {
        if ([string]::IsNullOrWhiteSpace($outputText)) {
            $outputText = "benchmark command failed with exit code $exitCode"
        }

        return [PSCustomObject]@{
            status = "failed"
            exit_code = $exitCode
            error = $outputText
            report = $null
        }
    }

    if ([string]::IsNullOrWhiteSpace($outputText)) {
        return [PSCustomObject]@{
            status = "failed"
            exit_code = 0
            error = "benchmark command completed without emitting JSON"
            report = $null
        }
    }

    try {
        $report = $outputText | ConvertFrom-Json
    }
    catch {
        return [PSCustomObject]@{
            status = "failed"
            exit_code = 0
            error = "benchmark command completed but did not emit valid JSON: $($_.Exception.Message)"
            report = $null
        }
    }

    [PSCustomObject]@{
        status = "completed"
        exit_code = 0
        error = $null
        report = $report
    }
}

function Get-PercentileValue {
    param(
        [double[]]$SortedValues,
        [double]$Percentile
    )

    if ($SortedValues.Count -eq 0) {
        throw "cannot compute a percentile for an empty value set"
    }

    if ($SortedValues.Count -eq 1) {
        return [double]$SortedValues[0]
    }

    $position = ($SortedValues.Count - 1) * ($Percentile / 100.0)
    $lowerIndex = [int][Math]::Floor($position)
    $upperIndex = [int][Math]::Ceiling($position)
    if ($lowerIndex -eq $upperIndex) {
        return [double]$SortedValues[$lowerIndex]
    }

    $weight = $position - $lowerIndex
    return [double](
        $SortedValues[$lowerIndex] +
        (($SortedValues[$upperIndex] - $SortedValues[$lowerIndex]) * $weight)
    )
}

function Get-LaunchMetricStats {
    param([double[]]$Values)

    if ($Values.Count -eq 0) {
        throw "cannot summarize an empty launch set"
    }

    $sorted = @($Values | Sort-Object)
    $sum = 0.0
    foreach ($value in $sorted) {
        $sum += [double]$value
    }
    $mean = $sum / $sorted.Count

    $variance = 0.0
    foreach ($value in $sorted) {
        $delta = [double]$value - $mean
        $variance += $delta * $delta
    }
    $variance /= $sorted.Count

    [ordered]@{
        median = [double](Get-PercentileValue -SortedValues $sorted -Percentile 50)
        p10 = [double](Get-PercentileValue -SortedValues $sorted -Percentile 10)
        p90 = [double](Get-PercentileValue -SortedValues $sorted -Percentile 90)
        mean = [double]$mean
        min = [double]$sorted[0]
        max = [double]$sorted[$sorted.Count - 1]
        stddev = [double][Math]::Sqrt($variance)
    }
}

function New-AggregatedReport {
    param(
        [string]$Method,
        [string]$Language,
        [int]$MessageSize,
        [int]$MessageCount,
        [int]$WarmupCount,
        [int]$Trials,
        [int]$LaunchCountValue,
        [bool]$StableAffinityEnabled,
        [object[]]$LaunchReports
    )

    $averageStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.average_micros })
    $totalStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.total_micros })
    $minStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.min_micros })
    $maxStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.max_micros })
    $stddevStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.stddev_micros })
    $rateStats = Get-LaunchMetricStats -Values @($LaunchReports | ForEach-Object { [double]$_.summary.message_rate })

    [ordered]@{
        method = $Method
        language = $Language
        launch_count = $LaunchCountValue
        stable_affinity = $StableAffinityEnabled
        child_ready = (@($LaunchReports | Where-Object { -not $_.child_ready }).Count -eq 0)
        config = [ordered]@{
            message_count = $MessageCount
            message_size = $MessageSize
            warmup_count = $WarmupCount
            trials = $Trials
            output_format = "json"
            role = "parent"
        }
        methodology = [ordered]@{
            launch_count = $LaunchCountValue
            stable_affinity = $StableAffinityEnabled
            representative_metric = "median_average_micros"
            spread_metrics = @("p10_average_micros", "p90_average_micros")
        }
        launches = @(
            for ($launchIndex = 0; $launchIndex -lt $LaunchReports.Count; $launchIndex++) {
                [ordered]@{
                    launch_index = $launchIndex + 1
                    report = $LaunchReports[$launchIndex]
                }
            }
        )
        summary = [ordered]@{
            total_micros = $totalStats.median
            average_micros = $averageStats.median
            min_micros = $minStats.median
            max_micros = $maxStats.median
            stddev_micros = $stddevStats.median
            message_rate = $rateStats.median
            mean_average_micros = $averageStats.mean
            min_average_micros = $averageStats.min
            max_average_micros = $averageStats.max
            p10_average_micros = $averageStats.p10
            p90_average_micros = $averageStats.p90
            launch_stddev_average_micros = $averageStats.stddev
        }
    }
}

function New-FailureReport {
    param(
        [hashtable]$Entry,
        [int]$LaunchCountValue,
        [bool]$StableAffinityEnabled,
        [int]$FailedLaunch,
        [object]$FailedResult,
        [object[]]$CompletedLaunchReports
    )

    [ordered]@{
        method = $Entry.method
        language = $Entry.language
        message_size = $Entry.message_size
        message_count = $Entry.message_count
        warmup_count = $Entry.warmup_count
        trials = $Entry.trials
        launch_count = $LaunchCountValue
        stable_affinity = $StableAffinityEnabled
        status = "failed"
        exit_code = $FailedResult.exit_code
        error = $FailedResult.error
        failed_launch = $FailedLaunch
        launches = @(
            for ($launchIndex = 0; $launchIndex -lt $CompletedLaunchReports.Count; $launchIndex++) {
                [ordered]@{
                    launch_index = $launchIndex + 1
                    report = $CompletedLaunchReports[$launchIndex]
                }
            }
        )
    }
}

$uvCommand = Resolve-UvCommand
if (-not $SkipPython -and -not $uvCommand) {
    throw "uv is required to run Python benchmarks; install uv or pass -SkipPython."
}

$nativeMethods = @(
    "copy-roundtrip",
    "anon-pipe",
    "named-pipe-byte-sync",
    "named-pipe-message-sync",
    "named-pipe-overlapped",
    "tcp-loopback",
    "shm-events",
    "shm-semaphores",
    "shm-mailbox-spin",
    "shm-mailbox-hybrid",
    "shm-ring-spin",
    "shm-ring-hybrid",
    "shm-raw-sync-event",
    "shm-raw-sync-busy",
    "iceoryx2-publish-subscribe-loan",
    "iceoryx2-request-response-loan",
    "af-unix",
    "udp-loopback",
    "mailslot",
    "rpc",
    "alpc"
)

$pythonMethods = @(
    @{ Name = "py-multiprocessing-pipe"; Module = "benchmarks.methods.python.py_multiprocessing_pipe.run" },
    @{ Name = "py-multiprocessing-queue"; Module = "benchmarks.methods.python.py_multiprocessing_queue.run" },
    @{ Name = "py-socket-tcp-loopback"; Module = "benchmarks.methods.python.py_socket_tcp_loopback.run" },
    @{ Name = "py-shared-memory-events"; Module = "benchmarks.methods.python.py_shared_memory_events.run" },
    @{ Name = "py-shared-memory-queue"; Module = "benchmarks.methods.python.py_shared_memory_queue.run" }
)

$startedAt = (Get-Date).ToString("o")
$manifest = New-Object System.Collections.Generic.List[object]
$failures = New-Object System.Collections.Generic.List[object]
$manifestPath = Join-Path $outputPath "manifest.json"
$runStatusPath = Join-Path $outputPath "run-status.json"
$summaryJsonPath = Join-Path $outputPath "summary.json"
$summaryCsvPath = Join-Path $outputPath "summary.csv"

Push-Location $repoRoot
try {
    Write-RunStatus -Status "running"

    cargo build --release --workspace | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    $metadata = [ordered]@{
        generated_at = (Get-Date).ToString("o")
        operating_system = (Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber)
        processor = (Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors)
        rustc = (& rustc --version)
        cargo = (& cargo --version)
        python = if ($uvCommand -and -not $SkipPython) { (Invoke-UvPythonCommand -Arguments @("--version") 2>&1) } else { $null }
        build_profile = "release"
        methodology = [ordered]@{
            default = @{
                message_count = $DefaultMessageCount
                warmup_count = $DefaultWarmupCount
                trials = $DefaultTrials
            }
            overrides = @{
                mailslot = @{
                    message_count = $MailslotMessageCount
                    warmup_count = $MailslotWarmupCount
                    trials = $MailslotTrials
                }
            }
            launch_count = $LaunchCount
            stable_affinity = $StableAffinity.IsPresent
            representative_metric = "median_average_micros"
            spread_metrics = @("p10_average_micros", "p90_average_micros")
            note = "High-iteration rerun with fixed counts across the message-size matrix. Each summary row aggregates fresh launches and uses the median launch-average latency as summary.average_micros."
        }
        message_sizes = $MessageSizes
        parameters = @{
            output_dir = $OutputDir
        }
    }
    $metadata | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-Path $outputPath "metadata.json")

    foreach ($size in $MessageSizes) {
        foreach ($method in $nativeMethods) {
            $params = Get-BenchmarkParams $method
            $fileName = "{0}-s{1}-native.json" -f $method, $size
            $destination = Join-Path $outputPath $fileName
            $entry = [ordered]@{
                method = $method
                language = "rust"
                message_size = $size
                output = $fileName
                message_count = $params.message_count
                warmup_count = $params.warmup_count
                trials = $params.trials
                launch_count = $LaunchCount
                stable_affinity = $StableAffinity.IsPresent
            }

            $launchReports = New-Object System.Collections.Generic.List[object]
            $failedResult = $null
            $failedLaunch = $null

            for ($launchIndex = 1; $launchIndex -le $LaunchCount; $launchIndex++) {
                Write-Host ("Running {0} (size={1}, count={2}, warmup={3}, trials={4}, launch={5}/{6})" -f $method, $size, $params.message_count, $params.warmup_count, $params.trials, $launchIndex, $LaunchCount)
                $result = Invoke-BenchmarkCommand -EnableStableAffinity $StableAffinity.IsPresent -Command {
                    cargo run --release -q -p $method -- --message-count $params.message_count --message-size $size --warmup-count $params.warmup_count --trials $params.trials --format json
                }
                if ($result.status -ne "completed") {
                    $failedResult = $result
                    $failedLaunch = $launchIndex
                    break
                }

                $launchReports.Add($result.report) | Out-Null
            }

            if ($null -ne $failedResult) {
                $failureReport = New-FailureReport -Entry $entry -LaunchCountValue $LaunchCount -StableAffinityEnabled $StableAffinity.IsPresent -FailedLaunch $failedLaunch -FailedResult $failedResult -CompletedLaunchReports ([object[]]$launchReports.ToArray())
                $failureReport | ConvertTo-Json -Depth 12 | Set-Content -Path $destination
                $entry.status = "failed"
                $entry.exit_code = $failedResult.exit_code
                $entry.error = $failedResult.error
                $script:failures.Add([ordered]@{
                    method = $method
                    language = "rust"
                    message_size = $size
                    output = $fileName
                    exit_code = $failedResult.exit_code
                    error = $failedResult.error
                    failed_launch = $failedLaunch
                }) | Out-Null
            }
            else {
                $aggregateReport = New-AggregatedReport -Method $method -Language "rust" -MessageSize $size -MessageCount $params.message_count -WarmupCount $params.warmup_count -Trials $params.trials -LaunchCountValue $LaunchCount -StableAffinityEnabled $StableAffinity.IsPresent -LaunchReports ([object[]]$launchReports.ToArray())
                $aggregateReport | ConvertTo-Json -Depth 12 | Set-Content -Path $destination
                $entry.status = "completed"
                $entry.exit_code = 0
            }

            $script:manifest.Add($entry) | Out-Null
            Write-Manifest
            Write-RunStatus -Status "running"
        }

        if (-not $SkipPython -and $uvCommand) {
            foreach ($method in $pythonMethods) {
                $params = Get-BenchmarkParams $method.Name
                $fileName = "{0}-s{1}-python.json" -f $method.Name, $size
                $destination = Join-Path $outputPath $fileName
                $entry = [ordered]@{
                    method = $method.Name
                    language = "python"
                    message_size = $size
                    output = $fileName
                    message_count = $params.message_count
                    warmup_count = $params.warmup_count
                    trials = $params.trials
                    launch_count = $LaunchCount
                    stable_affinity = $StableAffinity.IsPresent
                }

                $launchReports = New-Object System.Collections.Generic.List[object]
                $failedResult = $null
                $failedLaunch = $null

                for ($launchIndex = 1; $launchIndex -le $LaunchCount; $launchIndex++) {
                    Write-Host ("Running {0} (size={1}, count={2}, warmup={3}, trials={4}, launch={5}/{6})" -f $method.Name, $size, $params.message_count, $params.warmup_count, $params.trials, $launchIndex, $LaunchCount)
                    $result = Invoke-BenchmarkCommand -EnableStableAffinity $StableAffinity.IsPresent -Command {
                        Invoke-UvPythonCommand -Arguments @(
                            "-m", $method.Module,
                            "--message-count",
                            $params.message_count,
                            "--message-size",
                            $size,
                            "--warmup-count",
                            $params.warmup_count,
                            "--trials",
                            $params.trials,
                            "--format",
                            "json"
                        )
                    }
                    if ($result.status -ne "completed") {
                        $failedResult = $result
                        $failedLaunch = $launchIndex
                        break
                    }

                    $launchReports.Add($result.report) | Out-Null
                }

                if ($null -ne $failedResult) {
                    $failureReport = New-FailureReport -Entry $entry -LaunchCountValue $LaunchCount -StableAffinityEnabled $StableAffinity.IsPresent -FailedLaunch $failedLaunch -FailedResult $failedResult -CompletedLaunchReports ([object[]]$launchReports.ToArray())
                    $failureReport | ConvertTo-Json -Depth 12 | Set-Content -Path $destination
                    $entry.status = "failed"
                    $entry.exit_code = $failedResult.exit_code
                    $entry.error = $failedResult.error
                    $script:failures.Add([ordered]@{
                        method = $method.Name
                        language = "python"
                        message_size = $size
                        output = $fileName
                        exit_code = $failedResult.exit_code
                        error = $failedResult.error
                        failed_launch = $failedLaunch
                    }) | Out-Null
                }
                else {
                    $aggregateReport = New-AggregatedReport -Method $method.Name -Language "python" -MessageSize $size -MessageCount $params.message_count -WarmupCount $params.warmup_count -Trials $params.trials -LaunchCountValue $LaunchCount -StableAffinityEnabled $StableAffinity.IsPresent -LaunchReports ([object[]]$launchReports.ToArray())
                    $aggregateReport | ConvertTo-Json -Depth 12 | Set-Content -Path $destination
                    $entry.status = "completed"
                    $entry.exit_code = 0
                }

                $script:manifest.Add($entry) | Out-Null
                Write-Manifest
                Write-RunStatus -Status "running"
            }
        }
    }

    $summary = foreach ($entry in $manifest) {
        $reportPath = Join-Path $outputPath $entry.output
        $report = Get-Content -Raw -Path $reportPath | ConvertFrom-Json
        $hasSummary = $report.PSObject.Properties.Name -contains "summary"
        [PSCustomObject]@{
            method = $entry.method
            language = $entry.language
            message_size = $entry.message_size
            message_count = $entry.message_count
            warmup_count = $entry.warmup_count
            trials = $entry.trials
            launch_count = if ($report.PSObject.Properties.Name -contains "launch_count") { $report.launch_count } else { $entry.launch_count }
            stable_affinity = if ($report.PSObject.Properties.Name -contains "stable_affinity") { $report.stable_affinity } else { $entry.stable_affinity }
            representative_metric = if ($hasSummary) { "median_average_micros" } else { $null }
            status = if ($hasSummary) { "completed" } else { $report.status }
            exit_code = if ($hasSummary) { 0 } else { $report.exit_code }
            error = if ($hasSummary) { $null } else { $report.error }
            average_micros = if ($hasSummary) { $report.summary.average_micros } else { $null }
            mean_average_micros = if ($hasSummary) { $report.summary.mean_average_micros } else { $null }
            p10_average_micros = if ($hasSummary) { $report.summary.p10_average_micros } else { $null }
            p90_average_micros = if ($hasSummary) { $report.summary.p90_average_micros } else { $null }
            min_average_micros = if ($hasSummary) { $report.summary.min_average_micros } else { $null }
            max_average_micros = if ($hasSummary) { $report.summary.max_average_micros } else { $null }
            launch_stddev_average_micros = if ($hasSummary) { $report.summary.launch_stddev_average_micros } else { $null }
            total_micros = if ($hasSummary) { $report.summary.total_micros } else { $null }
            min_micros = if ($hasSummary) { $report.summary.min_micros } else { $null }
            max_micros = if ($hasSummary) { $report.summary.max_micros } else { $null }
            stddev_micros = if ($hasSummary) { $report.summary.stddev_micros } else { $null }
            message_rate = if ($hasSummary) { $report.summary.message_rate } else { $null }
            child_ready = if ($report.PSObject.Properties.Name -contains "child_ready") { $report.child_ready } else { $null }
            output = $entry.output
        }
    }

    $summary | ConvertTo-Json -Depth 6 | Set-Content -Path $summaryJsonPath
    $summary | Export-Csv -NoTypeInformation -Path $summaryCsvPath

    if ($failures.Count -gt 0) {
        $errorMessage = "$($failures.Count) benchmark runs failed. See run-status.json for details."
        Write-RunStatus -Status "failed" -ErrorMessage $errorMessage
        throw $errorMessage
    }

    Write-RunStatus -Status "completed"
}
catch {
    Write-Manifest
    Write-RunStatus -Status "failed" -ErrorMessage $_.Exception.Message
    throw
}
finally {
    Pop-Location
}
