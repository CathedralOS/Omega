use psi_arena::{Handle, HandleSpan};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::types::TypeConstraintNode;

use crate::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceHandle, ProgramPoint};

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
            let proof_fact = program.proof_facts.get(fact_handle);
            let place = append_proof_fact_place(program, facts, proof_fact);
            let payload = match proof_fact {
                ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
                ProofFact::Membership(membership) => FactPayload::DomainMembership {
                    value: membership.value,
                    domain: membership.domain,
                    domain_symbol: membership.domain_symbol,
                },
                ProofFact::Proposition(application) => FactPayload::PropositionApplication {
                    fact: fact_handle,
                    proposition: application.proposition,
                },
            };
            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Definition {
                    symbol: domain.symbol,
                },
                origin: FactOrigin::DomainDefinition {
                    domain_symbol: domain.symbol,
                },
                evidence: Default::default(),
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

fn append_proof_fact_place(
    program: &TypedTrees,
    facts: &mut FactPlan,
    proof_fact: &ProofFact,
) -> PlaceHandle {
    match proof_fact {
        ProofFact::Expression(expression) => {
            facts.append_place_from_expression(program, *expression)
        }
        ProofFact::Membership(membership) => {
            facts.append_place_from_expression(program, membership.value)
        }
        ProofFact::Proposition(application) => facts.append_symbol_place(application.proposition),
    }
}

fn append_invariant_definition_facts(program: &TypedTrees, facts: &mut FactPlan) {
    for invariant in program.invariant_definitions() {
        let mut refs = HandleSpan::empty();
        let place = facts.append_symbol_place(invariant.symbol);
        for constraint in type_constraint_handles(invariant.constraints) {
            let fact = facts.append_fact(Fact {
                place: FactPlace::Place(place),
                point: ProgramPoint::Definition {
                    symbol: invariant.symbol,
                },
                origin: FactOrigin::InvariantDefinition {
                    invariant_symbol: invariant.symbol,
                },
                evidence: Default::default(),
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
