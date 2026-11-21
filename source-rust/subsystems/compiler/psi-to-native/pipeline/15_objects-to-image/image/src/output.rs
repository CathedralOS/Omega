/*
We define the emitter output carriers (`ExecutableImageOutput`,
`EmittedImageOutput`, `ImageOutputKind`) and the strong SHA-256 validation
evidence records that bind compiler-encoded text, relocated text, function
region inventories, and entry footprints into a replayable chain.
*/

image_digest!(CompilerEntryRegionBindingDigest);
image_digest!(CompilerEntryFootprintBindingDigest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    pub final_image_layout: crate::FinalImageLayout,
    pub final_text_bytes: Vec<u8>,
    pub final_data_bytes: Vec<u8>,
    pub final_import_data_bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub imports: usize,
    pub relocations: usize,
    pub executable_regions: crate::PlacedExecutableRegionInventory,
    pub data_regions: crate::PlacedDataRegionInventory,
    pub import_data_regions: crate::PlacedDataRegionInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedImageOutput {
    pub bytes: Vec<u8>,
    pub final_image_layout: crate::FinalImageLayout,
    pub final_text_bytes: Vec<u8>,
    pub final_data_bytes: Vec<u8>,
    pub final_import_data_bytes: Vec<u8>,
    pub callback_placement_identity_report_fingerprint: u64,
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
    pub data_regions: crate::PlacedDataRegionInventory,
    pub import_data_regions: crate::PlacedDataRegionInventory,
    pub compiler_entry_region_binding: Option<CompilerEntryRegionBindingEvidence>,
    pub compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
}

impl EmittedImageOutput {
    pub fn validate_final_initialized_data_relocation_envelope(
        &self,
        encoded_data_bytes: &[u8],
        relocations: &object_file::RelocationPlan,
    ) -> Result<(), diagnostics::Diagnostic> {
        crate::relocation_envelope::validate_final_initialized_data_relocation_envelope(
            encoded_data_bytes,
            &self.final_data_bytes,
            relocations,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerEntryRegionBindingEvidence {
    pub function_identity: function_identity::MachineFunctionIdentity,
    pub object_symbol_handle: object_file::ObjectSymbolHandle,
    pub region_index: usize,
    pub symbol: String,
    pub section_offset: usize,
    pub address: u64,
    pub byte_count: usize,
    pub byte_digest: crate::PlacedExecutableRegionBytesDigest,
    pub byte_report_fingerprint: u64,
    pub inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    pub inventory_report_fingerprint: u64,
    pub final_region_binding_report_fingerprint: u64,
    pub evidence_digest: CompilerEntryRegionBindingDigest,
    pub evidence_report_fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerEntryFootprintBindingEvidence {
    pub entry_region_evidence_digest: CompilerEntryRegionBindingDigest,
    pub entry_region_evidence_report_fingerprint: u64,
    pub final_region_binding_report_fingerprint: u64,
    pub prior_inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    pub prior_inventory_report_fingerprint: u64,
    pub footprint_digest: crate::StateFootprintEvidenceDigest,
    pub footprint_report_fingerprint: u64,
    pub resulting_inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    pub resulting_inventory_report_fingerprint: u64,
    pub evidence_digest: CompilerEntryFootprintBindingDigest,
    pub evidence_report_fingerprint: u64,
}

image_digest!(EncodedCompilerTextDigest);
image_digest!(FinalCompilerTextDigest);
image_digest!(CompilerTextRelocationEnvelopeDigest);
image_digest!(CompilerTextDerivationDigest);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerTextValidationEvidence {
    pub encoded_text_digest: EncodedCompilerTextDigest,
    pub final_compiler_text_digest: FinalCompilerTextDigest,
    pub relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest,
    pub derivation_digest: CompilerTextDerivationDigest,
    pub encoded_text_report_fingerprint: u64,
    pub final_compiler_text_report_fingerprint: u64,
    pub relocation_envelope_report_fingerprint: u64,
    pub checked_instruction_validation_report_fingerprint: u64,
    pub checked_instruction_footprint_report_fingerprint: u64,
    pub derivation_report_fingerprint: u64,
    pub text_relocation_count: usize,
    pub checked_instruction_validation_count: usize,
}

impl CompilerTextValidationEvidence {
    pub fn recomputed_derivation_digest(&self) -> CompilerTextDerivationDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.compiler-text-derivation.sha256.v1\0",
        );
        digest.raw(self.encoded_text_digest.as_bytes());
        digest.raw(self.final_compiler_text_digest.as_bytes());
        digest.raw(self.relocation_envelope_digest.as_bytes());
        digest.raw(&self.encoded_text_report_fingerprint.to_le_bytes());
        digest.raw(&self.final_compiler_text_report_fingerprint.to_le_bytes());
        digest.raw(&self.relocation_envelope_report_fingerprint.to_le_bytes());
        digest.raw(
            &self
                .checked_instruction_validation_report_fingerprint
                .to_le_bytes(),
        );
        digest.raw(
            &self
                .checked_instruction_footprint_report_fingerprint
                .to_le_bytes(),
        );
        digest.u64(self.text_relocation_count as u64);
        digest.u64(self.checked_instruction_validation_count as u64);
        CompilerTextDerivationDigest::from_digest(digest.finish())
    }

    pub fn has_valid_derivation_digest(&self) -> bool {
        self.derivation_digest == self.recomputed_derivation_digest()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOutputKind {
    DirectExecutable,
}

pub fn emitted_direct_executable_output(output: ExecutableImageOutput) -> EmittedImageOutput {
    EmittedImageOutput {
        bytes: output.bytes,
        final_image_layout: output.final_image_layout,
        final_text_bytes: output.final_text_bytes,
        final_data_bytes: output.final_data_bytes,
        final_import_data_bytes: output.final_import_data_bytes,
        callback_placement_identity_report_fingerprint: 0,
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
        data_regions: output.data_regions,
        import_data_regions: output.import_data_regions,
        compiler_entry_region_binding: None,
        compiler_entry_footprint_binding: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
        CompilerTextValidationEvidence, EncodedCompilerTextDigest, FinalCompilerTextDigest,
    };

    fn text_evidence(encoded_digest: [u8; 32]) -> CompilerTextValidationEvidence {
        let mut evidence = CompilerTextValidationEvidence {
            encoded_text_digest: EncodedCompilerTextDigest::from_digest(encoded_digest),
            final_compiler_text_digest: FinalCompilerTextDigest::from_digest([2; 32]),
            relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([3; 32]),
            derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
            encoded_text_report_fingerprint: 11,
            final_compiler_text_report_fingerprint: 12,
            relocation_envelope_report_fingerprint: 13,
            checked_instruction_validation_report_fingerprint: 14,
            checked_instruction_footprint_report_fingerprint: 15,
            derivation_report_fingerprint: 16,
            text_relocation_count: 17,
            checked_instruction_validation_count: 18,
        };
        evidence.derivation_digest = evidence.recomputed_derivation_digest();
        evidence
    }

    #[test]
    fn compact_collision_cannot_substitute_strong_compiler_text_evidence() {
        let first = text_evidence([1; 32]);
        let second = text_evidence([99; 32]);

        assert_eq!(
            first.encoded_text_report_fingerprint,
            second.encoded_text_report_fingerprint
        );
        assert_eq!(
            first.derivation_report_fingerprint,
            second.derivation_report_fingerprint
        );
        assert_ne!(first.encoded_text_digest, second.encoded_text_digest);
        assert_ne!(first.derivation_digest, second.derivation_digest);
        assert!(first.has_valid_derivation_digest());
        assert!(second.has_valid_derivation_digest());
    }

    #[test]
    fn strong_derivation_rejects_digest_substitution() {
        let mut evidence = text_evidence([1; 32]);
        evidence.final_compiler_text_digest = FinalCompilerTextDigest::from_digest([77; 32]);
        assert!(!evidence.has_valid_derivation_digest());
    }
}
