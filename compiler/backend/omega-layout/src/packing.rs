use crate::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use omega_checked_trees::name::ProgramName;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct PlannedField {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_symbol: SymbolHandle,
    pub type_name: Arc<str>,
    pub type_descriptor: TypeLayoutDescriptor,
    pub layout: TypeLayout,
}

pub(super) fn pack_fields(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
) -> (HandleSpan<FieldLayout>, TypeLayout) {
    let mut offset = 0;
    let mut max_alignment = 1;

    let packed_fields = field_storage.insert_many(fields.into_iter().map(|field| {
        offset = align_to(offset, field.layout.alignment);
        max_alignment = max_alignment.max(field.layout.alignment);
        let field_offset = offset;
        let layout = field.layout;
        offset += layout.size;

        FieldLayout {
            symbol: field.symbol,
            name: field.name,
            offset: field_offset,
            type_symbol: field.type_symbol,
            type_name: field.type_name,
            type_descriptor: field.type_descriptor,
            layout,
        }
    }));

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
