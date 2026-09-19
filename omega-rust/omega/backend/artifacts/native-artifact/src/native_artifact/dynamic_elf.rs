//! Non-installable dynamic-ELF artifact construction and replay.

use super::identity::{
    NativeArtifactIdentityFields, derive_native_artifact_identity, foreign_call_custody_digest,
};
use super::{
    NativePhysicalEvidence, NativePhysicalEvidenceDerivation, NativePhysicalEvidenceGap,
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, boundary_application_coverage_identity, derive_physical_evidence,
    mixed_structural_scalar, validate_boundary_application_coverage,
    validate_foreign_stack_contribution, validate_ieee_float_fma_occurrences,
    validate_provider_execution_reports,
};
use ::boundary_applications::TerminalBoundaryApplicationCoverage;
use effects::{
    TerminalAuthorityClosureReviewReceipt, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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
    target: target::NativeTarget,
    psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    object: image_emission::ObjectArtifact,
    image: image_emission::RequestedDynamicElfImage,
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
    /// caller claim: `DynamicElfNativeArtifactParts` deliberately has no slot
    /// for it.
    physical_evidence_gap: Option<NativePhysicalEvidenceGap>,
    identity: DynamicElfNativeArtifactIdentity,
}

/// Reconstructible parts of one non-installable dynamic-ELF native candidate.
#[derive(Debug)]
pub struct DynamicElfNativeArtifactParts {
    pub target: target::NativeTarget,
    pub psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    pub object: image_emission::ObjectArtifact,
    pub image: image_emission::RequestedDynamicElfImage,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    /// The exact receiving permission policy this artifact was realized under,
    /// or `None` when production ran without a receiver-admission claim.
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
    pub physical_evidence: Option<NativePhysicalEvidence>,
}

/// Fresh dynamic writer inputs.  Physical evidence is rederived from the
/// canonical Terminal/object/final-image join rather than accepted from the
/// caller.
#[derive(Debug)]
pub struct DynamicElfNativeArtifactEmissionParts {
    pub target: target::NativeTarget,
    pub psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    pub object: image_emission::ObjectArtifact,
    pub image: image_emission::RequestedDynamicElfImage,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
    pub terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    /// The exact receiving permission policy this artifact was realized under,
    /// or `None` when production ran without a receiver-admission claim.
    pub terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: TerminalAuthorityClosureReviewReceipt,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
}

impl DynamicElfNativeArtifact {
    /// Complete fresh source-free dynamic emission by deriving every physical
    /// child that is supported by the exact Terminal/object/final-image join.
    pub fn from_emitted_parts(
        parts: DynamicElfNativeArtifactEmissionParts,
    ) -> Result<Self, &'static str> {
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(parts.image.emission().admitted().image()).as_bytes();
        let physical_evidence = match derive_physical_evidence(
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
            NativePhysicalEvidenceDerivation::Complete(evidence) => Some(evidence),
            NativePhysicalEvidenceDerivation::Unavailable
            | NativePhysicalEvidenceDerivation::Blocked(_) => None,
        };
        Self::from_replayed_parts(DynamicElfNativeArtifactParts {
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
        })
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
            identity: DynamicElfNativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity();
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.psi_artifact
            .validate()
            .map_err(|_| "dynamic native artifact contains an invalid canonical artifact")?;
        let semantic = self.psi_artifact.manifest().semantic();
        if self.object.psi() != semantic {
            return Err("dynamic native artifact semantic identity disagrees with its object");
        }
        if self.object.target() != self.target
            || self.image.emission().admitted().image().target != self.target
        {
            return Err("dynamic native artifact target disagrees with its object or image");
        }
        if self.image.artifact() != &self.object {
            return Err("dynamic native artifact image retained a substituted object");
        }
        image_emission::validate_requested_dynamic_elf_image(&self.object, &self.image)
            .map_err(|_| "dynamic native artifact failed exact requested-image replay")?;
        self.terminal_authority_closure_review
            .validate()
            .map_err(|_| "dynamic native artifact terminal-authority closure receipt is invalid")?;
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
                "dynamic native artifact closure receipt drifted from its exact realization inputs",
            );
        }
        let module = terminal_codec::decode_module(self.psi_artifact.semantic_bytes())
            .map_err(|_| "dynamic native artifact canonical semantics failed to decode")?;
        mixed_structural_scalar::validate(&module, &self.object)?;
        validate_boundary_application_coverage(
            &module,
            semantic,
            self.boundary_application_coverage.as_ref(),
            &self.physical_evidence_scope,
        )?;
        validate_ieee_float_fma_occurrences(&module, &self.object, &self.selected_provider_plans)?;
        if module.entry != self.object.entry() {
            return Err("dynamic native artifact entry disagrees with canonical semantics");
        }
        if self.selected_provider_closure_report_identity == 0 {
            return Err(
                "dynamic native artifact selected provider closure has the reserved zero identity",
            );
        }

        let mut required_executions = BTreeSet::new();
        for installed in self.object.boundary_settlements() {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|boundary| boundary.id == installed.settlement.boundary)
                .ok_or("dynamic native artifact settlement names an absent boundary requirement")?;
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
        for foreign in self.object.foreign_calls() {
            let target_operations::CallSiteOwner::Operation(owner) = foreign.owner else {
                return Err(
                    "dynamic native artifact foreign provider execution has no semantic operation owner",
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
                    "dynamic native artifact foreign execution does not rejoin one semantic operation",
                );
            };
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
                return Err(
                    "dynamic native artifact foreign execution owner is not a boundary call",
                );
            };
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or("dynamic native artifact foreign execution names an absent boundary")?;
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
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(self.image.emission().admitted().image()).as_bytes();
        let (expected_physical_evidence, expected_physical_evidence_gap) =
            match derive_physical_evidence(
                &self.physical_evidence_scope,
                &self.psi_artifact,
                self.target,
                &self.object,
                self.image.output(),
                final_image_symbol_digest,
                &self.selected_provider_plans,
                &self.provider_executions,
                self.boundary_application_coverage.as_ref(),
            )? {
                NativePhysicalEvidenceDerivation::Unavailable => (None, None),
                NativePhysicalEvidenceDerivation::Complete(evidence) => (Some(evidence), None),
                NativePhysicalEvidenceDerivation::Blocked(gap) => (None, Some(gap)),
            };
        if self.physical_evidence != expected_physical_evidence {
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
        let final_image_symbol_digest =
            *image::final_image_symbol_digest(self.image.emission().admitted().image()).as_bytes();
        let base = derive_native_artifact_identity(NativeArtifactIdentityFields {
            terminal_artifact_identity: *self.psi_artifact.manifest().identity().as_bytes(),
            target: self.target,
            object_text_bytes: self.object.text_bytes(),
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
            selected_provider_closure_digest: *self.selected_provider_closure_digest.as_bytes(),
            foreign_call_custody_digest: foreign_call_custody_digest(self.object.foreign_calls()),
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
        });
        let mut digest = Sha256::new();
        digest.update(DYNAMIC_ELF_NATIVE_ARTIFACT_IDENTITY_DOMAIN);
        digest.update(base.as_bytes());
        DynamicElfNativeArtifactIdentity(digest.finalize().into())
    }

    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn psi_artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.psi_artifact
    }

    pub const fn object(&self) -> &image_emission::ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &image_emission::RequestedDynamicElfImage {
        &self.image
    }

    pub const fn identity(&self) -> DynamicElfNativeArtifactIdentity {
        self.identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.provider_executions
    }

    pub const fn boundary_application_coverage(
        &self,
    ) -> Option<&TerminalBoundaryApplicationCoverage> {
        self.boundary_application_coverage.as_ref()
    }

    pub fn physical_evidence_scope(&self) -> NativePhysicalEvidenceScope {
        self.physical_evidence_scope.clone()
    }

    pub const fn physical_evidence(&self) -> Option<&NativePhysicalEvidence> {
        self.physical_evidence.as_ref()
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
        DynamicElfNativeArtifactParts {
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
