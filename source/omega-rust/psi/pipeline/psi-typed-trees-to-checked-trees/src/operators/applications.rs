use psi_checked_trees::{
    CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationDemand,
    CheckedBoundaryOperatorApplicationUseSite, CheckedOperatorFacts,
    CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand, CheckedValueOrigin,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::operator::ClosedOperatorApplicationArgument;
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
    let mut diagnostics = Vec::new();
    let mut applications = Vec::new();
    let mut symbolic_applications = Vec::new();
    for (_, operator_use) in operators.uses.iter() {
        if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
            continue;
        }
        let Some(operator) = psi_typed_trees::operator::declaration_by_symbol(
            program,
            operator_use.selected_operator_symbol,
        )
        .filter(|operator| operator.is_boundary) else {
            continue;
        };
        match checked_spelled_boundary_application(
            program,
            &symbols,
            operator,
            operator_use.expression,
            operator_use.origin,
            &spelled_operand_types(program, operator_use),
        ) {
            Ok(Some(application)) => applications.push(application),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

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
                    let operand_types =
                        named_expression_operand_types(program, call, operator_use.origin);
                    if application.arguments.iter().any(|argument| {
                        matches!(
                            argument,
                            psi_validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder { .. }
                        )
                    }) {
                        if let Some((machine_symbol, arguments)) =
                            rejoin_validated_symbolic_arguments(
                                program,
                                operator,
                                &application.arguments,
                                &operand_types,
                                operator_use.origin,
                                &mut diagnostics,
                            )
                        {
                            symbolic_applications.push(
                                CheckedSymbolicBoundaryOperatorApplicationDemand {
                                    site: CheckedBoundaryOperatorApplicationUseSite::Expression {
                                        expression,
                                        origin: operator_use.origin,
                                    },
                                    requirement_symbol: operator.symbol,
                                    machine_symbol,
                                    arguments,
                                },
                            );
                        }
                        continue;
                    }
                    if let Some(arguments) = rejoin_validated_arguments(
                        program,
                        &symbols,
                        operator,
                        &application.arguments,
                        Some(&operand_types),
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
                if application.arguments.iter().any(|argument| {
                    matches!(
                        argument,
                        psi_validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder { .. }
                    )
                }) {
                    diagnostics.push(psi_diagnostics::Diagnostic::error(
                        "symbolic statement boundary applications are not yet supported",
                    ));
                    continue;
                }
                if let Some(arguments) = rejoin_validated_arguments(
                    program,
                    &symbols,
                    operator,
                    &application.arguments,
                    None,
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
    operators.symbolic_boundary_applications = symbolic_applications;
    Ok(())
}

fn checked_spelled_boundary_application(
    program: &TypedTrees,
    symbols: &psi_validation::TopLevelSymbols<'_>,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Result<Option<CheckedBoundaryOperatorApplicationDemand>, psi_diagnostics::Diagnostic> {
    let Some(bindings) = psi_typed_trees::operator::closed_operator_application_for_operands(
        program,
        operator,
        operand_types,
    ) else {
        return Ok(None);
    };
    psi_validation::validate_closed_operator_application(program, symbols, operator, &bindings)?;
    Ok(Some(checked_boundary_application_from_bindings(
        operator,
        CheckedBoundaryOperatorApplicationUseSite::Expression { expression, origin },
        bindings,
    )))
}

fn checked_boundary_application_from_bindings(
    operator: &psi_typed_trees::operator::OperatorDefinition,
    site: CheckedBoundaryOperatorApplicationUseSite,
    bindings: Vec<ClosedOperatorApplicationArgument>,
) -> CheckedBoundaryOperatorApplicationDemand {
    let arguments = bindings
        .into_iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            let binder_ordinal =
                u32::try_from(ordinal).expect("operator static telescope ordinal overflow");
            match argument {
                ClosedOperatorApplicationArgument::Type {
                    binder_symbol,
                    type_reference,
                } => CheckedBoundaryOperatorApplicationArgument::Type {
                    binder_owner: operator.symbol,
                    binder_ordinal,
                    binder_symbol,
                    type_reference,
                },
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol,
                    declared_carrier,
                    value,
                } => CheckedBoundaryOperatorApplicationArgument::Const {
                    binder_owner: operator.symbol,
                    binder_ordinal,
                    binder_symbol,
                    declared_carrier,
                    value,
                },
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
    operand_types: Option<&[Option<TypeReferenceHandle>]>,
    diagnostics: &mut Vec<psi_diagnostics::Diagnostic>,
) -> Option<Vec<CheckedBoundaryOperatorApplicationArgument>> {
    let parameters = program.operator_type_parameters(operator);
    if !operator.lifetime_parameters.is_empty()
        || parameters.iter().any(|parameter| {
            !matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
                    | psi_typed_trees::data::TypeParameterKind::Const { .. }
            )
        })
        || validated.len() != parameters.len()
    {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "validated boundary application does not match the supported selected operator telescope",
        ));
        return None;
    }
    let mut closed = Vec::with_capacity(validated.len());
    let mut checked = Vec::with_capacity(validated.len());
    for (ordinal, (argument, parameter)) in validated.iter().zip(parameters).enumerate() {
        let (binder_owner, binder_ordinal, binder_symbol, closed_argument, checked_argument) = match argument {
            psi_validation::ValidatedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                binder_symbol,
                type_reference,
            } => (
                *binder_owner,
                *binder_ordinal,
                *binder_symbol,
                ClosedOperatorApplicationArgument::Type {
                    binder_symbol: *binder_symbol,
                    type_reference: *type_reference,
                },
                CheckedBoundaryOperatorApplicationArgument::Type {
                    binder_owner: *binder_owner,
                    binder_ordinal: *binder_ordinal,
                    binder_symbol: *binder_symbol,
                    type_reference: *type_reference,
                },
            ),
            psi_validation::ValidatedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                binder_symbol,
                declared_carrier,
                value,
            } => (
                *binder_owner,
                *binder_ordinal,
                *binder_symbol,
                ClosedOperatorApplicationArgument::Const {
                    binder_symbol: *binder_symbol,
                    declared_carrier: *declared_carrier,
                    value: value.clone(),
                },
                CheckedBoundaryOperatorApplicationArgument::Const {
                    binder_owner: *binder_owner,
                    binder_ordinal: *binder_ordinal,
                    binder_symbol: *binder_symbol,
                    declared_carrier: *declared_carrier,
                    value: value.clone(),
                },
            ),
            psi_validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder { .. } => {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "symbolic boundary application entered closed-application replay",
                ));
                return None;
            }
        };
        if binder_owner != operator.symbol
            || usize::try_from(binder_ordinal).ok() != Some(ordinal)
            || binder_symbol != parameter.symbol
        {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application does not rejoin the selected operator binder",
            ));
            return None;
        }
        closed.push(closed_argument);
        checked.push(checked_argument);
    }
    if let Err(diagnostic) =
        psi_validation::validate_closed_operator_application(program, symbols, operator, &closed)
    {
        diagnostics.push(diagnostic);
        return None;
    }
    if let Some(operand_types) = operand_types {
        let Some(rederived) = psi_typed_trees::operator::closed_operator_application_for_operands(
            program,
            operator,
            operand_types,
        ) else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application no longer closes from the selected use operands",
            ));
            return None;
        };
        if rederived != closed {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "validated boundary application differs from the application reconstructed from the selected use",
            ));
            return None;
        }
    }
    Some(checked)
}

fn rejoin_validated_symbolic_arguments(
    program: &TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    validated: &[psi_validation::ValidatedBoundaryOperatorApplicationArgument],
    operand_types: &[Option<TypeReferenceHandle>],
    origin: CheckedValueOrigin,
    diagnostics: &mut Vec<psi_diagnostics::Diagnostic>,
) -> Option<(
    psi_symbols::SymbolHandle,
    Vec<CheckedSymbolicBoundaryOperatorApplicationArgument>,
)> {
    let parameters = program.operator_type_parameters(operator);
    if !operator.lifetime_parameters.is_empty()
        || parameters.is_empty()
        || parameters
            .iter()
            .any(|parameter| !matches!(parameter.kind, psi_typed_trees::data::TypeParameterKind::Type))
        || parameters.len() != validated.len()
    {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "symbolic boundary application does not match the supported type-only operator telescope",
        ));
        return None;
    }

    let machine_symbol = validated.first().and_then(|argument| match argument {
        psi_validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
            machine_owner,
            ..
        } => Some(*machine_owner),
        _ => None,
    })?;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "symbolic boundary application lost its enclosing generic machine",
        ));
        return None;
    };
    if origin.machine_symbol() != Some(machine_symbol) {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "symbolic boundary application use does not belong to its enclosing generic machine",
        ));
        return None;
    }
    let machine_parameters = program.machine_type_parameters(machine);
    let rederived =
        psi_typed_trees::operator::symbolic_operator_type_application_for_operands(
            program,
            machine,
            operator,
            operand_types,
        );
    let Some(rederived) = rederived else {
        diagnostics.push(psi_diagnostics::Diagnostic::error(
            "symbolic boundary application no longer reconstructs from its selected use",
        ));
        return None;
    };

    let mut checked = Vec::with_capacity(validated.len());
    for (ordinal, ((argument, parameter), expected)) in validated
        .iter()
        .zip(parameters)
        .zip(&rederived)
        .enumerate()
    {
        let psi_validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            machine_owner,
            machine_binder_ordinal,
            machine_binder_symbol,
        } = argument
        else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "symbolic boundary application mixes closed and open arguments",
            ));
            return None;
        };
        let machine_parameter = usize::try_from(*machine_binder_ordinal)
            .ok()
            .and_then(|ordinal| machine_parameters.get(ordinal));
        if *binder_owner != operator.symbol
            || usize::try_from(*binder_ordinal).ok() != Some(ordinal)
            || *binder_symbol != parameter.symbol
            || *machine_owner != machine_symbol
            || machine_parameter.is_none_or(|candidate| {
                candidate.symbol != *machine_binder_symbol
                    || !matches!(candidate.kind, psi_typed_trees::data::TypeParameterKind::Type)
            })
            || expected.operator_binder_symbol != *binder_symbol
            || expected.machine_binder_ordinal != *machine_binder_ordinal
            || expected.machine_binder_symbol != *machine_binder_symbol
        {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "symbolic boundary application does not rejoin its operator and machine binders",
            ));
            return None;
        }
        checked.push(
            CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
                binder_owner: *binder_owner,
                binder_ordinal: *binder_ordinal,
                binder_symbol: *binder_symbol,
                machine_binder_ordinal: *machine_binder_ordinal,
                machine_binder_symbol: *machine_binder_symbol,
            },
        );
    }
    Some((machine_symbol, checked))
}

fn named_expression_operand_types(
    program: &TypedTrees,
    call: &psi_typed_trees::expression::TableCallExpression,
    origin: CheckedValueOrigin,
) -> Vec<Option<TypeReferenceHandle>> {
    let mut operand_types = Vec::new();
    if call.receiver.is_valid() {
        let receiver = expression_type_reference_for_origin(program, call.receiver, origin);
        if receiver.is_some() {
            operand_types.push(receiver);
        }
    }
    operand_types.extend(
        program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .map(|argument| {
                expression_type_reference_for_origin(program, *argument, origin).or_else(|| {
                    psi_validation::landed_integer_literal_type_reference(program, *argument)
                })
            }),
    );
    operand_types
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
