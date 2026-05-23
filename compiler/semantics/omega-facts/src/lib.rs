use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

pub type FactHandle = Handle<Fact>;
pub type FactRefHandle = Handle<FactRef>;
pub type FactContextHandle = Handle<FactContext>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactPlace {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramPoint {
    #[default]
    Global,
    Definition {
        symbol: SymbolHandle,
    },
    Machine {
        machine_symbol: SymbolHandle,
    },
    State {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    Statement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
    Call {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    Exit {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactOrigin {
    #[default]
    Unknown,
    DomainDefinition {
        domain_symbol: SymbolHandle,
    },
    InvariantDefinition {
        invariant_symbol: SymbolHandle,
    },
    TypeReference,
    ProofObligation,
    MachineContract {
        machine_symbol: SymbolHandle,
    },
    StateSignatureContract {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallRequires,
    CallEnsures,
    ExitEnsures,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractFactKind {
    #[default]
    Requires,
    Ensures,
    Trusted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProofObligationKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactPayload {
    BooleanExpression(ExpressionHandle),
    DomainMembership {
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
    },
    TypeConstraint {
        constraint: Handle<TypeConstraintNode>,
    },
    ProofObligation {
        kind: ProofObligationKind,
    },
    Contract {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
    },
    InvariantDefinition {
        constraint_count: usize,
    },
}

impl Default for FactPayload {
    fn default() -> Self {
        Self::BooleanExpression(ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fact {
    pub place: FactPlace,
    pub point: ProgramPoint,
    pub origin: FactOrigin,
    pub payload: FactPayload,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactRef {
    pub fact: FactHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactContext {
    pub point: ProgramPoint,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolFactSet {
    pub symbol: SymbolHandle,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactPlan {
    pub facts: Arena<Fact>,
    pub refs: Arena<FactRef>,
    pub contexts: Arena<FactContext>,
    pub symbol_sets: Arena<SymbolFactSet>,
}

impl FactPlan {
    pub fn with_capacity(fact_capacity: usize, context_capacity: usize) -> Self {
        Self {
            facts: Arena::with_capacity(fact_capacity),
            refs: Arena::with_capacity(fact_capacity),
            contexts: Arena::with_capacity(context_capacity),
            symbol_sets: Arena::with_capacity(fact_capacity),
        }
    }

    pub fn append_fact(&mut self, fact: Fact) -> FactHandle {
        self.facts.append(fact)
    }

    pub fn append_ref(&mut self, refs: &mut HandleSpan<FactRef>, fact: FactHandle) {
        self.refs.append_to_span(refs, FactRef { fact });
    }

    pub fn append_context(
        &mut self,
        point: ProgramPoint,
        facts: HandleSpan<FactRef>,
    ) -> FactContextHandle {
        self.contexts.append(FactContext { point, facts })
    }

    pub fn append_symbol_set(
        &mut self,
        symbol: SymbolHandle,
        facts: HandleSpan<FactRef>,
    ) -> Handle<SymbolFactSet> {
        self.symbol_sets.append(SymbolFactSet { symbol, facts })
    }
}
