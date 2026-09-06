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
    if state.symbol != target_state_symbol
        || !exact_direct_intrinsic_signature(
            program,
            program.state_parameters(state),
            state.return_type,
        )
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
                && exact_direct_intrinsic_signature(
                    program,
                    program.state_signature_parameters(requirement),
                    requirement.return_type,
                ))
            .then_some(requirement.symbol)
        });
    let requirement = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some((requirement, machine.attached_data_symbol))
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
