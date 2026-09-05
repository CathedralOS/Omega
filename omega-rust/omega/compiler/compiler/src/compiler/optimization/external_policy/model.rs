use bounded_process::BoundedProcessRunError;
use optimization_core::{ExternalDecisionLog, ExternalDecisionSchemaError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalPolicyFallback {
    FailClosed,
    UseRecordedBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalPolicySurfaceMismatch {
    Context,
    PointCount,
    PointInput { ordinal: usize },
    PointRule { ordinal: usize },
    CandidateSurface { ordinal: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExternalPolicyExecutionFailure {
    InvalidLimits,
    RequestOverflow {
        limit: usize,
        observed: usize,
    },
    Process(BoundedProcessRunError),
    UnsuccessfulExit {
        code: Option<i32>,
        unix_signal: Option<i32>,
        stderr: Vec<u8>,
    },
    ResponseSchema(ExternalDecisionSchemaError),
    SurfaceMismatch(ExternalPolicySurfaceMismatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalPolicyExecutionError(pub(crate) ExternalPolicyExecutionFailure);

impl std::fmt::Display for ExternalPolicyExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "external optimization policy failed: {:?}",
            self.0
        )
    }
}

impl std::error::Error for ExternalPolicyExecutionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExternalPolicyResolution {
    ExternalResponse,
    RecordedBaselineFallback(ExternalPolicyExecutionFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalPolicyExecutionOutcome {
    decisions: ExternalDecisionLog,
    resolution: ExternalPolicyResolution,
}

impl ExternalPolicyExecutionOutcome {
    pub(super) fn external(decisions: ExternalDecisionLog) -> Self {
        Self {
            decisions,
            resolution: ExternalPolicyResolution::ExternalResponse,
        }
    }

    pub(super) fn baseline(
        decisions: ExternalDecisionLog,
        failure: ExternalPolicyExecutionFailure,
    ) -> Self {
        Self {
            decisions,
            resolution: ExternalPolicyResolution::RecordedBaselineFallback(failure),
        }
    }

    pub(crate) const fn decisions(&self) -> &ExternalDecisionLog {
        &self.decisions
    }

    pub(crate) const fn resolution(&self) -> &ExternalPolicyResolution {
        &self.resolution
    }
}
