use super::super::*;

fn typed(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn rendered_rejection(source: &str) -> String {
    lower_typed_trees(typed(source))
        .expect_err("source should be rejected")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn direct_unconstrained_primitive_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Pair {
                left: u8;
                right: u16;
            }

            machine fill(pair: &write Pair) {
                pair.left = 1;
                pair.right = 2;
            }
        "#,
    ))
    .expect("one-level primitive record-field writes should lower");
}

#[test]
fn nested_unconstrained_primitive_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { value: u8; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.value = 1;
            }
        "#,
    ))
    .expect("nested invariant-free record-field writes should lower");
}

#[test]
fn nested_unconstrained_fixed_byte_array_record_field_is_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner {
                bytes: [u8; 4];
                spare: u8;
            }
            data Outer {
                inner: Inner;
                other: Inner;
            }

            machine fill(outer: &write Outer) {
                outer.inner.bytes = [1, 2, 3, 4];
            }
        "#,
    ))
    .expect("a whole fixed byte-array leaf behind a common-field path should lower");
}

#[test]
fn nested_non_byte_array_record_field_write_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { words: [u16; 2]; }
            data Outer { inner: Inner; }

            machine fill(outer: &write Outer) {
                outer.inner.words = [1, 2];
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("whole fixed byte array"),
        "unexpected diagnostic: {rendered}"
    );
}

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
            && rendered.contains("proven-in-bounds element of a fixed byte array"),
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
            && rendered.contains("array literal of 2 byte(s)"),
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
            && rendered.contains("leaf is an unrestricted primitive or whole fixed byte array"),
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
fn constrained_record_field_write_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Limited { value: u8 [0..=10]; }

            machine replace(limited: &write Limited, next: u8 [0..=10]) {
                limited.value = next;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("every field is relevant and unconstrained")
            && rendered.contains("leaf is an unrestricted primitive"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_byte_slice_length_metadata_is_readable() {
    lower_typed_trees(typed(
        r#"
            machine observe_length(bytes: &write [u8]) {
                let length: u64 = bytes.len;
            }
        "#,
    ))
    .expect("the direct write-only byte-slice descriptor length is metadata, not content");
}

#[test]
fn direct_write_only_byte_slice_other_metadata_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine observe_capacity(bytes: &write [u8]) {
                let capacity: u64 = bytes.capacity;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `capacity` from write-only parameter `bytes`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_fixed_array_length_remains_outside_slice_metadata_rung() {
    let rendered = rendered_rejection(
        r#"
            machine observe_length(bytes: &write [u8; 4]) {
                let length: u64 = bytes.len;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `len` from write-only parameter `bytes`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_record_field_named_len_remains_content() {
    let rendered = rendered_rejection(
        r#"
            data Header { len: u64; }

            machine observe_length(header: &write Header) {
                let length: u64 = header.len;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `len` from write-only parameter `header`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn nested_write_only_slice_descriptor_length_remains_content_driven() {
    let rendered = rendered_rejection(
        r#"
            data Holder<'data> { view: &'data [u8]; }

            machine observe_length<'data>(holder: &write Holder<'data>) {
                let length: u64 = holder.view.len;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `len` from write-only parameter `holder`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_byte_slice_content_read_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine observe(bytes: &write [u8], index: u64 [0..bytes.len]) {
                let byte: u8 = bytes[index];
            }
        "#,
    );
    assert!(
        rendered.contains("reads through index projection of write-only parameter `bytes`")
            && rendered.contains("never observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_record_field_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Pair { left: u8; right: u8; }

            machine observe(pair: &write Pair) {
                let prior: u8 = pair.left;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `left` from write-only parameter `pair`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn record_path_fixed_byte_element_rhs_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner { bytes: [u8; 4]; }
            data Outer { inner: Inner; }

            machine copy(outer: &write Outer) {
                outer.inner.bytes[0] = outer.inner.bytes[1];
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
fn whole_affine_record_replacement_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Pair { left: u8; right: u8; }

            machine replace(pair: &write Pair, replacement: Pair) {
                pair = move replacement;
            }
        "#,
    );
    assert!(
        rendered.contains("replaces whole write-only record `pair`")
            && rendered.contains("freely discardable root"),
        "unexpected diagnostic: {rendered}"
    );
}
