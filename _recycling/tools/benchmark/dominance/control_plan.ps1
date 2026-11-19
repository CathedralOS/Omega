param(
    [Parameter(Mandatory=$true)][string]$Baseline,
    [Parameter(Mandatory=$true)][string]$Candidate,
    [Parameter(Mandatory=$true)][string]$Project,
    [Parameter(Mandatory=$true)][string]$LibraryRoot,
    [Parameter(Mandatory=$true)][string]$Output
)
# Process mode measures source preparation and Product checking. This diagnostic
# example executes neither Build nor provider settlement/native publication.
$ErrorActionPreference='Stop'
$projectPath=(Resolve-Path -LiteralPath $Project).Path
$source=Join-Path $projectPath 'main.omg'
$library=(Resolve-Path -LiteralPath $LibraryRoot).Path
$arguments=@($source,'--entry','main','--target','windows_x86_64')
$environment=@{OMEGA_LIBRARY_ROOT=$library}
$expected=$source+"`n  part 0: accepted`n"+$source+"`troute=accepted`n"
$plan=[ordered]@{
    schema='omega-performance-plan/1'
    name='Canonical square-sum Product checking control'
    working_directory=$PWD.Path
    inputs=@($projectPath,$library)
    baseline=@{program=(Resolve-Path -LiteralPath $Baseline).Path;arguments=$arguments;environment=$environment}
    candidate=@{program=(Resolve-Path -LiteralPath $Candidate).Path;arguments=$arguments;environment=$environment}
    warmups=1
    pairs=8
    timeout_ms=120000
    measurement=@{kind='process';stdin='';expected=@{exit_code=0;stdout=$expected;stderr=''}}
}
[IO.File]::WriteAllText([IO.Path]::GetFullPath($Output),($plan | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
