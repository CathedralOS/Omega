use checked_trees::{CarryFacts, DataCarryFact};

pub(super) fn build_carry_facts(program: &dyn ::validation::CheckingInput) -> CarryFacts {
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
        suspension_loans: arena::Arena::new(),
        suspension_value_loans: arena::Arena::new(),
    }
}
