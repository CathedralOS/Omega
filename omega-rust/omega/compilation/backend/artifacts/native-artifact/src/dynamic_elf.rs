/*
We construct, seal, and replay the non-installable Dynamic-ELF native artifact
(`DynamicElfNativeArtifact`). We keep `DynamicElfNativeArtifactIdentity`
domain-separated from `NativeArtifactIdentity` and replay boundary settlements
and foreign calls from the retained object after validating the requested
dynamic ELF image.
*/

use super::identity::derive_native_artifact_identity;
use super::{
    NativeArtifactParts, NativePhysicalEvidence, NativePhysicalEvidenceDerivation,
    NativePhysicalEvidenceGap, NativeProviderExecution, NativeSelectedProviderPlan,
    derive_physical_evidence, mixed_structural_scalar, validate_boundary_application_coverage,
    validate_ieee_float_fma_occurrences,
};
use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use sha2::{Digest, Sha256};
const DYNAMIC_ELF_NATIVE_ARTIFACT_IDENTITY_DOMAIN: &[u8] =
    b"omega.dynamic-elf-native-artifact.sha256.v1\0";

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

#[derive(Debug)]
#[must_use = "dynamic ELF native custody is non-installable and must be replayed"]
pub struct DynamicElfNativeArtifact {
    parts: DynamicElfNativeArtifactParts,
    physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    identity: DynamicElfNativeArtifactIdentity,
}

pub type DynamicElfNativeArtifactParts =
    NativeArtifactParts<image_emission::RequestedDynamicElfImage>;

pub(crate) type DynamicElfNativeArtifactEmissionParts =
    super::NativeArtifactEmissionParts<image_emission::RequestedDynamicElfImage>;

impl DynamicElfNativeArtifact {
    pub(crate) fn from_emitted_parts(
        parts: DynamicElfNativeArtifactEmissionParts,
    ) -> Result<Self, &'static str> {
        let (parts, physical_evidence_gap) =
            Self::derivation_of(&parts).map(|derivation| parts.into_parts(derivation))?;
        Ok(Self::seal(parts, physical_evidence_gap))
    }

    fn derivation_of(
        parts: &DynamicElfNativeArtifactEmissionParts,
    ) -> Result<NativePhysicalEvidenceDerivation, &'static str> {
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(parts.image.emission().admitted().image()).as_bytes();
        derive_physical_evidence(
            super::PhysicalEvidencePass::Publication,
            &parts.physical_evidence_scope,
            parts.psi_artifact.view().into(),
            parts.target,
            &parts.object,
            &parts.image.output().final_text_bytes,
            final_image_symbol_digest,
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )
    }

    pub fn from_replayed_parts(parts: DynamicElfNativeArtifactParts) -> Result<Self, &'static str> {
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(parts.image.emission().admitted().image()).as_bytes();
        let physical_evidence_gap =
            parts.replayed_gap(parts.image.output(), final_image_symbol_digest)?;
        let artifact = Self::seal(parts, physical_evidence_gap);
        artifact.validate()?;
        Ok(artifact)
    }

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
        let execution_scope = self.parts.object.native_execution_scope(
            &self.parts.psi_artifact,
            *self.parts.selected_provider_closure_digest.as_bytes(),
        )?;
        validate_boundary_application_coverage(
            module,
            semantic,
            self.parts.boundary_application_coverage.as_ref(),
            &self.parts.physical_evidence_scope,
            execution_scope,
        )?;
        validate_ieee_float_fma_occurrences(module, execution_scope)?;
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
        let (expected_physical_evidence, expected_physical_evidence_gap) = self
            .parts
            .replayed_evidence(self.parts.image.output(), final_image_symbol_digest)?;
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
        let base = derive_native_artifact_identity(
            self.parts.identity_fields(
                self.parts.image.output(),
                *image::final_image_symbol_digest(self.parts.image.emission().admitted().image())
                    .as_bytes(),
                self.parts.object.foreign_calls(),
                None,
                self.physical_evidence_gap.as_ref(),
            ),
        );
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

    pub const fn physical_evidence_gap(&self) -> Option<&NativePhysicalEvidenceGap> {
        self.physical_evidence_gap.as_ref()
    }

    pub fn into_parts(self) -> DynamicElfNativeArtifactParts {
        self.parts
    }
}
