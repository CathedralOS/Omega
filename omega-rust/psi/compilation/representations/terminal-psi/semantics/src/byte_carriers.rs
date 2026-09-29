//! Fixed-scalar record layout over a byte carrier.
//!
//! A record loan through a byte recast (`&self.buf[0] as &Desc`) mints no
//! storage: the checked recast judgment proved the record's representation
//! fits inside the byte region the captured index begins, and consumers see
//! an `Unrestricted` leaf copy of the record the region overlays. This module
//! is the shared shape of that overlay: a record is byte-carrier eligible
//! when every leaf is a fixed-width scalar, and its layout packs the
//! declaration-order leaves at scalar-size alignment — the same algebra the
//! checking side's `record_representation` proves. Verification re-checks the
//! static fit (index plus extent within the array's declared length); the
//! interpreter decodes each leaf little-endian out of consecutive `u8`
//! elements, the host byte order the realized image writes.

use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

/// One scalar leaf of a byte-carrier record: the declared field identity,
/// its byte offset inside the overlaid region, and its leaf scalar type.
pub struct ByteCarrierScalarLeaf {
    pub field: StructuralFieldId,
    pub offset: u64,
    pub scalar_type: ScalarType,
}

/// The complete fixed layout a byte-carrier record occupies: scalar leaves
/// at their computed offsets plus the region's total extent — leaf end
/// aligned up to the widest scalar — so a consumer checks fit with one
/// number and decodes with the leaf schedule.
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
    types: impl Iterator<Item = &'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> Option<ByteCarrierLayout> {
    let declaration = types.into_iter().find(|item| item.id == root)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut leaves = Vec::with_capacity(fields.len());
    let mut offset = 0u64;
    let mut widest = 1u64;
    for field in fields {
        if field.relevance.is_erased() {
            return None;
        }
        let (size, scalar_type) = leaf_scalar_carriers(field)?;
        // Scalar-size alignment matches the checked representation judgment:
        // a leaf's offset rounds up to its own width before it stores.
        offset = offset.checked_add(size)?.checked_sub(1)? / size * size;
        leaves.push(ByteCarrierScalarLeaf {
            field: field.id,
            offset,
            scalar_type,
        });
        offset = offset.checked_add(size)?;
        widest = widest.max(size.max(1));
    }
    let size = offset.checked_add(widest)?.checked_sub(1)? / widest * widest;
    Some(ByteCarrierLayout { leaves, size })
}
