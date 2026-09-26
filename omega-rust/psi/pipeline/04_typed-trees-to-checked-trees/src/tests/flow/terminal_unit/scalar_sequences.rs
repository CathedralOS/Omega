//! Scalar declaration ordinals are dense across intervening Unit calls.
use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn array_sequence_rejoins_every_nested_constant_projection_selection() {
    let checked = checked(
        "data Rows {} const Rows::VALUES: [[[u8;1];1];1] = [[[7]]];
         machine nested(value: u8) -> [[u8;1];2] { [Rows::VALUES[0][0], [value]] }",
    );
    let symbol = machine_named(&checked, "nested");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let [typed_trees::statement::StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("array completion");
    };
    let source =
        validation::scalar_array_elements(&checked.typed, symbol, *expression, state.return_type)
            .expect("nested array source");
    assert_eq!(source.projections.len(), 2);
    assert_eq!(source.elements.len(), 2);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .is_some()
    );
    for projection in source.projections {
        let mut changed = checked.clone();
        let mut uses = arena::Arena::new();
        uses.append(checked_trees::CheckedOperatorUseFact {
            expression: projection,
            spelling: language_core::OperatorSpelling::Index,
            status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
            selected_operator_symbol: symbols::SymbolHandle::from_parts(1, 1),
            ..Default::default()
        });
        changed.facts.operators = checked_trees::CheckedOperatorFacts::with_roots(
            uses,
            arena::Arena::new(),
            arena::Arena::new(),
        );
        let replanned = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            replanned.for_machine(symbol).is_none(),
            "nested projection {projection:?}"
        );
    }
}

#[test]
fn array_sequence_requires_exact_computation_call_roots() {
    use checked_trees::{CheckedCallScalarArgument, CheckedScalarComputationKind};
    let checked = checked(
        "machine identity(value: u8) -> u8 { value }
        machine values(value: u8) -> [u8;2] { [identity(value), identity(value)] }",
    );
    let symbol = machine_named(&checked, "values");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("array calls compose");
    let [
        CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("calls belong to scalar computations");
    };
    let [
        CheckedCallScalarArgument::Computation(first),
        CheckedCallScalarArgument::Computation(second),
    ] = elements.as_slice()
    else {
        panic!("two call roots");
    };
    let (first, second) = (*first, *second);
    let computations = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::Call {
        source_call: first_call,
        call_ordinal: 0,
        ..
    } = computations.nodes.get(first).kind
    else {
        panic!("first exact call");
    };
    let CheckedScalarComputationKind::Call {
        source_call: second_call,
        call_ordinal: 1,
        ..
    } = computations.nodes.get(second).kind
    else {
        panic!("second exact call");
    };
    assert_ne!(first_call, second_call);
    for duplicate_source in [false, true] {
        let mut changed = checked.clone();
        if duplicate_source {
            let CheckedScalarComputationKind::Call { source_call, .. } = &mut changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(second)
                .kind
            else {
                panic!("call node");
            };
            *source_call = first_call;
        } else {
            let state_handle = changed
                .facts
                .flow
                .control
                .states
                .iter()
                .find_map(|(handle, state)| {
                    (state.machine_symbol == symbol && state.state_symbol == plan.state)
                        .then_some(handle)
                })
                .unwrap();
            changed
                .facts
                .flow
                .control
                .states
                .get_mut(state_handle)
                .calls = arena::HandleSpan::default();
        }
        let replanned = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            replanned.for_machine(symbol).is_none(),
            "duplicate_source={duplicate_source}"
        );
    }
}
