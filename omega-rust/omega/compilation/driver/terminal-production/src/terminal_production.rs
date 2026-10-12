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
    /// Original successful source input for compilation-local content retention.
    /// This does not cross the canonical Terminal artifact boundary.
    pub original_source_input: Option<&'a psi_compiler::OriginalCheckedSourceInput>,
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
            original_source_input: None,
            checked,
            exact_toolchain_sources,
            machine,
            roots: psi_compiler::TerminalProgramRoots::default(),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Retain the actual original input beside this production request. It is
    /// not a certificate that a pruned/lowered artifact contains every input.
    pub fn with_original_source_input(
        mut self,
        input: &'a psi_compiler::OriginalCheckedSourceInput,
    ) -> Self {
        self.original_source_input = Some(input);
        self
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
            original_source_input: self.original_source_input,
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
