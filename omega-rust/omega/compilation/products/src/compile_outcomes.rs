// Ordered target results and compiler-request outcome classification.
// A refusal is `Incomplete` only when every error names an exceeded private
// provision; one ordinary error makes the refusal a `Reject` so an unfinished
// search never softens an established violation. Pending restricted Build
// requests remain structured on `CompileFailure::BuildConsentRequired` so
// package review can attribute and render them without parsing diagnostic prose.

use crate::CompileReport;
use diagnostics::{Diagnostic, ExceededProvision};
use target::TargetProfile;

#[derive(Debug, Clone)]
pub enum CompileFailure {
    Diagnostics(Vec<Diagnostic>),
    /// Actual source invocations ran before preparation, judgment or target
    /// admission failed. Keep their records alongside the diagnostic cause.
    SourceCompletion {
        diagnostics: Vec<Diagnostic>,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    },
    BuildConsentRequired(Vec<package_compilation::build_execution::RestrictedBuildRequest>),
}

impl CompileFailure {
    pub fn source_completion(
        diagnostics: Vec<Diagnostic>,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        if accounting.receipts().is_empty() {
            Self::Diagnostics(diagnostics)
        } else {
            Self::SourceCompletion {
                diagnostics,
                accounting,
            }
        }
    }

    pub fn source_completion_accounting(
        &self,
    ) -> Option<&build_time_evaluation::SourceCompletionAccounting> {
        match self {
            Self::SourceCompletion { accounting, .. } => Some(accounting),
            _ => None,
        }
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) | Self::SourceCompletion { diagnostics, .. } => diagnostics,
            Self::BuildConsentRequired(requests) => requests
                .into_iter()
                .map(|request| {
                    Diagnostic::error(format!(
                        "restricted Build execution requires explicit consent; generated-source and final product findings are unavailable; pending request: {request:?}"
                    ))
                })
                .collect(),
        }
    }
}

impl From<psi_compiler::SourcePreparationFailure> for CompileFailure {
    fn from(failure: psi_compiler::SourcePreparationFailure) -> Self {
        Self::source_completion(failure.diagnostics, failure.accounting)
    }
}

impl From<Vec<Diagnostic>> for CompileFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerRequestOutcome {
    Complete,
    Reject,
    Incomplete(ExceededProvision),
}

impl CompilerRequestOutcome {
    pub fn of_refusal(diagnostics: &[Diagnostic]) -> Self {
        let mut errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error());
        let Some(first) = errors.next().and_then(|error| error.exceeded_provision) else {
            return Self::Reject;
        };
        if errors.all(|error| error.exceeded_provision.is_some()) {
            Self::Incomplete(first)
        } else {
            Self::Reject
        }
    }

    pub const fn halt_value(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::Reject => 1,
            Self::Incomplete(_) => 2,
        }
    }
}

#[must_use]
pub struct CheckedCompileOutcome {
    pub result: Result<crate::CheckedCompilation, CompileFailure>,
    pub independent_components: crate::IndependentComponentDiscovery,
}

#[derive(Debug)]
pub struct CompileTargetOutcome {
    target: Option<TargetProfile>,
    result: Result<CompileReport, Vec<Diagnostic>>,
    failed_source_completion_accounting: Option<build_time_evaluation::SourceCompletionAccounting>,
}

impl CompileTargetOutcome {
    pub fn new(
        target: Option<TargetProfile>,
        result: Result<CompileReport, Vec<Diagnostic>>,
    ) -> Self {
        Self {
            target,
            result,
            failed_source_completion_accounting: None,
        }
    }

    pub fn with_failed_source_completion_accounting(
        mut self,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.failed_source_completion_accounting = Some(accounting);
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> Option<&build_time_evaluation::SourceCompletionAccounting> {
        match &self.result {
            Ok(report) => Some(report.source_completion_accounting()),
            Err(_) => self.failed_source_completion_accounting.as_ref(),
        }
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

    pub fn outcome(&self) -> CompilerRequestOutcome {
        match &self.result {
            Ok(_) => CompilerRequestOutcome::Complete,
            Err(diagnostics) => CompilerRequestOutcome::of_refusal(diagnostics),
        }
    }

    pub fn into_result(self) -> Result<CompileReport, Vec<Diagnostic>> {
        self.result
    }
}

#[derive(Debug)]
pub struct CompileOutcomes {
    outcomes: Box<[CompileTargetOutcome]>,
}

impl CompileOutcomes {
    pub fn new(outcomes: Vec<CompileTargetOutcome>) -> Self {
        Self {
            outcomes: outcomes.into_boxed_slice(),
        }
    }

    pub fn outcomes(&self) -> &[CompileTargetOutcome] {
        &self.outcomes
    }

    pub fn into_outcomes(self) -> Box<[CompileTargetOutcome]> {
        self.outcomes
    }

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
}
