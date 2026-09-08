//! Rejoin write-then-return effects with their complete authored statement roster.

use super::*;
use checked_trees::{expression::ExpressionNode, statement::StatementNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<bool, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    if machine.symbol != plan.machine {
        return unsupported("scalar return effects have a different authored machine");
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    if plan.effects.is_empty()
        && !statements
            .iter()
            .any(|statement| matches!(statement, StatementNode::Assignment(_)))
    {
        return Ok(false);
    }
    let [
        StatementNode::Assignment(assignment),
        StatementNode::Expression(returned),
    ] = statements
    else {
        return unsupported("scalar return effects do not cover the authored statement sequence");
    };
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination_parameter_index,
            value,
        },
    ] = plan.effects.as_slice()
    else {
        return unsupported("scalar return effect roster omits or duplicates its primitive store");
    };
    if *statement_index != 0
        || *destination_parameter_index != 0
        || plan.return_statement_ordinal != 1
        || !plan.bindings.is_empty()
        || !plan.cleanup_actions.is_empty()
        || plan.shared_boolean_convergence.is_some()
    {
        return unsupported("scalar return effects have stale statement or cleanup coordinates");
    }
    let [destination] = plan.structural_parameters.as_slice() else {
        return unsupported("primitive store return must retain its one borrowed destination");
    };
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
    let parameters = checked.state_parameters(state);
    let parameter = parameters.first().ok_or(LoweringError::Unsupported(
        "primitive store return has no authored destination",
    ))?;
    let checked_trees::types::TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return unsupported("primitive store return destination is not an authored reference");
    };
    let access = match access {
        language_semantics::ReferenceAccess::Mutable => {
            checked_trees::CheckedStructuralAccess::MutableBorrow
        }
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
        }
        language_semantics::ReferenceAccess::Shared => {
            return unsupported("primitive store return cannot write a shared borrow");
        }
    };
    if parameter.is_self
        || parameter.is_const
        || !parameter.is_mutable
        || destination.is_self
        || destination.position != 0
        || destination.access != access
        || destination.multiplicity != Multiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || parameters.len() != plan.scalar_parameters.len() + 1
    {
        return unsupported("primitive store return signature lost its authored exclusive custody");
    }
    let primitive =
        checked
            .primitive_type_reference(*referee)
            .ok_or(LoweringError::Unsupported(
                "primitive store return destination has no primitive referent",
            ))?;
    let shape = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types
        .iter()
        .find(|shape| shape.identity == destination.type_identity)
        .ok_or(LoweringError::Unsupported(
            "primitive store return destination shape is absent",
        ))?;
    if shape.shape != checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) {
        return unsupported(
            "primitive store return destination shape differs from its authored type",
        );
    }
    for (dense_position, scalar) in plan.scalar_parameters.iter().enumerate() {
        let authored_position = dense_position + 1;
        let parameter = parameters
            .get(authored_position)
            .ok_or(LoweringError::Unsupported(
                "primitive store return scalar parameter is absent",
            ))?;
        if usize::try_from(scalar.source_position).ok() != Some(authored_position)
            || parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || checked.primitive_type_reference(parameter.type_reference)
                != Some(scalar.primitive_type)
        {
            return unsupported(
                "primitive store return scalar namespace differs from its authored signature",
            );
        }
    }
    let ExpressionNode::Name(target) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("primitive store return destination is not a direct parameter");
    };
    if target.symbol != parameter.symbol || target.head_symbol != parameter.symbol {
        return unsupported(
            "primitive store return destination differs from its authored parameter",
        );
    }
    let expressions = &checked.facts.values.scalar_expressions;
    let (binding, expression) = expressions
        .bound_expression_at(plan.state, 0, CheckedScalarExpressionRole::AssignmentValue)
        .ok_or(LoweringError::Unsupported(
            "primitive store return lost its unique RHS binding",
        ))?;
    if binding.expression != assignment.value
        || binding.destination != parameter.symbol
        || expression != value
    {
        return unsupported("primitive store return RHS differs from its authored assignment");
    }
    crate::scalar_source_custody::validate_namespace(checked, binding)?;
    let (binding, _) = expressions
        .bound_expression_at(plan.state, 1, CheckedScalarExpressionRole::Return)
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
            destination_parameter_index,
            value,
            ..
        } = effect
        else {
            return unsupported("scalar return acquired an unsupported prefix effect");
        };
        let kind = crate::primitive_store::emit(
            *destination_parameter_index,
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
