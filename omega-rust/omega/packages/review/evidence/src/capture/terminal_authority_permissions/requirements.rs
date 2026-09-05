//! Resolve schema rows only within the exact inherited declaration closure.

use super::declarations::rejected;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use symbols::SymbolHandle;

pub(super) fn resolve(
    compilation: &CheckedCompilation,
    root: SymbolHandle,
    schema: &ServiceSchema,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut declarations = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let definitions = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "service inheritance has no unique exact declaration",
            ));
        };
        declarations.push(*definition);
        pending.extend(
            compilation
                .trait_requirements(definition)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    schema
        .methods
        .iter()
        .map(|method| {
            let mut matches = Vec::new();
            for owner in &declarations {
                if compilation
                    .typed
                    .symbols
                    .symbol_package_identity(owner.symbol)
                    != method.requirement_owner_package_identity
                {
                    continue;
                }
                for requirement in compilation.trait_machine_signatures(owner) {
                    if compilation
                        .typed
                        .symbols
                        .symbol_package_identity(requirement.symbol)
                        == method.requirement_owner_package_identity
                        && compilation
                            .normalized_trait_requirement_overload_identity(owner, requirement)
                            .identity()
                            == method.requirement_identity
                    {
                        matches.push(requirement.symbol);
                    }
                }
            }
            let [requirement] = matches.as_slice() else {
                return Err(rejected(
                    "service row has no unique exact inherited requirement",
                ));
            };
            Ok(*requirement)
        })
        .collect()
}
