//! Structural completion rejoins the exact returned owner, not an array-only
//! or fresh-constructor route. Producers validate construction/call/selection
//! custody; this boundary checks the authored return and complete statement
//! coverage before an ordinary result can leave its enclosing machine.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{CheckedArrayConstructionSource, CheckedStructuralAccess};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let source = checked
        .typed
        .machines()
        .iter()
        .find(|source| source.symbol == machine.machine)
        .ok_or(LoweringError::Unsupported(
            "structural result machine is absent",
        ))?;
    let [state] = checked.typed.machine_states(source) else {
        return unsupported("structural result state roster changed");
    };
    let Some(result) = &machine.structural_result else {
        let mut reference = state.return_type;
        while let checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            checked.typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        if !matches!(
            checked.typed.type_reference_table.type_reference(reference),
            checked_trees::types::TypeReferenceNode::Unit
        ) {
            return unsupported("Unit completion erases the authored result type");
        }
        return Ok(());
    };
    if state.symbol != machine.state
        || !source.body_is_present
        || result.multiplicity
            != validation::reference_result_custody::result_multiplicity(
                &checked.typed,
                state.return_type,
            )
        || !(validation::reference_result_custody::parts(&checked.typed, state.return_type)
            .is_some()
            || validation::is_closed_primitive_array_type(&checked.typed, state.return_type)
            || validation::has_plain_owned_contents_with_numeric_constraints(
                &checked.typed,
                state.return_type,
            ))
        || checked
            .typed
            .normalized_type_identity(state.return_type)
            .as_str()
            != result.type_identity
    {
        return unsupported("structural result signature changed");
    }
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    if !matches!(machine.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete { statement_index, .. }) if *statement_index as usize == statements.len())
    {
        return unsupported("structural completion coordinate differs from source");
    }
    let Some(StatementNode::Expression(expression)) = statements.last() else {
        return unsupported("structural result source has no completion value");
    };
    if !result.reference_sources.is_empty() {
        let mut establishments = machine.operations.iter().filter(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishReference { .. }
            )
        });
        let establishment = establishments.next().ok_or(LoweringError::Unsupported(
            "returned reference has no establishment",
        ))?;
        if establishments.next().is_some() {
            return unsupported("returned reference has ambiguous establishments");
        }
        super::reference_results::validate_establishment(checked, machine, establishment)?;
    } else if validation::reference_result_custody::parts(&checked.typed, state.return_type)
        .is_some()
    {
        return unsupported("reference result omits its retained ingress source");
    }
    match result.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            if !result.reference_sources.is_empty() {
                // The exact authored ingress, result binding and tail coordinate
                // were checked above; ordinary statement coverage still follows.
            } else {
                let mut producers =
                    machine
                        .operations
                        .iter()
                        .filter_map(|operation| match operation {
                            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                result: candidate,
                                discard_result_on_return: false,
                                ..
                            } if candidate.binding_ordinal == binding_ordinal => {
                                Some((candidate, true))
                            }
                            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                                source,
                                result: candidate,
                                ..
                            } if candidate.binding_ordinal == binding_ordinal => Some((
                                candidate,
                                *source == CheckedArrayConstructionSource::Statement,
                            )),
                            CheckedUnitEffectOperationPlan::StructuralCall {
                                coordinate,
                                result: candidate,
                                ..
                            } if candidate.binding_ordinal == binding_ordinal => {
                                Some((candidate, coordinate.call_ordinal == 0))
                            }
                            _ => None,
                        });
                let (binding, owns_statement_result) = producers.next().ok_or(
                    LoweringError::Unsupported("returned structural value has no exact producer"),
                )?;
                if producers.next().is_some()
                    || !owns_statement_result
                    || binding.type_identity != result.type_identity
                    || binding.multiplicity != result.multiplicity
                {
                    return unsupported("returned structural producer is ambiguous or changed");
                }
                if let ExpressionNode::Name(path) =
                    checked.typed.expression_table.expression(*expression)
                {
                    let Some(StatementNode::LocalData(local)) =
                        statements.get(binding.statement_index as usize)
                    else {
                        return unsupported("returned structural binding has no declaration");
                    };
                    if path.symbol != local.symbol
                        || path.head_symbol != local.symbol
                        || checked
                            .typed
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            != 1
                    {
                        return unsupported("returned structural binding differs from source");
                    }
                } else if binding.statement_index as usize + 1 != statements.len() {
                    return unsupported("returned constructor differs from source");
                }
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = machine
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "returned structural parameter is absent",
                ))?;
            let source_parameter = checked
                .typed
                .state_parameters(state)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "returned structural source parameter is absent",
                ))?;
            if parameter.access != CheckedStructuralAccess::Owned
                || source_parameter.is_const
                || source_parameter.is_mutable
                || source_parameter.is_self
                || !matches!(
                    parameter.multiplicity,
                    Multiplicity::Unrestricted | Multiplicity::Affine
                )
                || parameter.multiplicity != result.multiplicity
                || !parameter.qualifications.is_empty()
                || parameter.is_self
                || parameter.fused_service_erasure.is_some()
                || parameter.type_identity != result.type_identity
                || checked
                    .typed
                    .normalized_type_identity(source_parameter.type_reference)
                    .as_str()
                    != result.type_identity
                || !matches!(checked.typed.expression_table.expression(*expression),
                    ExpressionNode::Name(path) if path.symbol == source_parameter.symbol
                        && path.head_symbol == source_parameter.symbol
                        && checked.typed.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("returned structural parameter differs from its owned source");
            }
            if validation::is_closed_primitive_array_type(
                &checked.typed,
                source_parameter.type_reference,
            ) {
                super::scalar_arrays::validate_shape(checked, source_parameter.type_reference)?;
            } else {
                // Whole-value return does not inspect a record's fields or
                // select an array element. Rejoin the declared owned carrier
                // and exact transfer using the shared place validator; a
                // Named-only record test would exclude fixed arrays and closed
                // generic applications with the same ownership obligations.
                super::structural_values::source_custody::validate_owned_place(
                    checked,
                    machine.machine,
                    machine.state,
                    u32::try_from(statements.len() - 1).map_err(|_| {
                        LoweringError::Unsupported("return statement ordinal exceeds u32")
                    })?,
                    *expression,
                    state.return_type,
                    &checked_trees::CheckedUnitStructuralArgumentPlan {
                        source: result.source.clone(),
                        path: Vec::new(),
                        type_identity: result.type_identity.clone(),
                        access: CheckedStructuralAccess::Owned,
                    },
                )?;
            }
        }
        _ => return unsupported("structural return source is not an owned whole value"),
    }
    // Every authored statement owes an operation, except a final binding use.
    for index in 0..statements.len() {
        if index + 1 == statements.len()
            && matches!(
                checked.typed.expression_table.expression(*expression),
                ExpressionNode::Name(_)
            )
        {
            continue;
        }
        if !machine.operations.iter().any(|operation| {
            super::scalar_arrays::source_statement(operation) == Some(index as u32)
        }) {
            return unsupported("structural result body omits an authored statement");
        }
    }
    Ok(())
}
