//! Exact outer applications own the complete original requirement roster.
//! Source carriers and authored satisfaction edges supply operands, never the
//! selected provider. Open scopes remain ordinary source-checking obligations.

use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) struct Demand {
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub substitutions: Vec<(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
    )>,
    pub scope_starts: Vec<usize>,
}

pub(super) fn collect(program: &TypedTrees) -> Vec<Demand> {
    let mut roots = program
        .traits()
        .iter()
        .filter(|owner| program.trait_type_parameters(owner).is_empty())
        .map(|owner| (owner.symbol, Vec::new()))
        .chain(program.conformances().iter().map(|edge| {
            (
                edge.trait_symbol,
                program
                    .type_reference_table
                    .type_reference_handles(edge.arguments)
                    .to_vec(),
            )
        }))
        .chain(program.machines().iter().flat_map(|machine| {
            program
                .machine_trait_conformances(machine)
                .iter()
                .map(|edge| {
                    (
                        edge.symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(edge.arguments)
                            .to_vec(),
                    )
                })
        }))
        .collect::<Vec<_>>();
    for (reference, _) in program.type_reference_table.type_reference_rows() {
        if let Ok(Some(carrier)) =
            typed_trees::service::classify_exact_bound_service_carrier(program, reference)
        {
            roots.push((carrier.requirement, carrier.arguments(program).to_vec()));
        }
    }
    let mut scopes = Vec::<AppliedTraitScope<'_>>::new();
    for (owner, arguments) in roots {
        let Some(definition) = program.trait_definition_by_symbol(owner) else {
            continue;
        };
        if !definition.lifetime_parameters.is_empty()
            || program
                .require_closed_application_operands(&arguments)
                .is_err()
        {
            continue;
        }
        let Some(root) = AppliedTraitScope::new(program, definition, &arguments) else {
            continue;
        };
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
                    substitutions: scope.substitutions.clone(),
                    scope_starts: scope.substitution_scope_starts.clone(),
                })
        })
        .collect()
}
