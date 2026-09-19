use super::{rendered_rejection, typed};
use crate::lower_typed_trees;

#[test]
fn record_path_fixed_byte_array_literal_element_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[0] = 1;
            }
        "#,
    ))
    .expect("an in-bounds literal byte element behind an eligible record path should lower");
}

#[test]
fn record_path_fixed_byte_array_out_of_bounds_literal_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[4] = 1;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains(
                "proven-in-bounds element or statically normalized closed range of such a fixed array"
            ),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_proven_dynamic_element_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer, index: u64 [0..=3]) {
                outer.inner.bytes[index] = 1;
            }
        "#,
    ))
    .expect("a proven in-bounds dynamic byte element behind an eligible record path should lower");
}

#[test]
fn record_path_fixed_byte_array_unproved_dynamic_element_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer, index: u64) {
                outer.inner.bytes[index] = 1;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove index `index` is within length 4"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_dynamic_index_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner {
                bytes: [u8; 4];
                selected: u8 [0..=3];
            }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[outer.inner.selected] = 1;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `selected` from write-only parameter `outer`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_static_range_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[1..3] = [1, 2];
            }
        "#,
    ))
    .expect("a statically normalized byte range behind an eligible record path should lower");
}

#[test]
fn record_path_fixed_byte_array_symbolic_range_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer, start: u64 [0..=2]) {
                outer.inner.bytes[start..3] = [1, 2, 3];
            }
        "#,
    );
    assert!(
        rendered.contains("bounds are not statically known")
            && rendered.contains("requires literal bounds"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_open_ended_range_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[1..] = [1, 2, 3];
            }
        "#,
    );
    assert!(
        rendered.contains("omitted end") && rendered.contains("statically known end bound"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_range_nonliteral_rhs_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer, replacement: [u8; 2]) {
                outer.inner.bytes[1..3] = replacement;
            }
        "#,
    );
    assert!(
        rendered.contains("from a non-literal value")
            && rendered.contains("array literal of 2 element(s)"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_out_of_bounds_range_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[3..5] = [1, 2];
            }
        "#,
    );
    assert!(
        rendered.contains("range") && rendered.contains("length 4"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_reversed_range_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[3..1] = [1, 2];
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove subslice range ordering `3..1`")
            && rendered.contains("slice length 4"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_range_bound_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner {
                bytes: [u8; 4];
                start: u8 [0..=2];
            }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[outer.inner.start..3] = [1, 2, 3];
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `start` from write-only parameter `outer`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_array_range_rhs_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.bytes[1..3] = [outer.inner.bytes[0], 2];
            }
        "#,
    );
    assert!(
        rendered.contains("reads through index projection of write-only parameter `outer`")
            && rendered.contains("never observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn non_discardable_record_leaf_write_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Receipt [linear] { code: u8; }
            data Holder { receipt: Receipt; }

            machine replace(holder: &write Holder, next: Receipt) {
                holder.receipt = move next;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains(
                "leaf is an unrestricted primitive or a closed literal-ranged integer primitive proven in range at the store, a whole eligible unrestricted record or closed material `[copy]` sum, or a recursively literal fixed array whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums"
            ),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn nested_invariant_bearing_record_field_write_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner
            where
                value <= limit,
            {
                value: u8;
                limit: u8;
            }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.value = 1;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("invariant-free records"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn closed_ranged_record_field_write_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Limited { value: u8 [0..=10]; }
            data Outer { inner: Limited; }

            machine replace(limited: &write Limited, next: u8 [0..=10]) {
                limited.value = next;
            }

            machine fill(outer: &write Outer) {
                outer.inner.value = 4;
            }
        "#,
    ))
    .expect("a store proven within a field's closed literal integer range is a write-only place");
}

#[test]
fn literal_indexed_closed_ranged_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner [copy] { value: u8 [0..=10]; }
            data Outer { items: [Inner; 2]; }

            machine fill(outer: &write Outer, next: u8 [0..=10]) {
                outer.items[1].value = next;
            }
        "#,
    ))
    .expect("a closed-ranged field beneath a literal fixed-array element should lower");
}

#[test]
fn closed_ranged_record_field_out_of_range_literal_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u8 [0..=10]; }

            machine fill(limited: &write Limited) {
                limited.value = 20;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove assignment value `20`")
            && rendered.contains("expected 0..=10"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn closed_ranged_record_field_unproven_value_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u8 [0..=10]; }

            machine replace(limited: &write Limited, next: u8) {
                limited.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove assignment value `next`")
            && rendered.contains("expected 0..=10"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn closed_ranged_record_field_wider_source_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u8 [0..=10]; }

            machine replace(limited: &write Limited, next: u8 [0..=11]) {
                limited.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove assignment value `next`")
            && rendered.contains("expected 0..=10"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn policy_qualified_record_field_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u32 in Wrapping; }

            machine replace(limited: &write Limited, next: u32 in Wrapping) {
                limited.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("closed literal-ranged integer primitive"),
        "unexpected diagnostic: {rendered}"
    );
}
