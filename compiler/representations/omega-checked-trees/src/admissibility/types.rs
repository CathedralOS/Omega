use crate::{CheckFacts, FlowCallFact, FlowExitFact, FlowStateFact, FlowStatementFact};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptanceVerdict {
    #[default]
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceDimension {
    Borrow,
    Proof,
    Effects,
    Boundaries,
    Termination,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptanceCheckVerdict {
    #[default]
    Accepted,
    Rejected,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptanceCheckProvenance {
    #[default]
    AcceptedByEvidence,
    NotRequired,
    DiagnosticPending,
    RejectedByDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceCheck {
    pub dimension: AcceptanceDimension,
    pub verdict: AcceptanceCheckVerdict,
    pub evidence_count: usize,
    pub diagnostic_count: usize,
    pub provenance: AcceptanceCheckProvenance,
}

impl AcceptanceCheck {
    pub const fn accepted(dimension: AcceptanceDimension, evidence_count: usize) -> Self {
        Self {
            dimension,
            verdict: AcceptanceCheckVerdict::Accepted,
            evidence_count,
            diagnostic_count: 0,
            provenance: AcceptanceCheckProvenance::AcceptedByEvidence,
        }
    }

    pub const fn not_applicable(dimension: AcceptanceDimension) -> Self {
        Self {
            dimension,
            verdict: AcceptanceCheckVerdict::NotApplicable,
            evidence_count: 0,
            diagnostic_count: 0,
            provenance: AcceptanceCheckProvenance::NotRequired,
        }
    }

    pub const fn rejected(dimension: AcceptanceDimension, diagnostic_count: usize) -> Self {
        Self {
            dimension,
            verdict: AcceptanceCheckVerdict::Rejected,
            evidence_count: 0,
            diagnostic_count,
            provenance: if diagnostic_count == 0 {
                AcceptanceCheckProvenance::DiagnosticPending
            } else {
                AcceptanceCheckProvenance::RejectedByDiagnostic
            },
        }
    }

    pub const fn is_satisfied(self) -> bool {
        matches!(
            self.verdict,
            AcceptanceCheckVerdict::Accepted | AcceptanceCheckVerdict::NotApplicable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceSummary {
    pub verdict: AcceptanceVerdict,
    pub borrow: AcceptanceCheck,
    pub proof: AcceptanceCheck,
    pub effects: AcceptanceCheck,
    pub boundaries: AcceptanceCheck,
    pub termination: AcceptanceCheck,
}

pub trait AcceptanceView {
    fn summary(&self) -> AcceptanceSummary;

    fn check(&self, dimension: AcceptanceDimension) -> AcceptanceCheck {
        self.summary().check(dimension)
    }

    fn verdict(&self) -> AcceptanceVerdict {
        self.summary().verdict
    }

    fn is_accepted(&self) -> bool {
        self.summary().is_accepted()
    }

    fn is_dimension_satisfied(&self, dimension: AcceptanceDimension) -> bool {
        self.check(dimension).is_satisfied()
    }

    fn evidence_count(&self) -> usize {
        self.summary().evidence_count()
    }

    fn diagnostic_count(&self) -> usize {
        self.summary().diagnostic_count()
    }

    fn rejected_check_count(&self) -> usize {
        self.summary().rejected_check_count()
    }

    fn has_diagnostics(&self) -> bool {
        self.summary().has_diagnostics()
    }
}

impl AcceptanceSummary {
    pub const fn with_checks(
        borrow: AcceptanceCheck,
        proof: AcceptanceCheck,
        effects: AcceptanceCheck,
        boundaries: AcceptanceCheck,
        termination: AcceptanceCheck,
    ) -> Self {
        let verdict = if borrow.is_satisfied()
            && proof.is_satisfied()
            && effects.is_satisfied()
            && boundaries.is_satisfied()
            && termination.is_satisfied()
        {
            AcceptanceVerdict::Accepted
        } else {
            AcceptanceVerdict::Rejected
        };

        Self {
            verdict,
            borrow,
            proof,
            effects,
            boundaries,
            termination,
        }
    }

    pub const fn accepted(
        borrow_evidence: usize,
        proof_evidence: usize,
        effect_evidence: usize,
        boundary_evidence: usize,
        termination_evidence: usize,
    ) -> Self {
        Self::with_checks(
            AcceptanceCheck::accepted(AcceptanceDimension::Borrow, borrow_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Proof, proof_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Effects, effect_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Boundaries, boundary_evidence),
            if termination_evidence == 0 {
                AcceptanceCheck::not_applicable(AcceptanceDimension::Termination)
            } else {
                AcceptanceCheck::accepted(AcceptanceDimension::Termination, termination_evidence)
            },
        )
    }

    pub const fn is_accepted(self) -> bool {
        matches!(self.verdict, AcceptanceVerdict::Accepted)
            && self.borrow.is_satisfied()
            && self.proof.is_satisfied()
            && self.effects.is_satisfied()
            && self.boundaries.is_satisfied()
            && self.termination.is_satisfied()
    }

    pub const fn checks(self) -> [AcceptanceCheck; 5] {
        [
            self.borrow,
            self.proof,
            self.effects,
            self.boundaries,
            self.termination,
        ]
    }

    pub const fn check(self, dimension: AcceptanceDimension) -> AcceptanceCheck {
        match dimension {
            AcceptanceDimension::Borrow => self.borrow,
            AcceptanceDimension::Proof => self.proof,
            AcceptanceDimension::Effects => self.effects,
            AcceptanceDimension::Boundaries => self.boundaries,
            AcceptanceDimension::Termination => self.termination,
        }
    }

    pub const fn is_dimension_satisfied(self, dimension: AcceptanceDimension) -> bool {
        self.check(dimension).is_satisfied()
    }

    pub fn rejected_checks(self) -> impl Iterator<Item = AcceptanceCheck> {
        self.checks()
            .into_iter()
            .filter(|check| !check.is_satisfied())
    }

    pub fn evidence_count(self) -> usize {
        self.checks()
            .into_iter()
            .map(|check| check.evidence_count)
            .sum()
    }

    pub fn diagnostic_count(self) -> usize {
        self.checks()
            .into_iter()
            .map(|check| check.diagnostic_count)
            .sum()
    }

    pub fn rejected_check_count(self) -> usize {
        self.rejected_checks().count()
    }

    pub fn has_diagnostics(self) -> bool {
        self.diagnostic_count() > 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StateAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) state: &'facts FlowStateFact,
}

#[derive(Debug, Clone, Copy)]
pub struct StatementAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) statement: &'facts FlowStatementFact,
}

#[derive(Debug, Clone, Copy)]
pub struct CallAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) call: &'facts FlowCallFact,
}

#[derive(Debug, Clone, Copy)]
pub struct ExitAcceptance<'facts> {
    pub(crate) facts: &'facts CheckFacts,
    pub(crate) exit: &'facts FlowExitFact,
}
