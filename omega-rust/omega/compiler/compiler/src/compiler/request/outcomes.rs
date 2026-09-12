use crate::compiler::CompileReport;
use diagnostics::Diagnostic;
use target::TargetProfile;

/// An ordinary target product or its diagnostics. None identifies target-neutral
/// checking/Terminal production, never an implicit native target.
#[derive(Debug)]
pub struct CompileTargetOutcome {
    target: Option<TargetProfile>,
    result: Result<CompileReport, Vec<Diagnostic>>,
}

impl CompileTargetOutcome {
    pub(in crate::compiler) fn new(
        target: Option<TargetProfile>,
        result: Result<CompileReport, Vec<Diagnostic>>,
    ) -> Self {
        Self { target, result }
    }
    pub const fn target_profile(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn report(&self) -> Option<&CompileReport> {
        self.result.as_ref().ok()
    }
    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        self.result.as_ref().err().map(Vec::as_slice)
    }
    pub const fn succeeded(&self) -> bool {
        self.result.is_ok()
    }
    pub fn into_result(self) -> Result<CompileReport, Vec<Diagnostic>> {
        self.result
    }
}

/// One outcome per configuration, in canonical target order, including failures.
/// This collection grants no batch evidence, deployment or platform-support claim.
#[derive(Debug)]
pub struct CompileOutcomes {
    outcomes: Box<[CompileTargetOutcome]>,
    #[cfg(test)]
    prepared_terminal_native_input_count: usize,
}

impl CompileOutcomes {
    pub(in crate::compiler) fn new(outcomes: Vec<CompileTargetOutcome>) -> Self {
        Self {
            outcomes: outcomes.into_boxed_slice(),
            #[cfg(test)]
            prepared_terminal_native_input_count: 0,
        }
    }
    pub fn outcomes(&self) -> &[CompileTargetOutcome] {
        &self.outcomes
    }
    pub fn into_outcomes(self) -> Box<[CompileTargetOutcome]> {
        self.outcomes
    }

    /// Explicit extraction for consumers that requested exactly one product.
    /// Never discard a sibling failure or silently select the first target.
    pub fn into_single_report(self) -> Result<CompileReport, Vec<Diagnostic>> {
        if self.outcomes.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "expected one compilation outcome, received {}",
                self.outcomes.len()
            ))]);
        }
        self.outcomes
            .into_vec()
            .pop()
            .ok_or_else(|| vec![Diagnostic::error("compilation outcome is missing")])?
            .into_result()
    }
    pub(in crate::compiler) fn with_prepared_terminal_native_input_count(
        self,
        count: usize,
    ) -> Self {
        #[cfg(test)]
        {
            let mut outcomes = self;
            outcomes.prepared_terminal_native_input_count = count;
            outcomes
        }
        #[cfg(not(test))]
        {
            let _ = count;
            self
        }
    }
    #[cfg(test)]
    pub(in crate::compiler) const fn prepared_terminal_native_input_count(&self) -> usize {
        self.prepared_terminal_native_input_count
    }
}
