//! Cross-machine work-claim registry (`cargo xtask claims`) on `refs/coordination/omega-claims/main`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::repo_root;

const CLAIMS_REF: &str = "refs/coordination/omega-claims/main";
const PROTOCOL: &str = "omega-claims-v1";
const DEFAULT_LEASE_MINUTES: u64 = 480;
const NANOSECONDS_PER_SECOND: i128 = 1_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaimsRecord {
    protocol: String,
    #[serde(default)]
    claims: Vec<serde_json::Value>,
    #[serde(default)]
    notes: Vec<serde_json::Value>,
}

impl Default for ClaimsRecord {
    fn default() -> Self {
        Self {
            protocol: PROTOCOL.to_owned(),
            claims: Vec::new(),
            notes: Vec::new(),
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
        .checked_add(Duration::from_secs(DEFAULT_LEASE_MINUTES * 60))
        .ok_or("claim expiry exceeds the system clock range")?;
    Ok((format_utc(now)?, format_utc(expires)?))
}

fn status_snapshot(version: &str, record: &ClaimsRecord, now: SystemTime) -> serde_json::Value {
    let now = epoch_nanoseconds(now);
    let claims: Vec<_> = record
        .claims
        .iter()
        .filter(|claim| {
            claim
                .get("expires_utc")
                .and_then(serde_json::Value::as_str)
                .and_then(parse_expiry)
                .is_none_or(|expires| expires > now)
        })
        .collect();
    serde_json::json!({
        "state": if claims.is_empty() { "available" } else { "claimed" },
        "claims": claims,
        "notes_pending": record.notes.len(),
        "version": version,
    })
}

fn clock_error(error: String) -> u8 {
    eprintln!("claims: {error}");
    1
}

fn fetch_snapshot(repo: &Path) -> Result<(String, ClaimsRecord), u8> {
    let ls = Command::new("git")
        .args(["ls-remote", "origin", CLAIMS_REF])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            eprintln!("claims: git ls-remote failed: {e}");
            1u8
        })?;
    if !ls.status.success() {
        eprintln!("claims: cannot read the remote registry reference");
        return Err(1);
    }
    let stdout = String::from_utf8_lossy(&ls.stdout);
    let version = stdout
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if version.is_empty() {
        return Ok((String::new(), ClaimsRecord::default()));
    }
    let record = fetch_record(repo, &version)?;
    Ok((version, record))
}

fn fetch_record(repo: &Path, version: &str) -> Result<ClaimsRecord, u8> {
    // Fetch the exact advertised snapshot: another writer may move the ref
    // between ls-remote and fetch. Reading a different or empty record under
    // the advertised lease could overwrite other sessions' claims.
    // Only the newest record matters: each commit's message is the whole
    // registry, and the ref's history is tens of thousands of such commits,
    // so a full fetch can exhaust a session's memory.
    let fetch = crate::repository::with_git_fetch_lock(repo, || {
        Command::new("git")
            .args(["fetch", "-q", "--depth=1", "origin", version])
            .current_dir(repo)
            .stdin(Stdio::null())
            .status()
    })
    .map_err(|error| {
        eprintln!("claims: {error}");
        1u8
    })?;
    if !fetch.success() {
        eprintln!("claims: cannot fetch registry snapshot {version}");
        return Err(1);
    }
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

fn commit_and_push(repo: &Path, prev_version: &str, record: &ClaimsRecord) -> Result<(), u8> {
    let empty_tree = Command::new("git")
        .args(["hash-object", "-t", "tree", "--stdin"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| 1u8)?;
    let tree_sha = String::from_utf8_lossy(&empty_tree.stdout)
        .trim()
        .to_owned();
    let body = serde_json::to_string(record).map_err(|_| 1u8)?;

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
        .map_err(|_| 1u8)?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(body.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| 1u8)?;
    if !out.status.success() {
        eprintln!("claims: git commit-tree failed");
        return Err(1);
    }
    let new_sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let lease = format!("--force-with-lease={CLAIMS_REF}:{prev_version}");
    let target = format!("{new_sha}:{CLAIMS_REF}");
    let push = Command::new("git")
        .args(["push", "origin", &lease, &target])
        .current_dir(repo)
        .stdin(Stdio::null())
        .status()
        .map_err(|_| 1u8)?;
    if !push.success() {
        eprintln!("claims: push to {CLAIMS_REF} failed (concurrent writer or network error)");
        return Err(1);
    }
    Ok(())
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

pub fn run(args: &[String]) -> Result<(), u8> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        println!(
            "usage: cargo xtask claims <status|claim|release|note> [--board <file>] [--item <id>] [--owner <name>] [--path <paths>] [--ticket <id>] [--text <note>]"
        );
        return Ok(());
    }
    let repo = repo_root()?;
    let cmd = args[0].as_str();
    let (version, mut record) = fetch_snapshot(&repo)?;

    match cmd {
        "status" => {
            let out = status_snapshot(&version, &record, SystemTime::now());
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            Ok(())
        }
        "claim" => {
            let mut board: Option<String> = None;
            let mut item = String::new();
            let mut owner = String::from("agent");
            let mut paths: Vec<String> = Vec::new();
            let mut note: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--board" => {
                        i += 1;
                        board = args.get(i).cloned();
                    }
                    "--item" => {
                        i += 1;
                        item = args.get(i).cloned().unwrap_or_default();
                    }
                    "--owner" => {
                        i += 1;
                        owner = args.get(i).cloned().unwrap_or_default();
                    }
                    "--path" => {
                        i += 1;
                        if let Some(p) = args.get(i) {
                            paths.extend(
                                p.split(',')
                                    .flat_map(str::split_whitespace)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned),
                            );
                        }
                    }
                    "--note" => {
                        i += 1;
                        note = args.get(i).cloned();
                    }
                    _ => {}
                }
                i += 1;
            }
            if item.is_empty() {
                eprintln!("claims: --item is required for `claim`");
                return Err(2);
            }
            let ticket = generate_ticket_id();
            let (claimed_utc, expires_utc) =
                lease_timestamps(SystemTime::now()).map_err(clock_error)?;
            let entry = serde_json::json!({
                "ticket": ticket,
                "item": item,
                "board": board,
                "owner": owner,
                "paths": paths,
                "note": note,
                "lease_minutes": DEFAULT_LEASE_MINUTES,
                "claimed_utc": claimed_utc,
                "expires_utc": expires_utc,
            });
            record.claims.push(entry);
            commit_and_push(&repo, &version, &record)?;
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
        "release" => {
            let mut ticket = String::new();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--ticket" {
                    i += 1;
                    ticket = args.get(i).cloned().unwrap_or_default();
                }
                i += 1;
            }
            if ticket.is_empty() {
                eprintln!("claims: --ticket is required for `release`");
                return Err(2);
            }
            record
                .claims
                .retain(|c| c.get("ticket").and_then(|v| v.as_str()) != Some(ticket.as_str()));
            commit_and_push(&repo, &version, &record)?;
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
        "note" => {
            let mut ticket = String::new();
            let mut text = String::new();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--ticket" => {
                        i += 1;
                        ticket = args.get(i).cloned().unwrap_or_default();
                    }
                    "--text" => {
                        i += 1;
                        text = args.get(i).cloned().unwrap_or_default();
                    }
                    _ => {}
                }
                i += 1;
            }
            if ticket.is_empty() || text.is_empty() {
                eprintln!("claims: --ticket and --text are required for `note`");
                return Err(2);
            }
            let created_utc = format_utc(SystemTime::now()).map_err(clock_error)?;
            record.notes.push(serde_json::json!({
                "ticket": ticket,
                "item": "note",
                "owner": "agent",
                "text": text,
                "created_utc": created_utc,
                "swept_utc": null,
            }));
            commit_and_push(&repo, &version, &record)?;
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
        other => {
            eprintln!("claims: unknown subcommand `{other}`");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    }

    impl Drop for RegistryFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn absent_registry_is_empty_but_unreadable_registry_is_an_error() {
        let fixture = RegistryFixture::new();
        let (version, record) = fetch_snapshot(&fixture.reader).unwrap();
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
        assert!(fetch_snapshot(&fixture.reader).is_err());
    }

    #[test]
    fn malformed_existing_registry_cannot_be_replaced_by_an_empty_snapshot() {
        let fixture = RegistryFixture::new();
        let version = fixture.publish("interrupted registry payload");
        assert!(fetch_snapshot(&fixture.reader).is_err());
        assert_eq!(
            RegistryFixture::git(&fixture.remote, &["rev-parse", CLAIMS_REF]),
            version
        );
    }

    #[test]
    fn unsupported_registry_protocol_cannot_be_rewritten() {
        let fixture = RegistryFixture::new();
        fixture.publish(r#"{"protocol":"omega-claims-future","claims":[],"notes":[]}"#);
        assert!(fetch_snapshot(&fixture.reader).is_err());
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
        let (current, record) = fetch_snapshot(&fixture.reader).unwrap();
        assert_eq!(current, newer);
        assert_eq!(
            record.claims,
            replacement["claims"].as_array().unwrap().to_vec()
        );
    }

    #[test]
    fn missing_advertised_snapshot_refuses_even_if_a_cached_record_exists() {
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
        assert!(fetch_record(&fixture.reader, &version).is_err());
    }

    #[test]
    fn claim_lease_starts_at_the_supplied_clock_and_lasts_eight_hours() {
        let now = UNIX_EPOCH + Duration::new(1_794_527_999, 123_456_700);
        let (claimed, expires) = lease_timestamps(now).unwrap();
        assert_eq!(claimed, "2026-11-12T23:59:59.123456700+00:00");
        assert_eq!(expires, "2026-11-13T07:59:59.123456700+00:00");
        assert_eq!(format_utc(now).unwrap(), claimed);
        assert_eq!(parse_expiry(&claimed), Some(epoch_nanoseconds(now)));
        assert_eq!(
            parse_expiry(&expires).unwrap() - epoch_nanoseconds(now),
            8 * 60 * 60 * NANOSECONDS_PER_SECOND
        );
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
        let out = status_snapshot("snapshot", &record, now);
        assert_eq!(out["state"], "claimed");
        assert_eq!(out["claims"], json!([claims[2], claims[3]]));
        assert_eq!(out["version"], "snapshot");
        assert_eq!(out["notes_pending"], 1);
        assert_eq!(record.claims, claims);

        record.claims.truncate(2);
        let out = status_snapshot("snapshot", &record, now);
        assert_eq!(out["state"], "available");
        assert_eq!(out["claims"], json!([]));
        assert_eq!(record.claims.len(), 2);
    }

    #[test]
    fn status_keeps_unknown_expiry_and_honors_historical_far_future_expiry() {
        let claims = vec![
            json!({"ticket": "missing", "legacy_field": [1, 2]}),
            json!({"ticket": "null", "expires_utc": null}),
            json!({"ticket": "number", "expires_utc": 42}),
            json!({"ticket": "malformed", "expires_utc": "2026-02-30T00:00:00Z"}),
            json!({
                "ticket": "historical", "lease_minutes": 480,
                "claimed_utc": "2026-11-07T00:00:00+00:00",
                "expires_utc": "2099-01-01T00:00:00+00:00",
                "unknown_field": true
            }),
        ];
        let wire = json!({"protocol": PROTOCOL, "claims": claims, "notes": []});
        let record: ClaimsRecord = serde_json::from_value(wire.clone()).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1_794_527_999);
        let out = status_snapshot("snapshot", &record, now);
        assert_eq!(out["state"], "claimed");
        assert_eq!(out["claims"], json!(claims));
        assert_eq!(serde_json::to_value(record).unwrap(), wire);
        assert_eq!(
            status_snapshot("empty", &ClaimsRecord::default(), now)["state"],
            "available"
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
            let out = status_snapshot(
                "snapshot",
                &record,
                UNIX_EPOCH + Duration::from_secs(1_794_527_999),
            );
            assert_eq!(out["claims"].as_array().unwrap().len(), 1, "{text}");
        }
    }
}
