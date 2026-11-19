//! Physical publication replays each native realization against the operation
//! and application required by the published Psi artifact.

#![forbid(unsafe_code)]

pub mod coverage;
pub mod opaque_applications;
pub mod realization;
#[cfg(test)]
mod source_guards;

pub use coverage::OperatorApplicationCoverageRef;
pub use opaque_applications::{
    BoundaryOpaqueRepresentationApplication, BoundaryOpaqueRepresentationApplications,
};
pub use realization::{
    BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    TerminalBoundaryApplicationRealizations,
};
use terminal_psi::TerminalBoundaryApplicationDemands;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationCoverage {
    demands: TerminalBoundaryApplicationDemands,
    realizations: TerminalBoundaryApplicationRealizations,
    references: Vec<OperatorApplicationCoverageRef>,
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
