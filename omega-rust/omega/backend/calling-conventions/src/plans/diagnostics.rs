//! The plan diagnostics.

use crate::plans::CallingPolicyRejection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDiagnostic(pub String);

impl std::fmt::Display for PlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlanDiagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanDiagnostic {
    Rejected(CallingPolicyRejection),
    InvalidAcceptedPlan(PlanDiagnostic),
}

impl std::fmt::Display for BoundaryPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => {
                write!(
                    formatter,
                    "calling policy rejected the boundary: {}",
                    rejection.reason()
                )
            }
            Self::InvalidAcceptedPlan(diagnostic) => {
                write!(
                    formatter,
                    "calling policy accepted an invalid plan: {diagnostic}"
                )
            }
        }
    }
}

impl std::error::Error for BoundaryPlanDiagnostic {}
