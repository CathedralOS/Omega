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
        // Executable preparation still needs closed operands. Inspect the
        // supplying telescope, not inherited binder leaves that its complete
        // scope will resolve. Region-bearing admission remains fail-closed.
        let arguments = program
            .trait_type_parameters(root.declaration)
            .iter()
            .map(|parameter| root.parameter_argument(program, parameter.symbol))
            .collect::<Vec<_>>();
        if !root.declaration.lifetime_parameters.is_empty()
            || program
                .require_closed_application_operands(&arguments)
                .is_err()
        {
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
