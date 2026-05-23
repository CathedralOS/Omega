use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::name::ProgramName;
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
        domain: HandleSpan<ProgramName>,
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

    pub fn facts_for_symbol(&self, symbol: SymbolHandle) -> impl Iterator<Item = &Fact> {
        self.symbol_sets
            .iter()
            .filter(move |(_, set)| set.symbol == symbol)
            .flat_map(move |(_, set)| {
                self.refs
                    .span_or_empty(set.facts)
                    .iter()
                    .map(move |reference| self.facts.get(reference.fact))
            })
    }

    pub fn context_facts(&self, context: &FactContext) -> impl Iterator<Item = &Fact> {
        self.refs
            .span_or_empty(context.facts)
            .iter()
            .map(move |reference| self.facts.get(reference.fact))
    }
}

pub fn build_definition_fact_plan(program: &TypedTrees) -> FactPlan {
    let mut facts = FactPlan::with_capacity(
        estimated_definition_fact_capacity(program),
        estimated_definition_context_capacity(program),
    );

    append_domain_definition_facts(program, &mut facts);
    append_invariant_definition_facts(program, &mut facts);

    facts
}

fn estimated_definition_fact_capacity(program: &TypedTrees) -> usize {
    let domain_facts = program
        .domain_definitions()
        .iter()
        .map(|domain| program.proof_facts(domain).len())
        .sum::<usize>();
    let invariant_constraints = program
        .invariant_definitions()
        .iter()
        .map(|invariant| invariant.constraints.len())
        .sum::<usize>();

    domain_facts.saturating_add(invariant_constraints)
}

fn estimated_definition_context_capacity(program: &TypedTrees) -> usize {
    program
        .domain_definitions()
        .len()
        .saturating_add(program.invariant_definitions().len())
}

fn append_domain_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for domain in program.domain_definitions() {
        let mut refs = HandleSpan::empty();
        for fact_handle in proof_fact_handles(domain.facts) {
            let payload = match program.proof_facts.get(fact_handle) {
                ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
                ProofFact::Membership(membership) => FactPayload::DomainMembership {
                    value: membership.value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
                },
            };
            let fact = facts.append_fact(Fact {
                place: FactPlace::Symbol(domain.symbol),
                point: ProgramPoint::Definition {
                    symbol: domain.symbol,
                },
                origin: FactOrigin::DomainDefinition {
                    domain_symbol: domain.symbol,
                },
                payload,
            });
            facts.append_ref(&mut refs, fact);
        }
        facts.append_context(
            ProgramPoint::Definition {
                symbol: domain.symbol,
            },
            refs,
        );
        facts.append_symbol_set(domain.symbol, refs);
    }
}

fn append_invariant_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for invariant in program.invariant_definitions() {
        let mut refs = HandleSpan::empty();
        for constraint in type_constraint_handles(invariant.constraints) {
            let fact = facts.append_fact(Fact {
                place: FactPlace::Symbol(invariant.symbol),
                point: ProgramPoint::Definition {
                    symbol: invariant.symbol,
                },
                origin: FactOrigin::InvariantDefinition {
                    invariant_symbol: invariant.symbol,
                },
                payload: FactPayload::TypeConstraint { constraint },
            });
            facts.append_ref(&mut refs, fact);
        }
        facts.append_context(
            ProgramPoint::Definition {
                symbol: invariant.symbol,
            },
            refs,
        );
        facts.append_symbol_set(invariant.symbol, refs);
    }
}

fn proof_fact_handles(facts: HandleSpan<ProofFact>) -> impl Iterator<Item = Handle<ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}

fn type_constraint_handles(
    constraints: HandleSpan<TypeConstraintNode>,
) -> impl Iterator<Item = Handle<TypeConstraintNode>> {
    (0..constraints.count()).map(move |offset| {
        Handle::from_parts(
            constraints
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("type constraint handle index overflow"),
            constraints.start().generation(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{FactPayload, build_definition_fact_plan};
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::TypedTrees;
    use omega_typed_trees::domain::{DomainDefinition, ProofFact};
    use omega_typed_trees::expression::ExpressionNode;
    use omega_typed_trees::invariant::InvariantDefinition;
    use omega_typed_trees::name::ProgramName;
    use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

    #[test]
    fn builds_definition_fact_plan_for_domains_and_invariants() {
        let domain_symbol = SymbolHandle::from_arena_index(10);
        let invariant_symbol = SymbolHandle::from_arena_index(11);

        let mut program = TypedTrees::default();
        let expression = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(ProofFact::Expression(expression));
        program.push_domain_definition(DomainDefinition {
            symbol: domain_symbol,
            name: ProgramName::generated("Player::Alive"),
            target_type: TypeReferenceHandle::invalid(),
            facts: HandleSpan::from_parts(fact, 1),
            body_token_count: 1,
        });

        let constraint = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Named(ProgramName::generated("finite"))]);
        program.push_invariant_definition(InvariantDefinition {
            symbol: invariant_symbol,
            name: ProgramName::generated("Finite"),
            constraints: constraint,
        });

        let facts = build_definition_fact_plan(&program);

        assert_eq!(facts.facts.len(), 2);
        assert_eq!(facts.contexts.len(), 2);
        assert_eq!(facts.symbol_sets.len(), 2);
        assert!(
            facts
                .facts_for_symbol(domain_symbol)
                .any(|fact| matches!(fact.payload, FactPayload::BooleanExpression(_)))
        );
        assert!(
            facts
                .facts_for_symbol(invariant_symbol)
                .any(|fact| matches!(fact.payload, FactPayload::TypeConstraint { .. }))
        );
    }
}
