//! Predicate domains on `f32` and `f64`.
//!
//! Integer arithmetic implication does not apply: a float comparison is an
//! IEEE observation, and subtraction rounds. Two observations are still
//! enough to establish a predicate-only domain, and both are format-specific.
//!
//! A landed literal is one exact value. Its equality observation implies any
//! weaker bound the domain asks for, so `6.0` establishes `value >= 0.01`.
//! A parameter or other live place establishes the domain when an exit or
//! initializer premise already observes a bound that implies the predicate.
//!
//! Builtin subtraction `left - right` establishes `value >= 0` when the same
//! premises prove `left >= right` and both operands finite. The exact
//! difference is then nonnegative. Default rounding keeps that sign: a tiny
//! positive difference becomes `+0`, and overflow becomes `+infinity`, both
//! of which compare `>= 0`. A strict `>` is not claimed, because underflow
//! can produce zero. Without finiteness, `+infinity - +infinity` is NaN, so
//! the ordered comparison alone is not enough.

use checked_trees::CheckFacts;
use facts::{FactContextHandle, FactOrigin, FactPayload};
use numerics::bignum::ExactFloat;
use numerics::literals::FloatFormat;
use typed_trees::domain::DomainDefinition;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::operator::{primitive_float_binary_semantics, resolve_named_expression_call};
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

use crate::values::float_predicates::{comparison, has_ieee_meaning, literal};

pub(in crate::checks::contracts) fn expression_establishes_domain_predicate(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    domain: &DomainDefinition,
    predicate: ExpressionHandle,
) -> bool {
    let Some(format) = domain_float_format(program, domain) else {
        return false;
    };
    float_predicate_holds(
        program,
        facts,
        contexts,
        machine,
        state,
        statement_index,
        value,
        domain,
        predicate,
        format,
    )
}

fn domain_float_format(
    program: &dyn ::validation::CheckingInput,
    domain: &DomainDefinition,
) -> Option<FloatFormat> {
    match program.primitive_type_reference(domain.target_type) {
        Some(PrimitiveType::F32) => Some(FloatFormat::F32),
        Some(PrimitiveType::F64) => Some(FloatFormat::F64),
        _ => None,
    }
}

fn float_predicate_holds(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    domain: &DomainDefinition,
    predicate: ExpressionHandle,
    format: FloatFormat,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(predicate) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return float_predicate_holds(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            domain,
            binary.left,
            format,
        ) && float_predicate_holds(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            domain,
            binary.right,
            format,
        );
    }
    let Some(required) = required_subject_bound(program, domain, predicate, format) else {
        return false;
    };
    value_implies_bound(
        program,
        facts,
        contexts,
        machine,
        state,
        statement_index,
        value,
        format,
        required,
    )
}

fn required_subject_bound(
    program: &dyn ::validation::CheckingInput,
    domain: &DomainDefinition,
    predicate: ExpressionHandle,
    format: FloatFormat,
) -> Option<facts::FloatPredicate> {
    let binary = comparison(program, predicate)?;
    let (operator, bound) =
        if typed_trees::domain::domain_subject_reference(program, domain, binary.left) {
            (binary.operator, binary.right)
        } else if typed_trees::domain::domain_subject_reference(program, domain, binary.right) {
            (binary.operator.mirrored(), binary.left)
        } else {
            return None;
        };
    let bound = literal(program, bound, format)?;
    let builtin = has_ieee_meaning(
        program,
        domain.symbol,
        predicate,
        operator,
        domain.target_type,
    );
    // Token-lowered float comparisons are calls. `comparison` already
    // required the sealed primitive operator.
    let lowered_call = program.expression_table.call_node(predicate).is_some();
    if !builtin && !lowered_call {
        return None;
    }
    facts::FloatPredicate::comparison(format, operator, &bound)
}

fn value_implies_bound(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    format: FloatFormat,
    required: facts::FloatPredicate,
) -> bool {
    if literal(program, value, format).is_some_and(|bound| {
        facts::FloatPredicate::comparison(format, BinaryOperator::Equal, &bound)
            .is_some_and(|observed| observed.implies(required))
    }) {
        return true;
    }
    if premise_implies_bound(
        program,
        facts,
        contexts,
        machine,
        state,
        statement_index,
        value,
        format,
        required,
    ) {
        return true;
    }
    nonnegative_difference(
        program,
        facts,
        contexts,
        machine,
        state,
        statement_index,
        value,
        format,
    )
    .is_some_and(|observed| observed.implies(required))
}

fn premise_implies_bound(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    format: FloatFormat,
    required: facts::FloatPredicate,
) -> bool {
    let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    boolean_premises(program, facts, contexts).any(|premise| {
        let Some(binary) = comparison(program, premise) else {
            return false;
        };
        let (operator, bound) = if same_place(program, state, statement_index, binary.left, &place)
        {
            (binary.operator, binary.right)
        } else if same_place(program, state, statement_index, binary.right, &place) {
            (binary.operator.mirrored(), binary.left)
        } else {
            return false;
        };
        let Some(reference) =
            validation::expression_result_type_reference(program, machine, state, binary.left)
                .or_else(|| {
                    validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        binary.right,
                    )
                })
        else {
            return false;
        };
        if !has_ieee_meaning(program, machine.symbol, premise, operator, reference) {
            return false;
        }
        literal(program, bound, format)
            .and_then(|bound| facts::FloatPredicate::comparison(format, operator, &bound))
            .is_some_and(|observed| observed.implies(required))
    })
}

fn nonnegative_difference(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    format: FloatFormat,
) -> Option<facts::FloatPredicate> {
    let binary = comparison(program, value)?;
    if binary.operator != BinaryOperator::Subtract {
        return None;
    }
    let reference = validation::expression_result_type_reference(program, machine, state, value)?;
    let primitive = program.primitive_type_reference(reference)?;
    let value_format = match primitive {
        PrimitiveType::F32 => FloatFormat::F32,
        PrimitiveType::F64 => FloatFormat::F64,
        _ => return None,
    };
    if value_format != format {
        return None;
    }
    if !subtraction_has_ieee_meaning(program, machine, value, reference) {
        return None;
    }
    let left_finite = operand_is_finite(
        program,
        facts,
        contexts,
        state,
        statement_index,
        binary.left,
    );
    let right_finite = operand_is_finite(
        program,
        facts,
        contexts,
        state,
        statement_index,
        binary.right,
    );
    let ordered = left_is_at_least_right(
        program,
        facts,
        contexts,
        machine,
        state,
        statement_index,
        binary.left,
        binary.right,
    );
    if !left_finite || !right_finite || !ordered {
        return None;
    }
    facts::FloatPredicate::comparison(
        format,
        BinaryOperator::GreaterOrEqual,
        &ExactFloat::from_f64(0.0),
    )
}

fn subtraction_has_ieee_meaning(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let expected_format = match program.primitive_type_reference(reference) {
        Some(PrimitiveType::F32) => FloatFormat::F32,
        Some(PrimitiveType::F64) => FloatFormat::F64,
        _ => return false,
    };
    let Some(spelling) = typed_trees::operator::binary_operator_spelling(BinaryOperator::Subtract)
    else {
        return false;
    };
    if typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[Some(reference), Some(reference)],
    ) {
        return true;
    }
    // Token-lowered `left - right` is a call whose machine binding is the
    // sealed float subtract. `comparison` already classified that call;
    // this repeats the selection so a decoded operator is not trusted from
    // its spelling alone.
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some((target, _)) = validation::find_machine_by_entry_state(program, call.target_symbol)
    else {
        return false;
    };
    let Some(binding) = program.machine_token_binding_by_symbol(target.symbol) else {
        return false;
    };
    primitive_float_binary_semantics(program, binding).is_some_and(|(operator, format)| {
        operator == BinaryOperator::Subtract && format == expected_format
    })
}

fn operand_is_finite(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    state: &State,
    statement_index: usize,
    operand: ExpressionHandle,
) -> bool {
    let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        operand,
    ) else {
        return false;
    };
    boolean_premises(program, facts, contexts).any(|premise| {
        finite_call_argument(program, premise)
            .is_some_and(|argument| same_place(program, state, statement_index, argument, &place))
            || finite_call_argument(program, peel_true_comparison(program, premise)).is_some_and(
                |argument| same_place(program, state, statement_index, argument, &place),
            )
    })
}

fn left_is_at_least_right(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    machine: &Machine,
    state: &State,
    statement_index: usize,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let Some(left_place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        left,
    ) else {
        return false;
    };
    let Some(right_place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        right,
    ) else {
        return false;
    };
    boolean_premises(program, facts, contexts).any(|premise| {
        let Some(binary) = comparison(program, premise) else {
            return false;
        };
        let ordered = matches!(
            binary.operator,
            BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
        );
        let reversed = matches!(
            binary.operator,
            BinaryOperator::Less | BinaryOperator::LessOrEqual
        );
        if !has_ieee_meaning(
            program,
            machine.symbol,
            premise,
            binary.operator,
            validation::expression_result_type_reference(program, machine, state, left)
                .unwrap_or_default(),
        ) {
            return false;
        }
        (ordered
            && same_place(program, state, statement_index, binary.left, &left_place)
            && same_place(program, state, statement_index, binary.right, &right_place))
            || (reversed
                && same_place(program, state, statement_index, binary.left, &right_place)
                && same_place(program, state, statement_index, binary.right, &left_place))
    })
}

fn peel_true_comparison(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return expression;
    };
    if binary.operator == BinaryOperator::Equal
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        )
    {
        binary.left
    } else {
        expression
    }
}

fn finite_call_argument(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let call = program.expression_table.call_node(expression)?;
    let operator = resolve_named_expression_call(program, call)
        .or_else(|| program.operator_definition_by_symbol(call.target_symbol))?;
    let [namespace, requirement] = program.operator_path_members(operator.name) else {
        return None;
    };
    if requirement.as_str() != "is_finite" {
        return None;
    }
    if !matches!(namespace.as_str(), "F32" | "F64" | "Float") {
        return None;
    }
    let [argument] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };
    Some(*argument)
}

fn same_place(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: &validation::CanonicalPlace,
) -> bool {
    validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )
    .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
}

fn boolean_premises<'a>(
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    contexts: &'a [FactContextHandle],
) -> impl Iterator<Item = ExpressionHandle> + 'a {
    contexts.iter().flat_map(move |context| {
        let mut pending = facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
            .filter_map(|fact| premise_expression(fact.origin, fact.payload))
            .collect::<Vec<_>>();
        let mut conjuncts = Vec::new();
        while let Some(expression) = pending.pop() {
            if let Some(binary) = program.expression_table.binary_node(expression)
                && binary.operator == BinaryOperator::And
            {
                pending.push(binary.left);
                pending.push(binary.right);
            } else {
                conjuncts.push(expression);
            }
        }
        conjuncts
    })
}

fn premise_expression(origin: FactOrigin, payload: FactPayload) -> Option<ExpressionHandle> {
    if matches!(origin, FactOrigin::CallRequires | FactOrigin::CallEnsures) {
        return None;
    }
    match payload {
        FactPayload::ContractBooleanExpression {
            kind: facts::ContractFactKind::Requires | facts::ContractFactKind::Ensures,
            expression,
            instantiated,
            ..
        } if !instantiated.is_valid() => Some(expression),
        FactPayload::BooleanExpression(expression) => Some(expression),
        FactPayload::BooleanValue {
            expression,
            value: true,
        } => Some(expression),
        _ => None,
    }
}
