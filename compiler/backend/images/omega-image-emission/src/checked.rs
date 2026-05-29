use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_core::diagnostics::Diagnostic;
use omega_image::EmittedImageOutput;

pub fn emit_checked_executable_image(
    input: ExecutableImageInput<'_>,
    planned_text_bytes: usize,
) -> Result<EmittedImageOutput, Diagnostic> {
    if input.text_bytes.len() != planned_text_bytes {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            input.target,
            input.text_bytes.len(),
            planned_text_bytes
        )));
    }

    if let Some(emitted_output) = emit_executable_image(input) {
        return emitted_output;
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
}

#[cfg(test)]
mod tests {
    use super::emit_checked_executable_image;
    use crate::ExecutableImageInput;
    use omega_core::arena::{Arena, Handle};
    use omega_object_file::{ObjectPlan, RelocationPlan};
    use omega_target::NativeTarget;

    #[test]
    fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan {
            target,
            layout: omega_object_file::ObjectFileLayout {
                sections: Arena::new(),
                symbols: Arena::new(),
                entry_symbol: Handle::invalid(),
            },
        };
        let relocations = RelocationPlan {
            target,
            record_set: omega_object_file::RelocationRecordSet {
                records: Arena::new(),
            },
        };

        let diagnostic = emit_checked_executable_image(
            ExecutableImageInput {
                target,
                object: &object,
                relocations: &relocations,
                text_bytes: &[0xaa, 0xbb],
                data_bytes: &[],
            },
            4,
        )
        .expect_err("encoded/planned byte mismatch should fail before image dispatch");

        assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
        assert!(diagnostic.message.contains("planned 4 byte(s)"));
    }
}
