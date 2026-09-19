//! Construct and replay the exact Terminal/object/image join for a native artifact.
//!
//! `NativeArtifact` owns direct-image custody. The distinct non-installable
//! dynamic-ELF product is implemented in `dynamic_elf`; neither product confers
//! receiving authority merely by passing structural validation.

use std::collections::BTreeSet;

use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use abstract_operations::AbstractOperationPlan;
use effects::{
    TerminalAuthorityClosureReviewReceipt, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity,
};
use installation_evidence::ProviderExecutionEvidence;
use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
use terminal_psi::TerminalPsiIdentity;

mod boundary_applications;
mod dynamic_elf;
mod identity;
mod mixed_structural_scalar;
#[cfg(test)]
mod tests;

use crate::physical::{
    self, NativePhysicalEvidence, NativePhysicalEvidenceDerivation, NativePhysicalEvidenceGap,
    ValidatedOptimizedNativePhysicalEvidenceScope, derive_physical_evidence,
    derive_validated_optimization_scope,
};
pub(super) use boundary_applications::boundary_application_coverage_identity;
use boundary_applications::validate_boundary_application_coverage;
pub use dynamic_elf::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts,
    DynamicElfNativeArtifactIdentity, DynamicElfNativeArtifactParts,
};
use identity::{
    NativeArtifactIdentityFields, derive_native_artifact_identity, foreign_call_custody_digest,
};

/// Complete source-independent Terminal-Psi native realization.
#[derive(Debug)]
#[must_use = "a native artifact owns the canonical semantic and target artifact join"]
pub struct NativeArtifact {
    target: target::NativeTarget,
    psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    object: image_emission::ObjectArtifact,
    image: image_emission::ExecutableImage,
    selected_provider_closure_report_identity: u64,
    selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    provider_executions: Vec<NativeProviderExecution>,
    terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    physical_evidence_scope: NativePhysicalEvidenceScope,
    physical_evidence: Option<NativePhysicalEvidence>,
    /// The exact subject that stopped the scoped derivation when
    /// `physical_evidence` is absent. Derived here, never retained as a
    /// caller claim: `NativeArtifactParts` deliberately has no slot for it.
    physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    identity: NativeArtifactIdentity,
}

#[derive(Debug)]
pub struct NativeArtifactParts {
    pub target: target::NativeTarget,
    pub psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    pub object: image_emission::ObjectArtifact,
    pub image: image_emission::ExecutableImage,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    /// The exact receiving permission policy this artifact was realized under,
    /// or `None` when production ran without a receiver-admission claim. `None`
    /// never means an empty policy and cannot satisfy explicit admission
    /// replay.
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
    pub physical_evidence: Option<NativePhysicalEvidence>,
}

/// Fresh machine/image emission inputs. Physical evidence is deliberately
/// absent: this owner derives it from the exact Terminal, object, image, and
/// selected-plan custody before constructing a replayable artifact.
#[derive(Debug)]
pub struct NativeArtifactEmissionParts {
    pub target: target::NativeTarget,
    pub psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    pub object: image_emission::ObjectArtifact,
    pub image: image_emission::ExecutableImage,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    /// The exact receiving permission policy this emission was realized under,
    /// or `None` when production ran without a receiver-admission claim.
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
}

impl NativeArtifact {
    /// Complete fresh native emission by deriving the identity optimization
    /// projection and every currently supported physical child.
    pub fn from_emitted_parts(parts: NativeArtifactEmissionParts) -> Result<Self, &'static str> {
        let physical_evidence = match derive_physical_evidence(
            &parts.physical_evidence_scope,
            &parts.psi_artifact,
            parts.target,
            &parts.object,
            parts.image.output(),
            *parts.image.final_image_symbol_digest().as_bytes(),
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )? {
            NativePhysicalEvidenceDerivation::Complete(evidence) => Some(evidence),
            NativePhysicalEvidenceDerivation::Unavailable
            | NativePhysicalEvidenceDerivation::Blocked(_) => None,
        };
        // The fragment route seals the object's foreign-call roster inside its
        // replay surface; the emitted image receives that custody here, bound
        // from the scope the publication derivation produced for this exact
        // object before the artifact records it as emitted evidence.
        let custody = parts
            .physical_evidence_scope
            .normalized_foreign_call_custody()
            .to_vec();
        let mut image = parts.image;
        if !custody.is_empty() {
            image
                .bind_normalized_foreign_call_custody(custody)
                .map_err(|_| "native artifact image rejects fragment foreign call custody")?;
        }
        Self::from_replayed_parts(NativeArtifactParts {
            target: parts.target,
            psi_artifact: parts.psi_artifact,
            object: parts.object,
            image,
            selected_provider_closure_report_identity: parts
                .selected_provider_closure_report_identity,
            selected_provider_closure_digest: parts.selected_provider_closure_digest,
            selected_provider_plans: parts.selected_provider_plans,
            provider_executions: parts.provider_executions,
            terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: parts
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review: parts.terminal_authority_closure_review,
            boundary_application_coverage: parts.boundary_application_coverage,
            physical_evidence_scope: parts.physical_evidence_scope,
            physical_evidence,
        })
    }

    /// Rejoin already verified proof admission with target artifacts while
    /// replaying every source-free identity and byte relation retained here.
    pub fn from_replayed_parts(parts: NativeArtifactParts) -> Result<Self, &'static str> {
        // The gap is derivation state, not a retained claim: it is recomputed
        // from the replayed inputs so a retained artifact can never assert a
        // blocking subject the derivation would not have produced.
        let physical_evidence_gap = match derive_physical_evidence(
            &parts.physical_evidence_scope,
            &parts.psi_artifact,
            parts.target,
            &parts.object,
            parts.image.output(),
            *parts.image.final_image_symbol_digest().as_bytes(),
            &parts.selected_provider_plans,
            &parts.provider_executions,
            parts.boundary_application_coverage.as_ref(),
        )? {
            NativePhysicalEvidenceDerivation::Blocked(gap) => Some(gap),
            NativePhysicalEvidenceDerivation::Unavailable
            | NativePhysicalEvidenceDerivation::Complete(_) => None,
        };
        let mut artifact = Self {
            target: parts.target,
            psi_artifact: parts.psi_artifact,
            object: parts.object,
            image: parts.image,
            selected_provider_closure_report_identity: parts
                .selected_provider_closure_report_identity,
            selected_provider_closure_digest: parts.selected_provider_closure_digest,
            selected_provider_plans: parts.selected_provider_plans,
            provider_executions: parts.provider_executions,
            terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: parts
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review: parts.terminal_authority_closure_review,
            boundary_application_coverage: parts.boundary_application_coverage,
            physical_evidence_scope: parts.physical_evidence_scope,
            physical_evidence: parts.physical_evidence,
            physical_evidence_gap,
            identity: NativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity();
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.psi_artifact
            .validate()
            .map_err(|_| "native artifact contains an invalid canonical artifact")?;
        let semantic = self.psi_artifact.manifest().semantic();
        if self.object.psi() != semantic || self.image.psi() != semantic {
            return Err("native artifact semantic identity disagrees with its object or image");
        }
        if self.object.target() != self.target || self.image.target() != self.target {
            return Err("native artifact target disagrees with its object or image");
        }
        self.terminal_authority_closure_review
            .validate()
            .map_err(|_| "native artifact terminal-authority closure receipt is invalid")?;
        if self
            .terminal_authority_closure_review
            .terminal_artifact_identity()
            != *self.psi_artifact.manifest().identity().as_bytes()
            || self.terminal_authority_closure_review.target() != self.target
            || self
                .terminal_authority_closure_review
                .selected_provider_closure()
                .as_bytes()
                != self.selected_provider_closure_digest.as_bytes()
            || self.terminal_authority_closure_review.physical_policy()
                != self.terminal_authority_policy_identity
            || self.terminal_authority_closure_review.permission_policy()
                != self.terminal_authority_permission_policy_identity
        {
            return Err(
                "native artifact terminal-authority closure receipt drifted from its exact realization inputs",
            );
        }
        image_emission::validate_executable_image(&self.object, &self.image)
            .map_err(|_| "native artifact image failed object-to-image replay")?;
        let module = terminal_codec::decode_module(self.psi_artifact.semantic_bytes())
            .map_err(|_| "native artifact canonical semantics failed to decode")?;
        mixed_structural_scalar::validate(&module, &self.object)?;
        validate_boundary_application_coverage(
            &module,
            self.psi_artifact.manifest().semantic(),
            self.boundary_application_coverage.as_ref(),
            &self.physical_evidence_scope,
        )?;
        validate_ieee_float_fma_occurrences(&module, &self.object, &self.selected_provider_plans)?;
        if module.entry != self.object.entry() {
            return Err("native artifact entry disagrees with canonical semantics");
        }
        if self.selected_provider_closure_report_identity == 0 {
            return Err("native artifact selected provider closure has the reserved zero identity");
        }

        let mut required_executions = BTreeSet::new();
        for installed in self.image.boundary_settlements() {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|boundary| boundary.id == installed.settlement.boundary)
                .ok_or("native artifact image settlement names an absent boundary requirement")?;
            let image_emission::BoundaryExecutionRecord::AdmittedProvider(execution) =
                installed.settlement.execution
            else {
                continue;
            };
            required_executions.insert((
                boundary.identity.clone(),
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            ));
        }
        for foreign in self.image.foreign_calls() {
            let target_operations::CallSiteOwner::Operation(owner) = foreign.owner else {
                return Err(
                    "native artifact foreign provider execution has no semantic operation owner",
                );
            };
            let matching_operations = module
                .machines
                .iter()
                .filter(|machine| machine.id == foreign.machine)
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == owner)
                .collect::<Vec<_>>();
            let [operation] = matching_operations.as_slice() else {
                return Err(
                    "native artifact foreign provider execution does not rejoin one semantic operation",
                );
            };
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
                return Err(
                    "native artifact foreign provider execution owner is not a boundary call",
                );
            };
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or("native artifact foreign provider execution names an absent boundary")?;
            let execution = foreign.provider_execution;
            let contribution = &foreign.same_stack_contribution;
            validate_foreign_stack_contribution(
                &boundary.identity,
                execution,
                contribution.requirement_identity(),
                contribution.provider_plan_report_identity(),
                contribution.provider_plan_commitment().as_bytes(),
                &self.selected_provider_plans,
            )?;
            required_executions.insert((
                boundary.identity.clone(),
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            ));
        }
        validate_provider_execution_reports(
            &self.selected_provider_plans,
            &self.provider_executions,
            &required_executions,
        )?;
        let (expected_physical_evidence, expected_physical_evidence_gap) =
            match derive_physical_evidence(
                &self.physical_evidence_scope,
                &self.psi_artifact,
                self.target,
                &self.object,
                self.image.output(),
                *self.image.final_image_symbol_digest().as_bytes(),
                &self.selected_provider_plans,
                &self.provider_executions,
                self.boundary_application_coverage.as_ref(),
            )? {
                NativePhysicalEvidenceDerivation::Unavailable => (None, None),
                NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
                NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
            };
        if self.physical_evidence != expected_physical_evidence {
            return Err(
                "native artifact physical children disagree with its validated identity projection",
            );
        }
        if self.physical_evidence_gap != expected_physical_evidence_gap {
            return Err(
                "native artifact physical evidence gap disagrees with exact derivation replay",
            );
        }
        if self.identity != self.recomputed_identity() {
            return Err("native artifact identity disagrees with its retained authority");
        }
        Ok(())
    }

    fn recomputed_identity(&self) -> NativeArtifactIdentity {
        let output = self.image.output();
        let compiler_text_validation_digest = output
            .compiler_text_validation
            .map(|evidence| *evidence.derivation_digest.as_bytes());
        let compiler_function_validation = output.compiler_function_validation.map(|evidence| {
            (
                *evidence.evidence_digest().as_bytes(),
                evidence.evidence_report_fingerprint(),
            )
        });
        let compiler_entry_region_binding =
            output
                .compiler_entry_region_binding
                .as_ref()
                .map(|evidence| {
                    (
                        *evidence.evidence_digest.as_bytes(),
                        evidence.evidence_report_fingerprint,
                    )
                });
        let compiler_entry_footprint_binding =
            output.compiler_entry_footprint_binding.map(|evidence| {
                (
                    *evidence.evidence_digest.as_bytes(),
                    evidence.evidence_report_fingerprint,
                )
            });
        derive_native_artifact_identity(NativeArtifactIdentityFields {
            terminal_artifact_identity: *self.psi_artifact.manifest().identity().as_bytes(),
            target: self.target,
            object_text_bytes: self.object.text_bytes(),
            image_bytes: &output.bytes,
            final_text_bytes: &output.final_text_bytes,
            image_subsystem: self.image.subsystem(),
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
            final_image_symbol_digest: *self.image.final_image_symbol_digest().as_bytes(),
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
            foreign_call_custody_digest: foreign_call_custody_digest(self.image.foreign_calls()),
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
            physical_evidence_gap_identity: self
                .physical_evidence_gap
                .as_ref()
                .map(|gap| *gap.identity()),
        })
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.psi_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.psi_artifact.proof_bytes()
    }

    pub const fn psi_artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.psi_artifact
    }

    pub const fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &image_emission::ExecutableImage {
        &self.image
    }

    /// Non-authoritative compatibility/report identity of the selected
    /// provider closure.
    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.selected_provider_closure_report_identity
    }

    pub const fn selected_provider_closure_digest(&self) -> NativeSelectedProviderClosureDigest {
        self.selected_provider_closure_digest
    }

    pub const fn identity(&self) -> NativeArtifactIdentity {
        self.identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.provider_executions
    }

    /// Exact receiving target-policy identity accepted while realizing this
    /// artifact. This records classification policy only; it is not service
    /// containment or provider-execution admission evidence.
    pub const fn terminal_authority_policy_identity(&self) -> TerminalAuthorityPolicyIdentity {
        self.terminal_authority_policy_identity
    }

    /// The receiving permission-policy identity this artifact was realized
    /// under, or `None` when production made no receiver-admission claim.
    pub const fn terminal_authority_permission_policy_identity(
        &self,
    ) -> Option<TerminalAuthorityPermissionPolicyIdentity> {
        self.terminal_authority_permission_policy_identity
    }

    pub const fn terminal_authority_closure_review(
        &self,
    ) -> &TerminalAuthorityClosureReviewReceipt {
        &self.terminal_authority_closure_review
    }

    /// Exact D29 demand and realization custody. `None` means this artifact
    /// was realized without a checked source-to-Terminal join; an exact empty
    /// demand set is retained as `Some` with zero references.
    pub const fn boundary_application_coverage(
        &self,
    ) -> Option<&TerminalBoundaryApplicationCoverage> {
        self.boundary_application_coverage.as_ref()
    }

    /// Replay this artifact under a receiving authority's exact accepted
    /// target policy. Base validation cannot decide which policy a receiver
    /// accepts; this join makes policy substitution explicit.
    pub fn validate_for_terminal_authority_policy(
        &self,
        accepted: TerminalAuthorityPolicyIdentity,
    ) -> Result<(), &'static str> {
        self.validate()?;
        if self.terminal_authority_policy_identity != accepted {
            return Err("native artifact terminal-authority policy is not accepted");
        }
        Ok(())
    }

    /// Replay the complete D45 receiving-policy join. Both policies and the
    /// result of the receiver's actual closure review are independently
    /// accepted inputs; structural validation of freely constructible receipt
    /// data cannot confer receiving authority by itself.
    ///
    /// An artifact realized without a receiving permission policy carries no
    /// receiver-admission claim: this explicit admission replay rejects it
    /// rather than treating absence as an empty or implied policy.
    pub fn validate_for_terminal_authority_policies(
        &self,
        accepted_physical: TerminalAuthorityPolicyIdentity,
        accepted_permission: TerminalAuthorityPermissionPolicyIdentity,
        accepted_closure_review: [u8; 32],
    ) -> Result<(), &'static str> {
        self.validate_for_terminal_authority_policy(accepted_physical)?;
        if self.terminal_authority_permission_policy_identity != Some(accepted_permission) {
            return Err("native artifact terminal-authority permission policy is not accepted");
        }
        if self.terminal_authority_closure_review.identity() != accepted_closure_review {
            return Err("native artifact terminal-authority closure review is not accepted");
        }
        Ok(())
    }

    pub fn physical_evidence_scope(&self) -> NativePhysicalEvidenceScope {
        self.physical_evidence_scope.clone()
    }

    /// Complete D32 evidence for the currently supported physical lane.
    /// `None` means the artifact contains a role this implementation does not
    /// yet cover; it grants no final-realization claim for that role.
    pub const fn physical_evidence(&self) -> Option<&NativePhysicalEvidence> {
        self.physical_evidence.as_ref()
    }

    /// The exact subject that stopped this artifact's scoped D32 derivation
    /// when [`Self::physical_evidence`] is absent. `None` means the evidence
    /// is complete or the scope admits no derivation at all; a `Some` value
    /// names the first occurrence or retained record the derivation could
    /// not bind to a physical child.
    pub const fn physical_evidence_gap(&self) -> Option<&NativePhysicalEvidenceGap> {
        self.physical_evidence_gap.as_ref()
    }

    pub fn into_parts(self) -> NativeArtifactParts {
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
            physical_evidence: self.physical_evidence,
        }
    }
}

/// Collision-resistant identity of one complete, validated native artifact.
///
/// The identity binds the canonical Terminal artifact, selected native target,
/// pre-relocation object text, exact executable image, strong final-image
/// evidence, and the complete source-selected provider realization retained by
/// [`NativeArtifact`]. It is derived by this owner and cannot be supplied by a
/// caller reconstructing an artifact from parts.
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

/// Collision-resistant commitment to the complete source-selected provider
/// closure carried by this source-free artifact projection.
///
/// The digest is derived by the selected-closure owner and independently
/// replayed when the standalone component candidate rejoins source policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSelectedProviderClosureDigest([u8; 32]);

impl NativeSelectedProviderClosureDigest {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Strong source-free projection of one exact normalized provider-plan
/// digest. Unlike the compact report coordinate, these bytes bind every plan
/// field and can be carried into D41 physical-parent custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSelectedProviderPlanDigest([u8; 32]);

impl NativeSelectedProviderPlanDigest {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact upstream scope under which this artifact may derive D32 evidence.
///
/// Evidence requires complete checked D29 coverage custody, either for an
/// unoptimized Terminal handoff or an independently validated optimized
/// projection. Unsupported boundary/provider mechanisms still produce no
/// evidence during native replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePhysicalEvidenceScope {
    Unavailable,
    UnoptimizedCompleteBoundaryEvidence,
    ValidatedOptimizedProjection(ValidatedOptimizedNativePhysicalEvidenceScope),
}

impl NativePhysicalEvidenceScope {
    /// Foreign-call custody the fragment publication projected for the
    /// emitted image; empty for scopes that did not admit one.
    fn normalized_foreign_call_custody(&self) -> &[image_emission::ObjectForeignCall] {
        match self {
            Self::Unavailable | Self::UnoptimizedCompleteBoundaryEvidence => &[],
            Self::ValidatedOptimizedProjection(scope) => scope.foreign_call_custody(),
        }
    }

    /// Admit the shared fragment/object route after replaying its exact current
    /// program and object evidence. Empty and nonempty selections use the same
    /// join; no selected-lowering completion is invented for another phase.
    #[allow(clippy::too_many_arguments)]
    pub fn from_validated_fragment_publication(
        final_plan: &AbstractOperationPlan,
        terminal: TerminalPsiIdentity,
        validation: OptimizedAbstractPlanProjectionIdentity,
        final_unit: OptimizationUnitIdentity,
        boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
        source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
        object: &image_emission::ObjectArtifact,
    ) -> Result<Self, &'static str> {
        let coverage_identity =
            boundary_application_coverage_identity(Some(boundary_application_coverage))
                .expect("present boundary-application coverage has an identity");
        physical::derive_fragment_publication_scope(
            final_plan,
            terminal,
            validation,
            final_unit,
            boundary_application_coverage,
            coverage_identity,
            source,
            object,
        )
        .map(Self::ValidatedOptimizedProjection)
    }

    /// Admit the exact surviving D29 operator and D41 boundary-call roster of
    /// one independently validated optimized abstract projection.
    pub fn from_validated_optimization(
        final_plan: &AbstractOperationPlan,
        terminal: TerminalPsiIdentity,
        validation: OptimizedAbstractPlanProjectionIdentity,
        final_unit: OptimizationUnitIdentity,
        boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
    ) -> Result<Self, &'static str> {
        let coverage_identity =
            boundary_application_coverage_identity(Some(boundary_application_coverage))
                .expect("present boundary-application coverage has an identity");
        Ok(Self::ValidatedOptimizedProjection(
            derive_validated_optimization_scope(
                final_plan,
                terminal,
                validation,
                final_unit,
                boundary_application_coverage,
                coverage_identity,
            )?,
        ))
    }

    const fn requires_boundary_application_coverage(&self) -> bool {
        matches!(
            self,
            Self::UnoptimizedCompleteBoundaryEvidence | Self::ValidatedOptimizedProjection(_)
        )
    }
}

/// One selected provider plan projected into source-free native-artifact
/// reporting. Requirements are exact, canonical, strictly ordered, and
/// complete for this selected plan; the compact plan coordinate is not
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelectedProviderPlan {
    report_identity: u64,
    plan_digest: NativeSelectedProviderPlanDigest,
    requirement_identities: Vec<String>,
}

impl NativeSelectedProviderPlan {
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
        }
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
}

/// One source-free report projection of an exact admitted provider execution
/// selected during native realization.
///
/// The exact requirement string and selected-plan requirement catalog are
/// replayed here. Compact execution/root/contract coordinates remain reports;
/// provider authority stays with the non-constructible evidence borrowed by
/// lowering and the selected-closure digest retained by the artifact.
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

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn provider_execution_report_identity(&self) -> u64 {
        self.provider_execution_report_identity
    }

    pub const fn provider_execution_report_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    pub const fn normalized_root_report_identity(&self) -> u64 {
        self.normalized_root_report_identity
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
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

type ProviderExecutionReportKey = (String, u64, u64, u64, u64, u64);

fn validate_provider_execution_reports(
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    required_executions: &BTreeSet<ProviderExecutionReportKey>,
) -> Result<(), &'static str> {
    let mut prior_plan = None;
    for plan in selected_provider_plans {
        if plan.report_identity == 0
            || prior_plan.is_some_and(|prior| prior >= plan.report_identity)
        {
            return Err("native artifact selected provider plans are not canonical and unique");
        }
        prior_plan = Some(plan.report_identity);
        if plan.requirement_identities.is_empty()
            || plan
                .requirement_identities
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "native artifact selected provider requirements are not canonical and unique",
            );
        }
    }

    let mut prior_execution = None;
    let mut seen_requirements = BTreeSet::new();
    let mut reported_executions = BTreeSet::new();
    for execution in provider_executions {
        let key = (
            execution.requirement_identity(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
        );
        if prior_execution.is_some_and(|prior| prior >= key) {
            return Err("native artifact provider executions are not in canonical order");
        }
        prior_execution = Some(key);
        if !seen_requirements.insert(execution.requirement_identity()) {
            return Err("native artifact contains duplicate provider requirement executions");
        }
        let Some(plan) = selected_provider_plans
            .iter()
            .find(|plan| plan.report_identity == execution.provider_plan_report_identity())
        else {
            return Err("native artifact provider execution names an unselected plan");
        };
        if plan
            .requirement_identities
            .binary_search_by(|identity| identity.as_str().cmp(execution.requirement_identity()))
            .is_err()
        {
            return Err("native artifact provider execution is absent from its selected plan");
        }
        reported_executions.insert((
            execution.requirement_identity().to_owned(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        ));
    }
    if reported_executions != *required_executions {
        return Err("native artifact provider execution reports disagree with its image");
    }
    Ok(())
}

fn validate_foreign_stack_contribution(
    requirement_identity: &str,
    execution: machine_code::ProviderExecutionRecord,
    contribution_requirement_identity: &str,
    contribution_provider_plan_report_identity: u64,
    contribution_provider_plan_commitment: [u8; 32],
    selected_provider_plans: &[NativeSelectedProviderPlan],
) -> Result<(), &'static str> {
    if contribution_requirement_identity != requirement_identity
        || contribution_provider_plan_report_identity != execution.provider_plan_report_identity
    {
        return Err(
            "native artifact foreign stack contribution disagrees with its semantic requirement or provider execution",
        );
    }
    let Some(selected_plan) = selected_provider_plans
        .iter()
        .find(|plan| plan.report_identity() == contribution_provider_plan_report_identity)
    else {
        return Err("native artifact foreign stack contribution names an unselected provider plan");
    };
    if contribution_provider_plan_commitment != *selected_plan.plan_digest().as_bytes() {
        return Err(
            "native artifact foreign stack contribution disagrees with the exact selected provider plan",
        );
    }
    Ok(())
}

fn validate_ieee_float_fma_occurrences(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    selected_provider_plans: &[NativeSelectedProviderPlan],
) -> Result<(), &'static str> {
    let terminal_occurrences = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                    )
                })
                .map(move |operation| (machine, operation))
        })
        .collect::<Vec<_>>();
    let object_occurrences = object
        .functions()
        .iter()
        .flat_map(|function| {
            function
                .x86_scalar_fma_occurrences
                .iter()
                .map(move |occurrence| (function, occurrence))
        })
        .collect::<Vec<_>>();
    if terminal_occurrences.len() != object_occurrences.len() {
        return Err(
            "native artifact does not retain every Terminal nearest-FMA occurrence exactly once",
        );
    }
    let mut operations = BTreeSet::new();
    for (function, occurrence) in object_occurrences {
        if !operations.insert(occurrence.terminal_operation) {
            return Err("native artifact repeats one nearest-FMA occurrence");
        }
        let matching = terminal_occurrences
            .iter()
            .filter(|(machine, operation)| {
                machine.id == function.machine && operation.id == occurrence.terminal_operation
            })
            .collect::<Vec<_>>();
        let [(machine, operation)] = matching.as_slice() else {
            return Err("native artifact nearest-FMA custody names no unique Terminal operation");
        };
        let terminal_psi::OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } = operation.kind
        else {
            unreachable!("Terminal occurrence roster contains only nearest FMA")
        };
        let result = operation
            .result
            .scalar()
            .ok_or("Terminal nearest-FMA has no scalar result")?;
        let expected_format = match occurrence.format {
            machine_code::X86ScalarFmaFormat::Binary32 => {
                semantic_vocabulary::IeeeFloatFormat::Binary32
            }
            machine_code::X86ScalarFmaFormat::Binary64 => {
                semantic_vocabulary::IeeeFloatFormat::Binary64
            }
        };
        if result.id != occurrence.result
            || result.scalar_type != semantic_vocabulary::ScalarType::IeeeFloat(expected_format)
            || [left, right, addend]
                != [
                    occurrence.left.source_value,
                    occurrence.right.source_value,
                    occurrence.addend.source_value,
                ]
        {
            return Err("native artifact nearest-FMA changed its Terminal value graph");
        }
        for operand in [occurrence.left, occurrence.right, occurrence.addend] {
            let matching_constants = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|candidate| {
                    candidate.id == operand.defining_operation
                        && candidate.result.scalar().is_some_and(|result| {
                            result.id == operand.source_value
                                && result.scalar_type
                                    == semantic_vocabulary::ScalarType::IeeeFloat(expected_format)
                        })
                        && matches!(
                            candidate.kind,
                            terminal_psi::OperationKind::IeeeFloatConstant { value }
                                if value == operand.value
                        )
                })
                .count();
            if matching_constants != 1 {
                return Err("native artifact nearest-FMA changed one exact Terminal constant");
            }
        }
        let matching_plans = selected_provider_plans
            .iter()
            .filter(|plan| {
                plan.report_identity() == occurrence.provider_plan_report_identity
                    && plan.plan_digest().as_bytes() == &occurrence.provider_plan_digest
            })
            .collect::<Vec<_>>();
        let [selected_plan] = matching_plans.as_slice() else {
            return Err(
                "native artifact nearest-FMA does not rejoin one exact selected provider plan",
            );
        };
        let expected_requirement = occurrence.slot.selected_plan_requirement_identity();
        if selected_plan.requirement_identities() != [expected_requirement] {
            return Err("native artifact nearest-FMA selected plan changed its exact requirement");
        }
    }
    Ok(())
}
