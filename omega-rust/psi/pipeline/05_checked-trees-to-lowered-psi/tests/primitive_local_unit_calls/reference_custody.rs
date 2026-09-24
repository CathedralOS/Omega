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
fn readable_reference_initializer_keeps_authored_and_dense_parameter_positions_separate() {
    let source = super::reference_initializer::source("u8").replace(
        "output: &mut u8, input: &u8,\n            initial_output: &mut u8, replacement: u8",
        "replacement: u8, output: &mut u8, input: &u8,\n            initial_output: &mut u8",
    );
    let artifact = super::artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameters = &caller.structural_parameters;
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[1].position, 1);
    assert!(caller.blocks.iter().flat_map(|block| &block.operations).any(|operation|
        matches!(operation.kind, terminal_psi::OperationKind::PrimitiveScalarRead { source, ref path }
            if path.is_empty() && source == parameters[1].place)));
}

#[test]
fn write_only_primitive_inputs_cannot_initialize_locals_by_reading() {
    for scalar in [
        "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "bool", "f32", "f64",
    ] {
        let source = super::reference_initializer::source(scalar).replace(
            &format!("input: &{scalar}"),
            &format!("input: &write {scalar}"),
        );
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("write-only input cannot supply the initializer's value");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("write-only")
                    && diagnostic.message.contains("input")),
            "{scalar}: {diagnostics:?}"
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
