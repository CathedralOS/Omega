//! Source-free D29 boundary-operator application data retained beside one
//! canonical Terminal artifact.
//!
//! `demands` owns the demand vocabulary: exact-owner identities, the operator
//! requirement, the closed tagged application, and the ordered demand rows.
//! `realization` owns the role-specific companion selected for each demand.
//! `coverage` joins them into reconstructible identity references.
//! [`TerminalBoundaryApplicationCoverage`] is the current retained coverage of
//! one Terminal product; it carries no source paths, arena handles, selected
//! plans, or realization authority.

pub mod coverage;
pub mod demands;
pub mod opaque_applications;
pub mod realization;

pub use coverage::{BoundaryApplicationCoverageIdentity, OperatorApplicationCoverageRef};
pub use demands::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryNominalIdentity,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
    TerminalBoundaryApplicationDemands,
};
pub use opaque_applications::{
    BoundaryOpaqueRepresentationApplication, BoundaryOpaqueRepresentationApplications,
};
pub use realization::{
    BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    BoundaryApplicationRealizationRole, TerminalBoundaryApplicationRealizations,
};

/// Complete reconstructible D29 semantic coverage retained beside one
/// canonical Terminal product.
///
/// This carrier keeps the full demand and realization rows rather than only
/// their derived references. An empty value therefore means an exact empty
/// demand set; absence must be represented by an owning product separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationCoverage {
    demands: TerminalBoundaryApplicationDemands,
    realizations: TerminalBoundaryApplicationRealizations,
    references: Vec<OperatorApplicationCoverageRef>,
    /// Strong selected-application custody for every by-value opaque boundary
    /// edge in the same retained signatures. The canonical commitment set is
    /// part of this coverage's identity and replay, so producer and consumer
    /// artifacts compare the exact application at each edge.
    opaque_applications: opaque_applications::BoundaryOpaqueRepresentationApplications,
}

impl TerminalBoundaryApplicationCoverage {
    pub fn new(
        demands: TerminalBoundaryApplicationDemands,
        realizations: TerminalBoundaryApplicationRealizations,
    ) -> Result<Self, &'static str> {
        let references = realizations.coverage_references(&demands)?;
        Ok(Self {
            demands,
            realizations,
            references,
            opaque_applications:
                opaque_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        })
    }

    /// Retain the checked compilation's canonical by-value opaque-application
    /// custody beside this coverage. The custody is canonicalized at its own
    /// construction; it is evidence, not a demand or realization row.
    pub fn with_opaque_applications(
        mut self,
        applications: opaque_applications::BoundaryOpaqueRepresentationApplications,
    ) -> Self {
        self.opaque_applications = applications;
        self
    }

    pub fn validate_for_terminal(
        &self,
        terminal: terminal_psi::TerminalPsiIdentity,
    ) -> Result<(), &'static str> {
        self.demands.validate_for_terminal(terminal)?;
        self.realizations.validate_for_demands(&self.demands)?;
        if self.references != self.realizations.coverage_references(&self.demands)? {
            return Err("boundary application coverage references do not replay");
        }
        Ok(())
    }

    pub const fn demands(&self) -> &TerminalBoundaryApplicationDemands {
        &self.demands
    }

    pub const fn realizations(&self) -> &TerminalBoundaryApplicationRealizations {
        &self.realizations
    }

    pub fn references(&self) -> &[OperatorApplicationCoverageRef] {
        &self.references
    }

    /// Canonical selected-application custody for by-value opaque boundary
    /// edges. An artifact carrying no opaque edges retains the canonical empty
    /// set; absence of coverage entirely is owned by the artifact's `Option`.
    pub const fn opaque_applications(
        &self,
    ) -> &opaque_applications::BoundaryOpaqueRepresentationApplications {
        &self.opaque_applications
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalBoundaryApplicationDemands,
        TerminalBoundaryApplicationRealizations,
        opaque_applications::BoundaryOpaqueRepresentationApplications,
    ) {
        (self.demands, self.realizations, self.opaque_applications)
    }
}
