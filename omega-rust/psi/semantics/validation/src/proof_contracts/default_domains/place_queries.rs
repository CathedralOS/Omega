//! Place spelling, data-schema resolution, and standing-fact queries.
//!
//! These are read-only structural queries shared by the default-domain write
//! engine and reader hypotheses. They do not own flow state or diagnostics.

use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Render a Name-rooted place (`self.map`, `target`, `local.a`); `None`
/// for computed receivers. Slice 6: parameter/local roots are tracked too
/// -- their writes carry the same obligation; only their VALUATION model
/// differs (no born zero).
pub(super) fn self_place_spelling(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            members.first()?;
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = self_place_spelling(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = self_place_spelling(program, indexed.collection)?;
            let index = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Integer(value) => value.text().to_owned(),
                _ => return None,
            };
            Some(format!("{collection}[{index}]"))
        }
        ExpressionNode::Borrow(inner) => self_place_spelling(program, inner.target),
        _ => None,
    }
}

/// Slice 6: the born-zero valuation model applies only to machine-owned
/// (self-rooted) storage.
pub(super) fn is_self_rooted(spelling: &str) -> bool {
    spelling == "self" || spelling.starts_with("self.")
}

/// The write-target analogue of [`self_place_spelling`]: a non-literal index
/// renders the position unrepresentable, so it spells the wildcard segment
/// `[*]` rather than vanishing entirely. A write through an unresolvable
/// index is still a write -- dropping it would let a dynamic-position store
/// escape every invariant-window obligation (ch11's conservative ceiling on
/// unrepresentable origins).
pub(super) fn write_place_spelling(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let collection = write_place_spelling(program, indexed.collection)?;
            let index = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Integer(value) => value.text().to_owned(),
                _ => "*".to_owned(),
            };
            Some(format!("{collection}[{index}]"))
        }
        ExpressionNode::Member(member) => {
            let receiver = write_place_spelling(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        ExpressionNode::Borrow(inner) => write_place_spelling(program, inner.target),
        ExpressionNode::Name(_) => self_place_spelling(program, expression),
        _ => None,
    }
}

/// Whether a tracked place whose spelling is `place` also covers a reader or
/// writer at `nested`: every `.`-separated segment of `place` must cover the
/// corresponding `nested` segment, where a `[*]` segment covers any concrete
/// index at that position (`self.maps[*]` covers `self.maps[0]` and
/// `self.maps[0].start`, but never `self.maps_deep` or a sibling member).
pub(super) fn place_spelling_covers(place: &str, nested: &str) -> bool {
    let mut place_parts = place.split('.');
    let mut nested_parts = nested.split('.');
    loop {
        match (place_parts.next(), nested_parts.next()) {
            (Some(place_part), Some(nested_part)) => {
                if !segment_covers(place_part, nested_part) {
                    return false;
                }
            }
            (None, _) => return true,
            (Some(_), None) => return false,
        }
    }
}

/// One segment of a place spelling: `[*]` index positions in `place` cover
/// any `[<index>]` at the same position in `nested`; everything else must
/// match literally.
fn segment_covers(place: &str, nested: &str) -> bool {
    let mut place = place;
    let mut nested = nested;
    loop {
        let Some(at) = place.find("[*]") else {
            return place == nested || (place.is_empty() && nested.starts_with('['));
        };
        if !nested.starts_with(&place[..at]) {
            return false;
        }
        nested = &nested[at..];
        let Some(close) = nested.find(']') else {
            return false;
        };
        nested = &nested[close + 1..];
        place = &place[at + "[*]".len()..];
    }
}

/// True when `child` is strictly nested under `parent` (`child.field...` or
/// `child[...]`, wildcard segments included). A write to `parent` reseeds the
/// whole region, so every tracked sub-place lapses.
pub(super) fn is_subplace(child: &str, parent: &str) -> bool {
    child.starts_with(&format!("{parent}.")) || child.starts_with(&format!("{parent}["))
}

/// The expressions a place target still evaluates: every `Indexed` index
/// inside the place spine. The member/name spine itself is the write path,
/// not a read, so it is never reported; index expressions are ordinary
/// evaluated reads (and may contain calls).
pub(super) fn write_target_index_expressions(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut indexes = Vec::new();
    let mut cursor = target;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                indexes.push(indexed.index);
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Borrow(inner) => cursor = inner.target,
            _ => break,
        }
    }
    indexes
}

/// Every expression a transition evaluates: an optional `when` guard plus
/// each named target's argument list and each value target's expression.
/// Call sites inside these are consumption points exactly like call
/// statements.
pub(super) fn transition_evaluated_expressions(
    program: &TypedTrees,
    transition: &typed_trees::statement::TableTransition,
) -> Vec<ExpressionHandle> {
    let mut evaluated = Vec::new();
    if let typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
        evaluated.push(*guard);
    }
    for handle in [transition.target, transition.continuation] {
        if !handle.is_valid() {
            continue;
        }
        match program.statement_table.transition_target(handle) {
            typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                evaluated.extend_from_slice(program.statement_table.expression_handles(*arguments))
            }
            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                evaluated.push(*expression);
            }
            _ => {}
        }
    }
    evaluated
}

pub(super) fn domain_definition_by_name<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> Option<&'program DataDefinition> {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .filter(|definition| {
            !definition.where_facts.is_empty()
                || data_has_case_where_facts(program, definition)
                || crate::value_custody::data::data_requires_establishment(program, definition)
        })
}

/// CASE-CONSTRAINTS (ch12): does any case of `definition` carry `where`
/// facts? A constrained case's facts extend the default domain while that
/// case is active, so its writes join the same invariant-window net as a
/// `where`-carrying record's.
pub(super) fn data_has_case_where_facts(program: &TypedTrees, definition: &DataDefinition) -> bool {
    program.data_members(definition).iter().any(|member| {
        matches!(member, typed_trees::data::DataMember::Variant(variant)
            if !variant.where_facts.is_empty())
    })
}

/// Resolve the data value denoted by an expression. `declared_place_type`
/// intentionally treats bare `self` as a root rather than a value with an
/// authored local type, so machine-attached storage needs this explicit arm.
/// Keeping the arm here also makes `self.field` and nested local/parameter
/// receivers share the same establishment analysis without manufacturing a
/// synthetic type-reference handle for `self`.
pub(super) fn data_definition_for_expression<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<&'program DataDefinition> {
    if self_place_spelling(program, expression).as_deref() == Some("self") {
        let attached = machine.attached_data.as_ref()?;
        return program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached);
    }
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) {
        let mut collection_type = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            state,
            indexed.collection,
        )?;
        loop {
            match program.type_reference_table.type_reference(collection_type) {
                TypeReferenceNode::Reference { referee, .. } => collection_type = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => collection_type = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    return data_definition_for_type(program, *element_type);
                }
                _ => return None,
            }
        }
    }
    let receiver_type =
        crate::value_custody::places::declared_place_type(program, machine, state, expression)?;
    data_definition_for_type(program, receiver_type)
}

fn data_definition_for_type(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *name),
        TypeReferenceNode::Reference { referee, .. } => data_definition_for_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_definition_for_type(program, *base_type)
        }
        _ => None,
    }
}

pub(super) fn field_is_where_mentioned(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: &str,
) -> bool {
    fact_span_mentions_field(program, definition.where_facts, field)
}

/// Whether any fact in `facts` names `field` -- the write-target test shared
/// by type-wide `where` facts and a constrained case's own fact span.
pub(super) fn fact_span_mentions_field(
    program: &TypedTrees,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    field: &str,
) -> bool {
    program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .any(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_name(program, *expression, field)
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                membership_field_name(program, membership.value) == Some(field)
            }
            typed_trees::domain::ProofFact::Proposition(_) => false,
        })
}

pub(super) fn membership_field_name(program: &TypedTrees, value: ExpressionHandle) -> Option<&str> {
    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

fn expression_mentions_name(
    program: &TypedTrees,
    expression: ExpressionHandle,
    name: &str,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == name),
        ExpressionNode::Binary(binary) => {
            expression_mentions_name(program, binary.left, name)
                || expression_mentions_name(program, binary.right, name)
        }
        ExpressionNode::Member(member) => expression_mentions_name(program, member.receiver, name),
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_name(program, indexed.collection, name)
                || expression_mentions_name(program, indexed.index, name)
        }
        ExpressionNode::Borrow(inner) => expression_mentions_name(program, inner.target, name),
        ExpressionNode::Cast(cast) => expression_mentions_name(program, cast.value, name),
        ExpressionNode::Unary(unary) => expression_mentions_name(program, unary.operand, name),
        ExpressionNode::Atomic(atomic) => {
            expression_mentions_name(program, atomic.value, name)
                || expression_mentions_name(program, atomic.result, name)
        }
        ExpressionNode::Range(range) => {
            expression_mentions_name(program, range.start, name)
                || expression_mentions_name(program, range.end, name)
        }
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_mentions_name(program, *element, name)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_mentions_name(program, field.value, name)),
        ExpressionNode::Match(matched) => {
            expression_mentions_name(program, matched.subject, name)
                || program
                    .expression_table
                    .match_arms(matched.arms)
                    .iter()
                    .any(|arm| {
                        expression_mentions_name(program, arm.value, name)
                            || match arm.pattern {
                                typed_trees::expression::MatchPattern::Value(pattern) => {
                                    expression_mentions_name(program, pattern, name)
                                }
                                typed_trees::expression::MatchPattern::Wildcard => false,
                            }
                    })
        }
        ExpressionNode::Call(call) => {
            expression_mentions_name(program, call.receiver, name)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_mentions_name(program, *argument, name))
        }
        _ => false,
    }
}
