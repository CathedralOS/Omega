use crate::data::lower_type_parameters;
use crate::lowerer::Lowerer;
use crate::state::lower_state_signature_node;
use crate::type_reference::lower_child_type_references;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::signature::StateSignature;
use psi_symbol_resolved_trees::trait_definition::{
    TraitDefinition, TraitRequirement, TraitStorage,
};
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    trait_definition: &syntax::item::TraitDefinition,
) -> Result<TraitDefinition, Diagnostic> {
    let name = crate::name::lower_name(&trait_definition.name);
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, trait_definition.type_parameters)?;
    let conformance_bounds = crate::machine::lower_generic_conformance_bounds(
        lowerer,
        syntax_trees,
        &trait_definition.conformance_bounds,
    )?;
    let requires = lower_trait_requirements(
        lowerer,
        syntax_trees,
        trait_definition.parents,
        trait_definition.requires,
    )?;
    let machines = lower_trait_machine_signatures(
        lowerer,
        syntax_trees,
        &trait_definition.name,
        trait_definition.machines,
    )?;

    Ok(TraitDefinition {
        symbol: SymbolHandle::invalid(),
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
        name,
        storage: TraitStorage {
            lifetime_parameters: trait_definition
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters,
            conformance_bounds,
            requires,
            machines,
        },
    })
}

fn lower_trait_requirements(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    parents: HandleSpan<syntax::types::TypeReferenceHandle>,
    requires: HandleSpan<syntax::identifier::Identifier>,
) -> Result<HandleSpan<TraitRequirement>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for parent_handle in syntax_trees.type_references.type_reference_handles(parents) {
        let (name, lifetime_arguments, arguments) =
            match syntax_trees.type_references.type_reference(*parent_handle) {
                syntax::types::TypeReferenceNode::Named(name) => {
                    (name, Vec::new(), HandleSpan::empty())
                }
                syntax::types::TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                    ..
                } => (
                    base_name,
                    lifetime_arguments
                        .iter()
                        .map(crate::name::lower_name)
                        .collect(),
                    lower_child_type_references(lowerer, syntax_trees, *arguments)?,
                ),
                _ => {
                    return Err(Diagnostic::error(
                        "a trait parent must be a named trait, optionally with generic arguments",
                    ));
                }
            };
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .trait_requirements
            .append_to_span(
                &mut span,
                TraitRequirement {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(name),
                    lifetime_arguments,
                    arguments,
                },
            );
    }

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
                    lifetime_arguments: Vec::new(),
                    arguments: HandleSpan::empty(),
                },
            );
    }

    Ok(span)
}

fn lower_trait_machine_signatures(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    trait_name: &syntax::identifier::Identifier,
    machines: HandleSpan<syntax::item::StateSignatureHandle>,
) -> Result<HandleSpan<StateSignature>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for signature in syntax_trees.items.state_signatures(machines) {
        let lowered = lower_state_signature_node(
            lowerer,
            syntax_trees,
            syntax_trees.items.state_signature(*signature),
        )?;
        let handle = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .trait_machine_signatures
            .append_to_span(&mut span, lowered.signature);
        lowerer.pending_signature_service_reaches.push(
            crate::lowerer::PendingSignatureServiceReach {
                location: crate::lowerer::PendingSignatureLocation::Trait(handle),
                owner: crate::lowerer::PendingSignatureOwner::Trait(crate::name::lower_name(
                    trait_name,
                )),
                authored: lowered.service_reaches,
            },
        );
    }

    Ok(span)
}
