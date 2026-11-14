param(
    [Parameter(Mandatory=$true)][string]$Program,
    [Parameter(Mandatory=$true)][string[]]$ProgramArguments,
    [Parameter(Mandatory=$true)][string]$Output,
    [int]$TimeoutSeconds=120,
    [switch]$ProfileKnown
)
# Capture one process with stdin closed, under the shared workstation budget.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/../dominance/native_peak.ps1"
$shellProcess=Get-Process -Id $PID
$shellProcess.PriorityClass='Idle'
$shellProcess.ProcessorAffinity=0xFFFFC0
$env:MBX_SCHEDULER_CPUS='18'
$env:CARGO_BUILD_JOBS='18'
if (Test-Path -LiteralPath $Output) { throw 'Output directory must be new' }
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$outputPath=(Resolve-Path -LiteralPath $Output).Path
$programPath=(Resolve-Path -LiteralPath $Program).Path
$start=[Diagnostics.ProcessStartInfo]::new($programPath)
$start.UseShellExecute=$false
$start.CreateNoWindow=$true
$start.RedirectStandardInput=$true
$start.RedirectStandardOutput=$true
$start.RedirectStandardError=$true
$start.Environment['OMEGA_LIBRARY_ROOT']=(Join-Path $PWD 'source/library')
if ($ProfileKnown) { $start.Environment['OMEGA_PROFILE_KNOWN']='1' }
foreach ($argument in $ProgramArguments) { $start.ArgumentList.Add($argument) }
$process=[Diagnostics.Process]::new()
$process.StartInfo=$start
$timer=[Diagnostics.Stopwatch]::StartNew()
[void]$process.Start()
$handle=$process.SafeHandle
$process.StandardInput.Close()
$stdout=$process.StandardOutput.ReadToEndAsync()
$stderr=$process.StandardError.ReadToEndAsync()
$completed=$process.WaitForExit($TimeoutSeconds*1000)
if (-not $completed) { $process.Kill($true); $process.WaitForExit() }
$timer.Stop()
[IO.File]::WriteAllText((Join-Path $outputPath 'stdout.txt'),$stdout.GetAwaiter().GetResult())
[IO.File]::WriteAllText((Join-Path $outputPath 'stderr.txt'),$stderr.GetAwaiter().GetResult())
$record=[ordered]@{
    program=$programPath;sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $programPath).Hash
    arguments=$ProgramArguments;completed=$completed;exit_code=$process.ExitCode
    wall_ns=$timer.Elapsed.Ticks*100;cpu_ns=$process.TotalProcessorTime.Ticks*100
    peak_working_set_bytes=[OmegaDominanceProcessResources]::PeakWorkingSetBytes($handle)
    affinity='0xFFFFC0';priority='Idle'
}
$record | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $outputPath 'observation.json')
$process.Dispose()
Write-Output ($record | ConvertTo-Json -Compress -Depth 6)
if (-not $completed -or $record.exit_code -ne 0) { throw 'Observed process failed; retained output explains the failure' }
