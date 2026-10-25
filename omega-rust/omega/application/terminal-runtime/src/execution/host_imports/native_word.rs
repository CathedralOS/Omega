//! Authenticate a native integer word through zero-offset record envelopes.
//! The operation reports geometry; callers retain their own actual C type
//! restrictions (for example a record-valued u32 versus a pointer word).

use calling_conventions::{BoundarySignatureGeometry, BoundaryValueClass};

pub(super) struct NativeWordEnvelope {
    pub(super) byte_size: u16,
    pub(super) alignment: u16,
    pub(super) contains_record: bool,
}

pub(super) fn native_word_envelope(
    signature: &BoundarySignatureGeometry,
    mut root: u16,
) -> Result<NativeWordEnvelope, &'static str> {
    let initial = signature
        .shapes
        .get(usize::from(root))
        .ok_or("native word envelope references an absent root shape")?;
    let mut result = NativeWordEnvelope {
        byte_size: initial.byte_size,
        alignment: initial.alignment,
        contains_record: false,
    };
    // A complete finite roster bounds traversal without allocating another
    // visited set; cyclic and malformed nested edges cannot extend admission.
    for _ in 0..signature.shapes.len() {
        let shape = signature
            .shapes
            .get(usize::from(root))
            .ok_or("native word envelope references an absent shape")?;
        if shape.byte_size != result.byte_size || shape.alignment != result.alignment {
            return Err("native word envelope nodes disagree on extent or alignment");
        }
        match shape.class {
            BoundaryValueClass::Integer => return Ok(result),
            BoundaryValueClass::Record {
                first_field,
                field_count: 1,
            } => {
                let field = signature
                    .fields
                    .get(usize::from(first_field))
                    .ok_or("native word envelope references an absent field")?;
                if field.byte_offset != 0 {
                    return Err("native word envelope field must start at byte zero");
                }
                root = field.shape;
                result.contains_record = true;
            }
            _ => {
                return Err(
                    "native word envelope must end in an integer through single-field records",
                );
            }
        }
    }
    Err("native word envelope graph is cyclic")
}
