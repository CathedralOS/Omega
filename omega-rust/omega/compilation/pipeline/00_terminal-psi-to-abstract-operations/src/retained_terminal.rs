//! The retained Terminal product and the target-constrained proposal that has to
//! match it occurrence for occurrence - every boundary demand, callback and
//! nearest-FMA site - before native realization is allowed to begin.

mod native_providers;
mod proposal;

pub use proposal::{TerminalNativeProposalRequest, project_terminal_native_realization_proposal};

pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};
pub use native_providers::{PreparedRetainedNativeProviders, RetainedNativeProviderInputs};

pub use crate::provider_admission::TerminalCompilerBuiltinProposal;

pub use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};

/// Selected provider inputs retained together, without execution or permission
/// authority. Package permissions remain a separate independently joined input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeProviders {
    pub selection: effects::SelectedProviderPlanFacts,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
}

/// Exact occurrence catalogs produced while checked source custody exists.
/// Native replay independently rejoins each catalog to canonical operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeOccurrences {
    pub callbacks: Vec<TerminalCallbackOccurrenceProposal>,
    pub ieee_float_comparisons: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
    pub integer_comparisons: Vec<TerminalIntegerComparisonOccurrenceProposal>,
}

/// Keep the producer's complete coverage and its sealed checked scope intact.
/// Splitting coverage into demands, realizations and opaque applications would
/// discard its already-constructed identity references only to rebuild them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeBoundaryApplications {
    pub scope: lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

/// Target-constrained native inputs beside one canonical Terminal artifact.
/// Each producing owner transfers a complete unit. The validated entry owns
/// target identity; no independently mutable target/profile pair is retained.
/// Construction and later replay join every unit to the exact artifact. This
/// product grants no execution, installation, proof or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    application: target::NativeApplicationMetadata,
    program_entry: crate::ValidatedNativeProgramEntrySettlement,
    post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    behavior_exclusions: effects::BehaviorExclusions,
}

impl TerminalNativeRealizationProposal {
    /// Retain complete producer units, canonicalize permission order and replay
    /// their artifact-bound joins. Neither permissions nor exclusions are
    /// derived from the provider selection.
    pub fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        application: target::NativeApplicationMetadata,
        program_entry: crate::ValidatedNativeProgramEntrySettlement,
        post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
        providers: TerminalNativeProviders,
        occurrences: TerminalNativeOccurrences,
        boundary_applications: TerminalNativeBoundaryApplications,
        mut package_terminal_authority_permissions: Vec<
            effects::ServiceTerminalAuthorityPermission,
        >,
        behavior_exclusions: effects::BehaviorExclusions,
    ) -> Result<Self, &'static str> {
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    left.requirement_identity()
                        .cmp(right.requirement_identity())
                })
        });
        let proposal = Self {
            terminal_artifact_identity: artifact.manifest().identity(),
            application,
            program_entry,
            post_terminal_optimizations,
            providers,
            occurrences,
            boundary_applications,
            package_terminal_authority_permissions,
            behavior_exclusions,
        };
        proposal.validate_for_artifact(artifact)?;
        Ok(proposal)
    }

    pub fn validate_for_artifact(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        artifact
            .validate()
            .map_err(|_| "Terminal native proposal is paired with an invalid canonical artifact")?;
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("Terminal native proposal belongs to a different canonical artifact");
        }
        let complete_selection = optimization_core::OptimizationSelections::new(
            artifact
                .optimization()
                .selections()
                .as_slice()
                .iter()
                .copied()
                .map(optimization_core::Optimization::from)
                .chain(
                    self.post_terminal_optimizations
                        .selections()
                        .as_slice()
                        .iter()
                        .copied(),
                ),
        )
        .map_err(|_| "Terminal native proposal repeats an optimization across phase custody")?;
        if complete_selection.identity() != self.post_terminal_optimizations.complete_selection() {
            return Err(
                "Terminal native proposal optimization phases do not reconstruct the complete build selection",
            );
        }
        crate::validate_terminal_boundary_application_coverage(
            artifact,
            &self.boundary_applications.scope,
            &self.boundary_applications.coverage,
        )?;
        if self
            .program_entry
            .checked_entry()
            .source_signature_identity()
            != self.program_entry.source().identity().bytes()
            || self.program_entry.checked_entry().source_machine_symbol()
                != self.program_entry.source().machine_symbol()
        {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt drifts from its selected source entry",
            );
        }
        if self.program_entry.checked_entry().terminal_psi_identity()
            != artifact.manifest().semantic()
        {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt drifts from the canonical Terminal identity",
            );
        }
        crate::provider_admission::validate_evaluated_import_rows(
            self.target_profile(),
            self.providers.selection.plans(),
            &self.providers.external_bindings,
        )?;
        self.validate_package_terminal_authority_permissions()?;
        crate::provider_admission::validate_retained_compiler_builtins(
            &self.providers.compiler_builtins,
            self.providers.selection.plans(),
        )?;
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
            |_| "Terminal callback occurrence replay could not decode canonical semantics",
        )?;
        if module.entry != self.program_entry.checked_entry().terminal_entry() {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt names a different Terminal entry",
            );
        }
        program_entry_plan::validate_terminal_fused_service_establishments(
            &module,
            self.program_entry.source(),
            self.program_entry.checked_entry().receiver_eligibility(),
            self.program_entry.fused_service_establishments(),
            self.providers.selection.plans(),
        )?;
        // The retained exclusion union is a source-free admission policy over
        // this artifact's committed semantics: a consumer replays the same
        // Terminal-closure absence check the producing build enforced, so a
        // proposal whose retained policy the artifact cannot satisfy never
        // reaches realization. Boundary-to-service ownership is reconstructed
        // from the canonical requirement identities inside the artifact, not
        // trusted from producer rows. The producing admission gate
        // additionally checked the unoptimized closure committed as this
        // artifact's optimization input; this replay re-verifies the
        // published subject. Physical-authority-class rows produce no site in
        // this walk — mechanisms exist only at realization — but still demand
        // the bounded entry/call closure the walk enforces; their
        // adjudication waits for the mechanism-closure review at native
        // realization, which consumes this same retained union.
        match effects::establish_behavior_exclusions(
            &module,
            &[module.entry],
            &self.behavior_exclusions,
            &self.providers.selection,
        )
        .verdict()
        {
            effects::BehaviorExclusionVerdict::Satisfied => {}
            effects::BehaviorExclusionVerdict::Prohibited => {
                return Err(
                    "Terminal native proposal retains a possible behavior its authored exclusions prohibit",
                );
            }
            effects::BehaviorExclusionVerdict::InsufficientEvidence => {
                return Err(
                    "Terminal native proposal cannot establish the absence its authored exclusions require",
                );
            }
        }
        for demand in self.boundary_applications.coverage.demands().rows() {
            let matching_operations = module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == demand.terminal_operation())
                .count();
            if matching_operations != 1 {
                return Err(
                    "Terminal native proposal boundary demand does not name one canonical operation",
                );
            }
        }
        backend_plan::validate_callback_occurrences_for_module(
            &module,
            &self.occurrences.callbacks,
        )?;
        crate::validate_terminal_comparison_coverage(
            &module,
            self.providers.selection.plans(),
            &self.occurrences.ieee_float_comparisons,
            &self.occurrences.integer_comparisons,
            self.boundary_applications.coverage.realizations(),
            &self.boundary_applications.scope,
        )?;
        Ok(())
    }

    fn validate_package_terminal_authority_permissions(&self) -> Result<(), &'static str> {
        let mut previous = None;
        for permission in &self.package_terminal_authority_permissions {
            if permission.requirement_identity().is_empty()
                || permission
                    .requirement_identity()
                    .chars()
                    .any(char::is_control)
            {
                return Err(
                    "Terminal native proposal contains an invalid package terminal-authority permission requirement",
                );
            }
            let coordinate = (
                permission.service_schema(),
                permission.requirement_identity(),
            );
            if previous == Some(coordinate) {
                return Err(
                    "Terminal native proposal repeats a package terminal-authority permission",
                );
            }
            previous = Some(coordinate);
        }
        Ok(())
    }

    pub const fn terminal_artifact_identity(&self) -> terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub const fn target_profile(&self) -> target::TargetProfile {
        self.program_entry.source().target_slot().owner
    }

    pub const fn native_target(&self) -> target::NativeTarget {
        self.program_entry.target()
    }

    pub const fn application(&self) -> &target::NativeApplicationMetadata {
        &self.application
    }

    pub const fn program_entry(&self) -> &crate::ValidatedNativeProgramEntrySettlement {
        &self.program_entry
    }

    pub const fn post_terminal_optimizations(
        &self,
    ) -> &optimization_core::PostTerminalOptimizationSelectionProjection {
        &self.post_terminal_optimizations
    }

    pub const fn providers(&self) -> &TerminalNativeProviders {
        &self.providers
    }

    pub const fn occurrences(&self) -> &TerminalNativeOccurrences {
        &self.occurrences
    }

    pub const fn boundary_applications(&self) -> &TerminalNativeBoundaryApplications {
        &self.boundary_applications
    }

    /// Accepted package rows do not grant receiver permission. Re-entry must
    /// match independent package evidence and any explicit receiving policy.
    pub fn package_terminal_authority_permissions(
        &self,
    ) -> &[effects::ServiceTerminalAuthorityPermission] {
        &self.package_terminal_authority_permissions
    }

    /// Authored exclusions constrain canonical semantics and native mechanism
    /// use, even when no receiving permission policy is supplied.
    pub const fn behavior_exclusions(&self) -> &effects::BehaviorExclusions {
        &self.behavior_exclusions
    }
}

/// Canonical Terminal artifact coupled to every exact checked callback
/// placement and target-constrained native proposal that crossed its
/// production boundary.
///
/// Callback rows remain target-owned compiler evidence rather than Terminal-
/// Psi vocabulary. Keeping them in the same retained product prevents an
/// artifact-only consuming escape from silently discarding the sidecar. This
/// carrier grants no registration, invocation, address, or lifetime authority.
#[derive(Debug)]
pub struct RetainedTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
}

impl RetainedTerminalArtifact {
    /// Couples structurally valid rows to an artifact without reconstructing
    /// their checked-compilation provenance. The compiler's private product
    /// route supplies that provenance and preserves its checked row order.
    pub fn new(
        artifact: terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: None,
        };
        retained.validate()?;
        Ok(retained)
    }

    pub fn new_with_native_realization_proposal(
        artifact: terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: Some(native_realization_proposal),
        };
        retained.validate()?;
        Ok(retained)
    }

    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn callback_placements(&self) -> &[backend_plan::BoundNominalCallbackPlacement] {
        &self.callback_placements
    }

    pub const fn native_realization_proposal(&self) -> Option<&TerminalNativeRealizationProposal> {
        self.native_realization_proposal.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        Vec<backend_plan::BoundNominalCallbackPlacement>,
        Option<TerminalNativeRealizationProposal>,
    ) {
        (
            self.artifact,
            self.callback_placements,
            self.native_realization_proposal,
        )
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.artifact
            .validate()
            .map_err(|_| "retained Terminal product contains an invalid canonical artifact")?;
        for placement in &self.callback_placements {
            backend_plan::validate_bound_nominal_callback_placement(placement)
                .map_err(|_| "retained Terminal product contains an invalid callback placement")?;
        }
        if let Some(proposal) = &self.native_realization_proposal {
            proposal.validate_for_artifact(&self.artifact)?;
            backend_plan::validate_callback_occurrence_placements(
                &self.callback_placements,
                &proposal.occurrences.callbacks,
            )?;
        }
        Ok(())
    }
}
