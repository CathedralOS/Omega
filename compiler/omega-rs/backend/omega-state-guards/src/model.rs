use omega_control_flow::StateKey;
use omega_state_graph::RuntimeTransitionTarget;
use omega_target_operations::{StateGuardLowering, StateGuardOperator};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub expressions: ExpressionTable,
    pub guards: Arena<StateGuard>,
    pub operands: Arena<StateGuardOperand>,
}

impl StateGuardPlan {
    pub fn guard_for_source(
        &self,
        source: StateKey,
        statement_order: usize,
    ) -> Option<&StateGuard> {
        self.guards
            .iter()
            .find(|(_, guard)| guard.source == source && guard.statement_order == statement_order)
            .map(|(_, guard)| guard)
    }

    pub fn guard_for_dispatch(
        &self,
        source_dispatch_index: u32,
        statement_order: usize,
    ) -> Option<&StateGuard> {
        self.guards
            .iter()
            .find(|(_, guard)| {
                guard.source_dispatch_index == source_dispatch_index
                    && guard.statement_order == statement_order
            })
            .map(|(_, guard)| guard)
    }

    pub fn operand_span(
        &self,
        operands: HandleSpan<StateGuardOperand>,
    ) -> Option<&[StateGuardOperand]> {
        self.operands.span(operands)
    }
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
    pub statement_index: usize,
    pub kind: StateGuardKind,
    pub operator: StateGuardOperator,
    pub lowering: StateGuardLowering,
    pub expression: ExpressionHandle,
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
            statement_index: 0,
            kind: StateGuardKind::Always,
            operator: StateGuardOperator::None,
            lowering: StateGuardLowering::NoOp,
            expression: ExpressionHandle::invalid(),
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
    pub expression: ExpressionHandle,
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
            expression: ExpressionHandle::invalid(),
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
    RuntimeFrame,
    #[default]
    Unknown,
}
