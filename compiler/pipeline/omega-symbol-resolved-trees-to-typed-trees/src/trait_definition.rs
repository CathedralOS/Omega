use crate::program::Lowerer;
use crate::state::lower_state_signature;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    trait_definition: &resolved::trait_definition::TraitDefinition,
) -> Result<typed::trait_definition::TraitDefinition, Diagnostic> {
    let mut typed_trait = typed::trait_definition::TraitDefinition {
        symbol: trait_definition.symbol,
        is_boundary: trait_definition.is_boundary,
        name: crate::name::lower_name(&trait_definition.name),
        requires: omega_core::arena::HandleSpan::empty(),
        machines: omega_core::arena::HandleSpan::empty(),
    };

    for requirement in lowerer
        .source_trees
        .trait_requirements(trait_definition.requires)
    {
        lowerer.typed_trees.push_trait_requirement(
            &mut typed_trait,
            typed::trait_definition::TraitRequirement {
                symbol: requirement.symbol,
                name: crate::name::lower_name(&requirement.name),
            },
        );
    }

    for signature in lowerer
        .source_trees
        .trait_machine_signatures(trait_definition.machines)
    {
        let signature = lower_state_signature(lowerer, signature)?;
        lowerer
            .typed_trees
            .push_trait_machine_signature(&mut typed_trait, signature);
    }

    Ok(typed_trait)
}
