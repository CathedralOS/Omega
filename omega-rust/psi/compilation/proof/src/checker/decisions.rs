//! Successful checking output, recorded at the original obligation boundary.
//!
//! Measurements remain observations. Accepted facts retain the kernel's
//! actual acceptance and their source obligation handle instead of being
//! reduced to measurement counts or reconstructed by a later consumer.

use arena::Handle;
use proof_admission::AcceptedFact;

use super::{CheckedIntegerAssignment, ProofPlanMeasurements};
use crate::obligations::ProofObligation;

pub struct AcceptedProofJudgment {
    pub obligation: Handle<ProofObligation>,
    pub accepted: AcceptedFact,
}

/// Construction output consumed once by checked-tree fact publication.
#[derive(Default)]
pub struct ProofPlanDecisions {
    pub integer_assignments: Vec<CheckedIntegerAssignment>,
    pub accepted_judgments: Vec<AcceptedProofJudgment>,
}

#[derive(Default)]
pub(crate) struct ProofCheckRecorder {
    pub(crate) measurements: ProofPlanMeasurements,
    pub(crate) decisions: ProofPlanDecisions,
    pub(crate) obligation: Handle<ProofObligation>,
}

impl ProofCheckRecorder {
    pub(crate) fn retain_accepted_fact(&mut self, accepted: AcceptedFact) {
        self.measurements.record_accepted_fact(&accepted);
        self.decisions
            .accepted_judgments
            .push(AcceptedProofJudgment {
                obligation: self.obligation,
                accepted,
            });
    }
}
