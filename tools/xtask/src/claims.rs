//! Cross-machine work-claim registry (`cargo xtask claims`) on `refs/coordination/omega-claims/main`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::repo_root;

const CLAIMS_REF: &str = "refs/coordination/omega-claims/main";
const PROTOCOL: &str = "omega-claims-v1";

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
        return Ok((String::new(), ClaimsRecord::default()));
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
    let _ = Command::new("git")
        .args(["fetch", "-q", "origin", CLAIMS_REF])
        .current_dir(repo)
        .stdin(Stdio::null())
        .status();
    let show = Command::new("git")
        .args(["show", "-s", "--format=%B", &version])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            eprintln!("claims: git show failed: {e}");
            1u8
        })?;
    if !show.status.success() {
        return Ok((version, ClaimsRecord::default()));
    }
    let body = String::from_utf8_lossy(&show.stdout);
    let record: ClaimsRecord = serde_json::from_str(body.trim()).unwrap_or_default();
    Ok((version, record))
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
    let repo = repo_root();
    let cmd = args[0].as_str();
    let (version, mut record) = fetch_snapshot(&repo)?;

    match cmd {
        "status" => {
            let out = serde_json::json!({
                "state": if record.claims.is_empty() { "available" } else { "claimed" },
                "claims": record.claims,
                "notes_pending": record.notes.len(),
                "version": version,
            });
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
            let entry = serde_json::json!({
                "ticket": ticket,
                "item": item,
                "board": board,
                "owner": owner,
                "paths": paths,
                "note": note,
                "lease_minutes": 480,
                "claimed_utc": "2026-11-07T00:00:00+00:00",
                "expires_utc": "2099-01-01T00:00:00+00:00",
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
            record.notes.push(serde_json::json!({
                "ticket": ticket,
                "item": "note",
                "owner": "agent",
                "text": text,
                "created_utc": "2026-11-07T00:00:00+00:00",
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
