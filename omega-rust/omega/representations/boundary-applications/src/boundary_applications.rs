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
pub mod realization;

pub use coverage::{BoundaryApplicationCoverageIdentity, OperatorApplicationCoverageRef};
pub use demands::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryNominalIdentity,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
    TerminalBoundaryApplicationDemands,
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
        })
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

    pub fn into_parts(
        self,
    ) -> (
        TerminalBoundaryApplicationDemands,
        TerminalBoundaryApplicationRealizations,
    ) {
        (self.demands, self.realizations)
    }
}
