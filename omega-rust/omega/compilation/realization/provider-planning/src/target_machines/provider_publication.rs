// We close authored selections into source-free target proposals. Publication
// consumes retained authored inventories and ordinary whole-provider selection
// without deriving candidates or settling defaults. `provider_selection_roots`
// enumerates the complete interface roster for selections, including inherited
// boundary methods, so a whole-provider selection never turns into an
// unauthorized partial composition.

use crate::{ProviderPlanningContext, ProviderSelection, ProviderSelectionSubject};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub use provider_selection::PreparedTargetProviderPublication;

pub fn provider_selection_roots<'selection>(
    context: ProviderPlanningContext<'_>,
    selections: impl IntoIterator<Item = &'selection ProviderSelection>,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut roots = Vec::new();
    for selection in selections {
        match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity) => {
                let definition = context
                    .typed
                    .trait_definition_by_symbol(identity.symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "provider selection lost its exact trait declaration",
                        )]
                    })?;
                typed_trees::trait_definition::AppliedTraitScope::abstract_scope(definition).visit(
                    context.typed,
                    &mut |scope| {
                        roots.extend(
                            context
                                .typed
                                .trait_machine_signatures(scope.declaration)
                                .iter()
                                .map(|row| row.symbol),
                        );
                    },
                );
            }
            ProviderSelectionSubject::BoundaryRequirement(identity) => roots.push(identity.symbol),
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                roots.extend(family.coordinates().iter().map(|row| row.symbol));
            }
        }
    }
    if roots
        .iter()
        .any(|symbol| !symbol.is_valid() || !context.contains(*symbol))
    {
        return Err(vec![Diagnostic::error(
            "provider selection lost its exact product requirement owner",
        )]);
    }
    roots.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    roots.dedup();
    Ok(roots)
}
