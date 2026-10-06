//! Whole-type shape walks shared by program-scope judgments.

use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Whether `reference` carries a slice base under any reference or
/// constrained shells.
pub(crate) fn has_slice_base(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
    false
}

/// The scalar type pair a required/actual parameter-kind pair carries when
/// both are `Const` or both are `Value`.
pub(crate) fn scalar_parameter_type_pair(
    required: &TypeParameterKind,
    actual: &TypeParameterKind,
) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
    match (required, actual) {
        (
            TypeParameterKind::Const {
                type_reference: required_type,
            },
            TypeParameterKind::Const {
                type_reference: actual_type,
            },
        )
        | (
            TypeParameterKind::Value {
                type_reference: required_type,
            },
            TypeParameterKind::Value {
                type_reference: actual_type,
            },
        ) => Some((*required_type, *actual_type)),
        _ => None,
    }
}

/// A mutable actual reference may satisfy a shared required reference:
/// compare their referents for that one permitted attenuation.
pub(crate) fn attenuated_reference_pair(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> (TypeReferenceHandle, TypeReferenceHandle) {
    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual,
                access: language_semantics::ReferenceAccess::Mutable,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required,
                access: language_semantics::ReferenceAccess::Shared,
                ..
            },
        ) => (*actual, *required),
        _ => (actual, required),
    }
}

/// Every `Range` constraint `reference` declares under `Constrained` shells,
/// outermost first: `(minimum, maximum, end_inclusive)`.
pub(crate) fn range_constraints(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Vec<(ExpressionHandle, ExpressionHandle, bool)> {
    let mut ranges = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                continue;
            };
            ranges.push((*minimum, *maximum, *end_inclusive));
        }
        reference = *base_type;
    }
    ranges
}

/// `reference` with its `Constrained` shells peeled away.
pub(crate) fn unconstrained_base(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}

/// The data symbol a type reference names through its `Named`/`Generic` head.
pub(crate) fn head_data_symbol(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        _ => None,
    }
}
