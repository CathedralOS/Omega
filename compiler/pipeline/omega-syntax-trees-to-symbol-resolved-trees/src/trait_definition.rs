use crate::lowerer::Lowerer;
use crate::state::lower_state_signature_node;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::signature::StateSignature;
use omega_symbol_resolved_trees::trait_definition::{
    TraitDefinition, TraitRequirement, TraitStorage,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    trait_definition: &syntax::item::TraitDefinition,
) -> Result<TraitDefinition, Diagnostic> {
    let requires = lower_trait_requirements(lowerer, syntax_trees, trait_definition.requires);
    let machines =
        lower_trait_machine_signatures(lowerer, syntax_trees, trait_definition.machines)?;

    Ok(TraitDefinition {
        symbol: SymbolHandle::invalid(),
        is_boundary: trait_definition.is_boundary,
        name: crate::name::lower_name(&trait_definition.name),
        storage: TraitStorage { requires, machines },
    })
}

fn lower_trait_requirements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    requires: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<TraitRequirement> {
    let mut span = HandleSpan::empty();

    for required_trait in syntax_trees.items.identifier_path_members(requires) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .trait_requirements
            .append_to_span(
                &mut span,
                TraitRequirement {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(required_trait),
                },
            );
    }

    span
}

fn lower_trait_machine_signatures(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machines: HandleSpan<syntax::item::StateSignatureHandle>,
) -> Result<HandleSpan<StateSignature>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for signature in syntax_trees.items.state_signatures(machines) {
        let signature = lower_state_signature_node(
            lowerer,
            syntax_trees,
            syntax_trees.items.state_signature(*signature),
        )?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .trait_machine_signatures
            .append_to_span(&mut span, signature);
    }

    Ok(span)
}
