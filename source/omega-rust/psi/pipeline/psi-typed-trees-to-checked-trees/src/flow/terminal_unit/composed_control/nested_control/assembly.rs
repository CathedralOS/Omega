//! Checked state assembly and provider closure for admitted decision trees.

use super::*;

pub(super) fn finish(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
    topology: super::topology::NestedTopology<'_>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let leaves = topology
        .leaves
        .iter()
        .map(|state| {
            super::super::leaves::build(program, facts, machine, state, boundaries, &[], &[])
        })
        .collect::<Option<Vec<_>>>()?;
    let provider_inputs = topology
        .leaves
        .iter()
        .zip(&leaves)
        .map(|(state, leaf)| {
            let flow = state_flow(facts, machine.symbol, state.symbol)?;
            Some((
                *state,
                facts.flow.control.calls.span_or_empty(flow.calls),
                leaf.operations.as_slice(),
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let provider_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        &topology.attachment,
        &provider_inputs,
    )?;
    let control_operations = topology
        .controls
        .iter()
        .map(|state| super::operations::build(program, facts, machine, state))
        .collect::<Option<Vec<_>>>()?;
    let mut checked_states = topology
        .controls
        .iter()
        .zip(topology.control_parameters)
        .zip(topology.guards)
        .zip(topology.edges)
        .zip(control_operations)
        .map(|((((state, parameters), guard), edges), operations)| {
            CheckedComposedUnitControlStatePlan {
                state: state.symbol,
                structural_parameters: Vec::new(),
                scalar_parameters: parameters,
                entry_claims: Vec::new(),
                operations,
                terminator: CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    when_true: edges[0].clone(),
                    when_false: edges[1].clone(),
                },
            }
        })
        .collect::<Vec<_>>();
    checked_states.extend(leaves);
    super::super::assembly::finish(
        facts,
        machine,
        topology.attachment,
        provider_requirements,
        checked_states,
    )
}
