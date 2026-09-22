//! Matching a machine state against a trait signature: effect ceilings,
//! parameter matches, trait type bindings, lifetimes and fixed array
//! lengths.

use crate::value_custody::type_references::{type_reference_label, type_references_match};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Count the trailing parameters a specialized instance gained by realizing
/// runtime-bound `Value` binders as ordinary parameters. The specialization
/// cloner appends one parameter per runtime-bound slot after the template
/// state's authored parameters, so the tail is `instance arity - template
/// arity` on the corresponding state; every tail parameter carries a declared
/// `Value` carrier in binder order. Zero also reports a non-instance machine
/// or a tail that cannot be realized subjects: callers then apply the
/// ordinary strict arity check.
fn specialized_instance_realized_tail(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> usize {
    let Some(application) = program
        .machine_specializations
        .iter()
        .find(|application| application.instance == machine.symbol)
    else {
        return 0;
    };
    let Some(template) = program
        .machines()
        .iter()
        .find(|template| template.symbol == application.template)
    else {
        return 0;
    };
    // Cloned states keep template order, so the checked state's positional
    // counterpart in the template supplies the authored parameter count.
    let Some(state_ordinal) = program
        .machine_states(machine)
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)
    else {
        return 0;
    };
    let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
        return 0;
    };
    let realized = program
        .state_parameters(state)
        .len()
        .saturating_sub(program.state_parameters(template_state).len());
    if realized == 0 {
        return 0;
    }
    let carriers = program
        .data_type_parameters
        .span_or_empty(application.template_parameters)
        .iter()
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Value { type_reference } => Some(*type_reference),
            _ => None,
        })
        .collect::<Vec<_>>();
    let parameters = program.state_parameters(state);
    let trailing = &parameters[parameters.len() - realized..];
    // The tail must be realized `Value` subjects: their declared carriers
    // appear as an order-preserving subsequence of the template's `Value`
    // binder carriers.
    let mut cursor = 0usize;
    let mut matched = 0usize;
    for parameter in trailing {
        while cursor < carriers.len() {
            let carrier = carriers[cursor];
            cursor += 1;
            if type_references_match(program, parameter.type_reference, carrier) {
                matched += 1;
                break;
            }
        }
    }
    if matched == realized { realized } else { 0 }
}

pub(crate) fn validate_machine_state_satisfies_trait_signature_with_arguments(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    trait_lifetime_arguments: Option<&[u32]>,
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
    // A specialized instance appends one trailing ordinary parameter per
    // runtime-bound `Value` binder, after the template's authored parameters
    // in binder order (see monomorphization's realized-subject cloning). The
    // realized subjects carry a declared binder carrier at runtime rather
    // than declaring a new callable argument, so they are sliced off before
    // the arity and positional checks below.
    let realized_tail = specialized_instance_realized_tail(program, machine, state);
    if realized_tail > 0 {
        actual_parameters = &actual_parameters[..actual_parameters.len() - realized_tail];
    }
    // PRV4 self-forwarding adapters: a machine satisfying a BOUNDARY trait
    // requirement may take the trait ITSELF as one extra LEADING parameter
    // (adapter dispatch forwards the call's receiver there); the tail must
    // match the requirement exactly. This includes nominal provider adapters:
    // attachment groups rows into one provider closure but does not add a
    // runtime provider receiver. Plain traits keep the strict positional
    // match.
    if trait_definition.is_boundary
        && actual_parameters.len() == required_parameters.len() + 1
        && actual_parameters.first().is_some_and(|parameter| {
            let label = type_reference_label(program, parameter.type_reference);
            let leaf = label
                .rsplit("::")
                .next()
                .unwrap_or(label.as_str())
                .to_owned();
            let trait_leaf = trait_definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(trait_definition.name.as_str());
            leaf == trait_leaf
        })
    {
        actual_parameters = &actual_parameters[1..];
    }
    if actual_parameters.len() != required_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected {} parameter(s), got {}",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    let trait_type_parameters = program.trait_type_parameters(trait_definition);
    let requirement_type_parameters = program.state_signature_type_parameters(requirement);
    // A private application still carries its authored satisfaction edge, but
    // it does not declare a new universally quantified provider. The original
    // family remains live and receives ordinary universal refinement checking.
    // Rejoin only that exact family telescope here; keep checking this concrete
    // state's signature, laws, and effect ceilings below.
    let callable_machine = if !requirement_type_parameters.is_empty()
        && program.machine_type_parameters(machine).is_empty()
        && program
            .machine_trait_conformances(machine)
            .iter()
            .any(|edge| edge.requirement_symbol == requirement.symbol)
    {
        let mut applications = program
            .machine_specializations
            .iter()
            .filter(|application| application.instance == machine.symbol);
        if let Some(application) = applications.next() {
            let template = program.machines().iter().find(|template| {
                template.symbol == application.template
                    && template.symbol != machine.symbol
                    && template.type_parameters == application.template_parameters
                    && !program.machine_type_parameters(template).is_empty()
                    && program.machine_trait_conformances(template)
                        == program.machine_trait_conformances(machine)
            });
            let Some(template) = template.filter(|_| applications.next().is_none()) else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` generic satisfaction lost its exact original family and application",
                    machine.name,
                )));
                return;
            };
            template
        } else {
            machine
        }
    } else {
        machine
    };
    // A machine authored inline in a generic conformance closes over that
    // conformance name's telescope. Those captured parameters specialize the
    // row realization, but they are not callable parameters of the trait
    // requirement itself.
    let captured_parameter_symbols = program
        .conformances()
        .iter()
        .filter(|conformance| {
            program
                .closed_conformance_rows(conformance)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row.realization_machine == machine.symbol
                            && matches!(
                                row.source,
                                typed_trees::trait_definition::ConformanceRowSource::Inline
                                    | typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                            )
                    })
                })
        })
        .flat_map(|conformance| program.conformance_type_parameters(conformance))
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let actual_type_parameters = program
        .machine_type_parameters(callable_machine)
        .iter()
        .filter(|parameter| !captured_parameter_symbols.contains(&parameter.symbol))
        .cloned()
        .collect::<Vec<_>>();
    let indexed_law_telescope_groups = indexed_law_callable_telescope_groups(
        program,
        trait_definition,
        requirement,
        explicit_type_arguments,
    );
    let expected_callable_parameter_count = requirement_type_parameters.len()
        + indexed_law_telescope_groups
            .iter()
            .map(|group| group.len())
            .sum::<usize>();
    if expected_callable_parameter_count != actual_type_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: expected {} callable generic parameter(s), got {}",
            machine.name,
            trait_definition.name,
            requirement.name,
            expected_callable_parameter_count,
            actual_type_parameters.len(),
        )));
        return;
    }
    let effective_requirement_type_parameters = requirement_type_parameters
        .iter()
        .chain(
            indexed_law_telescope_groups
                .iter()
                .flat_map(|group| group.iter()),
        )
        .collect::<Vec<_>>();
    if let Some((index, (required, actual))) = effective_requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
        .find(|(_, (required, actual))| {
            !matches!(
                (&required.kind, &actual.kind),
                (TypeParameterKind::Type, TypeParameterKind::Type)
                    | (
                        TypeParameterKind::Const { .. },
                        TypeParameterKind::Const { .. }
                    )
                    | (
                        TypeParameterKind::Value { .. },
                        TypeParameterKind::Value { .. }
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        TypeParameterKind::Machine { .. }
                    )
            )
        })
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} has incompatible kinds (`{}` versus `{}`)",
            machine.name,
            trait_definition.name,
            requirement.name,
            index,
            required.name,
            actual.name,
        )));
        return;
    }
    let mut required_generic_types = trait_type_parameters.to_vec();
    required_generic_types.extend(
        requirement_type_parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .cloned(),
    );
    let generic_type_parameters = required_generic_types.iter().collect::<Vec<_>>();
    crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
        program,
        &format!(
            "provider machine `{}` for trait requirement `{}::{}`",
            machine.name, trait_definition.name, requirement.name
        ),
        requirement_type_parameters,
        &actual_type_parameters[..requirement_type_parameters.len()],
        &generic_type_parameters,
        diagnostics,
    );
    let mut actual_offset = requirement_type_parameters.len();
    for (representative, carrier_telescope) in indexed_law_telescope_groups.iter().enumerate() {
        let actual_end = actual_offset + carrier_telescope.len();
        let actual_group = &actual_type_parameters[actual_offset..actual_end];
        let carrier_generic_parameters = carrier_telescope.iter().collect::<Vec<_>>();
        crate::machine_calls::machine_parameters::validate_trait_callable_parameter_refinement(
            program,
            &format!(
                "provider machine `{}` indexed representative {} for proposition-law requirement `{}::{}`",
                machine.name,
                representative + 1,
                trait_definition.name,
                requirement.name
            ),
            carrier_telescope,
            actual_group,
            &carrier_generic_parameters,
            diagnostics,
        );
        actual_offset = actual_end;
    }
    let mut type_bindings = trait_type_parameters
        .iter()
        .zip(explicit_type_arguments.iter().copied())
        .map(|(parameter, actual)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            target: TraitTypeBindingTarget::Type(actual),
        })
        .collect::<Vec<_>>();
    type_bindings.extend(
        requirement_type_parameters
            .iter()
            .zip(actual_type_parameters.iter())
            .filter(|&(required, actual)| {
                matches!(
                    (&required.kind, &actual.kind),
                    (TypeParameterKind::Type, TypeParameterKind::Type)
                        | (
                            TypeParameterKind::Const { .. },
                            TypeParameterKind::Const { .. }
                        )
                        | (
                            TypeParameterKind::Value { .. },
                            TypeParameterKind::Value { .. }
                        )
                )
            })
            .map(|(required, actual)| TraitTypeBinding {
                parameter_symbol: required.symbol,
                parameter_name: required.name.as_str().to_owned(),
                target: TraitTypeBindingTarget::Parameter(actual.symbol),
            }),
    );
    for (index, (required, actual)) in requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
    {
        match (&required.kind, &actual.kind) {
            (TypeParameterKind::Type, TypeParameterKind::Type) => {
                if typed_trees::data::type_parameter_demands_stronger_properties(
                    required, actual,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} demands stronger type properties",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                }
            }
            (
                TypeParameterKind::Const {
                    type_reference: required_type,
                },
                TypeParameterKind::Const {
                    type_reference: actual_type,
                },
            )
            | (
                TypeParameterKind::Value {
                    type_reference: required_type,
                },
                TypeParameterKind::Value {
                    type_reference: actual_type,
                },
            ) if !type_references_match_with_trait_bindings(
                program,
                *actual_type,
                *required_type,
                &required_generic_types,
                &mut type_bindings,
            ) => diagnostics.push(Diagnostic::error(format!(
                "machine `{}` does not satisfy trait `{}` machine `{}`: callable const generic parameter {} has a different type",
                machine.name, trait_definition.name, requirement.name, index
            ))),
            _ => {}
        }
    }

    for (index, (actual, required)) in actual_parameters
        .iter()
        .zip(required_parameters.iter())
        .enumerate()
    {
        validate_trait_parameter_match(
            program,
            machine,
            state,
            trait_definition.name.as_str(),
            requirement,
            &required_generic_types,
            &mut type_bindings,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if let Some(trait_lifetime_arguments) = trait_lifetime_arguments {
        for (index, (actual, required)) in actual_parameters
            .iter()
            .zip(required_parameters.iter())
            .enumerate()
        {
            if !type_reference_lifetimes_match_requirement_application(
                program,
                actual.type_reference,
                required.type_reference,
                &trait_definition.lifetime_parameters,
                machine,
                trait_lifetime_arguments,
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: lifetime application does not match the declared `satisfies` edge",
                    machine.name,
                    state.name,
                    trait_definition.name,
                    requirement.name,
                    index,
                )));
            }
        }
    }

    if !type_references_match_with_trait_bindings(
        program,
        state.return_type,
        requirement.return_type,
        &required_generic_types,
        &mut type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, state.return_type)
        )));
    }
    if let Some(trait_lifetime_arguments) = trait_lifetime_arguments
        && !type_reference_lifetimes_match_requirement_application(
            program,
            state.return_type,
            requirement.return_type,
            &trait_definition.lifetime_parameters,
            machine,
            trait_lifetime_arguments,
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: return lifetime application does not match the declared `satisfies` edge",
            machine.name, state.name, trait_definition.name, requirement.name,
        )));
    }

    validate_trait_effect_ceiling(
        program,
        service_reaches,
        machine,
        state,
        trait_definition.name.as_str(),
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_services = program
        .service_reach_rows
        .services(requirement.service_reach_row);
    // The requirement stays fixed while the checked provider's published
    // service row includes conservative reach through ordinary helpers.
    let Some(summary) = service_reaches.for_machine(machine.symbol) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no inferred service-reach summary for requirement checking",
            machine.name,
        )));
        return;
    };
    for service in service_reaches.services(summary.effective) {
        if !allowed_services.contains(service) {
            let service_name = program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>");
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: service `{service_name}` is not allowed by the trait requirement",
                machine.name,
                state.name,
                trait_name,
                requirement.name,
            )));
        }
    }

    if machine.suspends && !requirement.suspends {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `suspends;` exceeds the trait requirement's operational ceiling",
            machine.name, state.name, trait_name, requirement.name,
        )));
    }
    if machine.blocks && !requirement.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `blocks;` exceeds the trait requirement's operational ceiling",
            machine.name, state.name, trait_name, requirement.name,
        )));
    }
}

fn validate_trait_parameter_match(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    trait_type_parameters: &[TypeParameter],
    type_bindings: &mut Vec<TraitTypeBinding>,
    index: usize,
    actual: &StateParameter,
    required: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if actual.is_self != required.is_self || actual.is_mutable != required.is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
        return;
    }

    // `&self` establishes receiver shape, not a reusable type-variable
    // binding. Attached machine self parameters carry the syntax placeholder
    // `Self`; binding the trait's `Self` to that placeholder makes a later
    // concrete `other: &Type` spuriously mismatch. The carrier is inferred
    // from the first non-receiver `Self` occurrence instead.
    if required.is_self {
        return;
    }

    if !type_references_match_with_trait_bindings(
        program,
        actual.type_reference,
        required.type_reference,
        trait_type_parameters,
        type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            type_reference_label(program, required.type_reference),
            type_reference_label(program, actual.type_reference)
        )));
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TraitTypeBinding {
    pub(crate) parameter_symbol: SymbolHandle,
    pub(crate) parameter_name: String,
    pub(crate) target: TraitTypeBindingTarget,
}

#[derive(Debug, Clone)]
pub(crate) enum TraitTypeBindingTarget {
    Type(TypeReferenceHandle),
    Parameter(SymbolHandle),
}

pub(crate) fn type_references_match_with_trait_bindings(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    trait_type_parameters: &[TypeParameter],
    bindings: &mut Vec<TraitTypeBinding>,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    // `Self` in a requirement type binds to the CARRIER on first use and must
    // match on every later use (rearrange settle 2026-07-18: free-machine
    // requirements are Self-shaped -- `machine add(a: Self, b: Self) -> Self`
    // -- and the carrier type is INFERRED from the satisfier's signature).
    if required_is_self_type(program, required) {
        if let Some(binding) = bindings.iter().find(|binding| {
            !binding.parameter_symbol.is_valid() && binding.parameter_name == "Self"
        }) {
            return binding_matches_actual(program, binding, actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: "Self".to_owned(),
            target: TraitTypeBindingTarget::Type(actual),
        });
        return true;
    }

    if let Some(parameter) = required_trait_type_parameter(program, required, trait_type_parameters)
    {
        if let Some(binding) = bindings.iter().find(|binding| {
            binding.parameter_symbol == parameter.symbol
                && binding.parameter_name == parameter.name.as_str()
        }) {
            return binding_matches_actual(program, binding, actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(actual),
        });
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                access: actual_access,
                // Lifetimes do not participate in trait-conformance matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                access: required_access,
                lifetime: _,
            },
        ) => {
            actual_access == required_access
                && type_references_match_with_trait_bindings(
                    program,
                    *actual_referee,
                    *required_referee,
                    trait_type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => type_references_match_with_trait_bindings(
            program,
            *actual_base,
            *required_base,
            trait_type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
        ) => {
            fixed_array_lengths_match_with_trait_bindings(
                program,
                actual_length,
                required_length,
                bindings,
            ) && type_references_match_with_trait_bindings(
                program,
                *actual_element,
                *required_element,
                trait_type_parameters,
                bindings,
            )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => type_references_match_with_trait_bindings(
            program,
            *actual_element,
            *required_element,
            trait_type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_base_symbol,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: required_base_symbol,
                arguments: required_arguments,
                ..
            },
        ) => {
            actual_base_symbol.is_valid()
                && actual_base_symbol == required_base_symbol
                && actual_arguments.count() == required_arguments.count()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required_arguments)
                            .iter(),
                    )
                    .all(|(actual_argument, required_argument)| {
                        type_references_match_with_trait_bindings(
                            program,
                            *actual_argument,
                            *required_argument,
                            trait_type_parameters,
                            bindings,
                        )
                    })
        }
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            },
            TypeReferenceNode::Generic { .. },
        ) if actual_symbol.is_valid() => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *actual_symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|origin| {
                type_references_match_with_trait_bindings(
                    program,
                    origin,
                    required,
                    trait_type_parameters,
                    bindings,
                )
            }),
        _ => type_references_match(program, actual, required),
    }
}

/// Recheck the part of a requirement type erased by ordinary runtime type
/// equality. `target_lifetimes` is the telescope a `required` lifetime
/// mention resolves against — the target trait's parameters for a trait
/// conformance, or the requirement machine's own telescope for a top-level
/// `boundary requirement` — and `raw_application` maps each target ordinal to
/// the realizing machine's lifetime ordinal. Lifetimes outside
/// `target_lifetimes` are callable-local and keep their existing owner.
pub(crate) fn type_reference_lifetimes_match_requirement_application(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    target_lifetimes: &[typed_trees::name::Identifier],
    machine: &Machine,
    raw_application: &[u32],
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                lifetime: actual_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                lifetime: required_lifetime,
                ..
            },
        ) => {
            requirement_lifetime_matches(
                actual_lifetime.as_ref(),
                required_lifetime.as_ref(),
                target_lifetimes,
                machine,
                raw_application,
            ) && type_reference_lifetimes_match_requirement_application(
                program,
                *actual_referee,
                *required_referee,
                target_lifetimes,
                machine,
                raw_application,
            )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => type_reference_lifetimes_match_requirement_application(
            program,
            *actual_base,
            *required_base,
            target_lifetimes,
            machine,
            raw_application,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                ..
            },
        )
        | (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => type_reference_lifetimes_match_requirement_application(
            program,
            *actual_element,
            *required_element,
            target_lifetimes,
            machine,
            raw_application,
        ),
        (
            TypeReferenceNode::Generic {
                lifetime_arguments: actual_lifetimes,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                lifetime_arguments: required_lifetimes,
                arguments: required_arguments,
                ..
            },
        ) => {
            actual_lifetimes.len() == required_lifetimes.len()
                && actual_lifetimes.iter().zip(required_lifetimes).all(
                    |(actual_lifetime, required_lifetime)| {
                        requirement_lifetime_matches(
                            Some(actual_lifetime),
                            Some(required_lifetime),
                            target_lifetimes,
                            machine,
                            raw_application,
                        )
                    },
                )
                && actual_arguments.count() == required_arguments.count()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required_arguments),
                    )
                    .all(|(actual_argument, required_argument)| {
                        type_reference_lifetimes_match_requirement_application(
                            program,
                            *actual_argument,
                            *required_argument,
                            target_lifetimes,
                            machine,
                            raw_application,
                        )
                    })
        }
        _ => true,
    }
}

fn requirement_lifetime_matches(
    actual: Option<&typed_trees::name::Identifier>,
    required: Option<&typed_trees::name::Identifier>,
    target_lifetimes: &[typed_trees::name::Identifier],
    machine: &Machine,
    raw_application: &[u32],
) -> bool {
    let Some(required) = required else {
        return true;
    };
    let Some(trait_ordinal) = target_lifetimes
        .iter()
        .position(|parameter| parameter.as_str() == required.as_str())
    else {
        return true;
    };
    let Some(machine_ordinal) = raw_application
        .get(trait_ordinal)
        .and_then(|ordinal| usize::try_from(*ordinal).ok())
    else {
        return false;
    };
    let Some(expected) = machine.lifetime_parameters.get(machine_ordinal) else {
        return false;
    };
    actual.is_some_and(|actual| actual.as_str() == expected.as_str())
}

fn fixed_array_lengths_match_with_trait_bindings(
    program: &TypedTrees,
    actual: &FixedArrayLength,
    required: &FixedArrayLength,
    bindings: &[TraitTypeBinding],
) -> bool {
    let FixedArrayLength::ConstParameter {
        symbol: required_symbol,
        name: required_name,
    } = required
    else {
        return actual == required;
    };
    let Some(binding) = bindings.iter().find(|binding| {
        binding.parameter_symbol == *required_symbol
            && binding.parameter_name == required_name.as_str()
    }) else {
        return actual == required;
    };
    let FixedArrayLength::ConstParameter {
        symbol: actual_symbol,
        ..
    } = actual
    else {
        return false;
    };
    match binding.target {
        TraitTypeBindingTarget::Parameter(expected_symbol) => {
            expected_symbol.is_valid() && *actual_symbol == expected_symbol
        }
        TraitTypeBindingTarget::Type(expected) => matches!(
            program.type_reference_table.type_reference(expected),
            TypeReferenceNode::Named { symbol, .. }
                if symbol.is_valid() && *actual_symbol == *symbol
        ),
    }
}

fn binding_matches_actual(
    program: &TypedTrees,
    binding: &TraitTypeBinding,
    actual: TypeReferenceHandle,
) -> bool {
    match &binding.target {
        TraitTypeBindingTarget::Type(expected) => match (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(*expected),
        ) {
            (
                TypeReferenceNode::Named {
                    symbol: actual_symbol,
                    ..
                },
                TypeReferenceNode::Named {
                    symbol: expected_symbol,
                    ..
                },
            ) if actual_symbol.is_valid() && expected_symbol.is_valid() => {
                actual_symbol == expected_symbol
                    || type_reference_is_instance_of_family(program, actual, *expected)
            }
            _ => {
                type_references_match(program, actual, *expected)
                    || type_reference_is_instance_of_family(program, actual, *expected)
            }
        },
        TraitTypeBindingTarget::Parameter(symbol) => matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            } if (*symbol).is_valid() && *actual_symbol == *symbol
        ),
    }
}

fn type_reference_is_instance_of_family(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    family: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named {
        symbol: family_symbol,
        ..
    } = program.type_reference_table.type_reference(family)
    else {
        return false;
    };
    let Some(family_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *family_symbol)
    else {
        return false;
    };
    if program.data_type_parameters(family_definition).is_empty() {
        return false;
    }
    matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Generic { base_symbol, .. } if base_symbol == family_symbol
    )
}

fn indexed_law_callable_telescope_groups<'program>(
    program: &'program TypedTrees,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
) -> Vec<&'program [TypeParameter]> {
    let has_proposition_law = program
        .state_signature_contracts(requirement)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| matches!(fact, ProofFact::Proposition(_)));
    if !has_proposition_law {
        return Vec::new();
    }
    let trait_parameters = program.trait_type_parameters(trait_definition);
    program
        .state_signature_parameters(requirement)
        .iter()
        .filter_map(|representative| {
            let carrier_parameter = required_trait_type_parameter(
                program,
                representative.type_reference,
                trait_parameters,
            )?;
            let carrier_index = trait_parameters
                .iter()
                .position(|parameter| parameter.symbol == carrier_parameter.symbol)?;
            let carrier_argument = *explicit_type_arguments.get(carrier_index)?;
            let TypeReferenceNode::Named {
                symbol: carrier_symbol,
                ..
            } = program
                .type_reference_table
                .type_reference(carrier_argument)
            else {
                return None;
            };
            let carrier = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *carrier_symbol)?;
            let telescope = program.data_type_parameters(carrier);
            (!telescope.is_empty()).then_some(telescope)
        })
        .collect()
}

fn required_is_self_type(program: &TypedTrees, required: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(required),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "Self"
    )
}

pub(crate) fn required_trait_type_parameter<'program>(
    program: &'program TypedTrees,
    required: TypeReferenceHandle,
    trait_type_parameters: &'program [TypeParameter],
) -> Option<&'program TypeParameter> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(required)
    else {
        return None;
    };

    symbol.is_valid().then_some(())?;
    trait_type_parameters
        .iter()
        .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
}

pub(crate) fn parameter_shape_label(program: &TypedTrees, parameter: &StateParameter) -> String {
    let qualifier = if parameter.is_mutable { "mut " } else { "" };
    if parameter.is_self {
        format!("&{qualifier}self")
    } else {
        format!(
            "{}: {}",
            parameter.name,
            type_reference_label(program, parameter.type_reference)
        )
    }
}
