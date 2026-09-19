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
                "leaf is an unrestricted primitive, a closed literal-ranged integer primitive proven in range at the store, an integer primitive carrying only an arithmetic-policy constraint, or a carrier qualified only by plain declared domains whose membership is proven on the stored value, a whole eligible unrestricted record or closed material `[copy]` sum, or a recursively literal fixed array whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums"
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
fn policy_qualified_record_field_write_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Limited { value: u32 in Wrapping; depth: i32 in Wrapping; }
            data Outer { inner: Limited; }

            machine replace(limited: &write Limited, next: u32 in Wrapping) {
                limited.value = next;
                limited.depth = 4;
            }

            machine fill(outer: &write Outer, next: u32 in Wrapping) {
                outer.inner.value = next;
            }
        "#,
    ))
    .expect("an arithmetic-policy-only integer leaf carries no membership obligation");
}

#[test]
fn literal_indexed_policy_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner [copy] { value: u32 in Wrapping; }
            data Outer { items: [Inner; 2]; }

            machine fill(outer: &write Outer, next: u32 in Wrapping) {
                outer.items[1].value = next;
            }
        "#,
    ))
    .expect("a policy-qualified field beneath a literal fixed-array element should lower");
}

#[test]
fn policy_qualified_record_field_mismatched_store_remains_rejected() {
    // The admitted leaf keeps every value-level obligation: storing a
    // differently-policed value still drops a semantic atom, which requires
    // an explicit `as`.
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u32 in Wrapping; }

            machine replace(limited: &write Limited, next: u32 in Saturating) {
                limited.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("implicit domain weakening") && rendered.contains("drops `Saturating`"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn float_policy_record_field_remains_rejected() {
    // The admitted policy leaf is an INTEGER primitive: float policies
    // (`f64 in Saturating`) stay outside the envelope.
    let rendered = rendered_rejection(
        r#"
            data Reading { value: f64 in Saturating; }

            machine replace(reading: &write Reading, next: f64 in Saturating) {
                reading.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("arithmetic-policy constraint"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn domain_qualified_record_field_is_writable() {
    // A plain declared domain on the leaf (`[u8; 8] in Utf8`) is a membership
    // predicate on the whole incoming value. Whole-leaf replacement displaces
    // the complete carrier footprint, and the ordinary write-side domain
    // check re-derives `in Utf8` from the field declaration and discharges it
    // against the stored value — the same obligation a `&mut` store owes.
    lower_typed_trees(typed(
        r#"
            domain [u8; 8]::Utf8
            requires
                valid_utf8(self);

            data Limited { label: [u8; 8] in Utf8; }
            data Outer { inner: Limited; }

            machine replace(limited: &write Limited, next: [u8; 8] in Utf8) {
                limited.label = next;
            }

            machine forward(outer: &write Outer, next: [u8; 8] in Utf8) {
                outer.inner.label = next;
            }
        "#,
    ))
    .expect("a store of a domain-proven value into a plain domain leaf should lower");
}

#[test]
fn domain_qualified_record_field_unproven_value_remains_rejected() {
    // Admission keeps every value-level obligation: a value that does not
    // already carry the declared domain still fails the write-side check.
    let rendered = rendered_rejection(
        r#"
            domain [u8; 8]::Utf8
            requires
                valid_utf8(self);

            data Limited { label: [u8; 8] in Utf8; }

            machine replace(limited: &write Limited, next: [u8; 8]) {
                limited.label = next;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove the value assigned to `limited.label`")
            && rendered.contains("requires every write to be established in that domain"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn domain_qualified_record_field_element_remains_rejected() {
    // An element store cannot re-establish whole-value domain membership, so
    // a partial write into a domain-qualified carrier stays outside the
    // envelope even though a whole-leaf store is admitted.
    let rendered = rendered_rejection(
        r#"
            domain [u8; 8]::Utf8
            requires
                valid_utf8(self);

            data Limited { label: [u8; 8] in Utf8; }

            machine replace(limited: &write Limited) {
                limited.label[0] = 120;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("element and range stores into a domain-qualified carrier"),
        "unexpected diagnostic: {rendered}"
    );
}
