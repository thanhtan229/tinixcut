$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path 'src-tauri/target/release/tinixcut.exe').Path
$root = Join-Path $env:TEMP 'tinixcut_phase17_transport_recalibration'
$results = [System.Collections.Generic.List[object]]::new()

function New-Child([string]$mode) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $exe
    $psi.Arguments = $mode
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $proc = [System.Diagnostics.Process]::new()
    $proc.StartInfo = $psi
    if (-not $proc.Start()) { throw "Could not start tinixcut $mode" }
    return $proc
}

function Send-Request($proc, $request, [string]$idKey, $expectedId, [switch]$ReturnResponse) {
    $json = ConvertTo-Json -InputObject $request -Depth 100 -Compress
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $proc.StandardInput.WriteLine($json)
    $proc.StandardInput.Flush()
    $line = $proc.StandardOutput.ReadLine()
    $timer.Stop()
    if ([string]::IsNullOrWhiteSpace($line)) {
        throw "Empty response for request $expectedId; exited=$($proc.HasExited)"
    }
    $response = ConvertFrom-Json -InputObject $line
    if ($response.$idKey -ne $expectedId) {
        throw "Correlation mismatch: expected $expectedId; got $($response.$idKey): $line"
    }
    if ($idKey -eq 'requestId' -and $response.ok -ne $true) {
        throw "JSONL error response: $line"
    }
    if ($idKey -eq 'id' -and ($null -ne $response.error -or $response.result.isError -eq $true)) {
        throw "MCP error response: $line"
    }
    if ($ReturnResponse) {
        return [pscustomobject]@{ elapsed_ms = $timer.Elapsed.TotalMilliseconds; response = $response }
    }
    return $timer.Elapsed.TotalMilliseconds
}

function Get-Stats([double[]]$samples, [double]$wallMs) {
    $sorted = @($samples | Sort-Object)
    $n = $sorted.Count
    if ($n % 2) { $median = $sorted[[int][math]::Floor($n / 2)] }
    else { $median = ($sorted[$n / 2 - 1] + $sorted[$n / 2]) / 2 }
    $p95Index = [int][math]::Ceiling(0.95 * $n) - 1
    $sampleMean = ($samples | Measure-Object -Sum).Sum / $n
    return [ordered]@{
        n = $n
        median_ms = [math]::Round($median, 6)
        p95_ms = [math]::Round($sorted[$p95Index], 6)
        aggregate_wall_ms = [math]::Round($wallMs, 6)
        aggregate_avg_ms = [math]::Round($sampleMean, 6)
        wall_throughput_avg_ms = [math]::Round(($wallMs / $n), 6)
        min_ms = [math]::Round($sorted[0], 6)
        max_ms = [math]::Round($sorted[$n - 1], 6)
    }
}

function New-Envelope([string]$projectId, [long]$revision, [int]$sequence, [string]$suffix, [bool]$dryRun) {
    return @{
        schemaVersion = 3
        projectId = $projectId
        baseRevision = $revision
        clientId = 'phase17-recalibration'
        clientEpoch = 1
        sequenceNumber = $sequence
        dryRun = $dryRun
        transactionId = "tx-$suffix"
        operations = @(@{
            operationId = "op-$suffix"
            operation = @{
                type = 'TrackMuteSet'
                payload = @{ track_id = 'track-recalibration-w0'; muted = (($sequence % 2) -eq 0) }
            }
        })
    }
}

function New-EnvelopeForWorkload([string]$projectId, [string]$trackId, [long]$revision, [int]$sequence, [string]$suffix, [bool]$dryRun) {
    $identitySequence = if ($dryRun) { 1 } else { $sequence }
    return @{
        schemaVersion = 3
        projectId = $projectId
        baseRevision = $revision
        clientId = 'phase17-recalibration'
        clientEpoch = 1
        sequenceNumber = $identitySequence
        dryRun = $dryRun
        transactionId = "tx-$suffix"
        operations = @(@{
            operationId = "op-$suffix"
            operation = @{
                type = 'TrackMuteSet'
                payload = @{ track_id = $trackId; muted = (($sequence % 2) -eq 0) }
            }
        })
    }
}

foreach ($workload in @('W0', 'W1', 'W2')) {
    $path = Join-Path $root "$workload.tcproj"
    $project = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $trackId = $project.tracks[0].track_id
    $projectId = $project.project_id
    $stdoutStats = @{}
    $proc = New-Child 'session'
    try {
        $null = Send-Request $proc @{
            protocolVersion = 1; requestId = "open-$workload"; method = 'session.open'
            params = @{ path = $path }
        } 'requestId' "open-$workload"

        $inspect = {
            param($i)
            Send-Request $proc @{
                protocolVersion = 1; requestId = "inspect-$workload-$i"; method = 'project.inspect'; params = @{}
            } 'requestId' "inspect-$workload-$i"
        }
        1..5 | ForEach-Object { $null = & $inspect $_ }
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $samples = @(1..50 | ForEach-Object { & $inspect $_ })
        $watch.Stop()
        $stdoutStats['project.inspect'] = Get-Stats $samples $watch.Elapsed.TotalMilliseconds

        $dryRun = {
            param($i)
            $envl = New-EnvelopeForWorkload $projectId $trackId 0 $i "$workload-jsonl-dry-$i" $true
            Send-Request $proc @{
                protocolVersion = 1; requestId = "dry-$workload-$i"; method = 'command.dryRun'
                params = @{ envelope = $envl }
            } 'requestId' "dry-$workload-$i"
        }
        1..5 | ForEach-Object { $null = & $dryRun $_ }
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $samples = @(1..50 | ForEach-Object { & $dryRun $_ })
        $watch.Stop()
        $stdoutStats['command.dryRun'] = Get-Stats $samples $watch.Elapsed.TotalMilliseconds

        $applyCounter = [pscustomobject]@{ Value = 0 }
        $apply = {
            param($i)
            $applyCounter.Value++
            $seq = $applyCounter.Value
            $envl = New-EnvelopeForWorkload $projectId $trackId ($seq - 1) $seq "$workload-jsonl-apply-$seq" $false
            Send-Request $proc @{
                protocolVersion = 1; requestId = "apply-$workload-$seq"; method = 'command.apply'
                params = @{ envelope = $envl; saveAfter = $false }
            } 'requestId' "apply-$workload-$seq"
        }
        1..5 | ForEach-Object { $null = & $apply $_ }
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $samples = @(1..50 | ForEach-Object { & $apply $_ })
        $watch.Stop()
        $stdoutStats['command.apply.saveAfterFalse'] = Get-Stats $samples $watch.Elapsed.TotalMilliseconds
        $verify = Send-Request $proc @{
            protocolVersion = 1; requestId = "verify-$workload"; method = 'project.inspect'; params = @{}
        } 'requestId' "verify-$workload" -ReturnResponse
        if ($verify.response.result.revision -ne 55) {
            throw "JSONL apply verification failed for $workload; expected revision 55, got $($verify.response.result.revision)"
        }

        $null = Send-Request $proc @{
            protocolVersion = 1; requestId = "shutdown-$workload"; method = 'session.shutdown'
        } 'requestId' "shutdown-$workload"
        $proc.StandardInput.Close()
        $proc.WaitForExit()
        $results.Add([ordered]@{ workload = $workload; transport = 'jsonl_stdio'; metrics = $stdoutStats })
    } finally {
        if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit() }
        $proc.Dispose()
    }

    $stdoutStats = @{}
    $proc = New-Child 'mcp'
    try {
        $meta = @{
            'io.modelcontextprotocol/protocolVersion' = '2026-07-28'
            'io.modelcontextprotocol/clientCapabilities' = @{}
            'io.modelcontextprotocol/clientInfo' = @{ name = 'phase17-recalibration'; version = '1.0.0' }
        }
        $null = Send-Request $proc @{
            jsonrpc = '2.0'; id = 1; method = 'server/discover'; params = @{ _meta = $meta }
        } 'id' 1

        foreach ($toolName in @('tinixcut_inspect', 'tinixcut_dry_run', 'tinixcut_apply')) {
            $n = if ($toolName -eq 'tinixcut_apply') { 30 } else { 30 }
            $mcpCounter = [pscustomobject]@{ Value = 0 }
            $invoke = {
                param($i)
                $args = @{ projectPath = $path }
                if ($toolName -ne 'tinixcut_inspect') {
                    if ($toolName -eq 'tinixcut_apply') { $mcpCounter.Value++; $sequence = $mcpCounter.Value; $revision = $sequence - 1 }
                    else { $sequence = 1; $revision = 0 }
                    $operationSuffix = if ($toolName -eq 'tinixcut_apply') { $sequence } else { $i }
                    $envl = New-EnvelopeForWorkload $projectId $trackId $revision $sequence "$workload-$toolName-$operationSuffix" ($toolName -eq 'tinixcut_dry_run')
                    $args.envelope = $envl
                }
                Send-Request $proc @{
                    jsonrpc = '2.0'; id = "$workload-$toolName-$i"; method = 'tools/call'
                    params = @{ _meta = $meta; name = $toolName; arguments = $args }
                } 'id' "$workload-$toolName-$i"
            }
            1..5 | ForEach-Object { $null = & $invoke $_ }
            $watch = [System.Diagnostics.Stopwatch]::StartNew()
            $samples = @(1..$n | ForEach-Object { & $invoke $_ })
            $watch.Stop()
            $stdoutStats[$toolName] = Get-Stats $samples $watch.Elapsed.TotalMilliseconds
        }
        $proc.StandardInput.Close()
        $proc.WaitForExit()
        $stderr = $proc.StandardError.ReadToEnd()
        $results.Add([ordered]@{ workload = $workload; transport = 'mcp_stdio'; metrics = $stdoutStats; stderr = $stderr })
    } finally {
        if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit() }
        $proc.Dispose()
    }
}

$results | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $root 'transport-results.json') -Encoding utf8
$results | ConvertTo-Json -Depth 20
