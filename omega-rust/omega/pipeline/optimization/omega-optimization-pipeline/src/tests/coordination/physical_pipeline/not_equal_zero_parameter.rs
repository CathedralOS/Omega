//! Baseline and AArch64 CBNZ routes for U64 parameter inequality with zero.

use crate::tests::*;
use omega_machine_optimizer::{Aarch64CbnzFusionError, Aarch64CbnzFusionWorkAxis};

fn optimized_source(
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let (semantic, proof) = conditional_u64_not_equal_zero_parameter_artifact();
    optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap()
}

#[test]
fn disabled_not_equal_zero_baseline_retains_compare_and_branch_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized_source(
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
                selected_lowering_budget(),
            ),
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::FixedFrame { realization } = staged else {
            panic!("disabled post-allocation selection must reach fixed-frame custody")
        };
        let homes = realization.homes();
        let machine = realization.machine();
        let selected = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let encoding = stage_optimized_layout_independent_selected_form_encoding(
            selected.selected(),
            machine,
            selected.register_environment().physical(),
        )
        .unwrap();
        let layout = stage_optimized_resolved_selected_form_layout(
            selected.selected(),
            machine,
            selected.register_environment().physical(),
            &encoding,
        )
        .unwrap();
        let entry = &selected.selected().plan().functions[0].blocks[0];
        assert_eq!(
            entry.instructions[0].kind,
            SelectedInstructionKind::CompareI64Zero
        );
        assert!(matches!(
            entry.terminator,
            SelectedTerminator::ConditionalBranch { ref instruction, .. }
                if instruction.kind == SelectedInstructionKind::ConditionalBranchNonZero
        ));
        let rows = layout.functions()[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let compare = rows
            .iter()
            .find(|row| {
                row.alternative.family
                    == omega_selected_instructions::MachineAlternativeFamily::CompareI64Zero
            })
            .unwrap();
        assert_eq!(
            compare.bytes.len(),
            if target.architecture == omega_target::Architecture::X86_64 {
                3
            } else {
                4
            }
        );
        assert!(rows.iter().any(|row| {
            row.alternative.family
                == omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
        }));
    }
}

#[test]
fn explicit_cbnz_fuses_not_equal_zero_and_preserves_both_operation_spans() {
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized_source(selections, selected_lowering_budget()),
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = staged
    else {
        panic!("not-equal-zero selection must reach the CBNZ route")
    };
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(optimization) =
        realization.optimization()
    else {
        panic!("selected post-allocation rule must be CBNZ fusion")
    };
    assert_eq!(optimization.fusion().receipt().action_count(), 1);
    assert_eq!(
        optimization.fusion().plan().usage,
        OptimizationWorkUsage {
            rule_evaluations: 2,
            candidates: 1,
            validation_steps: 1,
            commits: 1,
            iterations: 2,
        }
    );
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    validate_optimized_function_fragment_emission(&emitted).unwrap();
    let spans = emitted.fragments().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect::<Vec<_>>();
    let compare = spans
        .iter()
        .find(|span| {
            span.alternative.family
                == omega_selected_instructions::MachineAlternativeFamily::CompareI64Zero
        })
        .unwrap();
    assert!(compare.bytes.is_empty());
    assert_eq!(
        compare.provenance.operations,
        [
            OperationId::new(20_119).unwrap(),
            OperationId::new(20_111).unwrap()
        ]
    );
    assert_eq!(compare.provenance.fuel.len(), 2);
    let branch = spans
        .iter()
        .find(|span| {
            span.alternative.family
                == omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
        })
        .unwrap();
    assert_eq!(
        branch.provenance.operations,
        [OperationId::new(20_120).unwrap()]
    );
    assert_eq!(branch.provenance.fuel.len(), 1);
    assert_eq!(
        emitted
            .manifest()
            .record()
            .statistics
            .zero_byte_instruction_spans,
        1
    );
}

#[test]
fn not_equal_zero_route_enforces_cbnz_first_over_work_boundaries() {
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(1, 1, 1, 1, 2).unwrap(),
            Aarch64CbnzFusionWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(2, 1, 1, 1, 1).unwrap(),
            Aarch64CbnzFusionWorkAxis::Iterations,
        ),
    ] {
        assert!(matches!(
            stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized_source(selections.clone(), budget),
                NativeTarget::linux_arm64(),
                &[],
            ),
            Err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization(
                OptimizedPostAllocationMachineOptimizationError::Fusion(
                    Aarch64CbnzFusionError::BudgetExceeded(actual)
                )
            )) if actual == axis
        ));
    }
}
