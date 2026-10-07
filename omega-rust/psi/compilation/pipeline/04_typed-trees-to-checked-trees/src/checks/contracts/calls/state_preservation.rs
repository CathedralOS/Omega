//! Caller-state preservation: which fields, label names, and predicate
//! premises survive the statements between an incoming guard and this call.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{FactPlace, PlaceRoot, PlaceSegment};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

use crate::labels::symbol_name;

/// state's statements ran. `caller_state_preserves_field` matches member names
/// anywhere in a write target, so `pair.cap = ...` defeats a predicate's
/// `self.cap` read at a `pair` subject exactly as `self.cap = ...` does; the
/// field set is the subject place's own field segments plus every member the
/// domain predicates and a subject expression name. The label-name scan keeps
/// any unqualified token -- the subject's root, a rebound local -- quoting a
/// live premise rather than a stale one.
#[allow(clippy::too_many_arguments)]
pub(super) fn caller_state_preserves_predicate_premise(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &typed_trees::state::State,
    fact: &facts::Fact,
    predicate_expressions: &[typed_trees::expression::ExpressionHandle],
    required_labels: &[String],
) -> bool {
    let mut fields: Vec<typed_trees::name::Identifier> = Vec::new();
    for expression in predicate_expressions {
        collect_expression_self_fields(program, *expression, &mut fields);
    }
    if let Some(argument) = super::predicate_domains::requirement_subject_expression(facts, fact) {
        collect_expression_self_fields(program, argument, &mut fields);
    }
    if let FactPlace::Place(place_handle) = fact.place {
        let place = facts.semantic.places.get(place_handle);
        for segment in facts.semantic.place_segments.span_or_empty(place.segments) {
            if let PlaceSegment::Field { symbol } = segment {
                fields.push(typed_trees::name::Identifier::from(
                    symbol_name(program, *symbol).as_str(),
                ));
            }
        }
    }
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
        && required_labels.iter().all(|label| {
            caller_state_preserves_label_names(
                program,
                facts,
                state_flow,
                call_flow,
                state,
                label,
            )
        })
        // The name gate cannot see a `self`-rooted write (`self` is skipped
        // as a label token), so a prior CALL writing `self.limit` would leave
        // the guard premise quoting stale storage. Compare the mutated place
        // itself -- root-normalized to the machine's storage identity --
        // against the membership's subject place.
        && !facts
            .flow
            .state_call_prior_invalidations(state_flow, call_flow)
            .any(|invalidation| {
                let FactPlace::Place(place_handle) = fact.place else {
                    return false;
                };
                let subject = facts.semantic.places.get(place_handle);
                (invalidation.mutated_root)
                    == (subject.root)
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        facts
                            .flow
                            .invalidations
                            .segments
                            .span_or_empty(invalidation.mutated_segments),
                        facts.semantic.place_segments.span_or_empty(subject.segments),
                    )
            })
}

/// The guard match above compares display labels, so it stands only while
/// every unqualified name the instantiated requirement spells still refers to
/// the caller value the guard was evaluated against. Rebinding such a name
/// before the call -- `limit = 0`, a shadowing `let`, or a mutation through a
/// call's write frame (recorded as a pre-call invalidation) -- makes the label
/// quote a stale premise, and this route must refuse; the remaining provers
/// may still establish the fact from current evidence. Qualified `self.field`
/// operands are covered by the field walk above, so `self` itself is skipped.
pub(super) fn caller_state_preserves_label_names(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &typed_trees::state::State,
    required_label: &str,
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
            let PlaceRoot::Symbol(symbol) = invalidation.mutated_root else {
                return false;
            };
            names
                .iter()
                .any(|name| name.as_str() == symbol_name(program, symbol))
        })
}

pub(super) fn collect_expression_self_fields(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    field: &typed_trees::name::Identifier,
) -> bool {
    use typed_trees::statement::StatementNode;
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::Assignment(assignment) = statement
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
