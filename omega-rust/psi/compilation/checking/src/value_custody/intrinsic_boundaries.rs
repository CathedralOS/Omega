//! Exact typed identity of external-leaf boundary realizations.
//!
//! A bodyless leaf `machine leaf(..) satisfies Boundary::requirement via ..;`
//! owns no Omega body: its realization is the compiler intrinsic or the
//! evaluated foreign binding its `via` names. A direct call to such a leaf
//! from checked code (`ConsoleNativeProvider::write_byte(b)`, `darwin_poll(0,
//! 0, ms)`) is therefore a boundary call, and checking plans it against the
//! leaf's exact satisfied requirement: the requirement owns the published
//! contract, reach, crash routes and calling-policy application; Omega settles
//! it through the selected provider row that carries the leaf's realization
//! (a compiler-builtin settlement or a normalized foreign import). Treating
//! the leaf as an ordinary callee instead demands an executable body that no
//! stage can construct, and fabricating one would discard the binding's
//! exact ABI evidence.
//!
//! Two leaf families project, each only through one exact edge:
//!
//! - compiler-intrinsic leaves, identified by the sealed intrinsic catalog's
//!   declaration, signature and provider (`exact_compiler_intrinsic_boundary_requirement`);
//! - evaluated-binding leaves, whose ordinary `via <expression>` evaluates
//!   at build time to a `ForeignBinding` value. Their declared signature is
//!   the boundary signature, so projection requires it to equal the
//!   requirement's runtime signature exactly
//!   (`exact_external_leaf_boundary_requirement`).
//!
//! Call planning, boundary-plan collection, call-source custody, the
//! scalar-computation exclusion and service-reach inference all apply this
//! one join. For reach, an evaluated-binding leaf is an ordinary bodyless
//! machine whose satisfied requirement supplies its published reach, so a
//! call to it propagates that reach like any machine call rather than
//! counting as authored direct boundary use; a leaf call and a field call on
//! one requirement then publish the single reach its boundary plan needs.
//!
//! Honest boundaries:
//!
//! - The projected call resolves through the requirement's selected provider
//!   row, so it reaches the named leaf only while that leaf's family is the
//!   selected realization, the same premise the intrinsic route carries.
//! - A machine holding provider fields cannot yet make a projected leaf call:
//!   stage 05 and Terminal verification count every signature-directed
//!   boundary call there as a provider-field obligation.

use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::boundary_completions::exact_direct_intrinsic_signature;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// The exact boundary requirement a direct call to a bodyless external leaf
/// delegates to, with the leaf's attached provider (the invalid symbol for a
/// free leaf). Checking and lowering use this join for every external leaf;
/// the checked interpreter keeps `exact_compiler_intrinsic_boundary_requirement`
/// because it hosts only the intrinsic catalog.
pub fn exact_external_leaf_boundary_requirement(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    exact_compiler_intrinsic_boundary_requirement(program, target_state_symbol)
        .or_else(|| exact_evaluated_binding_boundary_requirement(program, target_state_symbol))
}

/// An ordinary evaluated-binding leaf's exact boundary-trait requirement.
/// The leaf carries exactly one conformance and it names its `via` producer;
/// the requirement is monomorphic and synchronous, owns no private callback
/// parameters, and publishes the leaf's own runtime signature.
fn exact_evaluated_binding_boundary_requirement(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let machine = program.machine_holding_state(target_state_symbol)?;
    if machine.body_is_present
        || !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization {
                binding: None,
                mechanism: None,
            }
        )
    {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [conformance] = program.machine_trait_conformances(machine) else {
        return None;
    };
    if state.symbol != target_state_symbol
        || conformance.external_binding.is_some()
        || !conformance.via_expression.is_valid()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
    {
        return None;
    }
    let (definition, requirement) = satisfied_trait_parts(program, machine, conformance)?;
    (definition.symbol == conformance.symbol
        && definition.is_boundary
        && definition.lifetime_parameters.is_empty()
        && program.trait_type_parameters(definition).is_empty()
        && requirement.symbol == conformance.requirement_symbol
        && requirement.lifetime_parameters.is_empty()
        && program
            .state_signature_type_parameters(requirement)
            .is_empty()
        && requirement.native_callback_parameters.is_empty()
        && !requirement.suspends
        && same_runtime_signature(
            program,
            (program.state_parameters(state), state.return_type),
            (
                program.state_signature_parameters(requirement),
                requirement.return_type,
            ),
        ))
    .then_some((requirement.symbol, machine.attached_data_symbol))
}

/// The satisfied trait's resolved definition and requirement, or `None`
/// when the conformance is not a plain trait satisfaction.
fn satisfied_trait_parts<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    conformance: &'program typed_trees::machine::TraitConformance,
) -> Option<(
    &'program typed_trees::trait_definition::TraitDefinition,
    &'program typed_trees::signature::StateSignature,
)> {
    let typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)?
    else {
        return None;
    };
    Some((definition, requirement))
}

/// Parameter-for-parameter equality of two runtime signatures: position,
/// receiver/const/mutable markers, relevance and normalized type identity,
/// then the normalized result identity. A coercible or merely equally shaped
/// signature does not qualify.
fn same_runtime_signature(
    program: &dyn crate::CheckingInput,
    (leaf_parameters, leaf_result): (
        &[typed_trees::signature::StateParameter],
        typed_trees::types::TypeReferenceHandle,
    ),
    (requirement_parameters, requirement_result): (
        &[typed_trees::signature::StateParameter],
        typed_trees::types::TypeReferenceHandle,
    ),
) -> bool {
    leaf_parameters.len() == requirement_parameters.len()
        && leaf_parameters
            .iter()
            .zip(requirement_parameters)
            .all(|(leaf, requirement)| {
                leaf.is_const == requirement.is_const
                    && leaf.is_mutable == requirement.is_mutable
                    && leaf.relevance == requirement.relevance
                    && program.normalized_type_identity(leaf.type_reference)
                        == program.normalized_type_identity(requirement.type_reference)
            })
        && program.normalized_type_identity(leaf_result)
            == program.normalized_type_identity(requirement_result)
}

/// The sealed compiler-intrinsic catalog's exact requirement edge. This is
/// the narrower identity the checked interpreter hosts; checking and lowering
/// consume `exact_external_leaf_boundary_requirement`.
pub fn exact_compiler_intrinsic_boundary_requirement(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    // State spans are disjoint append-only ranges: at most one machine can
    // contain a state symbol, so the holder is unique by construction.
    let machine = program.machine_holding_state(target_state_symbol)?;
    if !program.machine_type_parameters(machine).is_empty() {
        return None;
    }
    let (authored_binding, inferred_hosted_intrinsic) =
        intrinsic_realization_candidate(program, machine)?;
    let inferred_trait_name = inferred_hosted_intrinsic.map(|(_, trait_name)| trait_name);
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // A concrete byte leaf returns its requirement's nominal sum, not Unit.
    // Keep this separate from the scalar-to-Unit signature: this join also
    // feeds source custody and reach inference, not just host interpretation.
    let byte_result = (machine.name.as_str() == "ConsoleNativeProvider::read_byte"
        && program.state_parameters(state).is_empty())
    .then(|| exact_byte_read_result_type(program, state.return_type))
    .flatten();
    if state.symbol != target_state_symbol
        || !(exact_direct_intrinsic_signature(
            program,
            program.state_parameters(state),
            state.return_type,
        ) || byte_result.is_some())
    {
        return None;
    }

    let mut matches = program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            if ((inferred_trait_name.is_some()
                && (conformance.external_binding.is_some()
                    || conformance.via_expression.is_valid()
                    || conformance.external_binding_source_span.is_some()))
                || (inferred_trait_name.is_none()
                    && conformance.external_binding != authored_binding))
                || conformance.requirement.is_none()
                || !program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .is_empty()
            {
                return None;
            }
            let (definition, requirement) = satisfied_trait_parts(program, machine, conformance)?;
            let provider_prefix = inferred_hosted_intrinsic
                .map(|(provider_name, _)| provider_name)
                .unwrap_or_default();
            (definition.symbol == conformance.symbol
                && definition.is_boundary
                && inferred_trait_name
                    .is_none_or(|trait_name| definition.name.as_str() == trait_name)
                && definition.lifetime_parameters.is_empty()
                && program.trait_type_parameters(definition).is_empty()
                && requirement.symbol == conformance.requirement_symbol
                && (inferred_trait_name.is_none()
                    || machine
                        .name
                        .as_str()
                        .strip_prefix(provider_prefix)
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        == Some(requirement.name.as_str()))
                && requirement.lifetime_parameters.is_empty()
                && program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
                && requirement.native_callback_parameters.is_empty()
                && !requirement.suspends
                // The hosted byte-input leaf publishes `blocks` honestly:
                // it may occupy the worker while it waits for input. Every
                // other intrinsic boundary leaf must stay nonblocking.
                && byte_result.is_some() == requirement.blocks
                && ((byte_result.is_none()
                    && exact_direct_intrinsic_signature(
                        program,
                        program.state_signature_parameters(requirement),
                        requirement.return_type,
                    ))
                    || (byte_result.is_some()
                        && definition.name.as_str() == "Console"
                        && requirement.name.as_str() == "read_byte"
                        && program.state_signature_parameters(requirement).is_empty()
                        && exact_byte_read_result_type(program, requirement.return_type)
                            == byte_result)))
                .then_some(requirement.symbol)
        });
    let requirement = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some((requirement, machine.attached_data_symbol))
}

/// The machine-level half of the intrinsic identity: the authored
/// compiler-intrinsic binding, or the exact hosted catalog leaf (realization,
/// provider nominal, boundary trait) admitted without an authored `via`
/// binding. `None` when the machine cannot realize an intrinsic boundary.
fn intrinsic_realization_candidate(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<(
    Option<language_semantics::ExternalBindingId>,
    Option<(&'static str, &'static str)>,
)> {
    if machine.body_is_present || !machine.lifetime_parameters.is_empty() {
        return None;
    }
    let authored_binding = match machine.supply_mode {
        MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        } if program.external_bindings.identity(binding)
            == Some(&language_semantics::ExternalBindingIdentity::CompilerIntrinsic) =>
        {
            Some(binding)
        }
        _ => None,
    };
    let inferred_hosted_intrinsic = match machine.name.as_str() {
        "ConsoleNativeProvider::exit_process" | "ConsoleNativeProvider::write_byte" => {
            Some(("ConsoleNativeProvider", "Console"))
        }
        "ProcessExitNativeProvider::exit_process" => {
            Some(("ProcessExitNativeProvider", "ProcessExit"))
        }
        _ => None,
    }
    .filter(|(provider_name, _)| {
        machine.supply_mode == MachineSupplyMode::Boundary
            && machine.attached_data.as_ref().map(|name| name.as_str()) == Some(*provider_name)
    });
    (authored_binding.is_some() || inferred_hosted_intrinsic.is_some())
        .then_some((authored_binding, inferred_hosted_intrinsic))
}

/// A `CheckedShape` data declaration resolved through a type reference's
/// exact named head — the admission prologue hosted intrinsic result
/// contracts share. `None` for anything but a plain (no lifetime or type
/// parameters, generic instance, quotient, where-facts, zero-gate, carry or
/// Linear multiplicity) `CheckedShape` data named `name`. The returned
/// symbol is the type reference's head, which the declaration resolved to.
pub fn checked_shape_data_declaration<'program>(
    program: &'program dyn crate::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    name: &str,
) -> Option<(&'program typed_trees::data::DataDefinition, SymbolHandle)> {
    let symbol = &program
        .type_reference_table
        .type_reference(result_type)
        .as_named_symbol()?;
    let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
    if !symbol.is_valid()
        || data.name.as_str() != name
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || !data.lifetime_parameters.is_empty()
        || !program.data_type_parameters(data).is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
        || !data.where_facts.is_empty()
        || data.zero_gated
        || data.properties.carry.is_some()
        || data.properties.multiplicity == language_semantics::Multiplicity::Linear
    {
        return None;
    }
    Some((data, *symbol))
}

/// The supported byte-input result, resolved by declaration identity. This is
/// typed shape validation, not selected-package or native execution admission.
pub fn exact_byte_read_result_type(
    program: &dyn crate::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    use typed_trees::data::DataMember;

    let (data, symbol) = checked_shape_data_declaration(program, result_type, "ByteRead")?;
    let [DataMember::Variant(eof), DataMember::Variant(byte)] = program.data_members(data) else {
        return None;
    };
    if eof.name.as_str() != "Eof"
        || !eof.payload.is_empty()
        || !eof.where_facts.is_empty()
        || byte.name.as_str() != "Byte"
    {
        return None;
    }
    let [field] = program.data_payload_fields(byte) else {
        return None;
    };
    // The host operation stores one input byte in a bare `i32` carrier. A
    // qualified carrier names a domain this operation cannot establish.
    if field.name.as_str() != "value"
        || field.relevance.is_erased()
        || !matches!(
            program
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(field.type_reference) != Some(PrimitiveType::I32)
    {
        return None;
    }
    // The case's `where` facts may narrow the payload but must admit every
    // input byte, so only interval facts containing 0..=255 qualify.
    crate::case_where_field_intervals(program, byte)?
        .iter()
        .all(|(_, minimum, maximum)| {
            minimum.is_none_or(|minimum| minimum <= 0)
                && maximum.is_none_or(|maximum| maximum >= 255)
        })
        .then_some(symbol)
}
/// Completion belongs to the admitted requirement declaration, independently
/// of the provider selected to realize it. The join itself is typed-program
/// vocabulary; checking forwards its `CheckingInput` view.
pub fn boundary_requirement_completion(
    program: &dyn crate::CheckingInput,
    requirement: SymbolHandle,
) -> semantic_vocabulary::BoundaryCompletion {
    typed_trees::boundary_completions::boundary_requirement_completion(
        program.typed_program(),
        requirement,
    )
}

/// The core terminal-event identity used by both checking and interpretation.
pub fn is_canonical_process_exit_requirement(
    program: &dyn crate::CheckingInput,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    typed_trees::boundary_completions::is_canonical_process_exit_requirement(
        program.typed_program(),
        trait_symbol,
        requirement,
    )
}

/// Rejoin a semantic-role receipt to the exact package-owned declaration.
/// Consumer policy supplies the role; declaration spelling alone cannot.
pub fn valid_boundary_completion_declaration(
    program: &dyn crate::CheckingInput,
    declaration: &typed_trees::BoundaryCompletionDeclaration,
) -> bool {
    typed_trees::boundary_completions::valid_boundary_completion_declaration(
        program.typed_program(),
        declaration,
    )
}

/// Malformed source-role input must fail admission, never weaken an external
/// completion promise into an ordinary return by losing its exact join.
pub fn validate_boundary_completion_declarations(
    program: &dyn crate::CheckingInput,
) -> Result<(), &'static str> {
    typed_trees::boundary_completions::validate_boundary_completion_declarations(
        program.typed_program(),
    )
}
