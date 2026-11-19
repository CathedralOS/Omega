//! Remaining source evaluation accounting adapters.
use crate::evaluation::BuildTimeValue;
use build_scalar_evaluation::PURE_PSI_WORK_BUDGET;
pub use source_completion_accounting::{
    CanonicalScalarEvaluationReceipt, PureEvaluationRecord, PureTerminalEvaluationOutcome,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};

pub struct RecordedPreCheck {
    pub pending: Option<crate::evaluation::PreCheckEvaluation>,
    pub accounting: SourceCompletionAccounting,
}
