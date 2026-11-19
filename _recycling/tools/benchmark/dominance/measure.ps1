param(
    [Parameter(Mandatory=$true)][string]$Baseline,
    [Parameter(Mandatory=$true)][string]$Candidate,
    [Parameter(Mandatory=$true)][string]$Output,
    [int]$Pairs=8,
    [int]$TimeoutSeconds=120,
    [string]$Cases
)
# This measures the identical ignored graph driver in two retained test binaries.
# Each child owns one graph; preparation and queries have separate child timers.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/native_peak.ps1"
if ($Pairs -lt 6) { throw 'Use at least six paired observations' }
$p=Get-Process -Id $PID
$p.PriorityClass='Idle'
$p.ProcessorAffinity=0xFFFFC0
$env:MBX_SCHEDULER_CPUS='18'
$env:CARGO_BUILD_JOBS='18'
$executables=@{baseline=(Resolve-Path -LiteralPath $Baseline).Path;candidate=(Resolve-Path -LiteralPath $Candidate).Path}
if (Test-Path -LiteralPath $Output) { throw 'Output directory must be new' }
New-Item -ItemType Directory -Force -Path $Output | Out-Null
$root=(Resolve-Path -LiteralPath $Output).Path
if ($Cases) {
    $workloads=@(Get-Content -Raw -LiteralPath $Cases | ConvertFrom-Json)
} else {
    $workloads=@(
        foreach ($shape in @('chain','diamonds','loops','disconnected')) {
            foreach ($count in @(4,16,64,512,1024,2048,4096)) {
                [pscustomobject]@{shape=$shape;blocks=$count}
            }
        }
        foreach ($count in @(64,128,256,512)) {
            [pscustomobject]@{shape='reverse_chain';blocks=$count}
        }
    )
}
$hashes=@{}
foreach ($role in @('baseline','candidate')) { $hashes[$role]=(Get-FileHash -Algorithm SHA256 -LiteralPath $executables[$role]).Hash }
$metadata=[ordered]@{schema='omega-dominance-comparison/1';started_utc=[DateTime]::UtcNow.ToString('o');host_name=[Environment]::MachineName;os=[Environment]::OSVersion.VersionString;affinity='0xFFFFC0';priority='Idle';pairs=$Pairs;warmup_pairs=1;executables=$executables;sha256=$hashes;cases=$workloads;script_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash;native_peak_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $PSScriptRoot 'native_peak.ps1')).Hash}
$metadata | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $root 'metadata.json')
$journal=Join-Path $root 'observations.jsonl'
function Observe([string]$Role,$Case,[int]$Round,[bool]$Warmup) {
    $name=('{0}-{1}-{2:D3}-{3}-{4}' -f $Case.shape,$Case.blocks,$Round,$Warmup,$Role)
    $start=[Diagnostics.ProcessStartInfo]::new($executables[$Role])
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardInput=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $start.Environment['OMEGA_DOMINANCE_SHAPE']=[string]$Case.shape
    $start.Environment['OMEGA_DOMINANCE_BLOCKS']=[string]$Case.blocks
    foreach ($arg in @('graph::tests::scaling_observation','--exact','--ignored','--nocapture','--test-threads','1')) { $start.ArgumentList.Add($arg) }
    $process=[Diagnostics.Process]::new()
    $process.StartInfo=$start
    $timer=[Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $resourceHandle=$process.SafeHandle
    $process.StandardInput.Close()
    $stdout=$process.StandardOutput.ReadToEndAsync()
    $stderr=$process.StandardError.ReadToEndAsync()
    $completed=$process.WaitForExit($TimeoutSeconds*1000)
    if (-not $completed) { $process.Kill($true); $process.WaitForExit() }
    $timer.Stop()
    $out=$stdout.GetAwaiter().GetResult()
    $err=$stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $root ($name+'.stdout')),$out)
    [IO.File]::WriteAllText((Join-Path $root ($name+'.stderr')),$err)
    if (-not $completed -or $process.ExitCode -ne 0) { throw "Driver failed: $name, exit $($process.ExitCode), completed $completed" }
    $reports=@([regex]::Matches($out,'(?m)\{\"shape\":.*\}\s*$') | ForEach-Object { $_.Value | ConvertFrom-Json })
    if ($reports.Count -ne 1) { throw "Expected one driver report: $name" }
    $report=$reports[0]
    if ($report.shape -ne $Case.shape -or $report.blocks -ne $Case.blocks -or $report.queries -ne 100000 -or $report.build_ns -le 0 -or $report.query_ns -le 0) { throw "Driver disagrees with workload: $name" }
    $record=[ordered]@{role=$Role;round=$Round;warmup=$Warmup;shape=$report.shape;blocks=$report.blocks;edges=$report.edges;queries=$report.queries;checksum=$report.checksum;build_ns=$report.build_ns;query_ns=$report.query_ns;retained_bytes=$report.retained_bytes;process_wall_ns=$timer.Elapsed.Ticks*100;cpu_ns=$process.TotalProcessorTime.Ticks*100;peak_working_set_bytes=[OmegaDominanceProcessResources]::PeakWorkingSetBytes($resourceHandle)}
    [IO.File]::AppendAllText($journal,($record | ConvertTo-Json -Compress)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
    $process.Dispose()
    return $record
}
foreach ($case in $workloads) {
    foreach ($warmup in @($true,$false)) {
        $rounds=if ($warmup) {1} else {$Pairs}
        for ($round=0;$round -lt $rounds;$round++) {
            $roles=if ($round%2 -eq 0) {@('baseline','candidate')} else {@('candidate','baseline')}
            $a=Observe $roles[0] $case $round $warmup
            $b=Observe $roles[1] $case $round $warmup
            if ($a.checksum -ne $b.checksum -or $a.edges -ne $b.edges -or $a.retained_bytes -le 0 -or $b.retained_bytes -le 0) { throw 'Comparable inputs produced different answers or invalid storage' }
        }
    }
    Write-Output ('Completed {0} {1}: {2} measured pairs' -f $case.shape,$case.blocks,$Pairs)
}
foreach ($role in @('baseline','candidate')) {
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $executables[$role]).Hash -ne $hashes[$role]) { throw 'Retained executable changed during measurement' }
}
@{completed_utc=[DateTime]::UtcNow.ToString('o');observations_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $journal).Hash} | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $root 'complete.json')
