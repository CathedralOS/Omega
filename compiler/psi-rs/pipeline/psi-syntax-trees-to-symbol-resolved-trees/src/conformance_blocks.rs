use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbol_resolved_trees::trait_definition::{
    ConformanceImplementation, ConformanceRow, ConformanceRowSource,
};
use psi_symbols::SymbolHandle;

#[derive(Clone)]
struct TraitCatalogEntry {
    symbol: SymbolHandle,
    name: DiagnosticName,
    parents: Vec<SymbolHandle>,
    requirements: Vec<RequirementCatalogEntry>,
}

#[derive(Clone)]
struct RequirementCatalogEntry {
    declaring_trait: SymbolHandle,
    declaring_trait_name: DiagnosticName,
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    is_default: bool,
}

#[derive(Clone)]
struct MachineCatalogEntry {
    symbol: SymbolHandle,
    name: DiagnosticName,
    states: Vec<(DiagnosticName, SymbolHandle)>,
}

pub(crate) fn normalize_closed_conformance_blocks(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let trait_catalog = build_trait_catalog(program);
    let machine_catalog = build_machine_catalog(program);
    let normalized = program
        .conformances
        .iter()
        .map(|conformance| match &conformance.implementation {
            ConformanceImplementation::LegacyAttachedMachines => {
                Ok(ConformanceImplementation::LegacyAttachedMachines)
            }
            ConformanceImplementation::Closed { rows } => normalize_one(
                conformance.type_name.as_str(),
                conformance.trait_name.as_str(),
                rows,
                &trait_catalog,
                &machine_catalog,
            )
            .map(|rows| ConformanceImplementation::Closed { rows }),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut normalized = normalized.into_iter();
    program.conformances.for_each_mut(|conformance| {
        conformance.implementation = normalized
            .next()
            .expect("one normalized implementation per conformance");
    });
    Ok(())
}

fn build_trait_catalog(program: &SymbolResolvedTrees) -> Vec<TraitCatalogEntry> {
    program
        .traits
        .iter()
        .map(|trait_definition| TraitCatalogEntry {
            symbol: trait_definition.symbol,
            name: trait_definition.name.clone(),
            parents: program
                .trait_requirements(trait_definition.requires)
                .iter()
                .map(|parent| parent.symbol)
                .collect(),
            requirements: program
                .trait_machine_signatures(trait_definition.machines)
                .iter()
                .map(|requirement| RequirementCatalogEntry {
                    declaring_trait: trait_definition.symbol,
                    declaring_trait_name: trait_definition.name.clone(),
                    requirement: requirement.symbol,
                    requirement_name: requirement.name.clone(),
                    is_default: requirement.is_default,
                })
                .collect(),
        })
        .collect()
}

fn build_machine_catalog(program: &SymbolResolvedTrees) -> Vec<MachineCatalogEntry> {
    program
        .machines
        .iter()
        .map(|machine| MachineCatalogEntry {
            symbol: machine.symbol,
            name: machine.name.clone(),
            states: program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| {
                    let state = program.machine_state(*handle);
                    (state.name.clone(), state.symbol)
                })
                .collect(),
        })
        .collect()
}

fn normalize_one(
    subject_name: &str,
    trait_name: &str,
    authored_rows: &[ConformanceRow],
    trait_catalog: &[TraitCatalogEntry],
    machine_catalog: &[MachineCatalogEntry],
) -> Result<Vec<ConformanceRow>, Diagnostic> {
    let root = trait_catalog
        .iter()
        .find(|entry| entry.name.as_str() == trait_name)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            ))
        })?;
    let mut requirements = Vec::new();
    collect_requirement_closure(
        root.symbol,
        trait_catalog,
        &mut Vec::new(),
        &mut requirements,
    );

    let mut normalized = Vec::new();
    for authored in authored_rows {
        let candidates = requirements
            .iter()
            .filter(|requirement| {
                requirement.requirement_name == authored.requirement_name
                    && (authored.declaring_trait_name.as_str().is_empty()
                        || requirement.declaring_trait_name == authored.declaring_trait_name)
            })
            .collect::<Vec<_>>();
        let requirement = match candidates.as_slice() {
            [] => {
                let qualified = if authored.declaring_trait_name.as_str().is_empty() {
                    authored.requirement_name.as_str().to_owned()
                } else {
                    format!(
                        "{}::{}",
                        authored.declaring_trait_name, authored.requirement_name
                    )
                };
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` has no inherited requirement slot `{qualified}`"
                )));
            }
            [requirement] => *requirement,
            _ => {
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` member `{}` is ambiguous across inherited traits; use `DeclaringTrait::{}`",
                    authored.requirement_name, authored.requirement_name
                )));
            }
        };

        if normalized.iter().any(|row: &ConformanceRow| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` fills `{}::{}` more than once",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }

        let (machine, state) = resolve_realization(authored, machine_catalog).ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` row `{}::{}` names no exact callable realization `{}`",
                requirement.declaring_trait_name,
                requirement.requirement_name,
                authored.realization_name,
            ))
        })?;
        let mut row = authored.clone();
        row.declaring_trait = requirement.declaring_trait;
        row.declaring_trait_name = requirement.declaring_trait_name.clone();
        row.requirement = requirement.requirement;
        row.realization_machine = machine;
        row.realization_state = state;
        normalized.push(row);
    }

    for requirement in requirements {
        if normalized.iter().any(|row| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            continue;
        }
        if !requirement.is_default {
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` is incomplete: missing `{}::{}`",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }
        normalized.push(ConformanceRow {
            declaring_trait: requirement.declaring_trait,
            declaring_trait_name: requirement.declaring_trait_name.clone(),
            requirement: requirement.requirement,
            requirement_name: requirement.requirement_name.clone(),
            realization_machine: SymbolHandle::invalid(),
            realization_state: SymbolHandle::invalid(),
            realization_name: DiagnosticName::generated(format!(
                "{}::{}#default",
                requirement.declaring_trait_name, requirement.requirement_name
            )),
            source: ConformanceRowSource::TraitDefault,
        });
    }

    normalized.sort_by_key(|row| {
        (
            row.declaring_trait.arena_index(),
            row.requirement.arena_index(),
        )
    });
    Ok(normalized)
}

fn collect_requirement_closure(
    trait_symbol: SymbolHandle,
    catalog: &[TraitCatalogEntry],
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<RequirementCatalogEntry>,
) {
    if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
        return;
    }
    visited.push(trait_symbol);
    let Some(entry) = catalog.iter().find(|entry| entry.symbol == trait_symbol) else {
        return;
    };
    for requirement in &entry.requirements {
        if !output.iter().any(|existing| {
            existing.declaring_trait == requirement.declaring_trait
                && existing.requirement == requirement.requirement
        }) {
            output.push(requirement.clone());
        }
    }
    for parent in &entry.parents {
        collect_requirement_closure(*parent, catalog, visited, output);
    }
}

fn resolve_realization(
    row: &ConformanceRow,
    catalog: &[MachineCatalogEntry],
) -> Option<(SymbolHandle, SymbolHandle)> {
    let machine = catalog
        .iter()
        .find(|machine| machine.name == row.realization_name)?;
    let leaf = machine
        .name
        .as_str()
        .rsplit_once("::")
        .map_or(machine.name.as_str(), |(_, leaf)| leaf);
    let state = machine
        .states
        .iter()
        .find(|(name, _)| name.as_str() == leaf)
        .or_else(|| {
            machine
                .states
                .iter()
                .find(|(name, _)| name.as_str() == "entry")
        })?;
    Some((machine.symbol, state.1))
}
