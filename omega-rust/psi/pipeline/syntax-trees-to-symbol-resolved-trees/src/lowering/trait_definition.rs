//! Trait definitions: parents, requirements, conformance bounds, and machine
//! signatures.

use crate::lowering::data::lower_type_parameters;
use crate::lowering::state::lower_state_signature_node;
use crate::lowering::type_reference::lower_child_type_references;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::signature::StateSignature;
use symbol_resolved_trees::trait_definition::{
    TraitDefinition, TraitRefinementClause, TraitRequirement, TraitStorage,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    trait_definition: &syntax::item::TraitDefinition,
) -> Result<TraitDefinition, Diagnostic> {
    let name = crate::lowering::name::lower_name(&trait_definition.name);
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, trait_definition.type_parameters)?;
    let conformance_bounds = crate::lowering::machine::lower_generic_conformance_bounds(
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
    let refines = trait_definition
        .refines
        .map(|base| lower_trait_requirement_node(lowerer, syntax_trees, base))
        .transpose()?;
    let mut refinement_clauses = Vec::with_capacity(trait_definition.refinement_clauses.len());
    for clause in &trait_definition.refinement_clauses {
        let clause_signature = &clause.signature;
        let lowered = crate::lowering::state::lower_state_signature_parts(
            lowerer,
            syntax_trees,
            crate::lowering::state::StateSignatureParts {
                name: &clause_signature.name,
                spelling: clause_signature.spelling,
                lifetime_parameters: &clause_signature.lifetime_parameters,
                type_parameters: clause_signature.type_parameters,
                parameters: clause_signature.parameters,
                native_callback_parameters: &clause_signature.native_callback_parameters,
                return_type_handle: clause_signature.return_type,
                is_default: clause_signature.is_default,
                service_reach_is_installation_bound: clause_signature
                    .service_reach_is_installation_bound,
                service_reach_keyword_source_spans: &clause_signature
                    .service_reach_keyword_source_spans,
                service_reaches: clause_signature.service_reaches,
                invokes: clause_signature.invokes,
                suspends_keyword_source_spans: &clause_signature.suspends_keyword_source_spans,
                blocks_keyword_source_spans: &clause_signature.blocks_keyword_source_spans,
                suspends: clause_signature.suspends,
                blocks: clause_signature.blocks,
                contracts: clause_signature.contracts,
                terminates_guarantee: clause_signature.terminates_guarantee,
                where_facts: clause_signature.where_facts,
            },
        )?;
        refinement_clauses.push(TraitRefinementClause {
            requirement: clause
                .requirement
                .as_ref()
                .map(crate::lowering::name::lower_name),
            signature: lowered.signature,
            service_reaches: lowered.service_reaches,
        });
    }

    Ok(TraitDefinition {
        symbol: SymbolHandle::invalid(),
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
        name,
        storage: TraitStorage {
            lifetime_parameters: trait_definition
                .lifetime_parameters
                .iter()
                .map(crate::lowering::name::lower_name)
                .collect(),
            type_parameters,
            conformance_bounds,
            requires,
            machines,
            refines,
            refinement_clauses,
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
        let requirement = lower_trait_requirement_node(lowerer, syntax_trees, *parent_handle)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .trait_requirements
            .append_to_span(&mut span, requirement);
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
                    name: crate::lowering::name::lower_name(required_trait),
                    lifetime_arguments: Vec::new(),
                    arguments: HandleSpan::empty(),
                },
            );
    }

    Ok(span)
}

/// One `Name` or `Name<args>` reference in trait-position (parent, `requires`,
/// or refinement `= Base` head).
fn lower_trait_requirement_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    handle: syntax::types::TypeReferenceHandle,
) -> Result<TraitRequirement, Diagnostic> {
    let (name, lifetime_arguments, arguments) = match syntax_trees
        .type_references
        .type_reference(handle)
    {
        syntax::types::TypeReferenceNode::Named(name) => (name, Vec::new(), HandleSpan::empty()),
        syntax::types::TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } => (
            base_name,
            lifetime_arguments
                .iter()
                .map(crate::lowering::name::lower_name)
                .collect(),
            lower_child_type_references(lowerer, syntax_trees, *arguments)?,
        ),
        _ => {
            return Err(Diagnostic::error(
                "a trait parent must be a named trait, optionally with generic arguments",
            ));
        }
    };
    Ok(TraitRequirement {
        symbol: SymbolHandle::invalid(),
        name: crate::lowering::name::lower_name(name),
        lifetime_arguments,
        arguments,
    })
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
            crate::resolution::lowerer::PendingSignatureServiceReach {
                location: crate::resolution::lowerer::PendingSignatureLocation::Trait(handle),
                owner: crate::resolution::lowerer::PendingSignatureOwner::Trait(
                    crate::lowering::name::lower_name(trait_name),
                ),
                keyword_source_spans: lowered.service_reach_keyword_source_spans,
                authored: lowered.service_reaches,
            },
        );
    }

    Ok(span)
}
