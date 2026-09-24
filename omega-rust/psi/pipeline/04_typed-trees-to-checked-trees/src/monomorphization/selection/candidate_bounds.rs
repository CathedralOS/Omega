//! Approved type bounds and candidate conformance bound validation.

use crate::monomorphization::body_rewriting::{
    forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
};
use crate::monomorphization::selection::static_bindings::same_type_identity;
use crate::monomorphization::{
    Candidate, Diagnostic, StaticMachineArgument, SymbolHandle, SymbolKind, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees,
};

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

pub(crate) fn validate_candidate_conformance_bounds(
    program: &TypedTrees,
    candidate: &mut Candidate,
) -> Result<(), Vec<Diagnostic>> {
    candidate.inferred_conformance_arguments.clear();
    candidate.selected_bound_applications.clear();
    let mut diagnostics = Vec::new();
    for bound in &candidate.template.conformance_bounds {
        let Some(parameter_index) = candidate
            .template
            .type_parameters
            .iter()
            .position(|(symbol, _)| *symbol == bound.subject)
        else {
            continue;
        };
        let Some(binding) = candidate.type_bindings[parameter_index] else {
            continue;
        };
        let Some(type_name) = concrete_data_type_name(program, binding) else {
            diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{}`, which is not a nominal data type and cannot satisfy conformance bound `{}`",
                candidate.template.template_name,
                bound.subject_name,
                program.display_type_reference(binding),
                bound.carrier_name,
            )));
            continue;
        };
        let type_identity = program.display_type_reference(binding);

        if let Some(binder) = bound.binder {
            let Some(evidence_index) = candidate
                .template
                .evidence_parameters
                .iter()
                .position(|parameter| parameter.binder == Some(binder))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` lost explicit conformance binder `{}` from its specialization telescope",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                )));
                continue;
            };
            let Some(selected_binding) = candidate.evidence_bindings[evidence_index].as_ref()
            else {
                continue;
            };
            let selected_symbol = selected_binding.symbol;
            let Some(selected) = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == selected_symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to a symbol that is not a package-scoped conformance",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
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
            // A transparent refinement is a structural bound over an
            // existing base conformance, never a nominal satisfaction
            // target: "a static evidence binder may require it and receive
            // an explicitly selected `Logger` conformance whose complete
            // contract fits" (spec, conformances.md, Transparent
            // refinements). Compare identity against the resolved base, then
            // -- in the same step, because the resolution alone would admit
            // a non-fitting map -- check the selected rows against the
            // refinement's clauses.
            let carrier =
                crate::monomorphization::selection::resolve_bound_carrier(program, bound.carrier);
            let expected_trait = program
                .traits()
                .iter()
                .find(|definition| definition.symbol == carrier.base);
            if application.subject_identity.as_deref() != Some(type_identity.as_str())
                || expected_trait
                    .is_none_or(|definition| application.trait_definition != definition.symbol)
                || !conformance_application_arguments_match_candidate(
                    program,
                    candidate,
                    bound,
                    &application,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` cannot bind `{}` to conformance `{}`: expected a complete `{type_identity} satisfies {}` map with the instantiated trait arguments",
                    candidate.template.template_name,
                    bound
                        .binder_name
                        .as_ref()
                        .map_or("<missing>", |name| name.as_str()),
                    selected
                        .alias
                        .as_ref()
                        .map_or("<unnamed>", |name| name.as_str()),
                    bound.carrier_name,
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
                        bound
                            .binder_name
                            .as_ref()
                            .map_or("<missing>", |name| name.as_str()),
                    ),
                );
            }
            continue;
        }

        if let Some(selected) = &bound.selected_conformance {
            let application = match close_candidate_bound_application(program, candidate, selected)
            {
                Ok(application) => application,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if application.subject_identity.as_deref() != Some(type_identity.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "generic machine `{}` binds `{}` to `{type_name}`, but named conformance `{}::{}` belongs to `{}`",
                    candidate.template.template_name,
                    bound.subject_name,
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    application.subject_identity.as_deref().unwrap_or("<subjectless>"),
                )));
                continue;
            }
            candidate.selected_bound_applications.push(application);
            continue;
        }

        let matches = program
            .conformances()
            .iter()
            .filter(|conformance| {
                conformance
                    .carrier_name()
                    .is_some_and(|carrier| carrier.as_str() == type_name)
                    && conformance.trait_name == bound.carrier_name
                    && conformance_arguments_match_candidate(program, candidate, bound, conformance)
            })
            .map(|conformance| conformance.symbol)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [selected] => candidate.inferred_conformance_arguments.push(*selected),
            [] => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has no nominal conformance to `{}`",
                candidate.template.template_name, bound.subject_name, bound.carrier_name,
            ))),
            matches => diagnostics.push(Diagnostic::error(format!(
                "generic machine `{}` binds `{}` to `{type_name}`, which has {count} conformances to `{}`; select one with `where {} satisfies {type_name}::Name`",
                candidate.template.template_name,
                bound.subject_name,
                bound.carrier_name,
                bound.subject_name,
                count = matches.len(),
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn close_candidate_bound_application(
    program: &TypedTrees,
    candidate: &Candidate,
    selected: &StaticMachineArgument,
) -> Result<typed_trees::typed_trees::ClosedConformanceApplication, Diagnostic> {
    let rewrites = forwarded_static_argument_rewrites(program, candidate);
    let mut applications = [selected.clone()];
    substitute_forwarded_machine_arguments(&mut applications, &rewrites, &[]);
    crate::conformance::conformance_applications::close_conformance_application(
        program,
        &applications[0],
    )
}

pub(crate) fn concrete_data_type_name(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => concrete_data_type_name(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid() && program.symbols.get(*symbol).kind == SymbolKind::Data =>
        {
            Some(name.as_str())
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if base_symbol.is_valid()
            && program.symbols.get(*base_symbol).kind == SymbolKind::Data =>
        {
            Some(base_name.as_str())
        }
        _ => None,
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
            binding.map(|binding| (*symbol, program.display_type_reference(binding)))
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
    let carrier = crate::monomorphization::selection::resolve_bound_carrier(program, bound.carrier);
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

pub(crate) fn conformance_arguments_match_candidate(
    program: &TypedTrees,
    candidate: &Candidate,
    bound: &typed_trees::machine::GenericConformanceBound,
    conformance: &typed_trees::trait_definition::Conformance,
) -> bool {
    let actual = program
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    actual.len() == bound.arguments.len()
        && bound
            .arguments
            .iter()
            .zip(actual.iter())
            .all(|(required, actual)| {
                let required = candidate
                    .template
                    .type_parameters
                    .iter()
                    .zip(candidate.type_bindings.iter())
                    .find_map(|((symbol, _), binding)| {
                        let TypeReferenceNode::Named {
                            symbol: required_symbol,
                            ..
                        } = program.type_reference_table.type_reference(*required)
                        else {
                            return None;
                        };
                        (*symbol == *required_symbol)
                            .then_some(binding.as_ref().copied())
                            .flatten()
                    })
                    .unwrap_or(*required);
                same_type_identity(program, required, *actual)
            })
}
