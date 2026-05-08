use crate::control_flow::StateKey;
use crate::runtime_dispatch::guards::StateGuardKind;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_calls::StateCallLowering;
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
    pub target_arguments: Arena<Expression>,
    pub leaf_expansions: Arena<RuntimeLeafBranchExpansion>,
    pub leaf_operations: Arena<RuntimeLeafBranchOperation>,
    pub leaf_bindings: Arena<RuntimeLeafBranchBinding>,
    pub straight_line_expansions: Arena<RuntimeStraightLineBranchExpansion>,
    pub straight_line_operations: Arena<RuntimeStraightLineBranchOperation>,
    pub straight_line_bindings: Arena<RuntimeStraightLineBranchBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCall {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub target_key: StateKey,
    pub statement_index: usize,
    pub target_machine: ProgramName,
    pub target_state: ProgramName,
    pub argument_count: usize,
    pub expansion: RuntimeBranchCallExpansion,
    pub edges: HandleSpan<RuntimeBranchingCallEdge>,
}

impl Default for RuntimeBranchingCall {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            target_key: StateKey::default(),
            statement_index: 0,
            target_machine: ProgramName::default(),
            target_state: ProgramName::default(),
            argument_count: 0,
            expansion: RuntimeBranchCallExpansion::Unplanned,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCallEdge {
    pub order: usize,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub target_arguments: HandleSpan<Expression>,
    pub guard_kind: StateGuardKind,
    pub lowering: RuntimeBranchTargetLowering,
}

impl Default for RuntimeBranchingCallEdge {
    fn default() -> Self {
        Self {
            order: 0,
            target: RuntimeTransitionTarget::None,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            target_arguments: HandleSpan::empty(),
            guard_kind: StateGuardKind::Always,
            lowering: RuntimeBranchTargetLowering::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeBranchTargetLowering {
    Terminal,
    InlineLeaf,
    InlineStraightLine,
    InlineBranching,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeBranchCallExpansion {
    GuardedLeaf,
    GuardedLeafWithComplexGuards,
    NeedsStraightLineTarget,
    NeedsNestedBranchTarget,
    UnknownTarget,
    #[default]
    Unplanned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub branch_machine: ProgramName,
    pub branch_state: ProgramName,
    pub edge_order: usize,
    pub guard: TransitionGuard,
    pub resolved_guard: TransitionGuard,
    pub guard_kind: StateGuardKind,
    pub leaf_machine: ProgramName,
    pub leaf_state: ProgramName,
    pub bindings: HandleSpan<RuntimeLeafBranchBinding>,
    pub operations: HandleSpan<RuntimeLeafBranchOperation>,
}

impl Default for RuntimeLeafBranchExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            branch_machine: ProgramName::default(),
            branch_state: ProgramName::default(),
            edge_order: 0,
            guard: TransitionGuard::Always,
            resolved_guard: TransitionGuard::Always,
            guard_kind: StateGuardKind::Always,
            leaf_machine: ProgramName::default(),
            leaf_state: ProgramName::default(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchBinding {
    pub parameter_name: ProgramName,
    pub expression: Expression,
    pub kind: RuntimeLeafBranchBindingKind,
}

impl Default for RuntimeLeafBranchBinding {
    fn default() -> Self {
        Self {
            parameter_name: ProgramName::default(),
            expression: Expression::Integer(0),
            kind: RuntimeLeafBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchBindingKind {
    #[default]
    BranchParameter,
    LeafParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchOperation {
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub kind: RuntimeLeafBranchOperationKind,
}

impl Default for RuntimeLeafBranchOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            kind: RuntimeLeafBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchOperationKind {
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: Expression,
        value: Expression,
    },
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub branch_machine: ProgramName,
    pub branch_state: ProgramName,
    pub edge_order: usize,
    pub guard: TransitionGuard,
    pub resolved_guard: TransitionGuard,
    pub guard_kind: StateGuardKind,
    pub target_machine: ProgramName,
    pub target_state: ProgramName,
    pub bindings: HandleSpan<RuntimeStraightLineBranchBinding>,
    pub operations: HandleSpan<RuntimeStraightLineBranchOperation>,
}

impl Default for RuntimeStraightLineBranchExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            branch_machine: ProgramName::default(),
            branch_state: ProgramName::default(),
            edge_order: 0,
            guard: TransitionGuard::Always,
            resolved_guard: TransitionGuard::Always,
            guard_kind: StateGuardKind::Always,
            target_machine: ProgramName::default(),
            target_state: ProgramName::default(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchBinding {
    pub parameter_name: ProgramName,
    pub expression: Expression,
    pub kind: RuntimeStraightLineBranchBindingKind,
}

impl Default for RuntimeStraightLineBranchBinding {
    fn default() -> Self {
        Self {
            parameter_name: ProgramName::default(),
            expression: Expression::Integer(0),
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchBindingKind {
    #[default]
    BranchParameter,
    TargetParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchOperation {
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub kind: RuntimeStraightLineBranchOperationKind,
}

impl Default for RuntimeStraightLineBranchOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            kind: RuntimeStraightLineBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchOperationKind {
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: Expression,
        value: Expression,
    },
    StateCall {
        target_key: StateKey,
        target_machine: ProgramName,
        target_state: ProgramName,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalData,
    #[default]
    Other,
}
