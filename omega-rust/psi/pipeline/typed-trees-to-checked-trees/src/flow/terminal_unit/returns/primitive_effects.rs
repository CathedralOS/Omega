//! Call-free primitive-reference returns and prefixes shared with Unit effects.

use super::*;

/// Discover call-free primitive-reference bodies before their Unit callers. Nominal
/// cleanup has no catalog here and remains in the later return-plan phase.
pub(crate) fn build_checked_primitive_store_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter_map(|machine| build_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            machine
                .attachment_type_identity
                .as_deref()
                .into_iter()
                .chain(
                    machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
        ..Default::default()
    }
}

/// Refresh independent callees without erasing the retained nominal/selected
/// return plans used by selected Unit execution during its rebuild.
pub(crate) fn refresh_checked_primitive_store_scalar_return_plans(
    program: &TypedTrees,
    facts: &mut CheckFacts,
) {
    let mut primitive_returns = build_checked_primitive_store_scalar_return_plans(program, facts);
    let returns = &mut facts.flow.terminal_structural_scalar_returns;
    returns.machines.retain_mut(|plan| {
        if let Some(position) = primitive_returns
            .machines
            .iter()
            .position(|replacement| replacement.machine == plan.machine)
        {
            *plan = primitive_returns.machines.remove(position);
            true
        } else {
            plan.effects.is_empty() && !is_primitive_reference_plan(plan)
        }
    });
    returns.machines.extend(primitive_returns.machines);
    for shape in primitive_returns.structural_types {
        returns
            .structural_types
            .retain(|existing| existing.identity != shape.identity);
        returns.structural_types.push(shape);
    }
    returns
        .structural_types
        .sort_by(|left, right| left.identity.cmp(&right.identity));
}

pub(in crate::flow::terminal_unit) fn build_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedStructuralScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        || !matches!(
            program.statement_table.statements(state.statement_nodes),
            [StatementNode::Expression(_)]
                | [StatementNode::Assignment(_), StatementNode::Expression(_)]
        )
        || !has_plain_primitive_borrows(program, state)
    {
        return None;
    }
    let mut diagnostics = Vec::new();
    let plan = build_structural_scalar_return_machine(
        program,
        facts,
        None,
        shapes,
        machine,
        &mut diagnostics,
    )?;
    (diagnostics.is_empty()
        && is_primitive_reference_plan(&plan)
        && plan.bindings.is_empty()
        && plan.cleanup_actions.is_empty()
        && plan.caller_requirements.is_empty()
        && plan.scalar_requirements.is_empty())
    .then_some(plan)
}

/// This independent cohort carries unrestricted primitive borrows, never the
/// affine parameters whose return plans depend on nominal cleanup discovery.
pub(super) fn is_primitive_reference_plan(plan: &CheckedStructuralScalarReturnMachinePlan) -> bool {
    !plan.structural_parameters.is_empty()
        && plan.structural_parameters.iter().all(|parameter| {
            !parameter.is_self
                && parameter.multiplicity == Multiplicity::Unrestricted
                && parameter.access != CheckedStructuralAccess::Owned
                && parameter.qualifications.is_empty()
        })
}

pub(in crate::flow::terminal_unit) fn has_plain_primitive_borrows(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    let mut has_borrow = false;
    program.state_parameters(state).iter().all(|parameter| {
        if parameter.is_self || parameter.is_const {
            return false;
        }
        let reference = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                has_borrow = true;
                *referee
            }
            _ => parameter.type_reference,
        };
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(reference).is_some()
    }) && has_borrow
}

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
    if matches!(statements, [StatementNode::Expression(_)]) {
        return has_plain_primitive_borrows(program, state).then(Vec::new);
    }
    let [StatementNode::Assignment(_), StatementNode::Expression(_)] = statements else {
        return None;
    };
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

#[cfg(test)]
mod tests;
