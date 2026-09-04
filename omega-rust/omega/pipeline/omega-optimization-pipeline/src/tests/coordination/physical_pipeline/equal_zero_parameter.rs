//! Baseline and AArch64 CBNZ physical routes for U64 parameter equality with zero.

use crate::tests::*;
use omega_machine_optimizer::{Aarch64CbnzFusionError, Aarch64CbnzFusionWorkAxis};

fn optimized_source_with_budget(
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let (semantic, proof) = conditional_u64_equal_zero_parameter_artifact();
    optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap()
}

fn optimized_source(
    selections: OptimizationSelections,
) -> omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    optimized_source_with_budget(selections, selected_lowering_budget())
}

fn baseline_layout(target: NativeTarget) -> StagedFixedFrameFunctionRelativeRealization {
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized_source(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        target,
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::FixedFrame { realization } = staged else {
        panic!("empty optimization selection must reach fixed-frame function-relative custody")
    };
    realization
}

#[test]
fn post_allocation_disabled_baseline_retains_compare_zero_and_nonzero_branch_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let realization = baseline_layout(target);
        let homes = realization.homes();
        let layout = realization.layout();
        let selected = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        assert_eq!(
            selected
                .optimized_target()
                .optimized()
                .selections()
                .as_slice(),
            [Optimization::CopyPropagation]
        );
        assert!(
            selected
                .optimized_target()
                .optimized()
                .selections()
                .for_phase(
                    omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine
                )
                .is_empty()
        );
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
        let branch = rows
            .iter()
            .find(|row| {
                row.alternative.family
                    == omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
            })
            .unwrap();
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(compare.bytes, [0x48, 0x85, 0xff]);
                assert_eq!(branch.bytes[0], 0x0f);
                assert_eq!(branch.bytes[1], 0x85);
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    u32::from_le_bytes(compare.bytes.as_slice().try_into().unwrap()),
                    0xf100_001f
                );
                assert_eq!(
                    u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_001f,
                    0x5400_0001
                );
            }
        }
    }
}

#[test]
fn explicit_aarch64_cbnz_selection_crosses_allocation_and_elides_the_compare() {
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized_source(selections.clone()),
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = staged
    else {
        panic!("equal-zero selection must reach the post-allocation CBNZ route")
    };
    let allocation = realization.allocation().current();
    assert!(matches!(
        allocation.evidence(),
        omega_selected_instructions_to_register_homes::AllocationEvidence::RegisterHomes(_)
    ));
    let homes = &allocation;
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(optimization) =
        realization.optimization()
    else {
        panic!("the selected post-allocation rule must be CBNZ fusion")
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
    assert_eq!(
        realization.optimization().selections(),
        selections.identity()
    );
    assert_eq!(
        validate_optimized_aarch64_cbnz_fusion_custody(homes, realization.machine(), optimization,)
            .unwrap(),
        optimization.custody()
    );
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(&realization)
            .unwrap(),
        *realization.custody()
    );

    let action = &optimization.fusion().plan().actions[0];
    let baseline_rows = realization.baseline_layout().functions()[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|row| (row.instruction, row))
        .collect::<std::collections::BTreeMap<_, _>>();
    let fused_rows = realization.layout().functions()[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|row| (row.instruction, row))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(baseline_rows[&action.compare].bytes.len(), 4);
    assert_eq!(baseline_rows[&action.branch].bytes.len(), 4);
    assert!(fused_rows[&action.compare].bytes.is_empty());
    let branch = fused_rows[&action.branch];
    assert_eq!(branch.bytes.len(), 4);
    assert_eq!(
        u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_0000,
        0xb500_0000
    );
    assert_eq!(
        realization.baseline_layout().functions()[0].byte_count,
        realization.layout().functions()[0].byte_count + 4
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
            OperationId::new(20_019).unwrap(),
            OperationId::new(20_011).unwrap()
        ]
    );
    assert_eq!(
        compare.provenance.values,
        [
            ValueId::new(20_005).unwrap(),
            ValueId::new(20_006).unwrap(),
            ValueId::new(20_007).unwrap(),
        ]
    );
    assert_eq!(compare.provenance.fuel.len(), 2);
    assert_eq!(
        compare
            .provenance
            .fuel
            .iter()
            .map(|fuel| fuel.site)
            .collect::<Vec<_>>(),
        [
            PsiProvenance::Operation(OperationId::new(20_019).unwrap()),
            PsiProvenance::Operation(OperationId::new(20_011).unwrap()),
        ]
    );
    let retained_fuel = spans
        .iter()
        .map(|span| {
            let control_fuel = match &span.control {
                omega_machine_code::FunctionFragmentControlProvenance::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => when_taken.fuel.len() + when_fallthrough.fuel.len(),
                omega_machine_code::FunctionFragmentControlProvenance::None
                | omega_machine_code::FunctionFragmentControlProvenance::DirectInternalCall {
                    ..
                }
                | omega_machine_code::FunctionFragmentControlProvenance::Return { .. } => 0,
            };
            (span.provenance.fuel.len() + control_fuel) as u64
        })
        .sum::<u64>();
    assert_eq!(
        emitted
            .manifest()
            .record()
            .statistics
            .zero_byte_instruction_spans,
        1
    );
    assert_eq!(
        emitted
            .manifest()
            .record()
            .statistics
            .logical_fuel_settlements,
        retained_fuel
    );
}

#[test]
fn fuel_bearing_equal_zero_route_enforces_each_representable_first_over_work_boundary() {
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
                optimized_source_with_budget(selections.clone(), budget),
                NativeTarget::linux_arm64(),
                &[],
            ),
            Err(
                OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization(
                    OptimizedPostAllocationMachineOptimizationError::Fusion(
                        Aarch64CbnzFusionError::BudgetExceeded(actual)
                    )
                )
            ) if actual == axis
        ));
    }
}
