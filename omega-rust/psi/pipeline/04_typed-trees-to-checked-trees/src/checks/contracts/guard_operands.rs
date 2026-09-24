//! Whether a requirement's operands survive the jump's earlier operand writes.
//!
//! A named transition evaluates its guard first, then its operands left to
//! right, then jumps. The guard routes in `calls.rs` match a requirement's
//! instantiated label against a guard evaluated before any operand ran, so
//! they quote the value the guard read. An operand evaluated before the
//! operand the requirement reads may write that same storage —
//! `done(clear(&mut self.flag), self.flag)` — after which the guard's fact no
//! longer describes the value delivered. This module answers that one
//! question from the checked write frames of the operand calls: it grants
//! nothing and consults no source shape.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlace, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::statement::TransitionTargetHandle;

use super::assembly::expression_reads_overlapping_place;

/// What a requirement instance reads at the jump: the argument expression a
/// mentioned parameter binds, at that operand's position, or the receiver,
/// read at the jump itself after every operand.
enum RequirementRead {
    Argument(ExpressionHandle),
    Receiver,
}

/// `true` when no operand call evaluated before one of the requirement's
/// reads may write the storage that read names. `mentioned` lists the
/// symbols the requirement spells; only those naming a target parameter are
/// supplied by the jump, so only those can be read after an earlier operand.
/// An operand call whose write frame the checked facts cannot bound is
/// treated as writing everything.
pub(super) fn requirement_reads_survive_earlier_operand_writes(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    arguments: &[ExpressionHandle],
    target_parameters: &[StateParameter],
    mentioned: &[SymbolHandle],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let reads = requirement_reads(arguments, target_parameters, mentioned);
    if reads.is_empty() {
        return true;
    }
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    let receiver = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .map(|parameter| parameter.symbol);
    let arm_of = |call_ordinal: usize| -> Option<TransitionTargetHandle> {
        crate::semantic::calls::transition_call_target(
            program,
            machine,
            state,
            call_flow.statement_index,
            call_ordinal,
        )
    };
    // Only a named jump has operands that run between the guard and the
    // requirement's read. A statement or expression call, or a call inside
    // the guard itself, keeps the routes' existing prerequisites unchanged.
    let Some(jump_arm) = arm_of(call_flow.call_ordinal).filter(|arm| arm.is_valid()) else {
        return true;
    };
    let Some(state_flow_calls) = facts.flow.control.states.iter().find_map(|(_, owner)| {
        (owner.machine_symbol == machine.symbol && owner.state_symbol == state.symbol)
            .then(|| facts.flow.control.calls.span_or_empty(owner.calls))
    }) else {
        return false;
    };
    let borrow_calls = facts
        .borrow
        .states
        .iter()
        .find_map(|(_, owner)| {
            (owner.machine_symbol == machine.symbol && owner.state_symbol == state.symbol)
                .then(|| facts.borrow.calls.span_or_empty(owner.calls))
        })
        .unwrap_or(&[]);
    let summaries = crate::flow::StateMutationSummaryCache::default();
    // Call ordinals number the named jump before the calls inside its
    // operands, so ordinal order cannot say which ran first; operand position
    // can. Guard calls run before the guard decides and the other arm's calls
    // never run on this edge, so only calls located inside this arm's
    // operands remain.
    for operand_call in state_flow_calls.iter().filter(|call| {
        call.statement_index == call_flow.statement_index
            && call.call_ordinal != call_flow.call_ordinal
            && call.authored_expression.is_valid()
    }) {
        if arm_of(operand_call.call_ordinal) != Some(jump_arm) {
            continue;
        }
        let Some(operand_position) = arguments.iter().position(|argument| {
            expression_contains(program, *argument, operand_call.authored_expression)
        }) else {
            continue;
        };
        let later_reads = reads
            .iter()
            .filter(|(position, _)| *position > operand_position)
            .map(|(_, read)| read)
            .collect::<Vec<_>>();
        if later_reads.is_empty() {
            continue;
        }
        let Some(borrow_call) = borrow_calls.iter().find(|candidate| {
            candidate.statement_index == operand_call.statement_index
                && candidate.call_ordinal == operand_call.call_ordinal
        }) else {
            return false;
        };
        let Some(mut writes) = crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            &facts.borrow,
            borrow_call,
            &summaries,
            call_frames,
        ) else {
            return false;
        };
        // Callee-side index selectors do not evaluate in this state's value
        // namespace; an unknown coordinate may overlap any element.
        for write in &mut writes {
            for segment in &mut write.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    *expression = ExpressionHandle::invalid();
                }
            }
        }
        let written = writes.iter().any(|write| {
            later_reads.iter().any(|read| match read {
                RequirementRead::Receiver => {
                    receiver.is_some_and(|receiver| write.root == PlaceRoot::Symbol(receiver))
                }
                RequirementRead::Argument(argument) => expression_reads_overlapping_place(
                    program,
                    state.symbol,
                    call_flow.statement_index,
                    *argument,
                    write,
                ),
            })
        });
        if written {
            return false;
        }
    }
    true
}

/// The symbols a boolean contract expression names at its roots.
pub(super) fn boolean_requirement_mentions(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut mentioned = Vec::new();
    collect_root_mentions(program, expression, &mut mentioned);
    mentioned
}

/// The symbols a call-entry fact names: a boolean contract expression's
/// roots, or a domain membership's place root.
pub(super) fn fact_requirement_mentions(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Vec<SymbolHandle> {
    match fact.payload {
        FactPayload::ContractBooleanExpression { expression, .. } => {
            boolean_requirement_mentions(program, expression)
        }
        FactPayload::ContractDomainMembership { .. } => match fact.place {
            FactPlace::Place(place) => match facts.semantic.places.get(place).root {
                PlaceRoot::Symbol(symbol) => vec![symbol],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Each mentioned target parameter paired with the operand position at which
/// the jump reads it. Symbols that name no target parameter are not supplied
/// by the operands and are left to the other provers.
fn requirement_reads(
    arguments: &[ExpressionHandle],
    target_parameters: &[StateParameter],
    mentioned: &[SymbolHandle],
) -> Vec<(usize, RequirementRead)> {
    let non_self_count = target_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let uses_receiver = target_parameters.iter().any(|parameter| parameter.is_self)
        && arguments.len() == non_self_count;
    mentioned
        .iter()
        .filter_map(|symbol| {
            let index = target_parameters
                .iter()
                .position(|parameter| parameter.symbol == *symbol)?;
            let parameter = &target_parameters[index];
            if parameter.is_self {
                return Some((arguments.len(), RequirementRead::Receiver));
            }
            let argument_index = if uses_receiver {
                target_parameters[..index]
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .count()
            } else {
                index
            };
            let argument = *arguments.get(argument_index)?;
            Some((argument_index, RequirementRead::Argument(argument)))
        })
        .collect()
}

fn collect_root_mentions(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    mentioned: &mut Vec<SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    let mut recurse = |child| collect_root_mentions(program, child, mentioned);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let root = if path.head_symbol.is_valid() {
                path.head_symbol
            } else {
                path.symbol
            };
            if root.is_valid() && !mentioned.contains(&root) {
                mentioned.push(root);
            }
        }
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection);
            recurse(indexed.index);
        }
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Binary(binary) => {
            recurse(binary.left);
            recurse(binary.right);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Range(range) => {
            recurse(range.start);
            recurse(range.end);
        }
        ExpressionNode::Match(dispatch) => {
            recurse(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    recurse(pattern);
                }
                recurse(arm.value);
            }
        }
        ExpressionNode::Call(call) => {
            recurse(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                recurse(*value);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Whether `needle` occurs anywhere inside `expression`.
fn expression_contains(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    needle: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    if expression == needle {
        return true;
    }
    let recurse = |child| expression_contains(program, child, needle);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Indexed(indexed) => recurse(indexed.collection) || recurse(indexed.index),
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Binary(binary) => recurse(binary.left) || recurse(binary.right),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Range(range) => recurse(range.start) || recurse(range.end),
        ExpressionNode::Match(dispatch) => {
            recurse(dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern) if recurse(pattern))
                            || recurse(arm.value)
                    })
        }
        ExpressionNode::Call(call) => {
            recurse(call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| recurse(*argument))
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| recurse(*value)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| recurse(field.value)),
    }
}
