//! Invocation-local reuse of exactly matching Terminal inputs.

use super::prepared::{NativeInputReuseKey, PreparedNativeCompilation};
use artifacts::compile_timings::{StageMeta, TimingCategory};
use compilation_report::CompileReport;
use diagnostics::Diagnostic;

/// The shared Terminal-input preparation each exact input pays once: the
/// artifact's lowering and physical planning legs run inside this row until
/// the native realization internals own their own stage rows.
const NATIVE_INPUT_PREPARATION_STAGE: StageMeta = StageMeta::new(
    "native-input-preparation",
    "TerminalArtifact",
    "PreparedNativeRealizationInput",
    TimingCategory::Pipeline,
);

/// Prepared native realization inputs shared by every target of one
/// invocation whose exact Terminal identity, admission profile and physical
/// selections agree.
#[derive(Default)]
pub struct NativeInputReuse {
    inputs: Vec<(
        NativeInputReuseKey,
        Result<crate::PreparedNativeRealizationInput, Vec<Diagnostic>>,
    )>,
}

impl NativeInputReuse {
    /// Prepare each exact input once, including failed preparations. Physical
    /// realization and all target-specific authority remain with the caller's
    /// own prepared compilation.
    pub fn realize(
        &mut self,
        mut compilation: PreparedNativeCompilation,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let key = compilation.reuse_key();
        let input_index = match self
            .inputs
            .iter()
            .position(|(existing, _)| *existing == key)
        {
            Some(input_index) => input_index,
            None => {
                let input_index = self.inputs.len();
                let mut stage_timings = std::mem::take(compilation.checked.timings_mut());
                let prepared = stage_timings.record_result(NATIVE_INPUT_PREPARATION_STAGE, || {
                    compilation.prepare_reusable_input()
                });
                *compilation.checked.timings_mut() = stage_timings;
                self.inputs.push((key, prepared));
                input_index
            }
        };
        match &self.inputs[input_index].1 {
            Ok(input) => compilation.finish(input),
            Err(diagnostics) => Err(diagnostics.clone()),
        }
    }

    pub fn prepared_input_count(&self) -> usize {
        self.inputs.len()
    }
}
