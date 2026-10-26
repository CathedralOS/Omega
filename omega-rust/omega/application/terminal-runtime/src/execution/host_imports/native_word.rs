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

/// Exact semantic leaf below an owned native integer-word envelope. Geometry
/// is authenticated separately against the evaluated calling application.
pub(super) fn scalar_payload_path(
    program: &terminal_interpreter::TerminalProgram,
    root: semantic_vocabulary::StructuralTypeId,
    scalar: semantic_vocabulary::ScalarType,
) -> Result<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>, super::TerminalRuntimeError> {
    let mut path = Vec::new();
    scalar_payload(program, root, scalar, &mut Vec::new(), &mut path)?;
    Ok(path)
}

fn scalar_payload(
    program: &terminal_interpreter::TerminalProgram,
    root: semantic_vocabulary::StructuralTypeId,
    scalar: semantic_vocabulary::ScalarType,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    path: &mut Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
) -> Result<(), super::TerminalRuntimeError> {
    let invalid = super::TerminalRuntimeError::InvalidProvision;
    if visiting.contains(&root) {
        return Err(invalid("native word payload contains a recursive value"));
    }
    visiting.push(root);
    let declaration = program
        .structural_type(root)
        .ok_or(invalid("native word payload type is absent"))?;
    match &declaration.shape {
        terminal_psi::StructuralTypeShape::PrimitiveScalar(value) if *value == scalar => {}
        terminal_psi::StructuralTypeShape::Record { fields, .. } => {
            let relevant: Vec<_> = fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect();
            let [field] = relevant.as_slice() else {
                return Err(invalid(
                    "native owned value must contain exactly one scalar payload",
                ));
            };
            if field.storage.is_some()
                || matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Atomic(_)
                )
            {
                return Err(invalid(
                    "native word payload cannot use atomic or encoded storage",
                ));
            }
            path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                field.id,
            ));
            if let Some(child) = field.field_type.as_structural() {
                scalar_payload(program, child, scalar, visiting, path)?;
            } else if field.field_type.scalar_type() != Some(scalar) {
                return Err(invalid("free owned payload is not the exact scalar type"));
            }
        }
        _ => return Err(invalid("native owned value is not a public scalar payload")),
    }
    visiting.pop();
    Ok(())
}
