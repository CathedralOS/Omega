/*
We construct, seal, and replay the Terminal/object/image join for direct native
artifacts (`NativeArtifact`) and dispatch requested image emission across the
authority-distinct Direct (`NativeArtifact`) and Dynamic-ELF
(`DynamicElfNativeArtifact`) products (`RequestedNativeArtifact`). We also project
source-free provider selections (`NativeSelectedProviderPlan`) and provider
execution reports (`NativeProviderExecution`), validate D29 boundary-application
coverage (`validate_boundary_application_coverage`), and seal domain-separated
artifact identities (`NativeArtifactIdentity`).
*/

use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use effects::{
    SelectedExternalRequirementApplication, SelectedProviderPlanFacts,
    TerminalAuthorityClosureReviewReceipt, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity,
};
use image_emission::{ExecutableImageEmissionRequest, RequestedExecutableImage};
use installation_evidence::ProviderExecutionEvidence;
use terminal_psi::{TerminalIdentityTable, TerminalModule};
use terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeProviders;

mod dynamic_elf;
mod identity;
mod mixed_structural_scalar;
mod provider_execution;
#[cfg(test)]
mod tests;

use crate::physical::{
    self, NativePhysicalEvidence, NativePhysicalEvidenceDerivation, NativePhysicalEvidenceGap,
    PhysicalEvidencePass, ValidatedOptimizedNativePhysicalEvidenceScope, canonical_usize,
    derive_physical_evidence,
};
use crate::publication::publication_digest;
pub use dynamic_elf::{DynamicElfNativeArtifact, DynamicElfNativeArtifactIdentity};
use identity::{
    NativeArtifactIdentityFields, compiler_identity_evidence, derive_native_artifact_identity,
    foreign_call_custody_digest,
};

const CUSTODY_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.d29-custody.sha256.v3\0";

#[derive(Debug)]
pub enum NativeArtifactValidationError {
    Invariant(&'static str),
    ImageReplay(Diagnostic),
}

impl From<&'static str> for NativeArtifactValidationError {
    fn from(reason: &'static str) -> Self {
        Self::Invariant(reason)
    }
}

impl std::fmt::Display for NativeArtifactValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invariant(reason) => f.write_str(reason),
            Self::ImageReplay(diagnostic) => {
                write!(f, "native artifact image replay: {}", diagnostic.message)
            }
        }
    }
}

impl std::error::Error for NativeArtifactValidationError {}

impl From<NativeArtifactValidationError> for String {
    fn from(error: NativeArtifactValidationError) -> Self {
        error.to_string()
    }
}

impl NativeArtifactValidationError {
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::ImageReplay(diagnostic) => diagnostic,
            Self::Invariant(reason) => Diagnostic::error(reason),
        }
    }
}

pub(super) fn validate_boundary_application_coverage(
    module: &terminal_psi::TerminalModule,
    terminal: terminal_psi::TerminalPsiIdentity,
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
    physical_evidence_scope: &NativePhysicalEvidenceScope,
    execution_scope: Option<&terminal_psi_to_abstract_operations::NativeExecutionScope>,
) -> Result<(), &'static str> {
    // A retained native scope establishes which calls must have selected rows.
    // Without that custody, keep the full-module obligation; absence is not an
    // empty executable closure.
    let expected = execution_scope
        .map(|scope| scope.boundary_application_demands(module))
        .transpose()?;
    let demands = expected
        .as_deref()
        .unwrap_or(&module.boundary_application_demands);
    let Some(coverage) = coverage else {
        if !demands.is_empty() || physical_evidence_scope.requires_boundary_application_coverage() {
            return Err(
                "native artifact claims an exact empty D29 scope without retained coverage",
            );
        }
        return Ok(());
    };
    coverage.validate_for_terminal(terminal)?;
    if coverage.demands().rows() != demands {
        return Err("native boundary coverage does not match canonical executable demands");
    }
    for reference in coverage.references() {
        let matching_operations = module
            .operations()
            .filter(|operation| operation.id == reference.terminal_operation())
            .count();
        if matching_operations != 1 {
            return Err(
                "native artifact boundary application does not rejoin one Terminal operation",
            );
        }
    }
    Ok(())
}

pub(crate) fn boundary_application_coverage_identity(
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Option<[u8; 32]> {
    let coverage = coverage?;
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(CUSTODY_IDENTITY_DOMAIN);
    digest.raw(&canonical_usize(coverage.references().len()));
    for reference in coverage.references() {
        digest.raw(&reference.terminal().vocabulary_marker.get().to_le_bytes());
        digest.raw(reference.terminal().program_fingerprint.as_bytes());
        digest.raw(&reference.terminal_operation().get().to_le_bytes());
        digest.raw(reference.coverage().as_bytes());
    }
    let applications = coverage.opaque_applications();
    digest.raw(&canonical_usize(applications.rows().len()));
    for application in applications.rows() {
        digest.raw(&application.requirement_identity.0.to_le_bytes());
        digest.string(&application.abi.requirement_identity);
        digest.string(&application.abi.declaring_application);
        digest.raw(&canonical_usize(application.abi.operands.len()));
        for operand in &application.abi.operands {
            digest.raw(&operand.ordinal.to_le_bytes());
            digest.byte(operand.kind as u8);
            digest.string(&operand.identity);
        }
        digest.raw(&application.shape_root.to_le_bytes());
        digest.raw(&application.application_report_fingerprint.to_le_bytes());
        digest.raw(&application.selected_application_commitment);
    }
    Some(digest.finish())
}

#[derive(Debug)]
#[must_use = "requested native realization retains its authority-distinct image custody"]
pub enum RequestedNativeArtifact {
    Direct(NativeArtifact),
    DynamicElf(DynamicElfNativeArtifact),
}

impl RequestedNativeArtifact {
    pub fn emit_admitted(
        target: target::NativeTarget,
        terminal: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        published: crate::PublishedFragmentObject,
        image: ExecutableImageEmissionRequest,
        selected: &effects::SelectedProviderPlanFacts,
        providers: AdmittedNativeProviders<'_>,
        boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let terminal_authority_closure_review =
            providers.terminal_authority_closure_review().clone();
        let coverage = boundary_application_coverage.as_ref().ok_or_else(|| {
            crate::native_artifact_error(
                "physical publication",
                "missing boundary-application coverage",
            )
        })?;
        let physical_evidence_scope =
            NativePhysicalEvidenceScope::from_fragment_publication(coverage, &published.object)
                .map_err(|reason| crate::native_artifact_error("physical publication", reason))?;
        Self::emit(NativeArtifactEmissionParts {
            target,
            psi_artifact: terminal,
            object: published.object,
            image,
            selected_provider_closure_report_identity: selected.compatibility_report_identity(),
            selected_provider_closure_digest: NativeSelectedProviderClosureDigest::from_digest(
                *selected.identity_digest().as_bytes(),
            ),
            selected_provider_plans: NativeSelectedProviderPlan::project_selection(selected),
            provider_executions: providers
                .executions
                .into_iter()
                .map(NativeProviderExecution::from_evidence)
                .collect(),
            terminal_authority_policy_identity: providers.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: providers
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review,
            boundary_application_coverage,
            physical_evidence_scope,
        })
    }

    pub fn emit(
        parts: NativeArtifactEmissionParts<ExecutableImageEmissionRequest>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let image = image_emission::emit_executable_image(&parts.object, parts.image.clone())
            .map_err(|error| vec![error.diagnostic().clone()])?;
        match image {
            RequestedExecutableImage::Direct(image) => {
                NativeArtifact::from_emitted_parts(with_emitted_image(parts, image))
                    .map(Self::Direct)
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "native artifact native artifact replay failed: {error}"
                        ))]
                    })
            }
            RequestedExecutableImage::DynamicElf(image) => {
                DynamicElfNativeArtifact::from_emitted_parts(with_emitted_image(parts, image))
                    .map(Self::DynamicElf)
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "native artifact dynamic ELF native artifact replay failed: {error}"
                        ))]
                    })
            }
        }
    }
}

fn with_emitted_image<Image, EmittedImage>(
    parts: NativeArtifactEmissionParts<Image>,
    image: EmittedImage,
) -> NativeArtifactEmissionParts<EmittedImage> {
    NativeArtifactEmissionParts {
        target: parts.target,
        psi_artifact: parts.psi_artifact,
        object: parts.object,
        image,
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans,
        provider_executions: parts.provider_executions,
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review,
        boundary_application_coverage: parts.boundary_application_coverage,
        physical_evidence_scope: parts.physical_evidence_scope,
    }
}

#[derive(Debug)]
#[must_use = "a native artifact owns the canonical semantic and target artifact join"]
pub struct NativeArtifact {
    parts: NativeArtifactParts,
    physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    identity: NativeArtifactIdentity,
}

#[derive(Debug)]
pub struct NativeArtifactParts<Image = image_emission::ExecutableImage> {
    pub target: target::NativeTarget,
    pub psi_artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub object: image_emission::ObjectArtifact,
    pub image: Image,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
    pub physical_evidence: Option<NativePhysicalEvidence>,
}

#[derive(Debug)]
pub struct NativeArtifactEmissionParts<Image = image_emission::ExecutableImage> {
    pub target: target::NativeTarget,
    pub psi_artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub object: image_emission::ObjectArtifact,
    pub image: Image,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
}

impl<Image> NativeArtifactEmissionParts<Image> {
    fn into_parts(
        self,
        derivation: NativePhysicalEvidenceDerivation,
    ) -> (
        NativeArtifactParts<Image>,
        Option<NativePhysicalEvidenceGap>,
    ) {
        let (physical_evidence, physical_evidence_gap) = match derivation {
            NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
            NativePhysicalEvidenceDerivation::Unavailable => (None, None),
            NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
        };
        (
            NativeArtifactParts {
                target: self.target,
                psi_artifact: self.psi_artifact,
                object: self.object,
                image: self.image,
                selected_provider_closure_report_identity: self
                    .selected_provider_closure_report_identity,
                selected_provider_closure_digest: self.selected_provider_closure_digest,
                selected_provider_plans: self.selected_provider_plans,
                provider_executions: self.provider_executions,
                terminal_authority_policy_identity: self.terminal_authority_policy_identity,
                terminal_authority_permission_policy_identity: self
                    .terminal_authority_permission_policy_identity,
                terminal_authority_closure_review: self.terminal_authority_closure_review,
                boundary_application_coverage: self.boundary_application_coverage,
                physical_evidence_scope: self.physical_evidence_scope,
                physical_evidence,
            },
            physical_evidence_gap,
        )
    }
}

impl<Image> NativeArtifactParts<Image> {
    fn terminal_authority_closure_matches(&self) -> bool {
        self.terminal_authority_closure_review
            .terminal_artifact_identity()
            == *self.psi_artifact.manifest().identity().as_bytes()
            && self.terminal_authority_closure_review.target() == self.target
            && self
                .terminal_authority_closure_review
                .selected_provider_closure()
                .as_bytes()
                == self.selected_provider_closure_digest.as_bytes()
            && self.terminal_authority_closure_review.physical_policy()
                == self.terminal_authority_policy_identity
            && self.terminal_authority_closure_review.permission_policy()
                == self.terminal_authority_permission_policy_identity
    }

    fn identity_fields<'a>(
        &'a self,
        output: &'a image::EmittedImageOutput,
        final_image_symbol_digest: [u8; 32],
        foreign_calls: &[image_emission::ObjectForeignCall],
        image_subsystem: Option<u16>,
        physical_evidence_gap: Option<&NativePhysicalEvidenceGap>,
    ) -> NativeArtifactIdentityFields<'a> {
        let (
            compiler_text_validation_digest,
            compiler_function_validation,
            compiler_entry_region_binding,
            compiler_entry_footprint_binding,
        ) = compiler_identity_evidence(output);
        NativeArtifactIdentityFields {
            installation_assumptions: self.object.installation_environment_assumptions(),
            physical_entry_requirements: self
                .object
                .physical_entry_binding()
                .map(|binding| binding.requirements()),
            terminal_artifact_identity: *self.psi_artifact.manifest().identity().as_bytes(),
            target: self.target,
            object_text_bytes: self.object.text_bytes(),
            image_bytes: &output.bytes,
            final_text_bytes: &output.final_text_bytes,
            image_subsystem,
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
            selected_provider_closure_digest: *self.selected_provider_closure_digest.as_bytes(),
            foreign_call_custody_digest: foreign_call_custody_digest(foreign_calls),
            selected_provider_plans: &self.selected_provider_plans,
            provider_executions: &self.provider_executions,
            terminal_authority_policy_identity: self.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: self
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review_identity: self
                .terminal_authority_closure_review
                .identity(),
            boundary_application_coverage_identity: boundary_application_coverage_identity(
                self.boundary_application_coverage.as_ref(),
            ),
            physical_evidence_scope: &self.physical_evidence_scope,
            physical_evidence_identity: self
                .physical_evidence
                .as_ref()
                .map(|evidence| *evidence.identity()),
            physical_evidence_gap_identity: physical_evidence_gap.map(|gap| *gap.identity()),
        }
    }

    fn replayed_gap(
        &self,
        output: &image::EmittedImageOutput,
        final_image_symbol_digest: [u8; 32],
    ) -> Result<Option<NativePhysicalEvidenceGap>, &'static str> {
        Ok(
            match self.evidence_derivation(output, final_image_symbol_digest)? {
                NativePhysicalEvidenceDerivation::Blocked(gap) => Some(gap),
                NativePhysicalEvidenceDerivation::Unavailable
                | NativePhysicalEvidenceDerivation::Complete(_) => None,
            },
        )
    }

    fn replayed_evidence(
        &self,
        output: &image::EmittedImageOutput,
        final_image_symbol_digest: [u8; 32],
    ) -> Result<
        (
            Option<NativePhysicalEvidence>,
            Option<NativePhysicalEvidenceGap>,
        ),
        &'static str,
    > {
        Ok(
            match self.evidence_derivation(output, final_image_symbol_digest)? {
                NativePhysicalEvidenceDerivation::Unavailable => (None, None),
                NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
                NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
            },
        )
    }

    fn evidence_derivation(
        &self,
        image_output: &image::EmittedImageOutput,
        final_image_symbol_digest: [u8; 32],
    ) -> Result<NativePhysicalEvidenceDerivation, &'static str> {
        derive_physical_evidence(
            PhysicalEvidencePass::Receiver,
            &self.physical_evidence_scope,
            self.psi_artifact.view().into(),
            self.target,
            &self.object,
            &image_output.final_text_bytes,
            final_image_symbol_digest,
            &self.selected_provider_plans,
            &self.provider_executions,
            self.boundary_application_coverage.as_ref(),
        )
    }
}

impl NativeArtifact {
    pub fn from_emitted_parts(parts: NativeArtifactEmissionParts) -> Result<Self, &'static str> {
        if parts.image.foreign_calls() != parts.object.foreign_calls() {
            return Err("native artifact image differs from fragment foreign call custody");
        }
        let (parts, physical_evidence_gap) =
            Self::derivation_of(&parts).map(|derivation| parts.into_parts(derivation))?;
        Ok(Self::seal(parts, physical_evidence_gap))
    }

    fn derivation_of(
        parts: &NativeArtifactEmissionParts,
    ) -> Result<NativePhysicalEvidenceDerivation, &'static str> {
        derive_physical_evidence(
            PhysicalEvidencePass::Publication,
            &parts.physical_evidence_scope,
            parts.psi_artifact.view().into(),
            parts.target,
            &parts.object,
            &parts.image.output().final_text_bytes,
            *parts.image.final_image_symbol_digest().as_bytes(),
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )
    }

    pub fn from_replayed_parts(
        parts: NativeArtifactParts,
    ) -> Result<Self, NativeArtifactValidationError> {
        let physical_evidence_gap = parts.replayed_gap(
            parts.image.output(),
            *parts.image.final_image_symbol_digest().as_bytes(),
        )?;
        let artifact = Self::seal(parts, physical_evidence_gap);
        artifact.validate()?;
        Ok(artifact)
    }

    fn seal(
        parts: NativeArtifactParts,
        physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    ) -> Self {
        let mut artifact = Self {
            parts,
            physical_evidence_gap,
            identity: NativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity();
        artifact
    }

    pub fn validate(&self) -> Result<(), NativeArtifactValidationError> {
        let semantic = self.parts.psi_artifact.manifest().semantic();
        if self.parts.object.psi() != semantic || self.parts.image.psi() != semantic {
            return Err(
                "native artifact semantic identity disagrees with its object or image".into(),
            );
        }
        if self.parts.object.target() != self.parts.target
            || self.parts.image.target() != self.parts.target
        {
            return Err("native artifact target disagrees with its object or image".into());
        }
        if !self.parts.terminal_authority_closure_matches() {
            return Err("native artifact terminal-authority closure receipt drifted from its exact realization inputs".into());
        }
        image_emission::validate_direct_executable_image(&self.parts.object, &self.parts.image)
            .map_err(NativeArtifactValidationError::ImageReplay)?;
        let module = self.parts.psi_artifact.semantic_module();
        mixed_structural_scalar::validate(module, &self.parts.object)?;
        let execution_scope = self.parts.object.native_execution_scope(
            &self.parts.psi_artifact,
            *self.parts.selected_provider_closure_digest.as_bytes(),
        )?;
        validate_boundary_application_coverage(
            module,
            self.parts.psi_artifact.manifest().semantic(),
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
                "native artifact selected provider closure has the reserved zero identity".into(),
            );
        }

        provider_execution::replay_provider_executions(
            module,
            self.parts.image.boundary_settlements(),
            self.parts.image.foreign_calls(),
            &self.parts.selected_provider_plans,
            &self.parts.provider_executions,
        )
        .map_err(provider_execution::ProviderExecutionReplayError::direct_message)?;
        let final_image_symbol_digest = *self.parts.image.final_image_symbol_digest().as_bytes();
        let (expected_physical_evidence, expected_physical_evidence_gap) = self
            .parts
            .replayed_evidence(self.parts.image.output(), final_image_symbol_digest)?;
        if self.parts.physical_evidence != expected_physical_evidence {
            return Err(
                "native artifact physical children disagree with its validated identity projection"
                    .into(),
            );
        }
        if self.physical_evidence_gap != expected_physical_evidence_gap {
            return Err(
                "native artifact physical evidence gap disagrees with exact derivation replay"
                    .into(),
            );
        }
        if self.identity != self.recomputed_identity() {
            return Err("native artifact identity disagrees with its retained authority".into());
        }
        Ok(())
    }

    fn recomputed_identity(&self) -> NativeArtifactIdentity {
        derive_native_artifact_identity(self.parts.identity_fields(
            self.parts.image.output(),
            *self.parts.image.final_image_symbol_digest().as_bytes(),
            self.parts.image.foreign_calls(),
            self.parts.image.subsystem(),
            self.physical_evidence_gap.as_ref(),
        ))
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.parts.target
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.parts.psi_artifact.semantic_bytes()
    }

    pub fn psi_artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.parts.psi_artifact
    }

    pub const fn object(&self) -> &image_emission::ObjectArtifact {
        &self.parts.object
    }

    pub const fn image(&self) -> &image_emission::ExecutableImage {
        &self.parts.image
    }

    pub fn inspect_native_code_layouts(
        &self,
    ) -> Result<Vec<image_emission::LinkedNativeCodeLayout>, diagnostics::Diagnostic> {
        self.validate()
            .map_err(NativeArtifactValidationError::into_diagnostic)?;
        image_emission::inspect_native_code_layouts(&self.parts.object, &self.parts.image)
    }

    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.parts.selected_provider_closure_report_identity
    }

    pub const fn selected_provider_closure_digest(&self) -> NativeSelectedProviderClosureDigest {
        self.parts.selected_provider_closure_digest
    }

    pub const fn identity(&self) -> NativeArtifactIdentity {
        self.identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.parts.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.parts.provider_executions
    }

    pub const fn terminal_authority_policy_identity(&self) -> TerminalAuthorityPolicyIdentity {
        self.parts.terminal_authority_policy_identity
    }

    pub const fn terminal_authority_permission_policy_identity(
        &self,
    ) -> Option<TerminalAuthorityPermissionPolicyIdentity> {
        self.parts.terminal_authority_permission_policy_identity
    }

    pub const fn terminal_authority_closure_review(
        &self,
    ) -> &TerminalAuthorityClosureReviewReceipt {
        &self.parts.terminal_authority_closure_review
    }

    pub const fn boundary_application_coverage(
        &self,
    ) -> Option<&TerminalBoundaryApplicationCoverage> {
        self.parts.boundary_application_coverage.as_ref()
    }

    pub fn physical_evidence_scope(&self) -> NativePhysicalEvidenceScope {
        self.parts.physical_evidence_scope.clone()
    }

    pub const fn physical_evidence(&self) -> Option<&NativePhysicalEvidence> {
        self.parts.physical_evidence.as_ref()
    }

    pub const fn physical_evidence_gap(&self) -> Option<&NativePhysicalEvidenceGap> {
        self.physical_evidence_gap.as_ref()
    }

    pub fn into_parts(self) -> NativeArtifactParts {
        self.parts
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeArtifactIdentity([u8; 32]);

impl NativeArtifactIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for NativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for NativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

publication_digest!(NativeSelectedProviderClosureDigest);
publication_digest!(NativeSelectedProviderPlanDigest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePhysicalEvidenceScope {
    Unavailable,
    UnoptimizedCompleteBoundaryEvidence,
    ValidatedOptimizedProjection(ValidatedOptimizedNativePhysicalEvidenceScope),
}

impl NativePhysicalEvidenceScope {
    pub(crate) fn from_fragment_publication(
        boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
        object: &image_emission::ObjectArtifact,
    ) -> Result<Self, &'static str> {
        let coverage_identity =
            boundary_application_coverage_identity(Some(boundary_application_coverage))
                .ok_or("present boundary-application coverage has no identity")?;
        physical::derive_fragment_publication_scope(
            boundary_application_coverage,
            coverage_identity,
            object,
        )
        .map(Self::ValidatedOptimizedProjection)
    }

    const fn requires_boundary_application_coverage(&self) -> bool {
        matches!(
            self,
            Self::UnoptimizedCompleteBoundaryEvidence | Self::ValidatedOptimizedProjection(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelectedProviderPlan {
    report_identity: u64,
    plan_digest: NativeSelectedProviderPlanDigest,
    requirement_identities: Vec<String>,
    external_requirement_applications: Vec<SelectedExternalRequirementApplication>,
    family_realization_rows: Vec<effects::SelectedTargetFamilyRealization>,
    family_realization_identities: TerminalIdentityTable,
    closed_requirement_rows: Vec<(String, String)>,
    assumed_requirement_rows: Vec<(String, String)>,
    assumed_relationships: Vec<effects::AssumedBindingContractSelection>,
    assumed_closure: Option<[u8; 32]>,
}

impl NativeSelectedProviderPlan {
    pub(crate) fn project_selection(selection: &SelectedProviderPlanFacts) -> Vec<Self> {
        let mut plans = selection
            .plans()
            .iter()
            .map(|plan| {
                let report_identity = plan.report_fingerprint();
                let requirements = plan
                    .rows
                    .iter()
                    .flat_map(|row| {
                        selection.selected_row_requirement_identities(
                            report_identity,
                            plan.identities.text(row.requirement_identity),
                        )
                    })
                    .map(str::to_owned)
                    .collect();
                let mut projected = Self::new(
                    report_identity,
                    NativeSelectedProviderPlanDigest::from_digest(
                        *plan.identity_digest().as_bytes(),
                    ),
                    requirements,
                );
                projected.family_realization_rows = selection
                    .family_realization_rows()
                    .iter()
                    .filter(|row| row.provider_plan_report_identity == report_identity)
                    .cloned()
                    .collect();
                projected.family_realization_identities = selection.identities().clone();
                projected.closed_requirement_rows = selection
                    .specialized_requirement_rows()
                    .iter()
                    .filter(|row| row.provider_plan_report_identity == report_identity)
                    .map(|row| {
                        let text = |identity| {
                            selection
                                .identities()
                                .text(identity)
                                .unwrap_or_default()
                                .to_owned()
                        };
                        (
                            text(row.requirement_identity),
                            text(row.selected_requirement_identity),
                        )
                    })
                    .collect();
                projected.external_requirement_applications = selection
                    .external_requirement_applications()
                    .iter()
                    .filter(|row| row.provider_plan_report_identity == report_identity)
                    .cloned()
                    .collect();
                projected.assumed_relationships = selection.assumed_bindings().to_vec();
                if !projected.assumed_relationships.is_empty() {
                    projected.assumed_closure = Some(*selection.identity_digest().as_bytes());
                }
                for row in selection.assumed_bindings() {
                    if let Ok(supplied) = selection.assumed_supplier_identity(&row.required)
                        && projected
                            .requirement_identities
                            .iter()
                            .any(|identity| identity == supplied)
                    {
                        projected
                            .assumed_requirement_rows
                            .push((row.required.clone(), supplied.to_owned()));
                        projected.requirement_identities.push(row.required.clone());
                    }
                }
                projected.requirement_identities.sort();
                projected.requirement_identities.dedup();
                projected
            })
            .collect::<Vec<_>>();
        plans.sort_by_key(Self::report_identity);
        plans
    }

    pub fn new(
        report_identity: u64,
        plan_digest: NativeSelectedProviderPlanDigest,
        mut requirement_identities: Vec<String>,
    ) -> Self {
        requirement_identities.sort();
        requirement_identities.dedup();
        Self {
            report_identity,
            plan_digest,
            requirement_identities,
            external_requirement_applications: Vec::new(),
            family_realization_rows: Vec::new(),
            family_realization_identities: TerminalIdentityTable::from(vec![String::new()]),
            closed_requirement_rows: Vec::new(),
            assumed_requirement_rows: Vec::new(),
            assumed_relationships: Vec::new(),
            assumed_closure: None,
        }
    }

    pub(crate) fn assumption_closure(&self) -> Option<[u8; 32]> {
        self.assumed_closure
    }

    pub(crate) fn matches_assumption(
        &self,
        module: &TerminalModule,
        required: &str,
        evidence: &installation_evidence::InstalledAssumedBindingEvidence,
    ) -> bool {
        let identity = |id| {
            module
                .boundary_machine(id)
                .and_then(|row| module.identities.text(row.identity))
        };
        if identity(evidence.required) != Some(required) || self.assumed_closure.is_none() {
            return false;
        }
        let mut current = required;
        let mut path = Vec::new();
        for _ in 0..=self.assumed_relationships.len() {
            let Some(row) = self
                .assumed_relationships
                .iter()
                .find(|row| row.required == current)
            else {
                return identity(evidence.selected_requirement) == Some(current)
                    && path == evidence.supply_path;
            };
            if current == required {
                if identity(evidence.supplied_requirement) != Some(row.supplied.as_str()) {
                    return false;
                }
            } else {
                let Some(boundary) = module
                    .boundary_machines
                    .iter()
                    .find(|boundary| module.identities.text(boundary.identity) == Some(current))
                else {
                    return false;
                };
                path.push(boundary.id);
            }
            current = &row.supplied;
        }
        false
    }

    pub const fn report_identity(&self) -> u64 {
        self.report_identity
    }

    pub const fn plan_digest(&self) -> NativeSelectedProviderPlanDigest {
        self.plan_digest
    }

    pub fn requirement_identities(&self) -> &[String] {
        &self.requirement_identities
    }

    pub fn family_realization_rows(&self) -> &[effects::SelectedTargetFamilyRealization] {
        &self.family_realization_rows
    }

    pub fn external_requirement_applications(&self) -> &[SelectedExternalRequirementApplication] {
        &self.external_requirement_applications
    }

    pub(crate) fn validate_external_requirement_applications(&self) -> Result<(), &'static str> {
        let mut previous = None;
        for row in &self.external_requirement_applications {
            let requirement = self
                .family_realization_identities
                .text(row.requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("native external coordinate lost its closed requirement")?;
            let selected = self
                .family_realization_identities
                .text(row.selected_requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("native external coordinate lost its selected requirement")?;
            if previous.is_some_and(|previous| previous >= requirement)
                || requirement == selected
                || row.provider_plan_report_identity != self.report_identity
                || row.provider_plan_digest.as_bytes() != self.plan_digest.as_bytes()
                || self
                    .requirement_identities
                    .binary_search_by(|identity| identity.as_str().cmp(requirement))
                    .is_err()
                || self
                    .closed_requirement_rows
                    .iter()
                    .any(|(closed, selected)| {
                        closed == requirement
                            && selected.as_str()
                                != self
                                    .family_realization_identities
                                    .text(row.selected_requirement_identity)
                                    .unwrap_or_default()
                    })
                || self.family_realization_rows.iter().any(|family| {
                    (family.head_identity == row.requirement_identity
                        || family.member_identity == row.requirement_identity)
                        && family.selected_requirement_identity != row.selected_requirement_identity
                })
            {
                return Err(
                    "native external coordinate changed its exact selected plan or requirement roster",
                );
            }
            previous = Some(requirement);
        }
        Ok(())
    }

    pub fn family_realization_identities(&self) -> &TerminalIdentityTable {
        &self.family_realization_identities
    }

    pub fn selected_requirement_identity<'a>(&'a self, requirement: &'a str) -> &'a str {
        if let Some((_, supplied)) = self
            .assumed_requirement_rows
            .iter()
            .find(|(required, _)| required == requirement)
        {
            return supplied;
        }

        if let Some(row) = self.external_requirement_applications.iter().find(|row| {
            self.family_realization_identities
                .text(row.requirement_identity)
                == Some(requirement)
        }) {
            return self
                .family_realization_identities
                .text(row.selected_requirement_identity)
                .unwrap_or(requirement);
        }
        if let Some(row) = self.family_realization_rows.iter().find(|row| {
            self.family_realization_identities.text(row.head_identity) == Some(requirement)
                || self.family_realization_identities.text(row.member_identity) == Some(requirement)
        }) {
            return self
                .family_realization_identities
                .text(row.selected_requirement_identity)
                .unwrap_or(requirement);
        }
        self.closed_requirement_rows
            .iter()
            .find(|(closed, _)| closed == requirement)
            .map_or(requirement, |(_, selected)| selected.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderExecution {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

impl NativeProviderExecution {
    pub fn from_evidence(evidence: &dyn ProviderExecutionEvidence) -> Self {
        Self {
            requirement_identity: evidence.requirement_identity().to_owned(),
            provider_plan_report_identity: evidence.provider_plan_report_identity(),
            provider_execution_report_identity: evidence.provider_execution_report_identity(),
            provider_execution_report_fingerprint: evidence.provider_execution_report_fingerprint(),
            normalized_root_report_identity: evidence.normalized_root_report_identity(),
            boundary_contract_report_fingerprint: evidence.boundary_contract_report_fingerprint(),
        }
    }
}

impl ProviderExecutionEvidence for NativeProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn provider_execution_report_identity(&self) -> u64 {
        self.provider_execution_report_identity
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    fn normalized_root_report_identity(&self) -> u64 {
        self.normalized_root_report_identity
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

fn validate_ieee_float_fma_occurrences(
    module: &TerminalModule,
    execution_scope: Option<&terminal_psi_to_abstract_operations::NativeExecutionScope>,
) -> Result<(), &'static str> {
    if module
        .machines
        .iter()
        .filter(|machine| execution_scope.is_none_or(|scope| scope.contains_machine(machine.id)))
        .any(|machine| {
            machine.operations().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                )
            })
        })
    {
        return Err(
            "native artifact does not retain every Terminal nearest-FMA occurrence exactly once",
        );
    }
    Ok(())
}
