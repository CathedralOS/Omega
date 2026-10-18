//! Corpus outcome runner — a `harness = false` test target that walks
//! `tests/omega/{pass,fail,run}` and compiles every fixture, printing one
//! text record per fixture to stdout (format below): pass and run fixtures go through
//! `compiler::compile` with a `Check` product (run fixtures are compile-
//! checked only — the suite's execution leg is not reproduced here), fail
//! fixtures through the compiler session with reviewed repository package
//! inputs, continuing that checked compilation to native production only when
//! their own Build declares an entry. Source acceptance never restarts compilation
//! or demands an entry invented by the harness. A pass or run
//! fixture whose own `build.omg` binds a
//! program-entry root is instead realized for one target that build binds
//! (`CHECK_LEG_TARGET_PREFERENCE`, never the recording host) and the record
//! names it as `check-target:<target>`: the `Check` product establishes no
//! program entry, so the unit-plan roster its root claims would never be
//! demanded and an omitted plan would still record `checked`.
//! `tools/corpus_gate.py` diffs those records against the recorded
//! golden so an agent's inner loop is one binary plus a corpus diff instead
//! of a chain of per-crate assertion suites.
//! An authored `expected_terminal_targets.txt` requests one Terminal semantic
//! product on the check leg, or borrows the native leg's retained product.
//! Every target installation uses that exact artifact without recompilation,
//! canonical-root mutation, or fabricated runtime provisions.
//!
//! Running under nextest (`NEXTEST` is set by the harness) exits immediately
//! so this target never executes the corpus as one opaque "test".
//!
//! Knobs: `OMEGA_CORPUS_ROOT` (measured checkout, set by corpus_gate.py for
//! frozen runners built elsewhere), `OMEGA_CORPUS_FIXTURE_FILTER` (comma-separated `tier/group/name`
//! fragments), `OMEGA_CORPUS_SHARD=k/N` (deterministic hash slice, stable
//! across fixture additions), `OMEGA_CORPUS_JOBS` (worker count, default host
//! parallelism capped at 12), `OMEGA_CORPUS_FIXTURE_SECS` (per-fixture cap,
//! default 120 — a timed-out fixture detaches its compile, records status
//! `timeout`, and the pool applies back-pressure so stragglers cannot multiply
//! past the worker count), `OMEGA_CORPUS_RECORDS=<path>` (write the records
//! to that file rather than stdout, which a fixture's build log also reaches).
//!
//! `OMEGA_CORPUS_MIGRATION_FIXTURES` contains newline-separated exact
//! `pending/group/name` paths from the gate's reviewed migration roster. Only
//! those pending roots join the ordinary jobs; their source and authored oracles
//! are unchanged, and filter/shard/timeout/native/interpreted handling is shared.
//! Missing or escaping designations fail collection instead of disappearing.
//! This selects coverage, never semantic success or an unimplemented diagnostic.
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
//! `stdout:differs`. An `expected_stderr.txt` likewise records `stderr:match`
//! or `stderr:differs`; failed captures/oracle reads record `unavailable`.
//!
//! `OMEGA_CORPUS_INTERPRET=1` also executes each runnable fixture (the run
//! tier, `*_exit` and explicit native-exit fixtures) through the admitted
//! Terminal program/instance core for the host target (or the first target the
//! fixture's build binds). Exact entry/host provisions still missing produce
//! named pre-effect capability diagnostics; no checked-tree fallback supplies
//! their results. The record gains
//! `interpreter-exit:<code>` and, beside an `expected_stdout.txt`,
//! `interpreter-stdout:match` or `interpreter-stdout:differs`, with corresponding
//! `interpreter-stderr` facts for an authored stderr oracle; a run the
//! interpreter declines records `interpreter:declined`, and a program that
//! does not check for it records `interpreter:unchecked`. With the native leg
//! as well, the record gains `interpreter:agrees` or `interpreter:disagrees`
//! comparing the two exit codes. `interpreter:trapped` comes only from the
//! selected engine's typed Trap outcome, never diagnostic text or a native signal.
//! When realization and interpretation select the same target, interpretation
//! borrows the native report's exact Terminal artifact before publication; Build
//! runs once for that request. A native refusal supplies no retained intermediate
//! artifact and records unchecked without a source replay. The check golden's
//! canonical target and a different runtime target remain distinct program inputs.
//!
//! `OMEGA_CORPUS_CHECKED_INTERPRETER=1` explicitly selects the independent
//! checked-tree source oracle instead of Terminal execution. It records
//! `interpreter-engine:checked-tree` and consumes the exact selected entry and
//! provisioned receiver from checked compilation; it is never a fallback for a
//! Terminal refusal and cannot reuse a native report's Terminal artifact.
//!
//! A `rollback.txt` beside `main.omg` declares a release rollback overlay for
//! the fixture: one exact optimization case name per line, the spelling
//! `--disable-optimization` takes. The declared `OptimizationRollback` rides
//! every leg that compiles the fixture -- the check leg's realization, the
//! native build, the checked-compile fail and interpreter routes -- so a
//! published receipt names the overlay the run actually applied. On the
//! check leg it additionally compiles the fixture to its retained Terminal
//! artifact twice, once with and once without the overlay, and reports the
//! pair as facts: `rollback:` is the settled receipt the rolled build's
//! report publishes, `unrolled-receipt:` is `none` when the ordinary build
//! settles no overlay, `unrolled-semantic:`/`rolled-back-semantic:` are the
//! manifest fingerprints of the two semantic modules, and `module:differs`
//! or `module:same` answers whether the rolled-back rules actually rewrote
//! the module -- a `same` row names an overlay that proves nothing on that
//! source. Comparing semantic bytes (not executables) is the point: Omega
//! realization normalizes Psi rewrites away, so the Terminal artifact is
//! the last product where the rules' work is still visible. A malformed
//! `rollback.txt` becomes the fixture's own diagnostic rather than silently
//! running the ordinary path.
//!
//! An `expected_published.txt` beside `main.omg` is a publication oracle:
//! one `files/<path>` line for each file a producing build of the fixture
//! publishes into its completed-output set, or no lines when it must publish
//! nothing. The runner builds the fixture the way the CLI does -- the
//! Terminal product under the declared rollback overlay, published only when
//! the compile succeeds -- then lists the build directory's completed sets
//! and records `published:<paths>` or `published:none`. A refusal that
//! arrives after a build staged its outputs must leave no completed set. A
//! listing that differs from the oracle adds its own diagnostic, so the row
//! cannot keep its recorded status by accident.
//!
//! `unaccepted_build_output.txt` supplies a Build-consent refusal control:
//! relative staged filenames which must remain absent. The fail leg supplies
//! caller-owned output accounting, not privately issued staging, and no consent
//! guard. Source capture remains immutable; its actual output scope therefore
//! projects a restricted request. Both the refusal and absence of authored
//! effects are checked before the test directory is discarded.
//!
//! `accepted_entry_supplier.txt` supplies the consumer's explicit program-entry
//! acceptance: one declared dependency's package name per line makes that
//! package's `targets/<target>/entry.omg` schema the accepted `ProgramEntry`
//! binding for the compile, replacing the bundled library's default. A fixture
//! uses it to compile a different accepted contract than the bundled one —
//! for example a supplier whose valid physical convention mismatches the
//! selected slot.
//!
//! Record format, read by `tools/corpus_records.py`: one line per fixture,
//! `<tier/group/name> <status> <milliseconds>ms`, followed by ` expected` or
//! ` unexpected` when the fixture's `expected.txt` fragments were weighed
//! against its diagnostics, then any `key:value` execution facts. Each
//! diagnostic follows on its own line after one
//! tab, with backslash, newline, carriage return and tab escaped as `\\`,
//! `\n`, `\r` and `\t`.

use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct,
};
use std::path::{Path, PathBuf};
use std::{env, fs, process};

// Worker threads compile fixtures in parallel, and every compile allocates
// heavily. The macOS system allocator serializes those threads on its zone
// locks; mimalloc's per-thread heaps do not, which halves the corpus legs'
// wall time. Compiler output is unchanged: the records match either way.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "support/artifact_capture.rs"]
mod artifact_capture;
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/output_oracles.rs"]
mod output_oracles;
#[path = "support/terminal_execution.rs"]
mod terminal_execution;
#[path = "support/terminal_inspection.rs"]
mod terminal_inspection;
use fixture_package_inputs::{
    fixture_build_snapshot, repo_root, reviewed_repository_fixture_package_inputs,
    sealed_fixture_staging,
};

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

/// Exact reviewed pending roots, not a recursive walk of every unfinished
/// experiment. The gate validates capability/task/route metadata; this runner
/// additionally confines source selection to the pending tree before compilation.
fn migration_pending_mains(root: &Path) -> Result<Vec<PathBuf>, String> {
    let Some(requested) = env::var_os("OMEGA_CORPUS_MIGRATION_FIXTURES") else {
        return Ok(Vec::new());
    };
    let requested = requested
        .into_string()
        .map_err(|_| "migration fixture selection is not UTF-8".to_owned())?;
    let corpus = root.join("tests/omega");
    let mut mains = Vec::new();
    for fixture in requested.split('\n').filter(|line| !line.is_empty()) {
        let components: Vec<_> = fixture.split('/').collect();
        if components.len() < 3
            || components[0] != "pending"
            || fixture
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
            || components.iter().any(|component| {
                component.is_empty()
                    || matches!(*component, "." | "..")
                    || component.contains(['\\', ':'])
            })
        {
            return Err(format!(
                "invalid pending migration fixture path `{fixture}`"
            ));
        }
        let main = corpus.join(fixture).join("main.omg");
        if mains.contains(&main) {
            return Err(format!("duplicate pending migration fixture `{fixture}`"));
        }
        let canonical_pending = corpus
            .join("pending")
            .canonicalize()
            .map_err(|error| format!("cannot read pending migration fixture tree: {error}"))?;
        let canonical_main = main
            .canonicalize()
            .map_err(|error| format!("missing pending migration canary `{fixture}`: {error}"))?;
        if !canonical_main.starts_with(&canonical_pending) || !canonical_main.is_file() {
            return Err(format!(
                "pending migration canary escapes its tree: `{fixture}`"
            ));
        }
        mains.push(main);
    }
    Ok(mains)
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
        record.push_str(&escape_diagnostic(&sanitize(diagnostic)));
    }
    record
}

/// Records must be checkout-portable: diagnostics and panic payloads render
/// `path:line:col` positions with absolute fixture paths, so strip the repo
/// root prefix before a record reaches the golden.
///
/// Two prefixes name the same fixture. A fixture is compiled out of the
/// sealed snapshot staged for it, which keeps its repository-relative path
/// under a private per-process session root, so stripping that root yields
/// the same relative spelling the repository root yields and no recorded
/// position moved when the snapshot became the compiled source. A dependency
/// whose Build occurrence ran is read from its own sealed snapshot, staged
/// the same way under a sibling root. Paths that
/// still name the checkout — a sibling the harness read directly, an
/// authored dependency location — keep the repository prefix.
fn sanitize(diagnostic: &str) -> String {
    // Strip before escaping: a Windows backslash otherwise becomes two
    // characters and no longer matches the actual checkout path.
    let diagnostic = strip_root_prefix(diagnostic, sealed_fixture_staging::staging_session_root());
    let diagnostic = strip_root_prefix(
        &diagnostic,
        sealed_fixture_staging::dependency_staging_session_root(),
    );
    let root = repo_root();
    let diagnostic = strip_root_prefix(&diagnostic, &root);
    if (diagnostic.starts_with("tests/") || diagnostic.starts_with("tests\\"))
        && let Some((location, message)) = diagnostic.split_once(": ")
    {
        return format!("{}: {message}", location.replace('\\', "/"));
    }
    diagnostic
}

fn strip_root_prefix(diagnostic: &str, root: &Path) -> String {
    diagnostic
        .replace(&format!("{}/", root.display()), "")
        .replace(&format!("{}\\", root.display()), "")
}

fn unique_build_dir(sequence: usize) -> PathBuf {
    sealed_fixture_staging::temporary_session_root().join(format!("build-{sequence}"))
}

/// The exact optimization rules one fixture asks the release rollback to
/// subtract from its authored build selection: `rollback.txt` beside
/// `main.omg`, one exact case name per line -- the spelling
/// `--disable-optimization` takes. `Some(Ok)` supplies the overlay every leg
/// shares; `Some(Err)` is a malformed declaration and becomes the fixture's
/// own diagnostic so a typo cannot silently run the ordinary path; `None` is
/// an ordinary fixture with no declared overlay.
fn declared_optimization_rollback(
    fixture_dir: &Path,
) -> Option<Result<OptimizationRollback, String>> {
    let text = fs::read_to_string(fixture_dir.join("rollback.txt")).ok()?;
    Some(
        OptimizationRollback::from_exact_names(
            text.lines().map(str::trim).filter(|line| !line.is_empty()),
        )
        .map_err(|error| format!("rollback.txt: {error}")),
    )
}

/// One `key:value` fact carries no spaces, so the receipt's `; ` and `, `
/// separators collapse: `requested=[A, B]` records as `requested=[A,B]`.
fn space_free(text: &str) -> String {
    text.split(' ').collect()
}

/// Semantic bytes and report evidence one Terminal-artifact build retains.
/// The pair's two builds differ only in the declared overlay, so comparing
/// their modules isolates the rolled-back rules' own work.
struct TerminalSemanticLeg {
    /// The manifest's semantic program fingerprint, shortened to the 16-hex
    /// spelling the corpus already uses for plan fingerprints.
    fingerprint: String,
    artifact: compilation_products::RetainedTerminalArtifact,
    /// The settled rollback receipt this build's report publishes: `Some`
    /// exactly when the request carried a non-empty overlay.
    receipt: Option<String>,
}

/// Compile one fixture to its retained Terminal artifact for `target` and
/// keep its semantic bytes, manifest fingerprint and rollback receipt. The
/// Terminal product is the last boundary whose semantic module still shows
/// the Psi rules' rewrites -- Omega realization normalizes them away -- so
/// the rollback pair compares here rather than at the executable.
fn produce_terminal_semantic(
    root_path: &Path,
    build_dir: PathBuf,
    target: Option<String>,
    rollback: Option<&OptimizationRollback>,
) -> Result<TerminalSemanticLeg, Vec<diagnostics::Diagnostic>> {
    let prepared = reviewed_repository_fixture_package_inputs(root_path, target.as_deref())?;
    if prepared.assumed_permissions.is_some() {
        let request = fixture_package_inputs::accepted_fixture_product_request(
            prepared,
            target.as_deref(),
            build_dir.clone(),
            compiler::CompilationProduct::Terminal(rollback.cloned().unwrap_or_default()),
        )?;
        let report = fixture_package_inputs::produce_accepted_fixture(request)?;
        let receipt = report
            .optimization_rollback_receipt()
            .map(std::string::ToString::to_string);
        let artifact = report.into_retained_terminal_artifact().ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "Terminal product retained no artifact",
            )]
        })?;
        let fingerprint = artifact
            .artifact()
            .manifest()
            .semantic()
            .program_fingerprint
            .to_string();
        let _ = fs::remove_dir_all(&build_dir);
        return Ok(TerminalSemanticLeg {
            fingerprint: fingerprint[..16].to_owned(),
            artifact,
            receipt,
        });
    }
    let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
    let options = CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(build_dir.clone()),
        target_name: target,
    };
    let mut request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(rollback) = rollback {
        request = request.with_optimization_rollback(rollback.clone());
    }
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    if let Some(snapshot) = build_snapshot {
        request = request.with_build_snapshot(snapshot);
    }
    let report =
        compiler::compile(request).and_then(compiler::CompileOutcomes::into_single_report)?;
    let receipt = report
        .optimization_rollback_receipt()
        .map(std::string::ToString::to_string);
    let retained = report.into_retained_terminal_artifact().ok_or_else(|| {
        vec![diagnostics::Diagnostic::error(
            "Terminal-artifact product retained no artifact",
        )]
    })?;
    let artifact = retained.artifact();
    let fingerprint = artifact
        .manifest()
        .semantic()
        .program_fingerprint
        .to_string();
    let leg = TerminalSemanticLeg {
        fingerprint: fingerprint[..16].to_owned(),
        artifact: retained,
        receipt,
    };
    let _ = fs::remove_dir_all(&build_dir);
    Ok(leg)
}

/// A fixture declaring `rollback.txt` runs the check leg as a
/// Terminal-artifact pair: the same compile with and without the declared
/// overlay, for the first target its own `build.omg` binds a program entry
/// for. A fixture that binds no entry answers with the product's own
/// missing-entry diagnostic. The pair's facts pin the whole contract:
/// `rollback:` and `unrolled-receipt:` name the evidence each build's report
/// publishes, `unrolled-semantic:`/`rolled-back-semantic:` let the golden
/// record the module identities side by side, and `module:differs` versus
/// `module:same` states whether the rolled-back rules actually rewrote the
/// module -- on a selected-rule fixture `differs` is the vacuity guard that
/// makes the receipt's `applied` meaningful.
fn rollback_pair_facts(
    root_path: &Path,
    rollback: &OptimizationRollback,
    sequence: usize,
) -> (Vec<String>, Result<(), Vec<diagnostics::Diagnostic>>) {
    let build_dir = unique_build_dir(sequence);
    let target = declared_realization_target(root_path);
    let rolled = produce_terminal_semantic(
        root_path,
        build_dir.join("rolled"),
        target.clone(),
        Some(rollback),
    );
    let unrolled = produce_terminal_semantic(root_path, build_dir.join("unrolled"), target, None);
    let _ = fs::remove_dir_all(&build_dir);
    let (rolled, unrolled) = match (rolled, unrolled) {
        (Ok(rolled), Ok(unrolled)) => (rolled, unrolled),
        (Err(errors), _) | (_, Err(errors)) => return (Vec::new(), Err(errors)),
    };
    let facts = vec![
        format!(
            "rollback:{}",
            space_free(rolled.receipt.as_deref().unwrap_or("none"))
        ),
        format!(
            "unrolled-receipt:{}",
            space_free(unrolled.receipt.as_deref().unwrap_or("none"))
        ),
        format!("unrolled-semantic:{}", unrolled.fingerprint),
        format!("rolled-back-semantic:{}", rolled.fingerprint),
        if rolled.artifact.artifact().semantic_bytes()
            == unrolled.artifact.artifact().semantic_bytes()
        {
            "module:same".to_owned()
        } else {
            "module:differs".to_owned()
        },
    ];
    (facts, Ok(()))
}

/// The files a fixture's `expected_published.txt` promises, or `None` when it
/// declares no publication oracle.
fn declared_publication(fixture_dir: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(fixture_dir.join("expected_published.txt")).ok()?;
    let mut files = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    files.sort();
    Some(files)
}

/// Build and publish the fixture as the CLI does, then list what landed:
/// every file under each `completed/<set>/files/` directory, as
/// `files/<path>`. The compile's own refusal is not this check's verdict --
/// the fixture's ordinary legs record that -- so it only stops publication.
fn published_files(
    root_path: &Path,
    rollback: Option<&OptimizationRollback>,
    sequence: usize,
) -> Result<Vec<String>, String> {
    let build_dir = unique_build_dir(sequence).join("publication");
    let compiled = (|| {
        let prepared = reviewed_repository_fixture_package_inputs(
            root_path,
            declared_realization_target(root_path).as_deref(),
        )?;
        if prepared.assumed_permissions.is_some() {
            let request = fixture_package_inputs::accepted_fixture_product_request(
                prepared,
                declared_realization_target(root_path).as_deref(),
                build_dir.clone(),
                compiler::CompilationProduct::Terminal(rollback.cloned().unwrap_or_default()),
            )?;
            return fixture_package_inputs::produce_accepted_fixture(request);
        }
        let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
        let options = CompileOptions {
            root_path: prepared.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: declared_realization_target(root_path),
        };
        let mut request = CompileRequest::new(options)
            .with_requested_product(RequestedCompileProduct::TerminalArtifact);
        if let Some(rollback) = rollback {
            request = request.with_optimization_rollback(rollback.clone());
        }
        if let Some(package_inputs) = prepared.package_inputs {
            request = request.with_package_inputs(package_inputs);
        }
        if let Some(snapshot) = build_snapshot {
            request = request.with_build_snapshot(snapshot);
        }
        compiler::compile(request).and_then(compiler::CompileOutcomes::into_single_report)
    })();
    let listing = match compiled {
        Ok(report) => compilation_products::publication::publish_compilation(report, &build_dir)
            .map(|_| ())
            .and_then(|()| completed_files(&build_dir.join("completed"))),
        Err(_) => completed_files(&build_dir.join("completed")),
    };
    let _ = fs::remove_dir_all(&build_dir);
    listing
}

fn completed_files(sets: &Path) -> Result<Vec<String>, String> {
    fn walk(directory: &Path, prefix: &str, files: &mut Vec<String>) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                walk(&entry.path(), &name, files)?;
            } else {
                files.push(name);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    if !sets.exists() {
        return Ok(files);
    }
    let set_entries =
        fs::read_dir(sets).map_err(|error| format!("cannot list {}: {error}", sets.display()))?;
    for set in set_entries {
        let set = set.map_err(|error| error.to_string())?.path();
        let published = set.join("files");
        if published.is_dir() {
            walk(&published, "files", &mut files)?;
        }
    }
    files.sort();
    Ok(files)
}

/// The pass/run tier of `production_compile` restricted to the Check product:
/// reviewed package inputs plus `compiler::compile`, targetless.
fn check_pass_fixture(
    root_path: &Path,
    build_dir: PathBuf,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let prepared = reviewed_repository_fixture_package_inputs(root_path, None)?;
    let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
    let mut request =
        CompileRequest::for_targets(prepared.root_path, build_dir.clone(), vec![None])
            .with_requested_product(RequestedCompileProduct::Check);
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    if let Some(snapshot) = build_snapshot {
        request = request.with_build_snapshot(snapshot);
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

/// Whether runnable fixtures also execute on the explicitly selected engine.
fn interpreter_leg() -> bool {
    env::var_os("OMEGA_CORPUS_INTERPRET").is_some() || checked_interpreter_leg()
}

fn checked_interpreter_leg() -> bool {
    env::var_os("OMEGA_CORPUS_CHECKED_INTERPRETER").is_some()
}

/// Both execution legs read independent handles to the same authored byte
/// snapshot. Absence means an empty stream, as native fixture execution does;
/// unreadable existing input is an error, not fabricated EOF.
fn fixture_input(fixture_dir: &Path) -> std::io::Result<Box<dyn std::io::Read + Send>> {
    match fs::File::open(fixture_dir.join("input.txt")) {
        Ok(input) => Ok(Box::new(input)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Box::new(std::io::empty()))
        }
        Err(error) => Err(error),
    }
}

/// Dispatch the explicitly selected oracle. Terminal execution owns its canonical
/// artifact and instance lifecycle; checked execution owns its independent source
/// product. Both use reviewed fixture inputs, never an engine-refusal fallback.
fn interpret_fixture(
    root_path: &Path,
    fixture_dir: &Path,
    build_dir: PathBuf,
    target: String,
    rollback: Option<&OptimizationRollback>,
) -> terminal_execution::Observation {
    if checked_interpreter_leg() {
        return interpret_checked_fixture(root_path, fixture_dir, build_dir, &target, rollback);
    }
    let leg = match produce_terminal_semantic(root_path, build_dir, Some(target.clone()), rollback)
    {
        Ok(leg) => leg,
        Err(errors) => {
            return terminal_execution::Observation {
                facts: vec!["interpreter:unchecked".to_owned()],
                diagnostics: errors
                    .iter()
                    .map(|error| host_free_diagnostic_text(&error.message))
                    .collect(),
            };
        }
    };
    let input = match fixture_input(fixture_dir) {
        Ok(input) => input,
        Err(error) => return terminal_execution::Observation::failure(error),
    };
    terminal_execution::interpret(
        leg.artifact.artifact(),
        None,
        Some(&target),
        fixture_dir,
        1_000_000,
        input,
    )
}

/// Run the source-shaped oracle only when the caller explicitly selects it.
/// Admission, target and entry custody come from the same fixture harness as
/// every compile leg; no source name or unsupported Terminal result selects it.
fn interpret_checked_fixture(
    root_path: &Path,
    fixture_dir: &Path,
    build_dir: PathBuf,
    target: &str,
    rollback: Option<&OptimizationRollback>,
) -> terminal_execution::Observation {
    let compile_checked = || {
        let prepared = reviewed_repository_fixture_package_inputs(root_path, Some(target))?;
        let snapshot = fixture_build_snapshot(&prepared.root_path)?;
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(target));
        request.package_inputs = prepared.package_inputs;
        request.build_snapshot = snapshot;
        request.build_dir = Some(build_dir.clone());
        if let Some(rollback) = rollback {
            request.optimization_rollback = rollback.clone();
        }
        compiler::CompilationSession::default()
            .check(request)
            .result
            .map_err(|failure| failure.into_diagnostics())
    };
    let checked = compile_checked();
    let _ = fs::remove_dir_all(&build_dir);
    let checked = match checked {
        Ok(checked) => checked,
        Err(errors) => {
            return terminal_execution::Observation {
                facts: vec!["interpreter:unchecked".to_owned()],
                diagnostics: errors
                    .iter()
                    .map(|error| host_free_diagnostic_text(&error.message))
                    .collect(),
            };
        }
    };
    let Some(entry) = checked.execution().selected_program_entry.as_ref() else {
        return terminal_execution::Observation::failure(
            "checked fixture has no selected ProgramEntry",
        );
    };
    let mut input = match fixture_input(fixture_dir) {
        Ok(input) => input,
        Err(error) => return terminal_execution::Observation::failure(error),
    };
    let mut input_bytes = Vec::new();
    if let Err(error) = std::io::Read::read_to_end(&mut input, &mut input_bytes) {
        return terminal_execution::Observation::failure(error);
    }
    let outcome = checked_interpreter::interpret_entry(
        &checked.execution().program,
        checked_interpreter::BuildMachineEntry::Symbol(entry.machine_symbol()),
        &input_bytes,
        checked_interpreter::InterpretOptions {
            boundary_adapters: &checked.execution().dispatch.boundary_adapters,
            float_comparisons: &checked.execution().dispatch.float_comparisons,
            provisioned_context_parameter: entry
                .context_parameter()
                .is_valid()
                .then(|| entry.context_parameter()),
            ..checked_interpreter::InterpretOptions::default()
        },
    );
    let mut observation = match (outcome.crash_cause, outcome.error.as_deref()) {
        // The crash's site names where a trap fired, so a trapping harness or
        // fixture record says which machine and state crashed.
        (Some(typed_trees::signature::CrashCause::Trap), error) => {
            terminal_execution::Observation {
                facts: vec!["interpreter:trapped".to_owned()],
                diagnostics: error
                    .map(|site| format!("Checked interpreter: {site}"))
                    .into_iter()
                    .collect(),
            }
        }
        (None, None) => terminal_execution::Observation {
            facts: vec![format!("interpreter-exit:{}", outcome.exit_code)],
            diagnostics: Vec::new(),
        },
        // Only the executed checked cause satisfies a Trap oracle. Internal
        // evaluator errors and Abort remain distinct, regardless of wording.
        (_, Some(error)) => {
            return terminal_execution::Observation {
                facts: vec!["interpreter:declined".to_owned()],
                diagnostics: vec![format!("Checked interpreter: {error}")],
            };
        }
        (Some(cause), None) => {
            return terminal_execution::Observation {
                facts: vec!["interpreter:declined".to_owned()],
                diagnostics: vec![format!("Checked interpreter: explicit crash {cause:?}")],
            };
        }
    };
    if let Ok(expected) = fs::read_to_string(fixture_dir.join("expected_stdout.txt")) {
        let matches = std::str::from_utf8(&outcome.stdout)
            .is_ok_and(|actual| actual.replace("\r\n", "\n") == expected.replace("\r\n", "\n"));
        if !matches {
            output_oracles::show_differing_stream("interpreter stdout", &outcome.stdout);
        }
        observation.facts.push(
            if matches {
                "interpreter-stdout:match"
            } else {
                "interpreter-stdout:differs"
            }
            .to_owned(),
        );
    }
    observation
}

/// Both native product classes retain the source-free canonical artifact.
fn retained_native_terminal(
    report: &compilation_products::CompileReport,
) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
    report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|native| native.psi_artifact())
        })
}

/// Observe the native report's own immutable Terminal product before publication
/// consumes it. Both native image classes retain that exact artifact and profile;
/// this route never recompiles source or reconstructs package permission policy.
fn interpret_native_report(
    report: &compilation_products::CompileReport,
    fixture_dir: &Path,
    target: &str,
    roots: &[&compilation_products::TerminalRuntimeProvisions],
) -> terminal_execution::Observation {
    let artifact = retained_native_terminal(report);
    let Some(artifact) = artifact else {
        return terminal_execution::Observation::failure(
            "native report retained no Terminal artifact",
        );
    };
    observe_retained_terminal(
        artifact,
        report.runtime_provisions(),
        report
            .runtime_provisions()
            .map_or(&[], |provisions| provisions.target_provider_proposals()),
        Some(target),
        fixture_dir,
        roots,
    )
}

/// Optional target installations reuse this exact product; ordinary interpreter
/// observations remain selected by their existing leg, never by a refusal.
fn observe_retained_terminal(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
    proposals: &[backend_plan::TerminalTargetProviderProposal],
    target: Option<&str>,
    fixture_dir: &Path,
    roots: &[&compilation_products::TerminalRuntimeProvisions],
) -> terminal_execution::Observation {
    let mut observation = if interpreter_leg() && !checked_interpreter_leg() {
        match fixture_input(fixture_dir) {
            Ok(input) => terminal_execution::interpret(
                artifact,
                provisions,
                target,
                fixture_dir,
                1_000_000,
                input,
            ),
            Err(error) => terminal_execution::Observation::failure(error),
        }
    } else {
        terminal_execution::Observation::default()
    };
    if fixture_dir.join(terminal_execution::TARGET_ORACLE).exists() {
        let installations = terminal_execution::target_installations(
            artifact,
            proposals,
            roots,
            fixture_dir,
            1_000_000,
        );
        observation.facts.extend(installations.facts);
        observation.diagnostics.extend(installations.diagnostics);
    }
    observation
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

/// Build one fixture as a published native executable for `target`, under
/// the fixture's declared rollback overlay when it carries one. The second
/// return is the settled rollback receipt the build's report publishes --
/// `None` when the request carried no overlay -- so the record names the
/// subtraction the executable was realized from.
fn build_native_fixture(
    root_path: &Path,
    build_dir: &Path,
    target: &str,
    rollback: Option<&OptimizationRollback>,
    interpretation: Option<(&Path, &mut Option<terminal_execution::Observation>)>,
    capture: Option<&artifact_capture::Capture>,
) -> Result<(PathBuf, Option<String>), Vec<diagnostics::Diagnostic>> {
    let target_profile = target::TargetProfile::from_omega_target_name(Some(target))
        .map_err(|diagnostic| vec![diagnostic])?;
    let compiled_roots = interpretation
        .as_ref()
        .map(|(fixture_dir, _)| terminal_execution::compiled_target_provisions(fixture_dir))
        .transpose()
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        .unwrap_or(false);
    let mut targets = vec![Some(target_profile)];
    if compiled_roots {
        let fixture_dir = interpretation
            .as_ref()
            .map(|(directory, _)| *directory)
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "compiled target cohort lost its fixture",
                )]
            })?;
        for name in terminal_execution::requested_target_names(fixture_dir)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        {
            let profile = target::TargetProfile::from_omega_target_name(Some(&name))
                .map_err(|diagnostic| vec![diagnostic])?;
            if !targets.contains(&Some(profile)) {
                targets.push(Some(profile));
            }
        }
    }
    let prepared = reviewed_repository_fixture_package_inputs(root_path, Some(target))?;
    let accepted_reports = if prepared.assumed_permissions.is_some() {
        let requests = targets
            .iter()
            .map(|profile| {
                let name = profile.map(|profile| profile.target_name());
                let prepared = reviewed_repository_fixture_package_inputs(root_path, name)?;
                fixture_package_inputs::accepted_fixture_product_request(
                    prepared,
                    name,
                    build_dir.to_path_buf(),
                    compiler::CompilationProduct::Native(
                        compilation_products::NativeProductRequest {
                            private_entry_ram: Vec::new(),
                            terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
                            terminal_authority_permission_policy: None,
                            optimization_rollback: rollback.cloned().unwrap_or_default(),
                        },
                    ),
                )
            })
            .collect::<Result<Vec<_>, Vec<diagnostics::Diagnostic>>>()?;
        let outcomes = compiler::CompilationSession::default().compile_packages(requests);
        Some(
            targets
                .iter()
                .copied()
                .zip(outcomes)
                .map(|(profile, outcome)| {
                    outcome
                        .map_err(|error| error.failure.into_diagnostics())
                        .and_then(|output| {
                            output
                                .into_report()
                                .map_err(compiler::CompilationProductError::into_diagnostics)
                        })
                        .map(|report| (profile, report))
                })
                .collect::<Result<Vec<_>, Vec<diagnostics::Diagnostic>>>()?,
        )
    } else {
        None
    };
    let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
    let mut request =
        CompileRequest::for_targets(prepared.root_path, build_dir.to_path_buf(), targets)
            .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(rollback) = rollback {
        request = request.with_optimization_rollback(rollback.clone());
    }
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    if let Some(snapshot) = build_snapshot {
        request = request.with_build_snapshot(snapshot);
    }
    let mut reports = if let Some(reports) = accepted_reports {
        reports
    } else {
        let outcomes = compiler::compile(request)?;
        let mut reports = Vec::new();
        let mut failures = Vec::new();
        for outcome in outcomes.into_outcomes().into_vec() {
            let profile = outcome.target_profile();
            match outcome.into_result() {
                Ok(report) => reports.push((profile, report)),
                Err(diagnostics) => failures.extend(diagnostics),
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        reports
    };
    let primary = reports
        .iter()
        .position(|(profile, _)| *profile == Some(target_profile))
        .ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "native cohort lost its requested host product",
            )]
        })?;
    let report = &reports[primary].1;
    if let Some(artifact) = report.retained_native_artifact() {
        image_emission::verify_retained_target_operations(artifact.object())
            .map_err(|diagnostic| vec![diagnostic])?;
        // Retained-artifact replay belongs to this independent receiver, not
        // the compile that just produced the object. Audit the whole image,
        // including each private body's artifact-local object composition.
        image_emission::validate_direct_executable_image(artifact.object(), artifact.image())
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    if let Some(capture) = capture {
        let artifact = retained_native_terminal(report).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "native artifact lost its portable Terminal product",
            )]
        })?;
        capture
            .observe("terminal.psi", &artifact.to_bytes())
            .map_err(|error| vec![diagnostics::Diagnostic::error(error)])?;
    }
    if let Some((fixture_dir, observation)) = interpretation {
        let roots = if compiled_roots {
            let artifact = retained_native_terminal(report).ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "target cohort lost its canonical Terminal artifact",
                )]
            })?;
            let mut roots = Vec::new();
            for (_, sibling) in &reports {
                let sibling_artifact = retained_native_terminal(sibling).ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "target cohort lost a retained Terminal product",
                    )]
                })?;
                if !std::ptr::eq(artifact, sibling_artifact) {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "target cohort did not retain one shared canonical Terminal artifact",
                    )]);
                }
                roots.push(sibling.runtime_provisions().ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "target cohort lost its exact admitted runtime root",
                    )]
                })?);
            }
            roots
        } else {
            Vec::new()
        };
        *observation = Some(interpret_native_report(report, fixture_dir, target, &roots));
    }
    let receipt = report
        .optimization_rollback_receipt()
        .map(std::string::ToString::to_string);
    let (_, report) = reports.swap_remove(primary);
    let published = compilation_products::publication::publish_compilation(report, build_dir)
        .map_err(|error| vec![diagnostics::Diagnostic::error(error)])?;
    let executable = published.checked_native_executable_path().ok_or_else(|| {
        vec![diagnostics::Diagnostic::error(
            "published native artifact names no executable",
        )]
    })?;
    if let Some(capture) = capture {
        capture
            .observe_executable(executable)
            .map_err(|error| vec![diagnostics::Diagnostic::error(error)])?;
    }
    Ok((executable.to_path_buf(), receipt))
}

/// Run one built fixture in a fresh directory, stdin from `input.txt`, and
/// report its exit and authored stdout/stderr matches (line endings normalized).
/// Drain both pipes concurrently: either stream can exceed pipe capacity, and
/// neither capture failure can establish an empty-output match.
/// `launcher` prefixes the executable
/// when the host runs the target through a translator.
fn execute_fixture(
    executable: &Path,
    launcher: &[&str],
    fixture_dir: &Path,
    sequence: usize,
) -> Vec<String> {
    let work = sealed_fixture_staging::temporary_session_root().join(format!("run-{sequence}"));
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
        .stderr(std::process::Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        let _ = fs::remove_dir_all(&work);
        return vec!["exit:unrunnable".to_owned()];
    };
    let stdout_receiver = capture_stream(child.stdout.take());
    let stderr_receiver = capture_stream(child.stderr.take());
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
                // A host whose crash reporter stops answering leaves a trapped
                // child in uninterruptible exit, where neither the kill nor
                // `wait` ever completes. Waiting without a bound pinned this
                // worker's in-flight slot, and once every slot held such a
                // child the whole native leg stopped making progress. Give
                // the kill a short grace, then abandon the child unreaped.
                let grace = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while std::time::Instant::now() < grace && !matches!(child.try_wait(), Ok(Some(_)))
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                break "exit:timeout".to_owned();
            }
        }
    };
    // An abandoned child may hold either pipe open forever. Missing capture
    // stays unavailable rather than becoming an empty, success-shaped stream.
    let drain_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let stdout = receive_stream(stdout_receiver, drain_deadline);
    let stderr = receive_stream(stderr_receiver, drain_deadline);
    let _ = fs::remove_dir_all(&work);
    facts.insert(0, exit);
    facts.extend(output_oracles::observe(
        fixture_dir,
        "",
        stdout.as_deref(),
        stderr.as_deref(),
    ));
    facts
}

fn capture_stream(
    stream: Option<impl std::io::Read + Send + 'static>,
) -> std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    if let Some(mut stream) = stream {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = std::io::Read::read_to_end(&mut stream, &mut bytes).map(|_| bytes);
            let _ = sender.send(result);
        });
    }
    receiver
}

fn receive_stream(
    receiver: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    deadline: std::time::Instant,
) -> Option<Vec<u8>> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        .ok()
        .and_then(Result::ok)
}

/// Check a refusal control once and continue only to its authored product.
/// Entryless fixtures stop at checking: demanding a native entry after source
/// acceptance would hide that acceptance behind the harness's own refusal.
/// An entry-bearing or publication-bearing fixture can pin later production
/// failures. It requests that product without repeating preparation or Build.
/// Sponsors, rollback and output-effect controls belong to this one activation.
fn check_fail_fixture(
    root_path: &Path,
    build_dir: PathBuf,
    rollback: Option<&OptimizationRollback>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let target_name = declared_realization_target(root_path);
    let publishes = root_path
        .parent()
        .is_some_and(|directory| directory.join("expected_published.txt").is_file());
    let native_product = (target_name.is_some() || publishes).then(|| compilation_products::NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: rollback.cloned().unwrap_or_default(),
    });
    let prepared = reviewed_repository_fixture_package_inputs(root_path, target_name.as_deref())?;
    let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
    let mut request = CheckedCompileRequest::new(&prepared.root_path, target_name.as_deref());
    request.package_inputs = prepared.package_inputs;
    request.build_snapshot = build_snapshot;
    // A fail fixture's build machine may still reach the build filesystem
    // before the rule it is pinned on refuses, and the staging a compilation
    // defaults to is beside its root source, which is the sealed snapshot
    // here. Name the same private directory every other leg uses.
    request.build_dir = Some(build_dir.clone());
    let unaccepted_output = root_path
        .parent()
        .map(|directory| directory.join("unaccepted_build_output.txt"))
        .ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "fixture has no source directory",
            )]
        })?;
    let absent_outputs = match fs::read_to_string(&unaccepted_output) {
        Ok(contents) => {
            let outputs = contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| PathBuf::from(line.trim()))
                .collect::<Vec<_>>();
            if outputs.is_empty()
                || outputs.iter().any(|path| {
                    !path
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_)))
                })
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "unaccepted_build_output.txt must name nonempty relative filenames without traversal",
                )]);
            }
            let account_root = build_dir.parent().ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "fixture Build output has no account root",
                )]
            })?;
            request.filesystem_sponsor = Some(
                build_time_evaluation::BuildMachineFilesystemSponsor::new(account_root)
                    .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?,
            );
            // This control stops before execution. Pin its semantic request
            // profile so the shared check golden does not record the host's
            // otherwise different request meaning; no foreign Build runs.
            request.build_execution_profile = Some(target::TargetProfile::MacosArm64);
            outputs
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "could not read unaccepted Build output oracle: {error}"
            ))]);
        }
    };
    if let Some(rollback) = rollback {
        request.optimization_rollback = rollback.clone();
    }
    // Authored host policy, not a source recognizer or production mode. Small
    // sponsor ceilings exercise the real request boundary without allocating
    // hundreds of megabytes or executing the default fuel allowance.
    let fixture_root = root_path.parent().expect("fixture has a source directory");
    match fs::read_to_string(fixture_root.join("build_resource_limits.txt")) {
        Ok(contents) => {
            let limits: [u64; 4] = contents
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<Vec<_>, _>>()
                .ok()
                .and_then(|values| values.try_into().ok())
                .filter(|values: &[u64; 4]| values.iter().all(|value| *value != 0))
                .ok_or_else(|| vec![diagnostics::Diagnostic::error(
                    "build_resource_limits.txt requires four positive integers: fuel, entries, total bytes, object extent",
                )])?;
            let evaluation_limits = build_time_evaluation::BuildEvaluationSponsorLimits::new(
                limits[0],
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
            )
            .expect("positive authored fuel and resource ceilings");
            request.evaluation_sponsor = Some(build_time_evaluation::BuildEvaluationSponsor::new(
                evaluation_limits,
            ));
            fs::create_dir_all(build_dir.parent().expect("fixture staging has a parent"))
                .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?;
            let sponsor = build_time_evaluation::BuildMachineFilesystemSponsor::create_private(
                &build_dir,
                build_time_evaluation::BuildMachineFilesystemSponsorLimits {
                    maximum_entries: limits[1],
                    maximum_total_logical_bytes: limits[2],
                    maximum_object_extent: limits[3],
                },
            )
            .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?;
            let output_root = sponsor
                .session_root()
                .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?
                .join("output");
            // Corrupt-host controls start with a concrete hard-link alias.
            // No alias operation is exposed by the default Omega Build facet;
            // capture must reject the actual tree, not replay a sponsor model.
            match fs::read_to_string(fixture_root.join("build_output_alias.txt")) {
                Ok(contents) => {
                    let names = contents
                        .split_whitespace()
                        .map(PathBuf::from)
                        .collect::<Vec<_>>();
                    if names.len() != 2
                        || names.iter().any(|name| {
                            !name.components().all(|component| {
                                matches!(component, std::path::Component::Normal(_))
                            })
                        })
                    {
                        return Err(vec![diagnostics::Diagnostic::error(
                            "build_output_alias.txt requires two relative non-traversing filenames",
                        )]);
                    }
                    fs::create_dir(&output_root)
                        .and_then(|()| fs::write(output_root.join(&names[0]), b"untrusted alias"))
                        .and_then(|()| {
                            fs::hard_link(output_root.join(&names[0]), output_root.join(&names[1]))
                        })
                        .map_err(|error| {
                            vec![diagnostics::Diagnostic::error(format!(
                                "cannot prepare authored Build alias control: {error}",
                            ))]
                        })?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(vec![diagnostics::Diagnostic::error(error.to_string())]),
            }
            request.build_dir = Some(output_root);
            request.filesystem_sponsor = Some(sponsor);
            request.build_snapshot.get_or_insert_with(|| {
                package_compilation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(vec![diagnostics::Diagnostic::error(error.to_string())]),
    }
    let mut session = compiler::CompilationSession::default();
    let mut result = if let Some(native_product) = native_product {
        session
            .produce(
                request,
                compiler::CompilationProduct::Native(native_product),
            )
            .map(|_| ())
            .map_err(compiler::CompilationProductError::into_diagnostics)
    } else {
        session
            .check(request)
            .result
            .map(|_| ())
            .map_err(|failure| failure.into_diagnostics())
    };
    for output in absent_outputs {
        if build_dir.join(&output).try_exists().map_err(|error| {
            vec![diagnostics::Diagnostic::error(format!(
                "could not inspect unaccepted Build output: {error}"
            ))]
        })? {
            let diagnostic = diagnostics::Diagnostic::error(format!(
                "unaccepted Build executed its authored output effect `{}` before consent",
                output.display(),
            ));
            // An oracle violation must not inherit the expected consent
            // diagnostic: otherwise a post-effect refusal could satisfy this
            // negative fixture despite executing the unaccepted action.
            result = Err(vec![diagnostic]);
            break;
        }
    }
    let _ = fs::remove_dir_all(&build_dir);
    result
}

/// Realize one fixture as a native artifact for the first target its own
/// `build.omg` binds a program-entry root for, without publishing or running
/// it. A pass or run fixture on the check leg uses this because the `Check`
/// product establishes no program entry and so never demands the unit-plan
/// roster the fixture's own root binding claims. Realizing accounts that roster:
/// an omitted plan records `rejected` instead of `checked`.
fn realize_fixture(
    root_path: &Path,
    build_dir: PathBuf,
    target_name: Option<String>,
    rollback: Option<&OptimizationRollback>,
    interpretation: Option<(&Path, &mut Option<terminal_execution::Observation>)>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let runtime_target = target::TargetProfile::from_omega_target_name(target_name.as_deref())
        .map_err(|diagnostic| vec![diagnostic])?
        .target_name();
    let prepared = reviewed_repository_fixture_package_inputs(root_path, target_name.as_deref())?;
    let build_snapshot = fixture_build_snapshot(&prepared.root_path)?;
    if prepared.assumed_permissions.is_some() {
        let product =
            compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
                private_entry_ram: Vec::new(),
                terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
                terminal_authority_permission_policy: None,
                optimization_rollback: rollback.cloned().unwrap_or_default(),
            });
        let request = fixture_package_inputs::accepted_fixture_product_request(
            prepared,
            target_name.as_deref(),
            build_dir.clone(),
            product,
        )?;
        let result = fixture_package_inputs::produce_accepted_fixture(request).map(|report| {
            if let Some((fixture_dir, observation)) = interpretation {
                *observation = Some(interpret_native_report(
                    &report,
                    fixture_dir,
                    runtime_target,
                    &[],
                ));
            }
        });
        let _ = fs::remove_dir_all(&build_dir);
        return result;
    }
    let options = CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(build_dir.clone()),
        target_name,
    };
    let mut request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(rollback) = rollback {
        request = request.with_optimization_rollback(rollback.clone());
    }
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    if let Some(snapshot) = build_snapshot {
        request = request.with_build_snapshot(snapshot);
    }
    let result = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .map(|report| {
            if let Some((fixture_dir, observation)) = interpretation {
                *observation = Some(interpret_native_report(
                    &report,
                    fixture_dir,
                    runtime_target,
                    &[],
                ));
            }
        });
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
    let Ok(tokens) = source_files_to_tokens::Lexer::new(&text).tokenize() else {
        return Vec::new();
    };
    let Ok(syntax) = tokens_to_syntax_trees::parse_syntax_trees(&tokens) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    // Read the slot operand structurally. An explicit Build argument can be
    // a call containing commas, and comments may contain example bindings;
    // neither should change which target this fixture actually declares.
    for item in syntax.root_items() {
        let syntax_trees::item::Item::Machine(machine) = item else {
            continue;
        };
        for state in syntax.items.state_handles(machine.states) {
            let state = syntax.items.state(*state);
            for statement in syntax.items.statements(state.statements) {
                let syntax_trees::statement::StatementNode::RootBinding(binding) =
                    syntax.statements.statement(*statement)
                else {
                    continue;
                };
                let [target, .., slot] = binding.slot.as_ref() else {
                    continue;
                };
                if slot.as_str() == "ProgramEntry"
                    && !targets.iter().any(|seen| seen == target.as_str())
                {
                    targets.push(target.as_str().to_owned());
                }
            }
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
///
/// Only the native leg may prefer the leg's own target, because its records
/// go to that target's own golden, `tests/omega/corpus_native_<target>.txt`.
/// The check leg deliberately does not share this chooser -- see
/// [`CHECK_LEG_TARGET_PREFERENCE`] -- and unifying the two would put a
/// target-scoped verdict back into the one golden every host shares.
fn pass_realization_target(root_path: &Path, host: &str) -> Option<(String, bool)> {
    let declared = declared_realization_targets(root_path);
    if declared.iter().any(|target| target == host) {
        return Some((host.to_owned(), true));
    }
    declared.into_iter().next().map(|target| (target, false))
}

/// The order the check leg prefers among the targets a fixture's own
/// `build.omg` binds, so that one realized row is reproducible on every
/// recording host.
///
/// The check leg realizes a bound program entry to answer a
/// target-independent question -- is the entry's unit-plan roster actually
/// produced, or did it record `checked` with nothing demanding it -- through
/// a product that is unavoidably target-scoped: provider settlement, ABI and
/// emission all differ per target, and the omission itself only surfaces in
/// `realization/selected-dispatch`. So the leg needs one target and the
/// choice is convention. The convention may not be the recording host.
/// `tests/omega/corpus_outcomes.txt` is one golden shared by Linux, macOS and
/// Windows developers with no target dimension, so a host-chosen target
/// writes one host's target-scoped verdict as everyone's: provider planning's
/// canonical `FilesystemHost` settlement table has a `linux_x86_64` arm and
/// no other, which is how `pass/filesystem/native_close` came to record
/// `checked` from a Linux recorder while the `native_errno` row beside it was
/// a Darwin recording -- an impossible pair reproducible on neither host.
///
/// Two measurements pick the order among a fixture's declared targets. The
/// chosen target's realization must be complete enough that a target gap does
/// not mask the roster signal the leg carries: of the three hosted targets
/// the repository keeps a native golden for, `macos_arm64` records 849 built
/// against 63 rejected, `linux_x86_64` 588 against 73, and `windows_x86_64`
/// 333 against 156. And adopting the order must not relabel the golden's
/// existing rows, or the red this defect was hiding becomes unreadable
/// against the churn: this order changes the realized target for 1 of the
/// 1024 entry-binding fixtures relative to the golden's majority recordings,
/// where declaration order would change 801 of them -- 765 fixtures declare
/// `windows_x86_64` first.
///
/// A fixture binding none of these keeps its first declared target, which is
/// already a property of its source. The row names whichever target was
/// chosen, so revising this order is a visible per-row diff rather than a
/// silent reinterpretation of rows nobody re-measured.
const CHECK_LEG_TARGET_PREFERENCE: &[&str] = &["macos_arm64", "linux_x86_64", "windows_x86_64"];

/// The target the check leg accounts a pass/run fixture's program-entry
/// roster against, read from the fixture's own `build.omg` and never from
/// the recording host. `None` when the fixture binds no program-entry root
/// at all -- nothing claims a realizable program.
fn check_leg_realization_target(root_path: &Path) -> Option<String> {
    let declared = declared_realization_targets(root_path);
    CHECK_LEG_TARGET_PREFERENCE
        .iter()
        .find(|preferred| declared.iter().any(|target| target == *preferred))
        .map(|preferred| (*preferred).to_owned())
        .or_else(|| declared.into_iter().next())
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
    let terminal_target_oracle = fixture_dir.join(terminal_execution::TARGET_ORACLE).exists();
    // A host-specific oracle promises this host's physical presentation of
    // the exit status; it overrides the portable one in `tools/corpus_gate.py`.
    let runnable = tier == "run"
        || rel.ends_with("_exit")
        || terminal_target_oracle
        || fixture_dir.join("expected_native_exit.txt").is_file()
        || fixture_dir.join("expected_stdout.txt").is_file()
        || fixture_dir.join("expected_stderr.txt").is_file()
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
    // Prefer the native leg's actual selection, including a foreign-only
    // fixture. Otherwise prefer a declared host root and then a declared foreign
    // root. The interpreter must not demand an absent host RootBinding.
    let interpreter_target = (interpreter_leg() && tier != "fail" && runnable)
        .then(|| {
            realization
                .as_ref()
                .map(|(target, _)| target.clone())
                .or_else(|| {
                    target::TargetProfile::host_if_supported()
                        .and_then(|host| pass_realization_target(main, host.target_name()))
                        .map(|(target, _)| target)
                })
                .or_else(|| declared_realization_target(main))
        })
        .flatten();
    let mut interpreter_observation = None;
    let mut shared_interpreter_request = false;
    // A declared rollback overlay rides every leg the fixture compiles on.
    // A malformed declaration becomes the fixture's own rejected record.
    let (declared_rollback, rollback_error) = match declared_optimization_rollback(&fixture_dir) {
        Some(Ok(rollback)) => (Some(rollback), None),
        Some(Err(message)) => (None, Some(message)),
        None => (None, None),
    };
    let mut realized = false;
    let mut outcome = match (&rollback_error, tier, &realization) {
        (Some(message), _, _) => Err(vec![diagnostics::Diagnostic::error(message.clone())]),
        (_, "fail", _) => {
            check_fail_fixture(main, unique_build_dir(sequence), declared_rollback.as_ref())
        }
        (_, _, Some((target, executes_here))) => {
            realized = true;
            let build_dir = unique_build_dir(sequence);
            let interpretation = (terminal_target_oracle
                || (!checked_interpreter_leg() && interpreter_target.as_ref() == Some(target)))
            .then(|| {
                shared_interpreter_request = !checked_interpreter_leg();
                (fixture_dir.as_path(), &mut interpreter_observation)
            });
            let capture = artifact_capture::Capture::from_environment(tier, &rel, target);
            let mut built = match &capture {
                Ok(capture) => build_native_fixture(
                    main,
                    &build_dir,
                    target,
                    declared_rollback.as_ref(),
                    interpretation,
                    capture.as_ref(),
                ),
                Err(error) => Err(vec![diagnostics::Diagnostic::error(error.clone())]),
            };
            let mut integrity_error = None;
            if let Ok((executable, receipt)) = &built {
                if let Some(receipt) = receipt {
                    facts.push(format!("rollback:{}", space_free(receipt)));
                }
                let execution = (|| -> Result<(), String> {
                    let before = if let Ok(Some(capture)) = &capture {
                        Some(capture.verify_executable(executable)?)
                    } else {
                        None
                    };
                    if *executes_here
                        && runnable
                        && let Some(launcher) = native_launcher(target)
                    {
                        facts.extend(execute_fixture(
                            executable,
                            &launcher,
                            &fixture_dir,
                            sequence,
                        ));
                    }
                    if let Ok(Some(capture)) = &capture
                        && before != Some(capture.verify_executable(executable)?)
                    {
                        return Err(
                            "executing corpus image identity or timestamps changed".to_owned()
                        );
                    }
                    Ok(())
                })();
                integrity_error = execution.err();
            }
            if let Some(error) = integrity_error {
                built = Err(vec![diagnostics::Diagnostic::error(error)]);
            }
            let _ = fs::remove_dir_all(&build_dir);
            built.map(|_| ())
        }
        // An authored installation matrix observes one exact portable product,
        // not separately compiled native products for its named target rows.
        (_, _, _) if terminal_target_oracle => {
            shared_interpreter_request = !checked_interpreter_leg();
            let check_target = check_leg_realization_target(main);
            let leg = produce_terminal_semantic(
                main,
                unique_build_dir(sequence),
                check_target.clone(),
                declared_rollback.as_ref(),
            );
            match leg {
                Ok(leg) => {
                    if let Some(receipt) = &leg.receipt {
                        facts.push(format!("rollback:{}", space_free(receipt)));
                    }
                    if let Some(target) = &check_target {
                        facts.push(format!("check-target:{target}"));
                    }
                    interpreter_observation = Some(observe_retained_terminal(
                        leg.artifact.artifact(),
                        None,
                        leg.artifact
                            .native_realization_proposal()
                            .map_or(&[], |proposal| proposal.target_provider_proposals()),
                        interpreter_target.as_deref(),
                        &fixture_dir,
                        &[],
                    ));
                    Ok(())
                }
                Err(errors) => Err(errors),
            }
        }
        // A pass or run fixture that binds a program-entry root claims an
        // executable roster the targetless `Check` product never demands:
        // `omega --check` on the same root selects that entry and refuses
        // when its unit plan is omitted, while this route would record
        // `checked`. Realize the fixture for one target its own build binds,
        // so the roster is accounted before the row reads as executable, and
        // record that target: the realization verdict is a verdict about it,
        // and this golden has no other place to say so
        // (`CHECK_LEG_TARGET_PREFERENCE`).
        (_, _, _) => {
            let check_target = check_leg_realization_target(main);
            let interpretation = (!checked_interpreter_leg()
                && check_target.is_some()
                && check_target == interpreter_target)
                .then(|| {
                    shared_interpreter_request = true;
                    (fixture_dir.as_path(), &mut interpreter_observation)
                });
            let check_outcome = match (&declared_rollback, &check_target) {
                // A declared rollback runs the check leg as a Terminal-artifact
                // pair whose facts pin the overlay contract, then still realizes
                // the fixture's bound-entry roster the `Check` product never
                // demands, under the same overlay the legs share.
                (Some(rollback), target) => {
                    let (pair_facts, pair) = rollback_pair_facts(main, rollback, sequence);
                    facts.extend(pair_facts);
                    match (pair, target) {
                        (Err(errors), _) => Err(errors),
                        (Ok(()), Some(target)) => realize_fixture(
                            main,
                            unique_build_dir(sequence),
                            Some(target.clone()),
                            Some(rollback),
                            interpretation,
                        ),
                        (Ok(()), None) => Ok(()),
                    }
                }
                (None, Some(target)) => realize_fixture(
                    main,
                    unique_build_dir(sequence),
                    Some(target.clone()),
                    None,
                    interpretation,
                ),
                (None, None) => check_pass_fixture(main, unique_build_dir(sequence)),
            };
            // Named even when realization refused: the diagnostics below are
            // this target's refusal, and a row that does not say which target
            // it realized cannot be read as a verdict on any.
            if let Some(target) = &check_target {
                facts.push(format!("check-target:{target}"));
            }
            check_outcome
        }
    };
    let mut interpreter_diagnostics = Vec::new();
    if terminal_target_oracle && (!interpreter_leg() || checked_interpreter_leg()) {
        if let Some(observation) = interpreter_observation.take() {
            facts.extend(observation.facts);
            interpreter_diagnostics.extend(observation.diagnostics);
        } else {
            facts.push("terminal-targets:unchecked".to_owned());
        }
    }
    if interpreter_leg() && tier != "fail" && runnable {
        if checked_interpreter_leg() {
            facts.push("interpreter-engine:checked-tree".to_owned());
        }
        let observation = if let Some(observation) = interpreter_observation {
            observation
        } else if shared_interpreter_request || rollback_error.is_some() {
            // A failed same-target request returned no intermediate product.
            // Preserve its refusal; replaying Build would create a second
            // production with potentially different effects and custody.
            terminal_execution::Observation {
                facts: vec!["interpreter:unchecked".to_owned()],
                // The record already owns the exact request refusal below.
                diagnostics: Vec::new(),
            }
        } else if let Some(target) = interpreter_target {
            // The check golden's canonical target and the runtime target are
            // genuinely different program inputs, so each owns one production.
            interpret_fixture(
                main,
                &fixture_dir,
                unique_build_dir(sequence),
                target,
                declared_rollback.as_ref(),
            )
        } else {
            terminal_execution::Observation::failure(
                "no realization target supplies a selected entry",
            )
        };
        facts.extend(observation.facts);
        interpreter_diagnostics.extend(observation.diagnostics);
        if let Some(agreement) = interpreter_agreement(&facts) {
            facts.push(agreement);
        }
    }
    if rollback_error.is_none()
        && let Some(expected) = declared_publication(&fixture_dir)
    {
        let published = published_files(main, declared_rollback.as_ref(), sequence);
        let actual = published.as_ref().map(Vec::as_slice).unwrap_or(&[]);
        facts.push(if actual.is_empty() {
            "published:none".to_owned()
        } else {
            format!("published:{}", actual.join(","))
        });
        let mismatch = match &published {
            Err(error) => Some(format!("publication could not be listed: {error}")),
            Ok(actual) if *actual != expected => Some(format!(
                "published outputs [{}] differ from expected_published.txt [{}]",
                actual.join(", "),
                expected.join(", "),
            )),
            Ok(_) => None,
        };
        if let Some(message) = mismatch {
            let error = diagnostics::Diagnostic::error(message);
            match &mut outcome {
                Ok(()) => outcome = Err(vec![error]),
                Err(errors) => errors.push(error),
            }
        }
    }
    if outcome.is_ok() && fixture_dir.join(terminal_inspection::ORACLE).exists() {
        outcome = terminal_inspection::inspect(main, &fixture_dir);
        facts.push(if outcome.is_ok() {
            "terminal-inspections:match".to_owned()
        } else {
            "terminal-inspections:failed".to_owned()
        });
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
    let mut diagnostics = errors
        .iter()
        .map(|error| host_free_diagnostic_text(&error.message))
        .collect::<Vec<_>>();
    diagnostics.extend(interpreter_diagnostics);
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
        // Panic diagnostics sanitize source paths too. Publish completion only
        // after the last staging access, so exit-time cleanup cannot release
        // this session's reservation while a detached thread still uses it.
        live_owned.fetch_sub(1, std::sync::atomic::Ordering::Release);
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
                    // The compile thread died before publishing its record
                    // and its own fetch_sub, so decrement here to keep the
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
    let encoded_root = root
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    eprintln!("corpus_runner: fixture-root-v1 {encoded_root}");
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
    let mut migration_mains = match migration_pending_mains(&root) {
        Ok(mains) => mains,
        Err(message) => {
            eprintln!("corpus_runner: {message}");
            process::exit(2);
        }
    };
    let mut jobs: Vec<(&'static str, PathBuf, PathBuf, std::time::Duration)> = Vec::new();
    let tiers: &[&'static str] = if native_target().is_some() {
        &["pass", "run", "pending"]
    } else {
        &["pass", "fail", "run", "pending"]
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
        let mut mains = if tier == "pending" {
            std::mem::take(&mut migration_mains)
        } else {
            let mut discovered = Vec::new();
            collect_mains(&base, &mut discovered);
            discovered
        };
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
                // Exhaustion needs no execution capacity. In particular, the
                // last timed-out fixture must not delay publishing its record
                // while this worker waits for work that does not exist.
                let slot = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if slot >= order.len() {
                    break;
                }
                // Back-pressure: never leave more than `workers` compiles in
                // flight, stragglers included. Give up after 2*cap so genuinely
                // wedged compiles stall the pool only temporarily, not forever.
                let mut waited = std::time::Duration::ZERO;
                while live.load(std::sync::atomic::Ordering::Relaxed) >= workers && waited < cap * 2
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    waited += std::time::Duration::from_millis(50);
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
    let still_live = live.load(std::sync::atomic::Ordering::Acquire);
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
    // A build that writes its log reaches this process's real stdout, as the
    // compiler's other output does, so a consumer that parses records names a
    // file of their own instead of sharing that stream.
    let mut text = String::new();
    for record in records {
        text.push_str(&record.to_string());
        text.push('\n');
    }
    match env::var_os("OMEGA_CORPUS_RECORDS") {
        Some(path) => {
            if let Err(error) = std::fs::write(&path, text) {
                eprintln!("corpus_runner: could not write OMEGA_CORPUS_RECORDS {path:?}: {error}");
                process::exit(1);
            }
        }
        None => print!("{text}"),
    }
    // A live straggler can still remove build paths. Retain the reservation
    // until it dies with this process, rather than letting a same-PID session
    // reuse the root while those paths remain live.
    sealed_fixture_staging::discard_staged_snapshots(still_live);
    // Detached stragglers from timed-out fixtures die with the process here.
    process::exit(0);
}
