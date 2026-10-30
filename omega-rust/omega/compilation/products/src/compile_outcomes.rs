//! Ordered target results, without compilation authority.

use crate::CompileReport;
use diagnostics::{Diagnostic, ExceededProvision};
use target::TargetProfile;

/// Why a compilation stopped. A pending Build request is not a checked result:
/// no generated-source or final product findings exist until consent joins and
/// the exact activation is run. Keep the request structured so package review
/// can attribute and render its normalized meaning without parsing diagnostics.
#[derive(Debug)]
pub enum CompileFailure {
    Diagnostics(Vec<Diagnostic>),
    BuildConsentRequired(Vec<package_compilation::build_execution::RestrictedBuildRequest>),
}

impl CompileFailure {
    /// Diagnostic-only callers can report the stop, but cannot turn it into a
    /// review decision. Review consumers retain the structured request instead.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
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

impl From<Vec<Diagnostic>> for CompileFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

/// The [compiler request](../../../../../wiki/spec/build/compiler_request.md#validation-and-failures)
/// outcome one compilation publishes; its halt value is the process exit
/// status a compiler command reports.
///
/// A refusal's diagnostics decide its class. It is `Incomplete` only when every
/// error names an exceeded private provision — a resource or search bound
/// declined, with no verdict on the source — and it names the first such
/// provision in diagnostic order. One ordinary error makes the refusal a
/// `Reject`: an established violation is a verdict an unfinished search
/// cannot soften, and missing evidence never authorizes acceptance. The
/// reference compiler reports its own malfunctions as rejections or panics,
/// so it publishes no `InternalFailure` (halt value 3) through this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerRequestOutcome {
    Complete,
    Reject,
    Incomplete(ExceededProvision),
}

impl CompilerRequestOutcome {
    /// Classify a failed compilation from its diagnostics. A refusal with no
    /// error diagnostic names no provision and rejects.
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

    /// The contract's halt tag, which is also the process exit status.
    pub const fn halt_value(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::Reject => 1,
            Self::Incomplete(_) => 2,
        }
    }
}

/// One checked attempt retains observations even when later settlement rejects.
/// Rejected attempts publish no reusable preparation or output artifact.
#[must_use]
pub struct CheckedCompileOutcome {
    pub result: Result<crate::CheckedCompilation, CompileFailure>,
    pub independent_components: crate::IndependentComponentDiscovery,
}

/// An ordinary target product or its diagnostics. None identifies target-neutral
/// checking/Terminal production, never an implicit native target.
#[derive(Debug)]
pub struct CompileTargetOutcome {
    target: Option<TargetProfile>,
    result: Result<CompileReport, Vec<Diagnostic>>,
    failed_source_completion_accounting: Option<build_time_evaluation::SourceCompletionAccounting>,
}

impl CompileTargetOutcome {
    /// Pair an observed result with its requested target. This is a result
    /// record, not evidence that compilation ran or that the target is supported.
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

    /// Preserve the actual evaluation work of a refused attempt. This ledger
    /// grants no artifact or successful checking result.
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
    /// The compiler-request outcome this target's result publishes.
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

/// Outcomes in the order supplied by the coordinator, including failures.
/// The collection grants no deployment, platform-support, test, or audit
/// claim.
#[derive(Debug)]
pub struct CompileOutcomes {
    outcomes: Box<[CompileTargetOutcome]>,
}

impl CompileOutcomes {
    /// Retain all supplied results in the coordinator's order; failed
    /// children are kept and missing target results are not invented.
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
}
