//! Primitive replacement completes scalar evaluation before its ordered store.

use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn computed_primitive_store_rejects_replaced_roots_and_literal_meaning() {
    use checked_trees::{
        CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedScalarExpression,
    };
    let original = crate::front_end::checked_program(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32, choose: bool) {
            destination = match choose { true -> 7, false -> 9 };
        }
    "#,
    );
    let _original_artifact = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("Sink::fill"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("original computed assignment")
    .into_artifact();
    for mutation in 0..3 {
        let mut changed = original.clone();
        if mutation == 0 {
            let operation =
                changed
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .flat_map(|plan| &mut plan.operations)
                    .find(|operation| {
                        matches!(operation,
                    checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. })
                    })
                    .unwrap();
            let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                value,
                ..
            } = operation
            else {
                unreachable!()
            };
            *value = CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(7),
            });
        } else if mutation == 1 {
            let (handle, _) = changed
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .next()
                .unwrap();
            changed
                .facts
                .values
                .scalar_computations
                .roots
                .get_mut(handle)
                .machine = symbols::SymbolHandle::invalid();
        } else {
            let handle = changed
                .facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .find_map(|(handle, node)| {
                    matches!(
                        node.kind,
                        CheckedScalarComputationKind::Value(
                            CheckedScalarExpression::IntegerLiteral { .. }
                        )
                    )
                    .then_some(handle)
                })
                .unwrap();
            let CheckedScalarComputationKind::Value(CheckedScalarExpression::IntegerLiteral {
                literal,
                ..
            }) = &mut changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle)
                .kind
            else {
                unreachable!()
            };
            *literal = numerics::literals::IntegerLiteral::from_value(42);
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("Sink::fill")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "store mutation {mutation}"
        );
    }
}
