//! Product timing observations around Psi-owned compilation stages.
//!
//! Psi owns stable production labels; Omega borrows its existing request
//! recorder here. No timing carrier or product-report dependency crosses upward
//! into Psi, and this adapter neither retries nor interprets a stage judgment.

use artifacts::compile_timings::{CompileTimings, StageMeta};
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage};

/// Borrow the existing recorder instead of importing host reporting into Psi.
pub(crate) struct PsiStageTimings<'a> {
    timings: &'a mut CompileTimings,
}

impl<'a> PsiStageTimings<'a> {
    pub(crate) fn new(timings: &'a mut CompileTimings) -> Self {
        Self { timings }
    }
}

impl PsiCompilationObserver for PsiStageTimings<'_> {
    fn record_result<Value, Error>(
        &mut self,
        stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        // The first lowering leg is one product production attempt, including
        // attempts that refuse. The observer records work; it never drives it.
        if stage == PsiCompilationStage::Lowering {
            artifacts::compile_timings::counters::record_terminal_production();
        }
        let (name, input, output) = stage.observation_labels();
        self.timings
            .record_result(StageMeta::new(name, input, output), operation)
    }
}
