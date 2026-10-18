//! Place spelling, data-schema resolution, and standing-fact queries.
//!
//! These are read-only structural queries shared by the default-domain write
//! engine and reader hypotheses. They do not own flow state or diagnostics.

use language_core::{is_rooted_at, is_self_receiver};
use symbols::JoinedPath;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Exact declaration provenance, not the absence of an attached data binding.
/// Inputs arrive under caller checking; initialized locals under construction
/// checking. Bare local storage has authority only when its complete type
/// admits zero. Tracked writes/windows take precedence at each consumer.
pub(super) fn declared_root_established(
    program: &dyn crate::CheckingInput,
    state: &State,
    root: symbols::SymbolHandle,
) -> bool {
    if !root.is_valid() {
        return false;
    }
    if program.state_parameter_by_symbol(state, root).is_some() {
        return true;
    }
    // Const/value binders receive established carrier values at application;
    // unlike a reserved local they never denote a fresh zero representation.
    // A type binder supplies no value, and an unevaluated constant supplies no
    // completed snapshot. Exact symbols keep same-spelled locals independent.
    if program
        .data_type_parameter_by_symbol(root)
        .is_some_and(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Const { .. }
                    | typed_trees::data::TypeParameterKind::Value { .. }
            )
        })
        || program
            .const_declaration_by_symbol(root)
            .is_some_and(|declaration| declaration.evaluated_value.is_some())
    {
        return true;
    }
    program
        .state_local_declaration(state, root)
        .is_some_and(|(_, local)| {
            local.initial_value.is_valid()
                || !crate::value_custody::data::type_requires_establishment(
                    program,
                    local.type_reference,
                )
        })
}

/// The resolved storage root of a projection, never a rendered name lookup.
pub(super) fn expression_root_symbol(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> symbols::SymbolHandle {
    while expression.is_valid() {
        expression = match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => return path.head_symbol,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => break,
        };
    }
    symbols::SymbolHandle::invalid()
}

/// Render a Name-rooted place (`main.map`, `target`, `local.a`); `None`
/// for computed receivers. Slice 6: parameter/local roots are tracked too
/// -- their writes carry the same obligation; only their VALUATION model
/// differs (no born zero).
pub(super) fn self_place_spelling(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            members.first()?;
            Some(members.joined_path_with("."))
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
/// storage -- the record the machine's data binding names.
///
/// `data_binding` is the name that binding carries in the state being walked:
/// `self` under the receiver spelling, an ordinary parameter name
/// (`main`) under the static one. A machine with no data binding owns no
/// storage here, so nothing is data-rooted for it.
pub(super) fn is_data_rooted(data_binding: Option<&str>, spelling: &str) -> bool {
    data_binding.is_some_and(|root| is_rooted_at(root, spelling))
}

/// The write-target analogue of [`self_place_spelling`]: a non-literal index
/// renders the position unrepresentable, so it spells the wildcard segment
/// `[*]` rather than vanishing entirely. A write through an unresolvable
/// index is still a write -- dropping it would let a dynamic-position store
/// escape every invariant-window obligation (ch11's conservative ceiling on
/// unrepresentable origins).
pub(super) fn write_place_spelling(
    program: &dyn crate::CheckingInput,
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

/// Conservative overlap for invalidation only. Wildcards can occur in either
/// coordinate, including different dimensions; neither place need cover the
/// other. Prefixes overlap their descendants. This grants no place identity,
/// establishment, or permission to close a window.
pub(super) fn place_spellings_may_overlap(left: &str, right: &str) -> bool {
    fn coordinates(place: &str) -> impl Iterator<Item = &str> {
        place.split(['.', '[', ']']).filter(|part| !part.is_empty())
    }
    coordinates(left)
        .zip(coordinates(right))
        .all(|(left, right)| left == right || left == "*" || right == "*")
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    transition: &typed_trees::statement::TableTransition,
) -> Vec<ExpressionHandle> {
    let mut evaluated = Vec::new();
    if let typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
        evaluated.push(*guard);
    }
    for handle in transition.targets() {
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

/// CASE-CONSTRAINTS (ch12): does any case of `definition` carry `where`
/// facts? A constrained case's facts extend the default domain while that
/// case is active, so its writes join the same invariant-window net as a
/// `where`-carrying record's.
pub(super) fn data_has_case_where_facts(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> bool {
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
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<&'program DataDefinition> {
    if self_place_spelling(program, expression)
        .as_deref()
        .is_some_and(is_self_receiver)
    {
        let attached = machine.attached_data.as_ref()?;
        return program.data_definition_by_name(attached);
    }
    if let Some(indexed) = program.expression_table.indexed_node(expression) {
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

pub(super) fn data_definition_for_type(
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            // Source obligations belong to the exact applied declaration,
            // including a schematic body's open static arguments. Closure
            // admits runtime storage, not an exemption from checking a body.
            program
                .data_application_definition(handle)
                .or_else(|| program.closed_data_definition(handle))
        }
        TypeReferenceNode::Reference { referee, .. } => data_definition_for_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            data_definition_for_type(program, *base_type)
        }
        _ => None,
    }
}

pub(super) fn field_is_where_mentioned(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: &str,
) -> bool {
    fact_span_mentions_field(program, definition.where_facts, field)
}

/// Whether any fact in `facts` names `field` -- the write-target test shared
/// by type-wide `where` facts and a constrained case's own fact span.
pub(super) fn fact_span_mentions_field(
    program: &dyn crate::CheckingInput,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    field: &str,
) -> bool {
    program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .any(|fact| fact_names_field(program, fact, field))
}

/// Whether a single fact names any FIELD of `definition`. A `where` fact that
/// names none is a constraint over the declaration's const parameters alone:
/// `FixedBuffer<const N: u64> where N > 0 && N <= 8` is decided once, when the
/// specialization substitutes 4, and it is not a runtime invariant the
/// synthesized record's field writes re-prove. Folding such a fact against a
/// FIELD valuation can only answer "unknown".
pub(super) fn fact_mentions_any_field(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.name.as_str()),
            _ => None,
        })
        .any(|field| fact_names_field(program, fact, field))
}

fn fact_names_field(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    field: &str,
) -> bool {
    match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            expression_mentions_name(program, *expression, field)
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            membership_field_name(program, membership.value) == Some(field)
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    }
}

pub(super) fn membership_field_name(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<&str> {
    crate::expression_traversal::name_path_last_member(program, value)
}

fn expression_mentions_name(
    program: &dyn crate::CheckingInput,
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
