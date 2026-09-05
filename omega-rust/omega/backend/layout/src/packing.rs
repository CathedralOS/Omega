use crate::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use arena::{Arena, HandleSpan};
use checked_trees::name::Identifier;
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Debug)]
pub(super) struct PlannedField {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Arc<str>,
    pub type_descriptor: TypeLayoutDescriptor,
    pub layout: TypeLayout,
}

pub(super) fn pack_fields(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
) -> (HandleSpan<FieldLayout>, TypeLayout) {
    pack_fields_at(field_storage, fields, 0)
}

/// Pack `fields` sequentially starting at `base_offset` (used for case payload
/// regions, which start after the tag). The returned `TypeLayout::size` is the
/// ABSOLUTE aligned end offset (prefix included), so a caller overlaying
/// several cases takes the maximum across them.
pub(super) fn pack_fields_at(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
    base_offset: usize,
) -> (HandleSpan<FieldLayout>, TypeLayout) {
    let mut offset = base_offset;
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

/// Place `fields` at PRE-VALIDATED plan offsets (plan-laid value types,
/// layouts L4). The caller has checked arity; the plan pipeline validated
/// bounds, overlap, and alignment, so this is pure transcription -- the plan
/// dictates placement, the packer never second-guesses it.
pub(super) fn place_fields_by_plan(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
    offsets: &[usize],
    layout: TypeLayout,
) -> (HandleSpan<FieldLayout>, TypeLayout) {
    let mut next = 0usize;
    let placed_fields = field_storage.insert_many(fields.into_iter().map(|field| {
        let offset = offsets[next];
        next += 1;

        FieldLayout {
            symbol: field.symbol,
            name: field.name,
            offset,
            type_symbol: field.type_symbol,
            type_name: field.type_name,
            type_descriptor: field.type_descriptor,
            layout: field.layout,
        }
    }));

    (placed_fields, layout)
}

fn align_to(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        value
    } else {
        value.div_ceil(alignment) * alignment
    }
}
