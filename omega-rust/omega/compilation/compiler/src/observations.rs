// We adapt `CompileTimings` to Psi's `PsiCompilationObserver` trait so stage
// durations and terminal-production counters are recorded without importing
// host reporting carriers into Psi. Publication is one product production
// attempt, including an attempt that refuses.

use artifacts::compile_timings::{CompileTimings, StageMeta};
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage};

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
        if stage == PsiCompilationStage::Publication {
            artifacts::compile_timings::counters::record_terminal_production();
        }
        let (name, input, output) = stage.observation_labels();
        self.timings
            .record_result(StageMeta::new(name, input, output), operation)
    }
}
