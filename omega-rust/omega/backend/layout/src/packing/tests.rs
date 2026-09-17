//! Packing arithmetic tests: every placement step rejects an overflow of the
//! addressable size, and a maximal-but-valid layout still packs.

use super::{PlannedField, align_to, pack_fields, pack_fields_at, place_fields_by_plan};
use crate::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use arena::Arena;
use checked_trees::name::Identifier;
use std::sync::Arc;
use symbols::SymbolHandle;

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
    // The witnessed defect: `storage: [u8; u64::MAX]` occupies every byte, so
    // aligning the following `length: u64` field's offset wraps.
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
fn field_offset_that_would_wrap_is_rejected() {
    let mut storage = Arena::new();
    let error = pack_fields_at(&mut storage, [planned("tail", 8, 1)], usize::MAX - 3)
        .expect_err("a field ending past usize::MAX must be rejected");
    assert!(
        error.message.contains("overflows the addressable size")
            && error.message.contains("field `tail` of 8 byte(s)"),
        "{}",
        error.message
    );
}

#[test]
fn record_extent_alignment_that_would_wrap_is_rejected() {
    // Both fields place, but rounding the record's end up to its 8-byte
    // alignment passes usize::MAX.
    let mut storage = Arena::new();
    let error = pack_fields(
        &mut storage,
        [planned("word", 8, 8), planned("rest", usize::MAX - 9, 1)],
    )
    .expect_err("aligning the record extent past usize::MAX must be rejected");
    assert!(
        error.message.contains("overflows the addressable size")
            && error.message.contains("record extent"),
        "{}",
        error.message
    );
}

#[test]
fn maximal_but_valid_layout_still_packs() {
    // `usize::MAX - 16` bytes, then a u64: the aligned offset is
    // `usize::MAX - 15`, its end `usize::MAX - 7`, which is already a multiple
    // of 8, so the record fills the addressable size exactly up to the last
    // aligned extent.
    let mut storage = Arena::new();
    let (span, layout) = pack_fields(
        &mut storage,
        [planned("bulk", usize::MAX - 16, 1), planned("word", 8, 8)],
    )
    .expect("a layout ending on the last aligned extent packs");
    assert_eq!(offsets(&storage, span), [0, usize::MAX - 15]);
    assert_eq!(
        layout,
        TypeLayout {
            size: usize::MAX - 7,
            alignment: 8
        }
    );

    // A single unaligned field may occupy the whole addressable size.
    let mut storage = Arena::new();
    let (span, layout) = pack_fields(&mut storage, [planned("all", usize::MAX, 1)])
        .expect("a full-width byte array alone packs");
    assert_eq!(offsets(&storage, span), [0]);
    assert_eq!(
        layout,
        TypeLayout {
            size: usize::MAX,
            alignment: 1
        }
    );
}

#[test]
fn plan_directed_placement_rejects_arity_mismatch_and_wrapping_fields() {
    let layout = TypeLayout {
        size: 16,
        alignment: 8,
    };
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
