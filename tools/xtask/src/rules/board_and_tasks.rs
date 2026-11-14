//! Dangling task-ID citation checks and discrete `TASKS.md` board shape rules.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{FACTS_ROOT, collect_files_with_ext, is_ident_byte};

pub(super) const DANGLING_BASELINE: &str = "tools/ratchets/dangling_task_ids.txt";
pub(super) const TASK_BOARDS: &[&str] = &[
    "TASKS.md",
    "TASKS_OPTIMIZER.md",
    "TASKS_BOOTSTRAP.md",
    "OWNER_QUESTIONS.md",
];
pub(super) const TASKS_BOARD: &str = "TASKS.md";

fn is_task_segment(seg: &str) -> bool {
    if seg.is_empty()
        || !seg
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    seg.bytes().filter(|b| b.is_ascii_uppercase()).count() >= 2
}

fn extract_task_ids(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let prev_bad =
            i > 0 && (is_ident_byte(bytes[i - 1]) || matches!(bytes[i - 1], b'/' | b'.' | b'-'));
        if !prev_bad && (bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit()) {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit() || bytes[i] == b'-')
            {
                i += 1;
            }
            let candidate = &text[start..i];
            let next_bad = (i < bytes.len() && (is_ident_byte(bytes[i]) || bytes[i] == b'-'))
                || (i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_lowercase());
            if !next_bad {
                let segments: Vec<&str> = candidate.split('-').collect();
                if segments.len() >= 2 && segments.iter().all(|s| is_task_segment(s)) {
                    out.push(candidate.to_owned());
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

pub(super) fn board_task_ids(boards: &[String]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for text in boards {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("- **")
                && let Some(dot_end) = rest.find(".**")
            {
                let id = &rest[..dot_end];
                if !id.is_empty()
                    && id.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
                    && id.contains('-')
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
                {
                    ids.insert(id.to_owned());
                    let after = rest[dot_end + 3..].trim_start();
                    if after.starts_with('(')
                        && let Some(rp) = after.find(')')
                    {
                        for extra in extract_task_ids(&after[..=rp]) {
                            ids.insert(extra);
                        }
                    }
                }
            }
        }
    }
    ids
}

pub(super) fn citing_documents(root: &Path) -> BTreeMap<String, String> {
    let mut paths = vec![root.join("AGENTS.md"), root.join("omega-rust/pipeline.md")];
    collect_files_with_ext(&root.join(".agents/skills"), root, "md", &mut paths);
    collect_files_with_ext(&root.join("wiki/spec"), root, "md", &mut paths);
    if let Ok(entries) = fs::read_dir(root.join(FACTS_ROOT)) {
        let mut fact_readmes: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path().join("README.md")))
            .filter(|p| p.is_file())
            .collect();
        fact_readmes.sort();
        paths.extend(fact_readmes);
    }
    let mut docs = BTreeMap::new();
    for path in paths {
        if path.is_file()
            && let Ok(rel) = path.strip_prefix(root)
            && let Ok(text) = fs::read_to_string(&path)
        {
            let key = rel.to_string_lossy().replace('\\', "/");
            docs.insert(key, text);
        }
    }
    docs
}

pub(super) fn dangling_task_ids(
    documents: &BTreeMap<String, String>,
    known: &BTreeSet<String>,
) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for (path, text) in documents {
        for task in extract_task_ids(text) {
            if !known.contains(&task) {
                out.insert((path.clone(), task));
            }
        }
    }
    out
}

pub(super) fn read_dangling_baseline(text: &str) -> BTreeSet<(String, String)> {
    let mut set = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if let (Some(path), Some(task)) = (parts.next(), parts.next()) {
            set.insert((path.to_owned(), task.to_owned()));
        }
    }
    set
}

pub(super) fn dangling_task_violations(
    dangling: &BTreeSet<(String, String)>,
    baseline: &BTreeSet<(String, String)>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, task) in dangling {
        if !baseline.contains(&(path.clone(), task.clone())) || !path.starts_with(FACTS_ROOT) {
            violations.push(format!(
                "{path}: cites {task}, which is on no board ({}). Repoint it to the item that owns the work now (git log -S finds where it went), or delete it when the work is done",
                TASK_BOARDS.join(", ")
            ));
        }
    }
    let resolved: Vec<&(String, String)> = baseline.difference(dangling).collect();
    if let Some((first_path, first_task)) = resolved.first() {
        violations.push(format!(
            "{DANGLING_BASELINE}: {} entries no longer dangle ({first_path} {first_task} first). Lower the baseline: cargo xtask check-rules --update-dangling-task-ids",
            resolved.len()
        ));
    }
    violations
}

fn is_tasks_item_line(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("## ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("<a id=\"")
        && let Some(id) = rest.strip_suffix("\"></a>")
    {
        return !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    }
    if let Some(rest) = line.strip_prefix("  - ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("  Acceptance: ") {
        return rest.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    if let Some(rest) = line.strip_prefix("- **")
        && let Some((id, after)) = rest.split_once(".** ")
    {
        return !id.is_empty()
            && id.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
            && id.contains('-')
            && id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            && after.chars().next().is_some_and(|c| !c.is_whitespace());
    }
    false
}

/// The board may gain tasks. A new decision needs a place to land its
/// implementation work, so growth against the base is not a violation; the
/// one-line discrete shape still holds.
pub(super) fn tasks_board_violations(head_text: &str) -> Vec<String> {
    if head_text.is_empty() {
        return Vec::new();
    }
    let mut violations = Vec::new();
    let head_lines: Vec<&str> = head_text.lines().collect();
    let mut in_items = false;
    for (idx, line) in head_lines.iter().enumerate() {
        let line_no = idx + 1;
        if *line == "## Priority (human-set only)" {
            in_items = true;
        }
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with([' ', '\t'])
            && !line.starts_with("  - ")
            && !line.starts_with("  Acceptance: ")
        {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: wrapped continuation or over-indented line; write each bullet and Acceptance gate on a single discrete line"
            ));
        } else if in_items && !is_tasks_item_line(line) {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: unexpected line in item section; use only `## Section`, `- **ITEM.** ...`, `  - ...`, or `  Acceptance: ...`"
            ));
        }
        if line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("landed:")
        {
            violations.push(format!(
                "{TASKS_BOARD}:{line_no}: status-history phrase 'Landed:'; delete completed clauses outright instead of recording status prose"
            ));
        }
    }
    violations
}
