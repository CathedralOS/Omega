//! Refine conservative syntax-stage gates only after exact literal landing and
//! operator binding. Unsupported predicates retain their construction debt.

use typed_trees::TypedTrees;

pub(super) fn refine_default_domain_zero(program: &mut TypedTrees) {
    let admitted: Vec<_> = program
        .data_definitions()
        .iter()
        .enumerate()
        .filter_map(|(definition_index, definition)| {
            (definition.zero_gated && validation::data_predicates_admit_zero(program, definition))
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
