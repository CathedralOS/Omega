use crate::context::*;

pub(crate) fn build_invariant_facts(program: &psi_typed_trees::TypedTrees) -> InvariantFacts {
    let mut definitions = psi_arena::Arena::with_capacity(program.invariant_definitions().len());

    for definition in program.invariant_definitions() {
        definitions.append(InvariantFact {
            symbol: definition.symbol,
            name: definition.name.clone(),
            constraint_count: program
                .type_reference_table
                .constraints(definition.constraints)
                .len(),
        });
    }

    InvariantFacts::with_roots(definitions)
}
