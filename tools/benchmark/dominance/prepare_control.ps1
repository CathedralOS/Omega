param([Parameter(Mandatory=$true)][string]$Output, [switch]$IdentityBuild)
# Keep the existing workload's algorithm, constants and checksum. Explicitly
# qualify literals under the current domain rules; do not change its oracle.
$ErrorActionPreference='Stop'
$base='9b4898865ecd5789bea4def6c2da078192a4ab82'
if (Test-Path -LiteralPath $Output) { throw 'Output directory must be new' }
New-Item -ItemType Directory -Force -Path $Output | Out-Null
foreach ($name in @('main.omg','build.omg')) {
    $lines=git show ($base+':samples/cli/arithmetic/wrapping_square_sum/'+$name)
    if ($LASTEXITCODE -ne 0) { throw 'Pinned source is unavailable' }
    $text=($lines -join "`n")+"`n"
    if ($name -eq 'main.omg') {
        $changes=[ordered]@{
            'loop_(0, 1)'='loop_(0u64 as +Wrapping, 1u64 as +Wrapping)'
            'i <= 2000000'='i <= (2000000u64 as +Wrapping)'
            'i + 1'='i + (1u64 as +Wrapping)'
            'acc == 2666668666667000000'='acc == (2666668666667000000u64 as +Wrapping)'
        }
        foreach ($old in $changes.Keys) {
            if (-not $text.Contains($old)) { throw 'Pinned workload changed' }
            $text=$text.Replace($old,$changes[$old])
        }
    }
    if ($name -eq 'build.omg' -and $IdentityBuild) {
        $selection='(?s)\s*Optimizations::enable\([^;]+;'
        if ([regex]::Matches($text,$selection).Count -ne 6) { throw 'Pinned rule selection changed' }
        $text=[regex]::Replace($text,$selection,'')
        $text=[regex]::Replace($text,'(?m)^    // The staged Psi-phase promotion[^\n]*\n    // rows as measurement[^\n]*\n    // here lets each rule[^\n]*\n','    // This gate selects the identity optimization phase.'+"`n")
    }
    [IO.File]::WriteAllText((Join-Path (Resolve-Path -LiteralPath $Output).Path $name),$text,[Text.UTF8Encoding]::new($false))
}
Get-ChildItem -LiteralPath $Output -Filter '*.omg' | Get-FileHash -Algorithm SHA256
