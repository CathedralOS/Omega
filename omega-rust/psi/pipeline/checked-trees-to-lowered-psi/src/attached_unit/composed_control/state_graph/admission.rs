//! Rejoin every retained state, call, and edge before admitting the graph.

use super::*;
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use checked_trees::types::TypeReferenceNode;

/// Custody, not topology, selects this emitter. Once selected, failed source
/// rejoining must not retry an older matcher that ignores additional statements.
pub(in crate::attached_unit::composed_control) fn has_shared_graph_custody(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> bool {
    let Some(machine) = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
    else {
        return false;
    };
    machine.lifetime_parameters.is_empty()
        && checked.machine_type_parameters(machine).is_empty()
        && checked.machine_states(machine).iter().all(|state| {
            checked
                .state_parameters(state)
                .iter()
                .all(|parameter| !parameter.is_self)
        })
        && plan.body_qualifications.is_empty()
        && plan.provider_attachment_requirements.is_empty()
        && plan.states.iter().all(|state| {
            state.entry_claims.is_empty()
                && state.structural_parameters.iter().all(|parameter| {
                    parameter.multiplicity == Multiplicity::Unrestricted
                        && parameter.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                        && parameter.qualifications.is_empty()
                })
                && !matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
        })
}

pub(in crate::attached_unit) struct AdmittedGraph<'a> {
    pub(in crate::attached_unit::composed_control) boundaries:
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
    pub(in crate::attached_unit::composed_control) internal_targets:
        Vec<(crate::attached_unit::bodies::UnitBody<'a>, String)>,
    /// State-local view positions alias these immutable invocation parameters.
    pub(super) view_roots: Vec<Vec<usize>>,
}

pub(in crate::attached_unit::composed_control) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedGraph<'a>, LoweringError> {
    super::super::admission::validate_contract(checked, plan)?;
    if plan.states.len() < 2
        || !plan.body_qualifications.is_empty()
        || !plan.provider_attachment_requirements.is_empty()
        || checked
            .facts
            .qualifications
            .for_machine(plan.machine)
            .is_some_and(|fact| !fact.body_committed.is_empty())
    {
        return unsupported("Unit graph has unsupported qualification or provider custody");
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "Unit graph has no authored machine",
        ))?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.lifetime_parameters.is_empty()
        || !checked.machine_type_parameters(machine).is_empty()
    {
        return unsupported("Unit graph requires a closed checked body");
    }
    super::super::admission::exact_attachment(checked, plan)?;
    let source_states = checked.machine_states(machine);
    if source_states.len() != plan.states.len() {
        return unsupported("Unit graph state roster drifted");
    }
    if source_states
        .iter()
        .zip(&plan.states)
        .any(|(source, state)| source.symbol != state.state)
    {
        return unsupported("Unit graph state identity, contract, or custody drifted");
    }
    for (source, state) in source_states.iter().zip(&plan.states) {
        if source.symbol != state.state
            || !checked.state_contracts(source).is_empty()
            || !matches!(
                checked
                    .type_reference_table
                    .type_reference(source.return_type),
                TypeReferenceNode::Unit
            )
            || !state.entry_claims.is_empty()
        {
            return unsupported("Unit graph state identity, contract, or custody drifted");
        }
        if crate::attached_unit::parameters::checked_scalar_source_parameters(checked, source)?
            != state.scalar_parameters
        {
            return unsupported("Unit graph scalar signature disagrees with source");
        }
        let source_parameters = checked.state_parameters(source);
        if state.structural_parameters.len() + state.scalar_parameters.len()
            != source_parameters.len()
            || state
                .structural_parameters
                .windows(2)
                .any(|pair| pair[0].position >= pair[1].position)
        {
            return unsupported("Unit graph signature dropped or duplicated a parameter");
        }
        for parameter in &state.structural_parameters {
            let source = source_parameters.get(parameter.position as usize).ok_or(
                LoweringError::Unsupported("Unit graph view parameter position is invalid"),
            )?;
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = checked
                .type_reference_table
                .type_reference(source.type_reference)
            else {
                return unsupported("Unit graph structural parameter is not a borrowed view");
            };
            let TypeReferenceNode::Slice { element_type } =
                checked.type_reference_table.type_reference(*referee)
            else {
                return unsupported("Unit graph borrowed parameter is not a byte slice");
            };
            if *access != language_core::ReferenceAccess::Shared
                || checked.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
                || source.is_self
                || source.is_const
                || source.is_mutable
                || parameter.is_self
                || parameter.multiplicity != Multiplicity::Unrestricted
                || parameter.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || checked.normalized_type_identity(*referee).as_str() != parameter.type_identity
            {
                return unsupported("Unit graph view type or authority disagrees with source");
            }
            let mut shapes = checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .filter(|shape| shape.identity == parameter.type_identity);
            if !matches!(
                shapes.next().map(|shape| &shape.shape),
                Some(checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView
                ))
            ) || shapes.next().is_some()
            {
                return unsupported("Unit graph view has no exact byte descriptor shape");
            }
        }
        let statements = checked.statement_table.statements(source.statement_nodes);
        scalars::validate(checked, state)?;
        let prefix_count = statements
            .iter()
            .take_while(|statement| {
                matches!(
                    statement,
                    StatementNode::LocalData(_) | StatementNode::Assignment(_)
                )
            })
            .count();
        if prefix_count != state.bindings.len() {
            return unsupported("Unit graph scalar prefix dropped or added a binding");
        }
        let call_count = statements
            .iter()
            .enumerate()
            .skip(prefix_count)
            .take_while(|(ordinal, statement)| match statement {
                StatementNode::Call(_) => true,
                StatementNode::Expression(expression) if *ordinal + 1 == statements.len() => {
                    checked.expression_table.expression_is_valid(*expression)
                        && matches!(
                            checked.expression_table.expression(*expression),
                            checked_trees::expression::ExpressionNode::Call(_)
                        )
                }
                _ => false,
            })
            .count();
        let terminator_ordinal = prefix_count + call_count;
        if state.operations.len() != call_count {
            return unsupported("Unit graph dropped or added a body effect");
        }
        for (ordinal, operation) in state.operations.iter().enumerate() {
            let coordinate = match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    structural_arguments,
                    completion_receipts,
                    ..
                } if completion_receipts.is_empty()
                    && structural_arguments.iter().all(|argument| {
                        argument.source_parameter_index().is_some()
                            && argument.path.is_empty()
                            && argument.access
                                == checked_trees::CheckedStructuralAccess::SharedBorrow
                    }) =>
                {
                    coordinate
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    structural_arguments,
                    claim_transfers,
                    ..
                } if structural_arguments.is_empty() && claim_transfers.is_empty() => coordinate,
                _ => {
                    return unsupported(
                        "Unit graph operation requires additional value or ownership lowering",
                    );
                }
            };
            if coordinate.statement_index as usize != prefix_count + ordinal
                || coordinate.call_ordinal != 0
            {
                return unsupported("Unit graph reordered a source effect");
            }
        }
        let tail = &statements[terminator_ordinal..];
        match (&state.terminator, tail) {
            (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, []) => {}
            (
                CheckedComposedUnitControlTerminatorPlan::Jump { successor },
                [StatementNode::Transition(transition)],
            ) if transition.guard == TransitionGuardNode::Always => {
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    transition,
                    successor,
                    terminator_ordinal,
                )?;
            }
            (
                CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    when_true,
                    when_false,
                },
                [
                    StatementNode::Transition(true_source),
                    StatementNode::Transition(false_source),
                ],
            ) if matches!(true_source.guard, TransitionGuardNode::When(_)) => {
                edges::validate_fallback(checked, true_source, false_source)?;
                if checked.facts.values.scalar_expressions.expression_at(
                    state.state,
                    u32::try_from(terminator_ordinal)
                        .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?,
                    CheckedScalarExpressionRole::Guard,
                ) != Some(guard)
                {
                    return unsupported("Unit graph guard disagrees with checked expression");
                }
                let (binding, _) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        state.state,
                        when_true.statement_ordinal,
                        CheckedScalarExpressionRole::Guard,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph guard has no exact source binding",
                    ))?;
                crate::scalar_source_custody::validate_pure(checked, binding, ScalarType::Boolean)?;
                if !matches!(guard, CheckedScalarExpression::Boolean(_)) {
                    return unsupported("Unit graph guard is not Boolean");
                }
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    true_source,
                    when_true,
                    terminator_ordinal,
                )?;
                edges::validate(
                    checked,
                    plan,
                    source,
                    state,
                    false_source,
                    when_false,
                    terminator_ordinal + 1,
                )?;
            }
            _ => return unsupported("Unit graph terminator disagrees with authored state"),
        }
    }
    let view_roots = view_roots(plan)?;
    let states = plan.states.iter().collect::<Vec<_>>();
    let (boundaries, internal_targets) =
        super::super::admission::retain_call_targets(checked, plan.machine, &states)?;
    for (boundary, _) in &boundaries {
        if boundary.attachment_type_identity.is_some()
            || !boundary.domain_requirements.is_empty()
            || !boundary.result.is_unit()
        {
            return unsupported(
                "Unit graph boundary requires additional provider or result custody",
            );
        }
    }
    Ok(AdmittedGraph {
        boundaries,
        internal_targets,
        view_roots,
    })
}

fn view_roots(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<Vec<Vec<usize>>, LoweringError> {
    let mut incoming = vec![0_usize; plan.states.len()];
    for state in &plan.states {
        for successor in successors(state) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph edge has no target"))?;
            incoming[target] += 1;
        }
    }
    if incoming[0] != 0 || incoming[1..].contains(&0) {
        return unsupported("Unit graph is cyclic or has unreachable states");
    }
    let mut roots = vec![None; plan.states.len()];
    roots[0] = Some((0..plan.states[0].structural_parameters.len()).collect::<Vec<_>>());
    let mut ready = vec![0];
    let mut next = 0;
    while let Some(source) = ready.get(next).copied() {
        next += 1;
        for successor in successors(&plan.states[source]) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))?;
            let source_roots = roots[source]
                .as_ref()
                .ok_or(LoweringError::Unsupported("Unit graph view roots missing"))?;
            let aliases = successor
                .transfers
                .iter()
                .map(|transfer| {
                    source_roots
                        .get(transfer.source_parameter_index as usize)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph view transfer source missing",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if roots[target]
                .as_ref()
                .is_some_and(|previous| previous != &aliases)
            {
                return unsupported("Unit graph join needs structural descriptor rebinding");
            }
            roots[target] = Some(aliases);
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push(target);
            }
        }
    }
    if next != plan.states.len() {
        return unsupported("Unit graph cyclic safety is not retained");
    }
    roots
        .into_iter()
        .map(|roots| {
            roots.ok_or(LoweringError::Unsupported(
                "Unit graph state is unreachable",
            ))
        })
        .collect()
}
