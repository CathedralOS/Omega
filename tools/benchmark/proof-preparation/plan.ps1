param(
    [Parameter(Mandatory=$true)][string]$Baseline,
    [Parameter(Mandatory=$true)][string]$Candidate,
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$Output,
    [int]$Pairs=8
)
# This is the Build-independent source/Product route, not full compilation or
# native publication. The process oracle is the example's accepted verdict.
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $Output) { throw 'Plan path must be new' }
$sourcePath=(Resolve-Path -LiteralPath $Source).Path
$library=Join-Path $PWD 'source/library'
$arguments=@($sourcePath,'--entry','main','--target','windows_x86_64')
$plan=[ordered]@{
    schema='omega-performance-plan/1'
    name='Proof preparation source-route control: '+(Split-Path (Split-Path $sourcePath) -Leaf)
    working_directory=$PWD.Path
    inputs=@((Split-Path $sourcePath),$library)
    baseline=@{program=(Resolve-Path -LiteralPath $Baseline).Path;arguments=$arguments;environment=@{OMEGA_LIBRARY_ROOT=$library}}
    candidate=@{program=(Resolve-Path -LiteralPath $Candidate).Path;arguments=$arguments;environment=@{OMEGA_LIBRARY_ROOT=$library}}
    warmups=1;pairs=$Pairs;timeout_ms=120000
    measurement=@{kind='process';stdin='';expected=@{
        exit_code=0;stdout=$sourcePath+"`n  part 0: accepted`n"+$sourcePath+"`troute=accepted`n";stderr=''
    }}
}
[IO.File]::WriteAllText([IO.Path]::GetFullPath($Output),($plan | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
