//! Optimizer module role: executable entrance. Build-authored optimization selection.
//!
//! Admission binds the exact toolchain vocabulary before evaluation. The same
//! value then constructs the zeroed interpreter input and extracts the exact
//! selection afterward, so legacy empty-selection behavior cannot diverge.

mod selection;
mod vocabulary;

use omega_optimization_core::OptimizationSelections;
use psi_build_time_evaluation::BuildTimeValue;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

use vocabulary::OptimizationBuildVocabulary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BuildOptimizationAdmission {
    vocabulary: OptimizationBuildVocabulary,
}

impl BuildOptimizationAdmission {
    pub(super) fn admit(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        vocabulary::classify(typed).map(|vocabulary| Self { vocabulary })
    }

    pub(super) fn zero_build_field(self) -> Option<(String, BuildTimeValue)> {
        selection::zero_build_field(self.vocabulary)
    }

    pub(super) fn extract(
        self,
        build: &BuildTimeValue,
    ) -> Result<
        (
            OptimizationSelections,
            omega_optimization_pipeline::OptimizationReportRequest,
        ),
        String,
    > {
        selection::extract(build, self.vocabulary)
    }
}
