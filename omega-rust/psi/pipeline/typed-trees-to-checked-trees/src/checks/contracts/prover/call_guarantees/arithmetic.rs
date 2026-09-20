//! Arithmetic uses the same live invocation/result join as direct guarantee
//! matching. Each proposition has its own declaration substitution; exact
//! captured places become shared atoms only within this implication. No source
//! initializer is replayed, and no callee precondition is imported as a premise.
//! The existing scoped arithmetic engine owns entailment. This adapter owns
//! field identity, current-value custody and Exact builtin operation meaning.
//! Imported return guarantees are optional premises: an ordinary constructor
//! can project a required field directly to an already bounded caller value.
//! An unrelated call-result sibling does not change that scalar relationship.
//! The shared backward value-origin trace follows captured copies through
//! stores while proving intervening write preservation; it does not execute
//! initializers. Calls use the exact operand-evaluation frontier, whereas a
//! stable return uses the exit statement boundary. Those frontiers cannot be
//! interchanged when an earlier sibling operand mutates storage.

use super::callable::Callable;
use super::{Invocation, actual_projection, bound_place, direct_place};
use checked_trees::{CheckFacts, FlowStateFact};
use facts::{ContractFactKind, FactContextHandle, FactOrigin, FactPayload};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
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
    hypotheses: Vec<ScopedArithmeticHypothesis>,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let Some(goal) = at_call(program, facts, caller, required, goal, frames) else {
        return false;
    };
    let Some(call) = invocation_fact(facts, caller, required) else {
        return false;
    };
    proves_bound(
        program,
        facts,
        caller,
        required.statement,
        contexts,
        &goal,
        hypotheses,
        frames,
        Some(call),
    )
}

pub(in crate::checks) fn proves_at_exit(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
    goal: &ScopedArithmeticExpression,
) -> bool {
    let hypotheses = super::available(program, facts, caller, statement, contexts, frames)
        .into_iter()
        .filter_map(|guarantee| {
            at_call(
                program,
                facts,
                caller,
                &guarantee.invocation,
                guarantee.expression,
                frames,
            )
            .map(|proposition| ScopedArithmeticHypothesis {
                proposition,
                holds: true,
            })
        })
        .collect();
    proves_bound(
        program, facts, caller, statement, contexts, goal, hypotheses, frames, None,
    )
}

fn proves_bound(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[FactContextHandle],
    goal: &ScopedArithmeticExpression,
    mut hypotheses: Vec<ScopedArithmeticHypothesis>,
    frames: &validation::CallFrameResolver<'_>,
    call: Option<&checked_trees::FlowCallFact>,
) -> bool {
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
            &Callable::Machine { machine, state },
            expression,
            |occurrence, primitive| {
                let place = direct_place(program, occurrence)?;
                let place = if let Some(call) = call {
                    crate::flow::value_origin_at_call(
                        program,
                        &facts.flow,
                        machine,
                        caller,
                        call,
                        place,
                        Some(frames),
                    )?
                } else {
                    value_origin(program, caller, statement, frames, place)?
                };
                Some(atom(place, primitive))
            },
        ) {
            hypotheses.push(ScopedArithmeticHypothesis { proposition, holds });
        }
    }
    validation::scoped_arithmetic_implication(program, machine, &hypotheses, goal)
        == StrictArithmeticImplicationJudgment::Proven
}

pub(super) fn at_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
    frames: &validation::CallFrameResolver<'_>,
) -> Option<ScopedArithmeticExpression> {
    proposition(
        program,
        facts,
        &invocation.callable,
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
            let place = if matches!(place.root, facts::PlaceRoot::Expression(_)) {
                place
            } else {
                let machine = crate::lookup::machine_by_symbol(program, caller.machine_symbol)?;
                let call = invocation_fact(facts, caller, invocation)?;
                // The statement boundary predates earlier sibling operands.
                // Invocation inputs additionally owe their exact call-prefix
                // preservation, even when this call's own arguments are places.
                crate::flow::value_origin_at_call(
                    program,
                    &facts.flow,
                    machine,
                    caller,
                    call,
                    place,
                    Some(frames),
                )?
            };
            Some(atom(place, primitive))
        },
    )
}

fn invocation_fact<'facts>(
    facts: &'facts CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
) -> Option<&'facts checked_trees::FlowCallFact> {
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(caller.calls)
        .iter()
        .filter(|call| {
            call.statement_index == invocation.statement && call.call_ordinal == invocation.ordinal
        });
    let call = calls.next()?;
    calls.next().is_none().then_some(call)
}

pub(in crate::checks) fn value_origin(
    program: &TypedTrees,
    caller: &FlowStateFact,
    statement: usize,
    frames: &validation::CallFrameResolver<'_>,
    place: crate::flow::CanonicalPlace,
) -> Option<crate::flow::CanonicalPlace> {
    let machine = crate::lookup::machine_by_symbol(program, caller.machine_symbol)?;
    if matches!(place.root, facts::PlaceRoot::Expression(expression)
        if matches!(program.expression_table.expression(expression), ExpressionNode::Call(_)))
    {
        return Some(place);
    }
    crate::flow::value_origin_before_statement(
        program,
        machine,
        caller,
        statement,
        place,
        Some(frames),
    )
}

pub(in crate::checks) fn atom(
    place: crate::flow::CanonicalPlace,
    primitive: PrimitiveType,
) -> ScopedArithmeticValue {
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

pub(in crate::checks) fn proposition(
    program: &TypedTrees,
    facts: &CheckFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticExpression> {
    if !super::super::has_builtin_operators(program, &facts.operators, expression)
        || !callable.builtin_meaning(program, expression)
        || !predicate(program, callable, expression)
    {
        return None;
    }
    bind_occurrences(program, callable, expression, value)
}

pub(in crate::checks) fn scalar_term(
    program: &TypedTrees,
    facts: &CheckFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticExpression> {
    if !super::super::has_builtin_operators(program, &facts.operators, expression)
        || !callable.builtin_meaning(program, expression)
        || !term(program, callable, expression)
    {
        return None;
    }
    bind_occurrences(program, callable, expression, value)
}

fn bind_occurrences(
    program: &TypedTrees,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    mut value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticExpression> {
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    let mut bindings = Vec::new();
    for occurrence in occurrences {
        let reference = callable.scalar_reference(program, occurrence)?;
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
                    |place| {
                        place.machine_symbol == callable.owner_symbol() && place.segments.is_empty()
                    },
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

fn predicate(program: &TypedTrees, callable: &Callable<'_>, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::And => {
            predicate(program, callable, binary.left) && predicate(program, callable, binary.right)
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            term(program, callable, binary.left) && term(program, callable, binary.right)
        }
        _ => false,
    }
}

fn term(program: &TypedTrees, callable: &Callable<'_>, expression: ExpressionHandle) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => callable
            .scalar_reference(program, expression)
            .and_then(|reference| program.primitive_type_reference(reference))
            .is_some_and(fixed_integer),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            [binary.left, binary.right].into_iter().all(|operand| {
                term(program, callable, operand)
                    && callable
                        .scalar_reference(program, operand)
                        .map(|reference| {
                            program.arithmetic_domain_for_type_reference(reference)
                                == ArithmeticDomain::Exact
                        })
                        .unwrap_or_else(|| proof_integer_term(program, operand))
            })
        }
        ExpressionNode::Call(_) => validation::integer_embedding_argument(program, expression)
            .is_some_and(|(primitive, source)| {
                fixed_integer(primitive)
                    && matches!(
                        program.expression_table.expression(source),
                        ExpressionNode::Name(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Integer(_)
                    )
                    && term(program, callable, source)
            }),
        // Casts, indexing, computed actuals and selected calls need their own
        // meaning/capture evidence; no syntax-only polynomial interpretation.
        _ => false,
    }
}

// The ordinary runtime result-type query deliberately does not fabricate a
// runtime carrier for embed. Its exact builtin identity selects proof Int;
// only mathematical composition of those terms gets this fallback.
fn proof_integer_term(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if validation::integer_embedding_argument(program, expression).is_some() {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.landing().is_none(),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            proof_integer_term(program, binary.left) && proof_integer_term(program, binary.right)
        }
        _ => false,
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
