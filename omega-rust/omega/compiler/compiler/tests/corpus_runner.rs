//! Corpus outcome runner — a `harness = false` test target that walks
//! `tests/omega/{pass,fail,run}` and compiles every fixture, printing one
//! JSON record per fixture to stdout: pass and run fixtures go through
//! `compiler::compile` with a `Check` product (run fixtures are compile-
//! checked only — the suite's execution leg is not reproduced here), fail
//! fixtures through `compile_to_checked` with reviewed repository package
//! inputs. `tools/corpus_gate.py` diffs those records against the recorded
//! golden so an agent's inner loop is one binary plus a corpus diff instead
//! of a chain of per-crate assertion suites.
//!
//! Running under nextest (`NEXTEST` is set by the harness) exits immediately
//! so this target never executes the corpus as one opaque "test".
//!
//! Knobs: `OMEGA_CORPUS_FIXTURE_FILTER` (comma-separated `tier/group/name`
//! fragments), `OMEGA_CORPUS_SHARD=k/N` (deterministic hash slice, stable
//! across fixture additions), `OMEGA_CORPUS_JOBS` (worker count, default host
//! parallelism capped at 12), `OMEGA_CORPUS_FIXTURE_SECS` (per-fixture cap,
//! default 120 — a timed-out fixture detaches its compile, records
//! `status: "timeout"`, and the pool applies back-pressure so stragglers
//! cannot multiply past the worker count).

use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    compile_to_checked,
};
use std::path::{Path, PathBuf};
use std::{env, fs, process};

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};

fn collect_mains(dir: &Path, mains: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("main.omg").is_file() {
                mains.push(path.join("main.omg"));
            } else {
                collect_mains(&path, mains);
            }
        }
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn diagnostics_json(errors: &[diagnostics::Diagnostic]) -> String {
    format!(
        "[{}]",
        errors
            .iter()
            .map(|error| format!("\"{}\"", escape(&error.message)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Records must be checkout-portable: diagnostics and panic payloads render
/// `path:line:col` positions with absolute fixture paths, so strip the repo
/// root prefix before a record reaches the golden.
fn sanitize(record: String) -> String {
    record.replace(&format!("{}/", repo_root().display()), "")
}

fn unique_build_dir(sequence: usize) -> PathBuf {
    env::temp_dir().join(format!("omega-corpus-runner-{}-{sequence}", process::id()))
}

/// The pass/run tier of `production_compile` restricted to the Check product:
/// reviewed package inputs plus `compiler::compile`, targetless.
fn check_pass_fixture(
    root_path: &Path,
    build_dir: PathBuf,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let options = CompileOptions {
        root_path: root_path.to_path_buf(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
    };
    let package_inputs = reviewed_repository_fixture_package_inputs(root_path, None)?;
    let mut request =
        CompileRequest::new(options).with_requested_product(RequestedCompileProduct::Check);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let result = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .map(|_| ());
    let _ = fs::remove_dir_all(&build_dir);
    result
}

/// `check_canary`: reviewed repository package inputs plus `compile_to_checked`.
fn check_fail_fixture(root_path: &Path) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, None);
    request.package_inputs = reviewed_repository_fixture_package_inputs(root_path, None)?;
    compile_to_checked(request).map(|_| ())
}

fn expected_fragment_satisfied(
    fixture_dir: &Path,
    errors: &[diagnostics::Diagnostic],
) -> Option<bool> {
    let expected = fs::read_to_string(fixture_dir.join("expected.txt")).ok()?;
    let rendered: Vec<String> = errors.iter().map(|error| error.message.clone()).collect();
    Some(
        expected
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .all(|fragment| rendered.iter().any(|text| text.contains(fragment))),
    )
}

fn run_one(tier: &str, base: &Path, main: &Path, sequence: usize) -> (String, u128) {
    let started = std::time::Instant::now();
    let fixture_dir = main.parent().unwrap_or(base).to_path_buf();
    let rel = fixture_dir
        .strip_prefix(base)
        .unwrap_or(&fixture_dir)
        .to_string_lossy()
        .replace('\\', "/");
    let outcome = match tier {
        "fail" => check_fail_fixture(main),
        _ => check_pass_fixture(main, unique_build_dir(sequence)),
    };
    let errors: &[diagnostics::Diagnostic] =
        outcome.as_ref().err().map(Vec::as_slice).unwrap_or(&[]);
    let (status, diagnostics) = match &outcome {
        Ok(()) => ("checked".to_string(), "[]".to_string()),
        Err(errors) => ("rejected".to_string(), diagnostics_json(errors)),
    };
    let satisfied = expected_fragment_satisfied(&fixture_dir, errors);
    let satisfied_json = satisfied
        .map(|value| if value { "true" } else { "false" })
        .unwrap_or("null");
    let millis = started.elapsed().as_millis();
    (
        sanitize(format!(
            "  {{\"fixture\": \"{tier}/{}\", \"tier\": \"{tier}\", \"status\": \"{status}\", \"millis\": {millis}, \"expected_satisfied\": {satisfied_json}, \"diagnostics\": {diagnostics}}}",
            escape(&rel)
        )),
        millis,
    )
}

/// A compile that outlives its budget still burns CPU: the worker waits on a
/// channel, never a join, so a straggler detaches instead of blocking the
/// pool. `live` counts in-flight compiles so the pool can apply back-pressure
/// (a straggler-heavy corpus otherwise cascades — detached compiles saturate
/// the box, slow every remaining fixture into its cap, and spawn more
/// stragglers). `process::exit` at the end reclaims whatever is still live.
fn run_one_bounded(
    tier: &'static str,
    base: &Path,
    main: &Path,
    sequence: usize,
    cap: std::time::Duration,
    live: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
    timed_out: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> (String, u128) {
    let started = std::time::Instant::now();
    let main_owned = main.to_path_buf();
    let base_owned = base.to_path_buf();
    let fixture_dir = main_owned.parent().unwrap_or(&base_owned).to_path_buf();
    let rel = fixture_dir
        .strip_prefix(&base_owned)
        .unwrap_or(&fixture_dir)
        .to_string_lossy()
        .replace('\\', "/");
    let (sender, receiver) = std::sync::mpsc::channel();
    live.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let live_owned = std::sync::Arc::clone(live);
    let rel_owned = rel.clone();
    std::thread::spawn(move || {
        // catch_unwind keeps a panicking compile from killing the thread
        // silently — the crash becomes a record carrying the panic text, so
        // the golden diff shows *why* instead of just "crashed".
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_one(tier, &base_owned, &main_owned, sequence)
        }));
        live_owned.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        let record = match outcome {
            Ok(record) => record,
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
                    .unwrap_or_else(|| "unknown panic".to_string());
                (
                    sanitize(format!(
                        "  {{\"fixture\": \"{tier}/{}\", \"tier\": \"{tier}\", \"status\": \"crashed\", \"millis\": 0, \"expected_satisfied\": null, \"diagnostics\": [\"{}\"]}}",
                        escape(&rel_owned),
                        escape(&message)
                    )),
                    0,
                )
            }
        };
        let _ = sender.send(record);
    });
    match receiver.recv_timeout(cap) {
        Ok(record) => record,
        Err(error) => {
            let (status, millis) = match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => {
                    timed_out.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    ("timeout", started.elapsed().as_millis())
                }
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    // The compile thread panicked inside run_one — it died
                    // before its own fetch_sub, so decrement here to keep the
                    // back-pressure count honest. Distinct status: a crash is
                    // a bug, not a budget overrun.
                    live.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    ("crashed", started.elapsed().as_millis())
                }
            };
            (
                sanitize(format!(
                    "  {{\"fixture\": \"{tier}/{}\", \"tier\": \"{tier}\", \"status\": \"{status}\", \"millis\": {millis}, \"expected_satisfied\": null, \"diagnostics\": []}}",
                    escape(&rel)
                )),
                millis,
            )
        }
    }
}

/// `OMEGA_CORPUS_FIXTURE_FILTER`: comma-separated trimmed substrings matched
/// against `tier/group/name` (`termination` selects both pass and fail
/// termination fixtures; `fail/proofs` scopes to one tier+group). A set value
/// matching nothing fails rather than running empty — the canary filters use
/// the same convention.
fn fixture_filter() -> Option<Vec<String>> {
    let fragments: Vec<String> = env::var("OMEGA_CORPUS_FIXTURE_FILTER")
        .ok()?
        .split(',')
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .map(str::to_string)
        .collect();
    (!fragments.is_empty()).then_some(fragments)
}

/// `OMEGA_CORPUS_SHARD=k/N`: keep fixtures whose FNV-1a hash of
/// `tier/group/name` lands in shard `k` of `N`. Hashing the path (not the job
/// index) keeps shards stable as fixtures are added or renamed, so several
/// sessions or machines can record/diff disjoint slices of one golden.
fn fixture_shard() -> Option<(u64, u64)> {
    let value = env::var("OMEGA_CORPUS_SHARD").ok()?;
    let (k, n) = value.split_once('/')?;
    let (k, n) = (k.trim().parse().ok()?, n.trim().parse().ok()?);
    (n >= 1 && k < n).then_some((k, n))
}

fn shard_of(name: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// `OMEGA_CORPUS_TIMINGS=<path>`: a prior records file (the gate passes its
/// golden) supplying per-fixture `millis`. The pool then schedules heavy
/// fixtures first — longest-processing-time ordering overlaps capped
/// stragglers with the tail of small fixtures instead of serializing them at
/// the end. Execution order only: records stay alphabetical for stable
/// golden diffs.
fn prior_millis() -> std::collections::HashMap<String, u64> {
    let mut millis = std::collections::HashMap::new();
    let Some(path) = env::var_os("OMEGA_CORPUS_TIMINGS") else {
        return millis;
    };
    let Ok(text) = fs::read_to_string(&path) else {
        eprintln!("corpus_runner: could not read OMEGA_CORPUS_TIMINGS {path:?}");
        return millis;
    };
    // Fields may share one line (the runner's compact records) or sit on
    // their own lines (json.dumps indent=1) — associate a millis field with
    // the most recently seen fixture field instead of requiring one line.
    let mut current_fixture: Option<String> = None;
    for line in text.lines() {
        if let Some(fixture) = line
            .split("\"fixture\": \"")
            .nth(1)
            .and_then(|rest| rest.split('\"').next())
        {
            current_fixture = Some(fixture.to_string());
        }
        if let Some(ms) = line
            .split("\"millis\": ")
            .nth(1)
            .and_then(|rest| rest.split([',', ' ', '}']).next())
            .and_then(|value| value.parse::<u64>().ok())
            && let Some(fixture) = current_fixture.take()
        {
            millis.insert(fixture, ms);
        }
    }
    eprintln!("corpus_runner: {} prior timings loaded", millis.len());
    millis
}

fn main() {
    if env::var_os("NEXTEST").is_some() {
        return;
    }
    let root = repo_root();
    let workers = env::var("OMEGA_CORPUS_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|count| count.get().clamp(1, 12))
                .unwrap_or(4)
        });
    let shard = fixture_shard();
    let cap = env::var("OMEGA_CORPUS_FIXTURE_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(120));
    let filter = fixture_filter();
    let prior = prior_millis();
    let mut jobs: Vec<(&'static str, PathBuf, PathBuf, std::time::Duration)> = Vec::new();
    for tier in ["pass", "fail", "run"] {
        let base = root.join("tests/omega").join(tier);
        let mut mains = Vec::new();
        collect_mains(&base, &mut mains);
        mains.sort();
        for main in mains {
            let candidate = format!(
                "{tier}/{}",
                main.parent()
                    .unwrap()
                    .strip_prefix(&base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            if let Some(fragments) = &filter
                && !fragments
                    .iter()
                    .any(|fragment| candidate.contains(fragment))
            {
                continue;
            }
            if let Some((k, n)) = shard
                && shard_of(&candidate) % n != k
            {
                continue;
            }
            // The cap is a wedge detector, not a regression boundary: a
            // fixture's real time inflates under pool contention, so a known
            // fixture gets at least 3x its recorded time before "timeout" —
            // a flap near the boundary then needs a genuine slowdown, not a
            // busier box.
            let fixture_cap = prior
                .get(&candidate)
                .map(|ms| cap.max(std::time::Duration::from_millis(ms * 3)))
                .unwrap_or(cap);
            jobs.push((tier, base.clone(), main, fixture_cap));
        }
    }
    if jobs.is_empty() && (filter.is_some() || shard.is_some()) {
        eprintln!("corpus_runner: filter/shard matched no fixtures");
        process::exit(2);
    }
    // Execution order: alphabetical. Heavy-first (LPT) was measured *worse*
    // here — these compiles are memory-bound, so packing the heavy fixtures
    // together inflated every wall time ~3x and pushed them over their caps;
    // alphabetical interleaves heavy groups with light ones naturally.
    // `order` still exists so record output stays alphabetical even if a
    // future scheduler reorders execution.
    let order: Vec<usize> = (0..jobs.len()).collect();
    let order = std::sync::Arc::new(order);
    let jobs = std::sync::Arc::new(jobs);
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let live = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total = jobs.len();
    let records = std::sync::Arc::new(std::sync::Mutex::new(vec![String::new(); jobs.len()]));
    let mut workers_joined = Vec::new();
    for _ in 0..workers.min(jobs.len().max(1)) {
        let records = std::sync::Arc::clone(&records);
        let next = std::sync::Arc::clone(&next);
        let jobs = std::sync::Arc::clone(&jobs);
        let order = std::sync::Arc::clone(&order);
        let live = std::sync::Arc::clone(&live);
        let timed_out = std::sync::Arc::clone(&timed_out);
        let done = std::sync::Arc::clone(&done);
        workers_joined.push(std::thread::spawn(move || {
            loop {
                // Back-pressure: never leave more than `workers` compiles in
                // flight, stragglers included. Give up after 2*cap so genuinely
                // wedged compiles stall the pool only temporarily, not forever.
                let mut waited = std::time::Duration::ZERO;
                while live.load(std::sync::atomic::Ordering::Relaxed) >= workers && waited < cap * 2
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    waited += std::time::Duration::from_millis(50);
                }
                let slot = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if slot >= order.len() {
                    break;
                }
                let index = order[slot];
                let (tier, base, main, fixture_cap) = &jobs[index];
                let (record, _millis) =
                    run_one_bounded(tier, base, main, index, *fixture_cap, &live, &timed_out);
                records.lock().unwrap()[index] = record;
                let finished = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if finished.is_multiple_of(200) || finished == total {
                    eprintln!(
                        "corpus_runner: {finished}/{total} done, \
                         {} compile(s) in flight",
                        live.load(std::sync::atomic::Ordering::Relaxed)
                    );
                }
            }
        }));
    }
    for worker in workers_joined {
        let _ = worker.join();
    }
    let timed_out = timed_out.load(std::sync::atomic::Ordering::Relaxed);
    let still_live = live.load(std::sync::atomic::Ordering::Relaxed);
    if timed_out > 0 || still_live > 0 {
        eprintln!(
            "corpus_runner: {timed_out} fixture(s) timed out, \
             {still_live} compile(s) still in flight at exit"
        );
    }
    let records = std::sync::Arc::try_unwrap(records)
        .unwrap_or_else(|records| records.lock().unwrap().clone().into())
        .into_inner()
        .unwrap();
    println!("[\n{}\n]", records.join(",\n"));
    // Detached stragglers from timed-out fixtures die with the process here.
    process::exit(0);
}
