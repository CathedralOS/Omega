//! Required membership supplies a theory for one immutable captured subject.
//! The exact predicate root is checked through the common Boolean decomposer;
//! finding a comparison somewhere inside a predicate is not an entailment.
//! This consumes membership, including routed membership, without minting it.

use super::{PremiseScope, StatedOrderingPremise, decompose_premise_expression};
use typed_trees::domain::ProofMembershipFact;
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) fn append_membership_premises(
    program: &dyn ::validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    machine: &Machine,
    state: &State,
    membership: &ProofMembershipFact,
    premises: &mut Vec<StatedOrderingPremise>,
) {
    let mut definitions = program
        .domain_definitions()
        .iter()
        .filter(|definition| definition.symbol == membership.domain_symbol);
    let Some(domain) = definitions.next() else {
        return;
    };
    // A family symbol is not an instantiated theory. Aliases have already
    // expanded into atomic membership rows during typed lowering.
    if definitions.next().is_some()
        || !domain.type_parameters.is_empty()
        || !domain.index_arguments.is_empty()
        || !membership.domain_arguments.is_empty()
        || domain.alias.is_some()
        || !domain.semantic_id.is_valid()
        || domain.semantic_id != membership.semantic_domain
        || !typed_trees::domain::supports_symbol_only_proof(program, domain.symbol)
    {
        return;
    }
    if !validation::has_exact_integer_domain_subject(program, domain, lookup, membership.value)
        || !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            membership.value,
        )
    {
        return;
    }
    let Some(subject) = super::normalized_bound(program, lookup, membership.value)
        .or_else(|| super::projected_immutable_bound(program, lookup, membership.value))
    else {
        return;
    };
    for predicate in domain.facts.handles() {
        let Some(expression) = program.proof_facts.get(predicate).as_expression() else {
            continue;
        };
        decompose_premise_expression(
            program,
            lookup,
            PremiseScope::Domain {
                definition: domain,
                subject: subject.clone(),
            },
            *expression,
            false,
            &None,
            premises,
        );
    }
}
