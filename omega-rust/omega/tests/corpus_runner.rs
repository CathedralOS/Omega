//! Corpus outcome runner — a `harness = false` test target that walks
//! `tests/omega/{pass,fail,run}` and compiles every fixture, printing one
//! text record per fixture to stdout (format below): pass and run fixtures go through
//! `omega::compiler::compile` with a `Check` product (run fixtures are compile-
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
//! default 120 — a timed-out fixture detaches its compile, records status
//! `timeout`, and the pool applies back-pressure so stragglers cannot multiply
//! past the worker count).
//!
//! `OMEGA_CORPUS_NATIVE=1` switches to the native leg for the host target:
//! every pass and run fixture is built as a published native executable
//! (status `built`), fail fixtures are skipped, and run fixtures and pass
//! fixtures named `*_exit` are executed in a fresh temporary directory with
//! `input.txt` as stdin. Their records gain `exit:<code>` (or `exit:timeout`,
//! `exit:signal`) and, beside an `expected_stdout.txt`, `stdout:match` or
//! `stdout:differs`.
//!
//! `OMEGA_CORPUS_INTERPRET=1` also executes each runnable fixture (the run
//! tier and `*_exit` pass fixtures) on the checked interpreter that
//! `omega run --both` uses, for the host target (or the first target the
//! fixture's build binds), with `input.txt` as stdin. The record gains
//! `interpreter-exit:<code>` and, beside an `expected_stdout.txt`,
//! `interpreter-stdout:match` or `interpreter-stdout:differs`; a run the
//! interpreter declines records `interpreter:declined`, and a program that
//! does not check for it records `interpreter:unchecked`. With the native leg
//! as well, the record gains `interpreter:agrees` or `interpreter:disagrees`
//! comparing the two exit codes.
//!
//! Record format, read by `tools/corpus_records.py`: one line per fixture,
//! `<tier/group/name> <status> <milliseconds>ms`, followed by ` expected` or
//! ` unexpected` when the fixture's `expected.txt` fragments were weighed
//! against its diagnostics, then any `key:value` execution facts. Each
//! diagnostic follows on its own line after one
//! tab, with backslash, newline, carriage return and tab escaped as `\\`,
//! `\n`, `\r` and `\t`.

use omega::compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    compile_to_checked,
};
use std::path::{Path, PathBuf};
use std::{env, fs, process};

// Worker threads compile fixtures in parallel, and every compile allocates
// heavily. The macOS system allocator serializes those threads on its zone
// locks; mimalloc's per-thread heaps do not, which halves the corpus legs'
// wall time. Compiler output is unchanged: the records match either way.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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

/// One diagnostic on one line: the record format ends a diagnostic at the
/// line break, so the characters that would break it are escaped.
fn escape_diagnostic(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// One fixture's record: its header line, then one tab-indented line per
/// diagnostic.
fn record_text(
    tier: &str,
    relative: &str,
    status: &str,
    millis: u128,
    expected: Option<bool>,
    facts: &[String],
    diagnostics: &[String],
) -> String {
    let mut record = format!("{tier}/{relative} {status} {millis}ms");
    match expected {
        Some(true) => record.push_str(" expected"),
        Some(false) => record.push_str(" unexpected"),
        None => {}
    }
    for fact in facts {
        record.push(' ');
        record.push_str(fact);
    }
    for diagnostic in diagnostics {
        record.push_str("\n\t");
        record.push_str(&escape_diagnostic(diagnostic));
    }
    sanitize(record)
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
/// reviewed package inputs plus `omega::compiler::compile`, targetless.
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
    let result = omega::compiler::compile(request)
        .and_then(omega::compiler::CompileOutcomes::into_single_report)
        .map(|_| ());
    let _ = fs::remove_dir_all(&build_dir);
    result
}

/// The host target the native leg builds for, when `OMEGA_CORPUS_NATIVE` is set.
fn native_target() -> Option<&'static str> {
    env::var_os("OMEGA_CORPUS_NATIVE")?;
    target::TargetProfile::host_if_supported().map(|profile| profile.target_name())
}

/// Whether runnable fixtures also execute on the checked interpreter.
fn interpreter_leg() -> bool {
    env::var_os("OMEGA_CORPUS_INTERPRET").is_some()
}

/// Interpret one fixture's selected program entry, as `omega run --both`
/// does, and report its exit and stdout facts.
fn interpret_fixture(root_path: &Path, fixture_dir: &Path) -> Vec<String> {
    let Some(target) = target::TargetProfile::host_if_supported()
        .map(|profile| profile.target_name().to_owned())
        .or_else(|| declared_realization_target(root_path))
    else {
        return vec!["interpreter:unchecked".to_owned()];
    };
    let mut request = CheckedCompileRequest::new(root_path, Some(target.as_str()));
    let Ok(package_inputs) = reviewed_repository_fixture_package_inputs(root_path, Some(&target))
    else {
        return vec!["interpreter:unchecked".to_owned()];
    };
    request.package_inputs = package_inputs;
    let Ok(checked) = compile_to_checked(request) else {
        return vec!["interpreter:unchecked".to_owned()];
    };
    let Some(entry) = checked.selected_program_entry() else {
        return vec!["interpreter:unchecked".to_owned()];
    };
    let stdin = fs::read(fixture_dir.join("input.txt")).unwrap_or_default();
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        checked_interpreter::BuildMachineEntry::Symbol(entry.source_signature().machine_symbol()),
        &stdin,
        checked_interpreter::InterpretOptions::default(),
    );
    if outcome.error.is_some() {
        return vec!["interpreter:declined".to_owned()];
    }
    let mut facts = vec![format!("interpreter-exit:{}", outcome.exit_code)];
    if let Ok(expected) = fs::read_to_string(fixture_dir.join("expected_stdout.txt")) {
        let actual = String::from_utf8_lossy(&outcome.stdout).replace("\r\n", "\n");
        facts.push(if actual == expected.replace("\r\n", "\n") {
            "interpreter-stdout:match".to_owned()
        } else {
            "interpreter-stdout:differs".to_owned()
        });
    }
    facts
}

/// With both legs, whether the native and interpreted exit codes agree.
fn interpreter_agreement(facts: &[String]) -> Option<String> {
    let native = facts.iter().find_map(|fact| fact.strip_prefix("exit:"))?;
    let interpreted = facts
        .iter()
        .find_map(|fact| fact.strip_prefix("interpreter-exit:"))?;
    Some(if native == interpreted {
        "interpreter:agrees".to_owned()
    } else {
        "interpreter:disagrees".to_owned()
    })
}

/// Build one fixture as a published native executable for `target`.
fn build_native_fixture(
    root_path: &Path,
    build_dir: &Path,
    target: &str,
) -> Result<PathBuf, Vec<diagnostics::Diagnostic>> {
    let options = CompileOptions {
        root_path: root_path.to_path_buf(),
        build_dir: Some(build_dir.to_path_buf()),
        target_name: Some(target.to_owned()),
    };
    let package_inputs = reviewed_repository_fixture_package_inputs(root_path, Some(target))?;
    let mut request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = omega::compiler::compile(request)
        .and_then(omega::compiler::CompileOutcomes::into_single_report)?;
    let published = report
        .publish_retained_native_artifact(build_dir)
        .map_err(|error| vec![diagnostics::Diagnostic::error(error)])?;
    published
        .checked_native_executable_path()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "published native artifact names no executable",
            )]
        })
}

/// Run one built fixture in a fresh directory, stdin from `input.txt`, and
/// report its exit and, beside an `expected_stdout.txt`, whether stdout
/// matched it (line endings normalized).
fn execute_fixture(executable: &Path, fixture_dir: &Path, sequence: usize) -> Vec<String> {
    let work = env::temp_dir().join(format!("omega-corpus-run-{}-{sequence}", process::id()));
    let _ = fs::remove_dir_all(&work);
    if fs::create_dir_all(&work).is_err() {
        return vec!["exit:unrunnable".to_owned()];
    }
    let stdin = fs::File::open(fixture_dir.join("input.txt"))
        .map(std::process::Stdio::from)
        .unwrap_or_else(|_| std::process::Stdio::null());
    let spawned = process::Command::new(executable)
        .current_dir(&work)
        .stdin(stdin)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        let _ = fs::remove_dir_all(&work);
        return vec!["exit:unrunnable".to_owned()];
    };
    let mut stdout = child.stdout.take().expect("piped stdout");
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut bytes);
        bytes
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break status
                    .code()
                    .map_or_else(|| "exit:signal".to_owned(), |code| format!("exit:{code}"));
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break "exit:timeout".to_owned();
            }
        }
    };
    let output = reader.join().unwrap_or_default();
    let _ = fs::remove_dir_all(&work);
    let mut facts = vec![exit];
    if let Ok(expected) = fs::read_to_string(fixture_dir.join("expected_stdout.txt")) {
        let actual = String::from_utf8_lossy(&output).replace("\r\n", "\n");
        facts.push(if actual == expected.replace("\r\n", "\n") {
            "stdout:match".to_owned()
        } else {
            "stdout:differs".to_owned()
        });
    }
    facts
}

/// `check_canary`: reviewed repository package inputs plus `compile_to_checked`.
fn check_fail_fixture(root_path: &Path) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut request = CheckedCompileRequest::new(root_path, None);
    request.package_inputs = reviewed_repository_fixture_package_inputs(root_path, None)?;
    compile_to_checked(request).map(|_| ())
}

/// A fail fixture whose refusal belongs to a stage after checking -- a layout
/// size overflow, target lowering, provider selection -- checks clean on the
/// targetless route above and would otherwise be recorded as having stopped
/// rejecting. Realize it for the host before concluding that: the rejection is
/// real, it simply lives past where `compile_to_checked` stops.
fn realize_fail_fixture(
    root_path: &Path,
    build_dir: PathBuf,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let options = CompileOptions {
        root_path: root_path.to_path_buf(),
        build_dir: Some(build_dir.clone()),
        target_name: declared_realization_target(root_path),
    };
    let package_inputs = reviewed_repository_fixture_package_inputs(root_path, None)?;
    let mut request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let result = omega::compiler::compile(request)
        .and_then(omega::compiler::CompileOutcomes::into_single_report)
        .map(|_| ());
    let _ = fs::remove_dir_all(&build_dir);
    result
}

/// Every target the fixture's own `build.omg` binds a program-entry root for,
/// in declaration order. Empty for a fixture with no build declaration or no
/// bound root: such a fixture claims no realizable program, and asking for one
/// answers "native-artifact production requires one exact selected program
/// entry", which is this harness demanding an entry the fixture never
/// declared.
fn declared_realization_targets(root_path: &Path) -> Vec<String> {
    let Some(parent) = root_path.parent() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(parent.join("build.omg")) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for occurrence in text.split("roots.bind(").skip(1) {
        let Some(end) = occurrence.find("::") else {
            continue;
        };
        let target = occurrence[..end].trim();
        if !target.is_empty() && !targets.iter().any(|seen| seen == target) {
            targets.push(target.to_owned());
        }
    }
    targets
}

/// The first target the fixture's own `build.omg` binds a program-entry root
/// for. A fixture that declares only `uefi_x86_64` or `windows_x86_64` refuses
/// an absent `--target` with "selected target `<host>` has no bound required
/// root slot", which is this harness choosing a target the fixture never
/// declared, not the refusal the fixture pins. `None` keeps the host default
/// for a fixture with no build declaration.
fn declared_realization_target(root_path: &Path) -> Option<String> {
    declared_realization_targets(root_path).into_iter().next()
}

/// The target the native leg realizes one pass/run fixture for, and whether
/// the product can also execute here. A fixture binding the host runs; one
/// binding only foreign targets is still realized, for its declared target,
/// because target lowering and emission are most of what the leg measures.
fn pass_realization_target(root_path: &Path, host: &str) -> Option<(String, bool)> {
    let declared = declared_realization_targets(root_path);
    if declared.iter().any(|target| target == host) {
        return Some((host.to_owned(), true));
    }
    declared.into_iter().next().map(|target| (target, false))
}

/// Whether the fixture's expected fragments all appear in the diagnostics it
/// produced, or `None` when the question does not arise.
///
/// It does not arise for a fixture with no `expected.txt`, and it does not
/// arise for one that produced NO diagnostics: this route compiles targetless
/// through check and checked-compile, so an expectation naming a later stage
/// -- "native artifact target lowering failed", "requires a selected Fused
/// provider", a layout size overflow -- has nothing here to match and is not
/// unsatisfied, merely unevaluated. Answering `false` for those conflated them
/// with a fixture that rejected for the WRONG reason, which is the condition
/// this field exists to report. A fail-tier fixture that stops rejecting is
/// still visible: its `status` moves to `checked` against the golden.
fn expected_fragment_satisfied(
    fixture_dir: &Path,
    errors: &[diagnostics::Diagnostic],
) -> Option<bool> {
    let expected = fs::read_to_string(fixture_dir.join("expected.txt")).ok()?;
    if errors.is_empty() {
        return None;
    }
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
    let mut facts = Vec::new();
    // A pass/run fixture is realized only for a target it binds a program
    // entry for. Realizing one that binds none, or binding the host in its
    // place, reports this harness's own selection rather than the fixture's
    // behavior.
    let realization = native_target().and_then(|host| pass_realization_target(main, host));
    let mut realized = false;
    let mut outcome = match (tier, &realization) {
        ("fail", _) => check_fail_fixture(main),
        (_, Some((target, executes_here))) => {
            realized = true;
            let build_dir = unique_build_dir(sequence);
            let built = build_native_fixture(main, &build_dir, target);
            if let Ok(executable) = &built
                && *executes_here
                && (tier == "run" || rel.ends_with("_exit"))
            {
                facts = execute_fixture(executable, &fixture_dir, sequence);
            }
            let _ = fs::remove_dir_all(&build_dir);
            built.map(|_| ())
        }
        _ => check_pass_fixture(main, unique_build_dir(sequence)),
    };
    if tier == "fail" && outcome.is_ok() {
        outcome = realize_fail_fixture(main, unique_build_dir(sequence));
    }
    if interpreter_leg() && tier != "fail" && (tier == "run" || rel.ends_with("_exit")) {
        facts.extend(interpret_fixture(main, &fixture_dir));
        if let Some(agreement) = interpreter_agreement(&facts) {
            facts.push(agreement);
        }
    }
    let errors: &[diagnostics::Diagnostic] =
        outcome.as_ref().err().map(Vec::as_slice).unwrap_or(&[]);
    let status = match (&outcome, realized) {
        (Ok(()), true) => "built",
        (Ok(()), false) => "checked",
        (Err(_), _) => "rejected",
    };
    let diagnostics = errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>();
    let satisfied = expected_fragment_satisfied(&fixture_dir, errors);
    let millis = started.elapsed().as_millis();
    (
        record_text(tier, &rel, status, millis, satisfied, &facts, &diagnostics),
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
                    record_text(tier, &rel_owned, "crashed", 0, None, &[], &[message]),
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
                record_text(tier, &rel, status, millis, None, &[], &[]),
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
    // A header line is `<fixture> <status> <milliseconds>ms ...`; comment and
    // tab-indented diagnostic lines carry no timing.
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with('\t') {
            continue;
        }
        let mut fields = line.split(' ');
        if let (Some(fixture), Some(_status), Some(ms)) =
            (fields.next(), fields.next(), fields.next())
            && let Some(value) = ms.strip_suffix("ms").and_then(|ms| ms.parse::<u64>().ok())
        {
            millis.insert(fixture.to_string(), value);
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
    let tiers: &[&'static str] = if native_target().is_some() {
        &["pass", "run"]
    } else {
        &["pass", "fail", "run"]
    };
    if env::var_os("OMEGA_CORPUS_NATIVE").is_some() && native_target().is_none() {
        eprintln!("corpus_runner: this host has no catalogued Omega target to build for");
        process::exit(2);
    }
    for &tier in tiers {
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
    for record in records {
        println!("{record}");
    }
    // Detached stragglers from timed-out fixtures die with the process here.
    process::exit(0);
}
