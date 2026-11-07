//! Exact outer applications own the complete original requirement roster.
//! Source carriers and authored satisfaction edges supply operands, never the
//! selected provider. Open scopes remain ordinary source-checking obligations.

use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) struct Demand {
    pub requirement: symbols::SymbolHandle,
    pub argument_owner: symbols::SymbolHandle,
    pub environment: typed_trees::trait_definition::TraitApplicationEnvironment,
}

pub(super) fn collect(program: &TypedTrees) -> Vec<Demand> {
    let mut scopes = Vec::<AppliedTraitScope<'_>>::new();
    for root in program.applied_trait_roots() {
        // Keep the complete supplying environment through materialization.
        // Projecting a compound operand to its existing handle does not close
        // its children. Executable preparation checks the materialized tuple
        // once, before creating any instance; symbolic roots issue none.
        // Inventory retains regions; preparation establishes operand closure.
        // Neither operation admits a selected provider or runtime installation.
        root.visit(program, &mut |scope| {
            if !scopes
                .iter()
                .any(|existing| existing.same_application(program, scope))
            {
                scopes.push(scope.clone());
            }
        });
    }
    scopes
        .into_iter()
        .flat_map(|scope| {
            program
                .trait_machine_signatures(scope.declaration)
                .iter()
                .map(move |signature| Demand {
                    requirement: signature.symbol,
                    argument_owner: scope.argument_owner,
                    environment: scope.environment(),
                })
        })
        .collect()
}
