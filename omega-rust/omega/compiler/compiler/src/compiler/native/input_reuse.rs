//! Invocation-local reuse of exactly matching Terminal inputs.

use super::prepared::{NativeInputReuseKey, PreparedNativeCompilation};
use crate::CompileReport;
use diagnostics::Diagnostic;

#[derive(Default)]
pub(in crate::compiler) struct NativeInputReuse {
    inputs: Vec<(
        NativeInputReuseKey,
        Result<native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>>,
    )>,
}

impl NativeInputReuse {
    /// Prepare each exact input once, including failed preparations. Physical
    /// realization and all target-specific authority remain with the caller's
    /// own prepared compilation.
    pub(in crate::compiler) fn realize(
        &mut self,
        compilation: PreparedNativeCompilation,
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
                self.inputs
                    .push((key, compilation.prepare_reusable_input()));
                input_index
            }
        };
        match &self.inputs[input_index].1 {
            Ok(input) => compilation.finish(input),
            Err(diagnostics) => Err(diagnostics.clone()),
        }
    }

    pub(in crate::compiler) fn prepared_input_count(&self) -> usize {
        self.inputs.len()
    }
}
