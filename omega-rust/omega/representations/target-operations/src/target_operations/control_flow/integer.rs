//! Integer result control and explicit conditional arms.

use crate::{ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression};
use semantic_vocabulary::{ClaimId, EdgeId, ValueId};
use terminal_psi::{CrashCause, CrashPredicateTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
}
