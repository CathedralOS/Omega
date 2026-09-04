//! Caller-isolated local and aggregate classification for write-frame
//! inference.
//!
//! These queries decide whether an ordinary value is structurally incapable
//! of carrying caller-visible aliasing. They inspect only checked typed shapes;
//! the initializer query additionally admits only a finite direct-call tree.
//! Frame traversal and complete-or-opaque fallback remain in the parent.

use super::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::struct_literals::construction_field_type;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Traverse direct calls along initializer receiver/argument edges. Pure
/// leaves are neutral; calls hidden under operators, aggregates, or other
/// computed expressions remain outside this relation. The typed expression
/// tree is finite; a worklist handles call nesting without a depth limit.
pub(super) fn isolated_local_initializer_has_direct_call_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !expression_is_effectful_for_transparent_result(program, expression) {
            continue;
        }
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            return false;
        };
        if call.receiver.is_valid() {
            pending.push(call.receiver);
        }
        pending.extend_from_slice(program.expression_table.expression_handles(call.arguments));
    }
    true
}

pub(super) fn struct_literal_field_is_primitive(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    field_name: &str,
) -> bool {
    struct_literal_field_type(program, literal, field_name)
        .is_some_and(|field_type| program.primitive_type_reference(field_type).is_some())
}

pub(super) fn struct_literal_field_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(())?;
    construction_field_type(
        program,
        definition,
        literal.case_name.as_ref().map(|name| name.as_str()),
        field_name,
    )
}

pub(super) fn struct_literal_matches_expected_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    expected_type: TypeReferenceHandle,
) -> bool {
    let Some(expected_type) = crate::places::unwrapped_type_reference(program, expected_type)
    else {
        return false;
    };
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(expected_type)
    else {
        return false;
    };
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let Some(definition) = definitions.next() else {
        return false;
    };
    definitions.next().is_none()
        && definition.type_parameters.is_empty()
        && if symbol.is_valid() {
            definition.symbol == *symbol
        } else {
            definition.name == *name
        }
}

pub(super) fn type_is_caller_isolated_local(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    type_is_caller_isolated_local_inner(program, handle, &mut Vec::new())
}

fn type_is_caller_isolated_local_inner(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if program.primitive_type_reference(handle).is_some() {
        return true;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_is_caller_isolated_local_inner(program, *base_type, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_is_caller_isolated_local_inner(program, *element_type, visiting)
        }
        TypeReferenceNode::Named { symbol, name } => {
            let mut definitions = program.data_definitions().iter().filter(|definition| {
                if symbol.is_valid() {
                    definition.symbol == *symbol
                } else {
                    definition.name == *name
                }
            });
            let Some(definition) = definitions.next() else {
                return false;
            };
            if definitions.next().is_some() {
                return false;
            }
            data_definition_is_caller_isolated(program, definition, visiting)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(super) fn struct_literal_type_is_caller_isolated(
    program: &TypedTrees,
    literal: &TableStructLiteral,
) -> bool {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let Some(definition) = definitions.next() else {
        return false;
    };
    let unique_shape = match literal.case_name.as_ref() {
        None => program
            .data_members(definition)
            .iter()
            .all(|member| matches!(member, DataMember::Field(_))),
        Some(case_name) => {
            let mut variants = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if variant.name == *case_name => Some(variant),
                    _ => None,
                });
            variants.next().is_some() && variants.next().is_none()
        }
    };
    definitions.next().is_none()
        && unique_shape
        && data_definition_is_caller_isolated(program, definition, &mut Vec::new())
}

fn data_definition_is_caller_isolated(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if !definition.type_parameters.is_empty() || visiting.contains(&definition.symbol) {
        return false;
    }
    visiting.push(definition.symbol);
    let isolated = program
        .data_members(definition)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) => {
                type_is_caller_isolated_local_inner(program, field.type_reference, visiting)
            }
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().all(|field| {
                    type_is_caller_isolated_local_inner(program, field.type_reference, visiting)
                })
            }
        });
    visiting.pop();
    isolated
}
