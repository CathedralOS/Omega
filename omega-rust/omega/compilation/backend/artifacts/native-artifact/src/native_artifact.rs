//! Construct and replay the exact Terminal/object/image join for a native artifact.
//!
//! `NativeArtifact` owns direct-image custody. The distinct non-installable
//! dynamic-ELF product is implemented in `dynamic_elf`; neither product confers
//! receiving authority merely by passing structural validation.
//! Both replay provider requirements through `provider_execution`, supplying
//! the exact image or object rows admitted by their respective product.

use installation_evidence::ObjectEvidence;
use std::collections::BTreeSet;

use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use abstract_operations::AbstractOperationPlan;
use effects::{
    SelectedProviderPlanFacts, TerminalAuthorityClosureReviewReceipt,
    TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity,
};
use installation_evidence::ProviderExecutionEvidence;
use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
use terminal_psi::TerminalPsiIdentity;

mod boundary_applications;
mod dynamic_elf;
mod emission;
mod identity;
mod mixed_structural_scalar;
mod provider_execution;
#[cfg(test)]
mod tests;

use crate::physical::{
    self, NativePhysicalEvidence, NativePhysicalEvidenceDerivation, NativePhysicalEvidenceGap,
    ValidatedOptimizedNativePhysicalEvidenceScope, derive_physical_evidence,
};
use crate::publication::publication_digest;
pub(super) use boundary_applications::boundary_application_coverage_identity;
use boundary_applications::validate_boundary_application_coverage;
pub use dynamic_elf::DynamicElfNativeArtifact;
pub use emission::RequestedNativeArtifact;
use identity::{
    NativeArtifactIdentityFields, compiler_identity_evidence, derive_native_artifact_identity,
    foreign_call_custody_digest,
};

/// Complete source-independent Terminal-Psi native realization. Independent
/// target images may retain the same immutable Terminal artifact without copying
/// its semantic arenas or encoded sections.
#[derive(Debug)]
#[must_use = "a native artifact owns the canonical semantic and target artifact join"]
pub struct NativeArtifact {
    target: target::NativeTarget,
    psi_artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
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
    pub psi_artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
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
/// The image carrier is either a completed image or an explicit emission
/// request consumed by [`RequestedNativeArtifact::emit`]. It does not change
/// the evidence required by either image product.
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
    /// projection and every currently supported physical child. The artifact
    /// is sealed over the parts this compile just produced; it is not
    /// replayed (verification placement in `omega-rust/pipeline.md`).
    /// [`Self::from_replayed_parts`] and [`Self::validate`] replay it for
    /// retained-artifact readers.
    pub fn from_emitted_parts(parts: NativeArtifactEmissionParts) -> Result<Self, &'static str> {
        let (physical_evidence, physical_evidence_gap) = match derive_physical_evidence(
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
            NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
            NativePhysicalEvidenceDerivation::Unavailable => (None, None),
            NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
        };
        // Fragment calls already belong to the object and its emitted image.
        // Check the independent projection instead of appending calls after
        // image emission, which would omit their admitted stack demand.
        let custody = parts
            .physical_evidence_scope
            .normalized_foreign_call_custody();
        if !custody.is_empty() && parts.image.foreign_calls() != custody {
            return Err("native artifact image differs from fragment foreign call custody");
        }
        Ok(Self::seal(
            NativeArtifactParts {
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
                physical_evidence,
            },
            physical_evidence_gap,
        ))
    }

    /// Replay every source-free identity and byte relation retained here.
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
        let artifact = Self::seal(parts, physical_evidence_gap);
        artifact.validate()?;
        Ok(artifact)
    }

    /// Assemble the artifact and its identity over `parts` and the derived
    /// evidence gap.
    fn seal(
        parts: NativeArtifactParts,
        physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    ) -> Self {
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
        artifact
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let semantic = self.psi_artifact.manifest().semantic();
        if self.object.psi() != semantic || self.image.psi() != semantic {
            return Err("native artifact semantic identity disagrees with its object or image");
        }
        if self.object.target() != self.target || self.image.target() != self.target {
            return Err("native artifact target disagrees with its object or image");
        }
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
        image_emission::validate_direct_executable_image(&self.object, &self.image)
            .map_err(|_| "native artifact image failed object-to-image replay")?;
        let module = self.psi_artifact.semantic_module();
        mixed_structural_scalar::validate(module, &self.object)?;
        validate_boundary_application_coverage(
            module,
            self.psi_artifact.manifest().semantic(),
            self.boundary_application_coverage.as_ref(),
            &self.physical_evidence_scope,
        )?;
        validate_ieee_float_fma_occurrences(module, &self.object, &self.selected_provider_plans)?;
        self.object.validate_terminal_entry(&self.psi_artifact)?;
        if self.selected_provider_closure_report_identity == 0 {
            return Err("native artifact selected provider closure has the reserved zero identity");
        }

        provider_execution::replay_provider_executions(
            module,
            self.image.boundary_settlements(),
            self.image.foreign_calls(),
            &self.selected_provider_plans,
            &self.provider_executions,
        )
        .map_err(provider_execution::ProviderExecutionReplayError::direct_message)?;
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
        let (
            compiler_text_validation_digest,
            compiler_function_validation,
            compiler_entry_region_binding,
            compiler_entry_footprint_binding,
        ) = compiler_identity_evidence(output);
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

    pub fn psi_artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
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

// Collision-resistant commitment to the complete source-selected provider
// closure carried by this source-free artifact projection.
//
// The digest is derived by the selected-closure owner and independently
// replayed when the standalone component candidate rejoins source policy.
publication_digest!(NativeSelectedProviderClosureDigest);

// Strong source-free projection of one exact normalized provider-plan
// digest. Unlike the compact report coordinate, these bytes bind every plan
// field and can be carried into D41 physical-parent custody.
publication_digest!(NativeSelectedProviderPlanDigest);

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
    pub(crate) fn from_validated_fragment_publication(
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
    external_requirement_applications: Vec<effects::SelectedExternalRequirementApplication>,
    family_realization_rows: Vec<effects::SelectedTargetFamilyRealization>,
    family_realization_identities: terminal_psi::TerminalIdentityTable,
    /// Closed requirement identity paired with the template-keyed row that
    /// serves it, projected from the closure's specialization roster.
    closed_requirement_rows: Vec<(String, String)>,
    assumed_requirement_rows: Vec<(String, String)>,
    assumed_relationships: Vec<effects::AssumedBindingContractSelection>,
    assumed_closure: Option<[u8; 32]>,
}

impl NativeSelectedProviderPlan {
    /// Retain each selected plan's exact Terminal-facing requirement roster.
    /// Specialized rows supply closed identities instead of the authored
    /// template identity, matching provider admission without selecting or
    /// reconstructing providers from source. Artifact replay remains independent.
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
                    if let Ok(supplied) = selection.assumed_supplier_identity(&row.required) {
                        if projected
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
            family_realization_identities: terminal_psi::TerminalIdentityTable::from(vec![
                String::new(),
            ]),
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
        module: &terminal_psi::TerminalModule,
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

    pub fn external_requirement_applications(
        &self,
    ) -> &[effects::SelectedExternalRequirementApplication] {
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

    /// Identity table the stored family-realization row indices resolve
    /// against, cloned from the selected facts that projected them.
    pub fn family_realization_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.family_realization_identities
    }

    /// Recover the authority requirement without rewriting the original
    /// callable occurrence. These rows were projected from the exact selected
    /// closure and remain committed by both closure and artifact identities.
    /// A family alias resolves through its realization row; a closed
    /// application of a requirement template resolves to the template row
    /// that serves it; any other requirement is its own authority.
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
                .operations()
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
                .operations()
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
