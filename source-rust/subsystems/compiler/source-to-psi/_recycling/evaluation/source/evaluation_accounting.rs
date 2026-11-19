//! Remaining source evaluation accounting adapters.
use crate::evaluation::BuildTimeValue;
use build_scalar_evaluation::PURE_PSI_WORK_BUDGET;
pub use source_completion_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};

/// A build-time value with the engine-tagged usage record of the evaluation
/// that produced it. Both engines publish `BuildEvaluationUsage`, whose
/// measurement identity names the work and storage schedules, so consumers
/// such as evaluated binding digests never interpret one engine's counts as
/// another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    value: T,
    usage: evaluation_contracts::BuildEvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub(crate) fn new(value: T, usage: evaluation_contracts::BuildEvaluationUsage) -> Self {
        Self { value, usage }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> evaluation_contracts::BuildEvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, evaluation_contracts::BuildEvaluationUsage) {
        (self.value, self.usage)
    }
}


pub struct RecordedPreCheck {
    pub pending: Option<crate::evaluation::PreCheckEvaluation>,
    pub accounting: SourceCompletionAccounting,
}

pub(crate) fn pure_terminal_invocation_account()
-> Result<evaluation_accounting::EvaluationInvocationAccount, String> {
    let limits = evaluation_contracts::BuildEvaluationSponsorLimits::new(
        PURE_PSI_WORK_BUDGET,
        1,
        1,
        1,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    )
    .ok_or("pure invocation limits are invalid")?;
    let measurement = evaluation_contracts::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_contracts::TerminalStorageScheduleIdentity::CURRENT,
    };
    evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(limits, measurement)
        .map_err(|error| format!("pure invocation sponsor: {error:?}"))?
        .begin_invocation(measurement, PURE_PSI_WORK_BUDGET)
        .map_err(|error| format!("pure invocation account: {error:?}"))
}
