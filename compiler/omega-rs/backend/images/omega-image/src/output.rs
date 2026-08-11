#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    /// Exact relocated `.text` bytes before container padding/signing.
    pub final_text_bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub imports: usize,
    pub relocations: usize,
    pub executable_regions: crate::PlacedExecutableRegionInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedImageOutput {
    pub bytes: Vec<u8>,
    pub final_text_bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub kind: ImageOutputKind,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
    pub final_image_symbols: usize,
    pub final_image_imports: usize,
    pub final_image_relocations: usize,
    pub executable_regions: crate::PlacedExecutableRegionInventory,
    pub compiler_text_validation: Option<CompilerTextValidationEvidence>,
    pub compiler_function_validation: Option<CompilerFunctionValidationEvidence>,
}

/// Exact final-byte binding for the compiler function/instruction partition.
/// This proves complete instruction-boundary enumeration; individual ordinary
/// instruction footprint decoding remains a separate certificate class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerFunctionValidationEvidence {
    pub function_count: usize,
    pub instruction_count: usize,
    pub zero_width_instruction_count: usize,
    pub checked_assembly_instruction_count: usize,
    pub fixed_mechanics_instruction_count: usize,
    pub fixed_mechanics_validation_fingerprint: u64,
    pub fixed_mechanics_boundary_contract_fingerprint: u64,
    pub fixed_mechanics_footprint_fingerprint: u64,
    pub body_specification_instruction_count: usize,
    pub body_specification_validation_fingerprint: u64,
    pub body_specification_boundary_contract_fingerprint: u64,
    pub body_specification_footprint_fingerprint: u64,
    pub composed_footprint_fingerprint: u64,
    pub validation_fingerprint: u64,
}

impl CompilerFunctionValidationEvidence {
    pub fn evidence_fingerprint(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for bytes in [
            self.validation_fingerprint.to_le_bytes(),
            (self.function_count as u64).to_le_bytes(),
            (self.instruction_count as u64).to_le_bytes(),
            (self.zero_width_instruction_count as u64).to_le_bytes(),
            (self.checked_assembly_instruction_count as u64).to_le_bytes(),
            (self.fixed_mechanics_instruction_count as u64).to_le_bytes(),
            self.fixed_mechanics_validation_fingerprint.to_le_bytes(),
            self.fixed_mechanics_boundary_contract_fingerprint
                .to_le_bytes(),
            self.fixed_mechanics_footprint_fingerprint.to_le_bytes(),
            (self.body_specification_instruction_count as u64).to_le_bytes(),
            self.body_specification_validation_fingerprint.to_le_bytes(),
            self.body_specification_boundary_contract_fingerprint
                .to_le_bytes(),
            self.body_specification_footprint_fingerprint.to_le_bytes(),
            self.composed_footprint_fingerprint.to_le_bytes(),
        ] {
            for byte in bytes {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerTextValidationEvidence {
    pub encoded_text_fingerprint: u64,
    pub final_compiler_text_fingerprint: u64,
    pub relocation_envelope_fingerprint: u64,
    /// Checked-assembly instructions whose fixed encoding or normalized
    /// privilege-bearing envelope was validated at retained final boundaries.
    pub checked_instruction_validation_fingerprint: u64,
    pub derivation_fingerprint: u64,
    pub text_relocation_count: usize,
    pub checked_instruction_validation_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOutputKind {
    DirectExecutable,
}

pub fn emitted_direct_executable_output(output: ExecutableImageOutput) -> EmittedImageOutput {
    EmittedImageOutput {
        bytes: output.bytes,
        final_text_bytes: output.final_text_bytes,
        file_name: output.file_name,
        format: output.format,
        kind: ImageOutputKind::DirectExecutable,
        text_bytes: output.text_bytes,
        data_bytes: output.data_bytes,
        bss_bytes: output.bss_bytes,
        symbols: output.symbols,
        relocations: output.relocations,
        final_image_symbols: output.symbols,
        final_image_imports: output.imports,
        final_image_relocations: output.relocations,
        executable_regions: output.executable_regions,
        compiler_text_validation: None,
        compiler_function_validation: None,
    }
}
