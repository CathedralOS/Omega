//! Refine conservative syntax-stage gates only after exact literal landing and
//! operator binding. Unsupported predicates retain their construction debt.

use numerics::float_semantics::FloatMeaning;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::domain::ProofFact;

pub(super) fn refine_default_domain_zero(program: &mut TypedTrees) {
    let admitted: Vec<_> = program
        .data_definitions()
        .iter()
        .enumerate()
        .filter_map(|(definition_index, definition)| {
            // The early boolean also includes membership and first-case
            // reasons. This refinement owns only complete record predicates;
            // it cannot clear a reason it has not independently replayed.
            if !definition.zero_gated
                || !definition.type_parameters.is_empty()
                || definition.where_facts.is_empty()
                || program
                    .data_members(definition)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
            {
                return None;
            }
            program
                .proof_facts
                .span_or_empty(definition.where_facts)
                .iter()
                .all(|fact| {
                    let ProofFact::Expression(expression) = fact else {
                        return false;
                    };
                    validation::data_float_predicate(program, definition, *expression, &|_, _| {
                        Some(FloatMeaning::Zero { negative: false })
                    }) == Some(true)
                })
                .then_some(definition_index)
        })
        .collect();
    let definitions = program.roots.data_definitions;
    for definition_index in admitted {
        program
            .tables
            .data_definitions
            .span_mut_or_empty(definitions)[definition_index]
            .zero_gated = false;
    }
}

#[cfg(test)]
mod tests;
