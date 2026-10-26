use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

use super::insert_machine_type_parameter_children;
use crate::symbols::symbol_table::names::{symbol_seed, type_parameter_seed};

pub(in crate::symbols::symbol_table) fn insert_machine_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    machine_symbol: SymbolHandle,
    machine: &symbol_resolved_trees::machine::Machine,
    has_sources: bool,
    sources: Option<&source::SourceMap>,
) {
    let machine_children = builder.insert_children(
        machine_symbol,
        program
            .machine_type_parameters(machine)
            .iter()
            .map(|parameter| type_parameter_seed(parameter, false, has_sources))
            .chain(machine.conformance_bounds.iter().map(|bound| {
                symbol_seed(
                    SymbolKind::ConformanceParameter,
                    &bound.binder_name,
                    has_sources,
                )
            }))
            .chain(
                program
                    .machine_owned_data(machine.owned_data)
                    .iter()
                    .map(|owned_data| {
                        symbol_seed(SymbolKind::Field, &owned_data.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|state| program.machine_state(*state))
                    .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
            ),
    );
    let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

    insert_machine_type_parameter_children(
        builder,
        program,
        program.machine_type_parameters(machine),
        &mut machine_children,
        has_sources,
    );
    for bound in machine.conformance_bounds.iter() {
        let Some(binder_symbol) = machine_children.next() else {
            break;
        };
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
                binder_symbol,
                requirements.into_iter().map(|requirement| {
                    symbol_seed(SymbolKind::State, &requirement.name, has_sources)
                }),
            );
        }
    }
    for _ in program.machine_owned_data(machine.owned_data) {
        let _ = machine_children.next();
    }
    for state in program.machine_state_handles(machine.states) {
        if let Some(state_symbol) = machine_children.next() {
            let state = program.machine_state(*state);
            insert_state_symbol_children(builder, program, state_symbol, state, has_sources);
        }
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

fn insert_state_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    state_symbol: SymbolHandle,
    state: &symbol_resolved_trees::state::State,
    has_sources: bool,
) {
    builder.insert_children(
        state_symbol,
        program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
    );
}
