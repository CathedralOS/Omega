pub use omega_typed_trees::{
    data, expression, identity, invariant, machine, name, platform, signature, state,
    trait_definition, types,
};

use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

pub mod statement {
    pub use omega_typed_trees::statement::*;

    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionGuard {
        Always,
        When(crate::expression::Expression),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionTarget {
        Named {
            path: crate::expression::NamePath,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
            arguments: HandleSpan<crate::expression::Expression>,
        },
        Value(crate::expression::Expression),
        SelfTarget,
        Terminal,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowWritableRootFact {
    pub symbol: SymbolHandle,
    pub kind: BorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<BorrowCallFact>,
    pub loans: HandleSpan<BorrowLoanFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowLoanFact {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub access_segments: Arena<omega_facts::PlaceSegment>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub loans: Arena<BorrowLoanFact>,
    pub states: Arena<StateBorrowFact>,
}

impl BorrowFacts {
    pub fn access_segments(
        &self,
        access: &BorrowArgumentAccessFact,
    ) -> &[omega_facts::PlaceSegment] {
        self.access_segments.span_or_empty(access.segments)
    }

    pub fn accesses_overlap(
        &self,
        left: &BorrowArgumentAccessFact,
        right: &BorrowArgumentAccessFact,
    ) -> bool {
        left.root_symbol == right.root_symbol
            && place_segments_overlap(self.access_segments(left), self.access_segments(right))
    }

    pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[omega_facts::PlaceSegment] {
        self.access_segments.span_or_empty(loan.segments)
    }

    pub fn access_overlaps_loan(
        &self,
        access: &BorrowArgumentAccessFact,
        loan: &BorrowLoanFact,
    ) -> bool {
        access.root_symbol == loan.root_symbol
            && place_segments_overlap(self.access_segments(access), self.loan_segments(loan))
    }
}

fn place_segments_overlap(
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| left_segment == right_segment)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofFactKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner: ProofObligationOwner,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactKind {
    #[default]
    Requires,
    Ensures,
    Trusted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactOwner {
    #[default]
    Unknown,
    Machine {
        machine_symbol: SymbolHandle,
    },
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignature {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFact {
    pub kind: ContractProofFactKind,
    pub owner: ContractProofFactOwner,
    pub fact: Handle<omega_typed_trees::domain::ProofFact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFactRef {
    pub fact: Handle<ContractProofFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractCallFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofObligationOwner {
    #[default]
    Unknown,
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    MachineOwnedData {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    StateReturn {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    TransitionParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
}

impl Default for ProofObligationFact {
    fn default() -> Self {
        Self {
            kind: ProofFactKind::default(),
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
            owner: ProofObligationOwner::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_exits: Arena<ContractExitFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: name::Identifier,
    pub constraint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFacts {
    pub definitions: Arena<InvariantFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyPathFact {
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDependencyFact {
    pub domain_symbol: SymbolHandle,
    pub dependencies: HandleSpan<DomainDependencyPathFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainFacts {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub dependency_paths: Arena<DomainDependencyPathFact>,
    pub dependencies: Arena<DomainDependencyFact>,
}

impl DomainFacts {
    pub fn dependency_fact(&self, domain_symbol: SymbolHandle) -> Option<&DomainDependencyFact> {
        self.dependencies
            .iter()
            .find_map(|(_, fact)| (fact.domain_symbol == domain_symbol).then_some(fact))
    }

    pub fn dependency_paths<'a>(
        &'a self,
        dependency: &'a DomainDependencyFact,
    ) -> impl Iterator<Item = &'a [omega_facts::PlaceSegment]> + 'a {
        self.dependency_paths
            .span_or_empty(dependency.dependencies)
            .iter()
            .map(|path| self.segments.span_or_empty(path.segments))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowSemanticContextRef {
    pub context: omega_facts::FactContextHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowConstraintKind {
    #[default]
    Unknown,
    SemanticContext {
        context: omega_facts::FactContextHandle,
    },
    BorrowState {
        state: Handle<StateBorrowFact>,
    },
    BorrowCall {
        call: Handle<BorrowCallFact>,
    },
    BorrowWritableRoot {
        root: Handle<BorrowWritableRootFact>,
    },
    BorrowAccess {
        access: Handle<BorrowArgumentAccessFact>,
    },
    BorrowLoan {
        loan: Handle<BorrowLoanFact>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowConstraintRef {
    pub kind: FlowConstraintKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowInvalidationSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for FlowInvalidationSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInvalidationFact {
    pub source: FlowInvalidationSource,
    pub context: omega_facts::FactContextHandle,
    pub fact: omega_facts::FactHandle,
    pub mutated_root: omega_facts::PlaceRoot,
    pub mutated_segments: HandleSpan<omega_facts::PlaceSegment>,
    pub dependency_segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowWeakeningFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
    pub reason: FlowBorrowWeakeningReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub requires_contexts: HandleSpan<FlowSemanticContextRef>,
    pub requires_constraints: HandleSpan<FlowConstraintRef>,
    pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub exit_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub direct_effects: omega_effects::EffectSet,
    pub transitive_effects: omega_effects::EffectSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
    pub ensures_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub borrow_weakenings: HandleSpan<FlowBorrowWeakeningFact>,
    pub calls: HandleSpan<FlowCallFact>,
    pub exits: HandleSpan<FlowExitFact>,
    pub direct_effects: omega_effects::EffectSet,
    pub transitive_effects: omega_effects::EffectSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowFacts {
    pub semantic_context_refs: Arena<FlowSemanticContextRef>,
    pub constraint_refs: Arena<FlowConstraintRef>,
    pub invalidation_segments: Arena<omega_facts::PlaceSegment>,
    pub invalidations: Arena<FlowInvalidationFact>,
    pub borrow_weakenings: Arena<FlowBorrowWeakeningFact>,
    pub calls: Arena<FlowCallFact>,
    pub exits: Arena<FlowExitFact>,
    pub states: Arena<FlowStateFact>,
}

impl FlowFacts {
    pub fn constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> &[FlowConstraintRef] {
        self.constraint_refs.span_or_empty(constraints)
    }

    pub fn semantic_constraint_contexts<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = omega_facts::FactContextHandle> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::SemanticContext { context } => Some(context),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_state_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<StateBorrowFact>> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowState { state } => Some(state),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_call_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowCallFact>> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowCall { call } => Some(call),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_writable_root_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowWritableRoot { root } => Some(root),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_access_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowArgumentAccessFact>> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowAccess { access } => Some(access),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_loan_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + 'a {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowLoan { loan } => Some(loan),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. } => None,
            })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: omega_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub invariants: InvariantFacts,
    pub domains: DomainFacts,
    pub effects: omega_effects::EffectPlan,
    pub flow: FlowFacts,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl std::ops::Deref for CheckedTrees {
    type Target = omega_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<omega_typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &omega_typed_trees::TypedTrees {
        &self.typed
    }
}
