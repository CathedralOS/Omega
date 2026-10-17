//! Caller-state preservation: which fields, label names, and predicate
//! premises survive the statements between an incoming guard and this call.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

use super::super::assembly::expression_reads_overlapping_place;
use crate::labels::symbol_name;

/// The guard match above compares display labels, so it stands only while
/// every unqualified name the instantiated requirement spells still refers to
/// the caller value the guard was evaluated against. Rebinding such a name
/// before the call -- `limit = 0`, a shadowing `let`, or a mutation through a
/// call's write frame (recorded as a pre-call invalidation) -- makes the label
/// quote a stale premise, and this route must refuse; the remaining provers
/// may still establish the fact from current evidence. Qualified `self.field`
/// operands are covered by the field walk above, so `self` itself is skipped.
///
/// `reads` are the caller argument expressions the requirement instance
/// reads. A prior call write under a spelled root stales the premise only
/// when it may overlap one of those reads: popping `main.a` leaves a guard on
/// `main.b.count` intact. A write whose root no read shares cannot be placed
/// against the label and stays conservative.
pub(super) fn caller_state_preserves_label_names(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &typed_trees::state::State,
    required_label: &str,
    reads: &[ExpressionHandle],
) -> bool {
    let names = super::labels::unqualified_label_identifiers(required_label);
    if names.is_empty() {
        return true;
    }
    let rebound_before_call = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(call_flow.statement_index)
        .any(|statement| match statement {
            StatementNode::Assignment(assignment) => names.iter().any(|name| {
                super::labels::assignment_target_mentions_name(program, assignment.target, name)
            }),
            StatementNode::LocalData(local) => names
                .iter()
                .any(|name| local.name.as_str() == name.as_str()),
            _ => false,
        });
    if rebound_before_call {
        return false;
    }
    !facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .any(|invalidation| {
            let Some(symbol) = invalidation.mutated_root.as_symbol() else {
                return false;
            };
            if !names
                .iter()
                .any(|name| name.as_str() == symbol_name(program, symbol))
            {
                return false;
            }
            let written = CanonicalPlace {
                root: invalidation.mutated_root,
                segments: facts
                    .flow
                    .invalidations
                    .segments
                    .span_or_empty(invalidation.mutated_segments)
                    .to_vec(),
            };
            let read_places = reads
                .iter()
                .filter_map(|read| {
                    canonical_place_from_expression_in_state(
                        program,
                        state_flow.state_symbol,
                        call_flow.statement_index,
                        *read,
                    )
                })
                .collect::<Vec<_>>();
            !read_places.iter().any(|place| place.root == written.root)
                || reads.iter().any(|read| {
                    expression_reads_overlapping_place(
                        program,
                        state_flow.state_symbol,
                        call_flow.statement_index,
                        *read,
                        &written,
                    )
                })
        })
}

pub(super) fn collect_expression_self_fields(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    fields: &mut Vec<typed_trees::name::Identifier>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            fields.push(member.member.clone());
            collect_expression_self_fields(program, member.receiver, fields);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_self_fields(program, binary.left, fields);
            collect_expression_self_fields(program, binary.right, fields);
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_self_fields(program, inner.target, fields)
        }
        ExpressionNode::Cast(cast) => collect_expression_self_fields(program, cast.value, fields),
        _ => {}
    }
}

pub(super) fn caller_state_preserves_field(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    field: &typed_trees::name::Identifier,
) -> bool {
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let Some(assignment) = statement.as_assignment()
            && assignment_target_mentions_field(program, assignment.target, field)
        {
            return false;
        }
    }
    true
}

/// The field walk above sees ASSIGNMENTS only: `self.clear()` is a call whose
/// write lands in the state's prior-invalidation list instead. A mutation
/// rooted at this machine's own `self` storage and naming one of the fields
/// the premise reads stales that premise. Writes through a non-`self` root
/// stay with `caller_state_preserves_label_names`, which matches the root's
/// caller-visible name.
pub(super) fn caller_state_preserves_self_fields_against_call_writes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fields: &[typed_trees::name::Identifier],
) -> bool {
    !facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .any(|invalidation| {
            if (invalidation.mutated_root) != PlaceRoot::Symbol(state_flow.machine_symbol) {
                return false;
            }
            let segments = facts
                .flow
                .invalidations
                .segments
                .span_or_empty(invalidation.mutated_segments);
            // A whole-`self` write, or a segment that is not a plain field,
            // cannot be ruled out by the field names.
            segments.is_empty()
                || segments.iter().any(|segment| match segment {
                    PlaceSegment::Field { symbol } => fields
                        .iter()
                        .any(|field| field.as_str() == symbol_name(program, *symbol).as_str()),
                    _ => true,
                })
        })
}

fn assignment_target_mentions_field(
    program: &dyn ::validation::CheckingInput,
    target: typed_trees::expression::ExpressionHandle,
    field: &typed_trees::name::Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    match program.expression_table.expression(target) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || assignment_target_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => {
            assignment_target_mentions_field(program, inner.target, field)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_mentions_field(program, indexed.collection, field)
        }
        _ => false,
    }
}
