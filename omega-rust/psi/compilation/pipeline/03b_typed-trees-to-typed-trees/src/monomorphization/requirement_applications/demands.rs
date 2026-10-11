//! Exact outer applications own the complete original requirement roster.
//! Source carriers and authored satisfaction edges supply operands, never the
//! selected provider. Open scopes remain ordinary source-checking obligations.

use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) struct Demand {
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub argument_owner: symbols::SymbolHandle,
    pub substitutions: Vec<(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )>,
    pub scope_starts: Vec<usize>,
}

pub(super) fn collect(program: &TypedTrees) -> Vec<Demand> {
    let mut scopes = Vec::<AppliedTraitScope<'_>>::new();
    for root in program.applied_trait_roots() {
        // Keep the complete supplying environment through materialization.
        // Projecting a compound operand to its existing handle does not close
        // its children. Executable preparation checks the materialized tuple
        // once, before creating any instance; symbolic roots issue none.
        // Region-bearing admission remains fail-closed.
        if !root.declaration.lifetime_parameters.is_empty() {
            continue;
        }
        root.visit(program, &mut |scope| {
            if scope.declaration.is_boundary
                && scope.declaration.lifetime_parameters.is_empty()
                && !scopes
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
                    declaring_trait: scope.declaration.symbol,
                    requirement: signature.symbol,
                    argument_owner: scope.argument_owner,
                    substitutions: scope.substitutions.clone(),
                    scope_starts: scope.substitution_scope_starts.clone(),
                })
        })
        .collect()
}
