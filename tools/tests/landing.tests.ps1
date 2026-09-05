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
    $active = ''
    if ($references) {
        $record = (Git $remote @('show', '-s', '--format=%B', $claimReference)) | ConvertFrom-Json
        if ($record.active) { $active = $record.active.ticket }
    }
    if ($active -ne $Claim) { throw "Unexpected active ticket: $active" }
}

function Enqueue([string]$Directory, [string]$Owner) {
    return ((Run-Landing $Directory @('enqueue', '-Owner', $Owner)).Output | ConvertFrom-Json).ticket
}

function Reserve([string]$Directory, [string]$Owner) {
    $ticket = Enqueue $Directory $Owner
    return (Run-Landing $Directory @('claim', '-Ticket', $ticket)).Output | ConvertFrom-Json
}

function Queue-State {
    return (Run-Landing $writerA @('status')).Output | ConvertFrom-Json
}

function Replace-OnPush($Record, [string]$Expected) {
    $messageFile = Join-Path $testRoot 'replacement.json'
    [IO.File]::WriteAllText($messageFile, ($Record | ConvertTo-Json -Depth 8 -Compress), [Text.UTF8Encoding]::new($false))
    $replacement = Git $remote @('commit-tree', "$Expected^{tree}", '-p', $Expected, '-F', $messageFile)
    $remoteForShell = $remote.Replace('\', '/').Replace("'", "'\''")
    $markerForShell = (Join-Path $testRoot ([guid]::NewGuid().ToString('N'))).Replace('\', '/').Replace("'", "'\''")
    # One mutation after preflight, before the server sees the attempted push.
    Install-Hook $localHooks 'pre-push' "if [ ! -f '$markerForShell' ]; then touch '$markerForShell'; git --git-dir='$remoteForShell' update-ref $claimReference $replacement $Expected || exit 1; fi"
    $null = Git $writerA @('config', 'core.hooksPath', $localHooks)
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
        if ($setting[0] -ne 'core.hooksPath') { $null = Git $remote (@('config') + $setting) }
    }
    $null = Git $writerA @('commit', '--allow-empty', '-m', 'initial')
    $null = Git $writerA @('remote', 'add', 'origin', $remote)
    $null = Git $writerA @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Git $testRoot @('clone', '--branch', 'main', $remote, $writerB)
    foreach ($setting in @(@('user.name', 'Landing Test'), @('user.email', 'landing@example.invalid'), @('core.hooksPath', $emptyHooks), @('commit.gpgSign', 'false'))) {
        $null = Git $writerB (@('config') + $setting)
    }
    $localHooks = Join-Path $testRoot 'race-hooks'
    $initial = Git $writerA @('rev-parse', 'HEAD')
    Check ((Queue-State).state -eq 'available') 'an uninitialized queue is available'
    $null = Run-Landing $writerA @('enqueue') 1
    $null = Run-Landing $writerA @('claim', '-Owner', 'old client') 1
    Check $true 'enqueue requires an owner and claim requires a ticket'

    [IO.File]::WriteAllText((Join-Path $writerA 'unfinished.txt'), 'unfinished')
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'A') 1
    Remove-Item -LiteralPath (Join-Path $writerA 'unfinished.txt')
    Check ((Queue-State).queue.Count -eq 0) 'unfinished work cannot join the ready queue'

    $ticketA = Enqueue $writerA 'A / café'
    $ticketB = Enqueue $writerB 'B'
    $ticketC = Enqueue $writerB 'C'
    $state = Queue-State
    Check (($state.queue.ticket -join ',') -eq "$ticketA,$ticketB,$ticketC" -and $state.queue[0].owner -eq 'A / café') 'FIFO order and UTF-8 owners survive across writers'
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'A / café', '-Ticket', $ticketA)
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'impostor', '-Ticket', $ticketA) 1
    Check ((Queue-State).queue.Count -eq 3) 'retrying a live ticket never duplicates or changes its owner'
    $null = Run-Landing $writerB @('claim', '-Ticket', $ticketB) 2
    $null = Run-Landing $writerB @('claim', '-Ticket', $ticketC, '-WaitSeconds', '1', '-PollSeconds', '1') 2
    Check ((Queue-State).queue.Count -eq 3) 'later arrivals cannot jump ahead and wait timeout preserves position'

    $reservationA = (Run-Landing $writerA @('claim', '-Ticket', $ticketA)).Output | ConvertFrom-Json
    $repeated = (Run-Landing $writerA @('claim', '-Ticket', $ticketA)).Output | ConvertFrom-Json
    Check ($reservationA.base -eq $initial -and $repeated.base -eq $initial) 'head claims the current base and a retry returns the same reservation'
    $null = Run-Landing $writerB @('release', '-Claim', $ticketB) 1
    $null = Run-Landing $writerA @('cancel', '-Ticket', $ticketA) 1
    Check $true 'waiting tickets cannot release the owner and active tickets require release'
    $null = Run-Landing $writerB @('cancel', '-Ticket', $ticketC)
    Check (((Queue-State).queue.ticket -join ',') -eq "$ticketA,$ticketB") 'cancellation preserves the remaining order'

    $null = Git $writerA @('commit', '--allow-empty', '-m', 'candidate A')
    $candidateA = Git $writerA @('rev-parse', 'HEAD')
    $null = Run-Landing $writerA @('publish', '-Claim', $ticketA, '-Base', $initial, '-Candidate', $initial) 1
    $null = Run-Landing $writerA @('publish', '-Claim', $ticketA, '-Base', $candidateA, '-Candidate', $candidateA) 1
    [IO.File]::WriteAllText((Join-Path $writerA 'unfinished.txt'), 'unfinished')
    $null = Run-Landing $writerA @('publish', '-Claim', $ticketA, '-Base', $initial, '-Candidate', $candidateA) 1
    Remove-Item -LiteralPath (Join-Path $writerA 'unfinished.txt')
    Assert-Remote $initial $ticketA
    Check $true 'publication requires the exact clean candidate and reserved base'

    $remoteHooks = Join-Path $remote 'hooks'
    Install-Hook $remoteHooks 'pre-receive' 'while read old new ref; do if [ "$ref" = refs/heads/main ]; then exit 1; fi; done'
    $null = Run-Landing $writerA @('publish', '-Claim', $ticketA, '-Base', $initial, '-Candidate', $candidateA) 1
    Assert-Remote $initial $ticketA
    Check (((Queue-State).queue.ticket -join ',') -eq "$ticketA,$ticketB") 'server rejection preserves main, its owner, and every waiting ticket'
    Remove-Item -LiteralPath (Join-Path $remoteHooks 'pre-receive')

    # The second worktree waits locally and acquires automatically after A lands.
    $waitingB = Start-Landing $writerB @('claim', '-Ticket', $ticketB, '-WaitSeconds', '20', '-PollSeconds', '1')
    $null = Run-Landing $writerA @('publish', '-Claim', $ticketA, '-Base', $initial, '-Candidate', $candidateA)
    $waitResult = Finish-Landing $waitingB
    if ($waitResult.Code -ne 0) { throw "Wait failed: $($waitResult.Error) $($waitResult.Output)" }
    $reservationB = $waitResult.Output | ConvertFrom-Json
    Assert-Remote $candidateA $ticketB
    Check ($reservationB.base -eq $candidateA -and (Queue-State).queue.Count -eq 1) 'publication pops only the head and the local waiter reserves the newly published base'
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'A / café', '-Ticket', $ticketA) 1
    Check $true 'retired tickets cannot be resurrected by a resumed old owner'

    $null = Run-Landing $writerB @('recover', '-Claim', $ticketB) 1
    $null = Run-Landing $writerA @('recover', '-Claim', $ticketB, '-Reason', 'B confirmed it stopped')
    $replacement = Reserve $writerA 'replacement'
    $null = Run-Landing $writerB @('release', '-Claim', $ticketB) 1
    $null = Run-Landing $writerB @('publish', '-Claim', $ticketB, '-Base', $candidateA, '-Candidate', $candidateA) 1
    Assert-Remote $candidateA $replacement.claim
    Check $true 'recovery requires a reason and fences the former owner'
    $null = Run-Landing $writerA @('release', '-Claim', $replacement.claim)
    Check ((Queue-State).state -eq 'available' -and (Queue-State).version) 'an empty persistent queue prevents old clients from claiming an absent reference'

    # Both enqueue mutations must survive; their serialized order is authoritative.
    foreach ($round in 1..3) {
        $runningA = Start-Landing $writerA @('enqueue', '-Owner', "A round $round")
        $runningB = Start-Landing $writerB @('enqueue', '-Owner', "B round $round")
        $resultA = Finish-Landing $runningA
        $resultB = Finish-Landing $runningB
        if ($resultA.Code -ne 0 -or $resultB.Code -ne 0) { throw "Concurrent enqueue failed: $($resultA.Error) $($resultB.Error)" }
        $state = Queue-State
        Check ($state.queue.Count -eq 2 -and ($state.queue.ticket | Sort-Object -Unique).Count -eq 2) "simultaneous enqueue round $round retains both arrivals"
        $null = Run-Landing $writerA @('claim', '-Ticket', $state.queue[1].ticket) 2
        $head = (Run-Landing $writerA @('claim', '-Ticket', $state.queue[0].ticket)).Output | ConvertFrom-Json
        $null = Run-Landing $writerA @('release', '-Claim', $head.claim)
        $next = (Run-Landing $writerB @('claim', '-Ticket', $state.queue[1].ticket)).Output | ConvertFrom-Json
        $null = Run-Landing $writerB @('release', '-Claim', $next.claim)
    }

    $reservationA = Reserve $writerA 'A race'
    $null = Git $writerA @('commit', '--allow-empty', '-m', 'candidate after A')
    $candidateNext = Git $writerA @('rev-parse', 'HEAD')
    $version = (Queue-State).version
    $record = (Git $remote @('show', '-s', '--format=%B', $version)) | ConvertFrom-Json -AsHashtable
    $arriving = [guid]::NewGuid().ToString('N')
    $record.entries += @{ ticket = $arriving; owner = 'arrived during push'; created_utc = '2026-09-05T00:00:00Z' }
    Replace-OnPush $record $version
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateA, '-Candidate', $candidateNext)
    $null = Git $writerA @('config', 'core.hooksPath', $emptyHooks)
    Assert-Remote $candidateNext ''
    Check (((Queue-State).queue.ticket -join ',') -eq $arriving) 'enqueue during publication retries metadata without dropping the new arrival'
    $next = (Run-Landing $writerB @('claim', '-Ticket', $arriving)).Output | ConvertFrom-Json
    $null = Run-Landing $writerB @('release', '-Claim', $next.claim)

    $reservationA = Reserve $writerA 'A recovered during push'
    $null = Git $writerA @('commit', '--allow-empty', '-m', 'candidate after race')
    $candidateFinal = Git $writerA @('rev-parse', 'HEAD')
    $version = (Queue-State).version
    $record = (Git $remote @('show', '-s', '--format=%B', $version)) | ConvertFrom-Json -AsHashtable
    $record.entries = @()
    $record.active = $null
    Replace-OnPush $record $version
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateNext, '-Candidate', $candidateFinal) 1
    $null = Git $writerA @('config', 'core.hooksPath', $emptyHooks)
    Assert-Remote $candidateNext ''
    Check $true 'recovery between preflight and push prevents publication atomically'

    $reservationA = Reserve $writerA 'A linear history'
    $null = Git $writerB @('commit', '--allow-empty', '-m', 'divergent B')
    $divergent = Git $writerB @('rev-parse', 'HEAD')
    $null = Run-Landing $writerB @('publish', '-Claim', $reservationA.claim, '-Base', $candidateNext, '-Candidate', $divergent) 1
    $side = Git $writerA @('commit-tree', 'HEAD^{tree}', '-p', $candidateNext, '-m', 'side')
    $merge = Git $writerA @('commit-tree', 'HEAD^{tree}', '-p', $candidateFinal, '-p', $side, '-m', 'merge')
    $null = Git $writerA @('switch', '--detach', $merge)
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateNext, '-Candidate', $merge) 1
    $null = Git $writerA @('switch', '--detach', $candidateFinal)
    Assert-Remote $candidateNext $reservationA.claim
    Check $true 'divergent candidates and merge commits cannot overwrite linear main'

    $null = Git $writerB @('fetch', 'origin', 'main')
    $null = Git $writerB @('rebase', 'origin/main')
    $outsider = Git $writerB @('rev-parse', 'HEAD')
    $null = Git $writerB @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Run-Landing $writerA @('publish', '-Claim', $reservationA.claim, '-Base', $candidateNext, '-Candidate', $candidateFinal) 1
    Assert-Remote $outsider $reservationA.claim
    $null = Run-Landing $writerA @('release', '-Claim', $reservationA.claim)
    Check $true 'an uncoordinated main push is detected without overwriting it'

    $ticketA = Enqueue $writerA 'head that stopped'
    $ticketB = Enqueue $writerB 'next ready'
    $null = Run-Landing $writerA @('cancel', '-Ticket', $ticketA, '-Reason', 'head confirmed stopped')
    $next = (Run-Landing $writerB @('claim', '-Ticket', $ticketB)).Output | ConvertFrom-Json
    $null = Run-Landing $writerB @('release', '-Claim', $next.claim)
    Check $true 'cancelling an abandoned waiting head lets the next ticket advance'

    # Fetch and push may target different repositories. Coordinate at the latter.
    $alternate = Join-Path $testRoot 'alternate.git'
    $null = Git $testRoot @('clone', '--bare', $remote, $alternate)
    $null = Git $writerB @('commit', '--allow-empty', '-m', 'fetch destination moves')
    $fetchOnly = Git $writerB @('rev-parse', 'HEAD')
    $null = Git $writerB @('push', 'origin', 'HEAD:refs/heads/main')
    $null = Git $writerA @('config', 'remote.origin.pushurl', $alternate)
    $alternateClaim = Reserve $writerA 'A alternate'
    Check ($alternateClaim.base -eq $outsider -and (Queue-State).active.ticket -eq $alternateClaim.claim) 'queue, claim, and status use the actual push destination'
    Assert-Remote $fetchOnly ''
    $null = Run-Landing $writerA @('release', '-Claim', $alternateClaim.claim)
    $null = Git $writerA @('config', '--add', 'remote.origin.pushurl', $remote)
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'multiple destinations') 1
    Assert-Remote $fetchOnly ''
    Check $true 'multiple push destinations are refused without changing either queue'
    $null = Git $writerA @('config', '--unset-all', 'remote.origin.pushurl')

    # An in-flight v1 reservation must survive upgrade until explicitly released.
    $messageFile = Join-Path $testRoot 'legacy.json'
    [IO.File]::WriteAllText($messageFile, '{"protocol":"omega-landing-v1","owner":"legacy owner"}', [Text.UTF8Encoding]::new($false))
    $legacy = Git $remote @('commit-tree', 'refs/heads/main^{tree}', '-F', $messageFile)
    $previous = (Queue-State).version
    $null = Git $remote @('update-ref', $claimReference, $legacy, $previous)
    Check ((Queue-State).state -eq 'legacy_reserved') 'status identifies an in-flight legacy reservation'
    $null = Run-Landing $writerA @('enqueue', '-Owner', 'upgrade') 1
    Check ((Git $remote @('rev-parse', $claimReference)) -eq $legacy) 'FIFO upgrade refuses to replace a legacy owner'
    $null = Run-Landing $writerA @('release', '-Claim', $legacy)
    $upgraded = Reserve $writerA 'upgraded'
    $null = Run-Landing $writerA @('release', '-Claim', $upgraded.claim)
    Check ((Queue-State).state -eq 'available') 'FIFO initializes after an explicit legacy release'

    Write-Output "$script:passed landing checks passed. Fixtures: $testRoot"
} catch {
    [Console]::Error.WriteLine("FAILED: $($_.Exception.Message)`nFixtures preserved: $testRoot")
    exit 1
}
