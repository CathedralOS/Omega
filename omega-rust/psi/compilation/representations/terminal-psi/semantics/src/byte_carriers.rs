//! Fixed-scalar record layout over a byte carrier.
//!
//! A record loan through a byte recast (`&self.buf[0] as &Desc`) mints no
//! storage: the checked recast judgment proved the record's representation
//! fits inside the byte region the captured index begins, and consumers see
//! an `Unrestricted` leaf copy of the record the region overlays. This module
//! is the shared shape of that overlay. Closed authored placement and ordinary
//! packing use the same storage geometry query; an unresolved target-family
//! application supplies neither offsets nor a byte extent. Custody and the
//! checked fit judgment remain independent from this decoding schedule.

use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration};

/// One scalar leaf of a byte-carrier record: the declared field identity,
/// its byte offset inside the overlaid region, and its leaf scalar type.
pub struct ByteCarrierScalarLeaf {
    pub field: StructuralFieldId,
    pub offset: u64,
    pub scalar_type: ScalarType,
}

/// The complete fixed layout a byte-carrier record occupies: scalar leaves
/// at their closed offsets plus the complete region extent, including authored
/// padding, so a consumer checks fit and decodes with the same geometry.
pub struct ByteCarrierLayout {
    pub leaves: Vec<ByteCarrierScalarLeaf>,
    pub size: u64,
}

fn leaf_scalar_carriers(field: &StructuralFieldDeclaration) -> Option<(u64, ScalarType)> {
    match field.field_type {
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => (integer.bits() % 8 == 0)
            .then(|| (u64::from(integer.bits() / 8), ScalarType::Integer(integer))),
        StructuralFieldType::Scalar(ScalarType::Boolean) => Some((1, ScalarType::Boolean)),
        StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            (integer.bits() % 8 == 0)
                .then(|| (u64::from(integer.bits() / 8), ScalarType::Integer(integer)))
        }
        _ => None,
    }
}

/// The byte-carrier layout of a plain record whose non-erased fields are
/// all fixed-width scalars; anything richer — structural children, byte
/// sequences, erased members, floats — has no fixed overlay this vocabulary
/// admits, so `None` refuses it rather than guessing a layout.
pub fn byte_carrier_scalar_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<ByteCarrierLayout> {
    let declaration = types.clone().find(|item| item.id == root)?;
    let size = crate::reference_storage_size(types.clone(), root)?;
    let Some(fields) = declaration.shape.record_fields() else {
        return None;
    };
    let mut leaves = Vec::with_capacity(fields.len());
    for field in fields {
        if field.relevance.is_erased() {
            return None;
        }
        let (_, scalar_type) = leaf_scalar_carriers(field)?;
        let projection = crate::reference_storage_projection(
            types.clone(),
            root,
            &[terminal_psi::StructuralPathSegment::Field(
                field.identity.clone(),
            )],
        )?;
        if projection.logical_scalar != scalar_type || projection.stored_scalar != scalar_type {
            return None;
        }
        leaves.push(ByteCarrierScalarLeaf {
            field: field.id,
            offset: projection.byte_offset,
            scalar_type,
        });
    }
    Some(ByteCarrierLayout { leaves, size })
}
