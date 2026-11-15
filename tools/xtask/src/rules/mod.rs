//! Repository admission rules (`cargo xtask check-rules`):
//! - Per-file count ratchets (`source_text_reads.txt`, `contract_status_phrases.txt`,
//!   `dense_id_maps.txt`, `cache_state.txt`, `place_transport.txt`)
//! - Dangling task ID citations (`dangling_task_ids.txt`)
//! - Discrete `TASKS.md` board shape
//! - Diff hygiene (`MAX_NEW_OMG_FILES = 6`, the `samples/cli/facts/` freeze on growth, no new
//!   files under the retired `tests/omega/`, raw measurements limit)
//! - Commit title lane (`AGENTS.md` table) and PR `Agent:` provenance
//! - Compiler-lane PR `Gate:`, `Proof:` and `Spec:` lines

mod board_and_tasks;
mod commit_metadata;
mod diff_hygiene;
mod ratchets;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo_root;

use self::board_and_tasks::{
    DANGLING_BASELINE, TASK_BOARDS, TASKS_BOARD, board_task_ids, citing_documents,
    dangling_task_ids, dangling_task_violations, read_dangling_baseline, tasks_board_violations,
};
use self::commit_metadata::{
    gate_violations, lanes, proof_violations, provenance_violations, spec_violations,
    subject_violations,
};
use self::diff_hygiene::{diff_hygiene_violations, measurement_violations};
use self::ratchets::{
    RATCHET_SPECS, checking_interior_state_violations, compute_ratchet_counts, format_ratchet_file,
    ratchet_violations, read_baseline, semantic_id_names,
};

pub(super) const FACTS_ROOT: &str = "samples/cli/facts/";

pub(super) fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(super) fn show_git_file(root: &Path, rev: &str, path: &str) -> String {
    git(root, &["show", &format!("{rev}:{path}")]).unwrap_or_default()
}

pub(super) fn pr_diff_base(root: &Path, requested: Option<&str>) -> Result<String, String> {
    // Every diff rule must inspect the topic's changes, not undo work that
    // arrived on its target after the fork. Resolve once so concurrent fetches
    // cannot make the diff checks use different bases.
    let requested = requested.unwrap_or("origin/main");
    let bases = git(root, &["merge-base", "--all", requested, "HEAD"])?;
    let bases: Vec<_> = bases.lines().filter(|base| !base.is_empty()).collect();
    match bases.as_slice() {
        [base] => Ok((*base).to_owned()),
        _ => Err(format!(
            "expected one PR merge base for {requested} and HEAD, found {}; fetch the needed history or name an ancestor commit with --base",
            bases.len()
        )),
    }
}

pub(super) fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// --- File walking helpers ---

pub(super) fn collect_files_with_ext(dir: &Path, root: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut sorted: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    sorted.sort();
    for path in sorted {
        if let Ok(rel) = path.strip_prefix(root)
            && rel.components().any(|c| c.as_os_str() == "target")
        {
            continue;
        }
        if path.is_dir() {
            collect_files_with_ext(&path, root, ext, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut base: Option<String> = None;
    let mut title: Option<String> = None;
    let mut body_file: Option<PathBuf> = None;
    let mut update_flags = BTreeSet::new();
    let mut update_dangling = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--base" => {
                i += 1;
                base = args.get(i).cloned();
            }
            "--title" => {
                i += 1;
                title = args.get(i).cloned();
            }
            "--body-file" => {
                i += 1;
                body_file = args.get(i).map(PathBuf::from);
            }
            "--update-dangling-task-ids" => {
                update_dangling = true;
            }
            "-h" | "--help" => {
                println!(
                    "usage: cargo xtask check-rules [--base <rev>] [--title <title>] [--body-file <path>]"
                );
                return Ok(());
            }
            other => {
                let flag = other.strip_prefix("--").unwrap_or("");
                if RATCHET_SPECS.iter().any(|s| s.flag == flag) {
                    update_flags.insert(flag.to_owned());
                } else {
                    eprintln!("check-rules: unknown argument `{other}`");
                    return Err(2);
                }
            }
        }
        i += 1;
    }

    let root = repo_root()?;
    let semantic_ids = semantic_id_names(&root);

    if !update_flags.is_empty() {
        for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
            if update_flags.contains(spec.flag) {
                let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
                let text = format_ratchet_file(spec, &counts);
                let _ = fs::write(root.join(spec.baseline), text);
                println!(
                    "check_rules: wrote {} files to {}",
                    counts.len(),
                    spec.baseline
                );
            }
        }
        return Ok(());
    }

    let boards: Vec<String> = TASK_BOARDS
        .iter()
        .map(|b| fs::read_to_string(root.join(b)).unwrap_or_default())
        .collect();
    let dangling = dangling_task_ids(&citing_documents(&root), &board_task_ids(&boards));
    if update_dangling {
        let mut lines = vec![
            "# Task IDs cited by samples/cli/facts READMEs that no board holds.".to_owned(),
            "# Written by `cargo xtask check-rules --update-dangling-task-ids`.".to_owned(),
            "# Entries may only be removed: repoint a citation to its current board".to_owned(),
            "# item, or delete it when the work is done.".to_owned(),
        ];
        for (path, task) in &dangling {
            if path.starts_with(FACTS_ROOT) {
                lines.push(format!("{path} {task}"));
            }
        }
        let text = lines.join("\n") + "\n";
        let _ = fs::write(root.join(DANGLING_BASELINE), &text);
        println!(
            "check_rules: wrote {} citations to {DANGLING_BASELINE}",
            read_dangling_baseline(&text).len()
        );
        return Ok(());
    }

    let mut violations = Vec::new();
    for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
        let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
        let baseline_text = fs::read_to_string(root.join(spec.baseline)).unwrap_or_default();
        let baseline = read_baseline(&baseline_text);
        violations.extend(ratchet_violations(spec, &counts, &baseline));
    }

    violations.extend(checking_interior_state_violations(&root));

    let dangling_baseline_text =
        fs::read_to_string(root.join(DANGLING_BASELINE)).unwrap_or_default();
    violations.extend(dangling_task_violations(
        &dangling,
        &read_dangling_baseline(&dangling_baseline_text),
    ));

    let base_rev = pr_diff_base(&root, base.as_deref()).map_err(|error| {
        eprintln!("check_rules: {error}");
        2u8
    })?;

    // Diff checks against `base_rev`
    if let Ok(diff_out) = git(
        &root,
        &["diff", "--name-status", "--no-renames", &base_rev, "HEAD"],
    ) {
        let mut changes = Vec::new();
        for line in diff_out.lines() {
            if let Some((status, path)) = line.split_once('\t')
                && let Some(ch) = status.chars().next()
            {
                changes.push((ch, path.to_owned()));
            }
        }
        violations.extend(diff_hygiene_violations(&changes));
        violations.extend(measurement_violations(&root, &changes));
    }

    let head_tasks = fs::read_to_string(root.join(TASKS_BOARD))
        .unwrap_or_else(|_| show_git_file(&root, "HEAD", TASKS_BOARD));
    violations.extend(tasks_board_violations(&head_tasks));

    if let Some(t) = &title {
        let agents_text = fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default();
        violations.extend(subject_violations(t, &lanes(&agents_text)));
    }
    if let Some(bf) = body_file {
        let body = fs::read_to_string(&bf).unwrap_or_default();
        violations.extend(provenance_violations(&body));
        if let Some(t) = &title {
            violations.extend(gate_violations(t, &body));
            violations.extend(proof_violations(t, &body));
            violations.extend(spec_violations(t, &body, &root));
        }
    }

    for v in &violations {
        eprintln!("check_rules: {v}");
    }
    if violations.is_empty() {
        let short_base = &base_rev[..base_rev.len().min(12)];
        println!("check_rules: clean against {short_base}");
        Ok(())
    } else {
        Err(1)
    }
}
