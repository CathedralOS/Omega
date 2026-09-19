//! Arithmetic uses the same live invocation/result join as direct guarantee
//! matching. Each proposition has its own declaration substitution; exact
//! captured places become shared atoms only within this implication. No source
//! initializer is replayed, and no callee precondition is imported as a premise.
//! The existing scoped arithmetic engine owns entailment. This adapter owns
//! field identity, current-value custody and Exact builtin operation meaning.

use super::{Invocation, actual_projection, bound_place, captured_place, direct_place};
use checked_trees::{CheckFacts, FlowStateFact};
use facts::{ContractFactKind, FactContextHandle, FactOrigin, FactPayload};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

pub(super) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    contexts: &[FactContextHandle],
    required: &Invocation<'_>,
    goal: ExpressionHandle,
    mut hypotheses: Vec<ScopedArithmeticHypothesis>,
) -> bool {
    if hypotheses.is_empty() {
        return false;
    }
    let Some(goal) = at_call(program, facts, contexts, required, goal) else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller.machine_symbol)
    else {
        return false;
    };
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        caller.machine_symbol,
        caller.state_symbol,
    ) else {
        return false;
    };
    for fact in contexts.iter().flat_map(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
    }) {
        if matches!(
            fact.origin,
            FactOrigin::CallRequires | FactOrigin::CallEnsures
        ) {
            continue;
        }
        let (expression, holds) = match fact.payload {
            FactPayload::ContractBooleanExpression {
                kind: ContractFactKind::Requires,
                expression,
                instantiated,
                ..
            } if !instantiated.is_valid() => (expression, true),
            FactPayload::BooleanValue { expression, value } => (expression, value),
            _ => continue,
        };
        // Denying a conjunction does not deny either conjunct. The solver may
        // retain only readable conjuncts of an asserted premise, which is not
        // a sound weakening beneath negation.
        if !holds
            && matches!(program.expression_table.expression(expression),
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And)
        {
            continue;
        }
        if let Some(proposition) = proposition(
            program,
            facts,
            machine,
            state,
            expression,
            |occurrence, primitive| {
                let place = direct_place(program, occurrence)?;
                let place = captured_place(program, &facts.semantic, contexts, place)?;
                Some(atom(place, primitive))
            },
        ) {
            hypotheses.push(ScopedArithmeticHypothesis { proposition, holds });
        }
    }
    validation::scoped_arithmetic_implication(program, machine, &hypotheses, &goal)
        == StrictArithmeticImplicationJudgment::Proven
}

pub(super) fn at_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticExpression> {
    proposition(
        program,
        facts,
        invocation.machine,
        invocation.state,
        expression,
        |occurrence, primitive| {
            if let Some((actual, remaining)) = actual_projection(program, invocation, occurrence)
                && remaining.is_empty()
                && matches!(
                    program.expression_table.expression(actual),
                    ExpressionNode::Integer(_)
                )
            {
                return Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    expression: actual,
                    bindings: Vec::new(),
                }));
            }
            let place = bound_place(program, invocation, occurrence)?;
            let place = captured_place(program, &facts.semantic, contexts, place)?;
            Some(atom(place, primitive))
        },
    )
}

fn atom(place: crate::flow::CanonicalPlace, primitive: PrimitiveType) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        // These are transient solver coordinates from generational declaration,
        // call and field handles, never display labels or artifact identities.
        identity: format!("\0call-guarantee:{primitive:?}:{place:?}"),
        unsigned: matches!(
            primitive,
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
        ),
    }
}

fn proposition(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    mut value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticExpression> {
    if !super::super::has_builtin_operators(program, &facts.operators, expression)
        || !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        )
        || !predicate(program, machine, state, expression)
    {
        return None;
    }
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    let mut bindings = Vec::new();
    for occurrence in occurrences {
        let reference = scalar_reference(program, machine, state, occurrence)?;
        let primitive = program
            .primitive_type_reference(reference)
            .filter(|primitive| fixed_integer(*primitive))?;
        let binder = match program.expression_table.expression(occurrence) {
            ExpressionNode::Member(_) => ScopedArithmeticBinder::Projection(occurrence),
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1 =>
            {
                ScopedArithmeticBinder::Symbol(path.symbol)
            }
            ExpressionNode::Name(_)
                if validation::reserved_result_place(program, occurrence).is_some_and(
                    |place| place.machine_symbol == machine.symbol && place.segments.is_empty(),
                ) =>
            {
                ScopedArithmeticBinder::Result
            }
            _ => return None,
        };
        bindings.push(ScopedArithmeticBinding {
            binder,
            value: value(occurrence, primitive)?,
        });
    }
    Some(ScopedArithmeticExpression {
        expression,
        bindings,
    })
}

fn predicate(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::And => {
            predicate(program, machine, state, binary.left)
                && predicate(program, machine, state, binary.right)
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            term(program, machine, state, binary.left)
                && term(program, machine, state, binary.right)
        }
        _ => false,
    }
}

fn term(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => scalar_reference(program, machine, state, expression)
            .and_then(|reference| program.primitive_type_reference(reference)).is_some_and(fixed_integer),
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply) => {
            [binary.left, binary.right].into_iter().all(|operand| {
                term(program, machine, state, operand)
                    && scalar_reference(program, machine, state, operand)
                        .map(|reference| program.arithmetic_domain_for_type_reference(reference) == ArithmeticDomain::Exact)
                        .unwrap_or_else(|| matches!(program.expression_table.expression(operand), ExpressionNode::Integer(literal) if literal.landing().is_none()))
            })
        }
        // Casts, indexing, computed actuals and selected calls need their own
        // meaning/capture evidence; no syntax-only polynomial interpretation.
        _ => false,
    }
}

fn scalar_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    validation::reserved_result_place(program, expression)
        .filter(|place| place.machine_symbol == machine.symbol)
        .map(|place| place.type_reference)
        .or_else(|| {
            validation::expression_result_type_reference(program, machine, state, expression)
        })
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
