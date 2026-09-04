//! control flow integer in the assigned operations program.

use crate::AssignedBooleanExpression;
use crate::AssignedIntegerExpression;
use crate::AssignedScalarLocation;
use crate::ExpressionFrame;
use psi_core::ClaimId;
use psi_core::EdgeId;
use psi_core::ValueId;
use psi_terminal::CrashCause;
use psi_terminal::CrashPredicateTerm;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
}
