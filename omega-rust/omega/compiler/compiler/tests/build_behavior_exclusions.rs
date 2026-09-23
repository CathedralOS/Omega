//! End-to-end coverage for `builder.exclude_crash(CrashCause::X)`
//! (wiki/spec/build/behavior_exclusions.md): the authored exclusion is a
//! product-admission requirement recorded when the checked build evaluation
//! actually executes the call against the root `Build`, retaining the exact
//! toolchain `CrashCause` case identity and the authored source span.

use compiler::CheckedCompileRequest;
use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, compile, compile_to_checked,
    realize_retained_native_artifact,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
use fixture_package_inputs::{
    bundled_standard_library_dependency_declaration, bundled_standard_library_root,
    console_acceptance, fixture_package_identity, macos_entry_acceptance,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-build-behavior-exclusions-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary Omega project");
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create temporary Omega source directory");
        }
        fs::write(path, source).expect("write temporary Omega source");
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn toolchain_crash_cause_case(
    checked: &compiler::CheckedCompilation,
    case: &str,
) -> symbols::SymbolHandle {
    toolchain_exclusion_case(checked, "CrashCause", case)
}

fn toolchain_exclusion_case(
    checked: &compiler::CheckedCompilation,
    type_name: &str,
    case: &str,
) -> symbols::SymbolHandle {
    let definition = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str() == type_name
                && checked
                    .typed
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| checked.typed.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            && file.path == std::path::Path::new("<build-prelude>")
                    })
        })
        .expect("exact toolchain exclusion declaration");
    checked
        .typed
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.name.as_str() == case => {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("toolchain exclusion case")
}

#[test]
fn physical_exclusion_vocabulary_retains_exact_identity_and_canonical_union() {
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    let mut selections = String::new();
    // Reverse and repeat the roster: the authored occurrences stay distinct
    // while the admission union must use the shared classifier's exact set.
    for class in effects::TerminalAuthorityClass::ALL.into_iter().rev() {
        selections.push_str(&format!(
            "    builder.exclude_physical_authority(PhysicalAuthorityClass::{class:?});\n"
        ));
    }
    selections.push_str(
        "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);\n",
    );
    project.write(
        "build.omg",
        &product_build("physical-exclusion-roster", &selections),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("all physical classes are authorable");
    assert_eq!(
        checked.behavior_exclusions().len(),
        effects::TerminalAuthorityClass::ALL.len() + 1
    );
    for row in checked.behavior_exclusions() {
        let build_evaluation::AuthoredBehaviorExclusionKind::PhysicalAuthorityClass {
            class,
            case_symbol,
        } = row.kind
        else {
            panic!("physical class row");
        };
        assert_eq!(
            case_symbol,
            toolchain_exclusion_case(&checked, "PhysicalAuthorityClass", &format!("{class:?}"))
        );
        let source = checked
            .typed
            .symbols
            .source_file(row.source_span)
            .expect("authored span");
        assert_eq!(source.path.file_name().unwrap(), "build.omg");
    }
    let union = build_evaluation::authored_behavior_exclusion_set(checked.behavior_exclusions());
    assert_eq!(
        union.physical_authority_classes(),
        effects::TerminalAuthorityClass::ALL
    );
}

#[test]
fn physical_exclusions_follow_evaluated_helper_arguments_and_skip_untaken_calls() {
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    project.write("build.omg", r#"machine restrict(builder: &mut Build, class: PhysicalAuthorityClass) {
    builder.exclude_physical_authority(class);
}
machine unused(builder: &mut Build) {
    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessTermination);
}
machine build(builder: &mut Build) {
    builder.application("physical-exclusion-evaluated");
    builder.roots.bind(windows_x86_64::ProgramEntry, launch);
    let selected: PhysicalAuthorityClass = PhysicalAuthorityClass::ProcessOutput;
    restrict(builder, selected);
    transition false { true -> skipped(builder) false -> done() }
    state skipped(builder: &mut Build) { builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessInput); }
    state done() { }
}
"#);
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("executed helper selection");
    let [row] = checked.behavior_exclusions() else {
        panic!("only executed selection retained");
    };
    let helper = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "restrict")
        .unwrap();
    assert_eq!(row.selecting_machine, helper.symbol);
    let report = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("retained selected product");
    let retained = report.into_retained_terminal_artifact().unwrap();
    assert_eq!(
        retained
            .native_realization_proposal()
            .unwrap()
            .behavior_exclusions()
            .physical_authority_classes(),
        &[effects::TerminalAuthorityClass::ProcessOutput]
    );
}

#[test]
fn physical_exclusions_require_the_original_build_and_toolchain_case() {
    for (statement, expected) in [
        (
            "let mut other: Build = Build {}; other.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);",
            "source cannot construct the compiler-owned Build activation",
        ),
        (
            "builder.exclude_physical_authority(CrashCause::Trap);",
            "PhysicalAuthorityClass",
        ),
    ] {
        let project = TempProject::new();
        project.write("main.omg", QUIET_MAIN);
        project.write(
            "build.omg",
            &product_build("physical-exclusion-invalid", statement),
        );
        let Err(diagnostics) = compile_to_checked(CheckedCompileRequest::new(
            &project.main(),
            Some("windows_x86_64"),
        )) else {
            panic!("invalid physical selection must reject");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn physical_exclusion_admission_rechecks_the_evaluated_case_identity() {
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "physical-exclusion-replay",
            "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);\n",
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .unwrap();
    let authored = checked.behavior_exclusions()[0].clone();
    // Replay the enum-evidence admission boundary independently of execution.
    // Substituting an exact case of another compiler enum still must reject.
    let mut typed = checked.typed.clone();
    let site = typed
        .statement_table
        .insert(typed_trees::statement::StatementNode::Call(
            typed_trees::statement::TableCall {
                source_span: authored.source_span,
                ..Default::default()
            },
        ));
    let selection = |case_symbol| checked_interpreter::ExecutedBehaviorExclusion {
        machine: authored.selecting_machine,
        site: checked_interpreter::ExecutedBehaviorExclusionSite::Statement(site),
        kind: checked_interpreter::ExecutedBehaviorExclusionKind::PhysicalAuthorityClass {
            case_symbol,
        },
    };
    let physical = toolchain_exclusion_case(&checked, "PhysicalAuthorityClass", "ProcessOutput");
    assert_eq!(
        build_evaluation::harvest_behavior_exclusions(&typed, &[selection(physical)]).unwrap(),
        vec![authored.clone()]
    );
    for case_symbol in [
        toolchain_crash_cause_case(&checked, "Trap"),
        symbols::SymbolHandle::invalid(),
    ] {
        let diagnostics =
            build_evaluation::harvest_behavior_exclusions(&typed, &[selection(case_symbol)])
                .expect_err("substituted case cannot acquire physical authority identity");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exact compiler-owned PhysicalAuthorityClass case")
        }));
    }
}

#[test]
fn authored_physical_exclusion_reaches_the_native_product() {
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "physical-exclusion",
            "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);\n",
        ),
    );
    let report = compile(product_request(
        project.main(),
        RequestedCompileProduct::NativeArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a source-authored physical exclusion admits an output-free native product");
    assert!(report.retained_native_artifact().is_some());
}

#[test]
fn authored_physical_exclusion_publishes_and_runs_on_the_host() {
    let target = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos_arm64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows_x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux_x86_64"
    } else {
        eprintln!(
            "SKIP: native exclusion execution requires macOS ARM64, Windows x64, or Linux x64"
        );
        return;
    };
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "physical-exclusion-host",
            "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);\n",
        )
        .replace("windows_x86_64", target),
    );
    let request = CompileRequest::new(CompileOptions {
        root_path: project.main(),
        build_dir: None,
        target_name: Some(target.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("source-built output-free native product")
        .publish_retained_native_artifact(&project.0.join("out"))
        .expect("checked native publication");
    let executable = report
        .checked_native_executable_path()
        .expect("published executable");
    let output = std::process::Command::new(executable)
        .output()
        .expect("run checked native product");
    assert!(output.status.success(), "native result: {output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

/// The Console output composition's build entry: an application depending on
/// omega_language_std by path, selecting its native Console provider under
/// the exact `alias::decl` spelling cross-package operands require, bound to
/// the macOS ARM64 entry with `extra` carrying the authored exclusions.
fn console_output_build(name: &str, extra: &str) -> String {
    format!(
        r#"machine build(builder: &mut Build) {{
    builder.application("{name}");
{}    builder.select_provider<omega_language_std::Console, omega_language_std::ConsoleNativeProvider>();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
{extra}}}
"#,
        bundled_standard_library_dependency_declaration()
    )
}

/// Package inputs for the Console composition: the checked macOS entry
/// binding always, plus the std Console provider's exit binding only when
/// the selected provider is `ConsoleNativeProvider` — an app-authored
/// provider carries no toolchain exit semantics to accept.
fn console_package_inputs(
    project: &TempProject,
    name: &str,
    native_console_provider: bool,
) -> package_compilation::PackageCompilationInputs {
    let standard_library = bundled_standard_library_root();
    let root_identity = fixture_package_identity(81);
    let library_identity = fixture_package_identity(82);
    let entry_binding =
        macos_entry_acceptance::candidate_macos_entry_binding(&standard_library, library_identity)
            .expect("accept checked package entry");
    let inputs = package_compilation::PackageCompilationInputs::new_package(
        root_identity,
        vec![
            package_compilation::PackageSourceBinding::new(root_identity, name, project.0.clone()),
            package_compilation::PackageSourceBinding::new(
                library_identity,
                "omega-language-std",
                standard_library,
            ),
        ],
        vec![package_compilation::PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            library_identity,
        )],
    )
    .expect("fixture packages")
    .with_accepted_semantic_bindings(vec![entry_binding.clone()])
    .expect("exact entry binding");
    if !native_console_provider {
        return inputs;
    }
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some("macos_arm64"))
    })
    .expect("resolve exact Console provider");
    let binding =
        console_acceptance::candidate_console_exit_binding(&checked, library_identity, true, false)
            .expect("accept fixture Console provider");
    inputs
        .with_accepted_semantic_bindings(vec![entry_binding, binding])
        .expect("exact accepted binding")
}

#[test]
fn authored_physical_exclusion_rejects_exercised_output_not_unrelated_input() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        r#"use omega_language_std::console;
use omega::language::core::service;
data Main { console: Binding<Console>; }
machine Main::main(&mut self) reaches Console { self.console.write_byte(65); }
"#,
    );
    let build = console_output_build(
        "physical-exclusion-output",
        "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessInput);\n",
    );
    project.write("build.omg", &build);
    let inputs = console_package_inputs(&project, "physical-exclusion-output", true);
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_package_inputs(inputs.clone())
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
    };
    let allowed = compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("output does not exercise excluded input authority")
        .publish_retained_native_artifact(&project.0.join("allowed"))
        .expect("publish checked output program");
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        let output = std::process::Command::new(
            allowed
                .checked_native_executable_path()
                .expect("published executable"),
        )
        .output()
        .expect("run output program");
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"A");
        assert!(output.stderr.is_empty());
    } else {
        eprintln!("SKIP: output program execution requires macOS ARM64; cross-emission checked");
    }
    project.write(
        "build.omg",
        &build.replace("::ProcessInput", "::ProcessOutput"),
    );
    let Err(diagnostics) =
        compile(request()).and_then(compiler::CompileOutcomes::into_single_report)
    else {
        panic!("output cannot satisfy its physical exclusion");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("excluded physical authority class ProcessOutput")),
        "{diagnostics:?}"
    );

    // Optional optimizations cannot satisfy the physical axis either: the
    // verdict is adjudicated against the retained mechanism closure, whose
    // provider leaves keep their exercised dispositions regardless of the
    // optimization record the artifact commits.
    project.write(
        "build.omg",
        &build.replace(
            "    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessInput);\n",
            "    builder.optimizations.enable(Optimization::SparseConditionalConstantPropagation);\n    builder.optimizations.enable(Optimization::ControlFlowCleanup);\n    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);\n",
        ),
    );
    let Err(diagnostics) =
        compile(request()).and_then(compiler::CompileOutcomes::into_single_report)
    else {
        panic!("an optimized output cannot satisfy its physical exclusion");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("excluded physical authority class ProcessOutput")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_retained_physical_exclusion_replays_against_the_product_by_consumers() {
    // The retained Terminal product carries the authored exclusion union;
    // an independent consumer realizing it through
    // `realize_retained_native_artifact` re-runs the same mechanism-closure
    // adjudication the producing gate enforced — no source is consulted.
    // ProcessOutput violates this closure (the Console provider exercises
    // it), unrelated ProcessInput admits, and the verdict is identical with
    // and without a receiver permission policy because the physical axis is
    // independent of receiver admission.
    const OUTPUT_PROGRAM: &str = r#"use omega_language_std::console;
use omega::language::core::service;
data Main { console: Binding<Console>; }
machine Main::main(&mut self) reaches Console { self.console.write_byte(65); }
"#;
    let retained_for = |class: effects::TerminalAuthorityClass| {
        let project = TempProject::new();
        project.write("main.omg", OUTPUT_PROGRAM);
        project.write(
            "build.omg",
            &console_output_build(
                "physical-exclusion-replay",
                &format!(
                    "    builder.exclude_physical_authority(PhysicalAuthorityClass::{class:?});\n"
                ),
            ),
        );
        let inputs = console_package_inputs(&project, "physical-exclusion-replay", true);
        let retained = compile(
            CompileRequest::new(CompileOptions {
                root_path: project.main(),
                build_dir: None,
                target_name: Some("macos_arm64".into()),
            })
            .with_package_inputs(inputs)
            .with_requested_product(RequestedCompileProduct::TerminalArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("the excluded output product retains")
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
        (project, retained)
    };
    let realize = |retained: compilation_report::RetainedTerminalArtifact,
                   receiving_policy: bool| {
        // The accepted-package policy must rejoin the retained permission
        // rows exactly; the receiving policy is either that same policy or
        // absent, so this leg also covers receiver-policy independence.
        let proposal = retained
            .native_realization_proposal()
            .expect("native proposal");
        let subsystem = proposal.subsystem();
        let accepted_package_policy =
            native_realization::terminal_authority_permission_policy_with_rows(
                proposal.package_terminal_authority_permissions().to_vec(),
            )
            .expect("retained package permissions form the accepted policy");
        let terminal_authority_permission_policy =
            receiving_policy.then(|| accepted_package_policy.clone());
        realize_retained_native_artifact(
            retained,
            compiler::RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_package_policy,
                terminal_authority_permission_policy,
                image_request: native_realization::ExecutableImageEmissionRequest::direct(
                    subsystem,
                ),
                imports: &[],
            },
        )
        .map_err(|(_, diagnostics)| diagnostics)
    };

    let (_project, retained) = retained_for(effects::TerminalAuthorityClass::ProcessOutput);
    let diagnostics = realize(retained, true)
        .expect_err("a retained closure exercising ProcessOutput cannot satisfy its exclusion");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("excluded physical authority class ProcessOutput")),
        "{diagnostics:?}"
    );

    for with_receiving_policy in [false, true] {
        let (_project, retained) = retained_for(effects::TerminalAuthorityClass::ProcessInput);
        realize(retained, with_receiving_policy)
            .unwrap_or_else(|diagnostics| {
                panic!("an unrelated physical class must replay admitted: {diagnostics:#?}")
            })
            .as_direct()
            .expect("a direct image request produces direct custody")
            .validate()
            .expect("the admitted native artifact validates");
    }
}

/// The Console output program against an app-authored provider whose
/// satisfied methods exercise no physical authority: the silent replacement
/// is the benign side of the provider-replacement control — the same product
/// keeps its exclusion only until a provider that actually exercises the
/// class is selected back in.
const SILENT_CONSOLE_MAIN: &str = r#"use omega_language_std::console;
use omega::language::core::service;

pub data SilentConsole { }

machine SilentConsole::write_line(text: &[u8])
satisfies Console::write_line
{
}

machine SilentConsole::write(text: &[u8])
satisfies Console::write
{
}

machine SilentConsole::read_line(out_line: &mut [u8]) -> LineReadResult
satisfies Console::read_line
crashes Trap
{
    crash Trap;
}

machine SilentConsole::read_byte() -> ByteRead
satisfies Console::read_byte
crashes Trap
{
    crash Trap;
}

machine SilentConsole::write_byte(byte: i32)
satisfies Console::write_byte
{
}

machine SilentConsole::exit_process(return_code: i32)
satisfies Console::exit_process
{
    transition true { true -> idle() }
    state idle() {
        transition true { true -> idle() }
    }
}

data Main { console: Binding<Console>; }
machine Main::main(&mut self) reaches Console { self.console.write_byte(65); }
"#;

#[test]
fn replacing_a_silent_console_provider_with_excluded_behavior_rejects() {
    // Provider substitution cannot launder a physical exclusion: the verdict
    // tracks the SELECTED provider's exercised mechanism classes, not the
    // presence of a `Binding<Console>` field.
    let build_for = |provider: &str| {
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("silent-console-substitution");
{}    builder.select_provider<omega_language_std::Console, {}>();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);
}}
"#,
            bundled_standard_library_dependency_declaration(),
            provider
        )
    };

    // The silent provider admits: its write_byte exercises nothing, so the
    // ProcessOutput exclusion's mechanism-closure review finds no violated
    // class and the retained product forms for consumer replay.
    let silent = TempProject::new();
    silent.write("main.omg", SILENT_CONSOLE_MAIN);
    silent.write("build.omg", &build_for("SilentConsole"));
    let inputs = console_package_inputs(&silent, "silent-console-substitution", false);
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: silent.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_package_inputs(inputs)
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a provider exercising no ProcessOutput satisfies the exclusion");
    report
        .into_retained_native_artifact()
        .expect("retained native product")
        .validate()
        .expect("the admitted silent-provider artifact validates");

    // Selecting the native provider back exercises ProcessOutput through the
    // write_byte mechanism leaf, and the identical authored exclusion now
    // rejects at the same mechanism-closure adjudication.
    let native = TempProject::new();
    native.write("main.omg", SILENT_CONSOLE_MAIN);
    native.write(
        "build.omg",
        &build_for("omega_language_std::ConsoleNativeProvider"),
    );
    let inputs = console_package_inputs(&native, "silent-console-substitution", true);
    let diagnostics = compile(
        CompileRequest::new(CompileOptions {
            root_path: native.main(),
            build_dir: None,
            target_name: Some("macos_arm64".into()),
        })
        .with_package_inputs(inputs)
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("the replaced provider exercises the excluded class");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("excluded physical authority class ProcessOutput")),
        "{diagnostics:?}"
    );
}

#[test]
fn authored_exclude_crash_retains_exact_case_identity_and_authored_span() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-crash");
    builder.exclude_crash(CrashCause::Trap);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("an authored crash exclusion must check");
    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("checked build machine");

    let [exclusion] = checked.behavior_exclusions() else {
        panic!("exactly one authored exclusion must be retained");
    };
    let build_evaluation::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } =
        exclusion.kind
    else {
        panic!("the authored exclusion is a crash cause");
    };
    assert_eq!(cause, terminal_psi::CrashCause::Trap);
    assert_eq!(
        case_symbol,
        toolchain_crash_cause_case(&checked, "Trap"),
        "the exclusion must name the exact toolchain CrashCause case symbol"
    );
    assert_eq!(exclusion.selecting_machine, build.symbol);
    let source = checked
        .typed
        .symbols
        .source_file(exclusion.source_span)
        .expect("the exclusion must retain authored source custody");
    assert_eq!(
        source.path.file_name().and_then(|name| name.to_str()),
        Some("build.omg")
    );
    assert!(exclusion.source_span.span.start < exclusion.source_span.span.end);
}

#[test]
fn repeated_and_reordered_exclusions_collapse_to_the_canonical_union() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-union");
    builder.exclude_crash(CrashCause::Abort);
    builder.exclude_crash(CrashCause::Trap);
    builder.exclude_crash(CrashCause::Abort);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("repeated authored exclusions must check");

    // Authored occurrences keep their provenance; the canonical admission
    // set is order- and repetition-insensitive.
    assert_eq!(checked.behavior_exclusions().len(), 3);
    let exclusions =
        build_evaluation::authored_behavior_exclusion_set(checked.behavior_exclusions());
    assert!(exclusions.excludes_crash_cause(terminal_psi::CrashCause::Trap));
    assert!(exclusions.excludes_crash_cause(terminal_psi::CrashCause::Abort));
    assert_eq!(exclusions.crash_causes().len(), 2);
}

#[test]
fn a_reachable_helper_may_spell_the_selection_for_the_root() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine restrict(builder: &mut Build) {
    builder.exclude_crash(CrashCause::Abort);
}

machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-helper");
    restrict(builder);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("a helper inside the checked call scope may spell the selection");
    let restrict = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "restrict")
        .expect("checked helper machine");

    let [exclusion] = checked.behavior_exclusions() else {
        panic!("the helper's authored exclusion must be retained");
    };
    let build_evaluation::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } =
        exclusion.kind
    else {
        panic!("the authored exclusion is a crash cause");
    };
    assert_eq!(cause, terminal_psi::CrashCause::Abort);
    assert_eq!(case_symbol, toolchain_crash_cause_case(&checked, "Abort"));
    assert_eq!(exclusion.selecting_machine, restrict.symbol);
}

#[test]
fn a_spelled_exclusion_that_never_executes_selects_nothing() {
    // Exclusions are EVALUATED selections: `unused` spells the call but the
    // build entry never calls it, so nothing is admitted. The spelled text
    // alone cannot select a behavior requirement.
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine unused(builder: &mut Build) {
    builder.exclude_crash(CrashCause::Trap);
}

machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-out-of-scope");
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("a call that never executes selects nothing rather than rejecting");
    assert!(checked.behavior_exclusions().is_empty());
}

#[test]
fn a_selection_inside_an_untaken_branch_selects_nothing() {
    // The exclusion sits in a transition arm the evaluation never takes:
    // an untaken branch is not an executed selection even though the call
    // site is spelled inside the live build machine itself.
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-untaken");
    let flag: bool = false;
    transition flag { true -> exclude(builder) false -> done() }
    state exclude(builder: &mut Build) { builder.exclude_crash(CrashCause::Trap); }
    state done() { }
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("an untaken branch selects nothing rather than rejecting");
    assert!(checked.behavior_exclusions().is_empty());
}

#[test]
fn the_exclusion_argument_is_the_evaluated_cause_value() {
    // The selection carries the EVALUATED cause: a bound local carrying an
    // exact toolchain case selects exactly like the literal spelling.
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-indirect");
    let cause: CrashCause = CrashCause::Trap;
    builder.exclude_crash(cause);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("the evaluated CrashCause value is the selection");
    let [exclusion] = checked.behavior_exclusions() else {
        panic!("the evaluated cause must record exactly one exclusion");
    };
    let build_evaluation::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } =
        exclusion.kind
    else {
        panic!("the authored exclusion is a crash cause");
    };
    assert_eq!(cause, terminal_psi::CrashCause::Trap);
    assert_eq!(case_symbol, toolchain_crash_cause_case(&checked, "Trap"));
}

#[test]
fn an_authored_same_named_machine_is_not_the_toolchain_exclusion() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        r#"data Marker { }
machine Marker::exclude_crash(&mut self, cause: u8) { }

data Main { marker: Marker; }
machine Main::main(&mut self) {
    self.marker.exclude_crash(0);
}
const ANSWER: u32 = 42;
"#,
    );
    project.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.application(\"same-named-exclusion\"); }\n",
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("an authored same-named machine must not masquerade as the toolchain marker");
    assert!(checked.behavior_exclusions().is_empty());
}

/// The selected entry's closure retains a possible `CrashCause::Trap` crash
/// terminator inside `effect`, whose declared `crashes Trap` contract the
/// caller admits. The entry itself still returns normally; a crash site
/// inside an unconditionally crashing entry would lose its checked unit plan
/// before Terminal production.
const TRAPPING_MAIN: &str = r#"machine launch()
crashes Trap
{
    let v: bool = effect();
}
machine effect() -> bool crashes Trap { crash Trap; }
"#;

/// The same entry shape without any crash-capable callee carries no possible
/// crash terminator in its selected closure.
const QUIET_MAIN: &str = "machine launch() { let marker: u8 = 0; }\n";

fn product_build(name: &str, extra: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"{name}\");\n    builder.roots.bind(windows_x86_64::ProgramEntry, launch);\n{extra}}}\n"
    )
}

fn product_request(root: PathBuf, product: RequestedCompileProduct) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: root,
        build_dir: None,
        target_name: Some("windows_x86_64".into()),
    })
    .with_requested_product(product)
}

#[test]
fn exclude_crash_trap_rejects_a_terminal_product_that_can_trap() {
    let project = TempProject::new();
    project.write("main.omg", TRAPPING_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "exclusion-trap-product",
            "    builder.exclude_crash(CrashCause::Trap);\n",
        ),
    );

    let diagnostics = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("a selected closure retaining a possible Trap must reject at admission");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("crash cause Trap")
                && diagnostic.message.contains("crash terminator")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source_span.is_some()),
        "the rejection must point at the authored exclusion span"
    );
}

#[test]
fn exclude_crash_trap_rejects_the_native_product_route() {
    let project = TempProject::new();
    project.write("main.omg", TRAPPING_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "exclusion-trap-native",
            "    builder.exclude_crash(CrashCause::Trap);\n",
        ),
    );

    let diagnostics = compile(product_request(
        project.main(),
        RequestedCompileProduct::NativeArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("native admission must reject a closure retaining a possible Trap");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("crash cause Trap")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn an_exclusion_the_closure_never_reaches_still_admits() {
    let project = TempProject::new();
    project.write("main.omg", TRAPPING_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "exclusion-abort-product",
            "    builder.exclude_crash(CrashCause::Abort);\n",
        ),
    );

    // The retained possible crash is a Trap, not an Abort: exact causes, not
    // the broad published crash ceiling, decide the verdict.
    let report = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("an Abort exclusion admits a closure whose only possible crash is a Trap");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    let proposal = retained
        .native_realization_proposal()
        .expect("retained native proposal");
    let expected = build_evaluation::BehaviorExclusions::from_selections([
        build_evaluation::BehaviorExclusion::CrashCause(terminal_psi::CrashCause::Abort),
    ]);
    assert_eq!(
        proposal.behavior_exclusions(),
        &expected,
        "the retained proposal must carry the exact canonical exclusion union"
    );
}

#[test]
fn exclude_crash_trap_admits_a_trap_free_closure() {
    let project = TempProject::new();
    project.write("main.omg", QUIET_MAIN);
    project.write(
        "build.omg",
        &product_build(
            "exclusion-quiet-product",
            "    builder.exclude_crash(CrashCause::Trap);\n",
        ),
    );

    let report = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a selected closure with no possible Trap satisfies the exclusion");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    let proposal = retained
        .native_realization_proposal()
        .expect("retained native proposal");
    assert!(
        proposal
            .behavior_exclusions()
            .excludes_crash_cause(terminal_psi::CrashCause::Trap)
    );
}

#[test]
fn optimization_selection_cannot_satisfy_an_exclusion() {
    // The crash state sits behind a provably-false guard: optional Psi
    // optimization may remove the crash site from the published module, but
    // exclusion admissibility is decided on the unoptimized closure the
    // artifact's optimization record commits as its input.
    let project = TempProject::new();
    project.write(
        "main.omg",
        r#"machine launch()
crashes Trap
{
    let v: bool = effect();
}
machine effect() -> bool
crashes Trap
{
    transition (1 == 2) { true -> boom() false -> ok() }
    state boom() -> bool { crash Trap; }
    state ok() -> bool { true }
}
"#,
    );
    project.write(
        "build.omg",
        &product_build(
            "exclusion-optimized",
            "    builder.optimizations.enable(Optimization::SparseConditionalConstantPropagation);\n    builder.optimizations.enable(Optimization::ControlFlowCleanup);\n    builder.exclude_crash(CrashCause::Trap);\n",
        ),
    );

    let diagnostics = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("optional optimization must not determine exclusion admissibility");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("crash cause Trap")),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    // Witness that the selected passes really did transform the published
    // module: without the exclusion the same build produces an artifact whose
    // optimization record commits a changed output. The gate therefore could
    // not have replayed the artifact's published semantics — it checked the
    // committed optimization input.
    let unexcluded = TempProject::new();
    unexcluded.write(
        "main.omg",
        &fs::read_to_string(project.0.join("main.omg")).expect("reread fixture main"),
    );
    unexcluded.write(
        "build.omg",
        &product_build(
            "exclusion-optimized-witness",
            "    builder.optimizations.enable(Optimization::SparseConditionalConstantPropagation);\n    builder.optimizations.enable(Optimization::ControlFlowCleanup);\n",
        ),
    );
    let report = compile(product_request(
        unexcluded.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the same optimized program produces an artifact without exclusions");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    assert_ne!(
        retained.artifact().optimization().input_semantic(),
        retained.artifact().manifest().semantic(),
        "the selected optimization must change the published module for this witness to mean anything"
    );
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode published semantics");
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .all(|block| !matches!(block.terminator, terminal_psi::Terminator::Crash { .. })),
        "the optimized artifact no longer retains the crash site the exclusion rejected"
    );
}

#[test]
fn a_retained_exclusion_is_replayed_against_the_artifact_by_consumers() {
    // Produce the trapping artifact without authored exclusions so a proposal
    // can be rebuilt against it; the receiving replay is the same Terminal
    // closure check the producing admission gate ran.
    let project = TempProject::new();
    project.write("main.omg", TRAPPING_MAIN);
    project.write("build.omg", &product_build("exclusion-replay", ""));

    let report = compile(product_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the unexcluded trapping artifact produces");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    let proposal = retained
        .native_realization_proposal()
        .expect("retained native proposal");
    assert!(
        proposal.behavior_exclusions().is_empty(),
        "no exclusion was authored for this product"
    );

    let mut excluded = build_evaluation::BehaviorExclusions::default();
    excluded.union(&build_evaluation::BehaviorExclusions::from_selections([
        build_evaluation::BehaviorExclusion::CrashCause(terminal_psi::CrashCause::Trap),
    ]));
    assert!(
        compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: proposal.target_profile(),
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: proposal.external_binding_rows().to_vec(),
                boundary_opaque_applications: proposal.boundary_opaque_applications().clone(),
                package_terminal_authority_permissions: proposal
                    .package_terminal_authority_permissions()
                    .to_vec(),
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: excluded
            }
        )
        .is_err(),
        "a retained exclusion the artifact's closure violates must reject replay"
    );

    // The honest retained policy — an empty union here — replays cleanly
    // against the same artifact.
    assert!(
        compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: proposal.target_profile(),
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: proposal.external_binding_rows().to_vec(),
                boundary_opaque_applications: proposal.boundary_opaque_applications().clone(),
                package_terminal_authority_permissions: proposal
                    .package_terminal_authority_permissions()
                    .to_vec(),
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: proposal.behavior_exclusions().clone()
            }
        )
        .is_ok(),
        "the artifact's retained exclusion policy must replay satisfied"
    );
}

// The named next native control for physical exclusions
// (TASKS.md BUILD-EXCLUSION-REALIZATION): the sink-app/logger-kit
// composition from `tests/behavior_exclusions.rs`, targeted at
// macos_arm64, with `exclude_service<Sink>` replaced by
// `exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput)`
// and a NativeArtifact request. The fixture directories stay owned by
// the semantic-exclusions work; this probe replicates the composition
// inline.
const LOGGER_KIT_MAIN: &str = r#"use omega::language::core::service;

pub boundary trait Sink {
    machine emit(text: &[u8]);
}

pub data QuietSink { }

machine QuietSink::emit(text: &[u8])
satisfies Sink::emit
{
}

pub machine log_quiet(text: &[u8])
reaches Sink
{
    let marker: u8 = 0;
}
"#;

const LOGGER_KIT_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("logger-kit");
    builder.select_provider<Sink, QuietSink>();
}
"#;

const SINK_APP_MAIN: &str = r#"use logger_kit::main;
use omega::language::core::service;

data Main {
    sink: Binding<Sink>;
}

machine Main::main(&mut self)
reaches Sink
{
    self.sink.emit("still an invocation");
}
"#;

// Native realization of the sink composition (TASKS.md
// BUILD-EXCLUSION-REALIZATION): the silent service invocation — a
// provider-attached literal payload and a Unit-result boundary call into the
// installed provider — now carries native custody end-to-end, so the
// authored ProcessOutput exclusion admits the emitted image on every
// target. `exclude_service<Sink>` for the same invocation still rejects at
// the semantic layer (`silent_provider_for_an_actual_service_invocation_is_prohibited`).
fn sink_composition_project(project: &TempProject, target: &str) {
    project.write("logger-kit/main.omg", LOGGER_KIT_MAIN);
    project.write("logger-kit/build.omg", LOGGER_KIT_BUILD);
    project.write("app/main.omg", SINK_APP_MAIN);
    project.write(
        "app/build.omg",
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("sink-physical");
    builder.depend(Source::Path {{ location: "../logger-kit" }});
    builder.select_provider<logger_kit::Sink, logger_kit::QuietSink>();
    builder.exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput);
    builder.roots.bind({target}::ProgramEntry, Main::main);
}}
"#
        ),
    );
}

fn sink_composition_request(
    project: &TempProject,
    target: &str,
    product: RequestedCompileProduct,
) -> CompileRequest {
    let root_identity = fixture_package_identity(91);
    let library_identity = fixture_package_identity(92);
    let inputs = package_compilation::PackageCompilationInputs::new_package(
        root_identity,
        vec![
            package_compilation::PackageSourceBinding::new(
                root_identity,
                "sink-physical",
                project.0.join("app"),
            ),
            package_compilation::PackageSourceBinding::new(
                library_identity,
                "logger-kit",
                project.0.join("logger-kit"),
            ),
        ],
        vec![package_compilation::PackageDependencyBinding::new(
            root_identity,
            "logger_kit",
            library_identity,
        )],
    )
    .expect("fixture packages");
    CompileRequest::new(CompileOptions {
        root_path: project.0.join("app/main.omg"),
        build_dir: None,
        target_name: Some(target.into()),
    })
    .with_package_inputs(inputs)
    .with_requested_product(product)
}

#[test]
fn sink_composition_physical_exclusion_reaches_native_execution() {
    for target in ["macos_arm64", "linux_x86_64", "windows_x86_64"] {
        let project = TempProject::new();
        sink_composition_project(&project, target);
        let report = compile(sink_composition_request(
            &project,
            target,
            RequestedCompileProduct::NativeArtifact,
        ))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{target}: the silent-service composition realizes: {diagnostics:?}")
        });
        assert!(
            report.retained_native_artifact().is_some(),
            "{target}: a retained native artifact"
        );
    }
}

#[test]
fn sink_composition_physical_exclusion_publishes_and_runs_empty_on_the_host() {
    let target = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos_arm64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows_x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux_x86_64"
    } else {
        eprintln!(
            "SKIP: native exclusion execution requires macOS ARM64, Windows x64, or Linux x64"
        );
        return;
    };
    let project = TempProject::new();
    sink_composition_project(&project, target);
    let report = compile(sink_composition_request(
        &project,
        target,
        RequestedCompileProduct::NativeArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("silent-service native product")
    .publish_retained_native_artifact(&project.0.join("out"))
    .expect("checked native publication");
    let executable = report
        .checked_native_executable_path()
        .expect("published executable");
    let output = std::process::Command::new(executable)
        .output()
        .expect("run checked native product");
    assert!(output.status.success(), "native result: {output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn retained_sink_composition_replays_against_the_product_by_consumers() {
    // The retained Terminal product carries the authored ProcessOutput
    // exclusion; an independent consumer realizing it re-runs the same
    // mechanism-closure adjudication — the silent provider exercises no
    // class, so the replay admits and the artifact validates.
    for target in ["macos_arm64", "linux_x86_64", "windows_x86_64"] {
        let project = TempProject::new();
        sink_composition_project(&project, target);
        let retained = compile(sink_composition_request(
            &project,
            target,
            RequestedCompileProduct::TerminalArtifact,
        ))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{target}: the sink composition retains: {diagnostics:?}")
        })
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
        let proposal = retained
            .native_realization_proposal()
            .expect("native proposal");
        let subsystem = proposal.subsystem();
        let accepted_package_policy =
            native_realization::terminal_authority_permission_policy_with_rows(
                proposal.package_terminal_authority_permissions().to_vec(),
            )
            .expect("retained package permissions form the accepted policy");
        realize_retained_native_artifact(
            retained,
            compiler::RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy: accepted_package_policy,
                terminal_authority_permission_policy: None,
                image_request: native_realization::ExecutableImageEmissionRequest::direct(
                    subsystem,
                ),
                imports: &[],
            },
        )
        .unwrap_or_else(|(_, diagnostics)| {
            panic!("{target}: the silent closure replays admitted: {diagnostics:#?}")
        })
        .as_direct()
        .expect("a direct image request produces direct custody")
        .validate()
        .expect("the admitted native artifact validates");
    }
}

// The foreign-boundary e2e leg of BUILD-EXCLUSION-REALIZATION: a real
// composition whose boundary requirement binds `ProviderBinding::Import`
// reaches the mechanism-closure review under an authored exclusion. The
// receiving policy's explicit row classifies the normalized foreign
// mechanism with an exercised class; the exclusion then rejects the closure,
// naming the requirement and the foreign mechanism — the same adjudication a
// compiler-intrinsic leaf receives (the row-level seam is pinned in
// native-realization's
// `a_foreign_mechanism_leaf_exercising_an_excluded_class_rejects`).
const FOREIGN_BOUNDARY_MAIN: &str = r#"use omega::language::core::external_binding;

pub data ForeignMath {}

pub boundary requirement ForeignMath::exit_with(code: i32);

data ForeignMathProvider {}

linux_x86_64 machine foreign_exit_binding() -> ForeignBinding<9, 4, 11> {
    ForeignBinding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libc.so.6",
            symbol: "exit",
            version: "GLIBC_2.2.5",
        },
    }
}

machine ForeignMathProvider::exit_leaf(code: i32)
satisfies ForeignMath::exit_with via foreign_exit_binding();

data Main {}
machine Main::main(&mut self) {
    ForeignMath::exit_with(70);
}
"#;

fn foreign_boundary_build(exclusion: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"foreign-boundary-exclusion\");\n    builder.exclude_physical_authority({exclusion});\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n}}\n"
    )
}

fn foreign_boundary_request(root: PathBuf, product: RequestedCompileProduct) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: root,
        build_dir: None,
        target_name: Some("linux_x86_64".into()),
    })
    .with_requested_product(product)
}

// The mechanism identity is exact-identity, not name-matched: it is derived
// from the retained proposal's external binding rows (normalized locator +
// canonical boundary entry plan), the same derivation the admission path
// performs on the settled import.
fn foreign_boundary_policy(
    retained: &compilation_report::RetainedTerminalArtifact,
    exercised: &[effects::TerminalAuthorityClass],
) -> native_realization::TerminalAuthorityPolicy {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let rows = proposal
        .external_binding_rows()
        .iter()
        .filter_map(|row| {
            let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding else {
                return None;
            };
            Some(native_realization::TerminalAuthorityPolicyRow::new(
                native_realization::normalized_foreign_terminal_mechanism(
                    locator,
                    row.boundary_entry_plan.as_ref()?,
                )
                .expect("retained foreign boundary plan is canonical"),
                effects::TerminalAuthorityDisposition::from_classes(exercised.iter().copied()),
            ))
        })
        .collect::<Vec<_>>();
    assert!(
        !rows.is_empty(),
        "at least one normalized import policy row expected"
    );
    native_realization::terminal_authority_policy_with_rows(rows)
        .expect("receiving policy has exact normalized import rows")
}

// A minimal `ProviderExecutionEvidence` for the one import the composition
// selects — the settlement admission needs exact provider-plan custody, not
// a real interpreter report.
#[derive(Debug)]
struct ForeignLeafExecution {
    requirement: String,
    plan_report_identity: u64,
}

impl installation_evidence::ProviderExecutionEvidence for ForeignLeafExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
    }
    fn provider_plan_report_identity(&self) -> u64 {
        self.plan_report_identity
    }
    fn provider_execution_report_identity(&self) -> u64 {
        0x4d41_4348_4f01
    }
    fn provider_execution_report_fingerprint(&self) -> u64 {
        0x4d41_4348_4f02
    }
    fn normalized_root_report_identity(&self) -> u64 {
        0x4d41_4348_4f03
    }
    fn boundary_contract_report_fingerprint(&self) -> u64 {
        0x4d41_4348_4f04
    }
}

fn admit_foreign_import(
    retained: &compilation_report::RetainedTerminalArtifact,
) -> (
    ForeignLeafExecution,
    task_plans::AdmittedSameStackContribution,
) {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let matches = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::Import { .. }
                )
                .then_some((plan, row))
            })
        })
        .collect::<Vec<_>>();
    let [(plan, row)] = matches.as_slice() else {
        panic!("one selected evaluated import expected")
    };
    let plan_report_identity = plan.report_fingerprint();
    let plan_commitment = task_plans::SameStackProviderPlanCommitment::from_digest(
        *plan.identity_digest().as_bytes(),
    );
    let requirement = row.requirement_identity.clone();
    let same_stack = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: plan_report_identity,
            provider_plan_commitment: plan_commitment,
            requirement_identity: requirement.clone(),
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0x454c_4600_0110,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        plan_report_identity,
        plan_commitment,
        &requirement,
    )
    .expect("exact provider-plan custody admits the foreign leaf");
    (
        ForeignLeafExecution {
            requirement,
            plan_report_identity,
        },
        same_stack,
    )
}

#[test]
fn a_source_evaluated_import_exercising_an_excluded_class_rejects_at_closure_review() {
    let project = TempProject::new();
    project.write("main.omg", FOREIGN_BOUNDARY_MAIN);
    project.write(
        "build.omg",
        &foreign_boundary_build("PhysicalAuthorityClass::ProcessTermination"),
    );
    let retained = compile(foreign_boundary_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the boundary-requirement composition compiles to a Terminal product")
    .into_retained_terminal_artifact()
    .expect("the Terminal product retains its native realization proposal");
    let subsystem = retained
        .native_realization_proposal()
        .expect("native proposal")
        .subsystem();

    // A foreign mechanism carries no compiler-known disposition: the
    // exercised class is supplied by the receiving policy's explicit row —
    // exercised, not permissioned (no permission policy is attached at all)
    // — exactly as the settled-filesystem cohort or the consumer's package
    // policy supplies it in production.
    let policy = foreign_boundary_policy(
        &retained,
        &[effects::TerminalAuthorityClass::ProcessTermination],
    );
    let (execution, same_stack) = admit_foreign_import(&retained);
    let (_, diagnostics) = realize_retained_native_artifact(
        retained,
        compiler::RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy.clone(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: None,
            image_request: native_realization::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[compiler::SourceEvaluatedImportSettlement::new(
                &execution,
                &same_stack,
            )],
        },
    )
    .expect_err("an import leaf exercising the excluded class cannot realize");
    let message = format!("{diagnostics:?}");
    assert!(
        message.contains("behavior exclusions"),
        "the exclusion adjudication must fire: {message}"
    );
    assert!(message.contains("ForeignMath::exit_with"), "{message}");
    assert!(message.contains("NormalizedForeign"), "{message}");
    assert!(message.contains("ProcessTermination"), "{message}");

    // Control: excluding a class no leaf exercises passes mechanism
    // adjudication; the import-bearing image then proceeds past the review
    // and stops later — the union, not the mechanism inventory, decides.
    project.write(
        "build.omg",
        &foreign_boundary_build("PhysicalAuthorityClass::PortIo"),
    );
    let retained = compile(foreign_boundary_request(
        project.main(),
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the boundary-requirement composition compiles to a Terminal product")
    .into_retained_terminal_artifact()
    .expect("the Terminal product retains its native realization proposal");
    let subsystem = retained
        .native_realization_proposal()
        .expect("native proposal")
        .subsystem();
    let (execution, same_stack) = admit_foreign_import(&retained);
    let (_, diagnostics) = realize_retained_native_artifact(
        retained,
        compiler::RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: None,
            image_request: native_realization::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[compiler::SourceEvaluatedImportSettlement::new(
                &execution,
                &same_stack,
            )],
        },
    )
    .expect_err("the import-bearing image stops at interpreter custody");
    let message = format!("{diagnostics:?}");
    assert!(
        !message.contains("behavior exclusions"),
        "no exclusion adjudicates: {message}"
    );
}
