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
use facts::FactPayload;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::statement::TransitionTargetHandle;

use super::assembly::expression_reads_overlapping_place;

/// What a requirement instance reads at the jump: the argument expression a
/// mentioned parameter binds, at that operand's position.
enum RequirementRead {
    Argument(ExpressionHandle),
}

/// One call inside this arm's operands, with the places it may write.
struct ArmOperandWrites {
    authored_expression: ExpressionHandle,
    writes: Vec<validation::CanonicalPlace>,
}

/// The named jump this call belongs to and every call inside its operands with
/// the places each may write, or `None` when a write frame cannot be bounded
/// (callers treat that as "may write anything"). `jump_arm` is `None` when this
/// call is not a named jump at all: only a named jump has operands that run
/// between the guard and the requirement's read, so the routes' existing
/// prerequisites are unchanged.
struct ArmOperandFrame {
    jump_arm: Option<TransitionTargetHandle>,
    state_symbol: SymbolHandle,
    writes: Vec<ArmOperandWrites>,
}

fn arm_operand_frame(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ArmOperandFrame> {
    let (machine, state) =
        program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)?;
    let arm_of = |call_ordinal: usize| -> Option<TransitionTargetHandle> {
        validation::transition_call_target(
            program,
            machine,
            state,
            call_flow.statement_index,
            call_ordinal,
        )
    };
    let jump_arm = arm_of(call_flow.call_ordinal).filter(|arm| arm.is_valid());
    let Some(jump_arm) = jump_arm else {
        return Some(ArmOperandFrame {
            jump_arm: None,
            state_symbol: state.symbol,
            writes: Vec::new(),
        });
    };
    let state_flow_calls = facts
        .flow
        .control
        .state(machine.symbol, state.symbol)
        .map(|owner| facts.flow.control.calls.span_or_empty(owner.calls))?;
    let borrow_calls = facts
        .borrow
        .first_state_borrow(machine.symbol, state.symbol)
        .map(|(_, owner)| facts.borrow.calls.span_or_empty(owner.calls))
        .unwrap_or(&[]);
    let summaries = crate::flow::StateMutationSummaryCache::default();
    // Call ordinals number the named jump before the calls inside its operands,
    // so ordinal order cannot say which ran first; operand position can. Guard
    // calls run before the guard decides and the other arm's calls never run on
    // this edge, so only calls located inside this arm's operands remain.
    let mut writes = Vec::new();
    for operand_call in state_flow_calls.iter().filter(|call| {
        call.statement_index == call_flow.statement_index
            && call.call_ordinal != call_flow.call_ordinal
            && call.authored_expression.is_valid()
    }) {
        if arm_of(operand_call.call_ordinal) != Some(jump_arm) {
            continue;
        }
        let borrow_call = borrow_calls.iter().find(|candidate| {
            candidate.statement_index == operand_call.statement_index
                && candidate.call_ordinal == operand_call.call_ordinal
        })?;
        let mut mutated = crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            &facts.borrow,
            borrow_call,
            &summaries,
            call_frames,
        )?;
        // Callee-side index selectors do not evaluate in this state's value
        // namespace; an unknown coordinate may overlap any element.
        for write in &mut mutated {
            for segment in &mut write.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    *expression = ExpressionHandle::invalid();
                }
            }
        }
        writes.push(ArmOperandWrites {
            authored_expression: operand_call.authored_expression,
            writes: mutated,
        });
    }
    Some(ArmOperandFrame {
        jump_arm: Some(jump_arm),
        state_symbol: state.symbol,
        writes,
    })
}

/// `true` when no operand call evaluated before one of the requirement's
/// reads may write the storage that read names. `mentioned` lists the
/// symbols the requirement spells; only those naming a target parameter are
/// supplied by the jump, so only those can be read after an earlier operand.
/// An operand call whose write frame the checked facts cannot bound is
/// treated as writing everything.
pub(super) fn requirement_reads_survive_earlier_operand_writes(
    program: &dyn ::validation::CheckingInput,
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
    let Some(frame) = arm_operand_frame(program, facts, state_flow, call_flow, call_frames) else {
        return false;
    };
    if frame.jump_arm.is_none() {
        return true;
    }
    for operand in &frame.writes {
        let Some(operand_position) = arguments.iter().position(|argument| {
            expression_contains(program, *argument, operand.authored_expression)
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
        let written = operand.writes.iter().any(|write| {
            later_reads.iter().any(|read| match read {
                RequirementRead::Argument(argument) => expression_reads_overlapping_place(
                    program,
                    frame.state_symbol,
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

/// `true` when no call inside this arm's operands may write storage that a
/// captured transition subject's predicate reads.
///
/// A COMPUTED transition subject is evaluated once, before arm dispatch, and
/// captured into a generated local; the arms read the capture's truth. The
/// predicate's operands are read at that evaluation, before every arm operand,
/// so any operand call writing one of them leaves a guard-derived requirement
/// discharged from a value the jump no longer carries. The captured truth test
/// mentions none of those operands, which is why the recovered predicate is
/// what this check reads.
pub(super) fn premise_reads_survive_arm_operand_writes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    premise: ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(frame) = arm_operand_frame(program, facts, state_flow, call_flow, call_frames) else {
        return false;
    };
    if frame.jump_arm.is_none() {
        return true;
    }
    frame.writes.iter().all(|operand| {
        operand.writes.iter().all(|write| {
            !expression_reads_overlapping_place(
                program,
                frame.state_symbol,
                call_flow.statement_index,
                premise,
                write,
            )
        })
    })
}

/// The symbols a boolean contract expression names at its roots.
pub(super) fn boolean_requirement_mentions(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut mentioned = Vec::new();
    collect_root_mentions(program, expression, &mut mentioned);
    mentioned
}

/// The symbols a call-entry fact names: a boolean contract expression's
/// roots, or a domain membership's subject expression roots. Both are
/// callee-terms spellings -- a membership's PLACE was already substituted to
/// the caller subject and names no target parameter, so mentioning it could
/// never locate the operand position the requirement actually reads.
pub(super) fn fact_requirement_mentions(
    program: &dyn ::validation::CheckingInput,
    _facts: &CheckFacts,
    fact: &facts::Fact,
) -> Vec<SymbolHandle> {
    match fact.payload {
        FactPayload::ContractBooleanExpression { expression, .. } => {
            boolean_requirement_mentions(program, expression)
        }
        FactPayload::ContractDomainMembership { value, .. } => {
            boolean_requirement_mentions(program, value)
        }
        _ => Vec::new(),
    }
}

/// The caller argument expressions a requirement instance reads: one per
/// mentioned target parameter. Symbols that name no target parameter are not
/// supplied by the call and yield nothing.
pub(super) fn requirement_argument_reads(
    arguments: &[ExpressionHandle],
    target_parameters: &[StateParameter],
    mentioned: &[SymbolHandle],
) -> Vec<ExpressionHandle> {
    requirement_reads(arguments, target_parameters, mentioned)
        .into_iter()
        .map(|(_, RequirementRead::Argument(argument))| argument)
        .collect()
}

/// Each mentioned target parameter paired with the operand position at which
/// the jump reads it. Symbols that name no target parameter are not supplied
/// by the operands and are left to the other provers.
fn requirement_reads(
    arguments: &[ExpressionHandle],
    target_parameters: &[StateParameter],
    mentioned: &[SymbolHandle],
) -> Vec<(usize, RequirementRead)> {
    mentioned
        .iter()
        .filter_map(|symbol| {
            let index =
                typed_trees::signature::state_parameter_position(target_parameters, *symbol)?;
            let argument_index = index;
            let argument = *arguments.get(argument_index)?;
            Some((argument_index, RequirementRead::Argument(argument)))
        })
        .collect()
}

fn collect_root_mentions(
    program: &dyn ::validation::CheckingInput,
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
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
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
        ExpressionNode::SavedSubject(saved) => {
            if let Some(source) = program.saved_value_source(saved) {
                recurse(source);
            }
        }
        typed_trees::spelled_literal_expression_pattern!() => {}
    }
}

/// Whether `needle` occurs anywhere inside `expression`.
fn expression_contains(
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::Name(_)
        | typed_trees::spelled_literal_expression_pattern!() => false,
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Indexed(indexed) => recurse(indexed.collection) || recurse(indexed.index),
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Binary(binary) => recurse(binary.left) || recurse(binary.right),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Range(range) => recurse(range.start) || recurse(range.end),
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_deep_any(dispatch, |handle| recurse(handle)),
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
