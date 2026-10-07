//! Whole-type shape walks shared by program-scope judgments.

use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Whether `reference` carries a slice base under any reference or
/// constrained shells.

/// `Some(primitive)` when the type reference names a fixed integer primitive
/// in the Exact domain — the shared peel the bound walks apply before they
/// read an interval. `allow_addr` admits `Addr` as the pointer-width
/// unsigned member of the set.
pub(crate) fn exact_integer_primitive(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    allow_addr: bool,
) -> Option<PrimitiveType> {
    let primitive = program
        .type_reference_table
        .primitive_type(type_reference)?;
    let fixed = matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || (allow_addr && primitive == PrimitiveType::Addr);
    if !fixed
        || program.arithmetic_domain_for_type_reference(type_reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some(primitive)
}

/// A fixed-array or slice type reference's element type plus its
/// statically-known element count (`None` for slices and non-literal array
/// lengths); any other shape has neither.
pub(crate) fn collection_element_and_length(
    program: &TypedTrees,
    collection_type: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Option<usize>)> {
    match program.type_reference_table.type_reference(collection_type) {
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => Some((
            *element_type,
            match length {
                typed_trees::types::FixedArrayLength::Literal(length) => Some(*length),
                _ => None,
            },
        )),
        TypeReferenceNode::Slice { element_type } => Some((*element_type, None)),
        _ => None,
    }
}

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

/// The constraint slices of each `Constrained` shell `handle` carries,
/// reached through `Reference` peels, outermost first. `None` when any shell
/// demands a non-`Exact` arithmetic domain.
pub(crate) fn exact_constrained_shells<'p>(
    program: &'p TypedTrees,
    mut handle: TypeReferenceHandle,
) -> Option<Vec<&'p [TypeConstraintNode]>> {
    let mut shells = Vec::new();
    loop {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => handle = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = program.type_reference_table.constraints(*constraints);
                if constraints.iter().any(|constraint| {
                    matches!(
                        constraint,
                        TypeConstraintNode::ArithmeticDomain(domain)
                            if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                    )
                }) {
                    return None;
                }
                shells.push(constraints);
                handle = *base_type;
            }
            _ => return Some(shells),
        }
    }
}
