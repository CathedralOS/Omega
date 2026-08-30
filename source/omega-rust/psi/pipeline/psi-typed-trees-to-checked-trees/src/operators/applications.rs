use psi_checked_trees::{
    CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationDemand,
    CheckedBoundaryOperatorApplicationUseSite, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedValueOrigin,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::TypeReferenceHandle;

use super::{expression_type_reference_for_origin, indexed_operand_types};

pub(crate) fn bind_boundary_operator_application_demands(
    program: &TypedTrees,
    validated: &[psi_validation::ValidatedBoundaryOperatorApplication],
    operators: &mut CheckedOperatorFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = psi_validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    if symbol_diagnostics
        .iter()
        .any(psi_diagnostics::Diagnostic::is_error)
    {
        return Err(symbol_diagnostics);
    }
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
                checked_spelled_boundary_application(
                    program,
                    operator,
                    operator_use.expression,
                    operator_use.origin,
                    &spelled_operand_types(program, operator_use),
                )
            })
        })
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    for application in validated {
        let Some(operator) =
            psi_typed_trees::operator::declaration_by_symbol(program, application.requirement)
        else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application names no operator declaration",
            ));
            continue;
        };
        if !operator.is_boundary {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application names a non-boundary operator",
            ));
            continue;
        }
        match application.site {
            psi_validation::ValidatedBoundaryOperatorApplicationUseSite::Expression(expression) => {
                let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                else {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "validated named operator application no longer names a call expression",
                    ));
                    continue;
                };
                if psi_typed_trees::operator::resolve_named_expression_call(program, call)
                    .is_none_or(|selected| selected.symbol != application.requirement)
                {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "validated named operator application does not rejoin its selected requirement",
                    ));
                    continue;
                }
                let matching_uses = operators
                    .named_uses
                    .iter()
                    .filter_map(|(_, operator_use)| {
                        (operator_use.expression == expression
                            && operator_use.selected_operator_symbol == application.requirement)
                            .then_some(operator_use)
                    })
                    .collect::<Vec<_>>();
                if matching_uses.is_empty() {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "validated named operator application has no checked selected use",
                    ));
                    continue;
                }
                for operator_use in matching_uses {
                    if let Some(arguments) = rejoin_validated_arguments(
                        program,
                        &symbols,
                        operator,
                        &application.arguments,
                        &mut diagnostics,
                    ) {
                        applications.push(CheckedBoundaryOperatorApplicationDemand {
                            site: CheckedBoundaryOperatorApplicationUseSite::Expression {
                                expression,
                                origin: operator_use.origin,
                            },
                            requirement_symbol: operator.symbol,
                            arguments,
                        });
                    }
                }
            }
            psi_validation::ValidatedBoundaryOperatorApplicationUseSite::Statement(statement) => {
                let StatementNode::Call(call) = program.statement_table.statement(statement) else {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "validated named operator application no longer names a call statement",
                    ));
                    continue;
                };
                if call.target_symbol != application.requirement {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "validated statement operator application does not rejoin its selected requirement",
                    ));
                    continue;
                }
                if let Some(arguments) = rejoin_validated_arguments(
                    program,
                    &symbols,
                    operator,
                    &application.arguments,
                    &mut diagnostics,
                ) {
                    applications.push(CheckedBoundaryOperatorApplicationDemand {
                        site: CheckedBoundaryOperatorApplicationUseSite::Statement(statement),
                        requirement_symbol: operator.symbol,
                        arguments,
                    });
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    operators.boundary_applications = applications;
    Ok(())
}

fn checked_spelled_boundary_application(
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
    Some(checked_boundary_application_from_bindings(
        operator,
        CheckedBoundaryOperatorApplicationUseSite::Expression { expression, origin },
        bindings,
    ))
}

fn checked_boundary_application_from_bindings(
    operator: &psi_typed_trees::operator::OperatorDefinition,
    site: CheckedBoundaryOperatorApplicationUseSite,
    bindings: Vec<(psi_symbols::SymbolHandle, TypeReferenceHandle)>,
) -> CheckedBoundaryOperatorApplicationDemand {
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, (binder_symbol, type_reference))| {
            CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner: operator.symbol,
                binder_ordinal: u32::try_from(ordinal)
                    .expect("operator static telescope ordinal overflow"),
                binder_symbol,
                type_reference,
            }
        })
        .collect();
    CheckedBoundaryOperatorApplicationDemand {
        site,
        requirement_symbol: operator.symbol,
        arguments,
    }
}

fn rejoin_validated_arguments(
    program: &TypedTrees,
    symbols: &psi_validation::TopLevelSymbols<'_>,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    validated: &[psi_validation::ValidatedBoundaryOperatorApplicationArgument],
    diagnostics: &mut Vec<psi_diagnostics::Diagnostic>,
) -> Option<Vec<CheckedBoundaryOperatorApplicationArgument>> {
    let parameters = program.operator_type_parameters(operator);
    if !operator.lifetime_parameters.is_empty()
        || parameters.iter().any(|parameter| {
            !matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
            )
        })
        || validated.len() != parameters.len()
    {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "validated boundary application does not match the supported selected operator telescope",
        ));
        return None;
    }
    let mut arguments = Vec::with_capacity(validated.len());
    for (ordinal, (argument, parameter)) in validated.iter().zip(parameters).enumerate() {
        let psi_validation::ValidatedBoundaryOperatorApplicationArgument::Type {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            type_reference,
        } = argument;
        if *binder_owner != operator.symbol
            || usize::try_from(*binder_ordinal).ok() != Some(ordinal)
            || *binder_symbol != parameter.symbol
            || !matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
            )
            || !type_reference.is_valid()
        {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application does not rejoin the selected operator binder",
            ));
            return None;
        }
        for property in psi_validation::declared_property_requirements(&parameter.bounds) {
            if psi_validation::type_satisfies_declared_property(
                program,
                symbols,
                &[],
                *type_reference,
                property,
            ) {
                continue;
            }
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application no longer satisfies the selected operator binder bounds",
            ));
            return None;
        }
        arguments.push(CheckedBoundaryOperatorApplicationArgument::Type {
            binder_owner: *binder_owner,
            binder_ordinal: *binder_ordinal,
            binder_symbol: *binder_symbol,
            type_reference: *type_reference,
        });
    }
    Some(arguments)
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
