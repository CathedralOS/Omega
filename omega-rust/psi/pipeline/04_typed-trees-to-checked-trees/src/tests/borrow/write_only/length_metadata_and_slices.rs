use super::rendered_rejection;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

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
fn direct_write_only_byte_slice_proven_element_is_writable() {
    lower_typed_trees(
        typed_program(
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
