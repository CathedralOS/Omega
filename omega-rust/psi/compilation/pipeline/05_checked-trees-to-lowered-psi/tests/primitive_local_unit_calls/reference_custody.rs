//! Readable-reference initialization rejoins the authored input, not cached agreement.

use checked_trees::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn reference_initializer_rejects_coherent_cached_and_operation_read_substitution() {
    let source = super::reference_initializer::source("u8");
    let original = crate::front_end::checked_program(&source);
    let _ = super::artifact(&source);
    let caller = original
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .position(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
            })
        })
        .unwrap();
    let state_symbol = original.facts.flow.terminal_unit_effects.machines[caller].state;
    let state = original
        .machines()
        .iter()
        .flat_map(|machine| original.machine_states(machine))
        .find(|state| state.symbol == state_symbol)
        .unwrap();
    let parameters = original.state_parameters(state);
    let output = parameters[0].symbol;
    let input = parameters[1].symbol;
    for synchronize in [false, true] {
        let mut changed = original.clone();
        let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { value, .. } =
            &mut changed.facts.flow.terminal_unit_effects.machines[caller].operations[0]
        else {
            panic!("initialized primitive local");
        };
        let CheckedScalarExpression::StorageRead { symbol, .. } = value else {
            panic!("input must remain a storage read");
        };
        assert_eq!(*symbol, input);
        *symbol = output;
        let substituted = value.clone();
        if synchronize {
            let mut rows = changed
                .facts
                .values
                .scalar_expressions
                .expressions
                .iter_mut()
                .filter(|row| {
                    row.state == state_symbol
                        && row.statement_ordinal == 0
                        && row.role == CheckedScalarExpressionRole::StorageInitializer
                });
            rows.next().expect("initializer expression").expression = substituted;
            assert!(rows.next().is_none());
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("observe")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "reference read substitution with synchronized cache={synchronize}"
        );
    }
}

#[test]
fn retained_write_only_access_cannot_authorize_a_primitive_initializer_read() {
    for scalar in ["u8", "bool", "f32"] {
        let source = super::reference_initializer::source(scalar);
        let mut changed = crate::front_end::checked_program(&source);
        let _ = super::artifact(&source);
        let caller = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                })
            })
            .unwrap();
        assert_eq!(
            caller.structural_parameters[1].access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
        caller.structural_parameters[1].access =
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow;
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("observe")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "{scalar}: write-only parameter cannot retain a readable initializer"
        );
    }
}
