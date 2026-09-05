#requires -Version 7.2
<#
Ready worktrees join one FIFO queue across machines. Only its first ticket can
reserve final integration. GitHub stores an ordered list and active reservation
in one non-branch reference; builds and waiting run locally, without model calls.
Order is the order of successful queue updates, never workstation timestamps.

Every mutation compares the exact previous queue object. Publication advances
main and removes the active ticket in one atomic push. Concurrent enqueues can
retry the metadata update without repeating tests. Main is never force-pushed.
Tickets survive unrelated queue edits, but a removed ticket cannot publish.

The empty queue reference persists so old v1 clients cannot jump ahead. Its
empty-tree commits retain coordination history outside main. No ticket expires
automatically: cancel waiting work, or release/recover an active reservation.
This is cooperative coordination; direct pushes can still bypass the command.

See tools/landing.md for enqueue -> claim -> integrate -> check -> publish.
claim -WaitSeconds runs a bounded local wait; exit 2 retains the queued ticket.
Exit 1 means error or uncertain network outcome: inspect status before retrying.
The command checks Git state, not whether applicable tests actually passed.
Local independent-writer coverage lives in tools/tests/landing.tests.ps1.
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('status', 'enqueue', 'cancel', 'claim', 'publish', 'release', 'recover')]
    [string]$Command = 'status',
    [string]$Repository = '.',
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]*$')]
    [string]$Remote = 'origin',
    [string]$Owner,
    [string]$Ticket,
    [string]$Claim,
    [string]$Base,
    [string]$Candidate,
    [string]$Reason,
    [ValidateRange(0, 43200)]
    [int]$WaitSeconds = 0,
    [ValidateRange(1, 300)]
    [int]$PollSeconds = 10
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$claimReference = 'refs/coordination/omega-landing/main'
$mainReference = 'refs/heads/main'
$repositoryPath = [System.IO.Path]::GetFullPath($Repository)

function Invoke-Git {
    param([string[]]$Arguments, [string]$InputText, [switch]$AllowFailure)
    $start = [System.Diagnostics.ProcessStartInfo]::new('git')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardInputEncoding = [System.Text.UTF8Encoding]::new($false)
    $start.StandardOutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $start.StandardErrorEncoding = [System.Text.UTF8Encoding]::new($false)
    $start.Environment['GIT_TERMINAL_PROMPT'] = '0'
    foreach ($argument in (@('-C', $repositoryPath) + $Arguments)) {
        $start.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        if (-not $process.Start()) { throw 'Could not start git.' }
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.Write($InputText)
        $process.StandardInput.Close()
        $process.WaitForExit()
        $result = [pscustomobject]@{
            Code = $process.ExitCode
            Output = $stdout.GetAwaiter().GetResult().TrimEnd()
            Error = $stderr.GetAwaiter().GetResult().TrimEnd()
        }
    } finally { $process.Dispose() }
    if ($result.Code -ne 0 -and -not $AllowFailure) {
        throw "Git failed ($($result.Code)): $($result.Error) $($result.Output)"
    }
    return $result
}

function Require-ObjectId([string]$Value, [string]$Name) {
    if ($Value -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') {
        throw "$Name requires a full lowercase Git object ID."
    }
}

function Read-RemoteReferences {
    $response = Invoke-Git -Arguments @('ls-remote', '--refs', $pushUrl, $mainReference, $claimReference)
    $references = @{}
    foreach ($line in ($response.Output -split "`n")) {
        if (-not $line) { continue }
        $parts = $line.TrimEnd() -split "`t"
        if ($parts.Count -ne 2) { throw 'Unexpected remote reference response.' }
        Require-ObjectId $parts[0] 'Remote reference'
        $references[$parts[1]] = $parts[0]
    }
    if (-not $references.ContainsKey($mainReference)) { throw 'Remote main does not exist.' }
    return $references
}

function Read-Snapshot {
    $references = Read-RemoteReferences
    $version = if ($references.ContainsKey($claimReference)) { $references[$claimReference] } else { '' }
    $record = @{ protocol = 'omega-landing-v2'; entries = @(); active = $null }
    if ($version) {
        $null = Invoke-Git -Arguments @('fetch', '--no-tags', '--no-write-fetch-head', $pushUrl, $version)
        $record = (Invoke-Git -Arguments @('show', '-s', '--format=%B', $version)).Output | ConvertFrom-Json -AsHashtable
        if ($record.protocol -notin @('omega-landing-v1', 'omega-landing-v2')) {
            throw 'Unknown coordination format; do not replace it.'
        }
        if ($record.protocol -eq 'omega-landing-v2') {
            if (-not $record.ContainsKey('active') -or $record.entries -isnot [array]) { throw 'Invalid queue record.' }
            $seen = @{}
            foreach ($entry in $record.entries) {
                Require-Ticket $entry.ticket
                if ($seen.ContainsKey($entry.ticket) -or [string]::IsNullOrWhiteSpace($entry.owner)) { throw 'Invalid queue entry.' }
                $seen[$entry.ticket] = $true
            }
            if ($record.active) {
                if ($record.entries.Count -eq 0 -or $record.entries[0].ticket -ne $record.active.ticket) { throw 'Active ticket must be first.' }
                Require-ObjectId $record.active.base 'Reserved base'
            }
        }
    }
    return @{ version = $version; main = $references[$mainReference]; record = $record }
}

function Require-Ticket([string]$Value) {
    if ($Value -cnotmatch '^[0-9a-f]{32}$') { throw 'Supply the 32-character ticket returned by enqueue; enqueue before claiming.' }
}

function Write-Json($Value) {
    $Value | ConvertTo-Json -Depth 8 -Compress -EscapeHandling EscapeNonAscii
}

function Write-Status($Snapshot) {
    $record = $Snapshot.record
    if ($record.protocol -eq 'omega-landing-v1') {
        Write-Json @{ state = 'legacy_reserved'; claim = $Snapshot.version; owner = $record.owner; main = $Snapshot.main }
        return
    }
    $position = 0
    $queue = @($record.entries | ForEach-Object {
        $position++
        @{ ticket = $_.ticket; owner = $_.owner; position = $position; created_utc = $_.created_utc }
    })
    $state = if ($record.active) { 'reserved' } elseif ($queue.Count) { 'queued' } else { 'available' }
    Write-Json @{ state = $state; main = $Snapshot.main; active = $record.active; queue = $queue; version = $Snapshot.version }
}

function Require-CleanHead([string]$Expected) {
    $head = (Invoke-Git -Arguments @('rev-parse', 'HEAD')).Output
    if ($Expected -and $head -ne $Expected) { throw 'HEAD differs from the verified candidate.' }
    $dirty = (Invoke-Git -Arguments @('status', '--porcelain', '--untracked-files=normal')).Output
    if ($dirty) { throw 'Checkpoint tracked and untracked changes before enqueueing, reserving, or publishing.' }
}

function New-QueueObject($Record, [string]$Parent) {
    $Record.updated_utc = [DateTimeOffset]::UtcNow.ToString('o')
    $Record.nonce = [guid]::NewGuid().ToString('N')
    $tree = (Invoke-Git -Arguments @('mktree') -InputText '').Output
    $arguments = @('commit-tree', $tree, '-F', '-')
    if ($Parent) { $arguments += @('-p', $Parent) }
    return (Invoke-Git -Arguments $arguments -InputText (Write-Json $Record)).Output
}

try {
    $null = Invoke-Git -Arguments @('rev-parse', '--show-toplevel')
    $urls = @((Invoke-Git -Arguments @('remote', 'get-url', '--push', '--all', $Remote)).Output -split "`n")
    if ($urls.Count -ne 1 -or -not $urls[0]) { throw 'Exactly one push URL is required.' }
    $pushUrl = $urls[0]
    if ($Command -eq 'enqueue') {
        if ([string]::IsNullOrWhiteSpace($Owner) -or $Owner.Length -gt 200) { throw 'Supply a short, recognizable -Owner.' }
        if (-not $Ticket) { $Ticket = [guid]::NewGuid().ToString('N') }
    }
    if ($Command -in @('enqueue', 'claim', 'cancel')) { Require-Ticket $Ticket }
    if ($Command -eq 'recover' -and [string]::IsNullOrWhiteSpace($Reason)) { throw 'Recovery requires -Reason after checking with the owner.' }
    if ($Command -ne 'claim' -and $WaitSeconds) { throw '-WaitSeconds applies only to claim.' }
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($WaitSeconds)
    $collisions = 0

    :coordination while ($true) {
        $snapshot = Read-Snapshot
        $record = $snapshot.record
        if ($Command -eq 'status') { Write-Status $snapshot; exit 0 }
        if ($record.protocol -eq 'omega-landing-v1') {
            # A v1 owner must finish with its original client, or explicitly give
            # up the observed claim. Never overwrite an in-flight legacy owner.
            if ($Command -notin @('release', 'recover') -or $Claim -ne $snapshot.version) {
                throw 'A legacy reservation is active. Inspect status; finish with the v1 client or release its exact claim before using FIFO.'
            }
            $released = Invoke-Git -Arguments @('-c', 'push.followTags=false', 'push', '--atomic', '--porcelain',
                "--force-with-lease=${claimReference}:${Claim}", $pushUrl, ":${claimReference}") -AllowFailure
            if ($released.Code -ne 0) { throw "Legacy release uncertain; inspect status. $($released.Error)" }
            Write-Json @{ state = 'released'; claim = $Claim; action = $Command; reason = $Reason }
            exit 0
        }

        $result = $null
        switch ($Command) {
            'enqueue' {
                Require-CleanHead ''
                $existing = @($record.entries | Where-Object { $_.ticket -eq $Ticket })
                if ($existing.Count) {
                    if ($existing[0].owner -ne $Owner) { throw 'This ticket belongs to a different owner.' }
                    Write-Json @{ state = 'queued'; ticket = $Ticket; position = 1 + [array]::IndexOf($record.entries, $existing[0]) }
                    exit 0
                }
                # Retired tickets must never become valid again: otherwise a
                # resumed former owner could publish under a recreated claim.
                if ($snapshot.version -and (Invoke-Git -Arguments @('log', '-1', '--format=%H', '--fixed-strings', "--grep=$Ticket", $snapshot.version)).Output) {
                    throw 'This ticket was already removed. Enqueue with a new ticket.'
                }
                $record.entries += @{ ticket = $Ticket; owner = $Owner; created_utc = [DateTimeOffset]::UtcNow.ToString('o') }
                $result = @{ state = 'queued'; ticket = $Ticket; position = $record.entries.Count }
            }
            'claim' {
                Require-CleanHead ''
                if (-not @($record.entries | Where-Object { $_.ticket -eq $Ticket }).Count) { throw 'Ticket is no longer queued. Do not reuse a removed ticket.' }
                if ($record.entries[0].ticket -ne $Ticket) {
                    if ([DateTimeOffset]::UtcNow -ge $deadline) { Write-Status $snapshot; exit 2 }
                    $remaining = ($deadline - [DateTimeOffset]::UtcNow).TotalMilliseconds
                    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int][Math]::Min($PollSeconds * 1000, $remaining)) }
                    continue coordination
                }
                if ($record.active) {
                    $null = Invoke-Git -Arguments @('fetch', '--no-tags', '--no-write-fetch-head', $pushUrl, $record.active.base)
                    Write-Json @{ state = 'reserved'; claim = $Ticket; base = $record.active.base; owner = $record.entries[0].owner }
                    exit 0
                }
                $record.active = @{ ticket = $Ticket; base = $snapshot.main }
                $null = Invoke-Git -Arguments @('fetch', '--no-tags', '--no-write-fetch-head', $pushUrl, $snapshot.main)
                $result = @{ state = 'reserved'; claim = $Ticket; base = $snapshot.main; owner = $record.entries[0].owner }
            }
            'cancel' {
                if ($record.active -and $record.active.ticket -eq $Ticket) { throw 'Ticket is active. Release its claim instead.' }
                if (-not @($record.entries | Where-Object { $_.ticket -eq $Ticket }).Count) { throw 'Ticket is no longer queued.' }
                $record.entries = @($record.entries | Where-Object { $_.ticket -ne $Ticket })
                $result = @{ state = 'cancelled'; ticket = $Ticket; reason = $Reason }
            }
            default {
                Require-Ticket $Claim
                if (-not $record.active -or $record.active.ticket -ne $Claim) { throw 'The supplied claim is no longer active. Nothing was changed.' }
                if ($Command -eq 'publish') {
                    Require-ObjectId $Base 'Base'
                    Require-ObjectId $Candidate 'Candidate'
                    Require-CleanHead $Candidate
                    if ($Base -ne $record.active.base) { throw 'Base differs from the reserved integration base.' }
                    if ($snapshot.main -ne $Base) { throw 'Remote main advanced outside the reservation. Release and reconcile; do not retry blindly.' }
                    if ($Candidate -eq $Base) { throw 'There is nothing to publish. Release the reservation.' }
                    $null = Invoke-Git -Arguments @('merge-base', '--is-ancestor', $Base, $Candidate)
                    if ((Invoke-Git -Arguments @('rev-list', '--merges', "${Base}..${Candidate}")).Output) { throw 'The candidate contains merge commits; main must remain linear.' }
                    $result = @{ state = 'published'; candidate = $Candidate; released_claim = $Claim }
                } else {
                    $result = @{ state = 'released'; claim = $Claim; action = $Command; reason = $Reason }
                }
                $record.entries = @($record.entries | Where-Object { $_.ticket -ne $Claim })
                $record.active = $null
            }
        }

        $updated = New-QueueObject $record $snapshot.version
        $arguments = @('-c', 'push.followTags=false', 'push', '--atomic', '--porcelain',
            "--force-with-lease=${claimReference}:$($snapshot.version)", $pushUrl, "${updated}:${claimReference}")
        if ($Command -eq 'publish') { $arguments += "${Candidate}:${mainReference}" }
        $mutation = Invoke-Git -Arguments $arguments -AllowFailure
        if ($mutation.Code -eq 0) { Write-Json $result; exit 0 }

        $after = Read-RemoteReferences
        $afterVersion = if ($after.ContainsKey($claimReference)) { $after[$claimReference] } else { '' }
        if ($afterVersion -eq $updated -and ($Command -ne 'publish' -or $after[$mainReference] -eq $Candidate)) {
            Write-Json $result; exit 0
        }
        # A queue edit can race publication without invalidating its code or
        # active ticket. Re-read and reapply to preserve the new waiting entries.
        if ($afterVersion -eq $snapshot.version -or ++$collisions -ge 20) {
            throw "Update failed or outcome uncertain. Inspect status and main before retrying. $($mutation.Error) $($mutation.Output)"
        }
        Start-Sleep -Milliseconds (Get-Random -Minimum 50 -Maximum 200)
    }
} catch {
    [Console]::Error.WriteLine("landing: $($_.Exception.Message) Ticket=$Ticket Claim=$Claim")
    exit 1
}
