//! Corpus regression gate for pull requests and the nightly ledger sweep
//! (`cargo xtask corpus-gate`).
//!
//! CI ran no corpus before this gate, so a compiler change could break
//! hundreds of ledger rows and land green (#2370 broke more than 100). The
//! gate runs, for each pull request, the corpus rows its diff can affect and
//! fails when one of them gets worse than `tests/omega/corpus_outcomes.txt`
//! records: a `pass/` or `run/` row that stops checking, a `fail/` row the
//! compiler now accepts, or a `fail/` row that still rejects but no longer
//! with its pinned reason.
//!
//! - `plan` reads the diff against the base and chooses the rows: every
//!   touched fixture and its twins, a fixed fast smoke set, the fixture
//!   families the touched crates map to (`tools/ci/corpus_gate_paths.txt`),
//!   and, for any change that can reach the compiler, a sample spread over
//!   every tier and group. Rows are packed into shards by recorded time. A
//!   pull request that changes only ledgers and Markdown (#2375's rule) skips.
//! - `check` runs one shard at the candidate. A row worse than the ledger is
//!   run once more (a timeout on a loaded runner is not a regression), then
//!   excused only when the ledger row is the base's own and main already
//!   shows the same loss: in the nightly sweep's observations of main, or in
//!   a run of the row at the base commit. Whatever remains is the pull
//!   request's own and fails the job.
//! - `build` compiles the corpus runner once and writes it, with the receipt
//!   that names its commit and bytes, into a directory. A workflow run builds
//!   it in one job and hands that directory to every shard (`--runner`,
//!   `--base-runner`), which then compile nothing: before this, each of up to
//!   8 gate shards and 64 sweep shards compiled the same runner itself. A
//!   shard accepts the directory only for the exact clean commit it has
//!   checked out, and fails when it is missing or does not match.
//! - `sweep-shard` and `sweep-report` are the nightly whole-corpus run: each
//!   shard keeps its raw records, and the report splices every row whose
//!   outcome differs from the ledger, lists accepted `fail/` rows first as
//!   soundness alerts, and tables the rest by first diagnostic.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::corpus::{self, CorpusRecord};
use crate::{execution, producer, repo_root, rules};

const LEDGER: &str = "tests/omega/corpus_outcomes.txt";
const SMOKE_LIST: &str = "tools/ci/corpus_gate_smoke.txt";
const PATH_MAP: &str = "tools/ci/corpus_gate_paths.txt";
/// A fixture with no ledger row is budgeted as if it took this long.
const UNKNOWN_MILLIS: u64 = 30_000;
/// The broad sample skips fixtures recorded above this; named families and
/// touched fixtures do not.
const BROAD_HEAVY_MILLIS: u64 = 40_000;

// ---------------------------------------------------------------------------
// Row comparison
// ---------------------------------------------------------------------------

/// Why `after` is worse than `before`, or `None` when it is not.
///
/// `before` is the recorded row; with no row, the fixture's tier is the
/// expectation. Diagnostic wording and timing never count: only a lost
/// success, an accepted must-reject program, or a lost pinned reason.
pub fn worse(before: Option<&CorpusRecord>, after: &CorpusRecord) -> Option<String> {
    let rejects = |r: &CorpusRecord| matches!(r.status.as_str(), "rejected" | "incomplete");
    if after.tier == "fail" {
        let had_rejection = before.is_none_or(rejects);
        if had_rejection && !rejects(after) {
            return Some(format!(
                "must-reject fixture is now {} (was {})",
                after.status,
                before.map_or("unrecorded", |r| r.status.as_str())
            ));
        }
        let had_reason = before.map_or(rejects(after), |r| r.expected_satisfied == Some(true));
        if had_reason && rejects(after) && after.expected_satisfied == Some(false) {
            return Some("still rejects, but without its pinned reason".to_owned());
        }
        return None;
    }
    let had_success = before.is_none_or(corpus::is_recorded_success);
    if had_success && !corpus::is_recorded_success(after) {
        return Some(format!(
            "lost its success: {} -> {}",
            before.map_or("unrecorded", |r| r.status.as_str()),
            after.status
        ));
    }
    None
}

/// Whether two observations of one fixture record the same outcome.
fn same_outcome(a: &CorpusRecord, b: &CorpusRecord) -> bool {
    a.status == b.status
        && a.expected_satisfied == b.expected_satisfied
        && a.facts == b.facts
        && a.diagnostics == b.diagnostics
}

fn record_map(records: Vec<CorpusRecord>) -> BTreeMap<String, CorpusRecord> {
    records
        .into_iter()
        .map(|r| (r.fixture.clone(), r))
        .collect()
}

fn read_records(path: &Path) -> Result<BTreeMap<String, CorpusRecord>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    corpus::parse_records(&text)
        .map(record_map)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned())
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Plan {
    pub base: String,
    pub head: String,
    /// Why the pull request needs no corpus run, when it needs none.
    pub skip: Option<String>,
    pub shards: Vec<Vec<String>>,
    /// Why each fixture was chosen: touched, smoke, family or sample.
    pub reasons: BTreeMap<String, String>,
    pub budget_secs_per_shard: u64,
    pub recorded_secs: Vec<u64>,
    /// Whether the run should build the base commit's runner ahead of the
    /// shards ([`base_may_be_needed`]).
    #[serde(default)]
    pub needs_base: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathClass {
    /// Cannot change what any fixture compiles to.
    Neutral,
    /// Can reach the compiler, its library, or the corpus inputs.
    Compiler,
}

/// Paths that cannot change a corpus outcome. Everything else counts as
/// reaching the compiler, so an unlisted new directory fails closed into the
/// broad sample rather than out of the gate.
const NEUTRAL_PREFIXES: &[&str] = &[
    ".github/",
    ".agents/",
    ".claude/",
    "wiki/",
    "bootstrap/",
    "tests/bootstrap/",
    "tools/",
    "samples/",
];

pub fn classify(path: &str) -> PathClass {
    let prose = path.ends_with(".md") && !path.starts_with("tests/omega/");
    let ledger = path
        .strip_prefix("tests/omega/corpus_")
        .is_some_and(|rest| rest.ends_with(".txt") && !rest.contains('/'));
    if prose || ledger || NEUTRAL_PREFIXES.iter().any(|p| path.starts_with(p)) {
        PathClass::Neutral
    } else {
        PathClass::Compiler
    }
}

/// `prefix fragment,fragment` lines; `#` starts a comment.
pub fn parse_path_map(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (prefix, fragments) = line.split_once(char::is_whitespace)?;
            Some((
                prefix.to_owned(),
                fragments
                    .split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ))
        })
        .collect()
}

fn fnv1a(seed: u64, text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn group_of(fixture: &str) -> &str {
    let mut parts = fixture.splitn(3, '/');
    let tier_len = parts.next().map_or(0, str::len);
    let group_len = parts.next().map_or(0, str::len);
    &fixture[..(tier_len + 1 + group_len).min(fixture.len())]
}

/// The fixture a changed path belongs to, if any: the nearest directory
/// under `tests/omega/{pass,fail,run}/` that holds a `main.omg` in `all`.
fn owning_fixture<'a>(path: &str, all: &'a BTreeSet<String>) -> Option<&'a String> {
    let rest = path.strip_prefix("tests/omega/")?;
    let mut candidate = rest;
    while let Some((parent, _)) = candidate.rsplit_once('/') {
        if let Some(found) = all.get(parent) {
            return Some(found);
        }
        candidate = parent;
    }
    None
}

/// The same fixture name in the other tiers: a pass program's fail twin.
fn twins<'a>(fixture: &str, all: &'a BTreeSet<String>) -> Vec<&'a String> {
    let Some((_, suffix)) = fixture.split_once('/') else {
        return Vec::new();
    };
    ["pass", "fail", "run"]
        .iter()
        .filter_map(|tier| all.get(&format!("{tier}/{suffix}")))
        .filter(|name| name.as_str() != fixture)
        .collect()
}

/// Fixtures whose `build.omg` names a package under `samples/`: a change to
/// that sample reaches them.
fn sample_dependents(root: &Path, all: &BTreeSet<String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for fixture in all {
        let Ok(text) = fs::read_to_string(root.join("tests/omega").join(fixture).join("build.omg"))
        else {
            continue;
        };
        for piece in text.split('"') {
            if let Some(index) = piece.find("samples/") {
                out.push((
                    piece[index..].trim_end_matches('/').to_owned(),
                    fixture.clone(),
                ));
            }
        }
    }
    out
}

pub struct Selector<'a> {
    pub millis: &'a BTreeMap<String, u64>,
    pub chosen: BTreeMap<String, String>,
    pub spent: u64,
    pub budget: u64,
}

impl Selector<'_> {
    fn cost(&self, fixture: &str) -> u64 {
        self.millis.get(fixture).copied().unwrap_or(UNKNOWN_MILLIS)
    }

    /// Take `pool` whole when it fits, otherwise a sample spread round-robin
    /// over its tier/group families in a fixed hash order, until the budget
    /// is spent. `always` takes the pool whole regardless of the budget.
    fn take(&mut self, pool: &[String], reason: &str, always: bool, heavy_cap: Option<u64>) {
        let pool: Vec<&String> = pool
            .iter()
            .filter(|f| !self.chosen.contains_key(*f))
            .filter(|f| heavy_cap.is_none_or(|cap| self.cost(f) <= cap))
            .collect();
        let whole: u64 = pool.iter().map(|f| self.cost(f)).sum();
        if always || self.spent + whole <= self.budget {
            for fixture in pool {
                self.spent += self.cost(fixture);
                self.chosen.insert(fixture.clone(), reason.to_owned());
            }
            return;
        }
        let mut families: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
        for fixture in pool {
            families.entry(group_of(fixture)).or_default().push(fixture);
        }
        let mut queues: Vec<std::vec::IntoIter<&String>> = families
            .into_values()
            .map(|mut members| {
                members.sort_by_key(|f| (fnv1a(0x006f_6d65_6761, f), (*f).clone()));
                members.into_iter()
            })
            .collect();
        let sampled = format!("{reason} (sampled)");
        loop {
            let mut progressed = false;
            for queue in &mut queues {
                if let Some(fixture) = queue.next() {
                    progressed = true;
                    let cost = self.cost(fixture);
                    if self.spent + cost <= self.budget {
                        self.spent += cost;
                        self.chosen.insert(fixture.clone(), sampled.clone());
                    }
                }
            }
            if !progressed || self.spent >= self.budget {
                break;
            }
        }
    }
}

pub struct PlanInputs<'a> {
    pub changes: &'a [(char, String)],
    pub all: &'a BTreeSet<String>,
    pub millis: &'a BTreeMap<String, u64>,
    pub smoke: &'a [String],
    pub path_map: &'a [(String, Vec<String>)],
    pub sample_dependents: &'a [(String, String)],
    pub shards: usize,
    pub budget_secs_per_shard: u64,
}

/// Choose and pack the rows a diff can affect. Pure, so tests can drive it.
pub fn plan_selection(inputs: &PlanInputs) -> (Option<String>, BTreeMap<String, String>) {
    if rules::records_observations_only(inputs.changes) {
        return (
            Some(
                "only corpus ledgers and Markdown changed (#2375): nothing the corpus reads moved"
                    .to_owned(),
            ),
            BTreeMap::new(),
        );
    }
    let mut selector = Selector {
        millis: inputs.millis,
        chosen: BTreeMap::new(),
        spent: 0,
        budget: inputs.budget_secs_per_shard * 1000 * inputs.shards as u64,
    };
    let mut touched = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut reaches_compiler = false;
    for (_, path) in inputs.changes {
        if let Some(fixture) = owning_fixture(path, inputs.all) {
            touched.insert(fixture.clone());
            touched.extend(twins(fixture, inputs.all).into_iter().cloned());
            continue;
        }
        for (sample, fixture) in inputs.sample_dependents {
            if path.starts_with(&format!("{sample}/")) {
                touched.insert(fixture.clone());
            }
        }
        if classify(path) == PathClass::Compiler {
            reaches_compiler = true;
        }
        for (prefix, fragments) in inputs.path_map {
            if path.starts_with(prefix.as_str()) {
                families.extend(fragments.iter().cloned());
            }
        }
    }
    let touched: Vec<String> = touched.into_iter().collect();
    selector.take(&touched, "touched by the diff", true, None);
    let smoke: Vec<String> = inputs
        .smoke
        .iter()
        .filter(|f| inputs.all.contains(*f))
        .cloned()
        .collect();
    selector.take(&smoke, "smoke", true, None);
    if !families.is_empty() {
        let pool: Vec<String> = inputs
            .all
            .iter()
            .filter(|f| {
                families
                    .iter()
                    .any(|fragment| f.contains(fragment.as_str()))
            })
            .cloned()
            .collect();
        let label = format!(
            "family of a changed path ({})",
            families.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        // Families get at most two thirds of the budget, so the broad sample
        // still reaches every other group.
        let budget = selector.budget;
        selector.budget = selector.spent + (budget - selector.spent.min(budget)) * 2 / 3;
        selector.take(&pool, &label, false, None);
        selector.budget = budget;
    }
    if reaches_compiler {
        let pool: Vec<String> = inputs.all.iter().cloned().collect();
        selector.take(&pool, "corpus-wide sample", false, Some(BROAD_HEAVY_MILLIS));
    }
    (None, selector.chosen)
}

/// Whether a shard is likely to need the base commit's runner, so the run
/// should build it once ahead of the shards.
///
/// A shard runs rows at the base only to excuse a row that is worse than the
/// ledger. A diff that can reach the compiler or touches a fixture can make
/// rows worse in every shard, so the base is built once for all of them. A
/// diff of neutral paths runs only the smoke rows and cannot move them; the
/// rare row that still needs the base (main itself lost it) is attributed by
/// that shard building the base itself, as every shard did before.
pub fn base_may_be_needed(
    changes: &[(char, String)],
    all: &BTreeSet<String>,
    sample_dependents: &[(String, String)],
) -> bool {
    changes.iter().any(|(_, path)| {
        classify(path) == PathClass::Compiler
            || owning_fixture(path, all).is_some()
            || sample_dependents
                .iter()
                .any(|(sample, _)| path.starts_with(&format!("{sample}/")))
    })
}

/// The runner selects by substring, so `pass/a/b` also runs `pass/a/b_c`.
/// Those extra fixtures are checked too, and named here so their cost and
/// their verdicts are not a surprise.
pub fn filter_closure(
    chosen: &BTreeMap<String, String>,
    all: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut closure = chosen.clone();
    for fixture in all {
        if closure.contains_key(fixture) {
            continue;
        }
        if let Some(name) = chosen.keys().find(|name| fixture.contains(name.as_str())) {
            closure.insert(
                fixture.clone(),
                format!("matched by the runner's filter for {name}"),
            );
        }
    }
    closure
}

/// Longest-first packing by recorded time, so shards finish together.
/// A fixture travels with every chosen name it contains, because the runner
/// would select it in that name's shard anyway.
pub fn pack(
    chosen: &BTreeMap<String, String>,
    millis: &BTreeMap<String, u64>,
    shards: usize,
) -> (Vec<Vec<String>>, Vec<u64>) {
    let cost = |f: &str| millis.get(f).copied().unwrap_or(UNKNOWN_MILLIS);
    // Group each fixture under the shortest chosen name it contains.
    let mut units: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for fixture in chosen.keys() {
        let anchor = chosen
            .keys()
            .filter(|name| fixture.contains(name.as_str()))
            .min_by_key(|name| (name.len(), name.as_str()))
            .map_or(fixture.as_str(), String::as_str);
        units.entry(anchor).or_default().push(fixture);
    }
    let mut units: Vec<(u64, Vec<&str>)> = units
        .into_values()
        .map(|members| (members.iter().map(|f| cost(f)).sum(), members))
        .collect();
    units.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut out: Vec<Vec<String>> = vec![Vec::new(); shards.max(1)];
    let mut load = vec![0u64; shards.max(1)];
    for (weight, members) in units {
        let index = (0..out.len()).min_by_key(|&i| (load[i], i)).unwrap_or(0);
        load[index] += weight;
        out[index].extend(members.into_iter().map(str::to_owned));
    }
    for shard in &mut out {
        shard.sort();
    }
    (out, load.into_iter().map(|ms| ms / 1000).collect())
}

fn changed_paths(root: &Path, base: &str) -> Result<Vec<(char, String)>, String> {
    let text = git(
        root,
        &["diff", "--name-status", "--no-renames", base, "HEAD"],
    )?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let (status, path) = line.split_once('\t')?;
            Some((status.chars().next()?, path.to_owned()))
        })
        .collect())
}

fn ledger_millis(root: &Path) -> BTreeMap<String, u64> {
    fs::read_to_string(root.join(LEDGER))
        .ok()
        .and_then(|text| corpus::parse_records(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.fixture, r.millis))
        .collect()
}

fn read_list(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

fn write_github_output(pairs: &[(&str, String)]) {
    let Some(path) = std::env::var_os("GITHUB_OUTPUT") else {
        return;
    };
    let mut text = fs::read_to_string(&path).unwrap_or_default();
    for (key, value) in pairs {
        text.push_str(&format!("{key}={value}\n"));
    }
    let _ = fs::write(path, text);
}

fn append_step_summary(markdown: &str) {
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let mut text = fs::read_to_string(&path).unwrap_or_default();
        text.push_str(markdown);
        text.push('\n');
        let _ = fs::write(path, text);
    }
}

fn run_plan(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let mut base = None;
    let mut shards = 8usize;
    let mut budget = 2200u64;
    let mut out = PathBuf::from("build/corpus-gate/plan.json");
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match (args[i].as_str(), value) {
            ("--base", Some(v)) => base = Some(v),
            ("--shards", Some(v)) => {
                shards = v
                    .parse()
                    .map_err(|_| usage_error("--shards takes a count"))?
            }
            ("--shard-secs", Some(v)) => {
                budget = v
                    .parse()
                    .map_err(|_| usage_error("--shard-secs takes seconds"))?
            }
            ("--out", Some(v)) => out = PathBuf::from(v),
            (other, _) => {
                return Err(usage_error(&format!(
                    "unknown or incomplete option `{other}`"
                )));
            }
        }
        i += 2;
    }
    let base = base.ok_or_else(|| usage_error("plan needs --base <rev>"))?;
    let base = git(&root, &["rev-parse", &base]).map_err(infra)?;
    let head = git(&root, &["rev-parse", "HEAD"]).map_err(infra)?;
    let changes = changed_paths(&root, &base).map_err(infra)?;
    let all: BTreeSet<String> =
        corpus::selected_fixture_names(&root, &crate::fixture_selection::FixtureSelection::All)
            .map_err(infra)?
            .into_iter()
            .collect();
    let millis = ledger_millis(&root);
    let smoke = read_list(&root.join(SMOKE_LIST));
    let path_map = parse_path_map(&fs::read_to_string(root.join(PATH_MAP)).unwrap_or_default());
    let dependents = sample_dependents(&root, &all);
    let inputs = PlanInputs {
        changes: &changes,
        all: &all,
        millis: &millis,
        smoke: &smoke,
        path_map: &path_map,
        sample_dependents: &dependents,
        shards,
        budget_secs_per_shard: budget,
    };
    let (skip, chosen) = plan_selection(&inputs);
    let chosen = filter_closure(&chosen, &all);
    // Use only as many shards as the selection needs: a smoke-only plan
    // should not hold eight runners.
    let total: u64 = chosen
        .keys()
        .map(|f| millis.get(f).copied().unwrap_or(UNKNOWN_MILLIS))
        .sum();
    let needed = (total.div_ceil(budget.max(1) * 1000) as usize).clamp(1, shards.max(1));
    let (packed, recorded_secs) = if skip.is_some() {
        (Vec::new(), Vec::new())
    } else {
        pack(&chosen, &millis, needed)
    };
    let packed: Vec<Vec<String>> = packed.into_iter().filter(|s| !s.is_empty()).collect();
    let recorded_secs: Vec<u64> = recorded_secs.into_iter().filter(|s| *s > 0).collect();
    let needs_base = skip.is_none() && base_may_be_needed(&changes, &all, &dependents);
    let plan = Plan {
        base,
        head,
        skip: skip.clone(),
        shards: packed,
        reasons: chosen,
        budget_secs_per_shard: budget,
        recorded_secs,
        needs_base,
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| infra(e.to_string()))?;
    }
    execution::write_json(&out, &plan).map_err(infra)?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for reason in plan.reasons.values() {
        let key = reason
            .split(['('])
            .next()
            .unwrap_or(reason)
            .split(" for ")
            .next()
            .unwrap_or(reason)
            .trim()
            .to_owned();
        *counts.entry(key).or_default() += 1;
    }
    let mut summary = String::from("## Corpus gate plan\n\n");
    if let Some(reason) = &skip {
        summary.push_str(&format!("Skipped: {reason}.\n"));
    } else {
        summary.push_str(&format!(
            "{} rows of {} in {} shards, {} changed paths. Recorded seconds per shard: {:?}.\n\n| Why chosen | Rows |\n| --- | ---: |\n",
            plan.reasons.len(),
            all.len(),
            plan.shards.len(),
            changes.len(),
            plan.recorded_secs
        ));
        for (reason, count) in &counts {
            summary.push_str(&format!("| {reason} | {count} |\n"));
        }
    }
    println!("{summary}");
    append_step_summary(&summary);
    let matrix: Vec<usize> = (0..plan.shards.len()).collect();
    write_github_output(&[
        ("skip", skip.is_some().to_string()),
        (
            "shards",
            serde_json::to_string(&matrix).unwrap_or_else(|_| "[]".to_owned()),
        ),
        ("needs_base", plan.needs_base.to_string()),
        ("base", plan.base.clone()),
    ]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Verdict {
    pub fixture: String,
    pub reason: String,
    pub ledger: Option<String>,
    pub observed: String,
    pub first_diagnostic: String,
    /// `regression`, `flaky`, `on-main` (nightly) or `on-base`.
    pub class: String,
}

fn describe(record: &CorpusRecord) -> String {
    let mut text = record.status.clone();
    if let Some(satisfied) = record.expected_satisfied {
        text.push_str(if satisfied {
            " expected"
        } else {
            " unexpected"
        });
    }
    for (k, v) in &record.facts {
        text.push_str(&format!(" {k}:{v}"));
    }
    text
}

fn first_diagnostic(record: &CorpusRecord) -> String {
    let line = record.diagnostics.first().map_or("", String::as_str);
    let line = line.lines().next().unwrap_or("");
    let mut cut: String = line.chars().take(200).collect();
    if cut.len() < line.len() {
        cut.push('…');
    }
    cut
}

/// The corpus runner for the commit checked out at `root`: the one in
/// `prebuilt`, refused unless its receipt names this exact clean commit and
/// its bytes, or one compiled here. A prebuilt runner that is missing or does
/// not match is an error, never a reason to compile or to skip the rows.
fn runner_for(
    root: &Path,
    prebuilt: Option<&Path>,
    receipts: &Path,
) -> Result<producer::Producer, String> {
    match prebuilt {
        Some(directory) => producer::load_prebuilt(root, directory, receipts),
        None => producer::prepare(root, receipts, true, false),
    }
}

/// Keep every baseline consumer on the same explicit source boundary. The
/// executable's compile-time repo_root is the caller, never this baseline.
fn with_baseline_source<T>(
    caller: &Path,
    revision: &str,
    operation: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let source = crate::worktree::BaselineSource::acquire(caller, revision)?;
    operation(&source.root)
}

/// The value after each `--flag` in `flags`, in order; any other argument is
/// a usage error. For the subcommands whose options all take one value.
fn flag_values(args: &[String], flags: &[&str]) -> Result<Vec<Option<String>>, String> {
    let mut values = vec![None; flags.len()];
    let mut pairs = args.chunks(2);
    for pair in &mut pairs {
        match (flags.iter().position(|flag| *flag == pair[0]), pair.get(1)) {
            (Some(index), Some(value)) => values[index] = Some(value.clone()),
            _ => return Err(format!("unknown or incomplete option `{}`", pair[0])),
        }
    }
    Ok(values)
}

/// Build the corpus runner once, for other jobs to run.
///
/// Writes the runner and its receipt into `--out`. With `--at <rev>` the
/// runner is built from that commit in an isolated pool slot with its own
/// target directory. The caller's checkout is never switched, even on failure.
fn run_build(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let values = flag_values(args, &["--out", "--at"]).map_err(|e| usage_error(&e))?;
    let [Some(out), at] = values.as_slice() else {
        return Err(usage_error("build needs --out <dir>"));
    };
    let out = PathBuf::from(out);
    if out.exists() {
        return Err(infra(format!(
            "{} already exists; a runner directory is written once",
            out.display()
        )));
    }
    let evidence = execution::reserve_directory(&root.join("build/corpus-gate")).map_err(infra)?;
    let receipts = evidence.join("producer");
    let build = |source: &Path, target: Option<&Path>| {
        let producer = producer::prepare_in_target(source, &receipts, true, false, target)?;
        producer::export_runner(&producer, &receipts, &out)
    };
    let result = match at {
        Some(rev) => with_baseline_source(&root, rev, |source| {
            build(source, Some(&source.join("target")))
        }),
        None => build(&root, None),
    };
    result.map_err(infra)?;
    println!(
        "corpus-gate: runner for {} written to {}",
        producer::prebuilt_revision(&out).map_err(infra)?,
        out.display()
    );
    Ok(())
}

struct Observer<'a> {
    root: &'a Path,
    out: &'a Path,
    runs: usize,
    target: Option<&'a Path>,
}

impl Observer<'_> {
    /// Observe exactly `names` at the checked-out commit, with the runner in
    /// `prebuilt` when the run built one for this commit, and otherwise with
    /// a runner built here.
    fn observe(
        &mut self,
        label: &str,
        names: &[String],
        prebuilt: Option<&Path>,
    ) -> Result<BTreeMap<String, CorpusRecord>, String> {
        self.runs += 1;
        let evidence = self.out.join(format!("{}-{label}", self.runs));
        let receipts = evidence.join("producer");
        let producer = match (prebuilt, self.target) {
            (None, Some(target)) => {
                producer::prepare_in_target(self.root, &receipts, true, false, Some(target))?
            }
            _ => runner_for(self.root, prebuilt, &receipts)?,
        };
        let binary = producer
            .runner
            .as_deref()
            .ok_or("producer built no runner")?;
        let text = corpus::observe(
            self.root,
            binary,
            &crate::fixture_selection::FixtureSelection::exact(names.iter().cloned())?,
            "",
            None,
            false,
            false,
            &evidence,
        )?;
        producer.verify()?;
        corpus::parse_records(&text)
            .map(record_map)
            .map_err(|e| format!("unreadable runner records: {e}"))
    }
}

fn run_check(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let mut plan_path = None;
    let mut shard = None;
    let mut known_path: Option<PathBuf> = None;
    let mut out = PathBuf::from("build/corpus-gate");
    let mut base_attribution = true;
    let mut runner: Option<PathBuf> = None;
    let mut base_runner: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--no-base" {
            base_attribution = false;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).cloned();
        match (args[i].as_str(), value) {
            ("--plan", Some(v)) => plan_path = Some(PathBuf::from(v)),
            ("--shard", Some(v)) => {
                shard = Some(
                    v.parse::<usize>()
                        .map_err(|_| usage_error("--shard takes an index"))?,
                )
            }
            ("--known", Some(v)) => known_path = Some(PathBuf::from(v)),
            ("--runner", Some(v)) => runner = Some(PathBuf::from(v)),
            ("--base-runner", Some(v)) => base_runner = Some(PathBuf::from(v)),
            ("--out", Some(v)) => out = PathBuf::from(v),
            (other, _) => {
                return Err(usage_error(&format!(
                    "unknown or incomplete option `{other}`"
                )));
            }
        }
        i += 2;
    }
    let plan_path = plan_path.ok_or_else(|| usage_error("check needs --plan <file>"))?;
    let plan: Plan =
        serde_json::from_slice(&fs::read(&plan_path).map_err(|e| infra(e.to_string()))?)
            .map_err(|e| infra(format!("unreadable plan: {e}")))?;
    let shard = shard.unwrap_or(0);
    let Some(names) = plan.shards.get(shard).filter(|_| plan.skip.is_none()) else {
        println!("corpus-gate: shard {shard} has nothing to check");
        return Ok(());
    };
    let head = git(&root, &["rev-parse", "HEAD"]).map_err(infra)?;
    if head != plan.head {
        return Err(infra(format!(
            "plan was made at {} but HEAD is {head}",
            plan.head
        )));
    }
    // A base runner is used only if a row needs excusing, so check now that
    // the one handed over is the base's: a wrong or missing one fails the
    // shard whether or not a row turns out to need it.
    if let Some(directory) = &base_runner {
        let built = producer::prebuilt_revision(directory)
            .map_err(|e| infra(format!("base runner {}: {e}", directory.display())))?;
        if built != plan.base {
            return Err(infra(format!(
                "base runner {} was built from {built}, but the plan's base is {}",
                directory.display(),
                plan.base
            )));
        }
    }
    fs::create_dir_all(&out).map_err(|e| infra(e.to_string()))?;
    let out = execution::reserve_directory(&out).map_err(infra)?;
    // The published ledger, when a copy was downloaded, is what the post-merge
    // check last measured; the file in the repository is no longer edited by
    // pull requests. No pull request rewrites a published row, so the base's
    // ledger is the same one.
    let published = corpus::published_ledger(&root);
    let ledger = read_records(published.as_deref().unwrap_or(&root.join(LEDGER))).map_err(infra)?;
    let base_ledger = match &published {
        Some(path) => {
            println!(
                "corpus-gate: comparing with the published ledger {}",
                path.display()
            );
            ledger.clone()
        }
        None => git(&root, &["show", &format!("{}:{LEDGER}", plan.base)])
            .ok()
            .and_then(|text| corpus::parse_records(&text).ok())
            .map(record_map)
            .unwrap_or_default(),
    };
    // A row the pull request itself rewrote is judged against its new line
    // only; main's state of the old line cannot excuse it.
    let unchanged_by_pr = |fixture: &str| match (ledger.get(fixture), base_ledger.get(fixture)) {
        (Some(head_row), Some(base_row)) => same_outcome(head_row, base_row),
        (None, None) => true,
        _ => false,
    };
    let known = match &known_path {
        Some(path) if path.is_file() => read_records(path).unwrap_or_else(|error| {
            eprintln!("corpus-gate: ignoring nightly observations: {error}");
            BTreeMap::new()
        }),
        _ => BTreeMap::new(),
    };
    let started = std::time::Instant::now();
    let mut observer = Observer {
        root: &root,
        out: &out,
        runs: 0,
        target: None,
    };
    let observed = observer
        .observe("head", names, runner.as_deref())
        .map_err(infra)?;
    let mut verdicts: Vec<Verdict> = Vec::new();
    let mut measured = observed.clone();
    let mut suspects: Vec<String> = observed
        .values()
        .filter(|row| worse(ledger.get(&row.fixture), row).is_some())
        .map(|row| row.fixture.clone())
        .collect();
    println!(
        "corpus-gate: shard {shard}: {} rows observed, {} worse than the ledger",
        observed.len(),
        suspects.len()
    );
    let verdict = |row: &CorpusRecord, class: &str| Verdict {
        fixture: row.fixture.clone(),
        reason: worse(ledger.get(&row.fixture), row).unwrap_or_default(),
        ledger: ledger.get(&row.fixture).map(describe),
        observed: describe(row),
        first_diagnostic: first_diagnostic(row),
        class: class.to_owned(),
    };
    if !suspects.is_empty() {
        let again = observer
            .observe("head-confirm", &suspects, runner.as_deref())
            .map_err(infra)?;
        suspects.retain(|fixture| {
            let confirmed = again
                .get(fixture)
                .is_none_or(|row| worse(ledger.get(fixture), row).is_some());
            if !confirmed {
                verdicts.push(verdict(&observed[fixture], "flaky"));
                // The second run is the row's measurement; the first was noise.
                if let Some(row) = again.get(fixture) {
                    measured.insert(fixture.clone(), row.clone());
                }
            }
            confirmed
        });
    }
    // What this shard measured at the head, in the ledger's own format, so a
    // post-merge run can splice it into the ledger (`sweep-report --records`).
    let rows: Vec<CorpusRecord> = measured.into_values().collect();
    fs::write(out.join("records.txt"), corpus::splice_records("", &rows))
        .map_err(|e| infra(e.to_string()))?;
    // Excused when main already shows the same loss and the row is the base's.
    let mut remaining = Vec::new();
    for fixture in suspects {
        let row = &observed[&fixture];
        let on_main = unchanged_by_pr(&fixture)
            && known.get(&fixture).is_some_and(|main_row| {
                worse(ledger.get(&fixture), main_row).is_some()
                    && worse(Some(main_row), row).is_none()
            });
        if on_main {
            verdicts.push(verdict(row, "on-main"));
        } else {
            remaining.push(fixture);
        }
    }
    if !remaining.is_empty() && base_attribution {
        let excusable: Vec<String> = remaining
            .iter()
            .filter(|f| unchanged_by_pr(f))
            .cloned()
            .collect();
        let mut at_base = BTreeMap::new();
        if !excusable.is_empty() {
            // Even a prebuilt runner reads baseline fixtures and library
            // sources, so observation uses the same isolated source lease as
            // baseline builds. Never switch the candidate under other readers.
            at_base = with_baseline_source(&root, &plan.base, |source| {
                let target = source.join("target");
                let mut baseline = Observer {
                    root: source,
                    out: &out,
                    runs: observer.runs,
                    target: Some(&target),
                };
                baseline.observe("base", &excusable, base_runner.as_deref())
            })
            .map_err(infra)?;
        }
        remaining.retain(|fixture| {
            let row = &observed[fixture];
            let on_base = excusable.contains(fixture)
                && at_base.get(fixture).is_some_and(|base_row| {
                    worse(ledger.get(fixture), base_row).is_some()
                        && worse(Some(base_row), row).is_none()
                });
            if on_base {
                verdicts.push(verdict(row, "on-base"));
            }
            !on_base
        });
    }
    for fixture in &remaining {
        verdicts.push(verdict(&observed[fixture], "regression"));
    }
    verdicts.sort_by(|a, b| {
        (a.class.as_str(), a.fixture.as_str()).cmp(&(b.class.as_str(), b.fixture.as_str()))
    });
    execution::write_json(&out.join("verdicts.json"), &verdicts).map_err(infra)?;
    let regressions = verdicts.iter().filter(|v| v.class == "regression").count();
    let mut summary = format!(
        "## Corpus gate shard {shard}\n\n{} rows checked in {} s at `{}` against base `{}`: **{regressions} regression(s)**.\n",
        observed.len(),
        started.elapsed().as_secs(),
        &head[..head.len().min(12)],
        &plan.base[..plan.base.len().min(12)]
    );
    for (class, heading) in [
        ("regression", "Worse than the ledger because of this change"),
        (
            "on-base",
            "Already worse on the base commit (not this change)",
        ),
        (
            "on-main",
            "Already worse on main in the nightly sweep (not this change)",
        ),
        (
            "flaky",
            "Worse once, as recorded on a second run (not counted)",
        ),
    ] {
        let rows: Vec<&Verdict> = verdicts.iter().filter(|v| v.class == class).collect();
        if rows.is_empty() {
            continue;
        }
        summary.push_str(&format!("\n### {heading}: {}\n\n| Fixture | Change | Ledger | Observed | First diagnostic |\n| --- | --- | --- | --- | --- |\n", rows.len()));
        for v in rows {
            summary.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                v.fixture,
                v.reason,
                v.ledger.as_deref().unwrap_or("none"),
                v.observed,
                v.first_diagnostic.replace('|', "\\|")
            ));
        }
    }
    println!("{summary}");
    append_step_summary(&summary);
    fs::write(out.join("summary.md"), &summary).map_err(|e| infra(e.to_string()))?;
    if regressions > 0 {
        eprintln!(
            "corpus-gate: {regressions} row(s) got worse than the ledger; see the table above"
        );
        Err(3)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Nightly sweep
// ---------------------------------------------------------------------------

fn run_sweep_shard(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let values =
        flag_values(args, &["--shard", "--out", "--runner"]).map_err(|e| usage_error(&e))?;
    let [Some(shard), Some(out), runner] = values.as_slice() else {
        return Err(usage_error("sweep-shard needs --shard <k/N> --out <file>"));
    };
    let evidence = execution::reserve_directory(&root.join("build/corpus-gate")).map_err(infra)?;
    let producer = runner_for(
        &root,
        runner.as_deref().map(Path::new),
        &evidence.join("producer"),
    )
    .map_err(infra)?;
    let binary = producer.runner.as_deref().ok_or(4u8)?;
    let text = corpus::observe(
        &root,
        binary,
        &crate::fixture_selection::FixtureSelection::fragments("pass/,fail/,run/"),
        shard,
        None,
        false,
        false,
        &evidence,
    )
    .map_err(infra)?;
    producer.verify().map_err(infra)?;
    corpus::parse_records(&text).map_err(|e| infra(format!("unreadable runner records: {e}")))?;
    fs::write(out, text).map_err(|e| infra(e.to_string()))?;
    println!("corpus-gate: shard {shard} records written to {out}");
    Ok(())
}

#[derive(Debug, Default)]
pub struct SweepReport {
    pub soundness: Vec<(CorpusRecord, Option<CorpusRecord>)>,
    pub lost_reason: Vec<(CorpusRecord, Option<CorpusRecord>)>,
    pub lost_success: Vec<(CorpusRecord, Option<CorpusRecord>)>,
    pub gained: Vec<(CorpusRecord, Option<CorpusRecord>)>,
    pub moved: Vec<(CorpusRecord, Option<CorpusRecord>)>,
}

/// Sort every observed row whose outcome differs from the ledger.
pub fn classify_sweep(
    ledger: &BTreeMap<String, CorpusRecord>,
    observed: &BTreeMap<String, CorpusRecord>,
) -> SweepReport {
    let mut report = SweepReport::default();
    for row in observed.values() {
        let before = ledger.get(&row.fixture);
        if before.is_some_and(|b| same_outcome(b, row)) {
            continue;
        }
        let entry = (row.clone(), before.cloned());
        let rejects = |r: &CorpusRecord| matches!(r.status.as_str(), "rejected" | "incomplete");
        match worse(before, row) {
            Some(_) if row.tier == "fail" && !rejects(row) => report.soundness.push(entry),
            Some(_) if row.tier == "fail" => report.lost_reason.push(entry),
            Some(_) => report.lost_success.push(entry),
            None if before.is_some_and(|b| worse(None, b).is_some())
                && worse(None, row).is_none() =>
            {
                report.gained.push(entry)
            }
            None => report.moved.push(entry),
        }
    }
    report
}

fn sweep_markdown(
    report: &SweepReport,
    commit: &str,
    shards_seen: usize,
    shards_expected: usize,
    observed: usize,
) -> String {
    let short = &commit[..commit.len().min(12)];
    let total = report.soundness.len()
        + report.lost_reason.len()
        + report.lost_success.len()
        + report.gained.len()
        + report.moved.len();
    let mut md = String::new();
    md.push_str(&format!(
        "The nightly corpus sweep measured every fixture on main at `{short}` and found {total} rows whose outcome differs from `tests/omega/corpus_outcomes.txt`. This pull request records them, so the ledger says what the compiler does today; it changes only the ledger and this description, so every row it records was already true on main (tools/testing.md, \"Who may record a regression\").\n\n"
    ));
    if report.soundness.is_empty() {
        md.push_str("**Soundness alerts: none.** No must-reject fixture is accepted.\n\n");
    } else {
        md.push_str(&format!(
            "## ⚠ Soundness alerts: {} must-reject fixture(s) now accepted\n\nThe compiler accepts these `fail/` programs. Recording them keeps the ledger honest; it does not make them acceptable. Each needs a repair, and none should be merged as a baseline without someone reading this list.\n\n| Fixture | Ledger | Observed |\n| --- | --- | --- |\n",
            report.soundness.len()
        ));
        for (row, before) in report.soundness.iter().take(300) {
            md.push_str(&format!(
                "| `{}` | {} | {} |\n",
                row.fixture,
                before.as_ref().map_or("none".to_owned(), describe),
                describe(row)
            ));
        }
        if report.soundness.len() > 300 {
            md.push_str(&format!(
                "\n{} more are in the ledger diff.\n",
                report.soundness.len() - 300
            ));
        }
        md.push('\n');
    }
    md.push_str(&format!(
        "| Change | Rows |\n| --- | ---: |\n| must-reject fixture accepted (soundness) | {} |\n| must-reject fixture lost its pinned reason | {} |\n| pass/run fixture lost its success | {} |\n| fixture gained its expected outcome | {} |\n| other movement (diagnostics, facts) | {} |\n\n",
        report.soundness.len(),
        report.lost_reason.len(),
        report.lost_success.len(),
        report.gained.len(),
        report.moved.len()
    ));
    let mut by_diagnostic: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (row, _) in report.lost_reason.iter().chain(&report.lost_success) {
        by_diagnostic
            .entry(first_diagnostic(row))
            .or_default()
            .push(&row.fixture);
    }
    if !by_diagnostic.is_empty() {
        let mut groups: Vec<_> = by_diagnostic.into_iter().collect();
        groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
        md.push_str("## Worse rows by first diagnostic\n\n| Rows | First diagnostic | Examples |\n| ---: | --- | --- |\n");
        for (diagnostic, fixtures) in groups.iter().take(60) {
            let examples: Vec<String> = fixtures.iter().take(3).map(|f| format!("`{f}`")).collect();
            let shown = if diagnostic.is_empty() {
                "(none)".to_owned()
            } else {
                diagnostic.replace('|', "\\|")
            };
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                fixtures.len(),
                shown,
                examples.join(", ")
            ));
        }
        if groups.len() > 60 {
            md.push_str(&format!(
                "\n{} smaller groups are not shown.\n",
                groups.len() - 60
            ));
        }
        md.push('\n');
    }
    md.push_str(&format!(
        "## Validation\n\n`.github/workflows/corpus-sweep.yml` ran `cargo xtask corpus-gate sweep-shard` on {shards_seen} of {shards_expected} shards at `{commit}` ({observed} rows observed) and `cargo xtask corpus-gate sweep-report` spliced the differing rows with `--record`'s rules: each carries the measured commit as its stamp, and a pass/run row that lost a success carries `regressed-from:`.{}\n\n",
        if shards_seen < shards_expected { " Missing shards left their rows untouched." } else { "" }
    ));
    md
}

fn run_sweep_report(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let get = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let records_dir =
        get("--records").ok_or_else(|| usage_error("sweep-report needs --records <dir>"))?;
    let body = get("--body").unwrap_or_else(|| "build/corpus-gate/sweep-body.md".to_owned());
    let observed_out =
        get("--observed").unwrap_or_else(|| "build/corpus-gate/observed.txt".to_owned());
    let expected: usize = get("--shards").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut observed: BTreeMap<String, CorpusRecord> = BTreeMap::new();
    let mut seen = 0usize;
    let mut entries: Vec<_> = fs::read_dir(&records_dir)
        .map_err(|e| infra(format!("read {records_dir}: {e}")))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        // Artifact downloads nest each shard's file in its own directory.
        let files: Vec<PathBuf> = if path.is_dir() {
            fs::read_dir(&path)
                .map_err(|e| infra(e.to_string()))?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .collect()
        } else {
            vec![path]
        };
        for file in files
            .into_iter()
            .filter(|f| f.extension().is_some_and(|e| e == "txt"))
        {
            observed.extend(read_records(&file).map_err(infra)?);
            seen += 1;
        }
    }
    let ledger_path = root.join(LEDGER);
    let ledger_text = fs::read_to_string(&ledger_path).map_err(|e| infra(e.to_string()))?;
    let ledger = corpus::parse_records(&ledger_text)
        .map(record_map)
        .map_err(infra)?;
    let report = classify_sweep(&ledger, &observed);
    let stamp = corpus::current_stamp(&root);
    if stamp.ends_with("+wt") {
        return Err(infra(
            "the checkout has changes besides the ledger; a sweep records only a clean commit"
                .to_owned(),
        ));
    }
    let mut changed: Vec<CorpusRecord> = Vec::new();
    for (row, before) in report
        .soundness
        .iter()
        .chain(&report.lost_reason)
        .chain(&report.lost_success)
        .chain(&report.gained)
        .chain(&report.moved)
    {
        let mut row = row.clone();
        row.regressed_from = corpus::carried_regression(before.as_ref(), &row);
        row.recorded_at = Some(stamp.clone());
        changed.push(row);
    }
    if !changed.is_empty() {
        fs::write(&ledger_path, corpus::splice_records(&ledger_text, &changed))
            .map_err(|e| infra(e.to_string()))?;
    }
    let commit = git(&root, &["rev-parse", "HEAD"]).map_err(infra)?;
    let markdown = sweep_markdown(&report, &commit, seen, expected.max(seen), observed.len());
    for path in [&body, &observed_out] {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| infra(e.to_string()))?;
        }
    }
    fs::write(&body, &markdown).map_err(|e| infra(e.to_string()))?;
    let all: Vec<CorpusRecord> = observed.into_values().collect();
    fs::write(&observed_out, corpus::splice_records("", &all)).map_err(|e| infra(e.to_string()))?;
    append_step_summary(&markdown);
    println!("{markdown}");
    write_github_output(&[
        ("changed", changed.len().to_string()),
        ("soundness", report.soundness.len().to_string()),
    ]);
    Ok(())
}

// ---------------------------------------------------------------------------

fn usage_error(message: &str) -> u8 {
    eprintln!("corpus-gate: {message}\n{USAGE}");
    2
}

fn infra(message: impl std::fmt::Display) -> u8 {
    eprintln!("corpus-gate: {message}");
    4
}

const USAGE: &str = "usage: cargo xtask corpus-gate plan --base <rev> [--shards N] [--shard-secs S] [--out plan.json]
       cargo xtask corpus-gate build --out <runner-dir> [--at <rev>]
       cargo xtask corpus-gate check --plan plan.json --shard K [--known observed.txt] [--no-base] [--out dir]
                                     [--runner <runner-dir>] [--base-runner <runner-dir>]
       cargo xtask corpus-gate sweep-shard --shard k/N --out records.txt [--runner <runner-dir>]
       cargo xtask corpus-gate sweep-report --records <dir> [--shards N] [--body body.md] [--observed observed.txt]";

pub fn run(args: &[String]) -> Result<(), u8> {
    match args.first().map(String::as_str) {
        Some("plan") => run_plan(&args[1..]),
        Some("build") => run_build(&args[1..]),
        Some("check") => run_check(&args[1..]),
        Some("sweep-shard") => run_sweep_shard(&args[1..]),
        Some("sweep-report") => run_sweep_report(&args[1..]),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(usage_error(
            "expected plan, build, check, sweep-shard or sweep-report",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_build_and_observation_preserve_live_caller_even_on_failure() {
        struct Scratch(PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let scratch = Scratch(
            execution::reserve_directory(&std::env::temp_dir().join("omega-baseline-tests"))
                .unwrap(),
        );
        let repository = scratch.0.join("repository");
        fs::create_dir(&repository).unwrap();
        git(&repository, &["init", "-q"]).unwrap();
        let hooks = scratch.0.join("hooks");
        fs::create_dir(&hooks).unwrap();
        for (key, value) in [
            ("user.name", "Baseline test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", hooks.to_str().unwrap()),
        ] {
            git(&repository, &["config", key, value]).unwrap();
        }
        fs::write(
            repository.join(".gitignore"),
            "/.agents/worktree/\n/target/\n",
        )
        .unwrap();
        fs::write(repository.join("fixture.omg"), "baseline fixture").unwrap();
        git(&repository, &["add", "."]).unwrap();
        git(&repository, &["commit", "-qm", "baseline"]).unwrap();
        let base = git(&repository, &["rev-parse", "HEAD"]).unwrap();
        fs::write(repository.join("fixture.omg"), "candidate fixture").unwrap();
        git(&repository, &["commit", "-qam", "candidate"]).unwrap();
        let head = git(&repository, &["rev-parse", "HEAD"]).unwrap();
        git(
            &repository,
            &["update-ref", "refs/remotes/origin/main", &head],
        )
        .unwrap();
        let caller = repository.join(".agents/worktree/worker-1");
        let held = repository.join(".agents/worktree/worker-2");
        for path in [&caller, &held] {
            git(
                &repository,
                &["worktree", "add", "--detach", path.to_str().unwrap(), &head],
            )
            .unwrap();
        }
        let held_git = PathBuf::from(git(&held, &["rev-parse", "--absolute-git-dir"]).unwrap());
        fs::write(held_git.join("topic"), "tools-other-active-build\n").unwrap();
        let before = producer::source_identity(&caller).unwrap();
        assert_eq!(before["tracked_changes"], ""); // Caller is a released, clean pool slot.
        let exported = scratch.0.join("runner");
        fs::create_dir(&exported).unwrap();
        let mut first_source = PathBuf::new();
        let receipt = with_baseline_source(&caller, &base, |source| {
            first_source = source.to_owned();
            assert_ne!(source, caller);
            assert_ne!(source, held);
            // Simulate a concurrent fixture build reading the caller while the
            // baseline consumer reads its own source. No polling or timing race.
            std::thread::scope(|scope| {
                let reader = scope.spawn(|| {
                    assert_eq!(producer::source_identity(&caller).unwrap(), before);
                    assert_eq!(
                        fs::read_to_string(caller.join("fixture.omg")).unwrap(),
                        "candidate fixture"
                    );
                });
                assert_eq!(
                    fs::read_to_string(source.join("fixture.omg")).unwrap(),
                    "baseline fixture"
                );
                reader.join().unwrap();
            });
            let identity = producer::source_identity(source)?;
            assert_eq!(identity["revision"], base);
            assert_eq!(identity["tracked_changes"], "");
            // Synthetic artifact exercises the real exporter and loader without
            // a compiler build. Its bytes come from the requested source.
            let runner = source.join("fixture.omg");
            let digest = execution::digest(&runner)?;
            let receipt = serde_json::json!({"source": identity, "host": producer::host_target(),
                "executables": [{"path": runner, "sha256": digest,
                "cargo_artifact": {"target": {"name": "corpus_runner"}, "executable": runner}}]});
            let receipts = scratch.0.join("build-receipts");
            fs::create_dir(&receipts).unwrap();
            execution::write_json(&receipts.join("producer.json"), &receipt)?;
            execution::write_json(&receipts.join("build-inputs.json"), &serde_json::json!({}))?;
            let prepared = producer::Producer {
                runner: Some(runner),
                cli: None,
                executables: receipt["executables"].as_array().unwrap().clone(),
            };
            producer::export_runner(&prepared, &receipts, &exported)?;
            Ok(receipt)
        })
        .unwrap();
        assert_eq!(producer::source_identity(&caller).unwrap(), before);
        // Hold that idle baseline slot to force observation into a different
        // checkout: receipts authenticate revision/tree/bytes, never slot paths.
        let first_git =
            PathBuf::from(git(&first_source, &["rev-parse", "--absolute-git-dir"]).unwrap());
        assert!(!first_git.join("topic").exists());
        fs::write(first_git.join("topic"), "tools-retained-other-work\n").unwrap();
        let mut second_source = PathBuf::new();
        let error = with_baseline_source(&caller, &base, |source| -> Result<(), String> {
            second_source = source.to_owned();
            assert_ne!(source, first_source);
            let loaded = producer::load_prebuilt(source, &exported, &scratch.0.join("receipts"))?;
            loaded.verify()?;
            assert_eq!(producer::source_identity(source)?, receipt["source"]);
            assert!(
                producer::load_prebuilt(&caller, &exported, &scratch.0.join("wrong-source"))
                    .is_err()
            );
            Err("simulated baseline build or observation failure".to_owned())
        })
        .unwrap_err();
        assert_eq!(error, "simulated baseline build or observation failure");
        let second_git =
            PathBuf::from(git(&second_source, &["rev-parse", "--absolute-git-dir"]).unwrap());
        assert!(!second_git.join("topic").exists());
        assert!(
            !git(&repository, &["worktree", "list", "--porcelain"])
                .unwrap()
                .contains("locked")
        );
        assert_eq!(producer::source_identity(&caller).unwrap(), before);
        assert_eq!(git(&held, &["rev-parse", "HEAD"]).unwrap(), head);
        assert_eq!(
            fs::read_to_string(held_git.join("topic")).unwrap(),
            "tools-other-active-build\n"
        );
        assert!(with_baseline_source(&caller, "absent-revision", |_| Ok(())).is_err());
        assert_eq!(producer::source_identity(&caller).unwrap(), before);
        #[cfg(unix)]
        {
            let alias = scratch.0.join("caller-alias");
            std::os::unix::fs::symlink(&caller, &alias).unwrap();
            with_baseline_source(&alias, &base, |source| {
                assert_ne!(
                    fs::canonicalize(source).unwrap(),
                    fs::canonicalize(&alias).unwrap()
                );
                assert_eq!(producer::source_identity(&caller)?, before);
                Ok(())
            })
            .unwrap();
        }
        fs::write(caller.join("fixture.omg"), "uncommitted caller edit").unwrap();
        let dirty = producer::source_identity(&caller).unwrap();
        with_baseline_source(&caller, &base, |source| {
            assert_eq!(source, second_source); // Reuse the released slot, not caller.
            assert_eq!(producer::source_identity(source)?, receipt["source"]);
            Ok(())
        })
        .unwrap();
        assert_eq!(producer::source_identity(&caller).unwrap(), dirty);
        assert_eq!(
            fs::read_to_string(caller.join("fixture.omg")).unwrap(),
            "uncommitted caller edit"
        );
    }

    fn row(text: &str) -> CorpusRecord {
        corpus::parse_records(text).unwrap().remove(0)
    }

    #[test]
    fn worse_names_the_three_losses_and_nothing_else() {
        let pass_ok = row("pass/a/b checked 10ms @abc");
        let pass_bad = row("pass/a/b rejected 10ms\n\tsomething");
        assert!(worse(Some(&pass_ok), &pass_bad).is_some());
        assert!(worse(Some(&pass_bad), &pass_ok).is_none());
        // A pass row already red is not made worse by a new diagnostic.
        assert!(worse(Some(&pass_bad), &row("pass/a/b timeout 10ms")).is_none());

        let fail_ok = row("fail/a/b rejected 10ms expected\n\treason");
        assert!(
            worse(Some(&fail_ok), &row("fail/a/b checked 10ms"))
                .unwrap()
                .contains("now checked")
        );
        assert!(
            worse(
                Some(&fail_ok),
                &row("fail/a/b rejected 10ms unexpected\n\tother")
            )
            .unwrap()
            .contains("pinned reason")
        );
        assert!(
            worse(
                Some(&fail_ok),
                &row("fail/a/b rejected 99ms expected\n\treworded")
            )
            .is_none()
        );
        // Unrecorded fixtures are held to their tier.
        assert!(worse(None, &row("fail/a/c checked 1ms")).is_some());
        assert!(worse(None, &row("pass/a/c rejected 1ms")).is_some());
        assert!(worse(None, &row("pass/a/c checked 1ms")).is_none());
        // Losing a matched native rejection is a lost success.
        let native = row("pass/a/n rejected 1ms expected native-rejection:match");
        assert!(
            worse(
                Some(&native),
                &row("pass/a/n rejected 1ms expected native-rejection:mismatch")
            )
            .is_some()
        );
    }

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn plan_skips_observation_only_diffs_and_samples_compiler_changes() {
        let all = names(&[
            "pass/a/x",
            "pass/a/x_local",
            "fail/a/x",
            "pass/b/y",
            "fail/c/z",
            "run/d/w",
        ]);
        let millis: BTreeMap<String, u64> = all.iter().map(|f| (f.clone(), 1000)).collect();
        let smoke = vec!["run/d/w".to_owned()];
        let map = parse_path_map("omega-rust/omega/packages/ pass/b/  # packages\n");
        let base = |changes: &[(char, String)], budget| {
            plan_selection(&PlanInputs {
                changes,
                all: &all,
                millis: &millis,
                smoke: &smoke,
                path_map: &map,
                sample_dependents: &[],
                shards: 1,
                budget_secs_per_shard: budget,
            })
        };
        let ledger_only = [
            ('M', "tests/omega/corpus_outcomes.txt".to_owned()),
            ('M', "wiki/x.md".to_owned()),
        ];
        assert!(base(&ledger_only, 100).0.is_some());

        let fixture = [('M', "tests/omega/pass/a/x/main.omg".to_owned())];
        let (skip, chosen) = base(&fixture, 100);
        assert!(skip.is_none());
        assert_eq!(
            chosen.keys().cloned().collect::<Vec<_>>(),
            vec!["fail/a/x", "pass/a/x", "run/d/w"]
        );

        let tooling = [('M', ".github/workflows/x.yml".to_owned())];
        assert_eq!(
            base(&tooling, 100).1.len(),
            1,
            "a neutral change runs only the smoke set"
        );

        let compiler = [(
            'M',
            "omega-rust/omega/packages/manager/src/lib.rs".to_owned(),
        )];
        let (_, chosen) = base(&compiler, 4);
        assert_eq!(
            chosen.get("pass/b/y").map(String::as_str),
            Some("family of a changed path (pass/b/)")
        );
        assert_eq!(chosen.len(), 4, "the sample fills the budget and stops");
        let groups: BTreeSet<&str> = chosen.keys().map(|f| group_of(f)).collect();
        assert!(
            groups.len() >= 3,
            "the sample spreads over groups: {groups:?}"
        );
    }

    #[test]
    fn closure_and_packing_keep_substring_matches_together() {
        let all = names(&["pass/a/x", "pass/a/x_local", "pass/b/y", "pass/c/z"]);
        let chosen: BTreeMap<String, String> = ["pass/a/x", "pass/b/y", "pass/c/z"]
            .iter()
            .map(|f| ((*f).to_owned(), "t".to_owned()))
            .collect();
        let closure = filter_closure(&chosen, &all);
        assert!(closure["pass/a/x_local"].contains("pass/a/x"));
        let millis: BTreeMap<String, u64> = [
            ("pass/a/x", 5000),
            ("pass/a/x_local", 5000),
            ("pass/b/y", 6000),
            ("pass/c/z", 3000),
        ]
        .iter()
        .map(|(f, ms)| ((*f).to_owned(), *ms))
        .collect();
        let (shards, secs) = pack(&closure, &millis, 2);
        assert_eq!(shards[0], vec!["pass/a/x", "pass/a/x_local"]);
        assert_eq!(shards[1], vec!["pass/b/y", "pass/c/z"]);
        assert_eq!(secs, vec![10, 9]);
    }

    #[test]
    fn sweep_puts_accepted_fail_rows_first() {
        let ledger = record_map(
            corpus::parse_records(
                "fail/a/x rejected 1ms expected\n\tr\npass/a/y checked 1ms\npass/a/z rejected 1ms\n\told\nfail/a/q rejected 1ms expected\n\tr\n",
            )
            .unwrap(),
        );
        let observed = record_map(
            corpus::parse_records(
                "fail/a/x checked 1ms\npass/a/y rejected 1ms\n\tbroke\npass/a/z checked 1ms\nfail/a/q rejected 2ms expected\n\tr\n",
            )
            .unwrap(),
        );
        let report = classify_sweep(&ledger, &observed);
        assert_eq!(report.soundness.len(), 1);
        assert_eq!(report.lost_success.len(), 1);
        assert_eq!(report.gained.len(), 1);
        assert!(report.moved.is_empty(), "timing alone is not a move");
        let md = sweep_markdown(&report, "0123456789abcdef", 2, 2, 4);
        let alert = md.find("Soundness alerts").unwrap();
        let table = md.find("by first diagnostic").unwrap();
        assert!(alert < table && md.contains("`fail/a/x`") && md.contains("| 1 | broke |"));
    }

    #[test]
    fn base_is_built_ahead_only_when_the_diff_can_move_rows() {
        let all = names(&["pass/a/x", "run/d/w"]);
        let dependents = [("samples/demo".to_owned(), "run/d/w".to_owned())];
        let needs = |path: &str| base_may_be_needed(&[('M', path.to_owned())], &all, &dependents);
        assert!(needs("omega-rust/psi/x/src/lib.rs"));
        assert!(needs("source/library/std/filesystem.omg"));
        assert!(needs("tests/omega/pass/a/x/main.omg"));
        assert!(needs("samples/demo/main.omg"));
        assert!(needs("brand-new-dir/x.rs"));
        assert!(!needs(".github/workflows/corpus-gate.yml"));
        assert!(!needs("tools/xtask/src/corpus_gate.rs"));
        assert!(!needs("samples/other/main.omg"));
        assert!(!base_may_be_needed(&[], &all, &dependents));
    }

    #[test]
    fn single_value_options_parse_in_any_order_and_refuse_the_rest() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let flags = ["--shard", "--out", "--runner"];
        assert_eq!(
            flag_values(&args(&["--out", "r.txt", "--shard", "3/64"]), &flags).unwrap(),
            vec![Some("3/64".to_owned()), Some("r.txt".to_owned()), None]
        );
        assert_eq!(
            flag_values(
                &args(&["--runner", "d", "--shard", "0/1", "--out", "o"]),
                &flags
            )
            .unwrap()[2]
                .as_deref(),
            Some("d")
        );
        assert!(flag_values(&args(&["--runner"]), &flags).is_err());
        assert!(flag_values(&args(&["--rnner", "d"]), &flags).is_err());
        assert!(flag_values(&args(&["--out", "o", "stray"]), &flags).is_err());
    }

    #[test]
    fn a_missing_prebuilt_runner_is_an_error_not_a_local_build() {
        let root = repo_root();
        let scratch = std::env::temp_dir().join(format!("omega-gate-test-{}", std::process::id()));
        let refusal = runner_for(
            &root,
            Some(&scratch.join("absent")),
            &scratch.join("evidence"),
        )
        .err()
        .unwrap();
        assert!(refusal.contains("prebuilt runner"), "{refusal}");
        assert!(!scratch.join("evidence").exists());
    }

    #[test]
    fn neutral_paths_stay_neutral_and_unknown_paths_reach_the_compiler() {
        assert_eq!(classify("wiki/spec/x.md"), PathClass::Neutral);
        assert_eq!(
            classify(".github/workflows/corpus-gate.yml"),
            PathClass::Neutral
        );
        assert_eq!(
            classify("tests/omega/corpus_native_linux_x86_64.txt"),
            PathClass::Neutral
        );
        assert_eq!(
            classify("source/library/std/filesystem.omg"),
            PathClass::Compiler
        );
        assert_eq!(classify("Cargo.lock"), PathClass::Compiler);
        assert_eq!(classify("tests/omega/README.md"), PathClass::Compiler);
        assert_eq!(classify("brand-new-dir/x.rs"), PathClass::Compiler);
    }
}
