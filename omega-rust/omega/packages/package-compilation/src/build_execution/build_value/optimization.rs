// Build-authored optimization vocabulary admission and selection extraction.
//
// `BuildOptimizationAdmission` classifies the `Build` declaration before evaluation
// (`Canonical` vs `LegacyWithoutField`), constructs the zeroed `optimizations`
// field for the interpreter input, and extracts the evaluated `OptimizationSelections`
// and `OptimizationReportRequest` afterward so legacy empty-selection behavior
// cannot diverge from canonical decoding.

mod selection;
mod vocabulary;

use build_time_evaluation::BuildTimeValue;
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use typed_trees::TypedTrees;

use vocabulary::OptimizationBuildVocabulary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuildOptimizationAdmission {
    vocabulary: OptimizationBuildVocabulary,
}

impl BuildOptimizationAdmission {
    pub(crate) fn admit(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        vocabulary::classify(typed).map(|vocabulary| Self { vocabulary })
    }

    pub(crate) fn zero_build_field(self) -> Option<(String, BuildTimeValue)> {
        selection::zero_build_field(self.vocabulary)
    }

    pub(crate) fn extract(
        self,
        build: &BuildTimeValue,
    ) -> Result<
        (
            OptimizationSelections,
            optimization_core::OptimizationReportRequest,
        ),
        String,
    > {
        selection::extract(build, self.vocabulary)
    }
}
