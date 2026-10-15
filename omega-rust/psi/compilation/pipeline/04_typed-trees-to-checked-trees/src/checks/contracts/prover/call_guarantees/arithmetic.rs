//! Arithmetic uses the same live invocation/result join as direct guarantee
//! matching. Each proposition has its own declaration substitution; exact
//! captured places become shared atoms only within this implication. No source
//! initializer is replayed, and no callee precondition is imported as a premise.
//! The existing scoped arithmetic engine owns entailment. This adapter owns
//! field identity, current-value custody and selected builtin operation meaning.
//! Polynomial terms retain Exact arithmetic. A separately marked unsigned
//! Wrapping remainder keeps its exact operand roster through policy erasure;
//! the shared engine still needs independent positivity/nonnegativity premises
//! before using its divisor-relative bound. This admits no general cast or
//! syntax-only interpretation of an overridden operator.
//! Imported return guarantees are optional premises: an ordinary constructor
//! can project a required field directly to an already bounded caller value.
//! An unrelated call-result sibling does not change that scalar relationship.
//! The shared backward value-origin trace follows captured copies through
//! stores while proving intervening write preservation; it does not execute
//! initializers. Calls use the exact operand-evaluation frontier, whereas a
//! stable return uses the exit statement boundary. Those frontiers cannot be
//! interchanged when an earlier sibling operand mutates storage.
//! A captured store also retains the predicates proved about its operand at
//! evaluation. Those predicates bind historical value atoms, not the current
//! contents of the operand's former storage. Following the same atoms through
//! copies lets both later calls and returns use their bounds without reviving
//! invalidated live facts or assuming that a callee's precondition held.

use super::callable::Callable;
use super::{Invocation, actual_projection, bound_place, direct_place, invocation_fact};
use crate::checks::unsigned;
use crate::flow::ValueObservation;
use checked_trees::{CheckFacts, FlowStateFact};
use facts::{ContractFactKind, FactContextHandle, FactOrigin, FactPayload};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    contexts: &[FactContextHandle],
    required: &Invocation<'_>,
    goal: ExpressionHandle,
    hypotheses: Vec<ScopedArithmeticHypothesis>,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let Some(goal) = at_invocation(program, facts, caller, required, goal, &[], frames) else {
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
    goal: &ScopedArithmeticExpression,
) -> bool {
    let hypotheses = super::available(
        program,
        facts,
        caller,
        super::GuaranteeUse::StatementEntry(statement),
        contexts,
        frames,
    )
    .into_iter()
    .filter_map(|guarantee| {
        at_invocation(
            program,
            facts,
            caller,
            &guarantee.invocation,
            guarantee.expression,
            guarantee.post_return_writes.as_deref()?,
            frames,
        )
        .map(|proposition| ScopedArithmeticHypothesis {
            proposition,
            holds: true,
        })
    })
    .collect::<Vec<_>>();
    proves_bound(
        program, facts, caller, statement, contexts, goal, hypotheses, frames, None,
    )
}

fn append_store_hypotheses(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    hypotheses: &mut Vec<ScopedArithmeticHypothesis>,
    frames: &validation::CallFrameResolver<'_>,
) {
    // A store retires facts about the old slot, not truths about values
    // already captured from it. Reconstruct each completed store's premises
    // at that exact pre-store frontier. Origin tracing versions every leaf;
    // nothing here republishes an earlier predicate as current storage truth.
    for stored in facts
        .flow
        .control
        .statements
        .span_or_empty(caller.statements)
        .iter()
        .filter(|stored| stored.statement_index < statement)
    {
        if !facts.flow.control.operands.is_valid(stored.stored_value) {
            continue;
        }
        let operand = facts.flow.control.operands.get(stored.stored_value);
        let entry_contexts = facts
            .flow
            .semantic_constraint_contexts(stored.entry_constraints)
            .collect::<Vec<_>>();
        append_context_hypotheses(
            program,
            facts,
            caller,
            &entry_contexts,
            hypotheses,
            |place, observation| {
                observation_origin(
                    program,
                    facts,
                    caller,
                    stored.statement_index,
                    frames,
                    place,
                    observation,
                )
            },
        );
        let captured_contexts = facts
            .flow
            .semantic_constraint_contexts(stored.pre_store_constraints)
            .collect::<Vec<_>>();
        for guarantee in super::available(
            program,
            facts,
            caller,
            super::GuaranteeUse::EvaluatedContext(stored.statement_index),
            &captured_contexts,
            frames,
        ) {
            if let Some(writes) = guarantee.post_return_writes.as_deref()
                && let Some(proposition) = at_invocation(
                    program,
                    facts,
                    caller,
                    &guarantee.invocation,
                    guarantee.expression,
                    writes,
                    frames,
                )
            {
                hypotheses.push(ScopedArithmeticHypothesis {
                    proposition,
                    holds: true,
                });
            }
        }
        append_context_hypotheses(
            program,
            facts,
            caller,
            &captured_contexts,
            hypotheses,
            |place, observation| {
                crate::flow::operand_entry_origin_is_stable(program, &facts.flow, operand, &place)
                    .then_some(())?;
                observation_origin(
                    program,
                    facts,
                    caller,
                    stored.statement_index,
                    frames,
                    place,
                    observation,
                )
            },
        );
    }
}

fn proves_bound(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[FactContextHandle],
    goal: &ScopedArithmeticExpression,
    mut hypotheses: Vec<ScopedArithmeticHypothesis>,
    frames: &validation::CallFrameResolver<'_>,
    call: Option<&checked_trees::FlowCallFact>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
        return false;
    };
    append_store_hypotheses(program, facts, caller, statement, &mut hypotheses, frames);
    append_context_hypotheses(
        program,
        facts,
        caller,
        contexts,
        &mut hypotheses,
        |place, observation| {
            if let Some(call) = call {
                crate::flow::value_origin_at_call(
                    program,
                    &facts.flow,
                    machine,
                    caller,
                    call,
                    place,
                    Some(frames),
                    observation,
                )
            } else {
                observation_origin(
                    program,
                    facts,
                    caller,
                    statement,
                    frames,
                    place,
                    observation,
                )
            }
        },
    );
    validation::scoped_arithmetic_implication(program, machine, &hypotheses, goal)
        == StrictArithmeticImplicationJudgment::Proven
}

fn append_context_hypotheses(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    contexts: &[FactContextHandle],
    hypotheses: &mut Vec<ScopedArithmeticHypothesis>,
    origin: impl Fn(validation::CanonicalPlace, ValueObservation) -> Option<validation::CanonicalPlace>,
) {
    let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
        return;
    };
    let Some(state) =
        validation::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)
    else {
        return;
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
        if let Some(proposition) = proposition(
            program,
            facts,
            &Callable::Machine { machine, state },
            expression,
            |occurrence, primitive| {
                if let Some(length) = collection_length_value(
                    program,
                    &Callable::Machine { machine, state },
                    occurrence,
                    &origin,
                ) {
                    return Some(length);
                }
                let place = direct_place(program, occurrence)?;
                let place = origin(place, ValueObservation::Contents)?;
                Some(atom(place, primitive))
            },
        ) {
            hypotheses.push(ScopedArithmeticHypothesis { proposition, holds });
        }
    }
}

/// Inputs use their operand frontier; a normal guarantee additionally carries
/// the writes accepted by availability. Those observations have a new value
/// identity at return and must never trace through the invocation's old stores.
pub(super) fn at_invocation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
    post_return_writes: &[validation::CanonicalPlace],
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
                    owner: caller.state_symbol,
                    expression: actual,
                    bindings: Vec::new(),
                }));
            }
            let length =
                collection_length_subject(program, &invocation.callable, occurrence).is_some();
            let (observed, observation) = if length {
                (
                    program.expression_table.member_node(occurrence)?.receiver,
                    collection_length_observation(program, &invocation.callable, occurrence)?,
                )
            } else {
                (occurrence, ValueObservation::Contents)
            };
            let place = bound_place(program, invocation, observed)?;
            if !post_return_writes.is_empty() && matches!(place.root, facts::PlaceRoot::Symbol(_)) {
                let storage = crate::flow::rebase_exact_local_place(
                    program,
                    invocation.caller_state,
                    invocation.statement,
                    place.clone(),
                    Some(frames),
                )?;
                if post_return_writes
                    .iter()
                    .any(|write| observation.changed_by(program, &storage, write))
                {
                    // Availability established this post-call proposition, not
                    // equality to the entry value of the same storage. Keep an
                    // opaque version until a consumer has exact return custody.
                    return Some(ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0call-guarantee:return:{:?}:{}:{}:{primitive:?}:{length}:{storage:?}",
                            invocation.caller_state, invocation.statement, invocation.ordinal,
                        ),
                        unsigned: matches!(
                            primitive,
                            typed_trees::unsigned_integer_primitive_pattern!()
                        ),
                    });
                }
            }
            let place = if matches!(place.root, facts::PlaceRoot::Expression(_)) {
                place
            } else {
                let machine = program.machine_by_symbol(caller.machine_symbol)?;
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
                    observation,
                )?
            };
            Some(if length {
                length_atom(place)
            } else {
                atom(place, primitive)
            })
        },
    )
}

pub(in crate::checks) fn value_origin(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    frames: &validation::CallFrameResolver<'_>,
    place: validation::CanonicalPlace,
) -> Option<validation::CanonicalPlace> {
    observation_origin(
        program,
        facts,
        caller,
        statement,
        frames,
        place,
        ValueObservation::Contents,
    )
}

pub(in crate::checks) fn observation_origin(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    frames: &validation::CallFrameResolver<'_>,
    place: validation::CanonicalPlace,
    observation: ValueObservation,
) -> Option<validation::CanonicalPlace> {
    let machine = program.machine_by_symbol(caller.machine_symbol)?;
    if matches!(place.root, facts::PlaceRoot::Expression(expression)
        if matches!(program.expression_table.expression(expression), ExpressionNode::Call(_)))
    {
        return Some(place);
    }
    crate::flow::value_origin_before_statement(
        program,
        &facts.flow,
        machine,
        caller,
        statement,
        place,
        Some(frames),
        observation,
    )
}

pub(in crate::checks) fn atom(
    place: validation::CanonicalPlace,
    primitive: PrimitiveType,
) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        // These are transient solver coordinates from generational declaration,
        // call and field handles, never display labels or artifact identities.
        identity: format!("\0call-guarantee:{primitive:?}:{place:?}"),
        unsigned: matches!(
            primitive,
            typed_trees::unsigned_integer_primitive_pattern!()
        ),
    }
}

/// Metadata is a scalar observation of a descriptor, not a stored field named
/// `len`. The caller supplies exact descriptor origin/preservation at its own
/// evaluation frontier. Its atom is distinct from every element/value atom.
pub(in crate::checks) fn collection_length_value(
    program: &dyn ::validation::CheckingInput,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    origin: impl FnOnce(
        validation::CanonicalPlace,
        ValueObservation,
    ) -> Option<validation::CanonicalPlace>,
) -> Option<ScopedArithmeticValue> {
    let place = collection_length_subject(program, callable, expression)?;
    let place = origin(
        place,
        collection_length_observation(program, callable, expression)?,
    )?;
    Some(length_atom(place))
}

fn length_atom(place: validation::CanonicalPlace) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!("\0call-guarantee:length:{place:?}"),
        unsigned: true,
    }
}

fn collection_length_observation(
    program: &dyn ::validation::CheckingInput,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> Option<ValueObservation> {
    let member = program.expression_table.member_node(expression)?;
    let mut reference = callable.scalar_reference(program, member.receiver)?;
    let mut borrowed = false;
    loop {
        use typed_trees::types::TypeReferenceNode;
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => {
                borrowed = true;
                reference = *referee;
            }
            _ => break,
        }
    }
    // Slice length belongs to its descriptor. In particular, this does not
    // classify a qualified fixed byte owner's mutable live count as a binding.
    Some(
        if borrowed
            && matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Slice { .. }
            )
        {
            ValueObservation::Binding
        } else {
            ValueObservation::Contents
        },
    )
}

fn collection_length_subject(
    program: &dyn ::validation::CheckingInput,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> Option<validation::CanonicalPlace> {
    let member = program.expression_table.member_node(expression)?;
    let reference = callable.scalar_reference(program, member.receiver)?;
    if validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Declared(reference),
    ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return None;
    }
    direct_place(program, member.receiver)
}

pub(in crate::checks) fn proposition(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticExpression> {
    if !super::super::has_builtin_operators(program, &facts.operators, expression)
        || !callable.builtin_meaning(program, expression)
        || !predicate(program, &facts.operators, callable, expression)
    {
        return None;
    }
    bind_occurrences(program, facts, callable, expression, value)
}

pub(in crate::checks) fn scalar_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
    value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
) -> Option<ScopedArithmeticValue> {
    let (expression, remainder) = admitted_scalar_expression(program, facts, callable, expression)?;
    let term = bind_occurrences(program, facts, callable, expression, value)?;
    Some(if remainder {
        ScopedArithmeticValue::AdmittedIntegerRemainder(term)
    } else {
        ScopedArithmeticValue::Term(term)
    })
}

/// One admission owner for scalar denotations and captured call actuals.
/// Selected builtin meaning and policy precede every polynomial interpretation.
fn admitted_scalar_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, bool)> {
    if !super::super::has_builtin_operators(program, &facts.operators, expression)
        || !callable.builtin_meaning(program, expression)
        || !term(program, &facts.operators, callable, expression)
    {
        return None;
    }
    let remainder =
        admitted_wrapping_unsigned_remainder(program, &facts.operators, callable, expression);
    Some((remainder.unwrap_or(expression), remainder.is_some()))
}

fn bind_occurrences(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
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
    // Storage-dependency discovery deliberately records a measure's receiver.
    // The arithmetic roster also needs the metadata observation itself; it is
    // not a scalar read of that receiver's bytes.
    let mut expressions = Vec::new();
    validation::expression_traversal::collect_expression_tree(
        program,
        expression,
        &mut expressions,
    );
    for expression in expressions {
        if collection_length_subject(program, callable, expression).is_some()
            && !occurrences.contains(&expression)
        {
            occurrences.push(expression);
        }
    }
    let mut bindings = Vec::new();
    for occurrence in occurrences {
        // Opaque calls may receive noninteger values. They are independently
        // formed inputs, not polynomial leaves of the integer result.
        let Some(primitive) = callable
            .scalar_reference(program, occurrence)
            .and_then(|reference| program.primitive_type_reference(reference))
            .filter(|primitive| primitive.is_fixed_integer())
        else {
            continue;
        };
        let binder = match program.expression_table.expression(occurrence) {
            ExpressionNode::Member(_) => ScopedArithmeticBinder::Occurrence(occurrence),
            ExpressionNode::Name(path)
                if validation::is_single_segment_name_path(program, path) =>
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
            value: match binder {
                ScopedArithmeticBinder::Symbol(symbol)
                    if program
                        .data_type_parameter_by_symbol(symbol)
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Const { .. }
                            )
                        }) =>
                {
                    // A captured static binder is one immutable declaration,
                    // not a storage observation at the call/return frontier.
                    // Goal and premises share its exact generational identity;
                    // unrelated same-spelled binders remain distinct atoms.
                    ScopedArithmeticValue::Atom {
                        identity: format!("\0static-parameter:{symbol:?}"),
                        unsigned: matches!(
                            primitive,
                            typed_trees::unsigned_integer_primitive_pattern!()
                        ),
                    }
                }
                _ => value(occurrence, primitive)?,
            },
        });
    }
    let mut arguments = Vec::new();
    validation::expression_traversal::visit_post_order(
        program,
        expression,
        &mut Vec::new(),
        |_, node| {
            if let ExpressionNode::Call(call) = node {
                for argument in program.expression_table.expression_handles(call.arguments) {
                    if !arguments.contains(argument) {
                        arguments.push(*argument);
                    }
                }
            }
        },
    );
    for argument in arguments {
        let Some((selected, remainder)) =
            admitted_scalar_expression(program, facts, callable, argument)
        else {
            continue;
        };
        let term = ScopedArithmeticExpression {
            owner: callable.target_symbol(),
            expression: selected,
            bindings: bindings.clone(),
        };
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Occurrence(argument),
            value: if remainder {
                ScopedArithmeticValue::AdmittedIntegerRemainder(term)
            } else {
                ScopedArithmeticValue::Term(term)
            },
        });
    }
    Some(ScopedArithmeticExpression {
        owner: callable.target_symbol(),
        expression,
        bindings,
    })
}

fn predicate(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::And => {
            predicate(program, operators, callable, binary.left)
                && predicate(program, operators, callable, binary.right)
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            term(program, operators, callable, binary.left)
                && term(program, operators, callable, binary.right)
        }
        _ => false,
    }
}

fn term(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => callable
            .scalar_reference(program, expression)
            .and_then(|reference| program.primitive_type_reference(reference))
            .is_some_and(|primitive| primitive.is_fixed_integer()),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            [binary.left, binary.right].into_iter().all(|operand| {
                term(program, operators, callable, operand)
                    && callable
                        .scalar_reference(program, operand)
                        .map(|reference| {
                            program.arithmetic_domain_for_type_reference(reference)
                                == ArithmeticDomain::Exact
                        })
                        .unwrap_or_else(|| proof_integer_term(program, operand))
            })
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Modulo => {
            super::super::has_builtin_operators(program, operators, expression)
                && callable.builtin_meaning(program, expression)
                && builtin_wrapping_unsigned_remainder(program, callable, binary.left, binary.right)
                && term(program, operators, callable, binary.left)
                && term(program, operators, callable, binary.right)
        }
        ExpressionNode::Cast(cast) => {
            value_preserving_integer_coercion(program, callable, cast)
                && term(program, operators, callable, cast.value)
        }
        ExpressionNode::Call(_) => {
            if let Some((primitive, source)) =
                validation::integer_embedding_argument(program, expression)
            {
                return primitive.is_fixed_integer() && term(program, operators, callable, source);
            }
            // The shared scoped judgment transports the selected declaration's
            // result and published guarantees under this call's actual roster.
            // Formation/effects remain independent checking obligations.
            callable
                .scalar_reference(program, expression)
                .and_then(|reference| program.primitive_type_reference(reference))
                .is_some_and(|primitive| primitive.is_fixed_integer())
        }
        _ => false,
    }
}

/// A selected builtin unsigned remainder has one mathematical value in every
/// integer policy once division-by-zero has been discharged. This adapter does
/// not establish that obligation: the callee's `requires` and the independent
/// admitted remainder certificate do. It admits only the customer shape whose
/// operands share one explicit Wrapping carrier; arbitrary computed policy
/// mixtures stay outside this source-proof vocabulary.
fn builtin_wrapping_unsigned_remainder(
    program: &dyn ::validation::CheckingInput,
    callable: &Callable<'_>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let Some(left) = callable.scalar_reference(program, left) else {
        return false;
    };
    let Some(right) = callable.scalar_reference(program, right) else {
        return false;
    };
    let Some(left_primitive) = program.primitive_type_reference(left) else {
        return false;
    };
    let Some(right_primitive) = program.primitive_type_reference(right) else {
        return false;
    };
    left_primitive == right_primitive
        && unsigned(left_primitive)
        && program.arithmetic_domain_for_type_reference(left) == ArithmeticDomain::Wrapping
        && program.arithmetic_domain_for_type_reference(right) == ArithmeticDomain::Wrapping
}

/// Select the exact modulo node behind the one supported policy-erasure
/// spelling. The returned handle is authority-bearing for scoped entailment:
/// keep every checked operator, callable meaning, carrier and cast fence here.
fn admitted_wrapping_unsigned_remainder(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    callable: &Callable<'_>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let cast = program.expression_table.cast_node(expression)?;
    if !value_preserving_integer_coercion(program, callable, cast) {
        return None;
    }
    let Some(binary) = program.expression_table.binary_node(cast.value) else {
        return None;
    };
    (binary.operator == BinaryOperator::Modulo
        && super::super::has_builtin_operators(program, operators, cast.value)
        && callable.builtin_meaning(program, cast.value)
        && builtin_wrapping_unsigned_remainder(program, callable, binary.left, binary.right))
    .then_some(cast.value)
}

/// An admitted authored integer coercion preserves its mathematical payload.
/// Source formation independently proves any narrowing and target predicates;
/// policy qualifications govern later operations. Recasts remain bit views,
/// and floating conversions keep their separate denotation proof owner.
fn value_preserving_integer_coercion(
    program: &dyn ::validation::CheckingInput,
    callable: &Callable<'_>,
    cast: &typed_trees::expression::TableCastExpression,
) -> bool {
    !cast.form.is_recast()
        && callable
            .scalar_reference(program, cast.value)
            .and_then(|source| program.primitive_type_reference(source))
            .is_some_and(|source| source.is_fixed_integer())
        && program
            .primitive_type_reference(cast.target_type)
            .is_some_and(|target| target.is_fixed_integer())
}

fn proof_integer_term(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
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
