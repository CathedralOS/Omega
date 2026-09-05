#requires -Version 7.2
# Exercise the landing protocol with independent writers and an isolated bare remote.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$landing = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../landing.ps1'))
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('omega-landing-test-' + [guid]::NewGuid().ToString('N'))
$remote = Join-Path $testRoot 'remote.git'
$writerA = Join-Path $testRoot 'writer-a'
$writerB = Join-Path $testRoot 'writer-b'
$emptyHooks = Join-Path $testRoot 'empty-hooks'
$claimReference = 'refs/coordination/omega-landing/main'
$gitExecutable = @(Get-Command git -CommandType Application)[0].Source
$script:passed = 0
New-Item -ItemType Directory -Path $testRoot, $emptyHooks | Out-Null

function Git([string]$Directory, [string[]]$Arguments) {
    $output = & $gitExecutable -C $Directory @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git $Arguments failed: $output" }
    return ($output -join "`n").Trim()
}

function Start-Landing([string]$Directory, [string[]]$Arguments) {
    $start = [System.Diagnostics.ProcessStartInfo]::new((Join-Path $PSHOME 'pwsh.exe'))
    if (-not $IsWindows) { $start.FileName = Join-Path $PSHOME 'pwsh' }
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $start.StandardErrorEncoding = [System.Text.UTF8Encoding]::new($false)
    foreach ($argument in (@('-NoProfile', '-File', $landing, '-Repository', $Directory) + $Arguments)) {
        $start.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw 'Could not start landing command.' }
    return [pscustomobject]@{
        Process = $process
        Output = $process.StandardOutput.ReadToEndAsync()
        Error = $process.StandardError.ReadToEndAsync()
    }
}

function Finish-Landing($Running) {
    try {
        if (-not $Running.Process.WaitForExit(30000)) {
            $Running.Process.Kill($true)
            throw 'Local landing test timed out.'
        }
        return [pscustomobject]@{
            Code = $Running.Process.ExitCode
            Output = $Running.Output.GetAwaiter().GetResult().Trim()
            Error = $Running.Error.GetAwaiter().GetResult().Trim()
        }
    } finally { $Running.Process.Dispose() }
}

function Run-Landing([string]$Directory, [string[]]$Arguments, [int]$Expected = 0) {
    $result = Finish-Landing (Start-Landing $Directory $Arguments)
    if ($result.Code -ne $Expected) { throw "Expected exit $Expected, got $($result.Code): $($result.Error) $($result.Output)" }
    return $result
}

function Check([bool]$Condition, [string]$Description) {
    if (-not $Condition) { throw $Description }
    $script:passed++
    Write-Output "PASS $Description"
}

function Assert-Remote([string]$Main, [string]$Claim) {
    if ((Git $remote @('rev-parse', 'refs/heads/main')) -ne $Main) { throw 'Remote main changed unexpectedly.' }
    $references = Git $writerA @('ls-remote', '--refs', $remote, $claimReference)
    if (($Claim -and $references -ne "$Claim`t$claimReference") -or (-not $Claim -and $references)) {
        throw "Unexpected reservation: $references"
    }
}

function Install-Hook([string]$Directory, [string]$Name, [string]$Body) {
    New-Item -ItemType Directory -Path $Directory -Force | Out-Null
    $path = Join-Path $Directory $Name
    [System.IO.File]::WriteAllText($path, "#!/bin/sh`n$Body`n", [System.Text.UTF8Encoding]::new($false))
    if (-not $IsWindows) { & chmod +x $path; if ($LASTEXITCODE -ne 0) { throw 'chmod failed' } }
}

try {
    $null = Git $testRoot @('init', '--bare', $remote)
    $null = Git $testRoot @('init', '-b', 'main', $writerA)
    foreach ($setting in @(@('user.name', 'Landing Test'), @('user.email', 'landing@example.invalid'), @('core.hooksPath', $emptyHooks), @('commit.gpgSign', 'false'))) {
        $null = Git $writerA (@('config') + $setting)
    }
    $null = Git $writerA @('commit', '--allow-empty', '-m', 'initial')
    $null = Git $writerA @('remote', 'add', 'origin', $remote)
    $null = Git $writerA @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Git $testRoot @('clone', '--branch', 'main', $remote, $writerB)
    foreach ($setting in @(@('user.name', 'Landing Test'), @('user.email', 'landing@example.invalid'), @('core.hooksPath', $emptyHooks), @('commit.gpgSign', 'false'))) {
        $null = Git $writerB (@('config') + $setting)
    }
    $initial = Git $writerA @('rev-parse', 'HEAD')
    $status = (Run-Landing $writerA @('status')).Output | ConvertFrom-Json
    Check ($status.state -eq 'available') 'an unreserved repository is available'
    $null = Run-Landing $writerA @('claim') 1
    Assert-Remote $initial ''
    Check $true 'a claim requires an identifiable owner'

    [System.IO.File]::WriteAllText((Join-Path $writerA 'unfinished.txt'), 'unfinished')
    $null = Run-Landing $writerA @('claim', '-Owner', 'A') 1
    Remove-Item -LiteralPath (Join-Path $writerA 'unfinished.txt')
    Assert-Remote $initial ''
    Check $true 'uncheckpointed work cannot occupy the landing reservation'

    $reservationA = (Run-Landing $writerA @('claim', '-Owner', 'A / café')).Output | ConvertFrom-Json
    Check ($reservationA.base -eq $initial) 'claim returns the fetched integration base'
    $busy = (Run-Landing $writerB @('claim', '-Owner', 'B') 2).Output | ConvertFrom-Json
    if ($busy.claim -ne $reservationA.claim -or $busy.owner -ne 'A / café') {
        throw "Owner round trip failed: expected claim $($reservationA.claim), got $($busy.claim); owner '$($busy.owner)' ($(([char[]]$busy.owner | ForEach-Object { [int]$_ }) -join ','))"
    }
    Check ($busy.claim -eq $reservationA.claim -and $busy.owner -eq 'A / café') 'the other writer sees the UTF-8 owner and exits busy'
    $null = Run-Landing $writerB @('release', '-Claim', $initial) 1
    Assert-Remote $initial $reservationA.claim
    Check $true 'a different claim cannot release the reservation'
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $initial, '-Candidate', $initial) 1
    Check $true 'publishing an unchanged candidate requires release instead'

    $null = Git $writerA @('commit', '--allow-empty', '-m', 'candidate A')
    $candidateA = Git $writerA @('rev-parse', 'HEAD')
    [System.IO.File]::WriteAllText((Join-Path $writerA 'unfinished.txt'), 'unfinished')
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $initial, '-Candidate', $candidateA) 1
    Remove-Item -LiteralPath (Join-Path $writerA 'unfinished.txt')
    Check $true 'dirty candidates cannot publish'
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $initial, '-Candidate', $initial) 1
    Check $true 'the candidate must be the exact current HEAD'
    $null = Run-Landing $writerA @('recover', '-Claim', $reservationA.claim) 1
    Check $true 'recovery requires an explicit reason'
    $null = Run-Landing $writerB @('recover', '-Claim', $reservationA.claim, '-Reason', 'A abandoned integration in this test')
    $reservationB = (Run-Landing $writerB @('claim', '-Owner', 'B')).Output | ConvertFrom-Json
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $initial, '-Candidate', $candidateA) 1
    $null = Run-Landing $writerA @('release', '-Claim', $reservationA.claim) 1
    Assert-Remote $initial $reservationB.claim
    Check $true 'a recovered owner cannot publish or remove a replacement reservation'

    $null = Git $writerB @('commit', '--allow-empty', '-m', 'candidate B')
    $candidateB = Git $writerB @('rev-parse', 'HEAD')
    # Reject main on the server to prove release is not a separate successful push.
    $remoteHooks = Join-Path $remote 'hooks'
    Install-Hook $remoteHooks 'pre-receive' 'while read old new ref; do if [ "$ref" = refs/heads/main ]; then exit 1; fi; done'
    $null = Run-Landing $writerB @('publish', '-Claim', $reservationB.claim, '-Base', $initial, '-Candidate', $candidateB) 1
    Assert-Remote $initial $reservationB.claim
    Remove-Item -LiteralPath (Join-Path $remoteHooks 'pre-receive')
    Check $true 'server rejection leaves both main and the reservation intact'
    $published = (Run-Landing $writerB @('publish', '-Claim', $reservationB.claim, '-Base', $initial, '-Candidate', $candidateB)).Output | ConvertFrom-Json
    Assert-Remote $candidateB ''
    Check ($published.state -eq 'published') 'the owner publishes and releases together'

    # Writer A has divergent local history; the wrapper must never force main.
    $reservationA = (Run-Landing $writerA @('claim', '-Owner', 'A')).Output | ConvertFrom-Json
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateB, '-Candidate', $candidateA) 1
    Assert-Remote $candidateB $reservationA.claim
    Check $true 'a divergent candidate cannot overwrite main'
    $null = Git $writerA @('rebase', $candidateB)
    $candidateA = Git $writerA @('rev-parse', 'HEAD')
    $side = Git $writerA @('commit-tree', 'HEAD^{tree}', '-p', $candidateB, '-m', 'side')
    $merge = Git $writerA @('commit-tree', 'HEAD^{tree}', '-p', $candidateA, '-p', $side, '-m', 'merge')
    $null = Git $writerA @('switch', '--detach', $merge)
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateB, '-Candidate', $merge) 1
    $null = Git $writerA @('switch', '--detach', $candidateA)
    Check $true 'merge commits cannot enter linear main'

    # Replace the claim after the script's preflight but before Git sends updates.
    $localHooks = Join-Path $testRoot 'race-hooks'
    $remoteForShell = $remote.Replace('\', '/').Replace("'", "'\''")
    Install-Hook $localHooks 'pre-push' "git --git-dir='$remoteForShell' update-ref $claimReference $($reservationB.claim) $($reservationA.claim)"
    $null = Git $writerA @('config', 'core.hooksPath', $localHooks)
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateB, '-Candidate', $candidateA) 1
    $null = Git $writerA @('config', 'core.hooksPath', $emptyHooks)
    Assert-Remote $candidateB $reservationB.claim
    Check $true 'replacement during publication fences the stale owner atomically'
    $null = Run-Landing $writerB @('release', '-Claim', $reservationB.claim)

    # Keep both publishers active; exactly one may acquire, on every attempt.
    foreach ($round in 1..3) {
        $runningA = Start-Landing $writerA @('claim', '-Owner', "A round $round")
        $runningB = Start-Landing $writerB @('claim', '-Owner', "B round $round")
        $resultA = Finish-Landing $runningA
        $resultB = Finish-Landing $runningB
        Check ((@($resultA.Code, $resultB.Code) | Sort-Object) -join ',' -eq '0,2') "simultaneous acquisition round $round has one winner"
        $winner = if ($resultA.Code -eq 0) { $resultA } else { $resultB }
        $winnerRepository = if ($resultA.Code -eq 0) { $writerA } else { $writerB }
        $claim = ($winner.Output | ConvertFrom-Json).claim
        $null = Run-Landing $winnerRepository @('release', '-Claim', $claim)
    }

    $reservationA = (Run-Landing $writerA @('claim', '-Owner', 'A')).Output | ConvertFrom-Json
    $null = Git $writerB @('commit', '--allow-empty', '-m', 'uncoordinated writer')
    $outsider = Git $writerB @('rev-parse', 'HEAD')
    $null = Git $writerB @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateB, '-Candidate', $candidateA) 1
    Assert-Remote $outsider $reservationA.claim
    $null = Run-Landing $writerA @('release', '-Claim', $reservationA.claim)
    Check $true 'a writer bypassing the protocol is detected without overwriting its work'

    # Some remotes fetch from one repository but push to another. Claims and
    # main observations must follow the one actual publication destination.
    $alternate = Join-Path $testRoot 'alternate.git'
    $null = Git $testRoot @('clone', '--bare', $remote, $alternate)
    $null = Git $writerB @('commit', '--allow-empty', '-m', 'fetch destination moves')
    $fetchOnly = Git $writerB @('rev-parse', 'HEAD')
    $null = Git $writerB @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Git $writerA @('config', 'remote.origin.pushurl', $alternate)
    $alternateClaim = (Run-Landing $writerA @('claim', '-Owner', 'A alternate')).Output | ConvertFrom-Json
    Check ($alternateClaim.base -eq $outsider) 'the reserved base comes from the actual push destination'
    Assert-Remote $fetchOnly ''
    $alternateState = (Run-Landing $writerA @('status')).Output | ConvertFrom-Json
    Check ($alternateState.claim -eq $alternateClaim.claim) 'status and publication use the same push destination'
    $null = Run-Landing $writerA @('release', '-Claim', $alternateClaim.claim)
    $null = Git $writerA @('config', '--add', 'remote.origin.pushurl', $remote)
    $null = Run-Landing $writerA @('claim', '-Owner', 'A multiple destinations') 1
    Assert-Remote $fetchOnly ''
    Check $true 'multiple push destinations are refused without claiming either'

    Write-Output "$script:passed landing checks passed. Fixtures: $testRoot"
} catch {
    [Console]::Error.WriteLine("FAILED: $($_.Exception.Message)`nFixtures preserved: $testRoot")
    exit 1
}
