use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_core::diagnostics::Diagnostic;
use omega_image::{
    EmittedImageOutput, FinalExecutableRegionOrigin, PlacedExecutableRegionInventory,
};
use omega_target::Architecture;

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

    let architecture = input.target.architecture;
    let entry_symbol = omega_object_file::object_entry_symbol_name(input.object).to_owned();
    if let Some(emitted_output) = emit_executable_image(input) {
        let emitted_output = emitted_output?;
        validate_compiler_entry_call_return_bytes(
            architecture,
            &entry_symbol,
            &emitted_output.final_text_bytes,
            &emitted_output.executable_regions,
        )?;
        return Ok(emitted_output);
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
}

fn validate_compiler_entry_call_return_bytes(
    architecture: Architecture,
    entry_symbol: &str,
    final_text_bytes: &[u8],
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    let matching_entries = inventory
        .regions
        .iter()
        .filter(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == entry_symbol
        })
        .collect::<Vec<_>>();
    if matching_entries.len() != 1 {
        return Err(Diagnostic::error(format!(
            "final-byte validation requires exactly one compiler entry region named \
             `{entry_symbol}`; found {}",
            matching_entries.len()
        )));
    }
    let entry = matching_entries[0];
    let entry_end = entry
        .section_offset
        .checked_add(entry.byte_count)
        .filter(|end| *end <= final_text_bytes.len())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler entry region `{entry_symbol}` exceeds relocated .text during final-byte validation"
            ))
        })?;
    let bytes = &final_text_bytes[entry.section_offset..entry_end];
    let (prologue, epilogue): (Vec<u8>, Vec<u8>) = match architecture {
        Architecture::X86_64 => (
            omega_isa_x86_64::encode_function_enter_bytes().to_vec(),
            omega_isa_x86_64::encode_return_bytes().to_vec(),
        ),
        Architecture::Aarch64 => (
            omega_isa_aarch64::encode_function_enter_bytes().to_vec(),
            omega_isa_aarch64::encode_return_bytes().to_vec(),
        ),
    };
    if bytes.len() < prologue.len() + epilogue.len() {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` is too short for its fixed call-return mechanics"
        )));
    }
    if !bytes.starts_with(&prologue) {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` has invalid final function-entry bytes"
        )));
    }
    if !bytes.ends_with(&epilogue) {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` has invalid final function-return bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{emit_checked_executable_image, validate_compiler_entry_call_return_bytes};
    use crate::ExecutableImageInput;
    use omega_image::{
        FinalExecutableRegionOrigin, PlacedExecutableRegion, PlacedExecutableRegionInventory,
    };
    use omega_object_file::{ObjectPlan, RelocationPlan};
    use omega_target::NativeTarget;

    #[test]
    fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan::with_capacity(target, 0, 0);
        let relocations = RelocationPlan::with_target(target);

        let diagnostic = emit_checked_executable_image(
            ExecutableImageInput {
                target,
                object: &object,
                relocations: &relocations,
                text_bytes: &[0xaa, 0xbb],
                data_bytes: &[],
                subsystem: 3,
            },
            4,
        )
        .expect_err("encoded/planned byte mismatch should fail before image dispatch");

        assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
        assert!(diagnostic.message.contains("planned 4 byte(s)"));
    }

    #[test]
    fn validates_fixed_call_return_mechanics_in_relocated_entry_bytes() {
        let prologue = omega_isa_x86_64::encode_function_enter_bytes();
        let epilogue = omega_isa_x86_64::encode_return_bytes();
        let mut bytes = prologue
            .into_iter()
            .chain([0x90])
            .chain(epilogue)
            .collect::<Vec<_>>();
        let inventory = PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: bytes.len(),
            text_fingerprint: 1,
            inventory_fingerprint: 2,
            regions: vec![PlacedExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                address: 0x1000,
                byte_count: bytes.len(),
                byte_fingerprint: 3,
                symbol: "entry".into(),
                footprint: None,
            }],
            unclassified_gaps: Vec::new(),
        };

        validate_compiler_entry_call_return_bytes(
            omega_target::Architecture::X86_64,
            "entry",
            &bytes,
            &inventory,
        )
        .expect("exact encoder-owned mechanics should validate");
        bytes[0] ^= 0xff;
        let diagnostic = validate_compiler_entry_call_return_bytes(
            omega_target::Architecture::X86_64,
            "entry",
            &bytes,
            &inventory,
        )
        .expect_err("mutated final mechanics must reject");
        assert!(diagnostic.message.contains("function-entry bytes"));
    }
}
