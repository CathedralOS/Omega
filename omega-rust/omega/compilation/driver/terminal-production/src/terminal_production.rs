//! Product reporting for Psi-owned Terminal production.
//!
//! Psi owns the stage 05-07 sequence, the request and the application custody.
//! This entry supplies the product request's timing recorder and counts the
//! production it observes. It never lowers, verifies, selects targets or
//! executes guests.

use artifacts::compile_timings::CompileTimings;
use psi_compiler::{
    ProducedTerminalProgram, TerminalApplicationCustody, TerminalProductionCustody,
    TerminalProductionRequest,
};

use crate::stage_timings::PsiStageTimings;

pub use psi_compiler::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};

/// Produce the canonical Terminal artifact beside the custody the caller
/// asked to retain, recording the product production counter once and mapping
/// Psi's stage labels into the product report.
///
/// `applications` are ordinary Psi application rows: the product policy
/// ([`crate::TerminalBehaviorExclusions`]) reaches Psi as each row's
/// `admission`. An empty slice requests only the portable artifact.
pub fn produce_terminal_artifact(
    request: TerminalProductionRequest<'_>,
    applications: &[TerminalApplicationCustody<'_>],
    timings: &mut CompileTimings,
) -> Result<ProducedTerminalProgram, TerminalArtifactProductionError> {
    artifacts::compile_timings::counters::record_terminal_production();
    request.produce(
        TerminalProductionCustody { applications },
        &mut PsiStageTimings::new(timings),
    )
}
