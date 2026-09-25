//! Address-offset folding through the pre-allocation executor and the
//! `optimize_selected_instructions` entrance, on compiler-produced selected
//! plans. `staged_byte_field_call_with_selections` selects a projected
//! bounded byte-field call argument — `AddressOffset` staging for the field
//! pointer, then the live-length `Load64` and the data-pointer
//! `AddressOffset` reading it — the admissible fold surface;
//! `staged_byte_view_call_with_selections` selects the same call over a root
//! view parameter whose consumers decline.

use crate::tests::{
    NativeTarget, Optimization, OptimizationSelections, OptimizedPreAllocationCustodyError,
    PostAllocationSelectedTransformation, PreAllocationOptimizationCustodyFieldForTest,
    PreAllocationPolicy, PreAllocationTransformationIdentity, SelectedInstructionKind,
    StagedOptimizedAllocationLegality, StagedPreAllocationOptimizationRun,
    optimize_selected_instructions, run_pre_allocation_optimizations,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_register_homes_after_pre_allocation, staged_byte_field_call_with_selections,
    staged_byte_view_call_with_selections, validate_pre_allocation_optimization_custody,
};
use optimization_core::OptimizationWorkBudget;
use selected_instructions_to_selected_instructions::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    ValidatedSelectedAnalysis,
};

fn fold_suite() -> OptimizationSelections {
    OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SelectedAddressOffsetFoldV1,
    ])
    .unwrap()
}

fn legality(
    selected: crate::tests::StagedOptimizedSelectedInstructions,
) -> StagedOptimizedAllocationLegality {
    stage_optimized_allocation_legality(
        stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap(),
    )
    .unwrap()
}

fn byte_field_legality(target: NativeTarget) -> StagedOptimizedAllocationLegality {
    legality(staged_byte_field_call_with_selections(
        target,
        fold_suite(),
        crate::tests::selected_lowering_budget(),
    ))
}

fn instruction_count(run: &StagedPreAllocationOptimizationRun, byte_offset: u32) -> usize {
    run.current()
        .selected_plan()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            instruction.kind == SelectedInstructionKind::AddressOffset { byte_offset }
        })
        .count()
}

/// The projected field pointer's two descriptor consumers fold onto the
/// referent root: the live-length `Load64` carries the field's own offset
/// and the data-pointer projection carries the sum of both displacements.
/// Each committed step is one validated `AddressFold`; the terminal sweep
/// finds every remaining candidate inadmissible.
#[test]
fn positive_folds_every_admissible_displaced_consumer() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let run = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        assert_eq!(run.steps().len(), 2, "{target:?}");
        assert_eq!(
            run.policy(),
            PreAllocationPolicy::ADDRESS_FOLD_V1,
            "{target:?}"
        );
        for iteration in run.custody().iterations() {
            assert!(
                matches!(
                    iteration.transformation(),
                    PreAllocationTransformationIdentity::AddressFold(_),
                ),
                "{target:?}: every committed step is an address fold"
            );
        }
        // `AddressOffset { 8 }` is retained for any other readers, while the
        // folded chain reads the root with the combined displacement `8+8`.
        assert_eq!(
            instruction_count(&run, 16),
            1,
            "{target:?}: the folded data pointer carries the combined offset"
        );
        assert!(
            run.attempt().candidates() > 0,
            "{target:?}: the terminal sweep still sees fold-shaped consumers"
        );
        assert_eq!(
            run.attempt().candidates(),
            run.attempt().declined(),
            "{target:?}"
        );
        assert_eq!(
            validate_pre_allocation_optimization_custody(&run).unwrap(),
            *run.custody(),
            "{target:?}"
        );
    }
}

/// The root-parameter view stages its pointer with a `CopyI64`, not an
/// `AddressOffset`, so every displaced consumer in the plan declines and the
/// run publishes the unchanged program.
#[test]
fn negative_declines_every_candidate_and_publishes_unchanged() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_byte_view_call_with_selections(
            target,
            fold_suite(),
            crate::tests::selected_lowering_budget(),
        );
        let before = selected.selected().plan().clone();
        let run = run_pre_allocation_optimizations(legality(selected)).unwrap();
        assert!(run.steps().is_empty(), "{target:?}");
        assert!(run.attempt().candidates() > 0, "{target:?}");
        assert_eq!(
            run.attempt().candidates(),
            run.attempt().declined(),
            "{target:?}"
        );
        assert_eq!(run.current().selected_plan(), &before, "{target:?}");
        assert_eq!(
            validate_pre_allocation_optimization_custody(&run).unwrap(),
            *run.custody(),
            "{target:?}"
        );
    }
}

/// The measured work the honest run reports is exactly what its per-pass
/// budget must carry: the same usage admits, one unit less aborts the pass.
#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let run = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        let usage = run.custody().usage();
        let exact = OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps,
            usage.commits,
            usage.iterations,
        )
        .unwrap();
        run_pre_allocation_optimizations(legality(staged_byte_field_call_with_selections(
            target,
            fold_suite(),
            exact,
        )))
        .unwrap();
        let short = OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps - 1,
            usage.commits,
            usage.iterations,
        )
        .unwrap();
        assert!(
            matches!(
                run_pre_allocation_optimizations(legality(staged_byte_field_call_with_selections(
                    target,
                    fold_suite(),
                    short
                ),)),
                Err(
                    OptimizedPreAllocationCustodyError::PreAllocationBudgetExceeded { .. }
                        | OptimizedPreAllocationCustodyError::AddressFold(
                            crate::tests::AddressFoldError::WorkBudgetExceeded
                        )
                )
            ),
            "{target:?}"
        );
    }
}

/// Without the selection the suite carries no executed pre-allocation phase,
/// so the entrance publishes identity evidence and the plan keeps the
/// unprojected producer plus the zero-offset load and the second
/// eight-offset projection staged over it.
#[test]
fn disabled_selection_leaves_the_plan_untouched() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_byte_field_call_with_selections(
            target,
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            crate::tests::selected_lowering_budget(),
        );
        let before = selected.selected().plan().clone();
        let output = optimize_selected_instructions(selected).unwrap();
        assert_eq!(output.program().selected_plan(), &before, "{target:?}");
        assert_eq!(
            before
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::AddressOffset { byte_offset: 8 }
                    )
                })
                .count(),
            2,
            "{target:?}: the field projection and data pointer keep their own offsets"
        );
        assert!(matches!(
            output.into_replayed_evidence().unwrap(),
            SelectedInstructionOptimizationEvidence::Identity(_),
        ));
    }
}

/// Two executions over the same staged source produce identical steps,
/// terminal attempt, and custody receipt.
#[test]
fn repeated_runs_are_deterministic() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let first = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        let second = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        assert_eq!(first.custody(), second.custody(), "{target:?}");
        assert_eq!(first.steps(), second.steps(), "{target:?}");
        assert_eq!(first.attempt(), second.attempt(), "{target:?}");
        assert_eq!(
            first.current().selected_plan(),
            second.current().selected_plan(),
            "{target:?}"
        );
    }
}

/// The run ends only when a discovery sweep declines everything left: the
/// published program is itself a legal input whose next scan finds no
/// admissible candidate, which the retained terminal attempt records.
#[test]
fn published_run_is_a_clean_fixed_point() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let run = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        assert_eq!(
            run.attempt().candidates(),
            run.attempt().declined(),
            "{target:?}"
        );
        // Independent replay re-derives the whole iteration chain and the
        // terminal attempt from the source, not from the retained steps.
        assert_eq!(
            validate_pre_allocation_optimization_custody(&run).unwrap(),
            *run.custody(),
            "{target:?}"
        );
    }
}

/// Every single-field substitution of the retained custody receipt fails the
/// independent replay. Foreign receipts come from the opposite target's
/// authentic run.
#[test]
fn custody_rejects_every_one_field_substitution() {
    use OptimizedPreAllocationCustodyError::{ReceiptMismatch, SelectionProjectionMismatch};
    use PreAllocationOptimizationCustodyFieldForTest::*;
    let fields: [(
        &str,
        PreAllocationOptimizationCustodyFieldForTest,
        OptimizedPreAllocationCustodyError,
    ); 17] = [
        ("identity", Identity, ReceiptMismatch),
        ("source", Source, ReceiptMismatch),
        ("selections", Selections, SelectionProjectionMismatch),
        (
            "pre_allocation_selections",
            PreAllocationSelections,
            SelectionProjectionMismatch,
        ),
        ("policy", Policy, SelectionProjectionMismatch),
        ("budget", Budget, SelectionProjectionMismatch),
        ("usage", Usage, ReceiptMismatch),
        ("iteration_bound", IterationBound, ReceiptMismatch),
        ("removal_count", RemovalCount, ReceiptMismatch),
        (
            "initial_virtual_register_count",
            InitialVirtualRegisterCount,
            ReceiptMismatch,
        ),
        ("iterations", Iterations, ReceiptMismatch),
        ("attempt", Attempt, ReceiptMismatch),
        ("final_selected", FinalSelected, ReceiptMismatch),
        ("final_liveness", FinalLiveness, ReceiptMismatch),
        ("final_ranges", FinalRanges, ReceiptMismatch),
        ("final_legality", FinalLegality, ReceiptMismatch),
        (
            "final_virtual_register_count",
            FinalVirtualRegisterCount,
            ReceiptMismatch,
        ),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            run_pre_allocation_optimizations(byte_field_legality(target)).unwrap()
        };
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody(),
            donor.custody(),
            "{target:?}: the foreign target must produce a distinct custody receipt",
        );
        for (name, field, expected) in &fields {
            let mut substituted = build(target);
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(*field, &donor);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_pre_allocation_optimization_custody(&substituted),
                Err(expected.clone()),
                "{target:?}: independent replay must reject substituted custody field {name}",
            );
        }
    }
}

/// The stage entrance routes the selection to the pre-allocation executor
/// and publishes the run as replayable evidence carrying the transformed
/// program.
#[test]
fn entrance_publishes_pre_allocation_evidence() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_byte_field_call_with_selections(
            target,
            fold_suite(),
            crate::tests::selected_lowering_budget(),
        );
        let output = optimize_selected_instructions(selected).unwrap();
        assert_eq!(
            output
                .program()
                .selected_plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::AddressOffset { byte_offset: 16 }
                    )
                })
                .count(),
            1,
            "{target:?}: the entrance's published program carries the folded chain"
        );
        match output.into_replayed_evidence().unwrap() {
            SelectedInstructionOptimizationEvidence::PreAllocation(run) => {
                assert_eq!(run.steps().len(), 2, "{target:?}");
            }
            _ => panic!("{target:?}: expected pre-allocation evidence"),
        }
    }
}

/// A suite mixing the pre-allocation selection with a selected-lowering rule
/// spans two executed slices; the entrance rejects it rather than guessing an
/// execution order.
#[test]
fn mixed_executed_slices_are_an_unsupported_composition() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_byte_field_call_with_selections(
            target,
            OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedAddressOffsetFoldV1,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
            crate::tests::selected_lowering_budget(),
        );
        assert_eq!(
            optimize_selected_instructions(selected).unwrap_err(),
            SelectedInstructionOptimizationError::UnsupportedComposition,
            "{target:?}"
        );
    }
}

/// The optimized run feeds strict homes downstream: the post-allocation
/// manifest ledger records the committed transformations under their exact
/// address-fold family identity, in order, beside the analyses and home
/// identities it governs.
#[test]
fn downstream_homes_publish_the_address_fold_transformation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let run = run_pre_allocation_optimizations(byte_field_legality(target)).unwrap();
        let homes = stage_optimized_register_homes_after_pre_allocation(run).unwrap();
        let transformations = &homes
            .post_allocation_manifest()
            .record()
            .selected_transformations;
        assert_eq!(transformations.len(), 2, "{target:?}");
        for transformation in transformations {
            assert!(
                matches!(
                    transformation,
                    PostAllocationSelectedTransformation::AddressFold(_),
                ),
                "{target:?}: every ledger row is an address fold"
            );
        }
    }
}
