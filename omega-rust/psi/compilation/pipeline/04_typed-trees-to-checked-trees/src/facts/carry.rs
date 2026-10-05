use checked_trees::{CarryFacts, DataCarryFact};
use typed_trees::TypedTrees;

pub(super) fn build_carry_facts(program: &TypedTrees) -> CarryFacts {
    let data = program
        .data_definitions()
        .iter()
        .map(|definition| DataCarryFact {
            data: definition.symbol,
            declared: definition.properties.carry,
            effective: validation::effective_data_carry_policy(program, definition),
        })
        .collect();

    CarryFacts {
        data,
        suspension_crossings: Vec::new(),
        activation_wide_carry: Vec::new(),
        claim_policies: Vec::new(),
    }
}
