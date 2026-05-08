use crate::{FieldLayout, TypeLayout};
use omega_typed_program::name::ProgramName;

#[derive(Debug)]
pub(super) struct PlannedField {
    pub name: ProgramName,
    pub type_name: String,
    pub layout: TypeLayout,
}

pub(super) fn pack_fields(fields: Vec<PlannedField>) -> (Vec<FieldLayout>, TypeLayout) {
    let mut offset = 0;
    let mut max_alignment = 1;
    let mut packed_fields = Vec::new();

    for field in fields {
        offset = align_to(offset, field.layout.alignment);
        max_alignment = max_alignment.max(field.layout.alignment);
        packed_fields.push(FieldLayout {
            name: field.name,
            offset,
            type_name: field.type_name,
            layout: field.layout,
        });
        offset += field.layout.size;
    }

    let size = align_to(offset, max_alignment);

    (
        packed_fields,
        TypeLayout {
            size,
            alignment: max_alignment,
        },
    )
}

fn align_to(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        value
    } else {
        value.div_ceil(alignment) * alignment
    }
}
