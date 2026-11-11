//! Corpus verification, recording, and regression listing (`cargo xtask corpus`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{execution, producer};

use crate::repo_root;

pub(crate) const SELECTION_BUDGET: usize = 200;
const REGRESSION_PREFIX: &str = "regressed-from:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedRegression {
    pub fixture: String,
    pub status: String,
    pub flags: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CorpusRecord {
    pub fixture: String,
    pub tier: String,
    pub status: String,
    pub millis: u64,
    pub expected_satisfied: Option<bool>,
    pub facts: Vec<(String, String)>,
    pub diagnostics: Vec<String>,
    pub regressed_from: Option<String>,
    pub recorded_at: Option<String>,
}

/// Parse `tests/omega/corpus_outcomes.txt` and return all pinned regressions:
/// - `pass/` or `run/` rows whose status is not `checked`/`built` or that carry `regressed-from:*`
/// - `fail/` rows whose status is not `rejected` or that carry `unexpected`
pub fn list_pinned_regressions_from_text(text: &str) -> Vec<PinnedRegression> {
    let mut regressions = Vec::new();
    let mut current: Option<PinnedRegression> = None;

    let flush = |current: &mut Option<PinnedRegression>, out: &mut Vec<PinnedRegression>| {
        if let Some(row) = current.take() {
            let is_positive = row.fixture.starts_with("pass/") || row.fixture.starts_with("run/");
            let is_negative = row.fixture.starts_with("fail/");
            let has_regressed_from = row.flags.iter().any(|f| f.starts_with(REGRESSION_PREFIX));
            let is_green_status = row.status == "checked" || row.status == "built";
            let is_native_rejection = is_positive
                && row.status == "rejected"
                && row.flags.iter().any(|f| f == "expected")
                && row.flags.iter().any(|f| f == "native-rejection:match");
            let is_regression = if is_positive {
                (!is_green_status && !is_native_rejection) || has_regressed_from
            } else if is_negative {
                row.status != "rejected" || row.flags.iter().any(|f| f == "unexpected")
            } else {
                false
            };
            if is_regression {
                out.push(row);
            }
        }
    };

    for line in text.lines() {
        if let Some(diag) = line.strip_prefix('\t') {
            if let Some(row) = current.as_mut() {
                row.diagnostics.push(unescape_diagnostic(diag.trim()));
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || line.starts_with(' ') {
            continue;
        }
        flush(&mut current, &mut regressions);
        let mut tokens = trimmed.split_whitespace();
        let Some(fixture) = tokens.next() else {
            continue;
        };
        let Some(status) = tokens.next() else {
            continue;
        };
        let flags: Vec<String> = tokens
            .filter(|t| !t.starts_with('@') && !is_duration_token(t))
            .map(str::to_owned)
            .collect();
        current = Some(PinnedRegression {
            fixture: fixture.to_owned(),
            status: status.to_owned(),
            flags,
            diagnostics: Vec::new(),
        });
    }
    flush(&mut current, &mut regressions);
    regressions
}

fn is_duration_token(token: &str) -> bool {
    let digits = token
        .strip_suffix("ms")
        .or_else(|| token.strip_suffix('s'))
        .unwrap_or("");
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

fn escape_diagnostic(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn unescape_diagnostic(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn parse_records(text: &str) -> Result<Vec<CorpusRecord>, String> {
    let mut records: Vec<CorpusRecord> = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(diag) = line.strip_prefix('\t') {
            let Some(last) = records.last_mut() else {
                return Err(format!("line {line_no}: diagnostic before any fixture"));
            };
            last.diagnostics.push(unescape_diagnostic(diag));
            continue;
        }
        let fields: Vec<&str> = line.split(' ').collect();
        if fields.len() < 3 || !fields[2].ends_with("ms") {
            return Err(format!("line {line_no}: not a fixture record: {line}"));
        }
        let fixture = fields[0].to_owned();
        let tier = fixture.split('/').next().unwrap_or("").to_owned();
        let status = fields[1].to_owned();
        let millis: u64 = fields[2][..fields[2].len() - 2]
            .parse()
            .map_err(|_| format!("line {line_no}: invalid duration {}", fields[2]))?;
        let mut expected_satisfied: Option<bool> = None;
        let mut facts: Vec<(String, String)> = Vec::new();
        let mut regressed_from: Option<String> = None;
        let mut recorded_at: Option<String> = None;

        for (index, &token) in fields.iter().enumerate().skip(3) {
            if matches!(token, "expected" | "unexpected")
                && expected_satisfied.is_none()
                && facts.is_empty()
                && regressed_from.is_none()
            {
                expected_satisfied = Some(token == "expected");
            } else if let Some(stamp) = token.strip_prefix('@') {
                if index + 1 != fields.len() || stamp.is_empty() {
                    return Err(format!("line {line_no}: @{stamp} must be the last token"));
                }
                recorded_at = Some(stamp.to_owned());
            } else if let Some(reg) = token.strip_prefix(REGRESSION_PREFIX) {
                regressed_from = Some(reg.to_owned());
            } else if let Some((k, v)) = token.split_once(':') {
                facts.push((k.to_owned(), v.to_owned()));
            } else {
                return Err(format!("line {line_no}: unknown token {token:?}"));
            }
        }
        records.push(CorpusRecord {
            fixture,
            tier,
            status,
            millis,
            expected_satisfied,
            facts,
            diagnostics: Vec::new(),
            regressed_from,
            recorded_at,
        });
    }
    Ok(records)
}

fn record_lines(record: &CorpusRecord) -> Vec<String> {
    let mut header = format!("{} {} {}ms", record.fixture, record.status, record.millis);
    if let Some(sat) = record.expected_satisfied {
        header.push_str(if sat { " expected" } else { " unexpected" });
    }
    for (k, v) in &record.facts {
        header.push_str(&format!(" {k}:{v}"));
    }
    if let Some(reg) = &record.regressed_from {
        header.push_str(&format!(" {REGRESSION_PREFIX}{reg}"));
    }
    if let Some(stamp) = &record.recorded_at {
        header.push_str(&format!(" @{stamp}"));
    }
    let mut lines = vec![header];
    for diag in &record.diagnostics {
        lines.push(format!("\t{}", escape_diagnostic(diag)));
    }
    lines
}

fn block_fixture(block: &[String]) -> Option<&str> {
    let first = block.first()?;
    if first.trim().is_empty() || first.starts_with('#') || first.starts_with('\t') {
        return None;
    }
    first.split(' ').next()
}

pub fn splice_records(text: &str, records: &[CorpusRecord]) -> String {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        if line.starts_with('\t') && blocks.last().and_then(|b| block_fixture(b)).is_some() {
            blocks.last_mut().unwrap().push(line.to_owned());
        } else {
            blocks.push(vec![line.to_owned()]);
        }
    }
    let mut sorted = records.to_vec();
    sorted.sort_by(|a, b| a.fixture.cmp(&b.fixture));
    for record in &sorted {
        let replacement = record_lines(record);
        let existing: Vec<usize> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| (block_fixture(b) == Some(record.fixture.as_str())).then_some(i))
            .collect();
        if !existing.is_empty() {
            blocks[existing[0]] = replacement;
            for &idx in existing.iter().skip(1).rev() {
                blocks.remove(idx);
            }
            continue;
        }
        let pos = blocks
            .iter()
            .position(|b| block_fixture(b).is_some_and(|f| f > record.fixture.as_str()))
            .unwrap_or(blocks.len());
        blocks.insert(pos, replacement);
    }
    let mut out = String::with_capacity(text.len() + 256);
    for block in blocks {
        for line in block {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

fn carried_regression(prior: Option<&CorpusRecord>, observed: &CorpusRecord) -> Option<String> {
    let is_success = matches!(observed.status.as_str(), "built" | "checked");
    if is_success {
        return None;
    }
    let prior = prior?;
    if matches!(prior.status.as_str(), "built" | "checked") {
        let mut token = prior.status.clone();
        for (k, v) in &prior.facts {
            token.push_str(&format!("+{k}:{v}"));
        }
        Some(token)
    } else {
        prior.regressed_from.clone()
    }
}

fn host_target() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows_x86_64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos_arm64"
        } else {
            "macos_x86_64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux_arm64"
    } else {
        "linux_x86_64"
    }
}

pub fn selected_fixture_names(root: &Path, filter_text: &str) -> Result<Vec<String>, String> {
    let fragments: Vec<_> = filter_text
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    let corpus = root.join("tests/omega");
    let mut names = Vec::new();
    for tier in ["pass", "fail", "run"] {
        let base = corpus.join(tier);
        let mut pending = vec![base.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)
                .map_err(|e| format!("read {}: {e}", directory.display()))?
            {
                let entry = entry.map_err(|e| e.to_string())?;
                if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    continue;
                }
                let candidate = entry.path();
                if candidate.join("main.omg").is_file() {
                    let name = format!(
                        "{tier}/{}",
                        candidate
                            .strip_prefix(&base)
                            .map_err(|e| e.to_string())?
                            .to_string_lossy()
                            .replace('\\', "/")
                    );
                    if fragments.is_empty()
                        || fragments.iter().any(|fragment| name.contains(fragment))
                    {
                        names.push(name);
                    }
                } else {
                    pending.push(candidate);
                }
            }
        }
    }
    names.sort();
    Ok(names)
}

/// The runner ignores a shard it cannot use and runs the whole selection, so
/// `--shard 40/40` would be the entire corpus, not the last of forty slices.
fn valid_shard(text: &str) -> bool {
    text.trim().split_once('/').is_some_and(|(k, n)| {
        matches!((k.trim().parse::<u64>(), n.trim().parse::<u64>()), (Ok(k), Ok(n)) if k < n)
    })
}

fn selected_fixture_count(
    root: &Path,
    filter_text: &str,
    shard_text: &str,
) -> Result<usize, String> {
    let count = selected_fixture_names(root, filter_text)?.len();
    if let Some((_, n_str)) = shard_text.trim().split_once('/')
        && let Ok(shards) = n_str.parse::<usize>()
        && shards > 1
    {
        return Ok(count.div_ceil(shards));
    }
    Ok(count)
}

pub(crate) fn current_stamp(root: &Path) -> String {
    let rev = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()
        .is_some_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|line| !line.ends_with("tests/omega/corpus_outcomes.txt"))
        });
    if dirty { format!("{rev}+wt") } else { rev }
}

fn print_pinned_regressions(root: &Path, golden_path: &Path, filters: &[String]) -> Result<(), u8> {
    let text = fs::read_to_string(golden_path).map_err(|e| {
        eprintln!("corpus: cannot read {}: {e}", golden_path.display());
        2u8
    })?;
    let regressions = list_pinned_regressions_from_text(&text);
    let filtered: Vec<&PinnedRegression> = regressions
        .iter()
        .filter(|r| filters.is_empty() || filters.iter().any(|f| r.fixture.contains(f.as_str())))
        .collect();
    for row in &filtered {
        let diag = row.diagnostics.first().map(String::as_str).unwrap_or("");
        if diag.is_empty() {
            println!("{}\t{}", row.fixture, row.status);
        } else {
            println!("{}\t{}\t{diag}", row.fixture, row.status);
        }
    }
    let rel = golden_path.strip_prefix(root).unwrap_or(golden_path);
    eprintln!(
        "corpus: {} pinned regression(s) in {}",
        filtered.len(),
        rel.display()
    );
    Ok(())
}

/// Execute the existing harness with an explicit selection and retain original rows.
pub fn observe(
    root: &Path,
    binary: &Path,
    filter: &str,
    shard: &str,
    native_target: Option<&str>,
    interpret: bool,
    checked_interpreter: bool,
    check_sites: bool,
    evidence: &Path,
) -> Result<String, String> {
    if filter.trim().is_empty() {
        return Err("refusing an unfiltered harness invocation".to_owned());
    }
    fs::create_dir_all(evidence).map_err(|e| e.to_string())?;
    let records_path = evidence.join("records.txt");
    let mut command = Command::new(binary);
    command
        .current_dir(root)
        .stdin(Stdio::null())
        .env("OMEGA_CORPUS_ROOT", root)
        .env("OMEGA_LIBRARY_ROOT", root.join("source/library"))
        .env("OMEGA_CORPUS_RECORDS", &records_path)
        .env("OMEGA_CORPUS_FIXTURE_FILTER", filter)
        .env_remove("NEXTEST")
        .env_remove("OMEGA_CORPUS_MIGRATION_FIXTURES")
        .env_remove("OMEGA_CORPUS_SHARD")
        .env_remove("OMEGA_CORPUS_NATIVE")
        .env_remove("OMEGA_CORPUS_INTERPRET")
        .env_remove("OMEGA_CORPUS_CHECKED_INTERPRETER")
        .env_remove("OMEGA_CORPUS_NATIVE_TARGET");
    if !shard.is_empty() {
        command.env("OMEGA_CORPUS_SHARD", shard);
    }
    if let Some(target) = native_target {
        command
            .env("OMEGA_CORPUS_NATIVE", "1")
            .env("OMEGA_CORPUS_NATIVE_TARGET", target);
    }
    if interpret {
        command.env("OMEGA_CORPUS_INTERPRET", "1");
    }
    if checked_interpreter {
        command.env("OMEGA_CORPUS_CHECKED_INTERPRETER", "1");
    }
    if check_sites {
        command.env("OMEGA_COMPILER_CHECK_SITES", "1");
    }
    let status = execution::run(&mut command, evidence, "runner")?;
    if !status.success() {
        return Err(format!("runner exited {status}; original output retained"));
    }
    fs::read_to_string(records_path).map_err(|e| format!("read original runner records: {e}"))
}

fn validate_runtime_oracles(
    root: &Path,
    records: &[CorpusRecord],
    target: &str,
    native: bool,
    interpret: bool,
) -> Result<(), String> {
    for row in records {
        if row.tier == "fail" || !matches!(row.status.as_str(), "built" | "checked") {
            continue;
        }
        let main = root.join("tests/omega").join(&row.fixture).join("main.omg");
        let expected_native = if native {
            crate::runtime_oracles::exit_oracle(&main, Some(target), false, Some(&row.facts))
                .map_err(|error| format!("{}: {error}", row.fixture))?
        } else {
            None
        };
        let expected_interpreted = if interpret {
            crate::runtime_oracles::exit_oracle(&main, Some(target), true, Some(&row.facts))
                .map_err(|error| format!("{}: {error}", row.fixture))?
        } else {
            None
        };
        if native && interpret && (expected_native.is_some() || expected_interpreted.is_some()) {
            let native_completed = row.facts.iter().any(|(key, _)| key == "exit");
            let interpreted_completed = row.facts.iter().any(|(key, value)| {
                key == "interpreter-exit" || (key == "interpreter" && value == "trapped")
            });
            if !native_completed || !interpreted_completed {
                return Err(format!(
                    "{}: both native and interpreter outcomes are required",
                    row.fixture
                ));
            }
        }
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let mut filter = std::env::var("OMEGA_CORPUS_FIXTURE_FILTER").unwrap_or_default();
    let mut shard = std::env::var("OMEGA_CORPUS_SHARD").unwrap_or_default();
    let mut record = false;
    let mut native = false;
    let mut native_target: Option<String> = None;
    let mut interpret = false;
    let mut checked_interpreter = false;
    let mut list_pinned = false;
    let mut sweep = std::env::var("OMEGA_CORPUS_SWEEP").is_ok_and(|v| !v.is_empty());
    let mut golden: Option<PathBuf> = None;
    let mut runner: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!(
                    "usage: cargo xtask corpus [--list-pinned-regressions] [--filter <fragments>] [--shard <k/N>] [--record] [--native] [--native-target <target>] [--interpret] [--both] [--golden <path>] [--runner <path>] [--sweep]"
                );
                return Ok(());
            }
            "--list-pinned-regressions" => list_pinned = true,
            "--record" => record = true,
            "--native" => native = true,
            "--interpret" => interpret = true,
            "--checked-interpreter" => {
                interpret = true;
                checked_interpreter = true;
            }
            "--both" => {
                native = true;
                interpret = true;
            }
            "--sweep" => sweep = true,
            "--filter" => {
                i += 1;
                filter = args.get(i).cloned().ok_or_else(|| {
                    eprintln!("corpus: --filter requires a value");
                    2u8
                })?;
            }
            "--shard" => {
                i += 1;
                shard = args.get(i).cloned().ok_or_else(|| {
                    eprintln!("corpus: --shard requires a value");
                    2u8
                })?;
            }
            "--native-target" => {
                i += 1;
                let target = args.get(i).cloned().ok_or_else(|| {
                    eprintln!("corpus: --native-target requires a value");
                    2u8
                })?;
                native = true;
                native_target = Some(target);
            }
            "--golden" => {
                i += 1;
                let path = args.get(i).ok_or_else(|| {
                    eprintln!("corpus: --golden requires a path");
                    2u8
                })?;
                golden = Some(PathBuf::from(path));
            }
            "--runner" => {
                i += 1;
                let path = args.get(i).ok_or_else(|| {
                    eprintln!("corpus: --runner requires a path");
                    2u8
                })?;
                runner = Some(PathBuf::from(path));
            }
            other => {
                eprintln!("corpus: unknown option `{other}`");
                return Err(2);
            }
        }
        i += 1;
    }

    if !shard.trim().is_empty() && !valid_shard(&shard) {
        eprintln!(
            "corpus: --shard takes k/N with k from 0 to N-1; `{shard}` would run every fixture"
        );
        return Err(2);
    }

    let target = native_target.as_deref().unwrap_or_else(|| host_target());
    let golden_path = golden.unwrap_or_else(|| {
        let tests_omega = root.join("tests/omega");
        if native && interpret {
            let prefix = if checked_interpreter {
                "corpus_checked_both"
            } else {
                "corpus_both"
            };
            tests_omega.join(format!("{prefix}_{target}.txt"))
        } else if native {
            tests_omega.join(format!("corpus_native_{target}.txt"))
        } else if interpret {
            let name = if checked_interpreter {
                "corpus_checked_interpreted.txt"
            } else {
                "corpus_interpreted.txt"
            };
            tests_omega.join(name)
        } else {
            tests_omega.join("corpus_outcomes.txt")
        }
    });

    if list_pinned {
        let filters: Vec<String> = filter
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        return print_pinned_regressions(&root, &golden_path, &filters);
    }

    let selected = selected_fixture_count(&root, &filter, &shard).map_err(|e| {
        eprintln!("corpus: {e}");
        2u8
    })?;
    // The harness refuses an empty filter, so a whole sweep names the three
    // tiers every fixture name starts with. Listing each fixture instead made
    // one environment string of about a megabyte, which Linux will not pass
    // to a child process (its limit is 128 KiB per string).
    if filter.trim().is_empty() && sweep {
        filter = "pass/,fail/,run/".to_owned();
    }
    if selected > SELECTION_BUDGET && !sweep {
        eprintln!(
            "corpus: this selection runs {selected} fixtures; past {SELECTION_BUDGET} a run is a sweep. Name the fixtures with --filter, or pass --sweep."
        );
        return Err(2);
    }

    if runner.is_some() {
        eprintln!(
            "corpus: --runner cannot authenticate an external executable; use the current-source builder"
        );
        return Err(2);
    }
    if selected == 0 {
        eprintln!("corpus: selection matches no fixtures");
        return Err(2);
    }
    let evidence = execution::reserve_directory(&root.join("build/corpus-runs")).map_err(|e| {
        eprintln!("corpus: {e}");
        4u8
    })?;
    let producer =
        producer::prepare(&root, &evidence.join("producer"), true, false).map_err(|e| {
            eprintln!("corpus: {e}");
            4u8
        })?;
    let binary = producer.runner.as_deref().ok_or(4u8)?;
    let records_text = observe(
        &root,
        binary,
        &filter,
        &shard,
        native.then_some(target),
        interpret,
        checked_interpreter,
        false,
        &evidence,
    )
    .map_err(|e| {
        eprintln!("corpus: {e}");
        4u8
    })?;
    producer.verify().map_err(|e| {
        eprintln!("corpus: {e}");
        4u8
    })?;

    let mut observed = parse_records(&records_text).map_err(|e| {
        eprintln!("corpus: unreadable runner records: {e}");
        4u8
    })?;

    validate_runtime_oracles(&root, &observed, target, native, interpret).map_err(|error| {
        eprintln!("corpus: {error}; original observations retained, golden unchanged");
        3u8
    })?;

    let golden_text = fs::read_to_string(&golden_path).unwrap_or_default();
    let prior_records = parse_records(&golden_text).unwrap_or_default();
    let prior_map: BTreeMap<String, CorpusRecord> = prior_records
        .into_iter()
        .map(|r| (r.fixture.clone(), r))
        .collect();

    if record {
        let stamp = current_stamp(&root);
        for row in &mut observed {
            row.regressed_from = carried_regression(prior_map.get(&row.fixture), row);
            row.recorded_at = Some(stamp.clone());
        }
        let spliced = splice_records(&golden_text, &observed);
        fs::write(&golden_path, spliced).map_err(|e| {
            eprintln!("corpus: failed to write {}: {e}", golden_path.display());
            4u8
        })?;
        println!(
            "corpus: recorded {} fixture outcome(s) to {}",
            observed.len(),
            golden_path.display()
        );
        return Ok(());
    }

    let mut regressed = 0usize;
    let mut moved = 0usize;
    for row in &observed {
        let Some(before) = prior_map.get(&row.fixture) else {
            println!("+ {}: new fixture ({})", row.fixture, row.status);
            moved += 1;
            continue;
        };
        let is_fail = row.tier == "fail";
        let status_regressed = if is_fail {
            matches!(before.status.as_str(), "rejected" | "incomplete")
                && !matches!(row.status.as_str(), "rejected" | "incomplete")
        } else {
            let rank = |s: &str| match s {
                "built" => 2,
                "checked" => 1,
                _ => 0,
            };
            rank(&row.status) < rank(&before.status)
        };
        let expected_regressed =
            before.expected_satisfied == Some(true) && row.expected_satisfied == Some(false);
        if status_regressed || expected_regressed {
            println!(
                "~ {}: REGRESSED ({} -> {})",
                row.fixture, before.status, row.status
            );
            regressed += 1;
        } else if before.status != row.status
            || before.expected_satisfied != row.expected_satisfied
            || before.facts != row.facts
            || before.diagnostics != row.diagnostics
        {
            println!(
                "~ {}: moved ({} -> {})",
                row.fixture, before.status, row.status
            );
            moved += 1;
        }
    }

    if regressed > 0 {
        eprintln!(
            "corpus: {regressed} regressed, {moved} moved across {} observed fixture(s)",
            observed.len()
        );
        Err(3)
    } else if moved > 0 {
        println!(
            "corpus: {moved} moved (0 regressed) across {} observed fixture(s)",
            observed.len()
        );
        Err(1)
    } else {
        println!("corpus: clean — {} fixture(s) verified", observed.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shards_count_from_zero_and_stop_before_the_total() {
        assert!(valid_shard("0/40"));
        assert!(valid_shard(" 39/40 "));
        assert!(!valid_shard("40/40"));
        assert!(!valid_shard("3"));
        assert!(!valid_shard("a/40"));
    }

    #[test]
    fn header_only_and_trap_oracles_guard_recording_without_changing_raw_rows() {
        let root = std::env::temp_dir().join(format!(
            "omega-corpus-exit-oracles-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let directory = root.join("tests/omega/pass/borrows/header_only");
        fs::create_dir_all(&directory).unwrap();
        let main = directory.join("main.omg");
        fs::write(&main, "// Expected exit: 0.\n").unwrap();
        assert_eq!(
            crate::runtime_oracles::exit_oracle(&main, None, false, None).unwrap(),
            Some("0".to_owned())
        );
        let rows = |facts: &str| {
            let text = format!("pass/borrows/header_only built 1ms {facts}");
            parse_records(text.trim_end()).unwrap()
        };
        let wrong = rows("exit:1 interpreter-exit:1 interpreter:agrees");
        let original = wrong.clone();
        assert!(validate_runtime_oracles(&root, &wrong, "linux_x86_64", true, true).is_err());
        assert_eq!(wrong, original);
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:0 interpreter-exit:0 interpreter:agrees"),
                "linux_x86_64",
                true,
                true
            )
            .is_ok()
        );
        assert!(validate_runtime_oracles(&root, &rows(""), "linux_x86_64", true, true).is_err());
        assert!(
            validate_runtime_oracles(&root, &rows("exit:0 exit:1"), "linux_x86_64", true, false)
                .is_err()
        );
        let refused = parse_records(
            "pass/borrows/header_only rejected 1ms expected native-rejection:match\n\tauthored publication refusal\n",
        ).unwrap();
        assert!(validate_runtime_oracles(&root, &refused, "linux_x86_64", true, true).is_ok());
        fs::write(&main, "machine main() {}\n").unwrap();
        fs::write(directory.join("expected_interpreter_exit.txt"), "0\n").unwrap();
        assert!(
            crate::runtime_oracles::exit_oracle(&main, None, false, None)
                .unwrap()
                .is_none()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("interpreter-exit:0"),
                "linux_x86_64",
                false,
                true
            )
            .is_ok()
        );
        assert!(validate_runtime_oracles(&root, &rows(""), "linux_x86_64", false, true).is_err());
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:1 interpreter-exit:0 interpreter:disagrees"),
                "linux_x86_64",
                true,
                true
            )
            .is_err()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("interpreter-exit:0"),
                "linux_x86_64",
                true,
                true
            )
            .is_err()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:0 interpreter-exit:0 interpreter:agrees"),
                "linux_x86_64",
                true,
                true
            )
            .is_ok()
        );
        fs::write(directory.join("expected_interpreter_exit.txt"), "trap\n").unwrap();
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:4 interpreter:trapped"),
                "linux_x86_64",
                true,
                true
            )
            .is_ok()
        );
        fs::remove_file(directory.join("expected_interpreter_exit.txt")).unwrap();
        fs::write(&main, "// Expected exit: 0.\n").unwrap();
        fs::write(directory.join("expected_native_exit.txt"), "trap\n").unwrap();
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:4 interpreter:trapped"),
                "linux_x86_64",
                true,
                true
            )
            .is_ok()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:11 interpreter:trapped"),
                "linux_x86_64",
                true,
                true
            )
            .is_err()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:4 interpreter:declined"),
                "linux_x86_64",
                true,
                true
            )
            .is_err()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:5 interpreter:trapped"),
                "linux_arm64",
                true,
                true
            )
            .is_ok()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:-1073741795 interpreter:trapped"),
                "windows_x86_64",
                true,
                true
            )
            .is_ok()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:1 interpreter:trapped"),
                "linux_x86_64",
                true,
                true
            )
            .is_err()
        );
        assert!(
            validate_runtime_oracles(
                &root,
                &rows("exit:signal signal:4 interpreter:trapped interpreter-engine:checked-tree"),
                "linux_x86_64",
                true,
                true
            )
            .is_ok()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selection_stops_at_fixture_roots_like_the_harness() {
        let root = std::env::temp_dir().join(format!(
            "omega-fixture-selection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for tier in ["pass", "fail", "run"] {
            fs::create_dir_all(root.join("tests/omega").join(tier)).unwrap();
        }
        for path in [
            "pass/group/control",
            "pass/group/control/build/generated",
            "pass/group/control_more",
        ] {
            let directory = root.join("tests/omega").join(path);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("main.omg"), "input").unwrap();
        }
        assert_eq!(
            selected_fixture_names(&root, "pass/group/control").unwrap(),
            ["pass/group/control", "pass/group/control_more"]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identifies_positive_and_negative_pinned_regressions() {
        let sample = "\
# header
pass/math/good checked 12ms @abc1234
pass/math/broken rejected 15ms @abc1234
\terror: obligation failed
fail/math/pinned rejected expected 8ms @abc1234
\terror: expected refusal
fail/math/unpinned checked unexpected 9ms @abc1234
";
        let regressions = list_pinned_regressions_from_text(sample);
        let names: Vec<&str> = regressions.iter().map(|r| r.fixture.as_str()).collect();
        assert_eq!(names, vec!["pass/math/broken", "fail/math/unpinned"]);
    }

    #[test]
    fn parses_and_splices_records_without_disturbing_neighbors() {
        let original = "\
# comment
pass/a/first checked 10ms @111111111111
pass/b/second rejected 20ms @111111111111
\told diagnostic
pass/c/third checked 30ms @111111111111
";
        let update = parse_records("pass/b/second checked 15ms @222222222222\n").unwrap();
        let spliced = splice_records(original, &update);
        assert!(spliced.contains("pass/a/first checked 10ms @111111111111\n"));
        assert!(spliced.contains("pass/b/second checked 15ms @222222222222\n"));
        assert!(!spliced.contains("old diagnostic"));
        assert!(spliced.contains("pass/c/third checked 30ms @111111111111\n"));
    }
}
