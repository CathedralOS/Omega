use super::shared::trait_definition_by_symbol;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::trait_definition::TraitDefinition;

pub(crate) fn validate_trait_requirements(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        for requirement in program.trait_requirements(trait_definition) {
            if trait_definition_by_symbol(program, requirement.symbol).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requires unknown trait `{}`",
                    trait_definition.name, requirement.name
                )));
            }
        }
    }

    let mut reported_cycle_symbols = Vec::new();
    for trait_definition in program.traits() {
        let mut path = Vec::new();
        validate_trait_requirement_cycles(
            program,
            trait_definition,
            &mut path,
            &mut reported_cycle_symbols,
            diagnostics,
        );
    }
}

fn validate_trait_requirement_cycles(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    path: &mut Vec<SymbolHandle>,
    reported_cycle_symbols: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reported_cycle_symbols
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    if let Some(cycle_start) = path
        .iter()
        .position(|symbol| *symbol == trait_definition.symbol)
    {
        let cycle_symbols = path[cycle_start..]
            .iter()
            .copied()
            .chain(std::iter::once(trait_definition.symbol))
            .collect::<Vec<_>>();
        let mut cycle = path[cycle_start..]
            .iter()
            .filter_map(|symbol| trait_definition_by_symbol(program, *symbol))
            .map(|trait_definition| trait_definition.name.to_string())
            .collect::<Vec<_>>();
        cycle.push(trait_definition.name.to_string());

        diagnostics.push(Diagnostic::error(format!(
            "trait requirement cycle detected: {}",
            cycle.join(" -> ")
        )));
        reported_cycle_symbols.extend(cycle_symbols);
        return;
    }

    path.push(trait_definition.symbol);
    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_trait_requirement_cycles(
            program,
            required_trait,
            path,
            reported_cycle_symbols,
            diagnostics,
        );
    }
    path.pop();
}
