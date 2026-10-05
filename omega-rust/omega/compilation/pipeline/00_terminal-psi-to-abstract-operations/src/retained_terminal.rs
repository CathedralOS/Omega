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
/// The proposal never leaves the process that produced it and has no codec,
/// so each unit is constructed once from its derivation in
/// `project_terminal_native_realization_proposal` and no later stage replays
/// its artifact joins. This product grants no execution, installation, proof
/// or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
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
    /// Retain complete producer units and canonicalize permission order.
    /// Neither permissions nor exclusions are derived from the provider
    /// selection. Every unit arrives from its derivation in this compile, so
    /// construction does not replay its joins against the artifact again
    /// (pipeline.md, "Verification placement").
    pub fn new(
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
    ) -> Self {
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    left.requirement_identity()
                        .cmp(right.requirement_identity())
                })
        });
        Self {
            application,
            program_entry,
            post_terminal_optimizations,
            providers,
            occurrences,
            boundary_applications,
            package_terminal_authority_permissions,
            behavior_exclusions,
        }
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
    /// Couple the artifact with the callback placements and native proposal
    /// this compile produced beside it. The proposal's producer already
    /// joined every unit to this artifact; the carrier adds no second replay.
    pub fn new_with_native_realization_proposal(
        artifact: terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Self {
        Self {
            artifact,
            callback_placements,
            native_realization_proposal: Some(native_realization_proposal),
        }
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
}
