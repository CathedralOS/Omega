//! Required membership supplies a theory for one immutable captured subject.
//! The exact predicate root is replayed through the common Boolean decomposer;
//! finding a comparison somewhere inside a predicate is not an entailment.
//! This consumes membership, including routed membership, without minting it.

use super::{PremiseScope, StatedOrderingPremise, decompose_premise_expression};
use crate::checked_trees::{BorrowCompatibilityPremiseSource, ContractProofFact};
use arena::Handle;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::domain::{ProofFact, ProofMembershipFact};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;

pub(super) fn append_membership_premises(
    program: &TypedTrees,
    lookup: &crate::validation::ImmutableBoundLookup<'_>,
    machine: &Machine,
    state: &State,
    fact: Handle<ContractProofFact>,
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
        || !symbol_resolved_trees_to_typed_trees::typed_trees::domain::supports_symbol_only_proof(
            program,
            domain.symbol,
        )
    {
        return;
    }
    if !crate::validation::has_exact_integer_domain_subject(
        program,
        domain,
        lookup,
        membership.value,
    ) || !crate::validation::has_builtin_bound_expression_meaning(
        program,
        machine,
        Some(state),
        membership.value,
    ) {
        return;
    }
    let Some(subject) = super::normalized_bound(program, lookup, membership.value)
        .or_else(|| super::projected_immutable_bound(program, lookup, membership.value))
    else {
        return;
    };
    for offset in 0..domain.facts.count() {
        let Some(index) = domain.facts.start().arena_index().checked_add(offset) else {
            return;
        };
        let predicate = Handle::from_parts(index, domain.facts.start().generation());
        let ProofFact::Expression(expression) = program.proof_facts.get(predicate) else {
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
            BorrowCompatibilityPremiseSource::RequiresDomain {
                membership: fact,
                domain: domain.symbol,
                predicate,
            },
            &None,
            premises,
        );
    }
}
