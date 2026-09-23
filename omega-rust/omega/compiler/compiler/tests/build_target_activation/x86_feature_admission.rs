use super::{
    TempProject, application_build, diagnostic_text, exact_target_build, fixture_package_identity,
    package_inputs_with_standard_library, pass_canary_main,
};
use crate::{bundled_standard_library_root, console_acceptance, fixtures, linux_entry_acceptance};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeRealizationRequest, compile, compile_to_checked,
    realize_retained_native_artifact,
};

/// Package inputs for a std-linked fixture that selects the Linux x86-64
/// program entry. A package-sourced `targets/linux_x86_64/entry.omg` cannot
/// take the bundled-source path, so the checked schema needs the same
/// explicit accepted binding the macOS entry requires.
fn package_inputs_with_linux_entry(
    main: &std::path::Path,
) -> package_compilation::PackageCompilationInputs {
    let entry_binding = linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
        &bundled_standard_library_root(),
        fixture_package_identity(2),
    )
    .expect("the fixture explicitly accepts the checked Linux entry schema");
    package_inputs_with_standard_library(main)
        .with_accepted_semantic_bindings(vec![entry_binding])
        .expect("Linux entry acceptance binds to the std package")
}

/// Package inputs for a std-linked fixture that selects the Linux ARM64
/// program entry; its package-sourced contract needs the same explicit
/// accepted binding.
fn package_inputs_with_linux_arm64_entry(
    main: &std::path::Path,
) -> package_compilation::PackageCompilationInputs {
    let entry_binding = linux_entry_acceptance::candidate_linux_arm64_entry_binding(
        &bundled_standard_library_root(),
        fixture_package_identity(2),
    )
    .expect("the fixture explicitly accepts the checked Linux ARM64 entry schema");
    package_inputs_with_standard_library(main)
        .with_accepted_semantic_bindings(vec![entry_binding])
        .expect("Linux ARM64 entry acceptance binds to the std package")
}

/// Select the package inputs for one target leg: the Linux entries carry the
/// accepted contract binding, every other target keeps the plain std-linked
/// inputs.
fn package_inputs_for_target(
    main: &std::path::Path,
    target: &str,
) -> package_compilation::PackageCompilationInputs {
    if target == "linux_x86_64" {
        package_inputs_with_linux_entry(main)
    } else if target == "linux_arm64" {
        package_inputs_with_linux_arm64_entry(main)
    } else {
        package_inputs_with_standard_library(main)
    }
}

#[test]
fn root_build_aliases_cannot_mutate_target_or_replace_the_activation() {
    for (operation, expected) in [
        (
            "alias.target = TargetProfile::MacosArm64;",
            "Build.target is compiler-owned and cannot be assigned",
        ),
        (
            "let target: &mut TargetProfile = &mut alias.target;",
            "Build.target is compiler-owned and cannot enter a mutable or write-only borrow",
        ),
        (
            "alias = alias;",
            "Build activation cannot be replaced as a whole value",
        ),
    ] {
        let project = TempProject::new(&exact_target_build(&format!(
            "let alias: &mut Build = &mut builder; {operation}"
        )));
        let diagnostics = diagnostic_text(&project);
        assert!(diagnostics.contains(expected), "{operation}: {diagnostics}");
    }
}

#[test]
fn authored_legacy_build_is_rejected_instead_of_receiving_a_hidden_target() {
    let project = TempProject::new(
        r#"data Build {
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
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &baseline.main(),
        Some("windows_x86_64"),
    ))
    .expect("generic x86 baseline must remain available");
    assert_eq!(checked.x86_scalar_fma_provider(), None);

    let opted_in = TempProject::new(&exact_target_build(
        "    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;",
    ));
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &opted_in.main(),
        Some("windows_x86_64"),
    ))
    .expect("exact x86 build may select the canonical AVX+FMA3 deployment pair");
    let provider = checked
        .x86_scalar_fma_provider()
        .expect("explicit feature selection must retain one admitted provider");
    assert_eq!(provider.profile(), target::TargetProfile::WindowsX64);
    assert!(provider.has_canonical_identity());
    assert_eq!(
        provider.deployment().features(),
        &target::X86_SCALAR_FMA_REQUIRED_FEATURES
    );
    assert!(
        checked.x86_scalar_fma_plan_associations().is_empty(),
        "feature admission without source demand must not fabricate an association"
    );
}

#[test]
fn exact_x86_fma_demand_fails_closed_without_feature_admission() {
    let main = pass_canary_main(fixtures::NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT);
    for target in ["linux_x86_64", "windows_x86_64"] {
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs_for_target(&main, target)),
            ..CheckedCompileRequest::new(&main, Some(target))
        })
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
            target::TargetProfile::LinuxX64,
            target::TargetProfile::WindowsX64,
        ),
        (
            "windows_x86_64",
            target::TargetProfile::WindowsX64,
            target::TargetProfile::LinuxX64,
        ),
    ] {
        let main = pass_canary_main(fixtures::X86_FMA_PLAN_ASSOCIATION);
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs_for_target(&main, target)),
            ..CheckedCompileRequest::new(&main, Some(target))
        })
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
                target::X86ScalarFmaSlot::Binary32,
                target::X86ScalarFmaSlot::Binary64,
            ]
        );
        assert_eq!(
            associations
                .iter()
                .map(|association| association.selected_builtin())
                .collect::<Vec<_>>(),
            vec![
                symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
                symbols::BuiltinFunction::FloatFusedMultiplyAddF64,
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

        let wrong_profile_provider = target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
            other_profile,
            &target::X86_SCALAR_FMA_REQUIRED_FEATURES,
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
            effects::SelectedProviderPlanFacts::from_selected_plans(substituted_plans)
                .expect("structurally valid substituted closure");
        assert!(
            !associations[0].matches_checked_inputs(&substituted, provider),
            "compact coordinates cannot authorize an exact-plan substitution"
        );
        assert_eq!(provider.profile(), profile);
    }
}

#[test]
fn boundary_operator_and_float_adapters_retain_terminal_execution() {
    let project = TempProject::with_main(
        r#"use omega::language::core::float_operations;
use omega_language_std::console;
use omega::language::core::service;

data Arithmetic {}
boundary operator Arithmetic::identity(value: i32) -> i32;
data ArithmeticProvider {}
machine ArithmeticProvider::identity(value: i32) -> i32
    satisfies Arithmetic::identity { value }

data Main { console: Binding<Console>; }
machine Main::main(&mut self) reaches Console {
    let fused32: f32 = F32::fused_multiply_add(2.0f32, 3.0f32, 4.0f32);
    let fused64: f64 = F64::fused_multiply_add(2.0f64, 3.0f64, 4.0f64);
    let value: i32 = Arithmetic::identity(7);
    self.console.exit_process(value);
}
"#,
        &application_build(
            r#"    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
    builder.select_provider<Arithmetic::identity, ArithmeticProvider>();"#,
        ),
    );
    let package_inputs = package_inputs_with_standard_library(&project.main());
    let entry_binding = linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
        &bundled_standard_library_root(),
        fixture_package_identity(2),
    )
    .expect("the fixture explicitly accepts the checked Linux entry schema");
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![entry_binding.clone()])
        .unwrap();
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some("linux_x86_64"))
    })
    .expect("derive the exact mixed fixture provider plans");
    let console_binding = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        fixture_package_identity(2),
        true,
        false,
    )
    .expect("the fixture explicitly accepts Console output and exit");
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![entry_binding, console_binding])
        .unwrap();
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_package_inputs(package_inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| panic!("mixed selected execution failed: {diagnostics:#?}"));
    let retained = report.into_retained_terminal_artifact().unwrap();
    retained
        .validate()
        .expect("mixed Terminal artifact verifies");
    let proposal = retained.native_realization_proposal().unwrap();
    assert_eq!(
        proposal
            .ieee_float_fma_occurrences()
            .iter()
            .map(|occurrence| occurrence.format())
            .collect::<Vec<_>>(),
        [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64
        ],
        "boundary settlement preserves both ordered FMA applications"
    );
    assert_eq!(
        proposal
            .checked_boundary_operator_scope()
            .occurrences()
            .len(),
        3,
        "both float applications and the checked operator retain occurrence custody"
    );
    assert_eq!(proposal.boundary_application_realizations().rows().iter().filter(|realization| {
        realization.role() == boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody
    }).count(), 1, "boundary settlement preserves the checked operator realization");
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
        &application_build(
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
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
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
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
            == boundary_applications::BoundaryApplicationRealizationRole::ExactCompilerIntrinsic
            && realization.selected_plan_digest() != &[0; 32]
            && terminal_operations.contains(&realization.terminal_operation())
    }));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("canonical Terminal semantics decode");

    for occurrence in occurrences {
        let plan = &proposal.selected_provider_plans().plans()[occurrence.provider_plan_index()];
        assert_eq!(plan.target, "linux_x86_64");
        let admission = occurrence
            .x86_admission()
            .expect("x86 occurrence retains admitted deployment provider");
        assert_eq!(
            admission.provider().profile(),
            target::TargetProfile::LinuxX64
        );
        assert_eq!(
            admission.slot(),
            match occurrence.format() {
                semantic_vocabulary::IeeeFloatFormat::Binary32 =>
                    target::X86ScalarFmaSlot::Binary32,
                semantic_vocabulary::IeeeFloatFormat::Binary64 =>
                    target::X86ScalarFmaSlot::Binary64,
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
            terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        ));
    }
    retained
        .validate()
        .expect("FMA occurrence proposal replays against canonical artifact");
    let native = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(
                    native_realization::current_terminal_authority_permission_policy(),
                ),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
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
            native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
        ) && matches!(
            child.occurrence(),
            native_realization::NativePhysicalOccurrence::Operator(_)
        ) && child.relocation()
            == native_realization::PhysicalRelocationDisposition::DirectInstructionBytes
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
            target::X86ScalarFmaSlot::Binary32,
            target::X86ScalarFmaSlot::Binary64,
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
    *selected = native_realization::NativeSelectedProviderPlan::new(
        selected.report_identity(),
        native_realization::NativeSelectedProviderPlanDigest::from_digest(substituted_digest),
        selected.requirement_identities().to_vec(),
    );
    let error = compilation_report::RetainedNativeArtifact::from_replayed_parts(parts)
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
        &application_build(
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
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
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
    let native = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: native_realization::current_terminal_authority_policy(),
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(
                    native_realization::current_terminal_authority_permission_policy(),
                ),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
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
    let main = pass_canary_main(fixtures::NAMED_PROVIDER_FUSED_MULTIPLY_ADD_EXIT);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs_for_target(&main, "linux_arm64")),
        ..CheckedCompileRequest::new(&main, Some("linux_arm64"))
    })
    .expect("AArch64 FMA remains admitted by its own target realization");
    assert_eq!(checked.x86_scalar_fma_provider(), None);
    assert!(checked.x86_scalar_fma_plan_associations().is_empty());
}

#[test]
fn x86_fma_build_admission_binds_the_exact_selected_profile() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("profile-bound-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let linux = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("linux_x86_64"),
    ))
    .expect("Linux x86 deployment selection");
    let windows = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("Windows x86 deployment selection");
    let linux = linux.x86_scalar_fma_provider().expect("Linux admission");
    let windows = windows
        .x86_scalar_fma_provider()
        .expect("Windows admission");

    assert_eq!(linux.profile(), target::TargetProfile::LinuxX64);
    assert_eq!(windows.profile(), target::TargetProfile::WindowsX64);
    assert_ne!(linux.identity(), windows.identity());
}

#[test]
fn non_x86_profile_rejects_x86_deployment_feature_selection() {
    let project = TempProject::new(
        r#"machine build(builder: &mut Build) {
    builder.application("invalid-arm-fma");
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("linux_arm64"),
    ))
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
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
