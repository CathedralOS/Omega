use super::conditional_fixture::Comparison;
use super::*;
use optimization_core::{Optimization, OptimizationSelections};
use semantic_vocabulary::IntegerSign;

#[test]
fn catalog_integer_predicates_use_shared_publication_without_opt_in() {
    for comparison in [
        Comparison::NotEqual,
        Comparison::EqualZero,
        Comparison::NotEqualZero,
    ] {
        let (semantic, proof) = conditional_fixture::artifact(comparison, IntegerSign::Unsigned);
        for target in [
            target::NativeTarget::windows_x64(),
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ] {
            let selected = if target.architecture == target::Architecture::X86_64 {
                Optimization::X86SelectXorZeroI64MaterializationV1
            } else {
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            };
            for selections in [
                OptimizationSelections::default(),
                OptimizationSelections::new([selected]).unwrap(),
            ] {
                publish(&semantic, &proof, target, &selections);
            }
        }
    }
}

#[test]
fn boolean_parameter_publication_waits_for_its_ordinary_scalar_abi() {
    let (semantic, proof) =
        conditional_fixture::artifact(Comparison::BooleanParameter, IntegerSign::Unsigned);
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        let optimized = crate::optimize_verified_abstract_input(
            input,
            crate::compiler_baseline_request_v1(&OptimizationSelections::default()),
        )
        .unwrap();
        let target_program =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
            )
            .unwrap();
        assert!(
            target_program.target_operations().functions[0]
                .fixed_integer_scalar_abi
                .is_none()
        );
        assert!(!is_fragment_publication_program(&target_program));
        // The catalog still admits the form for physical construction. Only
        // native publication lacks the ordinary Boolean ABI carrier.
        target_operations_to_selected_instructions::legalize_target_operations(
            target_program.target_operations(),
            target_program.optimized().plan(),
            target_program.optimized().unit(),
        )
        .unwrap();
    }
}

#[test]
fn scalar_conditional_fragments_reach_native_object_publication() {
    for (comparison, sign) in [
        (Comparison::Equal, IntegerSign::Unsigned),
        (Comparison::LessThan, IntegerSign::Unsigned),
        (Comparison::LessOrEqual, IntegerSign::Unsigned),
        (Comparison::LessThan, IntegerSign::Signed),
        (Comparison::LessOrEqual, IntegerSign::Signed),
    ] {
        let (semantic, proof) = conditional_fixture::artifact(comparison, sign);
        for target in [
            target::NativeTarget::windows_x64(),
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ] {
            let architecture = target.architecture;
            let physical_rule = if architecture == target::Architecture::X86_64 {
                Optimization::X86SelectXorZeroI64MaterializationV1
            } else {
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            };
            let mut choices = vec![
                OptimizationSelections::default(),
                OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                    .unwrap(),
                OptimizationSelections::new([physical_rule]).unwrap(),
            ];
            if architecture == target::Architecture::X86_64 {
                choices.push(
                    OptimizationSelections::new([
                        Optimization::X86RelaxConditionalBranchesToRel8V1,
                    ])
                    .unwrap(),
                );
            }
            let mut baseline_bytes = 0;
            for selections in choices {
                let (published, mut emitted) = publish(&semantic, &proof, target, &selections);
                let function = &emitted.fragments().functions[0];
                let terminal = function.blocks[0].instructions.last().unwrap();
                assert!(matches!(
                    terminal.control,
                    machine_code::FunctionFragmentControlProvenance::ConditionalBranch { .. }
                ));
                assert_eq!(function.provenance.edges.len(), 4);
                let bytes = published.text_bytes().len();
                if selections.is_empty() {
                    baseline_bytes = bytes;
                }
                if selections.contains(Optimization::X86RelaxConditionalBranchesToRel8V1) {
                    assert_eq!(terminal.bytes.len(), 2);
                    assert!(
                        bytes < baseline_bytes,
                        "selected relaxation must change real branch bytes"
                    );
                }
                if selections.contains(Optimization::X86SelectXorZeroI64MaterializationV1) {
                    assert!(
                        bytes < baseline_bytes,
                        "selected zero materialization must rewrite a real arm"
                    );
                }
                // Independent fragment replay rejects a plausible redirected
                // branch against the retained selected/layout evidence.
                let terminal = emitted.fragments_mut().functions[0].blocks[0]
                    .instructions
                    .last_mut()
                    .unwrap();
                let machine_code::FunctionFragmentBranchEvidence::Conditional(branch) =
                    terminal.branch.as_deref_mut().unwrap()
                else {
                    unreachable!()
                };
                branch.when_taken_offset = branch.when_fallthrough_offset;
                assert!(
                    machine_emission::validate_optimized_function_fragment_emission(&emitted)
                        .is_err()
                );
            }
        }
    }
}

fn publish(
    semantic: &[u8],
    proof: &[u8],
    target: target::NativeTarget,
    selections: &OptimizationSelections,
) -> (
    image_emission::ObjectArtifact,
    machine_emission::StagedOptimizedFunctionFragmentEmission,
) {
    let build = || {
        let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            semantic,
            proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        let optimized = crate::optimize_verified_abstract_input(
            input,
            crate::compiler_baseline_request_v1(selections),
        )
        .unwrap();
        let validation = optimized.validation();
        let plan = optimized.plan().clone();
        let post_terminal = optimized.selections().project_post_terminal();
        let target_program =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
            )
            .unwrap();
        assert!(
            is_fragment_publication_program(&target_program),
            "default production must use the shared stages"
        );
        let physical = crate::stage_optimized_verified_physical_pipeline(
            target_program,
            post_terminal.selections(),
        )
        .unwrap_or_else(|error| panic!("{target:?} {selections:?}: {error:?}"));
        (physical, plan, validation)
    };
    let (physical, plan, validation) = build();
    let demands =
        boundary_applications::TerminalBoundaryApplicationDemands::new(plan.psi, Vec::new())
            .unwrap();
    let realizations =
        boundary_applications::TerminalBoundaryApplicationRealizations::new(&demands, Vec::new())
            .unwrap();
    let coverage =
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .unwrap();
    let selected_lowering = selections.contains(Optimization::SelectedIncomingU12ExactAddImmediate);
    let (published, scope) = emit_optimized_fragments(
        physical,
        OptimizedFragmentPublicationRequest {
            boundary_application_coverage: selected_lowering.then_some(&coverage),
            optimized_plan: &plan,
            terminal: validation.psi(),
            validation: validation.identity(),
            final_unit: validation.final_unit(),
        },
    )
    .unwrap_or_else(|error| panic!("{target:?} {selections:?}: publication {error:?}"));
    let image = image_emission::emit_executable_image(&published, 3).unwrap();
    image_emission::validate_executable_image(&published, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let encoded = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&encoded).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    assert_eq!(
        image_emission::derive_installation_stack_demand(&decoded, &image, published.entry())
            .unwrap(),
        image_emission::derive_stack_demand(&published, published.entry()).unwrap(),
    );
    if selected_lowering {
        assert!(matches!(
            scope,
            native_artifact::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
        ));
    }
    // Retain the current fragment stage for corruption controls. The published
    // object above is validated through image and installation replay; no
    // legacy machine-plan reconstruction participates in this test.
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        build().0.into_function_fragment_emission_source(),
    )
    .unwrap();
    machine_emission::validate_optimized_function_fragment_emission(&emitted).unwrap();
    (published, emitted)
}

#[test]
fn source_common_return_conditionals_use_the_shared_native_pipeline() {
    for (operand, comparison) in [
        ("u64", "=="),
        ("u64", "<"),
        ("u64", "<="),
        ("i64", "<"),
        ("i64", "<="),
    ] {
        let checked = crate::tests::fixtures::checked_source::checked(&format!(
            "machine value(left: {operand}, right: {operand}) -> u64\nrequires true\nensures result == result\n{{ transition left {comparison} right {{ true -> 1234605616436508552u64 _ -> 0u64 }} }}"
        ));
        let artifact = terminal_production::produce_terminal_artifact(&checked, "value").unwrap();
        for target in [
            target::NativeTarget::windows_x64(),
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
        ] {
            let selections = [
                OptimizationSelections::default(),
                OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                    .unwrap(),
                OptimizationSelections::new([
                    if target.architecture == target::Architecture::X86_64 {
                        Optimization::X86RelaxConditionalBranchesToRel8V1
                    } else {
                        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                    },
                ])
                .unwrap(),
            ];
            for selection in selections {
                let (_, mut emitted) = publish(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    target,
                    &selection,
                );
                let function = &emitted.fragments().functions[0];
                assert_eq!(function.blocks.len(), 4);
                assert_eq!(function.provenance.edges.len(), 5);
                assert_eq!(
                    function
                        .blocks
                        .iter()
                        .filter(|block| matches!(
                            block.instructions.last().unwrap().control,
                            machine_code::FunctionFragmentControlProvenance::Return { .. }
                        ))
                        .count(),
                    1
                );
                assert_eq!(
                    function
                        .blocks
                        .iter()
                        .filter(|block| matches!(
                            block.instructions.last().unwrap().control,
                            machine_code::FunctionFragmentControlProvenance::Jump { .. }
                        ))
                        .count(),
                    2
                );
                let terminal = emitted.fragments_mut().functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| {
                        matches!(
                            instruction.control,
                            machine_code::FunctionFragmentControlProvenance::Jump { .. }
                        )
                    })
                    .unwrap();
                let machine_code::FunctionFragmentBranchEvidence::Jump(branch) =
                    terminal.branch.as_deref_mut().unwrap()
                else {
                    unreachable!()
                };
                branch.target_offset = 0;
                assert!(
                    machine_emission::validate_optimized_function_fragment_emission(&emitted)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn substituted_conditional_inputs_reject_at_legalization() {
    let (semantic, proof) = conditional_fixture::artifact(Comparison::Equal, IntegerSign::Unsigned);
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&OptimizationSelections::default()),
    )
    .unwrap();
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let plan = target.optimized().plan();
    let native = target.target_operations();
    let admitted = |abstracted: &abstract_operations::AbstractOperationPlan,
                    targeted: &target_operations::TargetOperationPlan| {
        target_operations_to_selected_instructions::legalize_target_operations(
            targeted,
            abstracted,
            target.optimized().unit(),
        )
        .is_ok()
    };
    assert!(is_fragment_publication_program(&target));
    assert!(admitted(plan, native));
    let mut wrong_order = plan.clone();
    let abstract_operations::AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &mut wrong_order.functions[0].operations[1]
    else {
        unreachable!()
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    assert!(!admitted(&wrong_order, native));
    let mut repeated_parameter = native.clone();
    let target_operations::TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: target_operations::TargetBooleanExpression::IntegerEqual { left, right, .. },
        ..
    } = &mut repeated_parameter.functions[0].operation
    else {
        unreachable!()
    };
    *right = left.clone();
    assert!(!admitted(plan, &repeated_parameter));
}
