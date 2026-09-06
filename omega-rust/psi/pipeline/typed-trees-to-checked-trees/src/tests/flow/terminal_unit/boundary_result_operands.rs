//! A returned boundary scalar keeps operand evaluation separate from settlement.

use super::*;
use checked_trees::{CheckedCallScalarArgument, CheckedScalarComputationKind};

const SOURCE: &str = r#"
    machine identity(value: bool) -> bool { value }
    pub data Receipt [linear] { value: u64; }
    boundary machine Receipt::settle(self, value: bool) -> bool
        reaches PortIo ensures true;
    data Root {}
    machine Root::enter(receipt: Receipt) -> bool reaches PortIo {
        let result: bool = receipt.settle(identity(true) && identity(false));
        result
    }
"#;

#[test]
fn returned_boundary_result_retains_one_outer_call_and_operand_roots() {
    let checked = checked(SOURCE);
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine)
        .expect("scalar-returning body retains boundary operand evaluation");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        scalar_arguments,
        completion_receipts,
        structural_arguments,
        ..
    } = &plan.boundary_call
    else {
        panic!("one boundary operation");
    };
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    assert_eq!(plan.entry_claims.len(), 1);
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        completion_receipts[0].claim_identity,
        plan.entry_claims[0].claim_identity
    );
    let [CheckedCallScalarArgument::Computation(root)] = scalar_arguments.as_slice() else {
        panic!("one Boolean operand graph");
    };
    let computations = &checked.facts.values.scalar_computations;
    assert!(matches!(
        computations.nodes.get(*root).kind,
        CheckedScalarComputationKind::Select { .. }
    ));
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
        .collect::<Vec<_>>();
    assert_eq!(
        roots.len(),
        1,
        "outer boundary call is not duplicated as an initializer computation"
    );
    assert_eq!(
        roots[0].1.role,
        CheckedScalarExpressionRole::BoundaryCallArgument {
            call_ordinal: 0,
            argument_ordinal: 0
        }
    );
    assert_eq!(plan.return_statement_ordinal, 1);
    let source = checked
        .machines()
        .iter()
        .find(|source| source.symbol == machine)
        .unwrap();
    let state = &checked.machine_states(source)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("authored result");
    };
    assert!(validation::result_initializer_call_is_supported(
        &checked.typed,
        source,
        local.initial_value
    ));
    assert!(!validation::unit_result_initializer_call_is_supported(
        &checked.typed,
        source,
        local.initial_value
    ));
}

#[test]
fn returned_boundary_result_rejects_invalid_nested_or_outer_occurrences() {
    let original = checked(SOURCE);
    let machine = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine)
        .unwrap();
    let source_calls = original
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine && state.state_symbol == plan.state)
                .then_some(state.calls)
        })
        .unwrap();
    let handles = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter_map(|(handle, call)| {
            original
                .facts
                .flow
                .control
                .calls
                .span(source_calls)
                .unwrap()
                .iter()
                .any(|candidate| std::ptr::eq(candidate, call))
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 3);
    for mutation in 0..3 {
        let mut changed = original.clone();
        let handle = handles
            .iter()
            .copied()
            .find(|handle| {
                (original.facts.flow.control.calls.get(*handle).call_ordinal == 0)
                    == (mutation == 0)
            })
            .unwrap();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.authored_expression = arena::Handle::invalid(),
            1 => call.call_ordinal = 0,
            _ => call.statement_index = 1,
        }
        let rebuilt =
            crate::flow::build_checked_boundary_scalar_return_plans(&changed.typed, &changed.facts);
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "occurrence mutation {mutation} rejects"
        );
    }
}
