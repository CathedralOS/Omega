use super::fixture_roster;
use super::{
    optional_intrinsic_diagnostic_label, retained_float_differential_result_identity,
    selected_intrinsic_diagnostic_label,
};
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, CompileRequest, CompilerOptions,
    RequestedCompileProduct, compile, compile_canary_without_output,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    compile_rooted_canary_for_target, executable_name, fail_canary, fs,
    hosted_main_program_entry_build_for, interpret, pass_canary,
    reviewed_repository_fixture_package_inputs,
};
use checked_interpreter::InterpretOptions;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use compiler::CheckedCompileRequest;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn domain_operator_selection_records_signature_domain_meaning_as_evidence() {
    let canary = pass_canary(fixture_roster::DOMAINS_DOMAIN_OPERATOR_PROVEN_FACT_SELECTS_MEANING);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("signature-selected domain canary should compile to checked trees");

    let selected_domain_uses = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.spelling == language_core::OperatorSpelling::Add)
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter(|candidate| candidate.is_domain_owned())
        .count();
    assert!(
        selected_domain_uses > 0,
        "expected the signature's `Quantity::Additive` selection to choose the domain-owned `+` meaning \
         and record it in the operator evidence"
    );
}

#[test]
fn float_operator_spellings_record_named_core_identities() {
    let canary = pass_canary(fixture_roster::OPERATORS_FLOAT_OPERATOR_IDENTITIES);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("core float operation identities should compile");

    let selected_names: Vec<String> = checked
        .facts
        .operators
        .resolved_uses()
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter_map(|candidate| {
            checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == candidate.operator_symbol)
        })
        .map(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect();

    for required in [
        "Float::add",
        "Float::divide",
        "Float::greater",
        "Float::equal",
    ] {
        assert!(
            selected_names.iter().any(|name| name == required),
            "expected primitive float spelling to select `{required}`, got {selected_names:?}"
        );
    }

    let operator_name = |operator: &typed_trees::operator::OperatorDefinition| {
        checked
            .typed
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::")
    };
    for required in [
        "F64::fused_multiply_add",
        "F32::classify",
        "F64::add_toward_positive",
    ] {
        let operator = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator_name(operator) == required)
            .unwrap_or_else(|| panic!("missing source-visible float requirement `{required}`"));
        assert!(
            operator.is_boundary,
            "`{required}` is a primitive carrier boundary requirement"
        );
        assert!(
            !checked.typed.operator_contracts(operator).is_empty(),
            "`{required}` must publish equality against FloatSemantics"
        );
    }
    let semantic_fma = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator_name(operator) == "FloatSemantics::fused_multiply_add")
        .expect("source-visible executable FMA semantic identity");
    assert!(
        !semantic_fma.is_boundary,
        "FloatSemantics is pure core computation, not a target boundary"
    );
}

#[test]
fn float_match_checked_interpreter_executes_selected_arm_comparisons() {
    let canary = pass_canary("expressions/match_float_patterns");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("float match selects its ordinary core provider");
    let outcome =
        checked_interpreter::interpret_entry(&checked, "launch", &[], InterpretOptions::default());
    assert_eq!(outcome.error, None, "selected Match equality must execute");
    assert_eq!(outcome.exit_code, 0);
}

#[test]
fn float_match_arms_retain_distinct_applications_of_the_selected_provider() {
    let canary = pass_canary("expressions/match_float_patterns");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("float patterns select their exact core equality provider");
    let uses = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| operator_use)
        .filter(|operator_use| {
            matches!(
                operator_use.occurrence,
                checked_trees::CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 2);
    assert_ne!(uses[0].application_site(), uses[1].application_site());
    for operator_use in uses {
        assert!(!operator_use.provider_plan_commitment.is_empty());
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("each arm retains an actually selected plan");
        assert_eq!(
            operator_use.provider_plan_commitment.as_bytes(),
            plan.identity_digest().as_bytes()
        );
        let operator = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            .unwrap();
        assert_eq!(
            plan.schema.trait_name,
            typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator)
        );
        assert_eq!(
            checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .filter(
                    |application| application.site == operator_use.application_site()
                        && application.requirement_symbol == operator_use.selected_operator_symbol
                )
                .count(),
            1
        );
        assert!(
            matches!(
                checked
                    .typed
                    .expression_table
                    .expression(operator_use.expression),
                typed_trees::expression::ExpressionNode::Match(_)
            ),
            "selection must not replace Match with equality"
        );
    }
}

#[test]
fn float_match_executes_selected_arms_through_verified_terminal() {
    let canary = pass_canary("expressions/match_float_patterns");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("macos_arm64"),
    ))
    .unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .expect("the unchanged call-bearing Match customer must reach Terminal");
    compiler::validate_lowered_ieee_float_comparison_custody(&checked, &lowered)
        .expect("each comparison independently rejoins its selected provider");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 2);
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("selected Match application scope survives canonical publication");
    assert_eq!(produced.boundary_operator_scope().occurrences().len(), 2);
    for change_relation in [false, true] {
        let mut changed = lowered.clone();
        // Omit replaceable debug evidence so this tests source custody rather
        // than rejection of an old debug map's semantic digest.
        changed.debug_map = None;
        let occurrence = &mut changed.selected_ieee_float_comparison_occurrences[0];
        if change_relation {
            occurrence.comparison = semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual;
            let operation_id = occurrence.terminal_operation;
            for machine in &mut changed.semantic_module.machines {
                for block in &mut machine.blocks {
                    for operation in &mut block.operations {
                        if operation.id == operation_id {
                            let terminal_psi::OperationKind::IeeeFloatCompare {
                                comparison, ..
                            } = &mut operation.kind
                            else {
                                panic!("comparison occurrence")
                            };
                            *comparison =
                                semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual;
                        }
                    }
                }
            }
        } else {
            occurrence.format = semantic_vocabulary::IeeeFloatFormat::Binary64;
        }
        let optimized =
            lowered_psi_to_lowered_psi::run_psi_optimization(changed, Default::default())
                .expect("source custody corruption can remain valid portable semantics");
        let changed_artifact =
            lowered_psi_to_terminal_psi::finalize_terminal_artifact(&optimized).unwrap();
        assert!(
            lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
                &checked,
                &changed_artifact,
                optimized.lowered(),
            )
            .is_err(),
            "published source scope must independently rederive relation and format"
        );
    }
    let artifact = produced.artifact();
    for (value, first, second, expected) in [
        (1.0_f32.to_bits(), 1.0_f32.to_bits(), 2.0_f32.to_bits(), 7),
        (2.0_f32.to_bits(), 1.0_f32.to_bits(), 2.0_f32.to_bits(), 9),
        (3.0_f32.to_bits(), 1.0_f32.to_bits(), 2.0_f32.to_bits(), 11),
        (1.0_f32.to_bits(), 1.0_f32.to_bits(), 1.0_f32.to_bits(), 7),
        (0x8000_0000, 0, 0x8000_0000, 7),
        (0x7fc0_0042, 0x7fc0_0042, 0x7fc0_0042, 11),
        (2.0_f32.to_bits(), 0x7fc0_0042, 2.0_f32.to_bits(), 9),
    ] {
        let arguments = [value, first, second].map(|bits| {
            terminal_interpreter::TerminalScalarValue::IeeeFloat(
                semantic_vocabulary::IeeeFloatValue::Binary32(bits),
            )
        });
        let measured = terminal_interpreter::interpret_terminal_artifact_measured(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("decoded and independently verified Match executes");
        assert_eq!(
            measured.value(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(expected),
                },
            )
        );
    }
}

#[test]
fn float_match_native_publication_retains_both_selected_physical_children() {
    let main_path = pass_canary("expressions/match_float_patterns").join("main.omg");
    for target_name in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-native-float-match-{target_name}-{}",
            std::process::id()
        ));
        let mut request = CompileRequest::new(CompilerOptions {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target_name.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
        if let Some(packages) =
            reviewed_repository_fixture_package_inputs(&main_path, Some(target_name))
                .expect("review the authored match application")
        {
            request = request.with_package_inputs(packages);
        }
        let terminal_request = request
            .clone()
            .with_requested_product(RequestedCompileProduct::TerminalArtifact);
        let report = compiler::compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| {
                panic!("publish unchanged float Match for {target_name}: {diagnostics:#?}")
            });
        let artifact = report
            .retained_native_artifact()
            .expect("retained native product");
        artifact
            .validate()
            .expect("independently replay complete native artifact");
        let physical = artifact
            .physical_evidence()
            .expect("complete physical coverage");
        assert_eq!(
            physical.children().len(),
            2,
            "both selected equality arms survive as distinct physical children"
        );
        let retained = compiler::compile(terminal_request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .expect("retain exact source and provider proposal")
            .into_retained_terminal_artifact()
            .unwrap();
        let selections = retained
            .native_realization_proposal()
            .unwrap()
            .post_terminal_optimizations()
            .selections()
            .clone();
        let replayed = {
            let image_request = native_realization::ExecutableImageEmissionRequest::direct(
                retained
                    .native_realization_proposal()
                    .expect("native proposal")
                    .subsystem(),
            );
            compiler::realize_retained_native_artifact(
                retained,
                compiler::RetainedNativeRealizationRequest {
                    profile: &proof_admission::AdmissionProfile::default(),
                    optimization_selections: &selections,
                    terminal_authority_policy:
                        native_realization::current_terminal_authority_policy(),
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
        .expect("source-free re-entry consumes the same selected comparison custody");
        replayed
            .validate()
            .expect("independently replay retained native re-entry");
        assert_eq!(replayed.physical_evidence().unwrap().children().len(), 2);
        let published = report
            .publish_retained_native_artifact(&build_dir)
            .expect("publish validated image");
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) && target_name == "macos_arm64" {
            let output = Command::new(published.checked_native_executable_path().unwrap())
                .output()
                .expect("execute the actual rooted match application");
            assert_eq!(output.status.code(), Some(0));
        }
        fs::remove_dir_all(&build_dir).expect("remove this test's generated image");
    }
}

#[test]
fn float_provider_plan_identities_ignore_arena_and_display_perturbations() {
    fn float_plan_snapshot(checked: &compiler::CheckedCompilation) -> Vec<(String, u64)> {
        checked
            .selected_provider_plans()
            .plans()
            .iter()
            .filter(|plan| plan.name.starts_with("FloatNativeProvider::satisfies::"))
            .map(|plan| (plan.name.clone(), plan.report_fingerprint()))
            .collect()
    }

    let canary = pass_canary(fixture_roster::OPERATORS_FLOAT_OPERATOR_IDENTITIES);
    let baseline = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("baseline float provider plans should check");
    let baseline_snapshot = float_plan_snapshot(&baseline);
    assert!(
        !baseline_snapshot.is_empty(),
        "float identity invariant requires selected native provider plans"
    );
    for plan in baseline
        .selected_provider_plans()
        .plans()
        .iter()
        .filter(|plan| plan.name.starts_with("FloatNativeProvider::satisfies::"))
    {
        let mut renamed_display = plan.clone();
        renamed_display.origin_package = "non-semantic display perturbation".to_owned();
        assert_eq!(
            renamed_display.report_fingerprint(),
            plan.report_fingerprint(),
            "readable origin labels must not enter exact float plan identity"
        );
    }

    let scratch = std::env::temp_dir().join(format!(
        "omega-float-plan-identity-perturbation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("float plan identity scratch directory");
    let source = fs::read_to_string(canary.join("main.omg")).expect("read float plan canary");
    let source = source.replacen(
        "use omega::language::core::float_operations;",
        "use omega::language::core::option;\nuse omega::language::core::float_operations;",
        1,
    );
    let source = source.replacen(
        "data Main {",
        "data ArenaPadding {\n    value: i32;\n}\n\ndata Main {",
        1,
    );
    fs::write(scratch.join("main.omg"), source).expect("write perturbed float plan source");
    fs::copy(canary.join("build.omg"), scratch.join("build.omg"))
        .expect("copy float plan build configuration");

    let perturbed = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &scratch.join("main.omg"),
        None,
    ))
    .expect("arena-perturbed float provider plans should check");
    assert_eq!(
        float_plan_snapshot(&perturbed),
        baseline_snapshot,
        "unrelated imported and local declarations must not leak arena coordinates into float plan identity"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn migrated_float_provider_plans_are_selected_for_every_native_target() {
    const MIGRATED_REQUIREMENTS: &[&str] = &[
        "Float::add",
        "Float::subtract",
        "Float::multiply",
        "Float::divide",
        "Float::equal",
        "Float::not_equal",
        "Float::less",
        "Float::less_or_equal",
        "Float::greater",
        "Float::greater_or_equal",
        "F32::minimum",
        "F64::minimum",
        "F32::maximum",
        "F64::maximum",
        "F32::negate",
        "F64::negate",
        "F32::square_root",
        "F64::square_root",
        "F32::is_nan",
        "F64::is_nan",
        "F32::is_finite",
        "F64::is_finite",
        "F32::is_infinite",
        "F64::is_infinite",
        "F32::is_normal",
        "F64::is_normal",
        "F32::is_subnormal",
        "F64::is_subnormal",
        "F32::classify",
        "F64::classify",
        "F32::multiply_then_add",
        "F64::multiply_then_add",
        "F32::fused_multiply_add",
        "F64::fused_multiply_add",
        "F32::fused_multiply_add_toward_zero",
        "F64::fused_multiply_add_toward_zero",
        "F32::fused_multiply_add_toward_positive",
        "F64::fused_multiply_add_toward_positive",
        "F32::fused_multiply_add_toward_negative",
        "F64::fused_multiply_add_toward_negative",
        "F32::add_toward_zero",
        "F64::add_toward_zero",
        "F32::add_toward_positive",
        "F64::add_toward_positive",
        "F32::add_toward_negative",
        "F64::add_toward_negative",
        "F32::subtract_toward_zero",
        "F64::subtract_toward_zero",
        "F32::subtract_toward_positive",
        "F64::subtract_toward_positive",
        "F32::subtract_toward_negative",
        "F64::subtract_toward_negative",
        "F32::multiply_toward_zero",
        "F64::multiply_toward_zero",
        "F32::multiply_toward_positive",
        "F64::multiply_toward_positive",
        "F32::multiply_toward_negative",
        "F64::multiply_toward_negative",
        "F32::divide_toward_zero",
        "F64::divide_toward_zero",
        "F32::divide_toward_positive",
        "F64::divide_toward_positive",
        "F32::divide_toward_negative",
        "F64::divide_toward_negative",
        "F32::square_root_toward_zero",
        "F64::square_root_toward_zero",
        "F32::square_root_toward_positive",
        "F64::square_root_toward_positive",
        "F32::square_root_toward_negative",
        "F64::square_root_toward_negative",
        "F32::from_f64",
        "F64::from_f32",
        "F32::from_i8",
        "F32::from_i16",
        "F32::from_i32",
        "F32::from_i64",
        "F32::from_u8",
        "F32::from_u16",
        "F32::from_u32",
        "F32::from_u64",
        "F64::from_i8",
        "F64::from_i16",
        "F64::from_i32",
        "F64::from_i64",
        "F64::from_u8",
        "F64::from_u16",
        "F64::from_u32",
        "F64::from_u64",
        "I8::from_f32",
        "I8::from_f64",
        "I16::from_f32",
        "I16::from_f64",
        "I32::from_f32",
        "I32::from_f64",
        "I64::from_f32",
        "I64::from_f64",
        "U8::from_f32",
        "U8::from_f64",
        "U16::from_f32",
        "U16::from_f64",
        "U32::from_f32",
        "U32::from_f64",
        "U64::from_f32",
        "U64::from_f64",
    ];
    let canary = pass_canary(fixture_roster::OPERATORS_FLOAT_OPERATOR_IDENTITIES);
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .unwrap_or_else(|diagnostics| {
            panic!("core float provider plans should check for {target}: {diagnostics:#?}")
        });
        let operator_path = |operator: &typed_trees::operator::OperatorDefinition| {
            checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        };
        let expected_intrinsic = |operator: &typed_trees::operator::OperatorDefinition| {
            let path = operator_path(operator);
            if !MIGRATED_REQUIREMENTS.contains(&path.as_str()) {
                return None;
            }
            if path.contains("::fused_multiply_add")
                && !matches!(target, "linux_arm64" | "macos_arm64")
            {
                // Generic x86-64 remains SSE2-baseline. Its FMA requirement
                // must wait for a feature-qualified or checked software plan.
                return None;
            }
            provider_planning::compiler_intrinsic_diagnostic_label(&checked.typed, operator)
        };
        let mut used_intrinsics = std::collections::BTreeSet::new();

        for operator_use in checked.facts.operators.resolved_uses() {
            let Some(candidate) = checked.facts.operators.selected_candidate(operator_use) else {
                continue;
            };
            let Some(operator) = checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == candidate.operator_symbol)
            else {
                continue;
            };
            let Some(expected_intrinsic) = expected_intrinsic(operator) else {
                continue;
            };
            assert_ne!(
                operator_use.provider_plan_report_fingerprint, 0,
                "{target} {expected_intrinsic} use must retain the selected ProviderPlan identity"
            );
            assert!(
                !operator_use.provider_plan_commitment.is_empty(),
                "{target} {expected_intrinsic} use must retain the exact ProviderPlan commitment"
            );
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
                .expect("operator evidence must resolve to one retained selected plan");
            assert_eq!(plan.target, target);
            assert_eq!(
                plan.schema.trait_name,
                typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                )
            );
            let [row] = plan.rows.as_slice() else {
                panic!("exact operator plan must retain one realization row");
            };
            assert_eq!(row.method, "realize");
            assert!(
                matches!(
                    &row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                ),
                "{target} selected the wrong {expected_intrinsic} realization: {row:?}"
            );
            used_intrinsics.insert(expected_intrinsic);
        }

        assert_eq!(
            used_intrinsics,
            [
                "Float::add.f32".to_owned(),
                "Float::add.f64".to_owned(),
                "Float::divide.f32".to_owned(),
                "Float::divide.f64".to_owned(),
                "Float::equal.f32".to_owned(),
                "Float::equal.f64".to_owned(),
                "Float::greater.f32".to_owned(),
                "Float::greater.f64".to_owned(),
                "Float::greater_or_equal.f32".to_owned(),
                "Float::greater_or_equal.f64".to_owned(),
                "Float::less.f32".to_owned(),
                "Float::less.f64".to_owned(),
                "Float::less_or_equal.f32".to_owned(),
                "Float::less_or_equal.f64".to_owned(),
                "Float::multiply.f32".to_owned(),
                "Float::multiply.f64".to_owned(),
                "Float::not_equal.f32".to_owned(),
                "Float::not_equal.f64".to_owned(),
                "Float::subtract.f32".to_owned(),
                "Float::subtract.f64".to_owned(),
            ]
            .into_iter()
            .collect(),
            "every primitive float operation used by the canary must consume its target plan"
        );

        let mut selected_count = 0usize;
        for operator in checked.typed.operators() {
            let Some(expected_intrinsic) = expected_intrinsic(operator) else {
                continue;
            };
            selected_count += 1;
            let slot = typed_trees::operator::boundary_operator_requirement_identity(
                &checked.typed,
                operator,
            );
            let plan = checked
                .selected_provider_plans()
                .plans()
                .iter()
                .find(|plan| plan.target == target && plan.schema.trait_name == slot)
                .unwrap_or_else(|| {
                    panic!("{target} must select exact plan slot {slot} for {expected_intrinsic}")
                });
            assert!(
                matches!(
                    plan.rows.as_slice(),
                    [row]
                        if row.method == "realize"
                            && matches!(
                                &row.binding,
                                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            )
                ),
                "{target} selected the wrong realization for {expected_intrinsic}: {plan:?}"
            );
        }
        assert_eq!(
            selected_count,
            if matches!(target, "linux_arm64" | "macos_arm64") {
                146
            } else {
                138
            },
            "all migrated primitive and target-valid named-operation slots must select"
        );
        let selected_fma_plans = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .filter(|plan| {
                plan.target == target
                    && plan.rows.iter().any(|row| {
                        matches!(
                            &row.binding,
                            effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                        ) && optional_intrinsic_diagnostic_label(&checked, plan)
                            .is_some_and(|label| label.contains("::fused_multiply_add"))
                    })
            })
            .count();
        assert_eq!(
            selected_fma_plans,
            if matches!(target, "linux_arm64" | "macos_arm64") {
                8
            } else {
                0
            },
            "only baseline-FMADD AArch64 targets may select nearest or directed FMA slots"
        );
    }
}

#[test]
fn primitive_float_arithmetic_and_comparisons_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.primitive-arithmetic-comparison.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 finite add/subtract/multiply/divide",
        "binary64 finite add/subtract/multiply/divide",
        "binary32 equality and ordered comparisons",
        "binary64 equality and ordered comparisons",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x42ad_e03a_f099_ff9f;
    const PRIMITIVE_REQUIREMENTS: &[&str] = &[
        "Float::add",
        "Float::subtract",
        "Float::multiply",
        "Float::divide",
        "Float::equal",
        "Float::not_equal",
        "Float::less",
        "Float::less_or_equal",
        "Float::greater",
        "Float::greater_or_equal",
    ];

    let canary = pass_canary(fixture_roster::OPERATORS_FLOAT_OPERATOR_IDENTITIES);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("primitive float arithmetic and comparisons should compile");
    let operator_path = |operator: &typed_trees::operator::OperatorDefinition| {
        checked
            .typed
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::")
    };
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.resolved_uses() {
        let Some(candidate) = checked.facts.operators.selected_candidate(operator_use) else {
            continue;
        };
        let Some(operator) = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == candidate.operator_symbol)
        else {
            continue;
        };
        if !PRIMITIVE_REQUIREMENTS.contains(&operator_path(operator).as_str()) {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
            .expect("primitive float evidence must retain its selected plan");
        let [row] = plan.rows.as_slice() else {
            panic!("primitive float plan must retain one exact realization row");
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding else {
            panic!("primitive float plan must select a compiler intrinsic");
        };
        selected_intrinsics.insert(selected_intrinsic_diagnostic_label(&checked, plan));
        selected_plan_identities.push(plan.report_fingerprint());
    }
    let expected_intrinsics = [
        "Float::add.f32",
        "Float::add.f64",
        "Float::divide.f32",
        "Float::divide.f64",
        "Float::equal.f32",
        "Float::equal.f64",
        "Float::greater.f32",
        "Float::greater.f64",
        "Float::greater_or_equal.f32",
        "Float::greater_or_equal.f64",
        "Float::less.f32",
        "Float::less.f64",
        "Float::less_or_equal.f32",
        "Float::less_or_equal.f64",
        "Float::multiply.f32",
        "Float::multiply.f64",
        "Float::not_equal.f32",
        "Float::not_equal.f64",
        "Float::subtract.f32",
        "Float::subtract.f64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        20,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation and format"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete primitive float matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-primitive-float-arithmetic-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("primitive float arithmetic and comparisons should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("primitive float arithmetic and comparison canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-primitive-float-arithmetic-comparison-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("primitive float cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy primitive float canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write primitive float build source");
        compile(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("primitive float operations should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_format_conversion_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.format-conversion.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary64-to-binary32 halfway tie-to-even",
        "binary64-to-binary32 just-above-halfway",
        "binary32-to-binary64 exact widening",
        "binary64 infinity to binary32 infinity",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xd1dd_0dcd_c054_6c30;

    let canary = pass_canary(fixture_roster::FLOAT_RUNTIME_NAMED_FORMAT_CONVERSION_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("public float-format conversion requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            name.contains("::from_f").then_some((operator_use, name))
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 5, "all five conversion calls retain a plan");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for (operator_use, intrinsic) in selected {
        selected_intrinsics.insert(intrinsic.clone());
        selected_plan_identities.push(
            checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
                .expect("format conversion evidence must retain its selected plan")
                .report_fingerprint(),
        );
        let typed_trees::expression::ExpressionNode::Cast(cast) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("selected conversion `{intrinsic}` must rewrite to one typed cast");
        };
        let expected = if intrinsic == "F32::from_f64.f64" {
            typed_trees::types::PrimitiveType::F32
        } else if intrinsic == "F64::from_f32.f32" {
            typed_trees::types::PrimitiveType::F64
        } else {
            panic!("unexpected conversion intrinsic `{intrinsic}`");
        };
        assert_eq!(
            checked.typed.primitive_type_reference(cast.target_type),
            Some(expected)
        );
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::from_f64.f64".to_owned(),
            "F64::from_f32.f32".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per direction"
    );

    let classification_plan_evidence = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            (name == "F32::is_infinite.f32").then_some((
                operator_use.expression,
                operator_use.provider_plan_report_fingerprint,
            ))
        })
        .collect::<Vec<_>>();
    let classification_plan_evidence =
        classification_plan_evidence
            .into_iter()
            .fold(Vec::new(), |mut unique, evidence| {
                if !unique.contains(&evidence) {
                    unique.push(evidence);
                }
                unique
            });
    assert_eq!(
        classification_plan_evidence.len(),
        2,
        "both repeated classification calls must retain checked plan evidence"
    );
    assert!(
        classification_plan_evidence[0].1 != 0
            && classification_plan_evidence[0].1 == classification_plan_evidence[1].1,
        "repeated classification calls must agree on one exact nonzero plan"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute nearest-even format conversion; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-format-conversion-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("public float-format conversions should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public float-format conversion canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-format-conversion-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("format conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_integer_to_float_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.integer-to-float.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "narrow signed source extension",
        "narrow unsigned source extension",
        "signed binary32 precision-boundary tie",
        "signed binary64 precision-boundary tie",
        "maximum unsigned64 to binary32",
        "maximum unsigned64 to binary64",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x141a_7a9a_5d2a_1ceb;

    let canary = pass_canary(fixture_roster::FLOAT_RUNTIME_NAMED_INTEGER_TO_FLOAT_CONVERSION_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("public integer-to-float requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            name.contains("::from_i")
                .then_some((plan.report_fingerprint(), name.clone()))
                .or_else(|| {
                    name.contains("::from_u")
                        .then_some((plan.report_fingerprint(), name.clone()))
                })
        })
        .collect::<Vec<_>>();
    let selected_intrinsics = selected
        .iter()
        .map(|(_, intrinsic)| intrinsic.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut selected_plan_identities = selected
        .iter()
        .map(|(identity, _)| *identity)
        .collect::<Vec<_>>();
    let expected_intrinsics = [
        "F32::from_i8.i8",
        "F32::from_i16.i16",
        "F32::from_i32.i32",
        "F32::from_i64.i64",
        "F32::from_u8.u8",
        "F32::from_u16.u16",
        "F32::from_u32.u32",
        "F32::from_u64.u64",
        "F64::from_i8.i8",
        "F64::from_i16.i16",
        "F64::from_i32.i32",
        "F64::from_i64.i64",
        "F64::from_u8.u8",
        "F64::from_u16.u16",
        "F64::from_u32.u32",
        "F64::from_u64.u64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        16,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per source/destination slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete integer-to-float matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-integer-to-float-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "public integer-to-float requirements should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public integer-to-float canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-integer-to-float-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("integer-to-float conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_to_integer_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.float-to-integer.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 to every signed width toward zero",
        "binary32/binary64 to every unsigned width toward zero",
        "in-range Trapping result dispatch",
        "signed upper-overflow saturation",
        "unsigned negative-input saturation",
        "NaN saturation to zero",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x9bed_09d4_a629_c573;

    let canary = pass_canary(fixture_roster::FLOAT_RUNTIME_NAMED_FLOAT_TO_INTEGER_CONVERSION_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("public float-to-integer requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            [
                "I8::", "I16::", "I32::", "I64::", "U8::", "U16::", "U32::", "U64::",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            .then_some((operator_use, name.clone()))
        })
        .collect::<Vec<_>>();
    let selected_intrinsics = selected
        .iter()
        .map(|(_, intrinsic)| intrinsic.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_intrinsics = [
        "I8::from_f32.f32.exact",
        "I8::from_f64.f64.exact",
        "I8::from_f64.f64.saturating",
        "I16::from_f32.f32.exact",
        "I16::from_f64.f64.exact",
        "I32::from_f32.f32.exact",
        "I32::from_f64.f64.exact",
        "I32::from_f64.f64.trapping",
        "I32::from_f64.f64.saturating",
        "I64::from_f32.f32.exact",
        "I64::from_f64.f64.exact",
        "U8::from_f32.f32.exact",
        "U8::from_f64.f64.exact",
        "U16::from_f32.f32.exact",
        "U16::from_f64.f64.exact",
        "U32::from_f32.f32.exact",
        "U32::from_f64.f64.exact",
        "U64::from_f32.f32.exact",
        "U64::from_f64.f64.exact",
        "U64::from_f64.f64.saturating",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    let mut selected_plan_identities = selected
        .iter()
        .map(|(operator_use, _)| {
            checked
                .selected_provider_plans()
                .plan_by_report_fingerprint(operator_use.provider_plan_report_fingerprint)
                .expect("float-to-integer evidence must retain its selected plan")
                .report_fingerprint()
        })
        .collect::<Vec<_>>();
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        20,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per source/destination/domain slot"
    );

    for (operator_use, intrinsic) in selected {
        let typed_trees::expression::ExpressionNode::Cast(cast) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("selected conversion `{intrinsic}` must rewrite to one typed cast");
        };
        let expected_domain = if intrinsic.ends_with(".trapping") {
            numerics::arithmetic::ArithmeticDomain::Trapping
        } else if intrinsic.ends_with(".saturating") {
            numerics::arithmetic::ArithmeticDomain::Saturating
        } else {
            numerics::arithmetic::ArithmeticDomain::Exact
        };
        assert_eq!(
            cast.domain, expected_domain,
            "wrong rewrite for `{intrinsic}`"
        );
    }

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete float-to-integer matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-to-integer-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "public float-to-integer requirements should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public float-to-integer canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-to-integer-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("float-to-integer conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x86_64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_to_integer_rejections_are_explicit() {
    for &(name, expected) in fixture_roster::FLOAT_TO_INTEGER_FAIL_CANARIES {
        let diagnostics = compile_canary_without_output(&fail_canary(name))
            .expect_err("invalid public float-to-integer call unexpectedly compiled");
        let rendered = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains(expected),
            "{name} should report `{expected}`, got:\n{rendered}"
        );
    }
}

#[test]
fn named_float_to_integer_trapping_requirements_trap_in_both_engines() {
    for &name in fixture_roster::FLOAT_TO_INTEGER_TRAP_PASS_CANARIES {
        let canary = pass_canary(name);
        let main_path = canary.join("main.omg");
        let checked =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
                .expect("named Trapping float-to-integer requirement should compile");
        let interpreted = interpret(&checked, &[]);
        assert!(
            interpreted.error.is_some(),
            "{name} reached its post-conversion sentinel in the interpreter"
        );

        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir = std::env::temp_dir().join(format!(
            "omega-named-float-int-{leaf}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("named Trapping float-to-integer requirement should compile natively");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("named Trapping float-to-integer canary should start");
        assert!(
            !output.status.success(),
            "{name} reached its post-conversion sentinel natively"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}
