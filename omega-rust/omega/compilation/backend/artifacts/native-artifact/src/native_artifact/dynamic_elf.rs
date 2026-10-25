//! Non-installable dynamic-ELF artifact construction and replay.

use super::identity::{
    NativeArtifactIdentityFields, compiler_identity_evidence, derive_native_artifact_identity,
    foreign_call_custody_digest,
};
use super::{
    NativeArtifactParts, NativePhysicalEvidence, NativePhysicalEvidenceDerivation,
    NativePhysicalEvidenceGap, NativeProviderExecution, NativeSelectedProviderPlan,
    boundary_application_coverage_identity, derive_physical_evidence, mixed_structural_scalar,
    validate_boundary_application_coverage, validate_ieee_float_fma_occurrences,
};
use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use installation_evidence::ObjectEvidence;
use sha2::{Digest, Sha256};
const DYNAMIC_ELF_NATIVE_ARTIFACT_IDENTITY_DOMAIN: &[u8] =
    b"omega.dynamic-elf-native-artifact.sha256.v1\0";

/// Collision-resistant identity of one non-installable dynamic-ELF native
/// candidate.  This identity is domain-separated from [`crate::NativeArtifactIdentity`]
/// so equal output bytes cannot collapse the two authority classes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicElfNativeArtifactIdentity([u8; 32]);

impl DynamicElfNativeArtifactIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for DynamicElfNativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for DynamicElfNativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Complete source-free native custody after an import-bearing Linux object
/// has selected the dynamic-ELF writer.
///
/// This is intentionally not [`crate::NativeArtifact`].  Existing component,
/// installation, and publication APIs accept only that direct-image type and
/// therefore cannot treat dynamic byte custody as loader or execution
/// authority.
#[derive(Debug)]
#[must_use = "dynamic ELF native custody is non-installable and must be replayed"]
pub struct DynamicElfNativeArtifact {
    parts: DynamicElfNativeArtifactParts,
    /// The exact subject that stopped the scoped derivation when
    /// `parts.physical_evidence` is absent. Derived here, never retained as a
    /// claim: `NativeArtifactParts` deliberately has no slot for it.
    physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    identity: DynamicElfNativeArtifactIdentity,
}

/// Reconstructible parts of one non-installable dynamic-ELF native candidate:
/// the shared native publication row, carrying this product's image request.
pub type DynamicElfNativeArtifactParts =
    NativeArtifactParts<image_emission::RequestedDynamicElfImage>;

/// Fresh dynamic writer inputs.  Physical evidence is rederived from the
/// canonical Terminal/object/final-image join rather than accepted from the
/// caller.
pub(crate) type DynamicElfNativeArtifactEmissionParts =
    super::NativeArtifactEmissionParts<image_emission::RequestedDynamicElfImage>;

impl DynamicElfNativeArtifact {
    /// Complete fresh source-free dynamic emission by deriving every physical
    /// child that is supported by the exact Terminal/object/final-image join.
    /// The artifact is sealed over the parts this compile just produced; it is
    /// not replayed. [`Self::from_replayed_parts`] and [`Self::validate`]
    /// replay it for retained-artifact readers.
    pub(crate) fn from_emitted_parts(
        parts: DynamicElfNativeArtifactEmissionParts,
    ) -> Result<Self, &'static str> {
        let (parts, physical_evidence_gap) =
            Self::derivation_of(&parts).map(|derivation| parts.into_parts(derivation))?;
        Ok(Self::seal(parts, physical_evidence_gap))
    }

    /// The physical children of the requested dynamic image, over the exact
    /// Terminal/object/final-image join this compile just produced. The
    /// admitted image inside the writer request is this format's digest source.
    fn derivation_of(
        parts: &DynamicElfNativeArtifactEmissionParts,
    ) -> Result<NativePhysicalEvidenceDerivation, &'static str> {
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(parts.image.emission().admitted().image()).as_bytes();
        derive_physical_evidence(
            &parts.physical_evidence_scope,
            &parts.psi_artifact,
            parts.target,
            &parts.object,
            parts.image.output(),
            final_image_symbol_digest,
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )
    }

    /// Reconstruct a retained candidate while independently replaying every
    /// source-free field.  No installation or publication authority is
    /// produced by successful replay.
    pub fn from_replayed_parts(parts: DynamicElfNativeArtifactParts) -> Result<Self, &'static str> {
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(parts.image.emission().admitted().image()).as_bytes();
        // The gap is derivation state, not a retained claim: it is recomputed
        // from the replayed inputs so a retained candidate can never assert a
        // blocking subject the derivation would not have produced.
        let physical_evidence_gap = match derive_physical_evidence(
            &parts.physical_evidence_scope,
            &parts.psi_artifact,
            parts.target,
            &parts.object,
            parts.image.output(),
            final_image_symbol_digest,
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )? {
            NativePhysicalEvidenceDerivation::Blocked(gap) => Some(gap),
            NativePhysicalEvidenceDerivation::Unavailable
            | NativePhysicalEvidenceDerivation::Complete(_) => None,
        };
        let artifact = Self::seal(parts, physical_evidence_gap);
        artifact.validate()?;
        Ok(artifact)
    }

    /// Assemble the artifact and its identity over `parts` and the derived
    /// evidence gap.
    fn seal(
        parts: DynamicElfNativeArtifactParts,
        physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    ) -> Self {
        let mut artifact = Self {
            parts,
            physical_evidence_gap,
            identity: DynamicElfNativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity();
        artifact
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let semantic = self.parts.psi_artifact.manifest().semantic();
        if self.parts.object.psi() != semantic {
            return Err("dynamic native artifact semantic identity disagrees with its object");
        }
        if self.parts.object.target() != self.parts.target
            || self.parts.image.emission().admitted().image().target != self.parts.target
        {
            return Err("dynamic native artifact target disagrees with its object or image");
        }
        if self.parts.image.artifact() != &self.parts.object {
            return Err("dynamic native artifact image retained a substituted object");
        }
        image_emission::validate_requested_dynamic_elf_image(&self.parts.object, &self.parts.image)
            .map_err(|_| "dynamic native artifact failed exact requested-image replay")?;
        if !self.parts.terminal_authority_closure_matches() {
            return Err(
                "dynamic native artifact closure receipt drifted from its exact realization inputs",
            );
        }
        let module = self.parts.psi_artifact.semantic_module();
        mixed_structural_scalar::validate(module, &self.parts.object)?;
        validate_boundary_application_coverage(
            module,
            semantic,
            self.parts.boundary_application_coverage.as_ref(),
            &self.parts.physical_evidence_scope,
        )?;
        validate_ieee_float_fma_occurrences(module)?;
        self.parts
            .object
            .validate_terminal_entry(&self.parts.psi_artifact)?;
        if self.parts.selected_provider_closure_report_identity == 0 {
            return Err(
                "dynamic native artifact selected provider closure has the reserved zero identity",
            );
        }

        super::provider_execution::replay_provider_executions(
            module,
            self.parts.object.boundary_settlements(),
            self.parts.object.foreign_calls(),
            &self.parts.selected_provider_plans,
            &self.parts.provider_executions,
        )
        .map_err(super::provider_execution::ProviderExecutionReplayError::dynamic_message)?;
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(self.parts.image.emission().admitted().image())
                .as_bytes();
        let (expected_physical_evidence, expected_physical_evidence_gap) =
            match derive_physical_evidence(
                &self.parts.physical_evidence_scope,
                &self.parts.psi_artifact,
                self.parts.target,
                &self.parts.object,
                self.parts.image.output(),
                final_image_symbol_digest,
                &self.parts.selected_provider_plans,
                &self.parts.provider_executions,
                self.parts.boundary_application_coverage.as_ref(),
            )? {
                NativePhysicalEvidenceDerivation::Unavailable => (None, None),
                NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
                NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
            };
        if self.parts.physical_evidence != expected_physical_evidence {
            return Err("dynamic native artifact physical children disagree with exact replay");
        }
        if self.physical_evidence_gap != expected_physical_evidence_gap {
            return Err(
                "dynamic native artifact physical evidence gap disagrees with exact derivation replay",
            );
        }
        if self.identity != self.recomputed_identity() {
            return Err("dynamic native artifact identity disagrees with retained custody");
        }
        Ok(())
    }

    fn recomputed_identity(&self) -> DynamicElfNativeArtifactIdentity {
        let output = self.parts.image.output();
        let (
            compiler_text_validation_digest,
            compiler_function_validation,
            compiler_entry_region_binding,
            compiler_entry_footprint_binding,
        ) = compiler_identity_evidence(output);
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(self.parts.image.emission().admitted().image())
                .as_bytes();
        let base = derive_native_artifact_identity(NativeArtifactIdentityFields {
            terminal_artifact_identity: *self.parts.psi_artifact.manifest().identity().as_bytes(),
            target: self.parts.target,
            object_text_bytes: self.parts.object.text_bytes(),
            image_bytes: &output.bytes,
            final_text_bytes: &output.final_text_bytes,
            image_subsystem: None,
            output_file_name: &output.file_name,
            output_format: &output.format,
            output_counts: [
                output.text_bytes,
                output.data_bytes,
                output.bss_bytes,
                output.symbols,
                output.relocations,
                output.final_image_symbols,
                output.final_image_imports,
                output.final_image_relocations,
            ],
            callback_placement_identity_report_fingerprint: output
                .callback_placement_identity_report_fingerprint,
            final_image_symbol_digest,
            executable_region_inventory_digest: *output
                .executable_regions
                .inventory_digest
                .as_bytes(),
            executable_region_inventory_report_fingerprint: output
                .executable_regions
                .inventory_report_fingerprint,
            compiler_text_validation_digest,
            compiler_function_validation,
            compiler_entry_region_binding,
            compiler_entry_footprint_binding,
            selected_provider_closure_digest: *self
                .parts
                .selected_provider_closure_digest
                .as_bytes(),
            foreign_call_custody_digest: foreign_call_custody_digest(
                self.parts.object.foreign_calls(),
            ),
            selected_provider_plans: &self.parts.selected_provider_plans,
            provider_executions: &self.parts.provider_executions,
            terminal_authority_policy_identity: self.parts.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: self
                .parts
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review_identity: self
                .parts
                .terminal_authority_closure_review
                .identity(),
            boundary_application_coverage_identity: boundary_application_coverage_identity(
                self.parts.boundary_application_coverage.as_ref(),
            ),
            physical_evidence_scope: &self.parts.physical_evidence_scope,
            physical_evidence_identity: self
                .parts
                .physical_evidence
                .as_ref()
                .map(|evidence| *evidence.identity()),
            physical_evidence_gap_identity: self
                .physical_evidence_gap
                .as_ref()
                .map(|gap| *gap.identity()),
        });
        let mut digest = Sha256::new();
        digest.update(DYNAMIC_ELF_NATIVE_ARTIFACT_IDENTITY_DOMAIN);
        digest.update(base.as_bytes());
        DynamicElfNativeArtifactIdentity(digest.finalize().into())
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.parts.target
    }

    pub fn psi_artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.parts.psi_artifact
    }

    pub const fn object(&self) -> &image_emission::ObjectArtifact {
        &self.parts.object
    }

    pub const fn image(&self) -> &image_emission::RequestedDynamicElfImage {
        &self.parts.image
    }

    pub const fn identity(&self) -> DynamicElfNativeArtifactIdentity {
        self.identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.parts.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.parts.provider_executions
    }

    pub const fn boundary_application_coverage(
        &self,
    ) -> Option<&TerminalBoundaryApplicationCoverage> {
        self.parts.boundary_application_coverage.as_ref()
    }

    pub const fn physical_evidence(&self) -> Option<&NativePhysicalEvidence> {
        self.parts.physical_evidence.as_ref()
    }

    /// The exact subject that stopped this candidate's scoped D32 derivation
    /// when [`Self::physical_evidence`] is absent. `None` means the evidence
    /// is complete or the scope admits no derivation at all; a `Some` value
    /// names the first occurrence or retained record the derivation could
    /// not bind to a physical child.
    pub const fn physical_evidence_gap(&self) -> Option<&NativePhysicalEvidenceGap> {
        self.physical_evidence_gap.as_ref()
    }

    pub fn into_parts(self) -> DynamicElfNativeArtifactParts {
        self.parts
    }
}
