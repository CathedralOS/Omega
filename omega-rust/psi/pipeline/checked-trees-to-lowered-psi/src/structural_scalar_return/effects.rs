//! Rejoin primitive-reference returns with their complete authored statement roster.

use super::*;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<bool, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    if machine.symbol != plan.machine {
        return unsupported("scalar return effects have a different authored machine");
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    let parameters = checked.state_parameters(state);
    let primitive_reference_body = parameters.iter().any(|parameter| {
        matches!(checked.type_reference_table.type_reference(parameter.type_reference),
            checked_trees::types::TypeReferenceNode::Reference { referee, .. }
                if checked.primitive_type_reference(*referee).is_some())
    }) || plan.structural_parameters.iter().any(|parameter| {
        parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.access != checked_trees::CheckedStructuralAccess::Owned
    });
    if plan.effects.is_empty()
        && !primitive_reference_body
        && !statements
            .iter()
            .any(|statement| matches!(statement, StatementNode::Assignment(_)))
    {
        return Ok(false);
    }
    let (returned, store) = match (statements, plan.effects.as_slice()) {
        ([StatementNode::Expression(returned)], []) => (returned, None),
        (
            [
                StatementNode::Assignment(_),
                StatementNode::Expression(returned),
            ],
            [
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index: 0,
                    destination:
                        checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index: 0,
                        },
                    value,
                },
            ],
        ) => (returned, Some(value)),
        _ => {
            return unsupported(
                "scalar return effect roster omits or duplicates its authored primitive store",
            );
        }
    };
    if plan.return_statement_ordinal != u32::from(store.is_some())
        || !plan.bindings.is_empty()
        || !plan.cleanup_actions.is_empty()
        || plan.shared_boolean_convergence.is_some()
    {
        return unsupported("scalar return effects have stale statement or cleanup coordinates");
    }
    let mut flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .filter(|flow| flow.machine_symbol == plan.machine && flow.state_symbol == plan.state);
    let flow = flows.next().ok_or(LoweringError::Unsupported(
        "primitive reference return lost its checked flow state",
    ))?;
    if flows.next().is_some()
        || checked.machine_states(machine).len() != 1
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !checked
            .facts
            .flow
            .control
            .calls
            .span(flow.calls)
            .ok_or(LoweringError::Unsupported(
                "primitive reference return has stale call custody",
            ))?
            .is_empty()
    {
        return unsupported("primitive reference return must retain one call-free checked body");
    }
    if checked
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.type_reference)
        .chain(std::iter::once(state.return_type))
        .any(|reference| {
            let node = checked.type_reference_table.type_reference(reference);
            let node = match node {
                checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                    checked.type_reference_table.type_reference(*referee)
                }
                _ => node,
            };
            matches!(
                node,
                checked_trees::types::TypeReferenceNode::Constrained { .. }
            )
        })
    {
        return unsupported("primitive store return has unsupported constrained types");
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "primitive store return has no checked contract",
        ))?;
    if !checked.machine_contracts(machine).is_empty()
        || !checked.state_contracts(state).is_empty()
        || !contract.crash.published().is_empty()
        || !plan.caller_requirements.is_empty()
        || !plan.scalar_requirements.is_empty()
        || checked
            .facts
            .proof
            .contract_facts
            .iter()
            .any(|(_, fact)| match fact.owner {
                checked_trees::ContractProofFactOwner::Machine { machine_symbol } => {
                    machine_symbol == plan.machine
                }
                checked_trees::ContractProofFactOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                } => machine_symbol == plan.machine && state_symbol == plan.state,
                _ => false,
            })
    {
        return unsupported("primitive store return has unsupported authored or checked contracts");
    }
    if machine.attached_data.is_none() != plan.attachment_type_identity.is_none() {
        return unsupported("primitive store return attachment differs from its authored owner");
    }
    if let Some(identity) = &plan.attachment_type_identity {
        let attachment = checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == machine.attached_data_symbol)
            .ok_or(LoweringError::Unsupported(
                "primitive store return has no resolved attachment",
            ))?;
        if !checked.data_type_parameters(attachment).is_empty() {
            return unsupported(
                "primitive store return attachment requires generic identity custody",
            );
        }
        let mut authored_identity = String::from("named(name(");
        for character in checked
            .symbols
            .display_path(attachment.symbol, "::")
            .chars()
        {
            if matches!(character, '\\' | '(' | ')' | ',') {
                authored_identity.push('\\');
            }
            authored_identity.push(character);
        }
        authored_identity.push_str("))");
        if identity != &authored_identity {
            return unsupported(
                "primitive store return attachment differs from its resolved declaration",
            );
        }
    }
    let mut structural = plan.structural_parameters.iter();
    let mut scalar = plan.scalar_parameters.iter();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_self || parameter.is_const {
            return unsupported("primitive reference return has unsupported parameter qualifiers");
        }
        if let checked_trees::types::TypeReferenceNode::Reference {
            access, referee, ..
        } = checked
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            let retained = structural.next().ok_or(LoweringError::Unsupported(
                "primitive reference return lost a structural parameter",
            ))?;
            let access = match access {
                language_semantics::ReferenceAccess::Shared => {
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                }
                language_semantics::ReferenceAccess::Mutable => {
                    checked_trees::CheckedStructuralAccess::MutableBorrow
                }
                language_semantics::ReferenceAccess::WriteOnly => {
                    checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                }
            };
            if !matches!(
                checked.type_reference_table.type_reference(*referee),
                checked_trees::types::TypeReferenceNode::Named { .. }
            ) {
                return unsupported("primitive reference return needs a plain primitive referent");
            }
            let primitive =
                checked
                    .primitive_type_reference(*referee)
                    .ok_or(LoweringError::Unsupported(
                        "primitive reference return has no primitive referent",
                    ))?;
            let identity = checked
                .normalized_type_identity_with_binders(*referee, &[])
                .into_string();
            let shape = checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .structural_types
                .iter()
                .find(|shape| shape.identity == retained.type_identity)
                .ok_or(LoweringError::Unsupported(
                    "primitive reference return shape is absent",
                ))?;
            if retained.is_self
                || usize::try_from(retained.position).ok() != Some(position)
                || retained.access != access
                || retained.multiplicity != Multiplicity::Unrestricted
                || !retained.qualifications.is_empty()
                || retained.type_identity != identity
                || shape.shape
                    != checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)
            {
                return unsupported(
                    "primitive reference return signature lost its authored referent custody",
                );
            }
        } else {
            let retained = scalar.next().ok_or(LoweringError::Unsupported(
                "primitive reference return lost a scalar parameter",
            ))?;
            if parameter.is_mutable
                || usize::try_from(retained.source_position).ok() != Some(position)
                || checked.primitive_type_reference(parameter.type_reference)
                    != Some(retained.primitive_type)
            {
                return unsupported(
                    "primitive reference return scalar namespace differs from its authored signature",
                );
            }
        }
    }
    if plan.structural_parameters.is_empty()
        || structural.next().is_some()
        || scalar.next().is_some()
    {
        return unsupported("primitive reference return has an inexact parameter roster");
    }
    if let Some(value) = store {
        let [destination] = plan.structural_parameters.as_slice() else {
            return unsupported("primitive store return must retain its one borrowed destination");
        };
        if destination.position != 0
            || destination.access == checked_trees::CheckedStructuralAccess::SharedBorrow
            || !parameters[0].is_mutable
        {
            return unsupported(
                "primitive store return signature lost its authored exclusive custody",
            );
        }
        crate::primitive_store::validate_assignment(checked, plan.state, 0, destination, value)?;
    }
    let expressions = &checked.facts.values.scalar_expressions;
    let (binding, _) = expressions
        .bound_expression_at(
            plan.state,
            plan.return_statement_ordinal,
            CheckedScalarExpressionRole::Return,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive store return lost its unique result binding",
        ))?;
    if binding.expression != *returned
        || checked.primitive_type_reference(state.return_type) != Some(plan.result_type)
    {
        return unsupported("primitive store return result differs from its authored expression");
    }
    crate::scalar_source_custody::validate_namespace(checked, binding)?;
    Ok(true)
}

#[cfg(test)]
mod tests;

pub(super) fn emit(
    plan: &CheckedStructuralScalarReturnMachinePlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    for effect in &plan.effects {
        let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            destination:
                checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index },
            value,
            ..
        } = effect
        else {
            return unsupported("scalar return acquired an unsupported prefix effect");
        };
        let kind = crate::primitive_store::emit(
            *parameter_index,
            value,
            parameters,
            structural_types,
            plan.scalar_parameters.len(),
            scalar_values,
            next_value,
            operations,
        )?;
        let operation = operations.allocate();
        operations.push(Operation {
            id: operation,
            result: terminal_psi::OperationResult::Unit,
            kind,
        });
    }
    Ok(())
}
