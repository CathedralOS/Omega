//! The retained Terminal product and the target-constrained proposal that has to
//! match it occurrence for occurrence - every boundary demand, callback and
//! nearest-FMA site - before native realization is allowed to begin.

mod native_providers;
mod proposal;

pub use proposal::{
    SelectedFamilyRealizationSource, TerminalNativeProposalRequest,
    project_terminal_native_realization_proposal,
};

pub use backend_plan::TerminalTargetProviderProposal;
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
    /// Declaration-qualified complete row projection, including closed
    /// requirements and family aliases. This is policy, not installation.
    pub checked_adapters: Vec<crate::SelectedProviderAdapter>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
    /// Build-admitted AVX+FMA3 provider, joined to x86 FMA demand by planning.
    pub x86_scalar_fma: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Exact occurrence catalogs produced while checked source custody exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeOccurrences {
    pub callbacks: Vec<TerminalCallbackOccurrenceProposal>,
    /// None retains an unmaterialized target callback, not an empty roster.
    /// Such a Terminal product remains usable, but cannot enter native lowering.
    pub callback_arguments: Option<Vec<crate::NativeCallbackArgumentProposal>>,
    pub ieee_float_comparisons: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
    pub integer_comparisons: Vec<TerminalIntegerComparisonOccurrenceProposal>,
}

/// Keep the producer's complete source-free coverage intact.
/// Splitting coverage into demands, realizations and opaque applications would
/// discard its already-constructed identity references only to rebuild them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeBoundaryApplications {
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

/// Program-wide provider and occurrence custody, independent of any selected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalNativeBodyProposal {
    providers: TerminalNativeProviders,
    occurrences: TerminalNativeOccurrences,
    boundary_applications: TerminalNativeBoundaryApplications,
    behavior_exclusions: effects::BehaviorExclusions,
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
    package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
    behavior_exclusions: effects::BehaviorExclusions,
    target_provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
}

impl TerminalNativeRealizationProposal {
    /// Retain complete producer units and canonicalize permission order.
    /// Neither permissions nor exclusions are derived from the provider
    /// selection. Every unit arrives from its derivation in this compile, so
    /// construction does not replay its joins against the artifact again
    /// (pipeline.md, "Verification placement").
    pub(crate) fn new(
        application: target::NativeApplicationMetadata,
        program_entry: crate::ValidatedNativeProgramEntrySettlement,
        post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
        providers: TerminalNativeProviders,
        occurrences: TerminalNativeOccurrences,
        boundary_applications: TerminalNativeBoundaryApplications,
        mut package_terminal_authority_permissions: Vec<
            effects::ServiceTerminalAuthorityPermission,
        >,
        package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
        behavior_exclusions: effects::BehaviorExclusions,
    ) -> Self {
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(&right.service_schema().as_bytes())
                .then_with(|| {
                    package_terminal_authority_permission_identities
                        .text(left.requirement_identity())
                        .cmp(
                            &package_terminal_authority_permission_identities
                                .text(right.requirement_identity()),
                        )
                })
        });
        Self {
            target_provider_proposals: Vec::new(),
            application,
            program_entry,
            post_terminal_optimizations,
            providers,
            occurrences,
            boundary_applications,
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
            behavior_exclusions,
        }
    }

    pub const fn target_profile(&self) -> target::TargetProfile {
        self.program_entry.source().target_slot().owner
    }

    /// Keep all authored deployment choices beside the portable artifact.
    /// These are pending selections, not the selected entry's backing or grants.
    pub fn with_target_provider_proposals(
        mut self,
        proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    ) -> Self {
        self.target_provider_proposals = proposals;
        self
    }

    pub fn target_provider_proposals(&self) -> &[backend_plan::TerminalTargetProviderProposal] {
        &self.target_provider_proposals
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

    /// The table the package permission rows' `TerminalIdentityIndex` values
    /// index.
    pub const fn package_terminal_authority_permission_identities(
        &self,
    ) -> &terminal_psi::TerminalIdentityTable {
        &self.package_terminal_authority_permission_identities
    }

    /// Authored exclusions constrain canonical semantics and native mechanism
    /// use, even when no receiving permission policy is supplied.
    pub const fn behavior_exclusions(&self) -> &effects::BehaviorExclusions {
        &self.behavior_exclusions
    }
}

/// Canonical Terminal artifact and its target-constrained native proposal.
///
/// Target products share immutable semantic storage; each owns only its own
/// realization proposal. Cloning an ownership handle never copies Terminal.
/// The proposal owns callback ABI and materialization rows; no checked-source
/// placement survives this boundary. This carrier grants no registration,
/// invocation, address, or lifetime authority.
#[derive(Debug)]
pub struct RetainedTerminalArtifact {
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
}

impl RetainedTerminalArtifact {
    /// Couple the artifact with the complete native proposal
    /// this compile produced beside it. The proposal's producer already
    /// joined every unit to this artifact; the carrier adds no second replay.
    pub fn new_with_native_realization_proposal(
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Self {
        Self {
            artifact,
            native_realization_proposal: Some(native_realization_proposal),
        }
    }

    pub fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn native_realization_proposal(&self) -> Option<&TerminalNativeRealizationProposal> {
        self.native_realization_proposal.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        Option<TerminalNativeRealizationProposal>,
    ) {
        (self.artifact, self.native_realization_proposal)
    }
}
