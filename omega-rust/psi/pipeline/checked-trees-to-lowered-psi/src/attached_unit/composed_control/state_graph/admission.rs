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
        // Legacy compile-known receiver observations can erase their runtime
        // receiver. Their existing emitter must still validate that erasure.
        && checked.machine_states(machine).iter().all(|source| {
            !checked.state_parameters(source).iter().any(|parameter| parameter.is_self)
                || plan.states.iter().find(|state| state.state == source.symbol)
                    .is_some_and(|state| state.structural_parameters.iter().any(|parameter| parameter.is_self))
        })
        && plan.body_qualifications.is_empty()
        && plan.states.iter().all(|state| {
            state.entry_claims.is_empty()
                && state.structural_parameters.iter().all(|parameter| {
                    parameter.multiplicity == Multiplicity::Unrestricted
                        && matches!(
                            parameter.access,
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        )
                        && parameter.qualifications.is_empty()
                })
        })
}

pub(in crate::attached_unit) struct AdmittedGraph<'a> {
    pub(in crate::attached_unit::composed_control) boundaries:
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
    pub(in crate::attached_unit::composed_control) internal_targets:
        Vec<(crate::attached_unit::bodies::UnitBody<'a>, String)>,
}

pub(in crate::attached_unit::composed_control) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedGraph<'a>, LoweringError> {
    super::super::admission::validate_contract(checked, plan)?;
    if plan.states.is_empty()
        || (plan.states.len() < 2 && plan.result == checked_trees::CheckedControlResultPlan::Unit)
        || !plan.body_qualifications.is_empty()
        || checked
            .facts
            .qualifications
            .for_machine(plan.machine)
            .is_some_and(|fact| {
                fact.body_committed.iter().any(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
            })
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
    super::ranking::validate_witness(checked, machine, plan)?;
    let attachment = super::super::admission::exact_attachment(checked, plan)?;
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
            || !super::returns::signature_matches(checked, source, &plan.result)
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
            if parameter.is_self {
                super::parameters::validate_receiver(checked, plan, source, parameter, *access)?;
                continue;
            }
            let TypeReferenceNode::Slice { element_type } =
                checked.type_reference_table.type_reference(*referee)
            else {
                return unsupported("Unit graph borrowed parameter is not a byte slice");
            };
            if !matches!(
                *access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) || checked.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
                || source.is_self
                || source.is_const
                || source.is_mutable != (*access == language_core::ReferenceAccess::Mutable)
                || parameter.is_self
                || parameter.multiplicity != Multiplicity::Unrestricted
                || parameter.access
                    != match *access {
                        language_core::ReferenceAccess::Mutable => {
                            checked_trees::CheckedStructuralAccess::MutableBorrow
                        }
                        _ => checked_trees::CheckedStructuralAccess::SharedBorrow,
                    }
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
        let terminator_ordinal = super::body::validate(checked, plan.machine, source, state)?;
        let tail = &statements[terminator_ordinal..];
        match (&state.terminator, tail) {
            (CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }, _) => {
                super::cases::validate(checked, plan, source, state, tail, terminator_ordinal)?;
            }
            (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, [])
                if plan.result == checked_trees::CheckedControlResultPlan::Unit => {}
            (
                CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. },
                [StatementNode::Expression(_)],
            ) => {
                super::returns::validate(checked, plan, source, state, terminator_ordinal)?;
            }
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
    topology::validate(plan)?;
    let states = plan.states.iter().collect::<Vec<_>>();
    let (boundaries, internal_targets) =
        super::super::admission::retain_call_targets(checked, plan.machine, &states)?;
    if let Some(attachment) = attachment {
        for state in &plan.states {
            for operation in &state.operations {
                crate::attached_unit::provider_attachments::validate_call_source(
                    checked,
                    plan.machine,
                    state.state,
                    operation,
                    &plan.provider_attachment_requirements,
                )?;
            }
        }
        let called = boundaries
            .iter()
            .map(|(boundary, _)| boundary.machine)
            .collect::<Vec<_>>();
        crate::attached_unit::provider_attachments::validate_provider_attachment_requirements(
            attachment,
            &plan.provider_attachment_requirements,
            &called,
        )?;
    } else if !plan.provider_attachment_requirements.is_empty() {
        return unsupported("free Unit graph cannot retain provider attachment requirements");
    }
    for (boundary, _) in &boundaries {
        if boundary.attachment_type_identity.is_some()
            || !boundary.domain_requirements.is_empty()
            || !(boundary.result.is_unit()
                || matches!(&boundary.result,
                CheckedBoundaryMachineResultPlan::Structural { multiplicity: Multiplicity::Affine, qualifications, .. }
                if qualifications.is_empty()))
        {
            return unsupported(
                "Unit graph boundary requires additional provider or result custody",
            );
        }
    }
    Ok(AdmittedGraph {
        boundaries,
        internal_targets,
    })
}
