use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked, realize_retained_terminal_artifact_with_source_evaluated_imports,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new(build: &str) -> Self {
        Self::with_main("const ANSWER: u32 = 42;\n", build)
    }

    fn with_main(main: &str, build: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../target/omega-test-projects")
            .join(format!(
                "omega-build-target-activation-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).expect("create temporary in-repository Omega project");
        fs::write(path.join("main.omg"), main).expect("write temporary Omega source");
        fs::write(path.join("build.omg"), build).expect("write temporary Omega build source");
        Self(path)
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

fn exact_target_build(body: &str) -> String {
    exact_profile_build("windows_x86_64", body)
}

fn exact_profile_build(target: &str, body: &str) -> String {
    format!(
        "target {target} {{ }}\nmachine build(builder: &mut Build) {{\n    builder.application(\"target-activation\");\n{body}\n}}\n"
    )
}

fn pass_canary_main(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../tests/omega/pass")
        .join(name)
        .join("main.omg")
}

fn diagnostic_text(project: &TempProject) -> String {
    compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect_err("immutable target violation must reject")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn exact_target_is_source_visible_and_drives_build_evaluation() {
    let project = TempProject::new(&exact_target_build(
        r#"    transition builder.target {
        TargetProfile::WindowsX86_64 -> windows(builder)
        _ -> other(builder)
    }
    state windows(builder: &mut Build) {
        builder.subsystem = Subsystem::Gui;
    }
    state other(builder: &mut Build) {
        builder.subsystem = Subsystem::Console;
    }"#,
    ));

    let checked = compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect("the selected target must be an ordinary readable Omega value");
    assert_eq!(
        checked.selected_target_profile(),
        Some(omega_target::TargetProfile::WindowsX64)
    );
    assert_eq!(checked.subsystem(), 2, "Windows branch must select Gui");
}

#[test]
fn legacy_and_canonical_cli_spellings_select_the_same_canonical_profile() {
    let project = TempProject::new(&exact_target_build(""));
    let legacy = compile_to_checked(&project.main(), Some("windows_x64"))
        .expect("legacy CLI alias should normalize before source selection");
    let canonical = compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect("canonical CLI spelling should compile");

    assert_eq!(
        legacy.selected_target_profile(),
        canonical.selected_target_profile()
    );
    assert_eq!(
        legacy.selected_native_target(),
        canonical.selected_native_target()
    );
    assert_eq!(
        legacy
            .selected_target_profile()
            .expect("selected profile")
            .target_name(),
        "windows_x86_64"
    );
}

#[test]
fn targetless_checking_retains_no_synthetic_target() {
    let project = TempProject::new(
        "machine build(builder: &mut Build) { builder.application(\"targetless\"); }\n",
    );

    let checked = compile_to_checked(&project.main(), None).expect("targetless check should pass");
    assert_eq!(checked.selected_target_profile(), None);
}

#[test]
fn direct_target_assignment_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        "    builder.target = TargetProfile::MacosArm64;",
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("Build.target is compiler-owned and cannot be assigned"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn transient_target_overwrite_then_restore_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        r#"    builder.target = TargetProfile::MacosArm64;
    builder.target = TargetProfile::WindowsX86_64;"#,
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("Build.target is compiler-owned and cannot be assigned"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn exclusive_target_borrow_is_rejected() {
    let project = TempProject::new(&exact_target_build(
        "    let target: &mut TargetProfile = &mut builder.target;",
    ));
    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains(
            "Build.target is compiler-owned and cannot enter a mutable or write-only borrow"
        ),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn authored_legacy_build_is_rejected_instead_of_receiving_a_hidden_target() {
    let project = TempProject::new(
        r#"target windows_x86_64 { }
data Build {
    subsystem: Subsystem;
    freestanding: bool;
}
data Subsystem {
    case Console;
}
machine build(builder: &mut Build) { }
"#,
    );

    let diagnostics = diagnostic_text(&project);
    assert!(
        diagnostics.contains("must not declare toolchain package vocabulary `Build`"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn exact_x86_build_must_opt_in_before_fma_admission_exists() {
    let baseline = TempProject::new(&exact_target_build(""));
    let checked = compile_to_checked(&baseline.main(), Some("windows_x86_64"))
        .expect("generic x86 baseline must remain available");
    assert_eq!(checked.x86_scalar_fma_provider(), None);

    let opted_in = TempProject::new(&exact_target_build(
        "    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;",
    ));
    let checked = compile_to_checked(&opted_in.main(), Some("windows_x86_64"))
        .expect("exact x86 build may select the canonical AVX+FMA3 deployment pair");
    let provider = checked
        .x86_scalar_fma_provider()
        .expect("explicit feature selection must retain one admitted provider");
    assert_eq!(provider.profile(), omega_target::TargetProfile::WindowsX64);
    assert!(provider.has_canonical_identity());
    assert_eq!(
        provider.deployment().features(),
        &omega_target::X86_SCALAR_FMA_REQUIRED_FEATURES
    );
    assert!(
        checked.x86_scalar_fma_plan_associations().is_empty(),
        "feature admission without source demand must not fabricate an association"
    );
}

#[test]
fn exact_x86_fma_demand_fails_closed_without_feature_admission() {
    let main = pass_canary_main("float/named_provider_fused_multiply_add_exit");
    for target in ["linux_x86_64", "windows_x86_64"] {
        let diagnostics = compile_to_checked(&main, Some(target))
            .expect_err("an exact-profile x86 FMA demand requires explicit deployment admission")
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            diagnostics.contains("requires explicit AVX+FMA3 admission"),
            "unexpected {target} diagnostics: {diagnostics}"
        );
        assert!(
            diagnostics.contains("F32::fused_multiply_add")
                && diagnostics.contains("F64::fused_multiply_add"),
            "each exact semantic slot must fail closed: {diagnostics}"
        );
    }
}

#[test]
fn admitted_x86_fma_demand_retains_exact_plan_associations() {
    for (target, profile, other_profile) in [
        (
            "linux_x86_64",
            omega_target::TargetProfile::LinuxX64,
            omega_target::TargetProfile::WindowsX64,
        ),
        (
            "windows_x86_64",
            omega_target::TargetProfile::WindowsX64,
            omega_target::TargetProfile::LinuxX64,
        ),
    ] {
        let main = pass_canary_main("float/x86_fma_plan_association");
        let checked = compile_to_checked(&main, Some(target))
            .unwrap_or_else(|diagnostics| panic!("{target} FMA admission failed: {diagnostics:?}"));
        let provider = checked
            .x86_scalar_fma_provider()
            .expect("explicit feature selection must retain one provider");
        let associations = checked.x86_scalar_fma_plan_associations();
        assert_eq!(
            associations.len(),
            2,
            "repeated calls deduplicate while F32 and F64 remain distinct"
        );
        assert_eq!(
            associations
                .iter()
                .map(|association| association.slot())
                .collect::<Vec<_>>(),
            vec![
                omega_target::X86ScalarFmaSlot::Binary32,
                omega_target::X86ScalarFmaSlot::Binary64,
            ]
        );
        assert_eq!(
            associations
                .iter()
                .map(|association| association.selected_builtin())
                .collect::<Vec<_>>(),
            vec![
                psi_symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
                psi_symbols::BuiltinFunction::FloatFusedMultiplyAddF64,
            ]
        );
        for association in associations {
            assert_eq!(association.selected_plan().target, target);
            assert_eq!(association.admitted_provider(), provider);
            assert!(
                association.matches_checked_inputs(checked.selected_provider_plans(), provider)
            );
        }
        assert_ne!(
            associations[0].selected_plan_digest(),
            associations[1].selected_plan_digest(),
            "F32 and F64 require distinct exact plans"
        );
        assert!(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .any(|plan| plan.schema.trait_name.contains("F32::multiply_then_add")),
            "the fixture must select multiply-then-add while the FMA association excludes it"
        );

        let wrong_profile_provider =
            omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
                other_profile,
                &omega_target::X86_SCALAR_FMA_REQUIRED_FEATURES,
            )
            .expect("canonical cross-profile provider fixture");
        assert!(
            associations.iter().all(|association| !association
                .matches_checked_inputs(checked.selected_provider_plans(), wrong_profile_provider)),
            "a provider for another policy profile must not substitute"
        );

        let mut substituted_plans = checked.selected_provider_plans().plans().to_vec();
        let associated_digest = associations[0].selected_plan_digest();
        substituted_plans
            .iter_mut()
            .find(|plan| plan.identity_digest() == associated_digest)
            .expect("associated plan must remain in the selected closure")
            .name
            .push_str(".substituted");
        let substituted =
            omega_effects::SelectedProviderPlanFacts::from_selected_plans(substituted_plans)
                .expect("structurally valid substituted closure");
        assert!(
            !associations[0].matches_checked_inputs(&substituted, provider),
            "compact coordinates cannot authorize an exact-plan substitution"
        );
        assert_eq!(provider.profile(), profile);
    }
}

#[test]
fn terminal_product_retains_exact_fma_operation_plan_and_x86_admission() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;

data Main { }

machine Main::main(&mut self) {
    let fused32: f32 = F32::fused_multiply_add(
        1.00000011920928955078125f32,
        0.99999988079071044921875f32,
        -1.0f32,
    );
    let fused64: f64 = F64::fused_multiply_add(
        1.0000000000000002220446049250313080847263336181640625f64,
        0.9999999999999997779553950749686919152736663818359375f64,
        -1.0f64,
    );
}
"#,
        &exact_profile_build(
            "linux_x86_64",
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;"#,
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .unwrap_or_else(|diagnostics| panic!("FMA Terminal custody failed: {diagnostics:#?}"));
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal report retains native proposal");
    let occurrences = proposal.ieee_float_fma_occurrences();
    assert_eq!(
        occurrences.len(),
        2,
        "both source-ordered selected FMA occurrences must survive"
    );
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.format())
            .collect::<Vec<_>>(),
        vec![
            psi_core::IeeeFloatFormat::Binary32,
            psi_core::IeeeFloatFormat::Binary64,
        ],
        "mixed formats must retain source order and distinct semantic slots"
    );
    assert_ne!(
        occurrences[0].terminal_operation(),
        occurrences[1].terminal_operation(),
        "plural source occurrences require distinct Terminal coordinates"
    );
    assert_ne!(
        occurrences[0].provider_plan_index(),
        occurrences[1].provider_plan_index(),
        "F32 and F64 must rejoin distinct selected plans"
    );
    let terminal_operations = occurrences
        .iter()
        .map(|occurrence| occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let application_realizations = proposal.boundary_application_realizations().rows();
    assert_eq!(application_realizations.len(), 2);
    assert!(application_realizations.iter().all(|realization| {
        realization.role()
            == omega_boundary_applications::BoundaryApplicationRealizationRole::ExactCompilerIntrinsic
            && realization.selected_plan_digest() != &[0; 32]
            && terminal_operations.contains(&realization.terminal_operation())
    }));
    let module = psi_terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("canonical Terminal semantics decode");

    for occurrence in occurrences {
        let plan = &proposal.selected_provider_plans().plans()[occurrence.provider_plan_index()];
        assert_eq!(plan.target, "linux_x86_64");
        let admission = occurrence
            .x86_admission()
            .expect("x86 occurrence retains admitted deployment provider");
        assert_eq!(
            admission.provider().profile(),
            omega_target::TargetProfile::LinuxX64
        );
        assert_eq!(
            admission.slot(),
            match occurrence.format() {
                psi_core::IeeeFloatFormat::Binary32 => omega_target::X86ScalarFmaSlot::Binary32,
                psi_core::IeeeFloatFormat::Binary64 => omega_target::X86ScalarFmaSlot::Binary64,
            }
        );
        let matching = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == occurrence.terminal_operation())
            .collect::<Vec<_>>();
        let [operation] = matching.as_slice() else {
            panic!("occurrence must name one exact Terminal operation")
        };
        assert!(matches!(
            operation.kind,
            psi_terminal::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        ));
    }
    retained
        .validate()
        .expect("FMA occurrence proposal replays against canonical artifact");
    let native = realize_retained_terminal_artifact_with_source_evaluated_imports(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        &[],
    )
    .unwrap_or_else(|diagnostics| panic!("FMA native custody failed: {diagnostics:#?}"));
    native
        .validate()
        .expect("FMA native artifact independently replays");
    let physical = native
        .physical_evidence()
        .expect("FMA native artifact retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 2);
    assert_eq!(physical.children().len(), 2);
    assert!(physical.children().iter().all(|child| {
        matches!(
            child.parent(),
            omega_terminal_psi_to_native_artifact::PhysicalChildParent::OperatorApplicationCoverage(
                _
            )
        ) && matches!(
            child.occurrence(),
            omega_terminal_psi_to_native_artifact::NativePhysicalOccurrence::Operator(_)
        ) && child.relocation()
            == omega_terminal_psi_to_native_artifact::PhysicalRelocationDisposition::DirectInstructionBytes
    }));
    let fma_functions = native
        .object()
        .functions()
        .iter()
        .filter(|function| !function.x86_scalar_fma_occurrences.is_empty())
        .collect::<Vec<_>>();
    let [function] = fma_functions.as_slice() else {
        panic!("one bounded FMA function must survive ordinary object construction")
    };
    assert_eq!(function.x86_scalar_fma.len(), 2);
    assert_eq!(function.x86_scalar_fma_occurrences.len(), 2);
    assert_eq!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .map(|occurrence| occurrence.terminal_operation)
            .collect::<Vec<_>>(),
        terminal_operations,
        "machine custody must retain the exact source-ordered Terminal roster"
    );
    assert_eq!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .map(|occurrence| occurrence.slot)
            .collect::<Vec<_>>(),
        vec![
            omega_target::X86ScalarFmaSlot::Binary32,
            omega_target::X86ScalarFmaSlot::Binary64,
        ]
    );
    let control = function
        .x86_floating_control
        .expect("native FMA function retains canonical MXCSR custody");
    assert_eq!(control.canonical_mxcsr, 0x1f80);
    assert_eq!(
        native.object().x86_scalar_fma_provider(),
        Some(function.x86_scalar_fma_occurrences[0].admitted_provider)
    );
    assert!(
        function
            .x86_scalar_fma_occurrences
            .iter()
            .all(|occurrence| {
                occurrence.admitted_provider
                    == function.x86_scalar_fma_occurrences[0].admitted_provider
            })
    );
    let occurrence = function.x86_scalar_fma_occurrences[0];
    let mut parts = native.into_parts();
    let selected = parts
        .selected_provider_plans
        .iter_mut()
        .find(|plan| plan.report_identity() == occurrence.provider_plan_report_identity)
        .expect("FMA occurrence rejoins one selected native plan");
    let mut substituted_digest = *selected.plan_digest().as_bytes();
    substituted_digest[0] ^= 1;
    *selected = omega_terminal_psi_to_native_artifact::NativeSelectedProviderPlan::new(
        selected.report_identity(),
        omega_terminal_psi_to_native_artifact::NativeSelectedProviderPlanDigest::from_digest(
            substituted_digest,
        ),
        selected.requirement_identities().to_vec(),
    );
    let error = omega_compilation_report::RetainedNativeArtifact::from_replayed_parts(parts)
        .expect_err("a substituted exact selected plan must reject native FMA replay");
    assert_eq!(
        error,
        "native artifact nearest-FMA does not rejoin one exact selected provider plan"
    );
}

#[test]
fn source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;

data Main { }

machine Main::after_fma(&mut self) { }

machine Main::main(&mut self) {
    let fused: f32 = F32::fused_multiply_add(
        1.00000011920928955078125f32,
        0.99999988079071044921875f32,
        -1.0f32,
    );
    self.after_fma();
}
"#,
        &exact_profile_build(
            "linux_x86_64",
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;"#,
        ),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .unwrap_or_else(|diagnostics| {
        panic!("FMA followed by an internal Unit call should lower: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains native proposal");
    assert_eq!(
        retained
            .native_realization_proposal()
            .unwrap()
            .ieee_float_fma_occurrences()
            .len(),
        1
    );
    let native = realize_retained_terminal_artifact_with_source_evaluated_imports(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        &[],
    )
    .unwrap_or_else(|diagnostics| {
        panic!("FMA plus internal Unit call should realize natively: {diagnostics:#?}")
    });
    native.validate().expect("native artifact replays");
    let function = native
        .object()
        .functions()
        .iter()
        .find(|function| !function.x86_scalar_fma_occurrences.is_empty())
        .expect("source FMA function survives object construction");
    let control = function
        .x86_floating_control
        .expect("FMA function has one canonical MXCSR envelope");
    let [call] = function.internal_unit_calls.as_slice() else {
        panic!("source function retains one internal Unit call")
    };
    assert!(control.install_offset + control.install_byte_count <= call.code_offset);
    assert!(call.code_offset + call.byte_count <= control.restore_offset);
}

#[test]
fn aarch64_fma_demand_is_not_an_x86_feature_association() {
    let main = pass_canary_main("float/named_provider_fused_multiply_add_exit");
    let checked = compile_to_checked(&main, Some("linux_arm64"))
        .expect("AArch64 FMA remains admitted by its own target realization");
    assert_eq!(checked.x86_scalar_fma_provider(), None);
    assert!(checked.x86_scalar_fma_plan_associations().is_empty());
}

#[test]
fn x86_fma_build_admission_binds_the_exact_selected_profile() {
    let project = TempProject::new(
        r#"target linux_x86_64 { }
target windows_x86_64 { }
machine build(builder: &mut Build) {
    builder.application("profile-bound-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let linux = compile_to_checked(&project.main(), Some("linux_x86_64"))
        .expect("Linux x86 deployment selection");
    let windows = compile_to_checked(&project.main(), Some("windows_x86_64"))
        .expect("Windows x86 deployment selection");
    let linux = linux.x86_scalar_fma_provider().expect("Linux admission");
    let windows = windows
        .x86_scalar_fma_provider()
        .expect("Windows admission");

    assert_eq!(linux.profile(), omega_target::TargetProfile::LinuxX64);
    assert_eq!(windows.profile(), omega_target::TargetProfile::WindowsX64);
    assert_ne!(linux.identity(), windows.identity());
}

#[test]
fn non_x86_profile_rejects_x86_deployment_feature_selection() {
    let project = TempProject::new(
        r#"target linux_arm64 { }
machine build(builder: &mut Build) {
    builder.application("invalid-arm-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(&project.main(), Some("linux_arm64"))
        .expect_err("an AArch64 profile cannot admit x86 deployment features")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        diagnostics.contains(
            "Build.x86_deployment_features cannot admit AVX+FMA3 for exact profile `linux_arm64`"
        ),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn targetless_build_cannot_mint_x86_deployment_feature_admission() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("targetless-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("targetless checking has no deployment feature field")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        diagnostics.contains("x86_deployment_features"),
        "unexpected diagnostics: {diagnostics}"
    );
}
