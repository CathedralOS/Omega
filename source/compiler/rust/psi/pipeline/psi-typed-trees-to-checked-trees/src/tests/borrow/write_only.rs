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
fn unrestricted_plain_record_leaves_are_wholly_replaceable() {
    lower_typed_trees(typed(
        r#"
            data Leaf [copy] {
                value: u16;
                enabled: bool;
            }
            data Inner { leaf: Leaf; sibling: u8; }
            data Outer { inner: Inner; other: Inner; }

            machine replace(outer: &write Outer, replacement: Leaf) {
                outer.inner.leaf = replacement;
                outer.other.leaf = Leaf { value: 9, enabled: true };
            }
        "#,
    ))
    .expect("an eligible unrestricted record leaf is one content-independent whole store");
}

#[test]
fn ineligible_record_leaves_remain_outside_whole_replacement() {
    for (name, source, expected) in [
        (
            "affine",
            r#"
                data Leaf { value: u16; }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = move replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "linear",
            r#"
                data Leaf [linear] { value: u16; }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = move replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "generic",
            r#"
                data Leaf<T [copy]> [copy] { value: T; }
                data Holder { leaf: Leaf<u16>; }
                machine replace(holder: &write Holder, replacement: Leaf<u16>) {
                    holder.leaf = replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
        (
            "invariant bearing",
            r#"
                data Leaf [copy]
                where
                    value <= limit,
                {
                    value: u16;
                    limit: u16;
                }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "invariant-dependent",
        ),
        (
            "sum",
            r#"
                data Leaf [copy] {
                    case Value(value: u16);
                    case Empty;
                }
                data Holder { leaf: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "sum-payload",
        ),
        (
            "qualified",
            r#"
                data Leaf [copy] { value: u16; }
                domain Leaf::Valid
                requires
                    self.value <= 10;
                data Holder { leaf: Leaf in Valid; }
                machine replace(holder: &write Holder, replacement: Leaf in Valid) {
                    holder.leaf = replacement;
                }
            "#,
            "qualified",
        ),
        (
            "erased",
            r#"
                data Leaf [copy] { value: u16; }
                data Holder { leaf [erased]: Leaf; }
                machine replace(holder: &write Holder, replacement: Leaf) {
                    holder.leaf = replacement;
                }
            "#,
            "erased field `leaf` has no runtime value",
        ),
        (
            "array of records",
            r#"
                data Leaf [copy] { value: u16; }
                data Holder { leaves: [Leaf; 2]; }
                machine replace(holder: &write Holder, replacement: [Leaf; 2]) {
                    holder.leaves = replacement;
                }
            "#,
            "whole eligible unrestricted record",
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name} leaf unexpectedly crossed the bounded record gate: {rendered}"
        );
    }
}

#[test]
fn unrestricted_record_leaf_observation_and_read_modify_write_remain_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Leaf [copy] { value: u16; enabled: bool; }
            data Holder { leaf: Leaf; }

            machine update(holder: &write Holder) {
                holder.leaf.value = holder.leaf.value + 1;
            }
        "#,
    );
    assert!(
        rendered.contains("reads field `value` from write-only parameter `holder`")
            && rendered.contains("never grants observation"),
        "unexpected diagnostic: {rendered}"
    );
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
fn direct_and_nested_unrestricted_primitive_fixed_arrays_are_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { words: [u16; 2]; }
            data Outer { inner: Inner; }

            machine fill(
                direct: &write [u32; 3],
                outer: &write Outer,
                direct_index: u64 [0..=2],
                nested_index: u64 [0..=1]
            ) {
                let direct_length: u64 = direct.len;
                direct = [1, 2, 3];
                direct[0] = 4;
                direct[direct_index] = 5;
                outer.inner.words = [1, 2];
                outer.inner.words[0] = 3;
                outer.inner.words[nested_index] = 4;
            }
        "#,
    ))
    .expect(
        "literal fixed arrays of unrestricted primitive scalars support whole and element stores",
    );
}

#[test]
fn direct_and_nested_primitive_fixed_array_ranges_are_writable() {
    lower_typed_trees(typed(
        r#"
            data Inner { words: [u32; 4]; }
            data Outer { inner: Inner; }

            machine fill(direct: &write [u16; 4], outer: &write Outer) {
                direct[1..3] = [7, 8];
                outer.inner.words[1..=2] = [70000, 80000];
            }
        "#,
    ))
    .expect("closed primitive-array ranges are content-independent exact stores");
}

#[test]
fn non_byte_fixed_array_range_shape_fences_remain_closed() {
    for (name, source, expected) in [
        (
            "symbolic bound",
            r#"
                machine fill(values: &write [u16; 4], start: u64 [0..=2]) {
                    values[start..3] = [1, 2, 3];
                }
            "#,
            "bounds are not statically known",
        ),
        (
            "open end",
            r#"
                machine fill(values: &write [u32; 4]) {
                    values[1..] = [1, 2, 3];
                }
            "#,
            "omitted end",
        ),
        (
            "nonliteral replacement",
            r#"
                machine fill(values: &write [u16; 4], replacement: [u16; 2]) {
                    values[1..3] = replacement;
                }
            "#,
            "from a non-literal value",
        ),
        (
            "wrong-width literal",
            r#"
                machine fill(values: &write [u16; 4]) {
                    values[1..3] = [7];
                }
            "#,
            "must supply exactly 2 element(s)",
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains(expected),
            "{name} unexpectedly crossed the closed range gate: {rendered}"
        );
    }
}

#[test]
fn non_unrestricted_primitive_fixed_array_roots_remain_rejected() {
    for (name, source) in [
        (
            "atomic",
            r#"machine fill(values: &write [AtomicU32; 2]) {}"#,
        ),
        (
            "constrained",
            r#"machine fill(values: &write [u16 [0..=10]; 2]) {}"#,
        ),
        ("generic", r#"machine fill<T>(values: &write [T; 2]) {}"#),
        (
            "noncopy",
            r#"
                data Receipt [linear] { code: u8; }
                machine fill(values: &write [Receipt; 2]) {}
            "#,
        ),
        (
            "sum",
            r#"
                data Choice {
                    case First(value: u16);
                    case Second;
                }
                machine fill(values: &write [Choice; 2]) {}
            "#,
        ),
        (
            "qualified",
            r#"
                domain [u8; 4]::Utf8
                requires
                    valid_utf8(self);

                machine fill(values: &write [u8; 4] in Utf8) {}
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        assert!(
            rendered.contains("literal fixed arrays of unrestricted primitive scalars"),
            "{name} array unexpectedly reached the checked write-only slice: {rendered}"
        );
    }
}

#[test]
fn non_byte_fixed_array_indexes_still_require_an_ordinary_bounds_proof() {
    for (name, source) in [
        (
            "literal out of bounds",
            r#"
                machine fill(values: &write [u16; 2]) {
                    values[2] = 7;
                }
            "#,
        ),
        (
            "unproved dynamic index",
            r#"
                machine fill(values: &write [u32; 2], index: u64) {
                    values[index] = 7;
                }
            "#,
        ),
    ] {
        let rendered = rendered_rejection(source);
        let expected = if name == "literal out of bounds" {
            "the literal index is outside the fixed array"
        } else {
            "cannot prove index `index` is within length 2"
        };
        assert!(
            rendered.contains(expected),
            "{name} unexpectedly bypassed ordinary index checking: {rendered}"
        );
    }
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
                "leaf is an unrestricted primitive, a whole eligible unrestricted record, or a literal fixed array of unrestricted primitive scalars"
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
fn direct_write_only_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(typed(
        r#"
            machine observe_length(bytes: &write [u8; 4]) {
                let length: u64 = bytes.len;
            }
        "#,
    ))
    .expect("the direct fixed byte-array length is static type metadata, not content");
}

#[test]
fn direct_write_only_fixed_array_length_supports_a_proven_element_store() {
    lower_typed_trees(typed(
        r#"
            machine fill(bytes: &write [u8; 4], index: u64 [0..bytes.len]) {
                let length: u64 = bytes.len;
                bytes[index] = 7;
            }
        "#,
    ))
    .expect("fixed-array length metadata should support its ordinary proven index bound");
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
fn record_held_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(typed(
        r#"
            data Holder { bytes: [u8; 4]; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.bytes.len;
            }
        "#,
    ))
    .expect("literal fixed-array length behind a plain record field is static metadata");
}

#[test]
fn nested_plain_record_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(typed(
        r#"
            data Inner {
                bytes: [u8; 4];
                sibling: u64;
            }
            data Holder {
                inner: Inner;
                sibling: u64;
            }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.inner.bytes.len;
            }
        "#,
    ))
    .expect("every receiver in a nested plain-record path has statically known common fields");
}

#[test]
fn nested_plain_record_non_byte_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(typed(
        r#"
            data Inner { words: [u16; 4]; }
            data Holder { inner: Inner; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.inner.words.len;
            }
        "#,
    ))
    .expect("literal fixed-array length is static independently of its element type");
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
fn nested_generic_record_fixed_array_length_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner<T> { bytes: [u8; 4]; marker: T; }
            data Holder<T> { inner: Inner<T>; }

            machine observe_length<T>(holder: &write Holder<T>) {
                let length: u64 = holder.inner.bytes.len;
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
fn qualified_record_held_fixed_array_length_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            domain [u8; 4]::Utf8
            requires
                valid_utf8(self);

            data Holder { bytes: [u8; 4] in Utf8; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.bytes.len;
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
fn invariant_bearing_record_fixed_array_length_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Inner
            where
                marker <= limit,
            {
                bytes: [u8; 4];
                marker: u8;
                limit: u8;
            }
            data Holder { inner: Inner; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.inner.bytes.len;
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
fn direct_write_only_byte_slice_proven_element_is_writable() {
    lower_typed_trees(typed(
        r#"
            machine fill(bytes: &write [u8], index: u64 [0..bytes.len]) {
                bytes[index] = 7;
            }
        "#,
    ))
    .expect("a runtime byte-slice index proven against descriptor length should lower");
}

#[test]
fn direct_write_only_byte_slice_unproved_element_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine fill(bytes: &write [u8], index: u64) {
                bytes[index] = 7;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove index `index` is within unknown slice length of `bytes`"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_byte_slice_literal_without_nonempty_proof_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine fill(bytes: &write [u8]) {
                bytes[0] = 7;
            }
        "#,
    );
    assert!(
        rendered.contains("cannot prove index `0` is within unknown slice length of `bytes`"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn direct_write_only_byte_slice_index_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine fill(bytes: &write [u8], index: u64 [0..bytes.len]) {
                bytes[bytes[index]] = 7;
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
fn direct_write_only_byte_slice_rhs_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine fill(bytes: &write [u8], index: u64 [0..bytes.len]) {
                bytes[index] = bytes[index];
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
fn direct_write_only_byte_slice_range_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine fill(bytes: &write [u8]) {
                bytes[0..1] = [7];
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection") && rendered.contains("range"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn nested_write_only_slice_element_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            data Holder<'data> { view: &'data [u8]; }

            machine fill<'data>(holder: &write Holder<'data>) {
                holder.view[0] = 7;
            }
        "#,
    );
    assert!(
        rendered.contains("unsupported write-only projection")
            && rendered.contains("direct byte slice"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn whole_write_only_byte_slice_replacement_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine replace(bytes: &write [u8], replacement: [u8]) {
                bytes = replacement;
            }
        "#,
    );
    assert!(
        rendered.contains("replaces whole write-only record `bytes`")
            && rendered.contains("whole-root replacement"),
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
fn non_byte_fixed_array_rhs_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine copy(words: &write [u16; 2]) {
                words[0] = words[1];
            }
        "#,
    );
    assert!(
        rendered.contains("reads through index projection of write-only parameter `words`")
            && rendered.contains("never observation"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn non_byte_fixed_array_range_rhs_observation_remains_rejected() {
    let rendered = rendered_rejection(
        r#"
            machine copy(words: &write [u16; 4]) {
                words[1..3] = [words[0], 7];
            }
        "#,
    );
    assert!(
        rendered.contains("reads through index projection of write-only parameter `words`")
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
