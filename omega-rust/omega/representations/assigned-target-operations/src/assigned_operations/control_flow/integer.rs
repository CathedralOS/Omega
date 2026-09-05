//! control flow integer in the assigned operations program.

use crate::AssignedBooleanExpression;
use crate::AssignedIntegerExpression;
use crate::AssignedScalarLocation;
use crate::ExpressionFrame;
use semantic_vocabulary::ClaimId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::ValueId;
use terminal_psi::CrashCause;
use terminal_psi::CrashPredicateTerm;

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
