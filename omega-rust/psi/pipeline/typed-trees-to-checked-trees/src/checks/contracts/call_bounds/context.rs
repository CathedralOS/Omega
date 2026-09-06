//! Integer comparison entailment from surviving call-entry facts.
//!
//! The arithmetic engine receives immutable parameter atoms, landed literals,
//! and independently selected Exact addition/subtraction/multiplication trees.
//! Caller storage and earlier initializers are never replayed.

use super::*;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use validation::{
    StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
    StrictArithmeticSymbolBinding,
};

mod arguments;

pub(in crate::checks::contracts) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    goal: ExpressionHandle,
) -> bool {
    prove(program, facts, caller, call, contexts, goal).unwrap_or(false)
}

fn prove(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    goal: ExpressionHandle,
) -> Option<bool> {
    let site = crate::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let ordinary_call = match &site {
        crate::CallSite::Expression { call: source, .. } => {
            source.target_symbol == call.target_symbol
                && (!source.receiver.is_valid()
                    || matches!(
                        program.expression_table.expression(source.receiver),
                        ExpressionNode::Name(path)
                            if is_data_namespace(program, path.symbol)
                                && (path.head_symbol == path.symbol
                                    || !path.head_symbol.is_valid())
                    ))
                && source.static_requirement_dispatch.is_none()
                && source.quotient_operation.is_none()
                && source.private_layout_operation.is_none()
                && source.machine_arguments.is_empty()
                && source.evidence_arguments.is_empty()
        }
        crate::CallSite::Statement(source) => {
            // The shared statement-call traversal already selected this exact
            // target. A data namespace is not a runtime receiver place.
            source.target_symbol == call.target_symbol
                && ((source.receiver.is_empty()
                    && !source.receiver_symbol.is_valid()
                    && !source.receiver_root_symbol.is_valid())
                    || (is_data_namespace(program, source.receiver_symbol)
                        && (source.receiver_root_symbol == source.receiver_symbol
                            || !source.receiver_root_symbol.is_valid())))
                && source.static_requirement_dispatch.is_none()
                && source.machine_arguments.is_empty()
                && source.evidence_arguments.is_empty()
        }
        crate::CallSite::TransitionNamed { .. } => false,
    };
    if !ordinary_call {
        return None;
    }
    let caller_machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller.machine_symbol)?;
    let caller_state =
        crate::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)?;
    let callee = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|state| state.symbol == call.target_symbol)
    })?;
    let parameters = program.state_parameters(program.machine_states(callee).first()?);
    let arguments = crate::call_site_argument_expressions(program, &site);
    if parameters.iter().any(|parameter| parameter.is_self) || parameters.len() != arguments.len() {
        return None;
    }
    let caller_parameters = program.state_parameters(caller_state);
    let bindings = caller_parameters
        .iter()
        .filter_map(|parameter| {
            let primitive = fixed_parameter_type(program, parameter)?;
            Some(binding(parameter.symbol, parameter.symbol, primitive))
        })
        .collect::<Vec<_>>();
    let mut argument_bindings = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            continue;
        };
        if !fixed_integer(primitive) {
            continue;
        }
        if fixed_parameter_type(program, parameter) != Some(primitive) {
            return None;
        }
        if arguments::primitive_type(
            program,
            facts,
            caller_machine.symbol,
            caller_parameters,
            *argument,
        ) != Some(primitive)
        {
            return None;
        }
        argument_bindings.push(validation::StrictArithmeticExpressionBinding {
            symbol: parameter.symbol,
            expression: *argument,
        });
    }
    if !comparison_is_supported(program, facts, callee.symbol, parameters, goal) {
        return None;
    }
    // This exact call-entry roster contains simultaneously active overlays,
    // not alternative arrival paths. Their surviving facts are joint premises.
    let hypotheses = contexts
        .iter()
        .flat_map(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
        })
        .filter(|fact| {
            !matches!(
                fact.origin,
                facts::FactOrigin::CallRequires | facts::FactOrigin::CallEnsures
            )
        })
        .filter_map(|fact| {
            // Call substitution lives separately from the authored expression.
            // This adapter only consumes declaration-shaped caller facts.
            let expression = match fact.payload {
                facts::FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Requires,
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => expression,
                facts::FactPayload::BooleanValue {
                    expression,
                    value: true,
                } => expression,
                _ => return None,
            };
            comparison_is_supported(
                program,
                facts,
                caller.machine_symbol,
                caller_parameters,
                expression,
            )
            .then_some(expression)
        })
        .collect::<Vec<_>>();
    Some(
        validation::strict_arithmetic_expression_implication_with_arguments(
            program,
            caller_machine,
            &hypotheses,
            goal,
            &bindings,
            &argument_bindings,
        ) == StrictArithmeticImplicationJudgment::Proven,
    )
}

fn is_data_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == symbol)
}

fn binding(
    formal: SymbolHandle,
    actual: SymbolHandle,
    primitive: PrimitiveType,
) -> StrictArithmeticSymbolBinding {
    StrictArithmeticSymbolBinding {
        symbol: formal,
        value: StrictArithmeticBindingValue::Atom {
            identity: format!("\0call-parameter:{actual:?}"),
            unsigned: matches!(
                primitive,
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
            ),
        },
    }
}

fn fixed_integer(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
}

fn fixed_parameter_type(program: &TypedTrees, parameter: &StateParameter) -> Option<PrimitiveType> {
    if !parameter.symbol.is_valid()
        || parameter.is_mutable
        || parameter.is_const
        || parameter.is_self
    {
        return None;
    }
    program
        .primitive_type_reference(parameter.type_reference)
        .filter(|primitive| fixed_integer(*primitive))
}

fn direct_parameter<'a>(
    program: &TypedTrees,
    parameters: &'a [StateParameter],
    expression: ExpressionHandle,
) -> Option<&'a StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    parameters
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)
}

fn comparison_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    let spelling = match binary.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    let operand = |expression| -> Option<(PrimitiveType, Option<TypeReferenceHandle>)> {
        if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
            let primitive = crate::values::scalar_expression_type(
                &checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal: literal.clone(),
                },
            )?;
            return fixed_integer(primitive).then_some((primitive, None));
        }
        let parameter = direct_parameter(program, parameters, expression)?;
        Some((
            fixed_parameter_type(program, parameter)?,
            Some(parameter.type_reference),
        ))
    };
    let Some((left, left_reference)) = operand(binary.left) else {
        return false;
    };
    let Some((right, right_reference)) = operand(binary.right) else {
        return false;
    };
    left == right
        && super::super::prover::has_builtin_operators(program, &facts.operators, expression)
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            &[left_reference, right_reference],
        )
}
