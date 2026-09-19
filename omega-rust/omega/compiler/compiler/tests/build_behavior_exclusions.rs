//! End-to-end coverage for `builder.exclude_crash(CrashCause::X)`
//! (wiki/spec/build/behavior_exclusions.md): the authored exclusion is a
//! product-admission requirement recorded when the checked build evaluation
//! actually executes the call against the root `Build`, retaining the exact
//! toolchain `CrashCause` case identity and the authored source span.

use compiler::CheckedCompileRequest;
use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, compile, compile_to_checked,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

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
    let definition = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str() == "CrashCause"
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
        .expect("exact toolchain CrashCause declaration");
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
        .expect("toolchain CrashCause case")
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
