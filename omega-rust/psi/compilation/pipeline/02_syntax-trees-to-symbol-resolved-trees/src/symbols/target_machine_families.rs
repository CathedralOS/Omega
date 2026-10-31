//! Resolve authored product target families once, while declaration names
//! still participate in resolution. Later stages join bodies by these handles.

use diagnostics::Diagnostic;
use source::DependencyScope;
use symbol_resolved_trees::SymbolResolvedTrees;

mod contracts;

pub(crate) use contracts::prepare_premises;

pub(crate) fn bind(program: &mut SymbolResolvedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut declarations = program
        .machines
        .iter()
        .enumerate()
        .filter(|(_, machine)| {
            machine.authored_target.is_some()
                && program
                    .symbols
                    .source_file(machine.name.source_span())
                    .is_none_or(|source| source.dependency_scope == DependencyScope::Product)
        })
        .map(|(machine_index, _)| machine_index)
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| {
        program.machines[*left]
            .name
            .as_str()
            .cmp(program.machines[*right].name.as_str())
    });
    let mut remaining = declarations.as_slice();
    while let Some(&first) = remaining.first() {
        let family_length = remaining
            .iter()
            .take_while(|&&member| {
                program.machines[member].name.as_str() == program.machines[first].name.as_str()
            })
            .count();
        let (family, rest) = remaining.split_at(family_length);
        // Every authored member, including a closed generic attachment, joins
        // its independent interface. Exact carrier symbols distinguish closed
        // applications; template provenance is not a reason to omit bodies.
        // No target catalog participates in this portable declaration join.
        // Equal spellings in distinct modules or packages are not one
        // family. Resolve that association here once; sibling calls and
        // typing consume the canonical symbol, not a second name search.
        for &member in family {
            let declaration = &program.machines[member];
            let members = family
                .iter()
                .map(|&candidate| &program.machines[candidate])
                .filter(|candidate| {
                    program.symbols.symbol_module(candidate.symbol)
                        == program.symbols.symbol_module(declaration.symbol)
                        && candidate.attached_data_symbol == declaration.attached_data_symbol
                        && program.symbols.same_source_package(
                            candidate.name.source_span(),
                            declaration.name.source_span(),
                        )
                });
            // Seeded resolution includes the retained base here. Duplicate
            // admission must cross that boundary too, without an Omega-owned
            // source-name roster tracking generated declarations in parallel.
            if members
                .clone()
                .filter(|candidate| {
                    candidate
                        .authored_target
                        .as_ref()
                        .map(|target| target.as_str())
                        == declaration
                            .authored_target
                            .as_ref()
                            .map(|target| target.as_str())
                })
                .count()
                > 1
            {
                return Err(vec![Diagnostic::error(format!(
                    "machine `{}` is implemented twice for `{}` -- a target supplies exactly one implementation of a contract machine",
                    declaration.name,
                    declaration.authored_target.as_ref().map_or("", |target| target.as_str()),
                )).with_source_span(declaration.name.source_span())]);
            }
            let mut interfaces = program.machines.iter().filter(|candidate| {
                candidate.target_family_interface
                    && candidate.name.as_str() == declaration.name.as_str()
                    && program.symbols.symbol_module(candidate.symbol)
                        == program.symbols.symbol_module(declaration.symbol)
                    && candidate.attached_data_symbol == declaration.attached_data_symbol
                    && program.symbols.same_source_package(
                        candidate.name.source_span(),
                        declaration.name.source_span(),
                    )
            });
            let Some(interface) = interfaces.next() else {
                return Err(vec![
                    Diagnostic::error(format!(
                        "target machine `{}` has no portable family interface",
                        declaration.name,
                    ))
                    .with_source_span(declaration.name.source_span()),
                ]);
            };
            let canonical = interface.symbol;
            if interfaces.next().is_some() {
                return Err(vec![
                    Diagnostic::error(format!(
                        "target machine `{}` has multiple portable family interfaces",
                        declaration.name,
                    ))
                    .with_source_span(declaration.name.source_span()),
                ]);
            }
            if declaration.target_family.is_valid() && declaration.target_family != canonical {
                return Err(vec![
                    Diagnostic::error("resolution changed a retained target-machine family")
                        .with_source_span(declaration.name.source_span()),
                ]);
            }
            program.machines[member].target_family = canonical;
        }
        remaining = rest;
    }
    if let Some(machine) = program.machines.iter().find(|machine| {
        machine.target_family_interface
            && machine.target_family.is_valid()
            && machine.target_family != machine.symbol
    }) {
        return Err(vec![
            Diagnostic::error("resolution changed a retained target-machine interface")
                .with_source_span(machine.name.source_span()),
        ]);
    }
    program.machines.for_each_mut(|machine| {
        if machine.target_family_interface {
            machine.target_family = machine.symbol;
        }
    });
    Ok(())
}
