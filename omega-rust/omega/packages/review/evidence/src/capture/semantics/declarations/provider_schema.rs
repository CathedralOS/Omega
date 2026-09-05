//! Exact declaring-schema joins for selected provider requirements.

use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use provider_planning::plans::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

/// The selected schema owns provider/calling policy. An inherited requirement
/// retains its own declaring schema; only exact reachable symbols establish
/// that association. Repeated paths to the same declaration do not add owners.
pub(crate) fn provider_requirement_schema(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
) -> Result<ProviderSchemaDeclaration, Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(root) = schema else {
        return Ok(schema);
    };
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut owners = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let definitions = compilation
            .traits()
            .iter()
            .filter(|candidate| candidate.symbol == symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "selected schema has no unique exact declaring trait",
            ));
        };
        let matching = compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|candidate| candidate.symbol == requirement)
            .count();
        if matching > 1 {
            return Err(rejected("selected schema repeats its exact requirement"));
        }
        if matching == 1 {
            owners.push(symbol);
        }
        pending.extend(
            compilation
                .trait_requirements(definition)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    let [owner] = owners.as_slice() else {
        return Err(rejected(
            "selected schema does not inherit one exact requirement declaration",
        ));
    };
    Ok(ProviderSchemaDeclaration::BoundaryTrait(*owner))
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected provider requirement rejects {reason}"
    ))]
}
