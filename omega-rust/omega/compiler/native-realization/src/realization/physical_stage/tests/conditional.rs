use super::conditional_fixture::Comparison;
use super::*;
use optimization_core::{Optimization, OptimizationSelections};
use semantic_vocabulary::IntegerSign;

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
                let plan = publish(&semantic, &proof, target, &selections);
                let function = &plan.functions[0];
                let machine_code::ScalarControlFlowEvidence::DirectConditional { branch } =
                    &function.scalar_stack.as_ref().unwrap().control_flow
                else {
                    panic!("actual branch custody");
                };
                assert_eq!(function.provenance.edges.len(), 4);
                let bytes = function.bytes.len();
                if selections.is_empty() {
                    baseline_bytes = bytes;
                }
                if selections.contains(Optimization::X86RelaxConditionalBranchesToRel8V1) {
                    assert_eq!(branch.branch_byte_count, 2);
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
                // Object construction independently decodes both paths. A
                // plausible but redirected branch record cannot pass replay.
                let mut corrupted = plan.clone();
                let machine_code::ScalarControlFlowEvidence::DirectConditional { branch } =
                    &mut corrupted.functions[0]
                        .scalar_stack
                        .as_mut()
                        .unwrap()
                        .control_flow
                else {
                    unreachable!()
                };
                branch.taken_offset = branch.fallthrough_offset;
                assert!(image_emission::build_object_artifact(&corrupted).is_err());
            }
        }
    }
}

fn publish(
    semantic: &[u8],
    proof: &[u8],
    target: target::NativeTarget,
    selections: &OptimizationSelections,
) -> machine_code::MachineCodePlan {
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
        fragment_program(&target_program),
        "default production must use the shared stages"
    );
    let physical = crate::stage_optimized_verified_physical_pipeline(
        target_program,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("{target:?} {selections:?}: {error:?}"));
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
            identity_scope: selections
                .is_empty()
                .then_some(native_artifact::NativePhysicalEvidenceScope::Unavailable),
            has_provider_installation: false,
            has_boundary_settlements: false,
            boundary_application_coverage: selected_lowering.then_some(&coverage),
            optimized_plan: &plan,
            terminal: validation.psi(),
            validation: validation.identity(),
            final_unit: validation.final_unit(),
        },
    )
    .unwrap_or_else(|error| panic!("{target:?} {selections:?}: publication {error:?}"));
    if selected_lowering {
        assert!(matches!(
            scope,
            native_artifact::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
        ));
    }
    image_emission::build_object_artifact(&published)
        .unwrap_or_else(|error| panic!("{target:?} {selections:?}: object {error:?}"));
    published
}

#[test]
fn source_common_return_conditionals_keep_their_existing_route() {
    let checked = crate::tests::fixtures::checked_source::checked(
        "machine value(left: u64, right: u64) -> u64
         requires true
         ensures result == result
         { transition left == right { true -> 1u64 _ -> 0u64 } }",
    );
    let artifact = terminal_production::produce_terminal_artifact(&checked, "value").unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
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
            optimized,
            target::NativeTarget::linux_x64(),
        )
        .unwrap();
    assert_eq!(
        target_program.optimized().plan().functions[0]
            .block_entries
            .len(),
        4
    );
    assert!(!fragment_program(&target_program));
    let assigned = target_operations_to_assigned_target_operations::assign_registers(
        target_program.target_operations(),
    )
    .unwrap();
    let plan = machine_emission::emit_machine_code(&assigned).unwrap();
    image_emission::build_object_artifact(&plan).unwrap();
}

#[test]
fn conditional_migration_excludes_unselected_input_shapes() {
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
    let abstracted = &target.optimized().plan().functions[0];
    let native = &target.target_operations().functions[0];
    assert!(fragment_shape::scalar_conditional(abstracted, native));
    let mut wrong_order = abstracted.clone();
    let abstract_operations::AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &mut wrong_order.operations[1]
    else {
        unreachable!()
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    assert!(!fragment_shape::scalar_conditional(&wrong_order, native));
    let mut repeated_parameter = native.clone();
    let target_operations::TargetOperation::ReturnIntegerExpressionConditionalControl {
        condition: target_operations::TargetBooleanExpression::IntegerEqual { left, right, .. },
        ..
    } = &mut repeated_parameter.operation
    else {
        unreachable!()
    };
    *right = left.clone();
    assert!(!fragment_shape::scalar_conditional(
        abstracted,
        &repeated_parameter
    ));
}
