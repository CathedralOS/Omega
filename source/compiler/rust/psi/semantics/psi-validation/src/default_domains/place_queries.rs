//! Place spelling, data-schema resolution, and standing-fact queries.
//!
//! These are read-only structural queries shared by the default-domain write
//! engine and reader hypotheses. They do not own flow state or diagnostics.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataDefinition;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
                || crate::data::data_requires_establishment(program, definition)
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
        let mut collection_type =
            crate::places::declared_place_type_raw(program, machine, state, indexed.collection)?;
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
    let receiver_type = crate::places::declared_place_type(program, machine, state, expression)?;
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
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .any(|fact| match fact {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_name(program, *expression, field)
            }
            psi_typed_trees::domain::ProofFact::Membership(membership) => {
                membership_field_name(program, membership.value) == Some(field)
            }
            psi_typed_trees::domain::ProofFact::Proposition(_) => false,
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
        _ => false,
    }
}
