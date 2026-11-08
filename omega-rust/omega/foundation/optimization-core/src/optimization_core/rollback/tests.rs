use super::{OptimizationRollback, OptimizationRollbackInputError};
use crate::{Optimization, OptimizationSelections};

#[test]
fn exact_names_reject_aliases_and_duplicates() {
    assert_eq!(
        OptimizationRollback::from_exact_names(["control_flow_cleanup"]),
        Err(OptimizationRollbackInputError::UnknownName(
            "control_flow_cleanup".to_owned()
        ))
    );
    assert_eq!(
        OptimizationRollback::from_exact_names(["ControlFlowCleanup", "ControlFlowCleanup"]),
        Err(OptimizationRollbackInputError::DuplicateRule(
            Optimization::ControlFlowCleanup
        ))
    );
}

#[test]
fn settlement_preserves_authored_selection_and_records_only_subtraction() {
    let build_selected = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::SparseConditionalConstantPropagation,
    ])
    .expect("distinct authored rules");
    let rollback =
        OptimizationRollback::new([Optimization::ControlFlowCleanup]).expect("one disabled rule");
    let settlement = rollback.settle(&build_selected);
    let effective = settlement.effective().clone();
    let receipt = settlement.into_receipt().expect("nonempty request");
    assert_eq!(receipt.build_selected(), &build_selected);
    assert_eq!(receipt.requested_disabled(), rollback.requested_disabled());
    assert_eq!(receipt.actually_disabled(), rollback.requested_disabled());
    assert_eq!(receipt.effective(), &effective);
    assert_eq!(
        effective.as_slice(),
        &[Optimization::SparseConditionalConstantPropagation]
    );
    assert!(receipt.is_consistent());
    assert_eq!(
        receipt.to_string(),
        "requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup]; effective=[SparseConditionalConstantPropagation]"
    );

    let identity = OptimizationRollback::default().settle(&build_selected);
    assert_eq!(identity.effective(), &build_selected);
    assert!(identity.into_receipt().is_none());

    let empty_build = OptimizationSelections::default();
    let no_op = rollback.settle(&empty_build);
    assert_eq!(no_op.effective(), &empty_build);
    let no_op_receipt = no_op
        .into_receipt()
        .expect("known unselected rule is visible");
    assert!(no_op_receipt.actually_disabled().is_empty());
    assert!(no_op_receipt.is_consistent());

    let all_disabled = OptimizationRollback::new(build_selected.as_slice().iter().copied())
        .expect("canonical selection")
        .settle(&build_selected);
    assert!(all_disabled.effective().is_empty());
    assert_eq!(
        all_disabled
            .into_receipt()
            .expect("nonempty request")
            .actually_disabled(),
        &build_selected
    );
}
