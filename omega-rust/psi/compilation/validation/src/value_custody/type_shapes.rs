//! Whole-type shape walks shared by source custody and contract judgments.

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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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

pub(crate) fn has_slice_base(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> bool {
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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

/// The data symbol a type reference names through its `Named`/`Generic` head.
pub(crate) fn head_data_symbol(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    program.type_reference_table.type_head_symbol(reference)
}

/// The constraint slices of each `Constrained` shell `handle` carries,
/// reached through `Reference` peels, outermost first. `None` when any shell
/// demands a non-`Exact` arithmetic domain.
pub fn exact_constrained_shells<'p>(
    program: &'p dyn crate::CheckingInput,
    mut handle: TypeReferenceHandle,
) -> Option<Vec<&'p [TypeConstraintNode]>> {
    let mut shells = Vec::new();
    loop {
        handle = program
            .type_reference_table
            .dereferenced_type_reference(handle);
        match program.type_reference_table.type_reference(handle) {
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

/// The symbolic Range maximum `(field, offset)` of a type reference's
/// `Constrained` shells, outermost first, under an Exact arithmetic domain
/// only. `None` when a shell demands a non-Exact domain or no Range names a
/// symbolic bound.
pub fn exact_shells_symbolic_range_maximum<'p>(
    program: &'p dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<(typed_trees::name::Identifier, i64)> {
    use typed_trees::types::TypeConstraintNode;
    exact_constrained_shells(program, handle)?
        .iter()
        .flat_map(|constraints| constraints.iter())
        .find_map(|constraint| match constraint {
            TypeConstraintNode::Range {
                maximum,
                end_inclusive,
                ..
            } => {
                let typed_trees::dependent_ranges::DependentBoundClass::SelfField(symbolic) =
                    typed_trees::dependent_ranges::dependent_range_maximum(
                        program,
                        *maximum,
                        *end_inclusive,
                    )?
                else {
                    return None;
                };
                Some((symbolic.field, symbolic.offset))
            }
            _ => None,
        })
}
