//! Cross-machine work-claim registry (`cargo xtask claims`) on the `work/claims` branch.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::repo_root;

// A branch, because the Git proxy in front of Claude Code cloud sessions
// accepts pushes only to `refs/heads/*`. The proxy also refuses forced
// updates and deletions, so every write fast-forwards this branch.
const CLAIMS_REF: &str = "refs/heads/work/claims";
// The registry before 2026-11-15. Older tools still write it, so status reads
// it too until TASKS.md `CLAIMS-LEGACY-READ-REMOVAL` deletes that read.
const LEGACY_CLAIMS_REF: &str = "refs/coordination/omega-claims/main";
const PROTOCOL: &str = "omega-claims-v1";
const LEASE_MINUTES: u64 = 120;
const WRITE_ATTEMPTS: u32 = 5;
const TICKETS_FILE: &str = "omega-claim-tickets";
const NANOSECONDS_PER_SECOND: i128 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClaimsRecord {
    protocol: String,
    #[serde(default)]
    claims: Vec<serde_json::Value>,
    #[serde(default)]
    notes: Vec<serde_json::Value>,
    /// The legacy claims the branch took over when it was seeded. Status
    /// shows a legacy claim only when its ticket is in neither the branch's
    /// claims nor this list, so a claim released on the branch stays gone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    seeded_tickets: Vec<String>,
}

impl Default for ClaimsRecord {
    fn default() -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            claims: Vec::new(),
            notes: Vec::new(),
            seeded_tickets: Vec::new(),
        }
    }
}

fn leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn month_days(year: u32, month: u32) -> u32 {
    match month {
        2 if leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_before_year(year: u32) -> i64 {
    let previous = i64::from(year) - 1;
    previous * 365 + previous / 4 - previous / 100 + previous / 400
}

fn decimal(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

// Unknown expiry must remain visible: only a complete, valid timestamp proves
// that a record has expired. The registry's historical offsets and fractions
// describe instants. Keep their precision independent of SystemTime: converting
// a future expiry to Windows' 100ns clock could round it down to the present.
fn parse_expiry(text: &str) -> Option<i128> {
    if text.len() < 20 || !text.is_ascii() {
        return None;
    }
    let bytes = text.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = decimal(&text[..4])?;
    let month = decimal(&text[5..7])?;
    let day = decimal(&text[8..10])?;
    let hour = decimal(&text[11..13])?;
    let minute = decimal(&text[14..16])?;
    let second = decimal(&text[17..19])?;
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > month_days(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let tail = &text[19..];
    let zone_start = tail.find(['Z', 'z', '+', '-'])?;
    let fraction = &tail[..zone_start];
    let nanos = if fraction.is_empty() {
        0
    } else {
        let digits = fraction.strip_prefix('.')?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let precision = digits.len().min(9);
        let nanos = decimal(&digits[..precision])? * 10u32.pow((9 - precision) as u32);
        // Round expiry finer than a nanosecond up, never into the past.
        nanos + u32::from(digits[precision..].bytes().any(|byte| byte != b'0'))
    };
    let zone = &tail[zone_start..];
    let offset = if matches!(zone, "Z" | "z") {
        0
    } else {
        if zone.len() != 6
            || !matches!(zone.as_bytes()[0], b'+' | b'-')
            || zone.as_bytes()[3] != b':'
        {
            return None;
        }
        let hours = decimal(&zone[1..3])?;
        let minutes = decimal(&zone[4..6])?;
        if hours > 23 || minutes > 59 {
            return None;
        }
        let sign = if zone.starts_with('+') { 1 } else { -1 };
        sign * i64::from(hours * 3600 + minutes * 60)
    };
    let days = days_before_year(year) - days_before_year(1970)
        + (1..month)
            .map(|m| i64::from(month_days(year, m)))
            .sum::<i64>()
        + i64::from(day - 1);
    let seconds = days * 86400 + i64::from(hour * 3600 + minute * 60 + second) - offset;
    Some(i128::from(seconds) * NANOSECONDS_PER_SECOND + i128::from(nanos))
}

fn epoch_nanoseconds(time: SystemTime) -> i128 {
    let (elapsed, sign) = match time.duration_since(UNIX_EPOCH) {
        Ok(elapsed) => (elapsed, 1),
        Err(error) => (error.duration(), -1),
    };
    sign * (i128::from(elapsed.as_secs()) * NANOSECONDS_PER_SECOND
        + i128::from(elapsed.subsec_nanos()))
}

fn format_utc(time: SystemTime) -> Result<String, String> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("UTC timestamp precedes the Unix epoch: {error}"))?;
    if elapsed.as_secs() > 253_402_300_799 {
        return Err("UTC timestamp exceeds year 9999".to_owned());
    }
    let mut days = elapsed.as_secs() / 86400;
    let mut year = 1970;
    while days >= u64::from(if leap_year(year) { 366u32 } else { 365 }) {
        days -= u64::from(if leap_year(year) { 366u32 } else { 365 });
        year += 1;
    }
    let mut month = 1;
    while days >= u64::from(month_days(year, month)) {
        days -= u64::from(month_days(year, month));
        month += 1;
    }
    let seconds = elapsed.as_secs() % 86400;
    let fraction = if elapsed.subsec_nanos() == 0 {
        String::new()
    } else {
        format!(".{:09}", elapsed.subsec_nanos())
    };
    Ok(format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}{fraction}+00:00",
        days + 1,
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
    ))
}

fn lease_timestamps(now: SystemTime) -> Result<(String, String), String> {
    let expires = now
        .checked_add(Duration::from_secs(LEASE_MINUTES * 60))
        .ok_or("claim expiry exceeds the system clock range")?;
    Ok((format_utc(now)?, format_utc(expires)?))
}

// A claim lives at most one lease past its last sign of life: when it was taken
// or last renewed, whatever expiry it records. 66 claims, all stamped
// 2026-11-07T00:00:00Z with a 2099 expiry, stayed visible a week later, some on
// paths that no longer existed, so a recorded expiry can shorten a claim but
// never lengthen it. Unknown times still
// keep a claim visible: only a parsed instant proves that it ended.
fn claim_expiry(claim: &serde_json::Value) -> Option<i128> {
    let instant = |field: &str| {
        claim
            .get(field)
            .and_then(serde_json::Value::as_str)
            .and_then(parse_expiry)
    };
    let alive = match (instant("claimed_utc"), instant("renewed_utc")) {
        (Some(claimed), Some(renewed)) => Some(claimed.max(renewed)),
        (claimed, renewed) => claimed.or(renewed),
    };
    let lease_ceiling =
        alive.map(|alive| alive + i128::from(LEASE_MINUTES * 60) * NANOSECONDS_PER_SECOND);
    match (instant("expires_utc"), lease_ceiling) {
        (Some(recorded), Some(ceiling)) => Some(recorded.min(ceiling)),
        (recorded, ceiling) => recorded.or(ceiling),
    }
}

// Renewal starts a fresh lease from `now` on a claim that is still live. A
// lapsed claim is restored instead by `revive_claim`.
fn renew_claim(record: &mut ClaimsRecord, ticket: &str, now: SystemTime) -> Result<bool, String> {
    let (renewed_utc, expires_utc) = lease_timestamps(now)?;
    let now = epoch_nanoseconds(now);
    let Some(claim) = record.claims.iter_mut().find(|claim| {
        claim.get("ticket").and_then(serde_json::Value::as_str) == Some(ticket)
            && claim_expiry(claim).is_none_or(|expires| expires > now)
    }) else {
        return Ok(false);
    };
    // A retried note must not shorten a renewal another writer already made.
    if claim
        .get("renewed_utc")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_expiry)
        .is_some_and(|renewed| renewed >= now)
    {
        return Ok(true);
    }
    if let Some(fields) = claim.as_object_mut() {
        fields.insert("renewed_utc".to_owned(), renewed_utc.into());
        fields.insert("expires_utc".to_owned(), expires_utc.into());
        fields.insert("lease_minutes".to_owned(), LEASE_MINUTES.into());
    }
    Ok(true)
}

/// Two claimed paths overlap when one is the other or lies inside it.
fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left.trim_end_matches('/');
    let right = right.trim_end_matches('/');
    let inside = |inner: &str, outer: &str| {
        inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('/'))
    };
    left == right || inside(left, right) || inside(right, left)
}

// A lapsed claim this worktree published is restored under its own ticket,
// item, owner and paths with a fresh lease, so an agent whose lease ran out
// during a long build does not retype its claim. Claiming never refused
// overlap, so neither does restoring; it returns the live claims of other
// tickets whose paths now overlap, for the caller to report.
fn revive_claim(
    record: &mut ClaimsRecord,
    saved: &serde_json::Value,
    now: SystemTime,
) -> Result<Vec<serde_json::Value>, String> {
    let ticket = saved
        .get("ticket")
        .and_then(serde_json::Value::as_str)
        .ok_or("a saved claim has no ticket")?;
    let (renewed_utc, expires_utc) = lease_timestamps(now)?;
    let instant = epoch_nanoseconds(now);
    let paths = |claim: &serde_json::Value| -> Vec<String> {
        claim
            .get("paths")
            .and_then(serde_json::Value::as_array)
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mine = paths(saved);
    let overlaps = record
        .claims
        .iter()
        .filter(|claim| {
            claim.get("ticket").and_then(serde_json::Value::as_str) != Some(ticket)
                && claim_expiry(claim).is_none_or(|expires| expires > instant)
                && paths(claim)
                    .iter()
                    .any(|theirs| mine.iter().any(|path| paths_overlap(path, theirs)))
        })
        .map(|claim| {
            serde_json::json!({
                "ticket": claim.get("ticket"),
                "owner": claim.get("owner"),
                "item": claim.get("item"),
            })
        })
        .collect();
    record
        .claims
        .retain(|claim| claim.get("ticket").and_then(serde_json::Value::as_str) != Some(ticket));
    let mut restored = saved.clone();
    if let Some(fields) = restored.as_object_mut() {
        fields.insert("renewed_utc".to_owned(), renewed_utc.into());
        fields.insert("expires_utc".to_owned(), expires_utc.into());
        fields.insert("lease_minutes".to_owned(), LEASE_MINUTES.into());
    }
    record.claims.push(restored);
    Ok(overlaps)
}

// Every write republishes the whole record. Ended claims and their notes grew
// it to 915 claims and 1,148 notes (1.7 MB per write); status hides the first
// and only counts the second. A write therefore drops ended claims and the
// notes of any ticket that no longer has a visible claim, so a note lasts only
// as long as its claim. Earlier records stay in the reference's history, which
// this does not rewrite. A claim whose expiry cannot be read stays, with its
// notes, as status keeps it.
fn prune_registry(record: &mut ClaimsRecord, now: SystemTime) {
    let now = epoch_nanoseconds(now);
    record
        .claims
        .retain(|claim| claim_expiry(claim).is_none_or(|expires| expires > now));
    let mut live: Vec<String> = record
        .claims
        .iter()
        .filter_map(|claim| claim.get("ticket").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect();
    live.sort_unstable();
    record.notes.retain(|note| {
        note.get("ticket")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|ticket| live.binary_search_by(|t| t.as_str().cmp(ticket)).is_ok())
    });
}

fn status_snapshot(snapshot: &Snapshot, now: SystemTime) -> serde_json::Value {
    let now = epoch_nanoseconds(now);
    let live = |claim: &&serde_json::Value| claim_expiry(claim).is_none_or(|expires| expires > now);
    let ticket = |value: &serde_json::Value| {
        value
            .get("ticket")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let empty = ClaimsRecord::default();
    let branch = snapshot.branch.as_ref().unwrap_or(&empty);
    let mut claims: Vec<serde_json::Value> = branch.claims.iter().filter(live).cloned().collect();
    let mut notes_pending = branch.notes.len();
    if let Some(legacy) = &snapshot.legacy {
        // Claims an older tool wrote after the branch was seeded. Mark them, so
        // a reader knows a branch write cannot renew or release them.
        let mut known: Vec<String> = branch.claims.iter().filter_map(ticket).collect();
        known.extend(branch.seeded_tickets.iter().cloned());
        known.sort_unstable();
        let mut shown = Vec::new();
        for claim in legacy.claims.iter().filter(live) {
            let Some(id) = ticket(claim) else { continue };
            if known.binary_search(&id).is_ok() {
                continue;
            }
            let mut claim = claim.clone();
            if let Some(fields) = claim.as_object_mut() {
                fields.insert("registry".to_owned(), LEGACY_CLAIMS_REF.into());
            }
            claims.push(claim);
            shown.push(id);
        }
        notes_pending += legacy
            .notes
            .iter()
            .filter(|note| ticket(note).is_some_and(|id| shown.contains(&id)))
            .count();
    }
    serde_json::json!({
        "state": if claims.is_empty() { "available" } else { "claimed" },
        "claims": claims,
        "notes_pending": notes_pending,
        "version": snapshot.heads.branch,
        "legacy_version": snapshot.heads.legacy,
    })
}

fn clock_error(error: String) -> u8 {
    eprintln!("claims: {error}");
    1
}

fn append_claim(record: &mut ClaimsRecord, claim: &serde_json::Value) -> Result<(), u8> {
    let ticket = &claim["ticket"];
    if let Some(existing) = record
        .claims
        .iter()
        .find(|existing| existing["ticket"] == *ticket)
    {
        // A later writer may already have renewed this same logical claim.
        // Compare its identity without replacing the newer lease fields.
        if ["claimed_utc", "item", "owner", "board", "paths", "note"]
            .into_iter()
            .all(|field| existing[field] == claim[field])
        {
            return Ok(());
        }
        eprintln!("claims: ticket {ticket} already names a different claim");
        return Err(1);
    }
    record.claims.push(claim.clone());
    Ok(())
}

fn append_note(
    record: &mut ClaimsRecord,
    ticket: &str,
    text: &str,
    now: SystemTime,
) -> Result<(), u8> {
    let created_utc = format_utc(now).map_err(clock_error)?;
    if record.notes.iter().any(|note| {
        note["ticket"] == ticket
            && note["created_utc"] == created_utc
            && note["item"] == "note"
            && note["owner"] == "agent"
            && note["text"] == text
    }) {
        return Ok(());
    }
    // A note is a sign of life: it renews the claim it names.
    renew_claim(record, ticket, now).map_err(clock_error)?;
    record.notes.push(serde_json::json!({
        "ticket": ticket,
        "item": "note",
        "owner": "agent",
        "text": text,
        "created_utc": created_utc,
        "swept_utc": null,
    }));
    Ok(())
}

/// The registry heads the remote advertises; an empty string means that
/// reference does not exist.
#[derive(Debug, Clone, Default, PartialEq)]
struct RegistryHeads {
    branch: String,
    legacy: String,
}

/// Which records one read needs. Status shows both registries during the
/// transition; a write needs the legacy record only to seed an absent branch.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LegacyRead {
    Always,
    WhenBranchAbsent,
}

/// One read of the remote registry: the heads it advertised and the records
/// at those exact commits.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    heads: RegistryHeads,
    branch: Option<ClaimsRecord>,
    legacy: Option<ClaimsRecord>,
}

impl Snapshot {
    /// The commit a write builds on and the record it changes. While the
    /// branch does not exist, the first write builds on the legacy record,
    /// so the branch starts with that history and those claims. It remembers
    /// which tickets it took over, so status does not show them again from
    /// the legacy reference after the branch releases them.
    fn write_base(&self) -> (String, ClaimsRecord) {
        if !self.heads.branch.is_empty() {
            return (
                self.heads.branch.clone(),
                self.branch.clone().unwrap_or_default(),
            );
        }
        if self.heads.legacy.is_empty() {
            return (String::new(), ClaimsRecord::default());
        }
        let mut record = self.legacy.clone().unwrap_or_default();
        record.seeded_tickets = record
            .claims
            .iter()
            .filter_map(|claim| claim.get("ticket").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect();
        (self.heads.legacy.clone(), record)
    }

    fn wanted(heads: &RegistryHeads, legacy: LegacyRead) -> (bool, bool) {
        let branch = !heads.branch.is_empty();
        let legacy = !heads.legacy.is_empty() && (legacy == LegacyRead::Always || !branch);
        (branch, legacy)
    }

    fn versions(heads: &RegistryHeads, legacy: LegacyRead) -> Vec<&str> {
        let (branch, legacy) = Self::wanted(heads, legacy);
        let mut versions = Vec::with_capacity(2);
        if branch {
            versions.push(heads.branch.as_str());
        }
        if legacy {
            versions.push(heads.legacy.as_str());
        }
        versions
    }

    fn read(repo: &Path, heads: RegistryHeads, legacy: LegacyRead) -> Result<Self, u8> {
        let (want_branch, want_legacy) = Self::wanted(&heads, legacy);
        let branch = want_branch
            .then(|| read_record(repo, &heads.branch))
            .transpose()?;
        let legacy = want_legacy
            .then(|| read_record(repo, &heads.legacy))
            .transpose()?;
        Ok(Self {
            heads,
            branch,
            legacy,
        })
    }
}

fn fetch_snapshot(repo: &Path, legacy: LegacyRead) -> Result<Snapshot, u8> {
    let heads = remote_heads(repo)?;
    let mut cached = true;
    for version in Snapshot::versions(&heads, legacy) {
        cached &= snapshot_is_cached(repo, version)?;
    }
    if cached {
        return Snapshot::read(repo, heads, legacy);
    }
    // A source fetch can hold admission for much longer than a claim update.
    // Sample the heads again after that wait, so the lease does not age in line.
    crate::repository::with_git_fetch(repo, |fetch| {
        Ok(fetch_snapshot_admitted(repo, legacy, fetch))
    })
    .map_err(fetch_error)?
}

fn fetch_error(error: String) -> u8 {
    eprintln!("claims: {error}");
    1
}

fn remote_heads(repo: &Path) -> Result<RegistryHeads, u8> {
    let ls = Command::new("git")
        .args(["ls-remote", "origin", CLAIMS_REF, LEGACY_CLAIMS_REF])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            eprintln!("claims: git ls-remote failed: {e}");
            1u8
        })?;
    if !ls.status.success() {
        eprintln!("claims: cannot read the remote registry references");
        return Err(1);
    }
    let mut heads = RegistryHeads::default();
    for line in String::from_utf8_lossy(&ls.stdout).lines() {
        let mut fields = line.split_whitespace();
        let (Some(version), Some(name)) = (fields.next(), fields.next()) else {
            continue;
        };
        // ls-remote matches patterns by suffix; keep only the exact names.
        if name == CLAIMS_REF {
            heads.branch = version.to_owned();
        } else if name == LEGACY_CLAIMS_REF {
            heads.legacy = version.to_owned();
        }
    }
    Ok(heads)
}

fn fetch_snapshot_admitted(
    repo: &Path,
    legacy: LegacyRead,
    fetch: &mut Command,
) -> Result<Snapshot, u8> {
    let heads = remote_heads(repo)?;
    fetch_versions_admitted(repo, &Snapshot::versions(&heads, legacy), fetch)?;
    Snapshot::read(repo, heads, legacy)
}

fn snapshot_is_cached(repo: &Path, version: &str) -> Result<bool, u8> {
    // Worktrees share immutable objects. Locally written snapshots need no
    // network fetch or shared fetch lock before the compare-and-swap push.
    let cached = Command::new("git")
        .args(["cat-file", "-e", &format!("{version}^{{commit}}")])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            eprintln!("claims: cannot inspect local snapshot {version}: {error}");
            1u8
        })?;
    Ok(cached.success())
}

fn fetch_versions_admitted(repo: &Path, versions: &[&str], fetch: &mut Command) -> Result<(), u8> {
    // Fetch the exact advertised snapshots: another writer may move a ref
    // between ls-remote and fetch. Reading a different or empty record under
    // the advertised base could overwrite other sessions' claims.
    // Only the newest record matters: each commit's message is the whole
    // registry, and the history is tens of thousands of such commits, so a
    // full fetch can exhaust a session's memory.
    let mut missing = Vec::with_capacity(versions.len());
    for version in versions {
        if !snapshot_is_cached(repo, version)? {
            missing.push(*version);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    let status = fetch
        .args(["-q", "--depth=1", "origin"])
        .args(&missing)
        .status()
        .map_err(|error| fetch_error(format!("cannot fetch snapshots {missing:?}: {error}")))?;
    if !status.success() {
        eprintln!("claims: cannot fetch registry snapshots {missing:?}");
        return Err(1);
    }
    Ok(())
}

#[cfg(test)]
fn fetch_record(repo: &Path, version: &str) -> Result<ClaimsRecord, u8> {
    if !snapshot_is_cached(repo, version)? {
        crate::repository::with_git_fetch(repo, |fetch| {
            Ok(fetch_versions_admitted(repo, &[version], fetch))
        })
        .map_err(fetch_error)??;
    }
    read_record(repo, version)
}

fn read_record(repo: &Path, version: &str) -> Result<ClaimsRecord, u8> {
    let show = Command::new("git")
        .args(["show", "-s", "--format=%B", version])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            eprintln!("claims: git show failed: {e}");
            1u8
        })?;
    if !show.status.success() {
        eprintln!("claims: cannot read registry snapshot {version}");
        return Err(1);
    }
    let body = String::from_utf8_lossy(&show.stdout);
    let record: ClaimsRecord = serde_json::from_str(body.trim()).map_err(|error| {
        eprintln!("claims: invalid registry snapshot {version}: {error}");
        1u8
    })?;
    if record.protocol != PROTOCOL {
        eprintln!("claims: unsupported registry protocol in snapshot {version}");
        return Err(1);
    }
    Ok(record)
}

#[derive(Debug)]
enum PublicationError {
    Prepare,
    Push,
}

fn commit_and_push(
    repo: &Path,
    prev_version: &str,
    record: &ClaimsRecord,
) -> Result<(), PublicationError> {
    let empty_tree = Command::new("git")
        .args(["hash-object", "-t", "tree", "--stdin"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| PublicationError::Prepare)?;
    if !empty_tree.status.success() {
        eprintln!("claims: cannot create registry tree");
        return Err(PublicationError::Prepare);
    }
    let tree_sha = String::from_utf8_lossy(&empty_tree.stdout)
        .trim()
        .to_owned();
    let body = serde_json::to_string(record).map_err(|_| PublicationError::Prepare)?;

    let mut commit_cmd = Command::new("git");
    commit_cmd.arg("commit-tree").arg(&tree_sha);
    if !prev_version.is_empty() {
        commit_cmd.arg("-p").arg(prev_version);
    }
    let mut child = commit_cmd
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| PublicationError::Prepare)?;
    let write = child.stdin.take().unwrap().write_all(body.as_bytes());
    let out = child
        .wait_with_output()
        .map_err(|_| PublicationError::Prepare)?;
    write.map_err(|error| {
        eprintln!("claims: cannot write registry payload: {error}");
        PublicationError::Prepare
    })?;
    if !out.status.success() {
        eprintln!("claims: git commit-tree failed");
        return Err(PublicationError::Prepare);
    }
    let new_sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    // A plain push, never forced: the new commit's parent is the tip this
    // write read, so the remote accepts it only while that is still the tip.
    // A concurrent writer makes it a non-fast-forward, which Git refuses.
    let target = format!("{new_sha}:{CLAIMS_REF}");
    let push = Command::new("git")
        .args(["push", "-q", "origin", &target])
        .current_dir(repo)
        .stdin(Stdio::null())
        .status()
        .map_err(|_| PublicationError::Push)?;
    if !push.success() {
        eprintln!("claims: push to {CLAIMS_REF} failed (concurrent writer or network error)");
        return Err(PublicationError::Push);
    }
    Ok(())
}

// Every agent writes the same ref, so a write loses to a concurrent one often
// (about a third of 66 back-to-back releases did). Each attempt reapplies the
// change to the newest snapshot rather than overwriting it.
fn update_registry<T>(
    repo: &Path,
    mut change: impl FnMut(&mut ClaimsRecord) -> Result<T, u8>,
) -> Result<T, u8> {
    let mut attempt = 1;
    let (mut version, mut record) =
        fetch_snapshot(repo, LegacyRead::WhenBranchAbsent)?.write_base();
    loop {
        let previous = record.clone();
        let outcome = change(&mut record)?;
        if record == previous {
            return Ok(outcome);
        }
        prune_registry(&mut record, SystemTime::now());
        match commit_and_push(repo, &version, &record) {
            Ok(()) => return Ok(outcome),
            Err(PublicationError::Push) => {
                let (latest_version, latest_record) =
                    fetch_snapshot(repo, LegacyRead::WhenBranchAbsent)?.write_base();
                // A lost acknowledgement can hide a successful publication.
                if latest_record == record {
                    return Ok(outcome);
                }
                if latest_version == version || attempt == WRITE_ATTEMPTS {
                    return Err(1);
                }
                eprintln!(
                    "claims: retrying against the newer registry ({attempt}/{WRITE_ATTEMPTS})"
                );
                version = latest_version;
                record = latest_record;
                attempt += 1;
            }
            Err(PublicationError::Prepare) => return Err(1),
        }
    }
}

// The claims this worktree holds, so `claims renew` needs no arguments. The
// file sits in the worktree's own git directory: each worktree renews only its
// own claims, and nothing is committed. Each line is the claim as it was
// published, so `renew` can restore a claim whose lease lapsed. A line from an
// older file holds only the ticket.
#[derive(Clone, Debug, PartialEq)]
struct HeldClaim {
    ticket: String,
    claim: Option<serde_json::Value>,
}

impl HeldClaim {
    fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let claim = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .filter(serde_json::Value::is_object);
        let ticket = claim
            .as_ref()
            .and_then(|claim| claim.get("ticket"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(line)
            .to_owned();
        Some(Self { ticket, claim })
    }

    fn line(&self) -> String {
        self.claim
            .as_ref()
            .map_or_else(|| self.ticket.clone(), serde_json::Value::to_string)
    }
}

fn tickets_file(repo: &Path) -> Option<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", TICKETS_FILE])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| repo.join(String::from_utf8_lossy(&output.stdout).trim()))
}

fn held_claims(repo: &Path) -> Vec<HeldClaim> {
    tickets_file(repo)
        .and_then(|file| std::fs::read_to_string(file).ok())
        .map(|text| text.lines().filter_map(HeldClaim::parse).collect())
        .unwrap_or_default()
}

fn write_held_claims(repo: &Path, held: &[HeldClaim]) {
    let Some(file) = tickets_file(repo) else {
        eprintln!("claims: cannot locate this worktree's ticket file; renew with --ticket");
        return;
    };
    let text: String = held
        .iter()
        .map(|held| format!("{}\n", held.line()))
        .collect();
    if let Err(error) = std::fs::write(&file, text) {
        eprintln!(
            "claims: cannot record held tickets in {}: {error}",
            file.display()
        );
    }
}

fn generate_ticket_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!(
        "{:016x}{:016x}",
        nanos as u64,
        (nanos >> 32) as u64 ^ pid as u64
    )
}

enum Request {
    Status,
    Claim {
        board: Option<String>,
        item: String,
        owner: String,
        paths: Vec<String>,
        note: Option<String>,
    },
    Release {
        ticket: String,
    },
    Renew {
        tickets: Vec<String>,
    },
    Note {
        ticket: String,
        text: String,
    },
}

fn option_value<'a>(
    args: &mut impl Iterator<Item = &'a str>,
    option: &str,
) -> Result<String, String> {
    args.next()
        .map(str::to_owned)
        .ok_or_else(|| format!("{option} requires a value"))
}

/// Help and malformed requests never need a workspace or registry connection.
/// A value such as `--text --help` remains text, not a help option.
fn parse(args: &[String]) -> Result<Option<Request>, String> {
    let mut args = args.iter().map(String::as_str);
    let Some(command) = args.next() else {
        return Ok(None);
    };
    match command {
        "-h" | "--help" => Ok(None),
        "status" => {
            if let Some(option) = args.next() {
                return match option {
                    "-h" | "--help" => Ok(None),
                    _ => Err(format!("unknown option `{option}` for `status`")),
                };
            }
            Ok(Some(Request::Status))
        }
        "claim" => {
            let mut board = None;
            let mut item = String::new();
            let mut owner = String::from("agent");
            let mut paths = Vec::new();
            let mut note = None;
            while let Some(option) = args.next() {
                match option {
                    "-h" | "--help" => return Ok(None),
                    "--board" => board = Some(option_value(&mut args, option)?),
                    "--item" => item = option_value(&mut args, option)?,
                    "--owner" => owner = option_value(&mut args, option)?,
                    "--path" => paths.extend(
                        option_value(&mut args, option)?
                            .split(',')
                            .flat_map(str::split_whitespace)
                            .filter(|path| !path.is_empty())
                            .map(str::to_owned),
                    ),
                    "--note" => note = Some(option_value(&mut args, option)?),
                    _ => return Err(format!("unknown option `{option}` for `claim`")),
                }
            }
            if item.is_empty() {
                return Err("--item is required for `claim`".to_owned());
            }
            Ok(Some(Request::Claim {
                board,
                item,
                owner,
                paths,
                note,
            }))
        }
        "release" => {
            let mut ticket = String::new();
            while let Some(option) = args.next() {
                match option {
                    "-h" | "--help" => return Ok(None),
                    "--ticket" => ticket = option_value(&mut args, option)?,
                    _ => return Err(format!("unknown option `{option}` for `release`")),
                }
            }
            if ticket.is_empty() {
                return Err("--ticket is required for `release`".to_owned());
            }
            Ok(Some(Request::Release { ticket }))
        }
        "renew" => {
            let mut tickets = Vec::new();
            while let Some(option) = args.next() {
                match option {
                    "-h" | "--help" => return Ok(None),
                    "--ticket" => tickets.push(option_value(&mut args, option)?),
                    _ => return Err(format!("unknown option `{option}` for `renew`")),
                }
            }
            Ok(Some(Request::Renew { tickets }))
        }
        "note" => {
            let mut ticket = String::new();
            let mut text = String::new();
            while let Some(option) = args.next() {
                match option {
                    "-h" | "--help" => return Ok(None),
                    "--ticket" => ticket = option_value(&mut args, option)?,
                    "--text" => text = option_value(&mut args, option)?,
                    _ => return Err(format!("unknown option `{option}` for `note`")),
                }
            }
            if ticket.is_empty() || text.is_empty() {
                return Err("--ticket and --text are required for `note`".to_owned());
            }
            Ok(Some(Request::Note { ticket, text }))
        }
        other => Err(format!("unknown subcommand `{other}`")),
    }
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let request = parse(args).map_err(|error| {
        eprintln!("claims: {error}");
        2u8
    })?;
    let Some(request) = request else {
        println!(
            "usage: cargo xtask claims <status|claim|release|renew|note> [--board <file>] [--item <id>] [--owner <name>] [--path <paths>] [--ticket <id>] [--text <note>]\n\
             A claim lasts {LEASE_MINUTES} minutes from when it was taken or last renewed. \
             `renew` (no --ticket: every claim this worktree took) and `note` extend it; `renew` restores a lapsed claim this worktree took."
        );
        return Ok(());
    };
    let repo = repo_root()?;

    match request {
        Request::Status => {
            let snapshot = fetch_snapshot(&repo, LegacyRead::Always)?;
            let out = status_snapshot(&snapshot, SystemTime::now());
            crate::write_status(|output| {
                writeln!(output, "{}", serde_json::to_string_pretty(&out).unwrap())
            })
        }
        Request::Claim {
            board,
            item,
            owner,
            paths,
            note,
        } => {
            let ticket = generate_ticket_id();
            let (claimed_utc, expires_utc) =
                lease_timestamps(SystemTime::now()).map_err(clock_error)?;
            let claim = serde_json::json!({
                    "ticket": ticket,
                    "item": item,
                    "board": board,
                    "owner": owner,
                    "paths": paths,
                    "note": note,
                    "lease_minutes": LEASE_MINUTES,
                    "claimed_utc": claimed_utc,
                    "expires_utc": expires_utc,
            });
            update_registry(&repo, |record| append_claim(record, &claim))?;
            let mut held = held_claims(&repo);
            held.push(HeldClaim {
                ticket: ticket.clone(),
                claim: Some(claim),
            });
            write_held_claims(&repo, &held);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "state": "claimed",
                    "ticket": ticket,
                    "item": item,
                }))
                .unwrap()
            );
            Ok(())
        }
        Request::Release { ticket } => {
            update_registry(&repo, |record| {
                record.claims.retain(|claim| {
                    claim.get("ticket").and_then(|value| value.as_str()) != Some(ticket.as_str())
                });
                Ok(())
            })?;
            let mut held = held_claims(&repo);
            held.retain(|held| held.ticket != ticket);
            write_held_claims(&repo, &held);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "state": "released",
                    "ticket": ticket,
                }))
                .unwrap()
            );
            Ok(())
        }
        Request::Renew { tickets } => {
            let held = held_claims(&repo);
            let tickets = if tickets.is_empty() {
                held.iter().map(|held| held.ticket.clone()).collect()
            } else {
                tickets
            };
            if tickets.is_empty() {
                eprintln!("claims: this worktree holds no claim to renew; pass --ticket <id>");
                return Err(1);
            }
            let saved = |ticket: &String| {
                held.iter()
                    .find(|held| held.ticket == *ticket)
                    .and_then(|held| held.claim.clone())
            };
            let (renewed, revived, overlaps, published) = update_registry(&repo, |record| {
                let now = SystemTime::now();
                let mut renewed = Vec::new();
                let mut revived = Vec::new();
                let mut overlaps = Vec::new();
                for ticket in &tickets {
                    if renew_claim(record, ticket, now).map_err(clock_error)? {
                        renewed.push(ticket.clone());
                    } else if let Some(claim) = saved(ticket) {
                        overlaps.extend(revive_claim(record, &claim, now).map_err(clock_error)?);
                        revived.push(ticket.clone());
                    }
                }
                // Save what is now published, so a ticket recorded before claims
                // were saved whole can be restored after a later lapse.
                let published = record
                    .claims
                    .iter()
                    .filter(|claim| {
                        claim
                            .get("ticket")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|ticket| {
                                renewed.iter().chain(&revived).any(|kept| kept == ticket)
                            })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                Ok((renewed, revived, overlaps, published))
            })?;
            let lapsed: Vec<_> = tickets
                .iter()
                .filter(|t| !renewed.contains(t) && !revived.contains(t))
                .collect();
            // A ticket with no saved claim cannot be restored; stop offering it.
            let mut held = held_claims(&repo);
            held.retain(|held| !lapsed.contains(&&held.ticket));
            for held in &mut held {
                if let Some(claim) = published
                    .iter()
                    .find(|claim| claim["ticket"] == held.ticket.as_str())
                {
                    held.claim = Some(claim.clone());
                }
            }
            write_held_claims(&repo, &held);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "state": if lapsed.is_empty() { "renewed" } else { "partially renewed" },
                    "renewed": renewed,
                    "revived": revived,
                    "overlapping": overlaps,
                    "lapsed": lapsed,
                    "lease_minutes": LEASE_MINUTES,
                }))
                .unwrap()
            );
            if !overlaps.is_empty() {
                eprintln!(
                    "claims: a restored claim overlaps live claims listed under `overlapping`; check what is in flight before editing"
                );
            }
            if lapsed.is_empty() {
                Ok(())
            } else {
                eprintln!(
                    "claims: tickets this worktree never recorded cannot be restored; claim again"
                );
                Err(1)
            }
        }
        Request::Note { ticket, text } => {
            let now = SystemTime::now();
            update_registry(&repo, |record| append_note(record, &ticket, &text, now))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "state": "noted",
                    "ticket": ticket,
                }))
                .unwrap()
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read_base(repo: &Path) -> Result<(String, ClaimsRecord), u8> {
        fetch_snapshot(repo, LegacyRead::Always).map(|snapshot| snapshot.write_base())
    }

    fn branch_status(version: &str, record: &ClaimsRecord, now: SystemTime) -> serde_json::Value {
        let snapshot = Snapshot {
            heads: RegistryHeads {
                branch: version.to_owned(),
                legacy: String::new(),
            },
            branch: Some(record.clone()),
            legacy: None,
        };
        status_snapshot(&snapshot, now)
    }

    struct RegistryFixture {
        root: std::path::PathBuf,
        reader: std::path::PathBuf,
        remote: std::path::PathBuf,
    }

    impl RegistryFixture {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("omega-claims-read-{}-{unique}", std::process::id()));
            let reader = root.join("reader");
            let remote = root.join("remote");
            std::fs::create_dir_all(&reader).unwrap();
            std::fs::create_dir_all(&remote).unwrap();
            let fixture = Self {
                root,
                reader,
                remote,
            };
            Self::git(&fixture.reader, &["init"]);
            Self::git(&fixture.remote, &["init"]);
            Self::git(&fixture.reader, &["config", "user.name", "Claims test"]);
            Self::git(
                &fixture.reader,
                &["config", "user.email", "claims@example.invalid"],
            );
            Self::git(
                &fixture.reader,
                &["remote", "add", "origin", fixture.remote.to_str().unwrap()],
            );
            fixture
        }

        fn git(root: &Path, arguments: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Claims test",
                    "-c",
                    "user.email=claims@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/nonexistent",
                ])
                .args(arguments)
                .current_dir(root)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }

        fn publish(&self, body: &str) -> String {
            Self::git(&self.remote, &["commit", "--allow-empty", "-m", body]);
            let version = Self::git(&self.remote, &["rev-parse", "HEAD"]);
            Self::git(&self.remote, &["update-ref", CLAIMS_REF, &version]);
            version
        }

        /// A record an older tool wrote to the legacy reference.
        fn publish_legacy(&self, body: &str) -> String {
            let tree = Self::git(&self.remote, &["hash-object", "-t", "tree", "/dev/null"]);
            let mut arguments = vec!["commit-tree", tree.as_str(), "-m", body];
            let previous = Self::git(
                &self.remote,
                &["for-each-ref", "--format=%(objectname)", LEGACY_CLAIMS_REF],
            );
            if !previous.is_empty() {
                arguments.extend(["-p", previous.as_str()]);
            }
            let version = Self::git(&self.remote, &arguments);
            Self::git(&self.remote, &["update-ref", LEGACY_CLAIMS_REF, &version]);
            version
        }

        fn remote_tip(&self, name: &str) -> String {
            Self::git(
                &self.remote,
                &["for-each-ref", "--format=%(objectname)", name],
            )
        }
    }

    impl Drop for RegistryFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn absent_registry_is_empty_but_unreadable_registry_is_an_error() {
        let fixture = RegistryFixture::new();
        let (version, record) = read_base(&fixture.reader).unwrap();
        assert!(version.is_empty());
        assert!(record.claims.is_empty());
        RegistryFixture::git(
            &fixture.reader,
            &[
                "remote",
                "set-url",
                "origin",
                fixture.root.join("missing").to_str().unwrap(),
            ],
        );
        assert!(read_base(&fixture.reader).is_err());
    }

    #[test]
    fn malformed_existing_registry_cannot_be_replaced_by_an_empty_snapshot() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish("interrupted registry payload");
        assert!(read_base(&fixture.reader).is_err());
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", CLAIMS_REF]),
            version
        );
    }

    #[test]
    fn unsupported_registry_protocol_cannot_be_rewritten() {
        let fixture = RegistryFixture::new();
        fixture.publish(r#"{"protocol":"omega-claims-future","claims":[],"notes":[]}"#);
        assert!(read_base(&fixture.reader).is_err());
    }

    #[test]
    fn advertised_snapshot_survives_a_concurrent_ref_update() {
        let fixture = RegistryFixture::new();
        let original =
            json!({"protocol": PROTOCOL, "claims": [{"ticket":"original"}], "notes": []});
        let advertised = fixture.publish(&original.to_string());
        let replacement =
            json!({"protocol": PROTOCOL, "claims": [{"ticket":"replacement"}], "notes": []});
        let newer = fixture.publish(&replacement.to_string());

        let record = fetch_record(&fixture.reader, &advertised).unwrap();
        assert_eq!(
            record.claims,
            original["claims"].as_array().unwrap().to_vec()
        );
        assert!(commit_and_push(&fixture.reader, &advertised, &record).is_err());
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", CLAIMS_REF]),
            newer
        );
        let (current, record) = read_base(&fixture.reader).unwrap();
        assert_eq!(current, newer);
        assert_eq!(
            record.claims,
            replacement["claims"].as_array().unwrap().to_vec()
        );
    }

    #[test]
    fn an_uncached_snapshot_uses_the_head_observed_after_fetch_admission() {
        let fixture = RegistryFixture::new();
        let observed = fixture.publish(
            &json!({"protocol": PROTOCOL,
            "claims": [{"ticket": "old"}], "notes": []})
            .to_string(),
        );
        assert_eq!(remote_heads(&fixture.reader).unwrap().branch, observed);
        assert!(!snapshot_is_cached(&fixture.reader, &observed).unwrap());
        let (latest, snapshot) = crate::repository::with_git_fetch(&fixture.reader, |fetch| {
            // Another writer may publish while this reader waits for admission.
            let latest = fixture.publish(
                &json!({"protocol": PROTOCOL,
                "claims": [{"ticket": "new", "unknown": [1, 2]}], "notes": []})
                .to_string(),
            );
            Ok((
                latest,
                fetch_snapshot_admitted(&fixture.reader, LegacyRead::Always, fetch)
                    .map(|snapshot| snapshot.write_base()),
            ))
        })
        .unwrap();
        let (version, record) = snapshot.unwrap();
        assert_eq!(version, latest);
        assert_ne!(version, observed);
        assert_eq!(record.claims, [json!({"ticket": "new", "unknown": [1, 2]})]);
        // A newer malformed head is a refusal, not a reason to reuse the old body.
        let snapshot = crate::repository::with_git_fetch(&fixture.reader, |fetch| {
            fixture.publish("malformed newer registry");
            Ok(
                fetch_snapshot_admitted(&fixture.reader, LegacyRead::Always, fetch)
                    .map(|snapshot| snapshot.write_base()),
            )
        })
        .unwrap();
        assert!(snapshot.is_err());
    }

    #[test]
    fn a_cached_snapshot_does_not_wait_for_fetch_admission() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        fetch_record(&fixture.reader, &version).unwrap();
        let snapshot = std::thread::scope(|threads| {
            crate::repository::with_git_fetch(&fixture.reader, |_| {
                let (send, receive) = std::sync::mpsc::channel();
                let reader = &fixture.reader;
                threads.spawn(move || {
                    let _ = send.send(read_base(reader));
                });
                Ok(receive.recv_timeout(Duration::from_secs(5)))
            })
            .unwrap()
        })
        .expect("cached reads finish while fetch admission is held")
        .unwrap();
        assert_eq!(snapshot.0, version);
        assert_eq!(snapshot.1, ClaimsRecord::default());
    }

    #[test]
    fn exact_cached_snapshot_needs_no_fetch_but_unknown_snapshot_still_refuses() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        fetch_record(&fixture.reader, &version).unwrap();
        RegistryFixture::git(
            &fixture.reader,
            &[
                "remote",
                "set-url",
                "origin",
                fixture.root.join("missing").to_str().unwrap(),
            ],
        );
        assert!(fetch_record(&fixture.reader, &version).is_ok());
        assert!(fetch_record(&fixture.reader, "1111111111111111111111111111111111111111").is_err());
        assert!(read_base(&fixture.reader).is_err());
    }

    #[test]
    fn concurrent_updates_reapply_without_losing_foreign_records() {
        let fixture = RegistryFixture::new();
        fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        let mut attempts = 0;
        update_registry(&fixture.reader, |record| {
            attempts += 1;
            if attempts == 1 {
                fixture.publish(
                    &json!({"protocol": PROTOCOL,
                        "claims": [{"ticket": "other", "unknown": [1, 2]}],
                        "notes": [{"ticket": "other", "text": "keep", "created_utc": "2026-11-14T00:00:00.123456789Z"}]
                    })
                    .to_string(),
                );
            }
            record.claims.push(json!({"ticket": "ours"}));
            Ok(())
        })
        .unwrap();
        assert_eq!(attempts, 2);
        let (_, record) = read_base(&fixture.reader).unwrap();
        assert_eq!(
            record.claims,
            vec![
                json!({"ticket": "other", "unknown": [1, 2]}),
                json!({"ticket": "ours"})
            ]
        );
        assert_eq!(
            record.notes[0]["created_utc"],
            "2026-11-14T00:00:00.123456789Z"
        );
    }

    #[test]
    fn an_unchanged_registry_is_not_published() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        update_registry(&fixture.reader, |_| Ok(())).unwrap();
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", CLAIMS_REF]),
            version
        );
    }

    #[test]
    fn replaying_a_claim_or_note_preserves_a_later_writers_changes() {
        let taken = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let (claimed_utc, expires_utc) = lease_timestamps(taken).unwrap();
        let claim = json!({"ticket": "ours", "item": "repair", "owner": "agent",
            "claimed_utc": claimed_utc, "expires_utc": expires_utc});
        let mut record = ClaimsRecord::default();
        append_claim(&mut record, &claim).unwrap();
        append_note(&mut record, "ours", "working", taken).unwrap();
        let later = taken + Duration::from_secs(3600);
        assert!(renew_claim(&mut record, "ours", later).unwrap());
        record
            .claims
            .push(json!({"ticket": "foreign", "unknown": {"keep": true}}));
        record.notes[0]["swept_utc"] = json!("later sweep");
        let before = record.clone();
        append_claim(&mut record, &claim).unwrap();
        append_note(&mut record, "ours", "working", taken).unwrap();
        assert_eq!(record, before);
        let mut different = claim.clone();
        different["item"] = json!("a different operation");
        assert!(append_claim(&mut record, &different).is_err());
        assert_eq!(record, before);
        // Another intentional note is distinct, even with identical text.
        append_note(&mut record, "ours", "working", later).unwrap();
        assert_eq!(record.notes.len(), 2);
    }

    #[test]
    fn local_preparation_errors_are_not_retried() {
        let fixture = RegistryFixture::new();
        fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        RegistryFixture::git(&fixture.reader, &["config", "user.name", ""]);
        let mut attempts = 0;
        assert!(
            update_registry(&fixture.reader, |record| {
                attempts += 1;
                record.claims.push(json!({"ticket": "ours"}));
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 1);
    }

    #[test]
    fn a_failed_push_without_a_competing_writer_is_not_retried() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish(&serde_json::to_string(&ClaimsRecord::default()).unwrap());
        RegistryFixture::git(
            &fixture.reader,
            &[
                "config",
                "remote.origin.pushurl",
                fixture.root.join("missing").to_str().unwrap(),
            ],
        );
        let mut attempts = 0;
        assert!(
            update_registry(&fixture.reader, |record| {
                attempts += 1;
                record.claims.push(json!({"ticket": "ours"}));
                Ok(())
            })
            .is_err()
        );
        assert_eq!(attempts, 1);
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", CLAIMS_REF]),
            version
        );
    }

    #[test]
    fn claim_lease_starts_at_the_supplied_clock_and_lasts_two_hours() {
        let now = UNIX_EPOCH + Duration::new(1_794_527_999, 123_456_700);
        let (claimed, expires) = lease_timestamps(now).unwrap();
        assert_eq!(claimed, "2026-11-12T23:59:59.123456700+00:00");
        assert_eq!(expires, "2026-11-13T01:59:59.123456700+00:00");
        assert_eq!(format_utc(now).unwrap(), claimed);
        assert_eq!(parse_expiry(&claimed), Some(epoch_nanoseconds(now)));
        assert_eq!(
            parse_expiry(&expires).unwrap() - epoch_nanoseconds(now),
            2 * 60 * 60 * NANOSECONDS_PER_SECOND
        );
    }

    #[test]
    fn renewal_restarts_the_lease_and_leaves_a_lapsed_claim_ended() {
        let hour = Duration::from_secs(3600);
        let taken = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let (claimed_utc, expires_utc) = lease_timestamps(taken).unwrap();
        let mut record = ClaimsRecord {
            claims: vec![
                json!({"ticket": "live", "claimed_utc": claimed_utc, "expires_utc": expires_utc}),
                json!({"ticket": "lapsed", "claimed_utc": claimed_utc, "expires_utc": expires_utc}),
            ],
            ..ClaimsRecord::default()
        };
        // An hour and a half in, renew one claim; it then outlives the first
        // lease and ends two hours after the renewal.
        let renewal = taken + hour + hour / 2;
        assert!(renew_claim(&mut record, "live", renewal).unwrap());
        let already_renewed = record.clone();
        assert!(renew_claim(&mut record, "live", taken + hour).unwrap());
        assert_eq!(record, already_renewed);
        let later = taken + 3 * hour;
        let visible = branch_status("v", &record, later)["claims"].clone();
        assert_eq!(visible.as_array().unwrap().len(), 1);
        assert_eq!(visible[0]["ticket"], "live");
        assert_eq!(
            claim_expiry(&record.claims[0]),
            Some(epoch_nanoseconds(renewal + 2 * hour))
        );
        // The unrenewed claim ended at two hours and stays ended.
        assert!(!renew_claim(&mut record, "lapsed", later).unwrap());
        assert!(!renew_claim(&mut record, "unknown", later).unwrap());
        assert_eq!(record.claims[1]["expires_utc"], expires_utc.as_str());
        // A renewal stamped later than a hand-edited expiry cannot extend it.
        let mut capped = json!({"ticket": "c", "claimed_utc": claimed_utc,
            "renewed_utc": format_utc(renewal).unwrap(), "expires_utc": claimed_utc});
        assert_eq!(claim_expiry(&capped), Some(epoch_nanoseconds(taken)));
        capped["expires_utc"] = "2099-01-01T00:00:00Z".into();
        assert_eq!(
            claim_expiry(&capped),
            Some(epoch_nanoseconds(renewal + 2 * hour))
        );
    }

    #[test]
    fn a_lapsed_claim_is_restored_from_its_saved_record_and_reports_overlap() {
        let hour = Duration::from_secs(3600);
        let taken = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let (claimed_utc, expires_utc) = lease_timestamps(taken).unwrap();
        let mine = json!({"ticket": "mine", "item": "I", "owner": "me",
            "paths": ["omega-rust/omega/compilation/native-compiler/"],
            "claimed_utc": claimed_utc, "expires_utc": expires_utc});
        let later = taken + 3 * hour;
        let (theirs_claimed, theirs_expires) = lease_timestamps(later - hour).unwrap();
        let mut record = ClaimsRecord {
            claims: vec![
                mine.clone(),
                json!({"ticket": "inside", "owner": "other", "item": "J",
                    "paths": ["omega-rust/omega/compilation/native-compiler/src/lib.rs"],
                    "claimed_utc": theirs_claimed, "expires_utc": theirs_expires}),
                json!({"ticket": "sibling", "paths": ["omega-rust/omega/compilation/native-compiler-old"],
                    "claimed_utc": theirs_claimed, "expires_utc": theirs_expires}),
                json!({"ticket": "ended", "paths": ["omega-rust"],
                    "claimed_utc": claimed_utc, "expires_utc": expires_utc}),
            ],
            ..ClaimsRecord::default()
        };
        assert!(!renew_claim(&mut record, "mine", later).unwrap());
        let overlaps = revive_claim(&mut record, &mine, later).unwrap();
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0]["ticket"], "inside");
        let restored = record
            .claims
            .iter()
            .filter(|claim| claim["ticket"] == "mine")
            .collect::<Vec<_>>();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0]["item"], "I");
        assert_eq!(restored[0]["claimed_utc"], claimed_utc.as_str());
        assert_eq!(
            claim_expiry(restored[0]),
            Some(epoch_nanoseconds(later + 2 * hour))
        );
        // The restored claim then renews like any live claim.
        assert!(renew_claim(&mut record, "mine", later + hour).unwrap());
    }

    #[test]
    fn held_claim_lines_read_saved_records_and_bare_tickets() {
        let saved = HeldClaim::parse(r#"{"ticket":"t1","item":"I"}"#).unwrap();
        assert_eq!(saved.ticket, "t1");
        assert_eq!(saved.claim.as_ref().unwrap()["item"], "I");
        assert_eq!(HeldClaim::parse(&saved.line()), Some(saved));
        let bare = HeldClaim::parse(" t2 ").unwrap();
        assert_eq!((bare.ticket.as_str(), bare.claim), ("t2", None));
        assert_eq!(HeldClaim::parse("  "), None);
        assert!(paths_overlap("a/b/", "a/b"));
        assert!(paths_overlap("a", "a/b/c"));
        assert!(!paths_overlap("a/b", "a/bc"));
    }

    #[test]
    fn observed_clock_retains_signed_epoch_nanoseconds() {
        assert_eq!(epoch_nanoseconds(UNIX_EPOCH), 0);
        let elapsed = Duration::new(1, 100);
        assert_eq!(epoch_nanoseconds(UNIX_EPOCH + elapsed), 1_000_000_100);
        assert_eq!(epoch_nanoseconds(UNIX_EPOCH - elapsed), -1_000_000_100);
    }

    #[test]
    fn utc_clock_handles_calendar_boundaries_and_reports_unrepresentable_times() {
        for (seconds, text) in [
            (0, "1970-01-01T00:00:00+00:00"),
            (946_684_799, "1999-12-31T23:59:59+00:00"),
            (951_782_400, "2000-02-29T00:00:00+00:00"),
            (4_107_542_400, "2100-03-01T00:00:00+00:00"),
            (253_402_300_799, "9999-12-31T23:59:59+00:00"),
        ] {
            let time = UNIX_EPOCH + Duration::from_secs(seconds);
            assert_eq!(format_utc(time).unwrap(), text);
            assert_eq!(parse_expiry(text), Some(epoch_nanoseconds(time)));
        }
        assert!(format_utc(UNIX_EPOCH - Duration::from_secs(1)).is_err());
        assert!(format_utc(UNIX_EPOCH + Duration::from_secs(253_402_300_800)).is_err());
        assert!(lease_timestamps(UNIX_EPOCH + Duration::from_secs(253_402_300_799)).is_err());
    }

    #[test]
    fn expiry_compares_instants_with_offsets_and_fractional_seconds() {
        for text in [
            "1970-01-01T00:00:00Z",
            "1970-01-01t00:00:00.000000000z",
            "1970-01-01T01:00:00+01:00",
            "1969-12-31T19:00:00-05:00",
        ] {
            assert_eq!(parse_expiry(text), Some(0), "{text}");
        }
        assert_eq!(parse_expiry("1969-12-31T23:59:59.5Z"), Some(-500_000_000));
        for (text, nanos) in [
            ("1970-01-01T00:00:00.1Z", 100_000_000),
            ("1970-01-01T00:00:00.0000000001Z", 1),
            ("1970-01-01T00:00:00.9999999999Z", 1_000_000_000),
            ("1970-01-01T00:00:00.123456789000Z", 123_456_789),
        ] {
            assert_eq!(parse_expiry(text), Some(nanos), "{text}");
        }
    }

    #[test]
    fn status_hides_expired_claims_at_the_exact_boundary_without_rewriting_history() {
        let now = UNIX_EPOCH + Duration::new(1_794_527_999, 123_456_700);
        // The observed clock fits Windows' 100ns storage. Authored expiry
        // strings must retain finer precision without entering that storage.
        let claims: Vec<_> = [
            ("before", "2026-11-12T23:59:59.123456699Z"),
            ("equal", "2026-11-12T23:59:59.123456700Z"),
            ("after", "2026-11-12T23:59:59.123456701Z"),
            ("finer", "2026-11-12T23:59:59.1234567001Z"),
        ]
        .into_iter()
        .map(|(ticket, expiry)| json!({"ticket": ticket, "expires_utc": expiry}))
        .collect();
        let mut record = ClaimsRecord {
            claims: claims.clone(),
            notes: vec![json!({"text": "retained note"})],
            ..ClaimsRecord::default()
        };
        let out = branch_status("snapshot", &record, now);
        assert_eq!(out["state"], "claimed");
        assert_eq!(out["claims"], json!([claims[2], claims[3]]));
        assert_eq!(out["version"], "snapshot");
        assert_eq!(out["notes_pending"], 1);
        assert_eq!(record.claims, claims);

        record.claims.truncate(2);
        let out = branch_status("snapshot", &record, now);
        assert_eq!(out["state"], "available");
        assert_eq!(out["claims"], json!([]));
        assert_eq!(record.claims.len(), 2);
    }

    #[test]
    fn status_keeps_unknown_expiry_and_caps_far_future_expiry_at_one_lease() {
        let unknown = vec![
            json!({"ticket": "missing", "legacy_field": [1, 2]}),
            json!({"ticket": "null", "expires_utc": null}),
            json!({"ticket": "number", "expires_utc": 42}),
            json!({"ticket": "malformed", "expires_utc": "2026-02-30T00:00:00Z"}),
        ];
        let mut claims = unknown.clone();
        // Taken 2026-11-07 with a 2099 expiry: its lease ended that morning.
        claims.push(json!({
            "ticket": "historical", "lease_minutes": 480,
            "claimed_utc": "2026-11-07T00:00:00+00:00",
            "expires_utc": "2099-01-01T00:00:00+00:00",
            "unknown_field": true
        }));
        let wire = json!({"protocol": PROTOCOL, "claims": claims, "notes": []});
        let record: ClaimsRecord = serde_json::from_value(wire.clone()).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let out = branch_status("snapshot", &record, now);
        assert_eq!(out["state"], "claimed");
        assert_eq!(out["claims"], json!(unknown));
        assert_eq!(serde_json::to_value(record).unwrap(), wire);
        assert_eq!(
            branch_status("empty", &ClaimsRecord::default(), now)["state"],
            "available"
        );
    }

    #[test]
    fn a_recorded_expiry_shortens_a_lease_but_never_extends_it() {
        let (claimed, expires) = lease_timestamps(UNIX_EPOCH + Duration::from_secs(1_000)).unwrap();
        let lease = i128::from(LEASE_MINUTES * 60) * NANOSECONDS_PER_SECOND;
        let start = parse_expiry(&claimed).unwrap();
        for (recorded, ends) in [
            (expires.as_str(), start + lease),
            ("2099-01-01T00:00:00Z", start + lease),
            ("1970-01-01T00:20:00Z", 1_200 * NANOSECONDS_PER_SECOND),
        ] {
            let claim = json!({"claimed_utc": claimed, "expires_utc": recorded});
            assert_eq!(claim_expiry(&claim), Some(ends), "{recorded}");
        }
        // Without a recorded expiry the lease still ends; without either the
        // claim stays visible.
        assert_eq!(
            claim_expiry(&json!({"claimed_utc": claimed})),
            Some(start + lease)
        );
        assert_eq!(claim_expiry(&json!({"claimed_utc": "soon"})), None);
    }

    #[test]
    fn a_write_drops_ended_claims_and_notes_without_a_visible_claim() {
        let now = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let (claimed_utc, expires_utc) = lease_timestamps(now).unwrap();
        let note = |ticket: Option<&str>| match ticket {
            Some(ticket) => json!({"ticket": ticket, "text": "milestone"}),
            None => json!({"text": "unowned"}),
        };
        let mut record = ClaimsRecord {
            claims: vec![
                json!({"ticket": "live", "claimed_utc": claimed_utc, "expires_utc": expires_utc}),
                json!({"ticket": "ended", "expires_utc": "2026-11-01T00:00:00Z"}),
                json!({"ticket": "zombie", "claimed_utc": "2026-11-07T00:00:00Z",
                    "expires_utc": "2099-01-01T00:00:00Z"}),
                json!({"ticket": "unreadable", "expires_utc": "soon"}),
            ],
            notes: vec![
                note(Some("live")),
                note(Some("ended")),
                note(Some("zombie")),
                note(Some("released")),
                note(Some("unreadable")),
                note(None),
            ],
            ..ClaimsRecord::default()
        };
        let visible = branch_status("v", &record, now)["claims"].clone();
        prune_registry(&mut record, now);
        // The stored record now holds exactly what status shows.
        assert_eq!(json!(record.claims), visible);
        assert_eq!(
            record.notes,
            vec![note(Some("live")), note(Some("unreadable"))]
        );
    }

    #[test]
    fn malformed_expiry_cannot_prove_a_claim_inactive() {
        for text in [
            "",
            "2000-01-01",
            "2000-01-01T00:00:00",
            "2000-01-01 00:00:00Z",
            "2000/01/01T00:00:00Z",
            "0000-01-01T00:00:00Z",
            "2000-00-01T00:00:00Z",
            "2000-13-01T00:00:00Z",
            "2000-01-00T00:00:00Z",
            "2000-04-31T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2000-01-01T24:00:00Z",
            "2000-01-01T00:60:00Z",
            "2000-01-01T00:00:60Z",
            "2000-01-01T00:00:00.Z",
            "2000-01-01T00:00:00.1xZ",
            "2000-01-01T00:00:00+24:00",
            "2000-01-01T00:00:00+00:60",
            "2000-01-01T00:00:00+0000",
            "2000-01-01T00:00:00Z12:34",
            "2000-01-01T00:00:00Z trailing",
            "2000-01-01T00:00:00é",
        ] {
            assert_eq!(parse_expiry(text), None, "{text}");
            let record = ClaimsRecord {
                claims: vec![json!({"expires_utc": text})],
                ..ClaimsRecord::default()
            };
            let out = branch_status(
                "snapshot",
                &record,
                UNIX_EPOCH + Duration::from_secs(1_794_527_999),
            );
            assert_eq!(out["claims"].as_array().unwrap().len(), 1, "{text}");
        }
    }

    #[test]
    fn the_first_write_seeds_the_branch_from_the_legacy_registry() {
        let fixture = RegistryFixture::new();
        let legacy = fixture.publish_legacy(
            &json!({"protocol": PROTOCOL,
                "claims": [{"ticket": "old", "unknown": true}],
                "notes": [{"ticket": "old", "text": "kept"}]})
            .to_string(),
        );
        update_registry(&fixture.reader, |record| {
            record.claims.push(json!({"ticket": "ours"}));
            Ok(())
        })
        .unwrap();
        let branch = fixture.remote_tip(CLAIMS_REF);
        // The branch continues the legacy history; the legacy ref is untouched.
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", &format!("{branch}^")]),
            legacy
        );
        assert_eq!(fixture.remote_tip(LEGACY_CLAIMS_REF), legacy);
        let (version, record) = read_base(&fixture.reader).unwrap();
        assert_eq!(version, branch);
        assert_eq!(
            record.claims,
            [
                json!({"ticket": "old", "unknown": true}),
                json!({"ticket": "ours"})
            ]
        );
        assert_eq!(record.notes.len(), 1);
        assert_eq!(record.seeded_tickets, ["old"]);

        // A later write fast-forwards the branch: the old tip is its parent.
        update_registry(&fixture.reader, |record| {
            record.claims.retain(|claim| claim["ticket"] != "ours");
            Ok(())
        })
        .unwrap();
        let next = fixture.remote_tip(CLAIMS_REF);
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", &format!("{next}^")]),
            branch
        );
    }

    #[test]
    fn status_merges_claims_older_tools_still_write_to_the_legacy_registry() {
        let fixture = RegistryFixture::new();
        fixture.publish_legacy(
            &json!({"protocol": PROTOCOL, "claims": [{"ticket": "seeded"}], "notes": []})
                .to_string(),
        );
        update_registry(&fixture.reader, |record| {
            record.claims.push(json!({"ticket": "ours"}));
            Ok(())
        })
        .unwrap();
        // An older tool keeps writing the legacy ref after the branch exists.
        let legacy = fixture.publish_legacy(
            &json!({"protocol": PROTOCOL,
                "claims": [{"ticket": "seeded"}, {"ticket": "late"}],
                "notes": [{"ticket": "late", "text": "old tool"}]})
            .to_string(),
        );
        // The branch releases the claim it took over from the legacy ref.
        update_registry(&fixture.reader, |record| {
            record.claims.retain(|claim| claim["ticket"] != "seeded");
            Ok(())
        })
        .unwrap();
        let snapshot = fetch_snapshot(&fixture.reader, LegacyRead::Always).unwrap();
        let out = status_snapshot(&snapshot, SystemTime::now());
        assert_eq!(
            out["claims"],
            json!([{"ticket": "ours"}, {"ticket": "late", "registry": LEGACY_CLAIMS_REF}])
        );
        assert_eq!(out["notes_pending"], 1);
        assert_eq!(out["version"], fixture.remote_tip(CLAIMS_REF));
        assert_eq!(out["legacy_version"], legacy);
        // A write never reads or rewrites the legacy ref once the branch exists.
        assert_eq!(fixture.remote_tip(LEGACY_CLAIMS_REF), legacy);
    }
}
