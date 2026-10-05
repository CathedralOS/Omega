//! Product timing observations around Psi-owned Terminal production.
//!
//! Psi owns stable production labels; Omega borrows its existing request
//! recorder here. No timing carrier or product-report dependency crosses upward
//! into Psi, and this adapter neither retries nor interprets a stage judgment.

use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use psi_terminal_production::{TerminalProductionObserver, TerminalProductionStage};

/// Borrow the existing recorder instead of importing host reporting into Psi.
pub(crate) struct ProductProductionObserver<'a> {
    pub(crate) timings: &'a mut CompileTimings,
}

impl TerminalProductionObserver for ProductProductionObserver<'_> {
    fn record_result<Value, Error>(
        &mut self,
        stage: TerminalProductionStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let (name, input, output) = stage.observation_labels();
        self.timings.record_result(
            StageMeta::new(name, input, output, TimingCategory::Pipeline),
            operation,
        )
    }
}
