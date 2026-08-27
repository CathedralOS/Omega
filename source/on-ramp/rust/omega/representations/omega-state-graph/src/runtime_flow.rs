mod builder;

use omega_control_flow::{StateKey, TransitionExpressionRefs};
use psi_arena::{Arena, HandleSpan};

pub use builder::{build_runtime_flow_plan, build_runtime_flow_plan_with_state_calls};

/// Identifies which specialized COPY of a source state a runtime node belongs to.
///
/// The control-flow plan is canonical: one state per source state. The runtime
/// flow may CLONE a called machine's states once per call-context so each call
/// site gets its own statically-wired continuation (specialization "B"). A clone
/// shares the canonical `StateKey` (so its operations/parameters/frame layout
/// resolve normally) but carries a distinct `CallContext` so edges, dispatch
/// indices, and labels can tell the copies apart. `ROOT` is the entry machine and
/// any un-cloned state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallContext(pub u32);

impl CallContext {
    pub const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFlowPlan {
    pub states: Arena<RuntimeState>,
    pub edges: Arena<RuntimeEdge>,
    pub cycles: Arena<RuntimeCycle>,
    pub cycle_states: Arena<RuntimeState>,
    /// Per-context MINTING CALL SITE, indexed by `CallContext.0`: the caller
    /// state + statement whose dispatched call minted the context, plus the
    /// CONTEXT THE CALLER RAN IN (its parent -- always minted earlier, so a
    /// forward scan can compose child bases from finished parents). Entry 0
    /// (ROOT) is the invalid key. This is the receiver-identity thread for
    /// per-instance dispatch (TASKS_FS "Stolen work #2"): downstream
    /// consumers look up the minting StateCall's receiver path and resolve
    /// the callee's TRUE storage base -- composed through the parent chain
    /// for non-entry callers -- instead of the first-type-match walk. Each
    /// tuple is `(source, statement, call ordinal, parent context)`.
    pub context_call_sites: Vec<(StateKey, usize, usize, CallContext)>,
}

impl RuntimeFlowPlan {
    pub fn with_capacity(
        state_capacity: usize,
        edge_capacity: usize,
        cycle_capacity: usize,
        cycle_state_capacity: usize,
    ) -> Self {
        Self {
            states: Arena::with_capacity(state_capacity),
            edges: Arena::with_capacity(edge_capacity),
            cycles: Arena::with_capacity(cycle_capacity),
            cycle_states: Arena::with_capacity(cycle_state_capacity),
            // Index 0 == ROOT: no minting call (self-parented placeholder).
            context_call_sites: vec![(
                StateKey::default(),
                usize::MAX,
                usize::MAX,
                CallContext::ROOT,
            )],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeState {
    pub key: StateKey,
    pub context: CallContext,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCycle {
    pub states: HandleSpan<RuntimeState>,
}

/// Identifies the caller's call-result slot a dispatched value call returns into.
/// Carried on a callee clone's TERMINAL edge so that, when the clone returns, the
/// terminal's value (`-> acc` / `-> 99`) is written back to the caller's `let n`
/// slot keyed by `(call_source_key, statement_index, call_ordinal)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallResultReturn {
    pub call_source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEdge {
    pub from: StateKey,
    /// The call-context copy of `from` this edge belongs to. Distinguishes edges
    /// of different clones of the same source state.
    pub from_context: CallContext,
    pub statement_index: usize,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub expressions: TransitionExpressionRefs,
    pub forms_cycle: bool,
    /// Set on a value-returning callee clone's terminal edge: where to write the
    /// terminal value (the caller's call-result slot). `None` for ordinary edges.
    pub call_result: Option<CallResultReturn>,
}

impl Default for RuntimeEdge {
    fn default() -> Self {
        Self {
            from: StateKey::default(),
            from_context: CallContext::ROOT,
            statement_index: 0,
            target: RuntimeTransitionTarget::None,
            continuation: RuntimeTransitionTarget::None,
            expressions: TransitionExpressionRefs::default(),
            forms_cycle: false,
            call_result: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeStateCallEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_key: StateKey,
    /// Whether the call is VALUE-position (role != Statement in the state-call
    /// plan: AssignmentValue / CallArgument / TransitionArgument /
    /// TransitionGuard). Authoritative from the plan -- the builder's old
    /// operation-sniffing heuristic missed TRANSITION-embedded calls
    /// (`true -> check(self.count(..))` has no Assignment/Expression op at the
    /// statement), so their clone terminals carried no CallResultReturn and the
    /// caller read a never-written slot.
    pub is_value: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTransitionTarget {
    State { key: StateKey, context: CallContext },
    Terminal,
    None,
    Unknown,
}

impl RuntimeTransitionTarget {
    /// A target referring to the canonical (un-cloned) copy of `key`.
    pub fn root_state(key: StateKey) -> Self {
        Self::State {
            key,
            context: CallContext::ROOT,
        }
    }
}

impl Default for RuntimeTransitionTarget {
    fn default() -> Self {
        Self::None
    }
}
