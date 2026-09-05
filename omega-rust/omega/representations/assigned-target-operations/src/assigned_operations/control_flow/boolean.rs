//! control flow boolean in the assigned operations program.

use crate::AssignedBooleanExpression;
use crate::AssignedScalarLocation;
use crate::ExpressionFrame;
use semantic_vocabulary::ClaimId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::ValueId;
use terminal_psi::CrashCause;
use terminal_psi::CrashPredicateTerm;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBooleanControl {
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
        location: AssignedScalarLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
}
