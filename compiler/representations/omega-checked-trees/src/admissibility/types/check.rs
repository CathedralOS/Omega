use super::AcceptanceDimension;

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
