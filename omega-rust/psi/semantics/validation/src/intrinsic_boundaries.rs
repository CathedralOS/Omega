//! Exact typed identity of compiler-intrinsic boundary realizations.

use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

pub fn exact_compiler_intrinsic_boundary_requirement(
    program: &TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let mut machines = program.machines().iter().filter(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state_symbol)
    });
    let machine = machines.next()?;
    if machines.next().is_some()
        || machine.body_is_present
        || !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
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
    let inferred_console_intrinsic = machine.supply_mode == MachineSupplyMode::Boundary
        && matches!(
            machine.name.as_str(),
            "ConsoleNativeProvider::exit_process" | "ConsoleNativeProvider::write_byte"
        )
        && machine.attached_data.as_ref().map(|name| name.as_str())
            == Some("ConsoleNativeProvider");
    if authored_binding.is_none() && !inferred_console_intrinsic {
        return None;
    }
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
            if ((inferred_console_intrinsic
                && (conformance.external_binding.is_some()
                    || conformance.via_expression.is_valid()
                    || conformance.external_binding_source_span.is_some()))
                || (!inferred_console_intrinsic
                    && conformance.external_binding != authored_binding))
                || conformance.requirement.is_none()
                || !program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .is_empty()
            {
                return None;
            }
            let typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            } = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)?
            else {
                return None;
            };
            (definition.symbol == conformance.symbol
                && definition.is_boundary
                && (!inferred_console_intrinsic || definition.name.as_str() == "Console")
                && definition.lifetime_parameters.is_empty()
                && program.trait_type_parameters(definition).is_empty()
                && requirement.symbol == conformance.requirement_symbol
                && (!inferred_console_intrinsic
                    || machine
                        .name
                        .as_str()
                        .strip_prefix("ConsoleNativeProvider::")
                        == Some(requirement.name.as_str()))
                && requirement.lifetime_parameters.is_empty()
                && program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
                && requirement.native_callback_parameters.is_empty()
                && !requirement.suspends
                && !requirement.blocks
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

/// The supported byte-input result, resolved by declaration identity. This is
/// typed shape validation, not selected-package or native execution admission.
pub fn exact_byte_read_result_type(
    program: &TypedTrees,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    use typed_trees::data::DataMember;
    use typed_trees::types::TypeConstraintNode;

    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(result_type)
    else {
        return None;
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)?;
    if !symbol.is_valid()
        || data.name.as_str() != "ByteRead"
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
    let [DataMember::Variant(eof), DataMember::Variant(byte)] = program.data_members(data) else {
        return None;
    };
    if eof.name.as_str() != "Eof" || !eof.payload.is_empty() || byte.name.as_str() != "Byte" {
        return None;
    }
    let [field] = program.data_payload_fields(byte) else {
        return None;
    };
    if field.name.as_str() != "value"
        || field.relevance.is_erased()
        || program.primitive_type_reference(field.type_reference) != Some(PrimitiveType::I32)
    {
        return None;
    }
    // Ranges may narrow the carrier but must include every input byte. Other
    // domain/proof promises cannot be established by this host operation.
    let mut carrier = field.type_reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(carrier)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range { minimum, maximum } = constraint else {
                return None;
            };
            let minimum = crate::arithmetic_domains::literal_i64(program, *minimum)?;
            let maximum = crate::arithmetic_domains::literal_i64(program, *maximum)?;
            if minimum > 0 || maximum < 255 {
                return None;
            }
        }
        carrier = *base_type;
    }
    Some(*symbol)
}

fn exact_direct_intrinsic_signature(
    program: &TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    return_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    !parameter.is_self
        && !parameter.is_const
        && !parameter.is_mutable
        && program.primitive_type_reference(parameter.type_reference) == Some(PrimitiveType::I32)
        && {
            let mut current = return_type;
            loop {
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Unit => break true,
                    _ => break false,
                }
            }
        }
}
