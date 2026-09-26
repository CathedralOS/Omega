use super::rendered_rejection;

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
