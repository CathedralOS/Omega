use super::shared::trait_definition_by_symbol;
use crate::type_references::{type_reference_label, type_references_match};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataMember, TypeParameter};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::{StateParameter, StateSignature};
use omega_typed_trees::state::State;
use omega_typed_trees::trait_definition::TraitDefinition;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Default native leaves may only expose types whose public shape determines
/// every ABI fact. A source-selected `Calling<C>` policy is the explicit escape
/// hatch: its checked plan can publish a canonical descriptor representation.
/// Without one, private slice/text/vector carriers must stop at a checked
/// adapter rather than silently inheriting the compiler's storage layout.
pub(crate) fn validate_external_leaf_native_shapes(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| conformance.via.is_some())
    {
        // Compiler intrinsics are not foreign ABI leaves: they select a
        // compiler-owned lowering whose safe carrier semantics are already
        // part of the target plan. In particular, Console::read_line may
        // retain its checked mutable-slice surface while the lowering derives
        // the concrete owned destination's capacity and live-length write.
        if conformance
            .via
            .as_deref()
            .is_some_and(|binding| binding.starts_with("CompilerIntrinsic("))
        {
            continue;
        }
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        if boundary_has_explicit_calling_policy(program, trait_definition) {
            continue;
        }
        let Some(requirement_name) = conformance.requirement.as_ref() else {
            continue;
        };
        let Some(requirement) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|requirement| requirement.name == *requirement_name)
        else {
            continue;
        };

        for parameter in program
            .state_signature_parameters(requirement)
            .iter()
            .filter(|parameter| !parameter.is_self)
        {
            if let Some(carrier) =
                private_native_carrier(program, parameter.type_reference, &mut Vec::new())
            {
                diagnostics.push(native_carrier_diagnostic(
                    program,
                    machine,
                    trait_definition,
                    requirement,
                    parameter.type_reference,
                    carrier,
                    "parameter",
                ));
            }
        }
        if requirement.return_type.is_valid()
            && let Some(carrier) =
                private_native_carrier(program, requirement.return_type, &mut Vec::new())
        {
            diagnostics.push(native_carrier_diagnostic(
                program,
                machine,
                trait_definition,
                requirement,
                requirement.return_type,
                carrier,
                "result",
            ));
        }
    }
}

fn boundary_has_explicit_calling_policy(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
) -> bool {
    let calling = program.traits().iter().find(|candidate| {
        candidate.name.as_str().rsplit("::").next() == Some("Calling")
            && program.trait_type_parameters(candidate).len() == 1
            && program.trait_machine_signatures(candidate).is_empty()
    });
    calling.is_some_and(|calling| {
        program
            .trait_requirements(trait_definition)
            .iter()
            .any(|requirement| requirement.symbol == calling.symbol)
    })
}

#[derive(Debug, Clone, Copy)]
enum PrivateNativeCarrier {
    Slice,
    Text,
    Vector,
}

impl PrivateNativeCarrier {
    const fn label(self) -> &'static str {
        match self {
            Self::Slice => "safe slice",
            Self::Text => "text view or bounded-text carrier",
            Self::Vector => "vector carrier",
        }
    }
}

fn private_native_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<PrivateNativeCarrier> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
                && carrier_is_slice_or_fixed_array(program, *base_type)
            {
                return Some(PrivateNativeCarrier::Text);
            }
            private_native_carrier(program, *base_type, visiting)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            private_native_carrier(program, *referee, visiting)
        }
        TypeReferenceNode::Slice { .. } => Some(PrivateNativeCarrier::Slice),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            private_native_carrier(program, *element_type, visiting)
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if base_name.as_str().rsplit("::").next() == Some("Vec") {
                return Some(PrivateNativeCarrier::Vector);
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| private_native_carrier(program, *argument, visiting))
        }
        TypeReferenceNode::Named { symbol, name } => {
            if name.as_str().rsplit("::").next() == Some("Vec") {
                return Some(PrivateNativeCarrier::Vector);
            }
            if !symbol.is_valid() || visiting.contains(symbol) {
                return None;
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)?;
            visiting.push(*symbol);
            let carrier = program.data_members(definition).iter().find_map(|member| {
                let DataMember::Field(field) = member else {
                    return None;
                };
                private_native_carrier(program, field.type_reference, visiting)
            });
            visiting.pop();
            carrier
        }
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Unit => None,
    }
}

fn carrier_is_slice_or_fixed_array(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => carrier_is_slice_or_fixed_array(program, *referee),
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => true,
        _ => false,
    }
}

fn native_carrier_diagnostic(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    type_reference: TypeReferenceHandle,
    carrier: PrivateNativeCarrier,
    position: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "external leaf `{}` cannot use {} `{}` as a default-native {} for `{}::{}`; declare the foreign pointer/length/terminator or record shape explicitly in a checked adapter, or select a custom `Calling<C>` policy that publishes a canonical representation",
        machine.name,
        carrier.label(),
        program.display_type_reference(type_reference),
        position,
        trait_definition.name,
        requirement.name,
    ))
}

pub(crate) fn validate_machine_trait_conformances(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            if validate_machine_boundary_operator_conformance(
                program,
                machine,
                conformance,
                diagnostics,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };
        let explicit_type_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected_type_arguments = program.trait_type_parameters(trait_definition).len();
        if explicit_type_arguments.len() != expected_type_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_type_arguments} generic argument(s), got {}",
                machine.name,
                trait_definition.name,
                explicit_type_arguments.len()
            )));
            continue;
        }

        // SINGLE-REQUIREMENT conformance (rearrange settle 2026-07-18): an
        // explicit `satisfies Trait::requirement`, or a bare trait name on a
        // FREE machine (free proof machines conform machine-by-machine to the
        // requirement bearing their own name -- whole-trait candidate lookup
        // never existed for them: a free machine's candidate set is itself).
        // Data-attached machines with a bare trait name keep the whole-trait
        // semantics below, unchanged.
        let named_requirement = conformance.requirement.clone();
        let single_requirement = named_requirement.clone().or_else(|| {
            machine
                .attached_data
                .is_none()
                .then(|| machine.name.clone())
        });
        if let Some(requirement_name) = single_requirement {
            validate_machine_single_requirement(
                program,
                machine,
                trait_definition,
                &requirement_name,
                named_requirement.is_some(),
                conformance.alias.as_ref().map(|alias| alias.as_str()),
                explicit_type_arguments,
                diagnostics,
            );
            continue;
        }

        let mut visited_traits = Vec::new();
        validate_machine_satisfies_trait(
            program,
            machine,
            trait_definition,
            explicit_type_arguments,
            diagnostics,
            &mut visited_traits,
        );
    }
}

/// Boundary-operator requirements share the ordinary machine `satisfies`
/// spelling with trait requirements, but resolve by exact overloaded
/// signature rather than a trait symbol. External target leaves inherit the
/// operator's contract through their admitted binding. Checked software
/// satisfiers remain fail-closed until operator-law entailment consumes their
/// bodies as well.
fn validate_machine_boundary_operator_conformance(
    program: &TypedTrees,
    machine: &Machine,
    conformance: &omega_typed_trees::machine::TraitConformance,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(requirement) = conformance.requirement.as_ref() else {
        return false;
    };
    let namespace = conformance.name.as_str();
    let requirement_name = requirement.as_str();
    let names_operator = program.operators().iter().any(|operator| {
        let path = program.operator_path_members(operator.name);
        matches!(path, [owner, member]
            if operator.is_boundary
                && owner.as_str() == namespace
                && member.as_str() == requirement_name)
    });
    if !names_operator {
        return false;
    }

    if !program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies type arguments to boundary-operator requirement `{}::{}`; the exact overloaded operator is selected from the machine signature",
            machine.name, namespace, requirement_name,
        )));
        return true;
    }

    let Some(operator) = omega_typed_trees::operator::resolve_satisfied_boundary_operator(
        program,
        machine,
        namespace,
        requirement_name,
    ) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not match one exact overload of boundary-operator requirement `{}::{}`; its entry parameter and result types must equal one declared requirement signature",
            machine.name, namespace, requirement_name,
        )));
        return true;
    };

    if conformance.via.is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "checked machine `{}` satisfies boundary operator `{}`, but checked operator-law entailment is not implemented yet; use an admitted target leaf with `via Binding::...` until the checked-software provider rung lands",
            machine.name,
            omega_typed_trees::operator::boundary_operator_requirement_identity(program, operator),
        )));
    }
    true
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
    explicit_type_arguments: &[TypeReferenceHandle],
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

    validate_machine_state_satisfies_trait_signature_with_arguments(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        explicit_type_arguments,
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
    explicit_type_arguments: &[TypeReferenceHandle],
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

        validate_machine_state_satisfies_trait_signature_with_arguments(
            program,
            state_machine,
            state,
            trait_definition,
            requirement,
            explicit_type_arguments,
            diagnostics,
        );
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };

        let required_arguments = compose_forwarded_trait_arguments(
            program,
            trait_definition,
            explicit_type_arguments,
            program
                .type_reference_table
                .type_reference_handles(requirement.arguments),
        );

        validate_machine_satisfies_trait(
            program,
            machine,
            required_trait,
            &required_arguments,
            diagnostics,
            visited_traits,
        );
    }

    visited_traits.pop();
}

/// Compose a parent edge such as `Forwarded<U>: Sink<U>` with the concrete
/// arguments of the child instance. The returned handles are existing nodes;
/// exact forwarded parameters need no allocation and cover the canonical
/// parent-binding form while preserving concrete/composite arguments as-is.
pub(super) fn compose_forwarded_trait_arguments(
    program: &TypedTrees,
    source_trait: &TraitDefinition,
    source_arguments: &[TypeReferenceHandle],
    parent_arguments: &[TypeReferenceHandle],
) -> Vec<TypeReferenceHandle> {
    let source_parameters = program.trait_type_parameters(source_trait);
    parent_arguments
        .iter()
        .map(|argument| {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*argument)
            else {
                return *argument;
            };
            source_parameters
                .iter()
                .zip(source_arguments.iter())
                .find(|(parameter, _)| {
                    (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                        || parameter.name.as_str() == name.as_str()
                })
                .map(|(_, concrete)| *concrete)
                .unwrap_or(*argument)
        })
        .collect()
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

pub(super) fn validate_machine_state_satisfies_trait_signature_with_arguments(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
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
    let mut type_bindings = trait_type_parameters
        .iter()
        .zip(explicit_type_arguments.iter().copied())
        .map(|(parameter, actual)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            actual,
        })
        .collect::<Vec<_>>();

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
    let allowed_services = program
        .service_reach_rows
        .services(requirement.service_reach_row);
    for service in program
        .service_reach_rows
        .services(machine.service_reach_row)
    {
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
        if let Some(binding) = bindings.iter().find(|binding| {
            !binding.parameter_symbol.is_valid() && binding.parameter_name == "Self"
        }) {
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
