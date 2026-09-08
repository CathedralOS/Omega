//! Primitive store prefixes shared with the Unit effect producer.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    // These bodies do not yet publish range/domain contracts. Inspect the
    // authored types rather than their constraint-erasing primitive carriers.
    if program
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.type_reference)
        .chain(std::iter::once(state.return_type))
        .any(|reference| {
            let node = program.type_reference_table.type_reference(reference);
            let node = match node {
                TypeReferenceNode::Reference { referee, .. } => {
                    program.type_reference_table.type_reference(*referee)
                }
                _ => node,
            };
            matches!(node, TypeReferenceNode::Constrained { .. })
        })
    {
        return None;
    }
    if !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || !facts
            .contract_plans
            .for_machine(machine.symbol)?
            .crash
            .published()
            .is_empty()
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let [StatementNode::Assignment(_), StatementNode::Expression(_)] = statements else {
        return None;
    };
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .is_empty()
    {
        return None;
    }
    // This is the actual authored effect prefix. Its assignment keeps the
    // source state's parameter identity, complete write frame and RHS facts.
    let store = build_write_only_primitive_store(
        program,
        facts,
        shapes,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        &statements[..1],
        None,
        None,
    )?;
    Some(vec![store])
}
