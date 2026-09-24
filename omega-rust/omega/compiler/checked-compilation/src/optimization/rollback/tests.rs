use super::{OptimizationRollback, OptimizationRollbackInputError};
use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

const EXECUTION_PHASES: [OptimizationExecutionPhase; 5] = [
    OptimizationExecutionPhase::Psi,
    OptimizationExecutionPhase::SelectedLowering,
    OptimizationExecutionPhase::AllocationRecovery,
    OptimizationExecutionPhase::PostAllocationMachine,
    OptimizationExecutionPhase::FunctionRelativeLayout,
];

#[test]
fn empty_settlement_preserves_build_selection_without_a_receipt() {
    let selected = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
    ])
    .expect("canonical build selection");
    let settlement = OptimizationRollback::default().settle(&selected);

    assert_eq!(settlement.effective(), &selected);
    assert_eq!(settlement.into_receipt(), None);
}

#[test]
fn nonempty_settlement_keeps_effective_selection_and_receipt_coherent() {
    let selected = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
    ])
    .expect("canonical build selection");
    let rollback = OptimizationRollback::new([Optimization::CopyPropagation])
        .expect("canonical rollback selection");
    let settlement = rollback.settle(&selected);
    let expected = OptimizationSelections::new([Optimization::ControlFlowCleanup])
        .expect("canonical effective selection");

    assert_eq!(settlement.effective(), &expected);
    let receipt = settlement
        .into_receipt()
        .expect("a nonempty request retains one report receipt");
    assert_eq!(receipt.build_selected(), &selected);
    assert_eq!(receipt.effective(), &expected);
    assert!(receipt.is_consistent());
}

#[test]
fn every_exact_rule_is_subtractive_phase_local_and_idempotent() {
    let all = OptimizationSelections::new(Optimization::ALL)
        .expect("the closed optimization vocabulary is duplicate-free");

    for disabled in Optimization::ALL {
        let rollback = OptimizationRollback::from_exact_names([disabled.build_case_name()])
            .expect("every build case name is an exact rollback name");
        let receipt = rollback
            .reconcile(&all)
            .expect("a nonempty rollback request leaves custody");
        let expected_effective = OptimizationSelections::new(
            Optimization::ALL
                .into_iter()
                .filter(|optimization| *optimization != disabled),
        )
        .expect("a vocabulary subset remains duplicate-free");

        assert_eq!(receipt.build_selected(), &all, "{disabled:?}");
        assert_eq!(
            receipt.requested_disabled().as_slice(),
            &[disabled],
            "{disabled:?}"
        );
        assert_eq!(
            receipt.actually_disabled().as_slice(),
            &[disabled],
            "{disabled:?}"
        );
        assert_eq!(receipt.effective(), &expected_effective, "{disabled:?}");
        assert!(receipt.is_consistent(), "{disabled:?}");

        for phase in EXECUTION_PHASES {
            let expected_phase =
                OptimizationSelections::new(Optimization::ALL.into_iter().filter(|optimization| {
                    *optimization != disabled && optimization.execution_phase() == phase
                }))
                .expect("a phase vocabulary subset remains duplicate-free");
            let effective_phase = receipt.effective().for_phase(phase);
            assert_eq!(effective_phase, expected_phase, "{disabled:?} in {phase:?}");
            assert!(
                !effective_phase.contains(disabled),
                "{disabled:?} leaked into {phase:?}"
            );
        }

        let repeated = rollback
            .reconcile(receipt.effective())
            .expect("the authored rollback request remains visible");
        assert!(repeated.actually_disabled().is_empty(), "{disabled:?}");
        assert_eq!(repeated.effective(), receipt.effective(), "{disabled:?}");
    }
}

#[test]
fn exact_names_are_canonical_subtractive_and_idempotent_for_absent_rules() {
    let rollback = OptimizationRollback::from_exact_names([
        "X86SelectXorZeroI64MaterializationV1",
        "CopyPropagation",
    ])
    .unwrap();
    let selected = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let receipt = rollback.reconcile(&selected).unwrap();
    assert_eq!(
        receipt.requested_disabled().as_slice(),
        &[
            Optimization::CopyPropagation,
            Optimization::X86SelectXorZeroI64MaterializationV1,
        ]
    );
    assert_eq!(
        receipt.actually_disabled().as_slice(),
        &[Optimization::CopyPropagation]
    );
    assert_eq!(
        receipt.effective().as_slice(),
        &[Optimization::ControlFlowCleanup]
    );
}

#[test]
fn unknown_and_duplicate_names_fail_closed() {
    assert_eq!(
        OptimizationRollback::from_exact_names(["copy_propagation"]),
        Err(OptimizationRollbackInputError::UnknownName(
            "copy_propagation".into()
        ))
    );
    assert_eq!(
        OptimizationRollback::from_exact_names(["CopyPropagation", "CopyPropagation"]),
        Err(OptimizationRollbackInputError::DuplicateRule(
            Optimization::CopyPropagation
        ))
    );
}
