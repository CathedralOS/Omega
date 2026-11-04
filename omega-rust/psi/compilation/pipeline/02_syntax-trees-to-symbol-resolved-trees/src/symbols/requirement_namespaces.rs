//! Keep an evidence binder's local methods aligned with their declarations.
//!
//! Symbol creation lists the public methods in declaration order. Constant
//! evaluation later uses that same list to find the declaration represented by
//! a selected local child, while retaining the child's exact binder and target.

use crate::tables::resolved_declarations;
use source::SourceMap;
use symbol_resolved_trees::{
    SymbolResolvedTrees, machine::GenericConformanceBound, name::DiagnosticName,
    signature::StateSignature, trait_definition::TraitDefinition,
};
use symbols::{SymbolHandle, SymbolKind};

pub(crate) fn visible_requirements<'program>(
    program: &'program SymbolResolvedTrees,
    bound: &GenericConformanceBound,
    sources: Option<&SourceMap>,
) -> Vec<&'program StateSignature> {
    let Some(declaration) = select_visible_trait(program, &bound.trait_name, sources) else {
        return Vec::new();
    };
    // Symbol creation can precede parent resolution. The ordinary resolver
    // diagnoses unavailable inherited declarations after this list is seeded.
    let mut requirements = Vec::new();
    collect_requirements(
        program,
        declaration,
        &mut |reference| Ok(select_visible_trait(program, &reference.name, sources)),
        &mut Vec::new(),
        &mut requirements,
    )
    .unwrap_or_else(|never: std::convert::Infallible| match never {});
    requirements
}

pub(crate) fn evidence_requirement(
    program: &SymbolResolvedTrees,
    target: SymbolHandle,
) -> Result<Option<&StateSignature>, String> {
    let selected = program.symbols.get(target);
    if selected.kind != SymbolKind::State
        || !selected.parent.is_valid()
        || program.symbols.get(selected.parent).kind != SymbolKind::ConformanceParameter
    {
        return Ok(None);
    }
    let binder = selected.parent;
    let owner = program.symbols.get(binder).parent;
    let bounds = declaration_conformance_bounds(program, owner)
        .ok_or("constant evidence child lost its declaring bound owner")?;
    let mut matches = bounds.iter().filter(|bound| bound.binder == binder);
    let bound = matches
        .next()
        .ok_or("constant evidence child lost its exact authored bound")?;
    if matches.next().is_some() {
        return Err("constant evidence child has multiple authored bounds".into());
    }
    let declaration = exact_trait(program, bound.trait_symbol)?;
    let mut requirements = Vec::new();
    collect_requirements(
        program,
        declaration,
        &mut |reference| exact_trait(program, reference.symbol).map(Some),
        &mut Vec::new(),
        &mut requirements,
    )?;
    let children = program
        .symbols
        .child_handles(binder)
        .ok_or("constant evidence binder lost its requirement children")?
        .collect::<Vec<_>>();
    if children.len() != requirements.len()
        || children
            .iter()
            .any(|child| program.symbols.get(*child).kind != SymbolKind::State)
    {
        return Err("constant evidence namespace disagrees with its declared requirements".into());
    }
    let ordinal = children
        .iter()
        .position(|child| *child == target)
        .ok_or("constant evidence requirement is not an exact binder child")?;
    Ok(Some(requirements[ordinal]))
}

fn declaration_conformance_bounds(
    program: &SymbolResolvedTrees,
    owner: SymbolHandle,
) -> Option<&[GenericConformanceBound]> {
    program
        .machines
        .iter()
        .find(|declaration| declaration.symbol == owner)
        .map(|declaration| declaration.conformance_bounds.as_slice())
        .or_else(|| {
            program
                .traits
                .iter()
                .find(|declaration| declaration.symbol == owner)
                .map(|declaration| declaration.conformance_bounds.as_slice())
        })
        .or_else(|| {
            program
                .data_definition_by_symbol(owner)
                .map(|declaration| declaration.conformance_bounds.as_slice())
        })
        .or_else(|| {
            program
                .type_aliases
                .iter()
                .find(|declaration| declaration.symbol == owner)
                .map(|declaration| declaration.conformance_bounds.as_slice())
        })
        .or_else(|| {
            program
                .traits
                .iter()
                .flat_map(|declaration| program.trait_machine_signatures(declaration.machines))
                .find(|signature| signature.symbol == owner)
                .map(|signature| signature.conformance_bounds.as_slice())
        })
        .or_else(|| {
            resolved_declarations(program)
                .data_type_parameters
                .iter()
                .find_map(|(_, parameter)| {
                    if parameter.symbol != owner {
                        return None;
                    }
                    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
                        &parameter.kind
                    else {
                        return None;
                    };
                    contract
                        .structural()
                        .map(|signature| signature.conformance_bounds.as_slice())
                })
        })
}

fn exact_trait(
    program: &SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Result<&TraitDefinition, String> {
    let mut declarations = program
        .traits
        .iter()
        .filter(|declaration| symbol.is_valid() && declaration.symbol == symbol);
    let declaration = declarations
        .next()
        .ok_or("constant evidence namespace lost a resolved trait declaration")?;
    if declarations.next().is_some() {
        return Err("constant evidence namespace has ambiguous trait identity".into());
    }
    Ok(declaration)
}

fn collect_requirements<'program, Error>(
    program: &'program SymbolResolvedTrees,
    declaration: &'program TraitDefinition,
    resolve_parent: &mut impl FnMut(
        &symbol_resolved_trees::trait_definition::TraitRequirement,
    ) -> Result<Option<&'program TraitDefinition>, Error>,
    visited: &mut Vec<&'program TraitDefinition>,
    output: &mut Vec<&'program StateSignature>,
) -> Result<(), Error> {
    if visited
        .iter()
        .any(|known| std::ptr::eq(*known, declaration))
    {
        return Ok(());
    }
    visited.push(declaration);
    output.extend(program.trait_machine_signatures(declaration.machines));
    // Refinements expose the base namespace before any additional parents.
    for reference in declaration
        .refines
        .iter()
        .chain(program.trait_requirements(declaration.requires))
    {
        if let Some(parent) = resolve_parent(reference)? {
            collect_requirements(program, parent, resolve_parent, visited, output)?;
        }
    }
    Ok(())
}

fn select_visible_trait<'program>(
    program: &'program SymbolResolvedTrees,
    reference_name: &DiagnosticName,
    sources: Option<&SourceMap>,
) -> Option<&'program TraitDefinition> {
    let reference = reference_name.source_span();
    let candidates = program
        .traits
        .iter()
        .filter(|candidate| {
            candidate.name == *reference_name
                && sources.is_none_or(|sources| {
                    sources.reference_can_see_declaration(reference, candidate.name.source_span())
                })
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
