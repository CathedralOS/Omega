//! Compute session model-audit provenance from local agent transcripts (`cargo xtask provenance`).
//!
//! Emits the canonical single-line commit/PR trailer:
//! `Agent: <name> | Model: <model> | Effort: <effort> | Tokens: <in> in (<cached> cached), <out> out | Steps: <N> | Time: <duration> | Steers: <N>`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde_json::Value;

use crate::repo_root;

#[derive(Debug, Default)]
struct SessionMetrics {
    agent: String,
    model: String,
    effort: String,
    input_tokens: u64,
    cached_tokens: u64,
    output_tokens: u64,
    steps: u64,
    active_seconds: u64,
    steers: u64,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.2}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn format_duration(seconds: u64) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    if mins >= 60 {
        let hours = mins / 60;
        let rem_mins = mins % 60;
        format!("{hours}h{rem_mins:02}m")
    } else if mins > 0 {
        format!("{mins}m{secs:02}s")
    } else {
        format!("{}s", secs.max(1))
    }
}

/// Parse an ISO-8601 UTC/offset timestamp (`YYYY-MM-DDTHH:MM:SS...`) to epoch seconds.
fn parse_iso_epoch(ts: &str) -> Option<i64> {
    if ts.len() < 19 {
        return None;
    }
    let year: i64 = ts.get(0..4)?.parse().ok()?;
    let month: i64 = ts.get(5..7)?.parse().ok()?;
    let day: i64 = ts.get(8..10)?.parse().ok()?;
    let hour: i64 = ts.get(11..13)?.parse().ok()?;
    let min: i64 = ts.get(14..16)?.parse().ok()?;
    let sec: i64 = ts.get(17..19)?.parse().ok()?;

    // Civil-from-days (Howard Hinnant algorithm)
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut epoch = days * 86400 + hour * 3600 + min * 60 + sec;

    let tail = &ts[19..];
    if let Some(pos) = tail.rfind(['+', '-'])
        && pos > 0
        && let Some(off) = tail.get(pos..)
        && off.len() >= 6
    {
        let sign = if off.starts_with('+') { 1i64 } else { -1i64 };
        let oh: i64 = off.get(1..3).and_then(|s| s.parse().ok()).unwrap_or(0);
        let om: i64 = off.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);
        epoch -= sign * (oh * 3600 + om * 60);
    }
    Some(epoch)
}

fn latest_origin_main_epoch(root: &Path) -> Option<i64> {
    let out = Command::new("git")
        .args(["log", "-1", "--format=%ct", "origin/main"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

fn find_latest_antigravity_transcript(home: &Path) -> Option<(PathBuf, SystemTime)> {
    let brain = home.join(".gemini/antigravity/brain");
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(brain).ok()?.flatten() {
        let candidate = entry.path().join(".system_generated/logs/transcript.jsonl");
        if let Ok(meta) = fs::metadata(&candidate)
            && let Ok(modified) = meta.modified()
            && newest.as_ref().is_none_or(|(_, t)| modified > *t)
        {
            newest = Some((candidate, modified));
        }
    }
    newest
}

fn find_latest_claude_transcript(home: &Path) -> Option<(PathBuf, SystemTime)> {
    let projects = home.join(".claude/projects");
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for dir in fs::read_dir(projects).ok()?.flatten() {
        let Ok(entries) = fs::read_dir(dir.path()) else {
            continue;
        };
        for file in entries.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
                && let Ok(meta) = fs::metadata(&path)
                && let Ok(modified) = meta.modified()
                && newest.as_ref().is_none_or(|(_, t)| modified > *t)
            {
                newest = Some((path, modified));
            }
        }
    }
    newest
}

fn normalize_antigravity_model(raw: &str) -> (String, String) {
    let mut effort = "high".to_owned();
    let mut base = raw.trim().to_owned();
    if let Some((prefix, rest)) = base.rsplit_once('(')
        && let Some(level) = rest.strip_suffix(')')
    {
        effort = level.trim().to_ascii_lowercase();
        base = prefix.trim().to_owned();
    }
    (base, effort)
}

fn parse_antigravity_transcript(path: &Path, since_epoch: Option<i64>) -> Option<SessionMetrics> {
    let content = fs::read_to_string(path).ok()?;
    let mut metrics = SessionMetrics {
        agent: "Antigravity".to_owned(),
        model: "Gemini 3.1 Pro".to_owned(),
        effort: "high".to_owned(),
        ..SessionMetrics::default()
    };

    let mut window_user_inputs = 0u64;
    let mut prev_ts: Option<i64> = None;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(text) = val["content"].as_str()
            && let Some(idx) = text.find("setting `Model Selection` from ")
        {
            let after = &text[idx..];
            if let Some(to_idx) = after.find(" to ") {
                let rest = &after[to_idx + 4..];
                if let Some(period) = rest.find('.') {
                    let (m, e) = normalize_antigravity_model(&rest[..period]);
                    metrics.model = m;
                    metrics.effort = e;
                }
            }
        }

        let step_ts = val["created_at"].as_str().and_then(parse_iso_epoch);
        if let (Some(cutoff), Some(ts)) = (since_epoch, step_ts)
            && ts < cutoff
        {
            continue;
        }

        if let Some(ts) = step_ts {
            if let Some(prev) = prev_ts {
                let delta = (ts - prev).max(0) as u64;
                // Cap single-step gaps at 5 minutes so idle pauses between tasks don't inflate active time.
                metrics.active_seconds += delta.min(300);
            }
            prev_ts = Some(ts);
        }

        match val["type"].as_str() {
            Some("USER_INPUT") => {
                window_user_inputs += 1;
            }
            Some("PLANNER_RESPONSE") => {
                metrics.steps += 1;
                let uncached = val["input_tokens"].as_u64().unwrap_or(0);
                let cached = val["cache_read_tokens"].as_u64().unwrap_or(0);
                let out = val["output_tokens"].as_u64().unwrap_or(0);
                metrics.input_tokens += uncached + cached;
                metrics.cached_tokens += cached;
                metrics.output_tokens += out;
            }
            _ => {}
        }
    }

    // If `since_epoch` filtered out all steps (e.g. nothing since the last commit), retry without cutoff.
    if metrics.steps == 0 && since_epoch.is_some() {
        return parse_antigravity_transcript(path, None);
    }

    metrics.steers = window_user_inputs.saturating_sub(1);
    Some(metrics)
}

fn parse_claude_transcript(path: &Path, since_epoch: Option<i64>) -> Option<SessionMetrics> {
    let content = fs::read_to_string(path).ok()?;
    let mut metrics = SessionMetrics {
        agent: "Claude Code".to_owned(),
        model: "claude-opus-4-6".to_owned(),
        effort: "high".to_owned(),
        ..SessionMetrics::default()
    };
    let mut window_user_inputs = 0u64;
    let mut prev_ts: Option<i64> = None;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let step_ts = val["timestamp"].as_str().and_then(parse_iso_epoch);
        if let (Some(cutoff), Some(ts)) = (since_epoch, step_ts)
            && ts < cutoff
        {
            continue;
        }
        if let Some(ts) = step_ts {
            if let Some(prev) = prev_ts {
                let delta = (ts - prev).max(0) as u64;
                metrics.active_seconds += delta.min(300);
            }
            prev_ts = Some(ts);
        }
        match val["type"].as_str() {
            Some("user") => {
                window_user_inputs += 1;
            }
            Some("assistant") => {
                let msg = &val["message"];
                if let Some(m) = msg["model"].as_str() {
                    metrics.model = m.to_owned();
                }
                let usage = &msg["usage"];
                let uncached = usage["input_tokens"].as_u64().unwrap_or(0)
                    + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                let cached = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                let out = usage["output_tokens"].as_u64().unwrap_or(0);
                if uncached + cached + out > 0 {
                    metrics.steps += 1;
                    metrics.input_tokens += uncached + cached;
                    metrics.cached_tokens += cached;
                    metrics.output_tokens += out;
                }
            }
            _ => {}
        }
    }

    if metrics.steps == 0 && since_epoch.is_some() {
        return parse_claude_transcript(path, None);
    }

    metrics.steers = window_user_inputs.saturating_sub(1);
    Some(metrics)
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut agent_override: Option<String> = None;
    let mut model_override: Option<String> = None;
    let mut effort_override: Option<String> = None;
    let mut transcript_override: Option<PathBuf> = None;
    let mut full_session = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--agent" => {
                i += 1;
                agent_override = args.get(i).cloned();
            }
            "--model" => {
                i += 1;
                model_override = args.get(i).cloned();
            }
            "--effort" => {
                i += 1;
                effort_override = args.get(i).cloned();
            }
            "--transcript" => {
                i += 1;
                transcript_override = args.get(i).map(PathBuf::from);
            }
            "--full-session" => {
                full_session = true;
            }
            "-h" | "--help" => {
                println!(
                    "usage: cargo xtask provenance [--agent <name>] [--model <model>] [--effort <effort>] [--transcript <path>] [--full-session]"
                );
                return Ok(());
            }
            other => {
                eprintln!("provenance: unknown argument `{other}`");
                return Err(2);
            }
        }
        i += 1;
    }

    let root = repo_root();
    let since_epoch = if full_session {
        None
    } else {
        latest_origin_main_epoch(&root)
    };

    let mut metrics = if let Some(path) = transcript_override {
        parse_antigravity_transcript(&path, since_epoch)
            .or_else(|| parse_claude_transcript(&path, since_epoch))
            .unwrap_or_default()
    } else if let Some(home) = home_dir() {
        let agy = find_latest_antigravity_transcript(&home);
        let claude = find_latest_claude_transcript(&home);
        match (agy, claude) {
            (Some((ap, at)), Some((cp, ct))) => {
                if at >= ct {
                    parse_antigravity_transcript(&ap, since_epoch).unwrap_or_default()
                } else {
                    parse_claude_transcript(&cp, since_epoch).unwrap_or_default()
                }
            }
            (Some((ap, _)), None) => {
                parse_antigravity_transcript(&ap, since_epoch).unwrap_or_default()
            }
            (None, Some((cp, _))) => parse_claude_transcript(&cp, since_epoch).unwrap_or_default(),
            (None, None) => SessionMetrics::default(),
        }
    } else {
        SessionMetrics::default()
    };

    if let Some(a) = agent_override {
        metrics.agent = a;
    }
    if let Some(m) = model_override {
        metrics.model = m;
    }
    if let Some(e) = effort_override {
        metrics.effort = e;
    }
    if metrics.agent.is_empty() {
        metrics.agent = "Antigravity".to_owned();
    }
    if metrics.model.is_empty() {
        metrics.model = "unknown".to_owned();
    }
    if metrics.effort.is_empty() {
        metrics.effort = "high".to_owned();
    }

    println!(
        "Agent: {} | Model: {} | Effort: {} | Tokens: {} in ({} cached), {} out | Steps: {} | Time: {} | Steers: {}",
        metrics.agent,
        metrics.model,
        metrics.effort,
        format_tokens(metrics.input_tokens),
        format_tokens(metrics.cached_tokens),
        format_tokens(metrics.output_tokens),
        metrics.steps.max(1),
        format_duration(metrics.active_seconds),
        metrics.steers,
    );
    Ok(())
}
