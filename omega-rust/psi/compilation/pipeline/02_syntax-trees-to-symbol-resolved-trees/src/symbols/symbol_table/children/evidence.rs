//! Explicit evidence namespaces shared by declaration and callable telescopes.

use crate::symbols::symbol_table::names::evidence_requirement_seed;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolTableAppender};

pub(super) fn insert_evidence_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    binder: SymbolHandle,
    bound: &symbol_resolved_trees::machine::GenericConformanceBound,
    sources: Option<&source::SourceMap>,
) {
    if let Some(trait_definition) =
        select_visible_trait_definition(program, &bound.trait_name, sources)
    {
        let mut requirements = Vec::new();
        collect_evidence_requirement_closure(
            program,
            trait_definition,
            sources,
            &mut Vec::new(),
            &mut requirements,
        );
        builder.insert_children(
            binder,
            requirements
                .into_iter()
                .map(|requirement| evidence_requirement_seed(&requirement.name)),
        );
    }
}

fn collect_evidence_requirement_closure<'program>(
    program: &'program SymbolResolvedTrees,
    trait_definition: &'program symbol_resolved_trees::trait_definition::TraitDefinition,
    sources: Option<&source::SourceMap>,
    visited: &mut Vec<(String, source::SourceSpan)>,
    output: &mut Vec<&'program symbol_resolved_trees::signature::StateSignature>,
) {
    if visited.iter().any(|(name, source_span)| {
        name == trait_definition.name.as_str()
            && *source_span == trait_definition.name.source_span()
    }) {
        return;
    }
    visited.push((
        trait_definition.name.as_str().to_owned(),
        trait_definition.name.source_span(),
    ));
    output.extend(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    );
    // A transparent refinement declares no requirements of its own: it
    // "names a structural bound on an existing base conformance, not a new
    // nominal satisfaction target" (spec, `language/conformances.md`). The
    // requirement namespace a binder over it selects is the base's.
    if let Some(base) = &trait_definition.refines
        && let Some(base_trait) = select_visible_trait_definition(program, &base.name, sources)
    {
        collect_evidence_requirement_closure(program, base_trait, sources, visited, output);
    }
    for parent in program.trait_requirements(trait_definition.requires) {
        let Some(parent_trait) = select_visible_trait_definition(program, &parent.name, sources)
        else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, sources, visited, output);
    }
}

fn reference_can_see_declaration(
    sources: Option<&source::SourceMap>,
    reference: source::SourceSpan,
    declaration: source::SourceSpan,
) -> bool {
    sources.is_none_or(|sources| sources.reference_can_see_declaration(reference, declaration))
}

fn select_visible_trait_definition<'program>(
    program: &'program SymbolResolvedTrees,
    reference_name: &symbol_resolved_trees::name::DiagnosticName,
    sources: Option<&source::SourceMap>,
) -> Option<&'program symbol_resolved_trees::trait_definition::TraitDefinition> {
    let reference = reference_name.source_span();
    let candidates = program
        .traits
        .iter()
        .filter(|candidate| {
            candidate.name == *reference_name
                && reference_can_see_declaration(sources, reference, candidate.name.source_span())
        })
        .collect::<Vec<_>>();
    candidates
        .iter()
        .copied()
        .find(|candidate| candidate.name.source_span().source_id == reference.source_id)
        .or_else(|| {
            sources.and_then(|sources| {
                candidates.iter().copied().find(|candidate| {
                    !sources.resolution_strata_separate(reference, candidate.name.source_span())
                })
            })
        })
        .or_else(|| candidates.first().copied())
}
