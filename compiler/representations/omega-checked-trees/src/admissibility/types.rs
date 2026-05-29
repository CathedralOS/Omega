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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceCheck {
    pub dimension: AcceptanceDimension,
    pub verdict: AcceptanceCheckVerdict,
    pub evidence_count: usize,
}

impl AcceptanceCheck {
    pub const fn accepted(dimension: AcceptanceDimension, evidence_count: usize) -> Self {
        Self {
            dimension,
            verdict: AcceptanceCheckVerdict::Accepted,
            evidence_count,
        }
    }

    pub const fn not_applicable(dimension: AcceptanceDimension) -> Self {
        Self {
            dimension,
            verdict: AcceptanceCheckVerdict::NotApplicable,
            evidence_count: 0,
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

impl AcceptanceSummary {
    pub const fn accepted(
        borrow_evidence: usize,
        proof_evidence: usize,
        effect_evidence: usize,
        boundary_evidence: usize,
        termination_evidence: usize,
    ) -> Self {
        Self {
            verdict: AcceptanceVerdict::Accepted,
            borrow: AcceptanceCheck::accepted(AcceptanceDimension::Borrow, borrow_evidence),
            proof: AcceptanceCheck::accepted(AcceptanceDimension::Proof, proof_evidence),
            effects: AcceptanceCheck::accepted(AcceptanceDimension::Effects, effect_evidence),
            boundaries: AcceptanceCheck::accepted(
                AcceptanceDimension::Boundaries,
                boundary_evidence,
            ),
            termination: if termination_evidence == 0 {
                AcceptanceCheck::not_applicable(AcceptanceDimension::Termination)
            } else {
                AcceptanceCheck::accepted(AcceptanceDimension::Termination, termination_evidence)
            },
        }
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

    pub fn rejected_checks(self) -> impl Iterator<Item = AcceptanceCheck> {
        self.checks()
            .into_iter()
            .filter(|check| !check.is_satisfied())
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
