use sha2::{Digest, Sha256};

macro_rules! image_evidence_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

image_evidence_digest!(CompilerEntryRegionBindingDigest);
image_evidence_digest!(CompilerEntryFootprintBindingDigest);
image_evidence_digest!(CompilerFunctionValidationDigest);

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
    pub byte_digest: crate::PlacedExecutableRegionBytesDigest,
    /// Compact report compatibility only.
    pub byte_report_fingerprint: u64,
    pub inventory_digest: crate::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only.
    pub inventory_report_fingerprint: u64,
    pub final_region_binding_fingerprint: u64,
    pub evidence_digest: CompilerEntryRegionBindingDigest,
    /// Compact report compatibility only.
    pub evidence_report_fingerprint: u64,
}

impl CompilerEntryRegionBindingEvidence {
    pub fn recomputed_evidence_digest(&self) -> CompilerEntryRegionBindingDigest {
        let mut digest = Sha256::new();
        digest.update(b"omega.compiler-entry-region-binding.sha256.v1\0");
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
        digest.update([role_tag]);
        let continuation = identity.associated_source_continuation();
        digest.update(u64::from(continuation.machine.arena_index()).to_le_bytes());
        digest.update(u64::from(continuation.machine.generation()).to_le_bytes());
        digest.update(u64::from(continuation.state.arena_index()).to_le_bytes());
        digest.update(u64::from(continuation.state.generation()).to_le_bytes());
        digest.update((continuation.segment_index as u64).to_le_bytes());
        digest.update(
            (identity
                .callback_thunk_placement_index()
                .unwrap_or(usize::MAX) as u64)
                .to_le_bytes(),
        );
        digest.update(u64::from(self.object_symbol_handle.arena_index()).to_le_bytes());
        digest.update(u64::from(self.object_symbol_handle.generation()).to_le_bytes());
        digest.update((self.region_index as u64).to_le_bytes());
        digest.update((self.symbol.len() as u64).to_le_bytes());
        digest.update(self.symbol.as_bytes());
        digest.update((self.section_offset as u64).to_le_bytes());
        digest.update(self.address.to_le_bytes());
        digest.update((self.byte_count as u64).to_le_bytes());
        digest.update(self.byte_digest.as_bytes());
        digest.update(self.inventory_digest.as_bytes());
        digest.update(self.final_region_binding_fingerprint.to_le_bytes());
        CompilerEntryRegionBindingDigest::from_digest(digest.finalize().into())
    }

    pub fn has_valid_evidence_digest(&self) -> bool {
        self.evidence_digest == self.recomputed_evidence_digest()
    }

    pub fn recomputed_evidence_report_fingerprint(&self) -> u64 {
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
        fingerprint_bytes(&mut hash, &self.byte_report_fingerprint.to_le_bytes());
        fingerprint_bytes(&mut hash, &self.inventory_report_fingerprint.to_le_bytes());
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
    pub entry_region_evidence_digest: CompilerEntryRegionBindingDigest,
    /// Compact report compatibility only.
    pub entry_region_evidence_report_fingerprint: u64,
    pub final_region_binding_fingerprint: u64,
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

impl CompilerEntryFootprintBindingEvidence {
    pub fn recomputed_evidence_digest(self) -> CompilerEntryFootprintBindingDigest {
        let mut digest = Sha256::new();
        digest.update(b"omega.compiler-entry-footprint-binding.sha256.v1\0");
        digest.update(self.entry_region_evidence_digest.as_bytes());
        digest.update(self.final_region_binding_fingerprint.to_le_bytes());
        digest.update(self.prior_inventory_digest.as_bytes());
        digest.update(self.footprint_digest.as_bytes());
        digest.update(self.resulting_inventory_digest.as_bytes());
        CompilerEntryFootprintBindingDigest::from_digest(digest.finalize().into())
    }

    pub fn recomputed_evidence_report_fingerprint(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for value in [
            self.entry_region_evidence_report_fingerprint,
            self.final_region_binding_fingerprint,
            self.prior_inventory_report_fingerprint,
            self.footprint_report_fingerprint,
            self.resulting_inventory_report_fingerprint,
        ] {
            fingerprint_bytes(&mut hash, &value.to_le_bytes());
        }
        hash
    }

    pub fn validate_identity(self) -> bool {
        self.entry_region_evidence_report_fingerprint != 0
            && self.final_region_binding_fingerprint != 0
            && self.prior_inventory_report_fingerprint != 0
            && self.footprint_report_fingerprint != 0
            && self.resulting_inventory_report_fingerprint != 0
            && self.prior_inventory_digest != self.resulting_inventory_digest
            && self.evidence_digest == self.recomputed_evidence_digest()
            && self.evidence_report_fingerprint == self.recomputed_evidence_report_fingerprint()
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
    /// Collision-resistant commitment to the complete normalized validation
    /// summary. Imported compact identities remain visible report fields, but
    /// consumers must carry this digest rather than treating their aggregate
    /// FNV value as publication or replay authority.
    pub fn evidence_digest(self) -> CompilerFunctionValidationDigest {
        let mut digest = Sha256::new();
        digest.update(b"omega.compiler-function-validation.sha256.v1\0");
        for value in [
            self.function_count,
            self.instruction_count,
            self.zero_width_instruction_count,
            self.checked_assembly_instruction_count,
            self.fixed_mechanics_instruction_count,
            self.body_specification_instruction_count,
        ] {
            digest.update((value as u64).to_le_bytes());
        }
        for value in [
            self.fixed_mechanics_validation_fingerprint,
            self.fixed_mechanics_boundary_contract_fingerprint,
            self.fixed_mechanics_footprint_fingerprint,
            self.body_specification_validation_fingerprint,
            self.body_specification_boundary_contract_fingerprint,
            self.body_specification_footprint_fingerprint,
            self.composed_footprint_fingerprint,
            self.final_region_binding_fingerprint,
            self.validation_fingerprint,
        ] {
            digest.update(value.to_le_bytes());
        }
        CompilerFunctionValidationDigest::from_digest(digest.finalize().into())
    }

    /// Compact report compatibility only. This is not evidence, admission,
    /// publication, or replay authority; use [`Self::evidence_digest`].
    pub fn evidence_report_fingerprint(self) -> u64 {
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

macro_rules! compiler_text_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

compiler_text_digest!(EncodedCompilerTextDigest);
compiler_text_digest!(FinalCompilerTextDigest);
compiler_text_digest!(CompilerTextRelocationEnvelopeDigest);
compiler_text_digest!(CompilerTextDerivationDigest);

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
    pub encoded_text_fingerprint: u64,
    /// Legacy compact report fingerprint. It is not an authority key; use
    /// `final_compiler_text_digest` or exact byte replay.
    pub final_compiler_text_fingerprint: u64,
    /// Legacy compact report fingerprint. It is not an authority key; use the
    /// strong relocation-envelope digest.
    pub relocation_envelope_fingerprint: u64,
    /// Checked-assembly instructions whose fixed encoding or normalized
    /// privilege-bearing envelope was validated at retained final boundaries.
    pub checked_instruction_validation_fingerprint: u64,
    pub checked_instruction_footprint_fingerprint: u64,
    /// Legacy compact report fingerprint retained for current report
    /// compatibility. `derivation_digest` is the collision-resistant join.
    pub derivation_fingerprint: u64,
    pub text_relocation_count: usize,
    pub checked_instruction_validation_count: usize,
}

impl CompilerTextValidationEvidence {
    pub fn recomputed_derivation_digest(&self) -> CompilerTextDerivationDigest {
        let mut digest = Sha256::new();
        digest.update(b"omega.compiler-text-derivation.sha256.v1\0");
        digest.update(self.encoded_text_digest.as_bytes());
        digest.update(self.final_compiler_text_digest.as_bytes());
        digest.update(self.relocation_envelope_digest.as_bytes());
        digest.update(self.encoded_text_fingerprint.to_le_bytes());
        digest.update(self.final_compiler_text_fingerprint.to_le_bytes());
        digest.update(self.relocation_envelope_fingerprint.to_le_bytes());
        digest.update(
            self.checked_instruction_validation_fingerprint
                .to_le_bytes(),
        );
        digest.update(self.checked_instruction_footprint_fingerprint.to_le_bytes());
        digest.update((self.text_relocation_count as u64).to_le_bytes());
        digest.update((self.checked_instruction_validation_count as u64).to_le_bytes());
        CompilerTextDerivationDigest::from_digest(digest.finalize().into())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn text_evidence(encoded_digest: [u8; 32]) -> CompilerTextValidationEvidence {
        let mut evidence = CompilerTextValidationEvidence {
            encoded_text_digest: EncodedCompilerTextDigest::from_digest(encoded_digest),
            final_compiler_text_digest: FinalCompilerTextDigest::from_digest([2; 32]),
            relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([3; 32]),
            derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
            encoded_text_fingerprint: 11,
            final_compiler_text_fingerprint: 12,
            relocation_envelope_fingerprint: 13,
            checked_instruction_validation_fingerprint: 14,
            checked_instruction_footprint_fingerprint: 15,
            derivation_fingerprint: 16,
            text_relocation_count: 17,
            checked_instruction_validation_count: 18,
        };
        evidence.derivation_digest = evidence.recomputed_derivation_digest();
        evidence
    }

    fn function_evidence() -> CompilerFunctionValidationEvidence {
        CompilerFunctionValidationEvidence {
            function_count: 1,
            instruction_count: 2,
            zero_width_instruction_count: 3,
            checked_assembly_instruction_count: 4,
            fixed_mechanics_instruction_count: 5,
            fixed_mechanics_validation_fingerprint: 6,
            fixed_mechanics_boundary_contract_fingerprint: 7,
            fixed_mechanics_footprint_fingerprint: 8,
            body_specification_instruction_count: 9,
            body_specification_validation_fingerprint: 10,
            body_specification_boundary_contract_fingerprint: 11,
            body_specification_footprint_fingerprint: 12,
            composed_footprint_fingerprint: 13,
            final_region_binding_fingerprint: 14,
            validation_fingerprint: 15,
        }
    }

    #[test]
    fn compact_collision_cannot_substitute_strong_compiler_text_evidence() {
        let first = text_evidence([1; 32]);
        let second = text_evidence([99; 32]);

        assert_eq!(
            first.encoded_text_fingerprint,
            second.encoded_text_fingerprint
        );
        assert_eq!(first.derivation_fingerprint, second.derivation_fingerprint);
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
                drifted.fixed_mechanics_validation_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.body_specification_footprint_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.final_region_binding_fingerprint ^= 1;
                drifted
            },
            {
                let mut drifted = evidence;
                drifted.validation_fingerprint ^= 1;
                drifted
            },
        ] {
            assert_ne!(drifted.evidence_digest(), expected);
        }
    }
}
