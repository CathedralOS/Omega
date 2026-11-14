/*
Compute session model-audit provenance from local agent transcripts (`cargo xtask provenance`).

We emit the single-line commit and pull-request trailer required by `check-rules`:
`Agent: <name> | Model: <model> | Effort: <effort> | Tokens: <in> in (<cached> cached), <out> out | Steps: <N> | Time: <duration> | Steers: <N>`

Why we read transcripts directly instead of trusting agent self-reports:
An LLM inside a coding harness frequently does not know which model ID or
reasoning-effort setting the host UI selected on its behalf, and manual CLI
overrides (`--model`, `--effort`) rot whenever the user switches models mid-run.
We therefore detect the active harness from its process environment first
(`ANTIGRAVITY_CONVERSATION_ID` / `ANTIGRAVITY_AGENT`, `CLAUDECODE` /
`CLAUDE_CODE_ENTRYPOINT`, `CODEX_THREAD_ID` / `CODEX_SANDBOX`) so concurrent
sessions across different harnesses never collide, falling back to the newest
session transcript across `~/.gemini/antigravity/brain`, `~/.claude/projects`,
and `~/.codex/sessions` when invoked from a standalone shell:
- Antigravity (`transcript.jsonl`): `<USER_SETTINGS_CHANGE>` blocks on
  `USER_INPUT` steps record `setting \`Model Selection\` from <old> to <model> (<effort>).`
  We restrict this scan to `type == "USER_INPUT"` so tool outputs (`GENERIC`)
  that quote earlier lines of `transcript.jsonl` never overwrite a newer model
  or effort change.
- Claude Code (`*.jsonl` + `~/.claude/settings.json`): `api-request` entries
  record `params.model` and `params.output_config.effort`, `attachment` entries
  with `type == "model"` record `identity.modelId`, `assistant` messages record
  `message.model` and per-turn token usage, and `~/.claude/settings.json`
  provides the fallback `effortLevel`.
- Codex (`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`): `turn_context`
  entries record `payload.model` and `payload.effort` (or
  `payload.collaboration_mode.settings.reasoning_effort`), while `event_msg`
  entries with `payload.type == "token_count"` record per-step
  `last_token_usage`.
*/

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
    let mut newest_user: Option<(PathBuf, SystemTime)> = None;
    let mut newest_any: Option<(PathBuf, SystemTime)> = None;
    for entry in fs::read_dir(brain).ok()?.flatten() {
        let candidate = entry.path().join(".system_generated/logs/transcript.jsonl");
        if let Ok(meta) = fs::metadata(&candidate)
            && let Ok(modified) = meta.modified()
        {
            if newest_any.as_ref().is_none_or(|(_, t)| modified > *t) {
                newest_any = Some((candidate.clone(), modified));
            }
            if let Ok(head) = fs::read_to_string(&candidate)
                && head
                    .lines()
                    .next()
                    .is_some_and(|l| l.contains("\"USER_INPUT\""))
                && newest_user.as_ref().is_none_or(|(_, t)| modified > *t)
            {
                newest_user = Some((candidate, modified));
            }
        }
    }
    newest_user.or(newest_any)
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

fn find_latest_codex_transcript(home: &Path) -> Option<(PathBuf, SystemTime)> {
    let sessions = home.join(".codex/sessions");
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    let mut stack = vec![sessions];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
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

fn extract_antigravity_settings_model(content: &str) -> Option<(String, String)> {
    let mut result = None;
    let mut rest = content;
    while let Some(start) = rest.find("<USER_SETTINGS_CHANGE>") {
        let after_start = &rest[start + "<USER_SETTINGS_CHANGE>".len()..];
        let Some(end) = after_start.find("</USER_SETTINGS_CHANGE>") else {
            break;
        };
        let block = &after_start[..end];
        if let Some(idx) = block.find("setting `Model Selection` from ") {
            let after_from = &block[idx + "setting `Model Selection` from ".len()..];
            if let Some(to_idx) = after_from.find(" to ") {
                let after_to = &after_from[to_idx + 4..];
                if let Some(period) = after_to.find('.') {
                    result = Some(normalize_antigravity_model(&after_to[..period]));
                }
            }
        }
        rest = &after_start[end + "</USER_SETTINGS_CHANGE>".len()..];
    }
    result
}

fn parse_antigravity_transcript(path: &Path, since_epoch: Option<i64>) -> Option<SessionMetrics> {
    let content = fs::read_to_string(path).ok()?;
    if !content.contains("\"PLANNER_RESPONSE\"") && !content.contains("\"USER_INPUT\"") {
        return None;
    }
    let mut metrics = SessionMetrics {
        agent: "Antigravity".to_owned(),
        model: String::new(),
        effort: "high".to_owned(),
        ..SessionMetrics::default()
    };

    let mut window_user_inputs = 0u64;
    let mut prev_ts: Option<i64> = None;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val["type"].as_str() == Some("USER_INPUT")
            && let Some(text) = val["content"].as_str()
            && let Some((m, e)) = extract_antigravity_settings_model(text)
        {
            metrics.model = m;
            metrics.effort = e;
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

    if metrics.steps == 0 && since_epoch.is_some() {
        return parse_antigravity_transcript(path, None);
    }

    metrics.steers = window_user_inputs.saturating_sub(1);
    Some(metrics)
}

fn claude_settings_effort(home: Option<&Path>) -> Option<String> {
    let settings_path = home?.join(".claude/settings.json");
    let content = fs::read_to_string(settings_path).ok()?;
    let val: Value = serde_json::from_str(&content).ok()?;
    val["effortLevel"]
        .as_str()
        .map(|s| s.trim().to_ascii_lowercase())
}

fn parse_claude_transcript(
    path: &Path,
    home: Option<&Path>,
    since_epoch: Option<i64>,
) -> Option<SessionMetrics> {
    let content = fs::read_to_string(path).ok()?;
    if !content.contains("\"api-request\"") && !content.contains("\"parentUuid\"") {
        return None;
    }
    let default_effort = claude_settings_effort(home).unwrap_or_else(|| "high".to_owned());
    let mut metrics = SessionMetrics {
        agent: "Claude Code".to_owned(),
        model: String::new(),
        effort: default_effort,
        ..SessionMetrics::default()
    };
    let mut window_user_inputs = 0u64;
    let mut prev_ts: Option<i64> = None;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val["attachment"]["type"].as_str() == Some("model")
            && let Some(m) = val["attachment"]["identity"]["modelId"].as_str()
        {
            metrics.model = m.to_owned();
        }
        if val["type"].as_str() == Some("api-request") {
            if let Some(m) = val["params"]["model"].as_str() {
                metrics.model = m.to_owned();
            }
            if let Some(e) = val["params"]["output_config"]["effort"].as_str() {
                metrics.effort = e.trim().to_ascii_lowercase();
            }
        }

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
        return parse_claude_transcript(path, home, None);
    }

    metrics.steers = window_user_inputs.saturating_sub(1);
    Some(metrics)
}

fn parse_codex_transcript(path: &Path, since_epoch: Option<i64>) -> Option<SessionMetrics> {
    let content = fs::read_to_string(path).ok()?;
    if !content.contains("\"session_meta\"") && !content.contains("\"turn_context\"") {
        return None;
    }
    let mut metrics = SessionMetrics {
        agent: "Codex".to_owned(),
        model: String::new(),
        effort: "high".to_owned(),
        ..SessionMetrics::default()
    };
    let mut window_user_inputs = 0u64;
    let mut prev_ts: Option<i64> = None;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val["type"].as_str() == Some("turn_context") {
            let payload = &val["payload"];
            if let Some(m) = payload["model"]
                .as_str()
                .or_else(|| payload["collaboration_mode"]["settings"]["model"].as_str())
            {
                metrics.model = m.to_owned();
            }
            if let Some(e) = payload["effort"]
                .as_str()
                .or_else(|| payload["collaboration_mode"]["settings"]["reasoning_effort"].as_str())
            {
                metrics.effort = e.trim().to_ascii_lowercase();
            }
        }

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

        if val["type"].as_str() == Some("event_msg") {
            let payload = &val["payload"];
            match payload["type"].as_str() {
                Some("task_started") | Some("user_message") => {
                    window_user_inputs += 1;
                }
                Some("token_count") => {
                    let usage = &payload["info"]["last_token_usage"];
                    let input = usage["input_tokens"].as_u64().unwrap_or(0);
                    let cached = usage["cached_input_tokens"].as_u64().unwrap_or(0);
                    let out = usage["output_tokens"].as_u64().unwrap_or(0);
                    if input + out > 0 {
                        metrics.steps += 1;
                        metrics.input_tokens += input;
                        metrics.cached_tokens += cached;
                        metrics.output_tokens += out;
                    }
                }
                _ => {}
            }
        }
    }

    if metrics.steps == 0 && since_epoch.is_some() {
        return parse_codex_transcript(path, None);
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

    let root = repo_root()?;
    let since_epoch = if full_session {
        None
    } else {
        latest_origin_main_epoch(&root)
    };
    let home = home_dir();

    let mut metrics = if let Some(path) = transcript_override {
        parse_antigravity_transcript(&path, since_epoch)
            .or_else(|| parse_claude_transcript(&path, home.as_deref(), since_epoch))
            .or_else(|| parse_codex_transcript(&path, since_epoch))
            .unwrap_or_default()
    } else if let Some(conv_id) = std::env::var_os("ANTIGRAVITY_CONVERSATION_ID") {
        let app_dir = std::env::var_os("ANTIGRAVITY_APP_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".gemini/antigravity")));
        app_dir
            .map(|dir| {
                dir.join("brain")
                    .join(conv_id)
                    .join(".system_generated/logs/transcript.jsonl")
            })
            .and_then(|path| parse_antigravity_transcript(&path, since_epoch))
            .unwrap_or_default()
    } else if std::env::var_os("ANTIGRAVITY_AGENT").is_some() {
        home.as_deref()
            .and_then(find_latest_antigravity_transcript)
            .and_then(|(path, _)| parse_antigravity_transcript(&path, since_epoch))
            .unwrap_or_default()
    } else if std::env::var_os("CLAUDECODE").is_some()
        || std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_some()
    {
        home.as_deref()
            .and_then(find_latest_claude_transcript)
            .and_then(|(path, _)| parse_claude_transcript(&path, home.as_deref(), since_epoch))
            .unwrap_or_default()
    } else if std::env::var_os("CODEX_THREAD_ID").is_some()
        || std::env::var_os("CODEX_SANDBOX").is_some()
        || std::env::var_os("CODEX_CI").is_some()
    {
        home.as_deref()
            .and_then(find_latest_codex_transcript)
            .and_then(|(path, _)| parse_codex_transcript(&path, since_epoch))
            .unwrap_or_default()
    } else if let Some(ref home_path) = home {
        let mut candidates: Vec<(&str, PathBuf, SystemTime)> = Vec::new();
        if let Some((p, t)) = find_latest_antigravity_transcript(home_path) {
            candidates.push(("antigravity", p, t));
        }
        if let Some((p, t)) = find_latest_claude_transcript(home_path) {
            candidates.push(("claude", p, t));
        }
        if let Some((p, t)) = find_latest_codex_transcript(home_path) {
            candidates.push(("codex", p, t));
        }
        candidates.sort_by_key(|a| std::cmp::Reverse(a.2));
        candidates
            .into_iter()
            .find_map(|(kind, path, _)| match kind {
                "antigravity" => parse_antigravity_transcript(&path, since_epoch),
                "claude" => parse_claude_transcript(&path, Some(home_path), since_epoch),
                "codex" => parse_codex_transcript(&path, since_epoch),
                _ => None,
            })
            .unwrap_or_default()
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
