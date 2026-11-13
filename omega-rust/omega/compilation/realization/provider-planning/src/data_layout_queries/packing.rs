// We implement sequential field packing (`pack_fields` and `pack_fields_at`)
// and plan-directed field placement (`place_fields_by_plan`). Every address
// arithmetic step over `usize`—aligned field offset, field end offset, and
// aligned record extent—is checked so an extent exceeding the host's
// addressable size produces a clean `Diagnostic` rejection rather than a
// wrapped offset or compiler panic.

use crate::data_layout_queries::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use arena::{Arena, HandleSpan};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::name::Identifier;

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
        crate::data_layout_queries::type_layout(size, max_alignment),
    ))
}

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

pub(crate) fn align_to(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        Some(value)
    } else {
        value.div_ceil(alignment).checked_mul(alignment)
    }
}

pub(crate) fn placement_overflow(step: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(format!(
        "layout placement overflows the addressable size: {step}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{PlannedField, align_to, pack_fields, place_fields_by_plan};
    use crate::data_layout_queries::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
    use arena::Arena;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::name::Identifier;

    fn planned(name: &str, size: usize, alignment: usize) -> PlannedField {
        PlannedField {
            symbol: SymbolHandle::invalid(),
            name: Identifier::from(name),
            type_symbol: SymbolHandle::invalid(),
            type_name: Arc::from("u8"),
            type_descriptor: TypeLayoutDescriptor::Unit,
            layout: TypeLayout { size, alignment },
        }
    }

    fn offsets(storage: &Arena<FieldLayout>, span: arena::HandleSpan<FieldLayout>) -> Vec<usize> {
        storage
            .span_or_empty(span)
            .iter()
            .map(|field| field.offset)
            .collect()
    }

    #[test]
    fn align_to_rejects_a_round_up_past_the_addressable_size() {
        assert_eq!(align_to(0, 8), Some(0));
        assert_eq!(align_to(13, 8), Some(16));
        assert_eq!(align_to(usize::MAX, 0), Some(usize::MAX));
        assert_eq!(align_to(usize::MAX - 8, 8), Some(usize::MAX - 7));
        assert_eq!(align_to(usize::MAX - 7, 8), Some(usize::MAX - 7));
        assert_eq!(align_to(usize::MAX - 6, 8), None);
        assert_eq!(align_to(usize::MAX, 2), None);
    }

    #[test]
    fn full_width_array_extent_followed_by_an_aligned_field_is_rejected() {
        let mut storage = Arena::new();
        let error = pack_fields(
            &mut storage,
            [planned("storage", usize::MAX, 1), planned("length", 8, 8)],
        )
        .expect_err("aligning past the full-width array must be rejected");
        assert!(
            error.message.contains("overflows the addressable size")
                && error.message.contains("aligning field `length`"),
            "{}",
            error.message
        );
        assert_eq!(storage.len(), 0, "a rejected placement records no fields");
    }

    #[test]
    fn maximal_but_valid_layout_still_packs() {
        let mut storage = Arena::new();
        let (span, layout) = pack_fields(
            &mut storage,
            [planned("bulk", usize::MAX - 16, 1), planned("word", 8, 8)],
        )
        .expect("a layout ending on the last aligned extent packs");
        assert_eq!(offsets(&storage, span), [0, usize::MAX - 15]);
        assert_eq!(
            layout,
            crate::data_layout_queries::type_layout(usize::MAX - 7, 8)
        );

        let mut storage = Arena::new();
        let (span, layout) = pack_fields(&mut storage, [planned("all", usize::MAX, 1)])
            .expect("a full-width byte array alone packs");
        assert_eq!(offsets(&storage, span), [0]);
        assert_eq!(
            layout,
            crate::data_layout_queries::type_layout(usize::MAX, 1)
        );
    }

    #[test]
    fn plan_directed_placement_rejects_arity_mismatch_and_wrapping_fields() {
        let layout = crate::data_layout_queries::type_layout(16, 8);
        let mut storage = Arena::new();
        let (span, placed) = place_fields_by_plan(
            &mut storage,
            [planned("first", 4, 4), planned("second", 8, 8)],
            &[8, 0],
            layout,
        )
        .expect("validated plan offsets transcribe");
        assert_eq!(offsets(&storage, span), [8, 0]);
        assert_eq!(placed, layout);

        let mut storage = Arena::new();
        let error = place_fields_by_plan(
            &mut storage,
            [planned("first", 4, 4), planned("second", 8, 8)],
            &[0],
            layout,
        )
        .expect_err("fewer offsets than fields must be rejected");
        assert!(error.message.contains("offset(s)"), "{}", error.message);

        let mut storage = Arena::new();
        let error = place_fields_by_plan(&mut storage, [planned("first", 4, 4)], &[0, 8], layout)
            .expect_err("more offsets than fields must be rejected");
        assert!(error.message.contains("offset(s)"), "{}", error.message);

        let mut storage = Arena::new();
        let error = place_fields_by_plan(
            &mut storage,
            [planned("first", 8, 8)],
            &[usize::MAX - 4],
            layout,
        )
        .expect_err("a plan-laid field ending past usize::MAX must be rejected");
        assert!(
            error.message.contains("overflows the addressable size"),
            "{}",
            error.message
        );
    }
}
