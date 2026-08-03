use super::{AcceptanceCheck, AcceptanceDimension};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptanceVerdict {
    #[default]
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceSummary {
    pub verdict: AcceptanceVerdict,
    pub borrow: AcceptanceCheck,
    pub proof: AcceptanceCheck,
    pub service_reach: AcceptanceCheck,
    pub suspension: AcceptanceCheck,
    pub blocking: AcceptanceCheck,
    pub boundaries: AcceptanceCheck,
    pub termination: AcceptanceCheck,
}

impl AcceptanceSummary {
    pub const fn with_checks(
        borrow: AcceptanceCheck,
        proof: AcceptanceCheck,
        service_reach: AcceptanceCheck,
        suspension: AcceptanceCheck,
        blocking: AcceptanceCheck,
        boundaries: AcceptanceCheck,
        termination: AcceptanceCheck,
    ) -> Self {
        let verdict = if borrow.is_satisfied()
            && proof.is_satisfied()
            && service_reach.is_satisfied()
            && suspension.is_satisfied()
            && blocking.is_satisfied()
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
            service_reach,
            suspension,
            blocking,
            boundaries,
            termination,
        }
    }

    pub const fn accepted(
        borrow_evidence: usize,
        proof_evidence: usize,
        service_reach_evidence: usize,
        suspension_evidence: usize,
        blocking_evidence: usize,
        boundary_evidence: usize,
        termination_evidence: usize,
    ) -> Self {
        Self::with_checks(
            AcceptanceCheck::accepted(AcceptanceDimension::Borrow, borrow_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Proof, proof_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::ServiceReach, service_reach_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Suspension, suspension_evidence),
            AcceptanceCheck::accepted(AcceptanceDimension::Blocking, blocking_evidence),
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
            && self.service_reach.is_satisfied()
            && self.suspension.is_satisfied()
            && self.blocking.is_satisfied()
            && self.boundaries.is_satisfied()
            && self.termination.is_satisfied()
    }

    pub const fn checks(self) -> [AcceptanceCheck; 7] {
        [
            self.borrow,
            self.proof,
            self.service_reach,
            self.suspension,
            self.blocking,
            self.boundaries,
            self.termination,
        ]
    }

    pub const fn check(self, dimension: AcceptanceDimension) -> AcceptanceCheck {
        match dimension {
            AcceptanceDimension::Borrow => self.borrow,
            AcceptanceDimension::Proof => self.proof,
            AcceptanceDimension::ServiceReach => self.service_reach,
            AcceptanceDimension::Suspension => self.suspension,
            AcceptanceDimension::Blocking => self.blocking,
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
