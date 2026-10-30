//! Whether a structural storage shape can be established by zeroed bytes.
//!
//! Entry-point receivers and hosted images establish zero-initialized
//! storage. Both verifiers owe the same shape walk: records, fixed arrays,
//! and the first declared case of a closed sum recurse; unknown or
//! duplicated declarations and cycles reject. What differs between bridges
//! is the per-field leaf policy — one bridge admits zero shared-view
//! descriptors, the other rejects every reference — so the field judgment
//! arrives as the caller's predicate.

use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Whether `structural_type`'s storage admits establishment by zeroed bytes,
/// given the module's structural `declarations`. `visiting` is the cycle
/// bound — callers pass a fresh empty vector. `zero_valid_field` is the
/// bridge's own leaf judgment, invoked per declared field.
pub fn zero_valid_record_storage(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    visiting: &mut Vec<StructuralTypeId>,
    zero_valid_field: impl Fn(
        &[StructuralTypeDeclaration],
        &StructuralFieldDeclaration,
        &mut Vec<StructuralTypeId>,
    ) -> bool
    + Copy,
) -> bool {
    let Some(declaration) =
        terminal_psi::unique_structural_declaration(declarations, structural_type, visiting)
    else {
        return false;
    };
    visiting.push(structural_type);
    let valid = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields
            .iter()
            .all(|field| zero_valid_field(declarations, field, visiting)),
        StructuralTypeShape::FixedArray { element, length } => {
            // An empty scalar array retains its schema without constructing a
            // fictional zero-valued element. Unknown and cyclic schemas fail.
            if length.literal() == Some(0)
                && crate::scalar_array_leaf_declaration(declarations.iter(), structural_type)
                    .is_some()
            {
                true
            } else {
                zero_valid_record_storage(declarations, *element, visiting, zero_valid_field)
            }
        }
        StructuralTypeShape::Sum { cases } => cases.first().is_some_and(|case| {
            case.fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
        }),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
                && cases.first().is_some_and(|case| {
                    case.fields
                        .iter()
                        .all(|field| zero_valid_field(declarations, field, visiting))
                })
        }
        StructuralTypeShape::PrimitiveScalar(
            ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
        ) => scalar_declaration_accepts_zero(declaration),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
        _ => false,
    };
    visiting.pop();
    valid
}

/// Zero is a value of this primitive declaration only when its exact retained
/// storage predicate accepts it. Physical carrier width alone grants no value.
pub fn scalar_declaration_accepts_zero(declaration: &StructuralTypeDeclaration) -> bool {
    matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        && declaration
            .scalar_restriction
            .as_ref()
            .is_none_or(crate::scalar_field_accepts_zero)
}
