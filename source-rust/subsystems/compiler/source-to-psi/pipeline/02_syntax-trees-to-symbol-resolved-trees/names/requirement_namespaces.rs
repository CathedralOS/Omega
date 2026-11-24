/*
    Keep an evidence binder's local methods aligned with their declarations.

    Symbol creation lists the public methods in declaration order. Constant
    evaluation later uses that same list to find the declaration represented by
    a selected local child, while retaining the child's exact binder and target.
*/

use crate::resolution::tables::resolved_declarations;
use crate::source_resolved::{
    SymbolResolvedTrees, machine::GenericConformanceBound, name::DiagnosticName,
    signature::StateSignature, trait_definition::TraitDefinition,
};
use crate::source_symbols::SymbolTable;
use symbols::{SymbolHandle, SymbolKind};

// File imports and all trait declarations must be installed before a binder's methods can be selected.
pub(super) type PendingEvidence = Vec<(SymbolHandle, DiagnosticName)>;

pub(super) fn install_evidence(
    program: &SymbolResolvedTrees,
    symbols: &mut SymbolTable,
    pending: PendingEvidence,
) {
    for (binder, trait_name) in pending {
        let select = |name: &DiagnosticName| {
            let symbol =
                super::lookup::top_level_symbol_for_source(symbols, SymbolKind::Trait, name);
            exact_trait(program, symbol)
        };
        let Ok(declaration) = select(&trait_name) else {
            continue;
        };
        let mut requirements = Vec::new();
        if collect_requirements(
            program,
            declaration,
            &mut |parent| select(&parent.name),
            &mut Vec::new(),
            &mut requirements,
        )
        .is_err()
        {
            continue;
        }
        // The local method inherits the binder's provenance. Its signature stays on the trait.
        symbols.append_children(
            binder,
            requirements.iter().map(|requirement| {
                (
                    SymbolKind::State,
                    symbols::SymbolNameRef::Borrowed(requirement.name.as_str()),
                )
            }),
        );
    }
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
    let bounds = retained_declaration_conformance_bounds(program, owner)?.ok_or_else(|| {
        // Missing owner metadata must still refuse. Retain the full owner
        // chain and bounded declaration joins so the omission is diagnosable.
        let mut lineage = Vec::new();
        let mut ancestor = owner;
        while ancestor.is_valid() && lineage.len() < 8 {
            let row = program.symbols.get(ancestor);
            lineage.push((ancestor, row.kind, program.symbols.name(ancestor).to_owned(),
                row.parent, row.generated_from, row.declaration_owner,
                program.symbols.symbol_source_span(ancestor),
                program.symbols.symbol_provenance_source_span(ancestor)));
            ancestor = row.parent;
        }
        let authored = program.symbols.symbol_provenance_source_span(owner);
        let same_source = |name: &DiagnosticName| authored.is_some_and(|span| name.is_source_backed() && name.source_span() == span);
        let mut declarations = Vec::new();
        for declaration in program.traits.iter().filter(|declaration| same_source(&declaration.name)) {
            if declarations.len() == 8 { break; }
            declarations.push(format!("trait {:?} {:?} bounds={:?}", declaration.symbol, declaration.name,
                declaration.conformance_bounds.iter().map(|bound| (bound.binder, bound.trait_symbol)).collect::<Vec<_>>()));
        }
        for declaration in program.type_aliases.iter().filter(|declaration| same_source(&declaration.name)) {
            if declarations.len() == 8 { break; }
            declarations.push(format!("alias {:?} {:?} bounds={:?}", declaration.symbol, declaration.name,
                declaration.conformance_bounds.iter().map(|bound| (bound.binder, bound.trait_symbol)).collect::<Vec<_>>()));
        }
        for declaration in program.machines.iter().filter(|declaration| same_source(&declaration.name)) {
            if declarations.len() == 8 { break; }
            declarations.push(format!("machine {:?} {:?} bounds={:?}", declaration.symbol, declaration.name,
                declaration.conformance_bounds.iter().map(|bound| (bound.binder, bound.trait_symbol)).collect::<Vec<_>>()));
        }
        let signatures = program.traits.iter().flat_map(|declaration| {
            program.trait_machine_signatures(declaration.machines).iter()
                .chain(declaration.refinement_clauses.iter().map(|clause| &clause.signature))
        }).filter(|signature| same_source(&signature.name)).take(8)
            .map(|signature| (signature.symbol, signature.name.as_str().to_owned(),
                signature.conformance_bounds.iter().map(|bound| (bound.binder, bound.trait_symbol)).collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        format!("constant evidence child lost its declaring bound owner; owner lineage={lineage:?}; same-source declarations={declarations:?}; same-source signatures={signatures:?}")
    })?;
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
        &mut |reference| exact_trait(program, reference.symbol),
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
            .zip(&requirements)
            .any(|(child, requirement)| {
                program.symbols.get(*child).kind != SymbolKind::State
                    || program.symbols.name(*child) != requirement.name.as_str()
            })
    {
        return Err("constant evidence namespace disagrees with its declared requirements".into());
    }
    let ordinal = children
        .iter()
        .position(|child| *child == target)
        .ok_or("constant evidence requirement is not an exact binder child")?;
    Ok(Some(requirements[ordinal]))
}

// Original authored calls can outlive root selection. Their exact retained
// owner and bound metadata remain readable without making them live again.
fn retained_declaration_conformance_bounds(
    program: &SymbolResolvedTrees,
    owner: SymbolHandle,
) -> Result<Option<&[GenericConformanceBound]>, String> {
    let roots = program
        .machines
        .retained_records()
        .map(|declaration| {
            (
                declaration.symbol,
                declaration.conformance_bounds.as_slice(),
            )
        })
        .chain(program.traits.retained_records().map(|declaration| {
            (
                declaration.symbol,
                declaration.conformance_bounds.as_slice(),
            )
        }))
        .chain(
            program
                .data_definitions
                .retained_records()
                .map(|declaration| {
                    (
                        declaration.symbol,
                        declaration.conformance_bounds.as_slice(),
                    )
                }),
        )
        .chain(program.type_aliases.retained_records().map(|declaration| {
            (
                declaration.symbol,
                declaration.conformance_bounds.as_slice(),
            )
        }))
        .chain(
            program
                .traits
                .retained_records()
                .flat_map(|declaration| program.trait_machine_signatures(declaration.machines))
                .map(|signature| (signature.symbol, signature.conformance_bounds.as_slice())),
        )
        .chain(
            resolved_declarations(program)
                .data_type_parameters
                .iter()
                .filter_map(|(_, parameter)| {
                    parameter.kind.contract_signature().map(|signature| {
                        (parameter.symbol, signature.conformance_bounds.as_slice())
                    })
                }),
        );
    let mut matching = roots.filter(|(symbol, _)| owner.is_valid() && *symbol == owner);
    let Some((_, bounds)) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err("evidence requirement has ambiguous retained bound owners".into());
    }
    Ok(Some(bounds))
}

fn exact_trait(
    program: &SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Result<&TraitDefinition, String> {
    let mut declarations = program
        .traits
        .retained_records()
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
        &crate::source_resolved::trait_definition::TraitRequirement,
    ) -> Result<&'program TraitDefinition, Error>,
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
    for reference in declaration
        .refines
        .iter()
        .chain(program.trait_requirements(declaration.requires))
    {
        let parent = resolve_parent(reference)?;
        collect_requirements(program, parent, resolve_parent, visited, output)?;
    }
    Ok(())
}
