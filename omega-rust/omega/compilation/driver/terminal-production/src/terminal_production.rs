//! Product reporting adapter for Psi-owned Terminal production.
//!
//! Psi owns the single stage 05-07 sequence. This adapter supplies the product
//! request's timing recorder. It never lowers, verifies, selects targets or
//! executes guests.

use checked_trees::CheckedTrees;
use psi_compiler::TerminalMachineSelection;

use crate::custody::{ProducedTerminalArtifact, TerminalProductionCustody};
use crate::stage_timings::PsiStageTimings;

pub use psi_compiler::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};

/// Checked source and exact machine selection for product compilation.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub machine: TerminalMachineSelection<'a>,
    pub roots: psi_compiler::TerminalProgramRoots<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    pub fn new(
        checked: &'a CheckedTrees,
        machine: TerminalMachineSelection<'a>,
        exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    ) -> Self {
        Self {
            checked,
            exact_toolchain_sources,
            machine,
            roots: psi_compiler::TerminalProgramRoots::default(),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Publish through the same Psi operation compile-time jobs can consume,
    /// retaining the source custody and product report requested by this caller.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        artifacts::compile_timings::counters::record_terminal_production();
        let TerminalProductionCustody {
            applications,
            timings,
        } = custody;
        let applications: Vec<_> = applications
            .iter()
            .map(|application| psi_compiler::TerminalApplicationCustody {
                entry_identity: application.entry_identity,
                entry_machine: application.entry_machine,
                admission_roots: application.admission_roots,
                admission: application
                    .behavior_exclusions
                    .as_ref()
                    .map(|policy| policy as &dyn psi_compiler::TerminalCompositionAdmission),
            })
            .collect();
        let produced = psi_compiler::TerminalProductionRequest {
            checked: self.checked,
            exact_toolchain_sources: self.exact_toolchain_sources,
            machine: self.machine,
            roots: self.roots,
            optimization_selections: self.optimization_selections,
        }
        .produce(
            psi_compiler::TerminalProductionCustody {
                applications: &applications,
            },
            &mut PsiStageTimings::new(timings),
        )?;
        Ok(produced)
    }
}
