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
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub states: Arena<StateBorrowFact>,
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
    pub name: name::ProgramName,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub requires_contexts: HandleSpan<FlowSemanticContextRef>,
    pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
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
    pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub calls: HandleSpan<FlowCallFact>,
    pub exits: HandleSpan<FlowExitFact>,
    pub direct_effects: omega_effects::EffectSet,
    pub transitive_effects: omega_effects::EffectSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowFacts {
    pub semantic_context_refs: Arena<FlowSemanticContextRef>,
    pub calls: Arena<FlowCallFact>,
    pub exits: Arena<FlowExitFact>,
    pub states: Arena<FlowStateFact>,
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
pub struct Program {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl std::ops::Deref for Program {
    type Target = omega_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<omega_typed_trees::TypedTrees> for Program {
    fn as_ref(&self) -> &omega_typed_trees::TypedTrees {
        &self.typed
    }
}
