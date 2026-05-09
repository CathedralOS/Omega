use crate::runtime_flow::RuntimeTransitionTarget;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target_program::{StateGuardLowering, StateGuardOperator};
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub guards: Arena<StateGuard>,
    pub operands: Arena<StateGuardOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuard {
    pub source: StateKey,
    pub source_dispatch_index: u32,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub statement_order: usize,
    pub kind: StateGuardKind,
    pub operator: StateGuardOperator,
    pub lowering: StateGuardLowering,
    pub expression: Expression,
    pub operands: HandleSpan<StateGuardOperand>,
    pub has_expression: bool,
    pub forms_cycle: bool,
}

impl Default for StateGuard {
    fn default() -> Self {
        Self {
            source: StateKey::default(),
            source_dispatch_index: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            statement_order: 0,
            kind: StateGuardKind::Always,
            operator: StateGuardOperator::None,
            lowering: StateGuardLowering::NoOp,
            expression: Expression::Boolean(true),
            operands: HandleSpan::empty(),
            has_expression: false,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardKind {
    #[default]
    Always,
    RuntimeEquality,
    RuntimeInequality,
    RuntimeOrdering,
    RuntimeExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuardOperand {
    pub expression: Expression,
    pub kind: StateGuardOperandKind,
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub resolved_value: i64,
    pub has_resolved_value: bool,
}

impl Default for StateGuardOperand {
    fn default() -> Self {
        Self {
            expression: Expression::Boolean(true),
            kind: StateGuardOperandKind::OtherExpression,
            storage: StateGuardOperandStorage::Unknown,
            byte_offset: 0,
            byte_size: 0,
            resolved_value: 0,
            has_resolved_value: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperandKind {
    Place,
    StaticSymbol,
    Literal,
    #[default]
    OtherExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperandStorage {
    MachineOwned,
    #[default]
    Unknown,
}
