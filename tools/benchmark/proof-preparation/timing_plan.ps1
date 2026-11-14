param(
    [Parameter(Mandatory=$true)][string]$Baseline,
    [Parameter(Mandatory=$true)][string]$Candidate,
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][int]$Repeats,
    [Parameter(Mandatory=$true)][ulong]$WorkUnits,
    [Parameter(Mandatory=$true)][string]$Checksum,
    [Parameter(Mandatory=$true)][string]$Output
)
# The timing examples compile allocator counters out and forward to System.
# This region replays preparation alone, excluding source setup.
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $Output) { throw 'Plan path must be new' }
$sourcePath=(Resolve-Path -LiteralPath $Source).Path
$arguments=@($sourcePath,[string]$Repeats,'prepare')
$plan=[ordered]@{
    schema='omega-performance-plan/1';name='Real-context proof preparation: '+(Split-Path (Split-Path $sourcePath) -Leaf)
    working_directory=$PWD.Path;inputs=@((Split-Path $sourcePath),(Join-Path $PWD 'source/library'))
    baseline=@{program=(Resolve-Path -LiteralPath $Baseline).Path;arguments=$arguments}
    candidate=@{program=(Resolve-Path -LiteralPath $Candidate).Path;arguments=$arguments}
    warmups=1;pairs=8;timeout_ms=120000
    measurement=@{kind='reported_work';stdin='';work_units=$WorkUnits;checksum=$Checksum}
}
[IO.File]::WriteAllText([IO.Path]::GetFullPath($Output),($plan | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
