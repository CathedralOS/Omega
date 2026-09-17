//! Sequential field packing and plan-directed field placement.
//!
//! Every placement is address arithmetic over `usize`: a field's aligned
//! offset, its end offset, and the record's aligned extent. A program can
//! spell an extent the host cannot represent (a `[u8; Capacity]` bound to
//! `u64::MAX`, or a record whose aligned end passes `usize::MAX`), so every
//! step is checked and an overflow is a `Diagnostic` rejection of the
//! placement, never a wrapped offset or a compiler panic.

use crate::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use arena::{Arena, HandleSpan};
use checked_trees::name::Identifier;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

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
) -> Result<(HandleSpan<FieldLayout>, TypeLayout), Diagnostic> {
    pack_fields_at(field_storage, fields, 0)
}

/// Pack `fields` sequentially starting at `base_offset` (used for case payload
/// regions, which start after the tag). The returned `TypeLayout::size` is the
/// ABSOLUTE aligned end offset (prefix included), so a caller overlaying
/// several cases takes the maximum across them.
///
/// Rejects the placement when a field's aligned offset, a field's end offset,
/// or the aligned record extent overflows the addressable size.
pub(super) fn pack_fields_at(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
    base_offset: usize,
) -> Result<(HandleSpan<FieldLayout>, TypeLayout), Diagnostic> {
    let fields = fields.into_iter();
    let mut placed = Vec::with_capacity(fields.size_hint().0);
    let mut offset = base_offset;
    let mut max_alignment = 1;

    for field in fields {
        let field_offset = align_to(offset, field.layout.alignment).ok_or_else(|| {
            placement_overflow(format!(
                "aligning field `{}` at offset {offset} to {} byte(s)",
                field.name, field.layout.alignment
            ))
        })?;
        max_alignment = max_alignment.max(field.layout.alignment);
        let layout = field.layout;
        offset = field_offset.checked_add(layout.size).ok_or_else(|| {
            placement_overflow(format!(
                "field `{}` of {} byte(s) at offset {field_offset}",
                field.name, layout.size
            ))
        })?;

        placed.push(FieldLayout {
            symbol: field.symbol,
            name: field.name,
            offset: field_offset,
            type_symbol: field.type_symbol,
            type_name: field.type_name,
            type_descriptor: field.type_descriptor,
            layout,
        });
    }

    let size = align_to(offset, max_alignment).ok_or_else(|| {
        placement_overflow(format!(
            "aligning the {offset}-byte record extent to {max_alignment} byte(s)"
        ))
    })?;

    Ok((
        field_storage.insert_many(placed),
        TypeLayout {
            size,
            alignment: max_alignment,
        },
    ))
}

/// Place `fields` at PRE-VALIDATED plan offsets (plan-laid value types,
/// layouts L4). The plan pipeline validated bounds, overlap, and alignment, so
/// this is transcription -- the plan dictates placement, the packer never
/// second-guesses it. It still refuses a plan whose offset count disagrees
/// with the field inventory or whose field would end past the addressable
/// size, since neither can be a validated plan.
pub(super) fn place_fields_by_plan(
    field_storage: &mut Arena<FieldLayout>,
    fields: impl IntoIterator<Item = PlannedField>,
    offsets: &[usize],
    layout: TypeLayout,
) -> Result<(HandleSpan<FieldLayout>, TypeLayout), Diagnostic> {
    let fields = fields.into_iter();
    let mut placed = Vec::with_capacity(fields.size_hint().0);
    for field in fields {
        let Some(&offset) = offsets.get(placed.len()) else {
            return Err(Diagnostic::error(format!(
                "plan-laid placement supplies {} offset(s) for more than {} field(s)",
                offsets.len(),
                placed.len()
            )));
        };
        if offset.checked_add(field.layout.size).is_none() {
            return Err(placement_overflow(format!(
                "plan-laid field `{}` of {} byte(s) at offset {offset}",
                field.name, field.layout.size
            )));
        }
        placed.push(FieldLayout {
            symbol: field.symbol,
            name: field.name,
            offset,
            type_symbol: field.type_symbol,
            type_name: field.type_name,
            type_descriptor: field.type_descriptor,
            layout: field.layout,
        });
    }
    if placed.len() != offsets.len() {
        return Err(Diagnostic::error(format!(
            "plan-laid placement supplies {} offset(s) for {} field(s)",
            offsets.len(),
            placed.len()
        )));
    }

    Ok((field_storage.insert_many(placed), layout))
}

/// Round `value` up to a multiple of `alignment`; `None` when the rounded
/// value passes `usize::MAX`. A zero alignment leaves the value unchanged.
pub(crate) fn align_to(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        Some(value)
    } else {
        value.div_ceil(alignment).checked_mul(alignment)
    }
}

/// The rejection for any placement step whose address arithmetic overflows.
pub(crate) fn placement_overflow(step: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!(
        "layout placement overflows the addressable size: {step}"
    ))
}
