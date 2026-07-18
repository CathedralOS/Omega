use super::shared::trait_definition_by_symbol;
use crate::type_references::{type_reference_label, type_references_match};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::TypeParameter;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::{StateParameter, StateSignature};
use omega_typed_trees::state::State;
use omega_typed_trees::trait_definition::TraitDefinition;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_machine_trait_conformances(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };

        // SINGLE-REQUIREMENT conformance (rearrange settle 2026-07-18): an
        // explicit `satisfies Trait::requirement`, or a bare trait name on a
        // FREE machine (free proof machines conform machine-by-machine to the
        // requirement bearing their own name -- whole-trait candidate lookup
        // never existed for them: a free machine's candidate set is itself).
        // Data-attached machines with a bare trait name keep the whole-trait
        // semantics below, unchanged.
        let named_requirement = conformance.requirement.clone();
        let single_requirement = named_requirement
            .clone()
            .or_else(|| machine.attached_data.is_none().then(|| machine.name.clone()));
        if let Some(requirement_name) = single_requirement {
            validate_machine_single_requirement(
                program,
                machine,
                trait_definition,
                &requirement_name,
                named_requirement.is_some(),
                conformance.alias.as_ref().map(|alias| alias.as_str()),
                diagnostics,
            );
            continue;
        }

        let mut visited_traits = Vec::new();
        validate_machine_satisfies_trait(
            program,
            machine,
            trait_definition,
            diagnostics,
            &mut visited_traits,
        );
    }
}

/// Conform THIS machine to ONE trait requirement (the machine-by-machine
/// carrier model): the machine's ENTRY signature must match the requirement's
/// (with `Self` binding to the carrier type on first use). LAW requirements
/// (an `ensures` on the requirement) additionally demand a proven-ensures |=
/// declared-law match (rung B: contract_entailment::check_law_conformance).
fn validate_machine_single_requirement(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement_name: &omega_typed_trees::name::Identifier,
    explicitly_named: bool,
    conformance_alias: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(requirement) = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|requirement| requirement.name == *requirement_name)
    else {
        if explicitly_named {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}`, but trait `{}` has no requirement named `{}`",
                machine.name,
                trait_definition.name,
                requirement_name,
                trait_definition.name,
                requirement_name
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "free machine `{}` satisfies trait `{}`, which has no requirement named `{}` -- \
                 a free machine's bare `satisfies` binds the requirement bearing its own name; \
                 name one explicitly with `satisfies {}::<requirement>`",
                machine.name, trait_definition.name, machine.name, trait_definition.name
            )));
        }
        return;
    };

    // The machine's conforming signature is its ENTRY state (the first state:
    // an implicit entry always parses first, and single-entry proof machines
    // are the shape this mode serves).
    let Some(entry_state) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but has no states",
            machine.name, trait_definition.name, requirement_name
        )));
        return;
    };

    validate_machine_state_satisfies_trait_signature(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        diagnostics,
    );

    // A LAW requirement (ensures on the requirement) demands the satisfier
    // PROVE the law: proven-ensures |= declared-law, forall-to-forall.
    crate::contract_entailment::check_law_conformance(
        program,
        machine,
        conformance_alias,
        trait_definition,
        requirement,
        diagnostics,
    );
}

fn validate_machine_satisfies_trait(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for requirement in program.trait_machine_signatures(trait_definition) {
        let Some((state_machine, state)) = trait_requirement_state(program, machine, requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies trait `{}` but is missing machine `{}`",
                machine.name, trait_definition.name, requirement.name
            )));
            continue;
        };

        validate_machine_state_satisfies_trait_signature(
            program,
            state_machine,
            state,
            trait_definition,
            requirement,
            diagnostics,
        );
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        validate_machine_satisfies_trait(
            program,
            machine,
            required_trait,
            diagnostics,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn trait_requirement_state<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    requirement: &StateSignature,
) -> Option<(&'program Machine, &'program State)> {
    trait_conformance_candidate_machines(program, machine)
        .into_iter()
        .find_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name == requirement.name)
                .map(|state| (candidate, state))
        })
}

fn trait_conformance_candidate_machines<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
) -> Vec<&'program Machine> {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return vec![machine];
    };

    let mut candidates = Vec::new();
    candidates.push(machine);
    candidates.extend(program.machines().iter().filter(|candidate| {
        !std::ptr::eq(*candidate, machine)
            && candidate.attached_data.as_ref() == Some(attached_data)
    }));
    candidates
}

pub(super) fn validate_machine_state_satisfies_trait_signature(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
    // PRV4 self-forwarding adapters: a FREE machine satisfying a BOUNDARY
    // trait requirement may take the trait ITSELF as one extra LEADING
    // parameter (adapter dispatch forwards the call's receiver there); the
    // tail must match the requirement exactly. Attached machines and plain
    // traits keep the strict positional match.
    if trait_definition.is_boundary
        && machine.attached_data.is_none()
        && actual_parameters.len() == required_parameters.len() + 1
        && actual_parameters.first().is_some_and(|parameter| {
            let label = type_reference_label(program, parameter.type_reference);
            let leaf = label.rsplit("::").next().unwrap_or(label.as_str()).to_owned();
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
    let mut type_bindings = Vec::new();

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
            trait_type_parameters,
            &mut type_bindings,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if !type_references_match_with_trait_bindings(
        program,
        state.return_type,
        requirement.return_type,
        trait_type_parameters,
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

    validate_trait_effect_ceiling(
        program,
        machine,
        state,
        trait_definition.name.as_str(),
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_effects = program.state_signature_effects(requirement);

    for effect in program.machine_effects(machine) {
        if !allowed_effects
            .iter()
            .any(|allowed| allowed.as_str() == effect.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: effect `{}` is not allowed by the trait requirement",
                machine.name,
                state.name,
                trait_name,
                requirement.name,
                effect
            )));
        }
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
struct TraitTypeBinding {
    parameter_symbol: SymbolHandle,
    parameter_name: String,
    actual: TypeReferenceHandle,
}

fn type_references_match_with_trait_bindings(
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
        if let Some(binding) = bindings
            .iter()
            .find(|binding| !binding.parameter_symbol.is_valid() && binding.parameter_name == "Self")
        {
            return type_references_match(program, actual, binding.actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: "Self".to_owned(),
            actual,
        });
        return true;
    }

    if let Some(parameter) = required_trait_type_parameter(program, required, trait_type_parameters)
    {
        if let Some(binding) = bindings.iter().find(|binding| {
            binding.parameter_symbol == parameter.symbol
                && binding.parameter_name == parameter.name.as_str()
        }) {
            return type_references_match(program, actual, binding.actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            actual,
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
                is_mutable: actual_mutable,
                // Lifetimes do not participate in trait-conformance matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                is_mutable: required_mutable,
                lifetime: _,
            },
        ) => {
            actual_mutable == required_mutable
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
            actual_length == required_length
                && type_references_match_with_trait_bindings(
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
                base_name: actual_base,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: required_base,
                arguments: required_arguments,
                ..
            },
        ) => {
            actual_base == required_base
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
        _ => type_references_match(program, actual, required),
    }
}

fn required_is_self_type(program: &TypedTrees, required: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(required),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "Self"
    )
}

fn required_trait_type_parameter<'program>(
    program: &'program TypedTrees,
    required: TypeReferenceHandle,
    trait_type_parameters: &'program [TypeParameter],
) -> Option<&'program TypeParameter> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(required)
    else {
        return None;
    };

    trait_type_parameters.iter().find(|parameter| {
        (parameter.symbol.is_valid() && parameter.symbol == *symbol)
            || parameter.name.as_str() == name.as_str()
    })
}

fn parameter_shape_label(program: &TypedTrees, parameter: &StateParameter) -> String {
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
