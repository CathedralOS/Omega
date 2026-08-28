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
    pub callback_placement_identity_fingerprint: u64,
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
    pub compiler_entry_region_binding: Option<CompilerEntryRegionBindingEvidence>,
    pub compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
}

/// Exact final-region custody for the object entry's compiler-private
/// function identity. This row is derived only after the complete function to
/// region join succeeds and is consumed when boundary footprint evidence is
/// attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerEntryRegionBindingEvidence {
    pub function_identity: omega_function_identity::MachineFunctionIdentity,
    pub object_symbol_handle: omega_object_file::ObjectSymbolHandle,
    pub region_index: usize,
    pub symbol: String,
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    pub byte_fingerprint: u64,
    pub inventory_fingerprint: u64,
    pub final_region_binding_fingerprint: u64,
    pub evidence_fingerprint: u64,
}

impl CompilerEntryRegionBindingEvidence {
    pub fn recomputed_evidence_fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let identity = self.function_identity;
        let role_tag = if identity.source_key().is_some() {
            1u8
        } else if identity.program_storage_entry_continuation().is_some() {
            2u8
        } else if identity.callback_thunk_placement_index().is_some() {
            3u8
        } else {
            0u8
        };
        fingerprint_bytes(&mut hash, &[role_tag]);
        let continuation = identity.associated_source_continuation();
        fingerprint_bytes(
            &mut hash,
            &u64::from(continuation.machine.arena_index()).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &u64::from(continuation.machine.generation()).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &u64::from(continuation.state.arena_index()).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &u64::from(continuation.state.generation()).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &(continuation.segment_index as u64).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &(identity
                .callback_thunk_placement_index()
                .unwrap_or(usize::MAX) as u64)
                .to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &u64::from(self.object_symbol_handle.arena_index()).to_le_bytes(),
        );
        fingerprint_bytes(
            &mut hash,
            &u64::from(self.object_symbol_handle.generation()).to_le_bytes(),
        );
        for value in [self.region_index, self.section_offset, self.byte_count] {
            fingerprint_bytes(&mut hash, &(value as u64).to_le_bytes());
        }
        fingerprint_bytes(&mut hash, &(self.symbol.len() as u64).to_le_bytes());
        fingerprint_bytes(&mut hash, self.symbol.as_bytes());
        fingerprint_bytes(&mut hash, &self.address.to_le_bytes());
        fingerprint_bytes(&mut hash, &self.byte_fingerprint.to_le_bytes());
        fingerprint_bytes(&mut hash, &self.inventory_fingerprint.to_le_bytes());
        fingerprint_bytes(
            &mut hash,
            &self.final_region_binding_fingerprint.to_le_bytes(),
        );
        hash
    }
}

/// Receipt for the sole authorized inventory mutation performed after final
/// region validation: attaching the checked boundary footprint to the exact
/// compiler entry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerEntryFootprintBindingEvidence {
    pub entry_region_evidence_fingerprint: u64,
    pub final_region_binding_fingerprint: u64,
    pub prior_inventory_fingerprint: u64,
    pub footprint_fingerprint: u64,
    pub resulting_inventory_fingerprint: u64,
    pub evidence_fingerprint: u64,
}

impl CompilerEntryFootprintBindingEvidence {
    pub fn recomputed_evidence_fingerprint(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for value in [
            self.entry_region_evidence_fingerprint,
            self.final_region_binding_fingerprint,
            self.prior_inventory_fingerprint,
            self.footprint_fingerprint,
            self.resulting_inventory_fingerprint,
        ] {
            fingerprint_bytes(&mut hash, &value.to_le_bytes());
        }
        hash
    }

    pub fn validate_identity(self) -> bool {
        self.entry_region_evidence_fingerprint != 0
            && self.final_region_binding_fingerprint != 0
            && self.prior_inventory_fingerprint != 0
            && self.footprint_fingerprint != 0
            && self.resulting_inventory_fingerprint != 0
            && self.evidence_fingerprint == self.recomputed_evidence_fingerprint()
    }
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
    /// Exact join from every compiler-private function identity through its
    /// object symbol to one placed final executable-region row.
    pub final_region_binding_fingerprint: u64,
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
            self.final_region_binding_fingerprint.to_le_bytes(),
        ] {
            for byte in bytes {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }
}

fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
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
    pub checked_instruction_footprint_fingerprint: u64,
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
        callback_placement_identity_fingerprint: 0,
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
        compiler_entry_region_binding: None,
        compiler_entry_footprint_binding: None,
    }
}
