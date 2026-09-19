//! Lowering call arguments into scalar plans: retained arguments, boundary
//! calls, direct call bindings and qualified call expressions.

use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::{
    CheckedLocatedScalarExpression, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

pub(crate) fn retain_call_arguments(
    arguments: Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>,
    parameters: &[StateParameter],
    locals: &[ScalarLocal],
    expressions: &mut Vec<CheckedLocatedScalarExpression>,
    source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
    binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
) {
    for (authored_argument, argument) in arguments {
        source_bindings.append(CheckedScalarExpressionBindings {
            destination: symbols::SymbolHandle::invalid(),
            state: argument.state,
            statement_ordinal: argument.statement_ordinal,
            role: argument.role,
            expression: authored_argument,
            symbols: binding_symbols.insert_many(
                parameters.iter().map(|parameter| parameter.symbol).chain(
                    locals
                        .iter()
                        .filter(|local| !local.is_mutable)
                        .map(|local| local.symbol),
                ),
            ),
        });
        expressions.push(argument);
    }
}

pub(crate) fn call_is_boundary(program: &TypedTrees, target_symbol: symbols::SymbolHandle) -> bool {
    let requirement_symbol = program
        .machine_parameter_signature(target_symbol)
        .map_or(target_symbol, |(_, signature)| signature.symbol);
    program.machines().iter().any(|machine| {
        machine.supply_mode.is_boundary_declaration()
            && program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == target_symbol)
    }) || program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == requirement_symbol)
    }) || validation::exact_compiler_intrinsic_boundary_requirement(program, target_symbol)
        .is_some()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_call_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    call_ordinal: usize,
    call_site: &crate::semantic_calls::CallSite<'_>,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>> {
    let target_symbol = match call_site {
        crate::semantic_calls::CallSite::Statement(call) => call.target_symbol,
        crate::semantic_calls::CallSite::Expression { call, .. } => call.target_symbol,
        crate::semantic_calls::CallSite::TransitionNamed { .. } => return None,
    };
    let is_boundary = call_is_boundary(program, target_symbol);

    let target_parameters = crate::semantic_calls::call_target_parameters(program, target_symbol)?;
    let explicit_arguments =
        crate::semantic_calls::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut scalar_index = 0usize;
    let mut structural_index = 0usize;
    let mut erased_index = 0usize;
    let mut output = Vec::new();
    for target in target_parameters {
        if target.is_self && !explicit_self {
            continue;
        }
        let argument = *explicit_arguments.get(explicit_index)?;
        explicit_index = explicit_index.checked_add(1)?;
        // An erased position consumes its authored argument but owns neither a
        // scalar nor a structural ordinal. Its lowered expression stays under
        // an erased role so a Unit call can rebuild the proof-only actuals.
        if crate::execution::terminal_unit::strips_erased_parameter(target)? {
            if !is_boundary {
                let expected_type = program.primitive_type_reference(target.type_reference)?;
                if let Some(lowered) = lower_return_expression(
                    program,
                    operators,
                    argument,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    expected_type,
                    exact_integer_casts,
                ) {
                    output.push((
                        argument,
                        CheckedLocatedScalarExpression {
                            state: state.symbol,
                            statement_ordinal,
                            role: CheckedScalarExpressionRole::ErasedUnitCallArgument {
                                call_ordinal: u32::try_from(call_ordinal).ok()?,
                                erased_ordinal: u32::try_from(erased_index).ok()?,
                            },
                            expression: lowered,
                        },
                    ));
                }
            }
            erased_index = erased_index.checked_add(1)?;
            continue;
        }
        let Some(expected_type) = program.primitive_type_reference(target.type_reference) else {
            let argument_ordinal = u32::try_from(structural_index).ok()?;
            structural_index = structural_index.checked_add(1)?;
            let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument)
            else {
                continue;
            };
            let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            else {
                continue;
            };
            if range.end_inclusive
                || operators.expression_use(argument).is_some_and(|selected| {
                    selected.spelling != language_core::OperatorSpelling::Range
                        || selected.selected_operator_symbol.is_valid()
                        || selected.candidate_count != 0
                        || !matches!(
                            selected.status,
                            CheckedOperatorResolutionStatus::Missing
                                | CheckedOperatorResolutionStatus::BuiltinFallback
                        )
                })
            {
                continue;
            }
            let call_ordinal = u32::try_from(call_ordinal).ok()?;
            for (endpoint, role) in [
                (
                    range.start,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                        call_ordinal,
                        argument_ordinal,
                    },
                ),
                (
                    range.end,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                        call_ordinal,
                        argument_ordinal,
                    },
                ),
            ] {
                if !endpoint.is_valid() {
                    continue;
                }
                if let Some(expression) = lower_return_expression(
                    program,
                    operators,
                    endpoint,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    PrimitiveType::U64,
                    exact_integer_casts,
                ) {
                    output.push((
                        endpoint,
                        CheckedLocatedScalarExpression {
                            state: state.symbol,
                            statement_ordinal,
                            role,
                            expression,
                        },
                    ));
                }
            }
            continue;
        };
        if target.is_self
            || target.is_const
            || (target.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, target).is_none())
        {
            return None;
        }
        let lowered = lower_return_expression(
            program,
            operators,
            argument,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        );
        if let Some(lowered) = lowered {
            output.push((
                argument,
                CheckedLocatedScalarExpression {
                    state: state.symbol,
                    statement_ordinal,
                    role: if is_boundary {
                        CheckedScalarExpressionRole::BoundaryCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    } else {
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(scalar_index).ok()?,
                        }
                    },
                    expression: lowered,
                },
            ));
        }
        scalar_index = scalar_index.checked_add(1)?;
    }
    (explicit_index == explicit_arguments.len()).then_some(output)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_direct_call_binding_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == call.target_symbol)
    })?;
    let target_parameters =
        crate::semantic_calls::call_target_parameters(program, call.target_symbol)?;
    if target_parameters.iter().any(|parameter| {
        parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
    }) {
        return None;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    // Erased positions own no `CallArgument` ordinal; the retained ones are
    // numbered densely to match the callee's stripped scalar signature.
    if target_parameters.iter().any(|target_parameter| {
        crate::execution::terminal_unit::strips_erased_parameter(target_parameter).is_none()
    }) {
        return None;
    }
    let mut argument_ordinal = 0u32;
    let mut erased_ordinal = 0u32;
    arguments
        .iter()
        .zip(target_parameters)
        .map(|(argument, target_parameter)| {
            let expected_type =
                program.primitive_type_reference(target_parameter.type_reference)?;
            let role = if target_parameter.relevance.is_erased() {
                let ordinal = erased_ordinal;
                erased_ordinal = erased_ordinal.checked_add(1)?;
                CheckedScalarExpressionRole::ErasedCallArgument {
                    binding_ordinal,
                    erased_ordinal: ordinal,
                }
            } else {
                let ordinal = argument_ordinal;
                argument_ordinal = argument_ordinal.checked_add(1)?;
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal: ordinal,
                }
            };
            Some((
                *argument,
                CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal,
                    role,
                    expression: lower_return_expression(
                        program,
                        operators,
                        *argument,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        expected_type,
                        exact_integer_casts,
                    )?,
                },
            ))
        })
        .collect()
}

/// Locate a call beneath only same-carrier integer qualifications. The actual
/// callee declaration supplies that carrier; no conversion result is guessed.
pub(crate) fn scalar_qualified_call_expression(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut targets = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
            {
                targets.push(program.primitive_type_reference(cast.target_type)?);
                expression = cast.value;
            }
            ExpressionNode::Call(call) => {
                let state = crate::semantic_calls::find_state(program, call.target_symbol)?;
                let primitive = program.primitive_type_reference(state.return_type)?;
                return (is_integer(primitive)
                    && primitive != PrimitiveType::Addr
                    && targets.iter().all(|target| *target == primitive))
                .then_some(expression);
            }
            _ => return None,
        }
    }
}
