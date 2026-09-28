//! Resolve authored product target families once, while declaration names
//! still participate in resolution. Later stages join bodies by these handles.

use source::DependencyScope;
use symbol_resolved_trees::SymbolResolvedTrees;

pub(crate) fn bind(program: &mut SymbolResolvedTrees) {
    let mut declarations = program
        .machines
        .iter()
        .enumerate()
        .filter(|(_, machine)| {
            machine.authored_target.is_some()
                && machine
                    .generic_data_origin
                    .template_source
                    .as_str()
                    .is_empty()
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
        // Admission retains every sibling but clears only the canonical
        // body's marker. Do not import Omega's target catalog to choose one
        // here. Unknown-only families remain inert.
        // Equal spellings in distinct modules or packages are not one
        // family. Resolve that association here once; sibling calls and
        // typing consume the canonical symbol, not a second name search.
        for &member in family {
            let declaration = &program.machines[member];
            let canonical = family
                .iter()
                .map(|&candidate| &program.machines[candidate])
                .find(|candidate| {
                    candidate.target.is_none()
                        && program.symbols.symbol_module(candidate.symbol)
                            == program.symbols.symbol_module(declaration.symbol)
                        && program.symbols.same_source_package(
                            candidate.name.source_span(),
                            declaration.name.source_span(),
                        )
                })
                .map(|machine| machine.symbol);
            if let Some(canonical) = canonical {
                program.machines[member].target_family = canonical;
            }
        }
        remaining = rest;
    }
}
