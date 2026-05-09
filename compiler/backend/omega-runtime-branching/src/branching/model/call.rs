use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::StateGuardKind;
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCall {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub target_key: StateKey,
    pub statement_index: usize,
    pub argument_count: usize,
    pub expansion: RuntimeBranchCallExpansion,
    pub edges: HandleSpan<RuntimeBranchingCallEdge>,
}

impl Default for RuntimeBranchingCall {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            target_key: StateKey::default(),
            statement_index: 0,
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
