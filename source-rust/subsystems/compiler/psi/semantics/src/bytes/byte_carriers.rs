
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
};

pub struct ByteCarrierScalarLeaf {
    pub field: StructuralFieldId,
    pub offset: u64,
    pub scalar_type: ScalarType,
}

pub struct ByteCarrierLayout {
    pub leaves: Vec<ByteCarrierScalarLeaf>,
    pub size: u64,
}

impl ByteCarrierLayout {
    /// A record occupies its complete layout, including padding after its last field.
    pub fn fits_at(&self, offset: u64, byte_extent: u64) -> bool {
        offset.checked_add(self.size).is_some_and(|end| end <= byte_extent)
    }
}

fn leaf_scalar_carriers(field: &StructuralFieldDeclaration) -> Option<(u64, ScalarType)> {
    match field.field_type {
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let bits = integer.bits()?;
            (bits % 8 == 0).then(|| (u64::from(bits / 8), ScalarType::Integer(integer)))
        }
        StructuralFieldType::Scalar(ScalarType::Boolean) => Some((1, ScalarType::Boolean)),
        StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let bits = integer.bits()?;
            (bits % 8 == 0).then(|| (u64::from(bits / 8), ScalarType::Integer(integer)))
        }
        _ => None,
    }
}

pub fn byte_carrier_scalar_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<ByteCarrierLayout> {
    let declaration = types.clone().find(|item| item.id == root)?;
    let size = crate::reference_storage_size(types.clone(), root)?;
    let fields = declaration.shape.record_fields()?;
    let mut leaves = Vec::with_capacity(fields.len());
    for field in fields {
        if field.relevance.is_erased() {
            return None;
        }
        let (_, scalar_type) = leaf_scalar_carriers(field)?;
        let projection = crate::reference_storage_projection(
            types.clone(),
            root,
            &[terminal_psi::StructuralPathSegment::Field(field.identity)],
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
