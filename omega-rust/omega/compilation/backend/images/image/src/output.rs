//! The evidence an emitter hands forward: text and function validation, entry
//! bindings, and the image output kinds that carry them.

use crate::final_image::byte_report_fingerprint;

image_digest!(CompilerEntryRegionBindingDigest);
image_digest!(CompilerEntryFootprintBindingDigest);
image_digest!(CompilerFunctionValidationDigest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    /// Exact target virtual placement used while applying final relocations.
    pub final_image_layout: crate::FinalImageLayout,
    /// Exact relocated `.text` bytes before container padding/signing.
    pub final_text_bytes: Vec<u8>,
    /// Exact relocated initialized-data bytes before container padding/signing.
    pub final_data_bytes: Vec<u8>,
    /// Exact writer-owned import-data bytes outside `.data` — the PE `.rdata`
    /// import table whose IAT slots the thunks load through. Empty when the
    /// target emits none.
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
    /// Access initialized-data custody: compiler data plus any writer-owned
    /// binding slots. The complete final data must classify or be refused.
    pub data_regions: crate::PlacedDataRegionInventory,
    /// Access custody over `final_import_data_bytes` — the bound pointer
    /// slots a closed thunk form decodes to. The canonical empty inventory
    /// when the target emits no import-data extent.
    pub import_data_regions: crate::PlacedDataRegionInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedImageOutput {
    pub bytes: Vec<u8>,
    pub final_image_layout: crate::FinalImageLayout,
    pub final_text_bytes: Vec<u8>,
    pub final_data_bytes: Vec<u8>,
    /// Exact writer-owned import-data bytes outside `.data`, as on
    /// [`ExecutableImageOutput`]. Empty when the target emits none.
    pub final_import_data_bytes: Vec<u8>,
    /// Compact reporting coordinate. Exact callback-placement rows are replayed
    /// before image emission; this value does not recreate that authority.
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
    /// Access custody over `final_import_data_bytes`, as on
    /// [`ExecutableImageOutput`].
    pub import_data_regions: crate::PlacedDataRegionInventory,
    pub compiler_text_validation: Option<CompilerTextValidationEvidence>,
    pub compiler_function_validation: Option<CompilerFunctionValidationEvidence>,
    pub compiler_entry_region_binding: Option<CompilerEntryRegionBindingEvidence>,
    pub compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
}

impl EmittedImageOutput {
    /// Prove that this output's exact final initialized data differs from its
    /// encoded input only at checked absolute data relocation slots.
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

/// Exact final-region custody for the object entry's compiler-private
/// function identity. This row is derived only after the complete function to
/// region join succeeds and is consumed when boundary footprint evidence is
/// attached.
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
    /// Compact report compatibility only.
    pub byte_report_fingerprint: u64,
    pub inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only.
    pub inventory_report_fingerprint: u64,
    /// Compact report coordinate beside this row's exact structural binding and
    /// collision-resistant evidence digest.
    pub final_region_binding_report_fingerprint: u64,
    pub evidence_digest: CompilerEntryRegionBindingDigest,
    /// Compact report compatibility only.
    pub evidence_report_fingerprint: u64,
}

/// Receipt for the sole authorized inventory mutation performed after final
/// region validation: attaching the checked boundary footprint to the exact
/// compiler entry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerEntryFootprintBindingEvidence {
    pub entry_region_evidence_digest: CompilerEntryRegionBindingDigest,
    /// Compact report compatibility only.
    pub entry_region_evidence_report_fingerprint: u64,
    /// Compact report coordinate; `entry_region_evidence_digest` retains the
    /// exact final-region binding authority.
    pub final_region_binding_report_fingerprint: u64,
    pub prior_inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only.
    pub prior_inventory_report_fingerprint: u64,
    pub footprint_digest: crate::StateFootprintEvidenceDigest,
    /// Compact report compatibility only.
    pub footprint_report_fingerprint: u64,
    pub resulting_inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only.
    pub resulting_inventory_report_fingerprint: u64,
    pub evidence_digest: CompilerEntryFootprintBindingDigest,
    /// Compact report compatibility only.
    pub evidence_report_fingerprint: u64,
}

/// Reporting summary for current compiler functions and their selected spans after
/// exact final-byte validation. The counters and compact coordinates describe
/// the completed validation but do not recreate it. Exact final text, placed
/// regions, entry-region custody, and state footprints remain in their own
/// structural carriers and domain-separated commitments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerFunctionValidationEvidence {
    pub function_count: usize,
    /// Number of selected instruction spans, not individual ISA instructions.
    /// Inserted frame protocol bytes are accounted separately below.
    pub instruction_count: usize,
    /// Selected instruction spans whose emitted byte sequence is empty.
    pub zero_width_instruction_count: usize,
    pub frame_prologue_byte_count: usize,
    /// Total inserted epilogue bytes across all retained return sites.
    pub frame_epilogue_byte_count: usize,
    /// First eight bytes of the retained strong manifest identity, little-endian.
    pub fragment_manifest_report_fingerprint: u64,
    /// First eight bytes of the retained strong frame-application identity, little-endian.
    pub frame_application_report_fingerprint: u64,
    /// Present only when exact boundary/entry footprint evidence exists.
    /// Common direct physical replay alone does not establish this coordinate.
    pub boundary_contract_report_fingerprint: Option<u64>,
    /// Compact report coordinate for validated final-region binding. The direct
    /// common path uses the executable inventory's compact fingerprint; an
    /// entry-footprint binding retains its exact join separately.
    pub final_region_binding_report_fingerprint: u64,
    /// Independently derived compiler-text validation's derivation report coordinate.
    pub final_text_validation_report_fingerprint: u64,
}

impl CompilerFunctionValidationEvidence {
    /// Collision-resistant commitment to the complete normalized reporting
    /// summary. This preserves custody of the summary as a whole; it does not
    /// promote any imported compact coordinate into underlying authority.
    /// Publication and replay also retain the exact text and region-inventory
    /// commitments described above.
    pub fn evidence_digest(self) -> CompilerFunctionValidationDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.compiler-function-validation.sha256.v2\0",
        );
        for value in [
            self.function_count,
            self.instruction_count,
            self.zero_width_instruction_count,
            self.frame_prologue_byte_count,
            self.frame_epilogue_byte_count,
        ] {
            digest.u64(value as u64);
        }
        for value in [
            self.fragment_manifest_report_fingerprint,
            self.frame_application_report_fingerprint,
            self.final_region_binding_report_fingerprint,
            self.final_text_validation_report_fingerprint,
        ] {
            digest.raw(&value.to_le_bytes());
        }
        digest.byte(u8::from(
            self.boundary_contract_report_fingerprint.is_some(),
        ));
        if let Some(boundary) = self.boundary_contract_report_fingerprint {
            digest.raw(&boundary.to_le_bytes());
        }
        CompilerFunctionValidationDigest::from_digest(digest.finish())
    }

    /// Compact reporting coordinate only. This is not evidence, admission,
    /// publication, or replay authority; use [`Self::evidence_digest`].
    pub fn evidence_report_fingerprint(self) -> u64 {
        byte_report_fingerprint(self.evidence_digest().as_bytes())
    }
}

image_digest!(EncodedCompilerTextDigest);
image_digest!(FinalCompilerTextDigest);
image_digest!(CompilerTextRelocationEnvelopeDigest);
image_digest!(CompilerTextDerivationDigest);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerTextValidationEvidence {
    /// Collision-resistant commitment to the exact compiler text before
    /// relocation. The compact fingerprint below remains report compatibility
    /// only.
    pub encoded_text_digest: EncodedCompilerTextDigest,
    /// Collision-resistant commitment to the exact compiler-owned prefix of
    /// final text.
    pub final_compiler_text_digest: FinalCompilerTextDigest,
    /// Collision-resistant commitment to the canonical ordered relocation
    /// envelope.
    pub relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest,
    /// Domain-separated commitment joining all strong text commitments and
    /// the remaining imported report fields.
    pub derivation_digest: CompilerTextDerivationDigest,
    /// Legacy compact report fingerprint. It is not an authority key; use
    /// `encoded_text_digest` or exact byte replay.
    pub encoded_text_report_fingerprint: u64,
    /// Legacy compact report fingerprint. It is not an authority key; use
    /// `final_compiler_text_digest` or exact byte replay.
    pub final_compiler_text_report_fingerprint: u64,
    /// Legacy compact report fingerprint. It is not an authority key; use the
    /// strong relocation-envelope digest.
    pub relocation_envelope_report_fingerprint: u64,
    /// Checked-assembly instructions whose fixed encoding or normalized
    /// privilege-bearing envelope was validated at retained final boundaries.
    pub checked_instruction_validation_report_fingerprint: u64,
    pub checked_instruction_footprint_report_fingerprint: u64,
    /// Legacy compact report fingerprint retained for current report
    /// compatibility. `derivation_digest` is the collision-resistant join.
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
        compiler_text_validation: None,
        compiler_function_validation: None,
        compiler_entry_region_binding: None,
        compiler_entry_footprint_binding: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerFunctionValidationEvidence, CompilerTextDerivationDigest,
        CompilerTextRelocationEnvelopeDigest, CompilerTextValidationEvidence,
        EncodedCompilerTextDigest, FinalCompilerTextDigest,
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

    fn function_evidence() -> CompilerFunctionValidationEvidence {
        CompilerFunctionValidationEvidence {
            function_count: 1,
            instruction_count: 5,
            zero_width_instruction_count: 3,
            frame_prologue_byte_count: 4,
            frame_epilogue_byte_count: 8,
            fragment_manifest_report_fingerprint: 6,
            frame_application_report_fingerprint: 7,
            boundary_contract_report_fingerprint: Some(11),
            final_region_binding_report_fingerprint: 14,
            final_text_validation_report_fingerprint: 15,
        }
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

    #[test]
    fn function_validation_digest_binds_every_normalized_field() {
        let evidence = function_evidence();
        let expected = evidence.evidence_digest();

        for drifted in [
            {
                let mut drifted = evidence;
                drifted.function_count += 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.instruction_count += 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.zero_width_instruction_count += 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.frame_prologue_byte_count += 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.frame_epilogue_byte_count += 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.fragment_manifest_report_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.frame_application_report_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.boundary_contract_report_fingerprint = None;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.boundary_contract_report_fingerprint = Some(12);
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.final_region_binding_report_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.final_text_validation_report_fingerprint ^= 1;
                drifted
            },
        ] {
            assert_ne!(drifted.evidence_digest(), expected);
        }
    }

    #[test]
    fn function_validation_digest_distinguishes_absent_and_zero_boundary() {
        let mut absent = function_evidence();
        absent.boundary_contract_report_fingerprint = None;
        let mut present = absent;
        present.boundary_contract_report_fingerprint = Some(0);
        assert_ne!(absent.evidence_digest(), present.evidence_digest());
    }
}
