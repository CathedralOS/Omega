#requires -Version 7.2
<#
Reserve the final integration and push to main while everyone keeps developing
in separate worktrees. GitHub stores one tiny, non-branch reference; all builds
and tests run locally. There is no polling agent, hosted job, or work ownership.

Acquire with an explicit empty expected reference. Publish main and delete the
exact owned reference in one atomic push. A recovered or replaced claim cannot
publish, even if its original process resumes. Main must fast-forward: the
conditional force option applies ONLY to the coordination reference.

Reservations never expire automatically. Use status to inspect an abandoned
claim, then recover its exact object ID after checking with the owner. This is
a cooperative workflow: a direct push that bypasses this command can still
advance main. This command enforces custody, not whether tests were run.

From the worker's clean, checkpointed worktree:
  $reservation = ./tools/landing.ps1 claim -Owner 'Jarod / pipeline' | ConvertFrom-Json
  git rebase $reservation.base
  # Run the checks required for the change and retain the verified full SHA.
  $verified = git rev-parse HEAD
  ./tools/landing.ps1 publish -Claim $reservation.claim -Base $reservation.base -Candidate $verified
On a failed gate, release the claim before further development:
  ./tools/landing.ps1 release -Claim $reservation.claim

Exit 2 means busy; exit 1 means an error or an uncertain network outcome.
After an uncertain push, inspect status and remote main before retrying.
Local two-writer regression coverage: tools/tests/landing.tests.ps1.
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('status', 'claim', 'publish', 'release', 'recover')]
    [string]$Command = 'status',
    [string]$Repository = '.',
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]*$')]
    [string]$Remote = 'origin',
    [string]$Owner,
    [string]$Claim,
    [string]$Base,
    [string]$Candidate,
    [string]$Reason
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

function Read-Claim([string]$ObjectId) {
    $null = Invoke-Git -Arguments @('fetch', '--no-tags', '--no-write-fetch-head', $pushUrl, $ObjectId)
    $message = (Invoke-Git -Arguments @('show', '-s', '--format=%B', $ObjectId)).Output
    $record = $message | ConvertFrom-Json
    if ($record.protocol -ne 'omega-landing-v1') { throw 'Unknown reservation format; do not replace it.' }
    return $record
}

function Write-Status($References) {
    if ($References.ContainsKey($claimReference)) {
        $objectId = $References[$claimReference]
        $record = Read-Claim $objectId
        [ordered]@{
            state = 'reserved'; claim = $objectId; owner = $record.owner
            created_utc = ([DateTimeOffset]$record.created_utc).ToUniversalTime().ToString('o')
            main = $References[$mainReference]
        } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
    } else {
        [ordered]@{ state = 'available'; main = $References[$mainReference] } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
    }
}

function Require-CleanHead([string]$Expected) {
    $head = (Invoke-Git -Arguments @('rev-parse', 'HEAD')).Output
    if ($Expected -and $head -ne $Expected) { throw 'HEAD differs from the verified candidate.' }
    $dirty = (Invoke-Git -Arguments @('status', '--porcelain', '--untracked-files=normal')).Output
    if ($dirty) { throw 'Checkpoint tracked and untracked changes before reserving or publishing.' }
}

try {
    $null = Invoke-Git -Arguments @('rev-parse', '--show-toplevel')
    $urls = @((Invoke-Git -Arguments @('remote', 'get-url', '--push', '--all', $Remote)).Output -split "`n")
    if ($urls.Count -ne 1 -or -not $urls[0]) { throw 'Exactly one push URL is required.' }
    $pushUrl = $urls[0]
    $references = Read-RemoteReferences

    if ($Command -eq 'status') { Write-Status $references; exit 0 }

    if ($Command -eq 'claim') {
        if ([string]::IsNullOrWhiteSpace($Owner) -or $Owner.Length -gt 200) {
            throw 'Supply a short, recognizable owner, for example -Owner "Jarod / pipeline".'
        }
        Require-CleanHead ''
        if ($references.ContainsKey($claimReference)) { Write-Status $references; exit 2 }
        $record = [ordered]@{
            protocol = 'omega-landing-v1'; owner = $Owner
            nonce = [guid]::NewGuid().ToString('N')
            created_utc = [DateTimeOffset]::UtcNow.ToString('o')
        } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
        $emptyTree = (Invoke-Git -Arguments @('mktree') -InputText '').Output
        $attempt = (Invoke-Git -Arguments @('commit-tree', $emptyTree, '-F', '-') -InputText $record).Output
        $acquisition = Invoke-Git -Arguments @(
            '-c', 'push.followTags=false', 'push', '--porcelain',
            "--force-with-lease=${claimReference}:", $pushUrl, "${attempt}:${claimReference}"
        ) -AllowFailure
        if ($acquisition.Code -ne 0) {
            $after = Read-RemoteReferences
            if ($after.ContainsKey($claimReference) -and $after[$claimReference] -ne $attempt) {
                Write-Status $after; exit 2
            }
            throw "Claim attempt $attempt had an uncertain or failed result. Inspect status before retrying. $($acquisition.Error)"
        }
        # Pin main after winning. A former owner's atomic publication may have
        # completed between the initial observation and our successful claim.
        $current = Read-RemoteReferences
        if (-not $current.ContainsKey($claimReference) -or $current[$claimReference] -ne $attempt) {
            throw "Claim $attempt was replaced or recovered. Inspect status; do not start integration."
        }
        $baseId = $current[$mainReference]
        $null = Invoke-Git -Arguments @('fetch', '--no-tags', '--no-write-fetch-head', $pushUrl, $baseId)
        [ordered]@{ state = 'reserved'; claim = $attempt; base = $baseId; owner = $Owner } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
        exit 0
    }

    Require-ObjectId $Claim 'Claim'
    if (-not $references.ContainsKey($claimReference) -or $references[$claimReference] -ne $Claim) {
        throw 'The supplied claim is no longer current. Nothing was changed.'
    }

    if ($Command -eq 'publish') {
        Require-ObjectId $Base 'Base'
        Require-ObjectId $Candidate 'Candidate'
        Require-CleanHead $Candidate
        if ($references[$mainReference] -ne $Base) { throw 'Remote main advanced outside the reservation. Release and reconcile; do not retry blindly.' }
        if ($Candidate -eq $Base) { throw 'There is nothing to publish. Release the reservation.' }
        $null = Invoke-Git -Arguments @('merge-base', '--is-ancestor', $Base, $Candidate)
        $merges = (Invoke-Git -Arguments @('rev-list', '--merges', "${Base}..${Candidate}")).Output
        if ($merges) { throw 'The candidate contains merge commits; main must remain linear.' }
        $publication = Invoke-Git -Arguments @(
            '-c', 'push.followTags=false', 'push', '--atomic', '--porcelain',
            "--force-with-lease=${claimReference}:${Claim}", $pushUrl,
            "${Candidate}:${mainReference}", ":${claimReference}"
        ) -AllowFailure
        if ($publication.Code -ne 0) {
            throw "Publication failed or its outcome is uncertain. Inspect remote main and claim $Claim before retrying. $($publication.Error) $($publication.Output)"
        }
        [ordered]@{ state = 'published'; candidate = $Candidate; released_claim = $Claim } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
        exit 0
    }

    if ($Command -eq 'recover' -and [string]::IsNullOrWhiteSpace($Reason)) {
        throw 'Recovery requires -Reason after checking that the observed owner has abandoned integration.'
    }
    $release = Invoke-Git -Arguments @(
        '-c', 'push.followTags=false', 'push', '--atomic', '--porcelain',
        "--force-with-lease=${claimReference}:${Claim}", $pushUrl, ":${claimReference}"
    ) -AllowFailure
    if ($release.Code -ne 0) {
        throw "Release failed or its outcome is uncertain. Inspect claim $Claim before retrying. $($release.Error) $($release.Output)"
    }
    [ordered]@{ state = 'released'; claim = $Claim; action = $Command; reason = $Reason } | ConvertTo-Json -Compress -EscapeHandling EscapeNonAscii
} catch {
    [Console]::Error.WriteLine("landing: $($_.Exception.Message)")
    exit 1
}
