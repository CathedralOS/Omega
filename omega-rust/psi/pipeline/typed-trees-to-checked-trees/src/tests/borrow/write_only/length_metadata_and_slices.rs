use super::{rendered_rejection, typed};
use crate::CheckingRequest;
use crate::lower_typed_trees;

#[test]
fn direct_write_only_byte_slice_length_metadata_is_readable() {
    lower_typed_trees(
        typed(
            r#"
            machine observe_length(bytes: &write [u8]) {
                let length: u64 = bytes.len;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed(
            r#"
            machine observe_length(bytes: &write [u8; 4]) {
                let length: u64 = bytes.len;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("the direct fixed byte-array length is static type metadata, not content");
}

#[test]
fn direct_write_only_fixed_array_length_supports_a_proven_element_store() {
    lower_typed_trees(
        typed(
            r#"
            machine fill(bytes: &write [u8; 4], index: u64 [0..bytes.len]) {
                let length: u64 = bytes.len;
                bytes[index] = 7;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed(
            r#"
            data Holder { bytes: [u8; 4]; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.bytes.len;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("literal fixed-array length behind a plain record field is static metadata");
}

#[test]
fn nested_plain_record_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(
        typed(
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
        ),
        &CheckingRequest::settled(),
    )
    .expect("every receiver in a nested plain-record path has statically known common fields");
}

#[test]
fn nested_plain_record_non_byte_fixed_array_length_metadata_is_readable() {
    lower_typed_trees(
        typed(
            r#"
            data Inner { words: [u16; 4]; }
            data Holder { inner: Inner; }

            machine observe_length(holder: &write Holder) {
                let length: u64 = holder.inner.words.len;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed(
            r#"
            machine fill(bytes: &write [u8], index: u64 [0..bytes.len]) {
                bytes[index] = 7;
            }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
        rendered.contains("replaces whole write-only aggregate `bytes`")
            && rendered.contains("freely discardable supported root"),
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
        rendered.contains("replaces whole write-only aggregate `pair`")
            && rendered.contains("freely discardable supported root"),
        "unexpected diagnostic: {rendered}"
    );
}
