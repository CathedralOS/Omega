$ErrorActionPreference = 'Stop'
$experimentRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$evidence = Join-Path $experimentRoot 'build/parser-pool-experiment'
$producers = Get-Content -Raw (Join-Path $evidence 'producers.json') | ConvertFrom-Json
$observations = foreach ($flavor in @('baseline', 'candidate')) {
    $cache = Join-Path $evidence "thread-cache-qualified-$flavor"
    $null = New-Item -ItemType Directory -Path $cache
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo.FileName = $producers.$flavor.binary
    $process.StartInfo.WorkingDirectory = $experimentRoot
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.Environment['RAYON_NUM_THREADS'] = '32'
    $process.StartInfo.Environment['OMEGA_PHASE_PROFILE'] = '1'
    $process.StartInfo.Environment['LOCALAPPDATA'] = $cache
    $process.StartInfo.Environment['OMEGA_LIBRARY_ROOT'] = Join-Path $experimentRoot 'source/library'
    $process.StartInfo.ArgumentList.Add('--timings')
    $process.StartInfo.ArgumentList.Add('--check')
    $process.StartInfo.ArgumentList.Add((Join-Path $experimentRoot 'samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine/main.omg'))
    $null = $process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $peak = 0
    while (-not $process.WaitForExit(10)) {
        $process.Refresh()
        $peak = [Math]::Max($peak, $process.Threads.Count)
    }
    $process.WaitForExit()
    $observation = [pscustomobject]@{
        flavor = $flavor
        workers_per_pool = 32
        observed_peak_root_threads = $peak
        exit = $process.ExitCode
        stdout = $stdout.Result
        stderr = $stderr.Result
    }
    $process.Dispose()
    $observation | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $evidence "thread-$flavor.json")
    if ($observation.exit -ne 0 -or $observation.stderr -notmatch 'source.graph.cold' -or $observation.stderr -notmatch 'machine_loop') {
        throw "Cold frontend was not observed for $flavor"
    }
    $observation
}
$observations | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $evidence 'thread-probes.json')
$observations | Select-Object flavor, workers_per_pool, observed_peak_root_threads, exit | Format-Table
if (($observations[0].observed_peak_root_threads - $observations[1].observed_peak_root_threads) -lt 30) {
    throw 'The candidate did not remove the expected global pool'
}
