//! Boolean result control and explicit conditional arms.

use crate::{ScalarParameterLocation, TargetBooleanExpression};
use psi_core::{ClaimId, EdgeId, ValueId};
use psi_terminal::{CrashCause, CrashPredicateTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnImmediate {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}
