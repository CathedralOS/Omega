//! Product timing observations around Psi-owned compilation stages.
//!
//! Psi owns stable production labels; Omega borrows its existing request
//! recorder here. No timing carrier or product-report dependency crosses upward
//! into Psi, and this adapter neither retries nor interprets a stage judgment.

use artifacts::compile_timings::{CompileTimings, StageMeta};
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage};

/// Borrow the existing recorder instead of importing host reporting into Psi.
pub struct PsiStageTimings<'a> {
    timings: &'a mut CompileTimings,
}

impl<'a> PsiStageTimings<'a> {
    pub fn new(timings: &'a mut CompileTimings) -> Self {
        Self { timings }
    }
}

impl PsiCompilationObserver for PsiStageTimings<'_> {
    fn record_result<Value, Error>(
        &mut self,
        stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let (name, input, output) = stage.observation_labels();
        self.timings
            .record_result(StageMeta::new(name, input, output), operation)
    }
}
