//! Element stores into collection-constrained array fields.
//!
//! A store `receiver.field[index] = value` changes one element of `field`.
//! Every `where` fact of the receiver's data that reads the field's contents
//! must hold again at the next consumption point, as for any other write
//! ([invariant windows](../../../../../../../wiki/spec/language/dependent_values.md#invariant-windows)).
//! Measures such as `field.len` cannot change and do not count as reads.
//!
//! A fact that applies a collection predicate re-proves by exploring the
//! predicate's checked body over the field's current element knowledge. That
//! knowledge starts from the facts standing before the first store (the
//! place is established and no window is open on it), or from nothing, and
//! each store replaces the written element with the proved set of the stored
//! value. A literal is that set's singleton. A value with no proved bounds
//! keeps the element's carrier. A computed index joins the same set into
//! every element, because the selector may name any of them. Any other fact
//! that reads the contents cannot be re-proved here, so its window stays open
//! and the consumption point rejects it.

use super::assignment_windows::tracked_receiver;
use super::place_queries::{data_definition_for_expression, write_place_spelling};
use super::state_flow::PlaceValuation;
use super::symbolic_values::{LiteralValue, literal_value};
use super::{InvariantWindow, TrackedPlace};
use crate::proof_contracts::collection_predicates::{
    self, ArgumentKnowledge, IntegerSet, unknown_knowledge,
};
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Handle `target = value` when `target` is an element of a data field.
/// Returns `false` when `target` is not such a store, leaving it to the
/// ordinary field-store handling.
pub(super) fn handle_element_store<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    inherited_windows: &[InvariantWindow],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
    statement_index: usize,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(target) else {
        return false;
    };
    // `receiver.field[index]` names one element; a deeper selector such as
    // `receiver.field[i][j]` only says that some element of `field` changed.
    let (member_expression, element_index) =
        match program.expression_table.expression(indexed.collection) {
            ExpressionNode::Member(_) => (indexed.collection, Some(indexed.index)),
            _ => match innermost_member(program, indexed.collection) {
                Some(member) => (member, None),
                None => return false,
            },
        };
    let Some(member) = program.expression_table.member_node(member_expression) else {
        return false;
    };
    let Some(receiver_spelling) = write_place_spelling(program, member.receiver) else {
        return false;
    };
    let Some(definition) =
        data_definition_for_expression(program, machine, Some(state), member.receiver)
    else {
        return false;
    };
    let Some(field) = data_field(program, definition, member.member.as_str()) else {
        return false;
    };
    let affected: Vec<ExpressionHandle> = program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression)
                if reads_field_contents(program, *expression, field.name.as_str()) =>
            {
                Some(*expression)
            }
            _ => None,
        })
        .collect();
    if affected.is_empty() {
        return true;
    }
    // The binding naming the record this state's places are rooted at; see
    // `place_queries::is_data_rooted`.
    let data_binding = program
        .state_data_binding(machine, state)
        .map(|name| name.as_str().to_owned());
    let data_binding = data_binding.as_deref();
    let position = tracked_receiver(
        program,
        state,
        super::place_queries::expression_root_symbol(program, member.receiver),
        tracked,
        receiver_spelling,
        definition,
        entry_valuations,
        poisoned_all,
        poisoned_paths,
        born_zero,
        data_binding,
    );
    let place = &mut tracked[position];
    // A wildcard receiver may name any covered element; this analysis keeps
    // no per-element identity for it.
    let precise_receiver = !place.spelling.contains("[*]");
    let standing = precise_receiver
        && place.established
        && !place.window_open
        && place.element_windows.is_empty()
        && !inherited_windows
            .iter()
            .any(|(spelling, _, _)| *spelling == place.spelling);
    let mut elements = current_elements(program, place, field, standing);
    if let Some(elements) = elements.as_mut() {
        let element_type = element_carrier(program, field.type_reference);
        let stored = match (literal_value(program, value), element_type) {
            (Some(LiteralValue::Integer(stored)), Some(_)) => IntegerSet::singleton(stored),
            (_, Some(carrier)) => {
                proved_stored_set(program, machine, state, statement_index, value)
                    .map(|proved| proved.intersect(&carrier))
                    .unwrap_or(carrier)
            }
            (_, None) => IntegerSet::empty(),
        };
        let literal_index = element_index
            .and_then(|index| program.expression_table.constant_integer_value(index))
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < elements.len());
        match (literal_index, element_index) {
            (Some(index), _) => elements[index] = stored,
            // A computed index may select any element.
            (None, Some(_)) => {
                for element in elements.iter_mut() {
                    *element = element.union(&stored);
                }
            }
            // A nested selector changes some element in an unknown way.
            (None, None) => *elements = unknown_elements(program, field).unwrap_or_default(),
        }
    }
    let all_hold = precise_receiver
        && elements.as_ref().is_some_and(|elements| {
            affected.iter().all(|fact| {
                collection_predicates::data_fact_decided(
                    program,
                    definition,
                    *fact,
                    &mut |argument| {
                        if argument.symbol == field.symbol {
                            return Some(ArgumentKnowledge::Array(elements.clone()));
                        }
                        other_field_knowledge(program, place, argument, standing)
                    },
                ) == Some(true)
            })
        });
    place.elements.retain(|(symbol, _)| *symbol != field.symbol);
    if let Some(elements) = elements {
        place.elements.push((field.symbol, elements));
    }
    place
        .element_windows
        .retain(|symbol| *symbol != field.symbol);
    if !all_hold {
        place.element_windows.push(field.symbol);
    }
    true
}

fn innermost_member(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut current = expression;
    for _ in 0..32 {
        match program.expression_table.expression(current) {
            ExpressionNode::Indexed(indexed) => current = indexed.collection,
            ExpressionNode::Member(_) => return Some(current),
            _ => return None,
        }
    }
    None
}

fn data_field<'program>(
    program: &'program dyn crate::CheckingInput,
    definition: &DataDefinition,
    name: &str,
) -> Option<&'program DataField> {
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => Some(field),
            _ => None,
        })
}

/// Whether `expression` reads the contents of the field named `field`: any
/// occurrence other than the receiver of a collection measure.
fn reads_field_contents(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    field: &str,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let node = program.expression_table.expression(expression);
    match node {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == field),
        ExpressionNode::Member(member)
            if language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                member.member.as_str(),
            )
            .is_some()
                && matches!(
                    program.expression_table.expression(member.receiver),
                    ExpressionNode::Name(_)
                ) =>
        {
            false
        }
        _ => {
            let mut reads = false;
            crate::expression_traversal::children(program, node, |child| {
                reads = reads || reads_field_contents(program, child, field);
            });
            reads
        }
    }
}

/// The integer set flow has already proved for `value` at this statement.
/// `None` when the occurrence has no closed bounds; the caller then keeps
/// the element carrier. This is the same observation exact arithmetic uses,
/// so a cited element theorem, a bounded parameter, a local, and a call
/// result all restore a collection predicate through one store.
fn proved_stored_set(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
) -> Option<IntegerSet> {
    let (low, high) =
        crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            value,
        )?;
    Some(IntegerSet::range(i128::from(low), i128::from(high)))
}

fn element_carrier(
    program: &dyn crate::CheckingInput,
    field_type: typed_trees::types::TypeReferenceHandle,
) -> Option<IntegerSet> {
    match unknown_knowledge(program, field_type)? {
        ArgumentKnowledge::Array(elements) => elements.into_iter().next(),
        _ => None,
    }
}

fn unknown_elements(
    program: &dyn crate::CheckingInput,
    field: &DataField,
) -> Option<Vec<IntegerSet>> {
    match unknown_knowledge(program, field.type_reference)? {
        ArgumentKnowledge::Array(elements) => Some(elements),
        _ => None,
    }
}

/// The element knowledge of an array field before this store: what earlier
/// stores in this state left, else what the standing facts imply, else
/// nothing. `None` when the field is not a fixed integer array.
fn current_elements(
    program: &dyn crate::CheckingInput,
    place: &TrackedPlace<'_>,
    field: &DataField,
    standing: bool,
) -> Option<Vec<IntegerSet>> {
    if let Some((_, elements)) = place
        .elements
        .iter()
        .find(|(symbol, _)| *symbol == field.symbol)
    {
        return Some(elements.clone());
    }
    if standing
        && let Some(elements) =
            collection_predicates::standing_field_elements(program, place.definition, field)
    {
        return Some(elements);
    }
    unknown_elements(program, field)
}

/// Knowledge of another field a re-proved fact reads.
fn other_field_knowledge(
    program: &dyn crate::CheckingInput,
    place: &TrackedPlace<'_>,
    field: &DataField,
    standing: bool,
) -> Option<ArgumentKnowledge> {
    if let Some(elements) = current_elements(program, place, field, standing)
        && !elements.is_empty()
    {
        return Some(ArgumentKnowledge::Array(elements));
    }
    let ArgumentKnowledge::Integer(carrier) = unknown_knowledge(program, field.type_reference)?
    else {
        return None;
    };
    let known = place
        .fields
        .iter()
        .find(|(name, _)| name == field.name.as_str())
        .map(|(_, value)| value.and_then(LiteralValue::integer));
    Some(ArgumentKnowledge::Integer(match known {
        Some(Some(value)) => IntegerSet::singleton(value),
        // A born-zero place reads zero in every field it has not written.
        None if place.born_zero => IntegerSet::singleton(0),
        _ => carrier,
    }))
}
