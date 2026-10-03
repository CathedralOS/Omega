//! Corpus outcome runner — a `harness = false` test target that walks
//! `tests/omega/{pass,fail,run}` and compiles every fixture, printing one
//! text record per fixture to stdout (format below): pass and run fixtures go through
//! `compiler::compile` with a `Check` product (run fixtures are compile-
//! checked only — the suite's execution leg is not reproduced here), fail
//! fixtures through `compile_to_checked` with reviewed repository package
//! inputs. A pass or run fixture whose own `build.omg` binds a
//! program-entry root is instead realized for the target the native leg
//! would choose (its bound host target, else its first declared): the
//! `Check` product establishes no program entry, so the unit-plan roster
//! its root claims would never be demanded and an omitted plan would
//! still record `checked`. `tools/corpus_gate.py` diffs those
//! records against the recorded
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
//! fixtures named `*_exit` or carrying `expected_native_exit.txt` (or this
//! host's `expected_native_exit.<target>.txt`) are executed
//! in a fresh temporary directory with
//! `input.txt` as stdin. `OMEGA_CORPUS_NATIVE_TARGET=<target>` builds for
//! another catalogued target instead; its products execute only where this
//! host runs them, which today means `macos_x86_64` under Rosetta
//! (`arch -x86_64`) on an Apple-silicon host. Other foreign targets are
//! realized without executing. Their records gain `exit:<code>` (or `exit:timeout`,
//! `exit:signal` plus `signal:<number>`) and, beside an `expected_stdout.txt`, `stdout:match` or
//! `stdout:differs`.
//!
//! `OMEGA_CORPUS_INTERPRET=1` also executes each runnable fixture (the run
//! tier, `*_exit` and explicit native-exit fixtures) on the checked interpreter that
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

use compiler::{
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

/// Diagnostics that name the selected target spell differently on every
/// host the corpus runs on, so a golden row recorded on one moved on all
/// the others while pinning the same refusal. Record two such spellings
/// with the dynamic part abstracted:
///
/// - the `This build realizes ...` clause, where the name is the whole
///   clause body, and
/// - provider-plan names, which prefix the selected target to a
///   `::satisfies::` path (`macos_arm64::FirstProvider::satisfies::Pick`),
///   with the plan content fingerprint `[xxxxxxxxxxxxxxxx]` that also
///   covers the selected target.
///
/// Declared target owners in names (`windows_x86_64::ProgramEntry`) carry
/// no `::satisfies::` marker and stay literal. Fragment matching keeps
/// weighing the unmodified diagnostic.
fn host_free_diagnostic_text(text: &str) -> String {
    const CLAUSE: &str = "This build realizes ";
    const STOP: &str = ", so ";
    const PLAN_NAME: &str = "<selected target>";
    const FINGERPRINT: &str = "<fingerprint>";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(CLAUSE) {
        let clause_start = start + CLAUSE.len();
        let Some(end) = rest[clause_start..].find(STOP) else {
            break;
        };
        out.push_str(&rest[..clause_start]);
        out.push('`');
        out.push_str(PLAN_NAME);
        out.push('`');
        rest = &rest[clause_start + end..];
    }
    out.push_str(rest);
    let text = out;

    let has_satisfies = text.contains("::satisfies::");
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text.as_str();
    while let Some(open) = rest.find('`') {
        let Some(width) = rest[open + 1..].find('`') else {
            break;
        };
        let close = open + 1 + width;
        let token = &rest[open + 1..close];
        out.push_str(&rest[..open]);
        let target_len = token
            .find("::")
            .map(|end| &token[..end])
            .filter(|name| {
                token.contains("::satisfies::")
                    && target::TargetProfile::from_canonical_target_name(name).is_ok()
            })
            .map(str::len);
        match target_len {
            Some(len) => {
                out.push('`');
                out.push_str(PLAN_NAME);
                out.push_str(&token[len..]);
                out.push('`');
            }
            None => {
                out.push('`');
                out.push_str(token);
                out.push('`');
            }
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    let text = out;
    // Plan fingerprints cover the selected target even when the plan name
    // is the provider-only spelling that hides it.
    if !has_satisfies {
        return text;
    }

    let mut out = String::with_capacity(text.len() + 8);
    let mut rest = text.as_str();
    while let Some(open) = rest.find('[') {
        let Some(width) = rest[open + 1..].find(']') else {
            break;
        };
        let close = open + 1 + width;
        let token = &rest[open + 1..close];
        out.push_str(&rest[..open]);
        if token.len() == 16 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            out.push('[');
            out.push_str(FINGERPRINT);
            out.push(']');
        } else {
            out.push('[');
            out.push_str(token);
            out.push(']');
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
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

/// The target the native leg builds for, when `OMEGA_CORPUS_NATIVE` is set:
/// the catalogued target `OMEGA_CORPUS_NATIVE_TARGET` names, else the host.
fn native_target() -> Option<&'static str> {
    env::var_os("OMEGA_CORPUS_NATIVE")?;
    match env::var("OMEGA_CORPUS_NATIVE_TARGET") {
        Ok(name) if !name.trim().is_empty() => {
            target::TargetProfile::from_canonical_target_name(name.trim())
                .ok()
                .map(|profile| profile.target_name())
        }
        _ => target::TargetProfile::host_if_supported().map(|profile| profile.target_name()),
    }
}

/// The command prefix that executes a `target` product on this host, or
/// `None` when this host cannot run it. The host's own target runs directly;
/// an Apple-silicon macOS host runs `macos_x86_64` images through Rosetta.
fn native_launcher(target: &str) -> Option<Vec<&'static str>> {
    let host = target::TargetProfile::host_if_supported().map(|profile| profile.target_name());
    if host == Some(target) {
        return Some(Vec::new());
    }
    if host == Some("macos_arm64") && target == "macos_x86_64" {
        return Some(vec!["arch", "-x86_64"]);
    }
    None
}

/// Whether runnable fixtures also execute on the checked interpreter.
fn interpreter_leg() -> bool {
    env::var_os("OMEGA_CORPUS_INTERPRET").is_some()
}

/// Interpret one fixture's selected program entry, as `omega run --both`
/// does, and report its exit and stdout facts.
fn interpret_fixture(root_path: &Path, fixture_dir: &Path) -> Vec<String> {
    let Some(target) = native_target()
        .map(str::to_owned)
        .or_else(|| {
            target::TargetProfile::host_if_supported()
                .map(|profile| profile.target_name().to_owned())
        })
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
    let Some(entry) = checked.execution().selected_program_entry.as_ref() else {
        return vec!["interpreter:unchecked".to_owned()];
    };
    let stdin = fs::read(fixture_dir.join("input.txt")).unwrap_or_default();
    let signature = entry.source_signature();
    let mut options = checked_interpreter::InterpretOptions::default();
    if let Some(context) = signature.receiver().source_symbol() {
        options = options.with_provisioned_context_parameter(context);
    }
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        checked_interpreter::BuildMachineEntry::Symbol(signature.machine_symbol()),
        &stdin,
        options,
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
            eprintln!(
                "corpus_runner: {} interpreter stdout mismatch: expected {:?}, actual {:?}",
                fixture_dir.display(),
                expected.chars().take(200).collect::<String>(),
                actual.chars().take(200).collect::<String>(),
            );
            "interpreter-stdout:differs".to_owned()
        });
    }
    facts
}

/// With both legs, whether the native exit is this host's presentation of
/// the interpreter's exact status. Unix wait status keeps the low eight bits
/// and Windows the full i32, the rule `omega run --both` applies in the CLI's
/// `ProcessExitObservation::matches_host_presentation`.
fn interpreter_agreement(facts: &[String]) -> Option<String> {
    let native = facts.iter().find_map(|fact| fact.strip_prefix("exit:"))?;
    let interpreted = facts
        .iter()
        .find_map(|fact| fact.strip_prefix("interpreter-exit:"))?;
    let presented = interpreted
        .parse::<i32>()
        .map(|status| if cfg!(unix) { status & 255 } else { status });
    Some(
        if presented.is_ok_and(|status| native == status.to_string()) {
            "interpreter:agrees".to_owned()
        } else {
            "interpreter:disagrees".to_owned()
        },
    )
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
    let report =
        compiler::compile(request).and_then(compiler::CompileOutcomes::into_single_report)?;
    let published = compilation_products::publication::publish_compilation(report, build_dir)
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
/// matched it (line endings normalized). `launcher` prefixes the executable
/// when the host runs the target through a translator.
fn execute_fixture(
    executable: &Path,
    launcher: &[&str],
    fixture_dir: &Path,
    sequence: usize,
) -> Vec<String> {
    let work = env::temp_dir().join(format!("omega-corpus-run-{}-{sequence}", process::id()));
    let _ = fs::remove_dir_all(&work);
    if fs::create_dir_all(&work).is_err() {
        return vec!["exit:unrunnable".to_owned()];
    }
    let stdin = fs::File::open(fixture_dir.join("input.txt"))
        .map(std::process::Stdio::from)
        .unwrap_or_else(|_| std::process::Stdio::null());
    let mut command = match launcher.split_first() {
        Some((program, arguments)) => {
            let mut command = process::Command::new(program);
            command.args(arguments).arg(executable);
            command
        }
        None => process::Command::new(executable),
    };
    let spawned = command
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
    let mut facts = Vec::new();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Preserve the actual signal: a segmentation fault must not
                // satisfy an expected trap merely because neither has a code.
                #[cfg(unix)]
                if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(&status) {
                    facts.push(format!("signal:{signal}"));
                }
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
    facts.insert(0, exit);
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

/// Realize one fixture as a native artifact for the first target its own
/// `build.omg` binds a program-entry root for, without publishing or running
/// it. Two routes lean on this:
///
/// - a fail fixture whose refusal belongs to a stage after checking -- a
///   layout size overflow, target lowering, provider selection -- checks
///   clean on `compile_to_checked` and is realized before it is recorded as
///   having stopped rejecting;
/// - a pass or run fixture on the check leg, where the `Check` product
///   establishes no program entry and so never demands the unit-plan roster
///   the fixture's own root binding claims -- realizing accounts that
///   roster, and an omitted plan records `rejected` with its `Lowering`
///   diagnostic instead of `checked`.
fn realize_fixture(
    root_path: &Path,
    build_dir: PathBuf,
    target_name: Option<String>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let package_inputs =
        reviewed_repository_fixture_package_inputs(root_path, target_name.as_deref())?;
    let options = CompileOptions {
        root_path: root_path.to_path_buf(),
        build_dir: Some(build_dir.clone()),
        target_name,
    };
    let mut request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let result = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
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
/// the fixture binds the leg's own target. A fixture binding that target
/// runs where the host can execute it; one binding only foreign targets is
/// still realized, for its declared target, because target lowering and
/// emission are most of what the leg measures.
fn pass_realization_target(root_path: &Path, host: &str) -> Option<(String, bool)> {
    let declared = declared_realization_targets(root_path);
    if declared.iter().any(|target| target == host) {
        return Some((host.to_owned(), true));
    }
    declared.into_iter().next().map(|target| (target, false))
}

/// The target the check leg accounts a pass/run fixture's program-entry
/// roster against: the same choice the native leg makes (its bound host
/// target, else its first declared), so a fixture's row answers the demand
/// `omega --check` on that root would make. `None` when the fixture binds
/// no program-entry root at all -- nothing claims a realizable program.
fn check_leg_realization_target(root_path: &Path) -> Option<String> {
    match target::TargetProfile::host_if_supported() {
        Some(profile) => {
            pass_realization_target(root_path, profile.target_name()).map(|(target, _)| target)
        }
        None => declared_realization_target(root_path),
    }
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
    // A host-specific oracle promises this host's physical presentation of
    // the exit status; it overrides the portable one in `tools/corpus_gate.py`.
    let runnable = tier == "run"
        || rel.ends_with("_exit")
        || fixture_dir.join("expected_native_exit.txt").is_file()
        || target::TargetProfile::host_if_supported().is_some_and(|host| {
            fixture_dir
                .join(format!("expected_native_exit.{}.txt", host.target_name()))
                .is_file()
        });
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
                && runnable
                && let Some(launcher) = native_launcher(target)
            {
                facts = execute_fixture(executable, &launcher, &fixture_dir, sequence);
            }
            let _ = fs::remove_dir_all(&build_dir);
            built.map(|_| ())
        }
        // A pass or run fixture that binds a program-entry root claims an
        // executable roster the targetless `Check` product never demands:
        // `omega --check` on the same root selects that entry and refuses
        // when its unit plan is omitted, while this route would record
        // `checked`. Realize the fixture for the target the native leg
        // would choose -- its bound host target when it declares one, else
        // its first declared -- so the roster is accounted before the row
        // reads as executable.
        _ => match check_leg_realization_target(main) {
            Some(target) => realize_fixture(main, unique_build_dir(sequence), Some(target)),
            None => check_pass_fixture(main, unique_build_dir(sequence)),
        },
    };
    if tier == "fail" && outcome.is_ok() {
        outcome = realize_fixture(
            main,
            unique_build_dir(sequence),
            declared_realization_target(main),
        );
    }
    if interpreter_leg() && tier != "fail" && runnable {
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
        // The record reads the compiler request's outcome the compiler itself
        // publishes: `incomplete` when every error names an exceeded private
        // provision — a resource/search limit declined, never a disproof of
        // the source — and `rejected` when any ordinary refusal is present.
        (Err(errors), _) => match compiler::CompilerRequestOutcome::of_refusal(errors) {
            compiler::CompilerRequestOutcome::Incomplete(_) => "incomplete",
            _ => "rejected",
        },
    };
    let diagnostics = errors
        .iter()
        .map(|error| host_free_diagnostic_text(&error.message))
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
        eprintln!(
            "corpus_runner: OMEGA_CORPUS_NATIVE_TARGET names no catalogued target, or this \
             host has no catalogued Omega target to build for"
        );
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
