//! Approved type bounds and candidate evidence binder validation.

use crate::monomorphization::{Candidate, Diagnostic, SymbolHandle, TypedTrees};

pub(crate) fn approved_type_bounds(program: &TypedTrees, candidates: &[Candidate]) -> Vec<bool> {
    let mut symbol_diagnostics = Vec::new();
    let symbols = validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
    candidates
        .iter()
        .map(|candidate| {
            candidate
                .template
                .parameter_bounds
                .iter()
                .zip(candidate.type_bindings.iter())
                .all(|(bounds, binding)| {
                    let Some(binding) = binding else {
                        return true;
                    };
                    let Some(unwrapped) = validation::unwrapped_type_reference(program, *binding)
                    else {
                        return false;
                    };
                    bounds.iter().all(|property| {
                        validation::type_satisfies_declared_property(
                            program,
                            &symbols,
                            &[],
                            unwrapped,
                            *property,
                        )
                    })
                })
        })
        .collect()
}

/// Every evidence binder's argument must be a named conformance whose trait
/// application equals the binder's after substituting the candidate's
/// bindings. The data types live in those trait arguments; no binding is a
/// subject that the conformance must belong to.
pub(crate) fn validate_candidate_conformance_bounds(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for (evidence_index, bound) in candidate.template.evidence_parameters.iter().enumerate() {
        let Some(selected_binding) = candidate.evidence_bindings[evidence_index].as_ref() else {
            continue;
        };
        let selected_symbol = selected_binding.symbol;
        let Some(selected) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == selected_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to a symbol that is not a named conformance",
                candidate.template.template_name, bound.binder_name,
            )));
            continue;
        };
        let application =
            match crate::conformance::conformance_applications::close_conformance_application(
                program,
                selected_binding,
            ) {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        // A transparent refinement is a structural bound over an existing
        // base conformance, never a nominal satisfaction target: "a static
        // evidence binder may require it and receive an explicitly selected
        // `Logger` conformance whose complete contract fits" (spec,
        // conformances.md, Transparent refinements). Compare identity against
        // the resolved base, then -- in the same step, because the resolution
        // alone would admit a non-fitting map -- check the selected rows
        // against the refinement's clauses.
        let carrier =
            crate::monomorphization::selection::resolve_bound_carrier(program, bound.trait_symbol);
        let expected_trait = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == carrier.base);
        if expected_trait.is_none_or(|definition| application.trait_definition != definition.symbol)
            || !conformance_application_arguments_match_candidate(
                program,
                candidate,
                bound,
                &application,
            )
        {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{}` implementation with the instantiated trait arguments",
                candidate.template.template_name,
                bound.binder_name,
                selected
                    .alias
                    .as_ref()
                    .map_or("<unnamed>", |name| name.as_str()),
                bound.trait_name,
            )));
            continue;
        }
        if let Some(refinement) = carrier.refinement {
            diagnostics.extend(
                crate::monomorphization::selection::refinement_fit_diagnostics(
                    program,
                    refinement,
                    selected,
                    candidate.template.template_name.as_str(),
                    bound.binder_name.as_str(),
                ),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn conformance_application_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &typed_trees::machine::GenericConformanceBound,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> bool {
    let substitutions = candidate
        .template
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((symbol, _), binding)| {
            binding.map(|binding| {
                (
                    *symbol,
                    program.normalized_type_identity(binding).into_string(),
                )
            })
        })
        .chain(
            candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .filter_map(|((symbol, _, _), binding)| {
                    binding.map(|binding| (*symbol, program.display_type_reference(binding)))
                }),
        )
        .collect::<Vec<_>>();
    let machine_lifetimes = program.machines()[candidate.template.machine_index]
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.as_str().to_owned(),
                "__ordinary_call_region".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let identity = |handle, substitutions: &[(SymbolHandle, String)]| {
        crate::conformance::conformance_applications::substituted_type_identity_with_lifetimes(
            program,
            handle,
            substitutions,
            &machine_lifetimes,
        )
    };

    // A refinement carrier is written with the REFINEMENT's parameters
    // (`Flipped<bool, i32>`), while the conformance being matched satisfies
    // the BASE (`Pair<i32, bool>`). The `= Base<...>` head carries the map
    // between them, so instantiate `refines.arguments` with the binder's
    // arguments before comparing. Heads that reorder or partially apply their
    // parameters agree only after that instantiation; comparing the binder's
    // own arguments against a base application comes out right by accident
    // only for a head that passes its parameters through in order.
    let carrier =
        crate::monomorphization::selection::resolve_bound_carrier(program, bound.trait_symbol);
    let required = match carrier
        .refinement
        .and_then(|refinement| refinement.refines.as_ref().map(|base| (refinement, base)))
    {
        Some((refinement, base)) => {
            let head = program
                .data_type_parameters
                .span_or_empty(refinement.type_parameters)
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(
                    bound
                        .arguments
                        .iter()
                        .map(|argument| identity(*argument, &substitutions)),
                )
                .collect::<Vec<_>>();
            program
                .type_reference_table
                .type_reference_handles(base.arguments)
                .iter()
                .map(|argument| identity(*argument, &head))
                .collect::<Vec<_>>()
        }
        None => bound
            .arguments
            .iter()
            .map(|argument| identity(*argument, &substitutions))
            .collect::<Vec<_>>(),
    };

    application.trait_arguments.len() == required.len()
        && required
            .iter()
            .zip(application.trait_arguments.iter())
            .all(|(required, actual)| {
                let actual = application.lifetime_arguments.iter().fold(
                    actual.clone(),
                    |identity, lifetime| {
                        identity.replace(&format!("'{lifetime}"), "'__ordinary_call_region")
                    },
                );
                *required == actual
            })
}
