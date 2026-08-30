use psi_checked_trees::{
    CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationDemand,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedValueOrigin,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::TypeReferenceHandle;

use super::{expression_type_reference_for_origin, indexed_operand_types};

pub(crate) fn bind_boundary_operator_application_demands(
    program: &TypedTrees,
    operators: &mut CheckedOperatorFacts,
) {
    let mut applications = operators
        .uses
        .iter()
        .filter_map(|(_, operator_use)| {
            if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
                return None;
            }
            psi_typed_trees::operator::declaration_by_symbol(
                program,
                operator_use.selected_operator_symbol,
            )
            .filter(|operator| operator.is_boundary)
            .and_then(|operator| {
                checked_boundary_application(
                    program,
                    operator,
                    operator_use.expression,
                    operator_use.origin,
                    &spelled_operand_types(program, operator_use),
                )
            })
        })
        .collect::<Vec<_>>();

    // Named generic operator arguments are retained syntactically today but
    // are not bound to the operator telescope by call validation. Only their
    // truthful monomorphic empty application can enter this checked cohort.
    applications.extend(operators.named_uses.iter().filter_map(|(_, operator_use)| {
        psi_typed_trees::operator::declaration_by_symbol(
            program,
            operator_use.selected_operator_symbol,
        )
        .filter(|operator| {
            operator.is_boundary
                && operator.lifetime_parameters.is_empty()
                && program.operator_type_parameters(operator).is_empty()
        })
        .and_then(|operator| {
            checked_boundary_application(
                program,
                operator,
                operator_use.expression,
                operator_use.origin,
                &[],
            )
        })
    }));
    operators.boundary_applications = applications;
}

fn checked_boundary_application(
    program: &TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<CheckedBoundaryOperatorApplicationDemand> {
    let bindings = psi_typed_trees::operator::closed_operator_type_application_for_operands(
        program,
        operator,
        operand_types,
    )?;
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, (binder_symbol, type_reference))| {
            Some(CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner: operator.symbol,
                binder_ordinal: u32::try_from(ordinal).ok()?,
                binder_symbol,
                type_reference,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CheckedBoundaryOperatorApplicationDemand {
        expression,
        origin,
        requirement_symbol: operator.symbol,
        arguments,
    })
}

fn spelled_operand_types(
    program: &TypedTrees,
    operator_use: &CheckedOperatorUseFact,
) -> Vec<Option<TypeReferenceHandle>> {
    match program.expression_table.expression(operator_use.expression) {
        ExpressionNode::Binary(binary) => vec![
            expression_type_reference_for_origin(program, binary.left, operator_use.origin),
            expression_type_reference_for_origin(program, binary.right, operator_use.origin),
        ],
        ExpressionNode::Indexed(indexed) => {
            indexed_operand_types(program, indexed, operator_use.origin)
        }
        _ => Vec::new(),
    }
}
