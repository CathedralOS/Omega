//! Authenticate the private descriptor operations and their borrowed storage.
use crate::CheckingInput;
use language_semantics::{MachineSupplyMode, ReferenceAccess, ServiceReachId};
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn boundary_state<'a>(
    program: &'a dyn CheckingInput,
    machine: &'a Machine,
) -> Result<&'a typed_trees::state::State, &'static str> {
    if machine.body_is_present
        || machine.supply_mode != MachineSupplyMode::Boundary
        || machine.suspends
        || machine.blocks
        || !machine.lifetime_parameters.is_empty()
        || !machine.contracts.is_empty()
        || !machine.invokes.is_empty()
        || machine.spelling.is_some()
        || !program.machine_trait_conformances(machine).is_empty()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.target_family_interface
        || !machine.conformance_bounds.is_empty()
        || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
        || !machine.owned_data.is_empty()
        || machine.admitted_same_stack_bytes != 0
        || machine.service_reach_is_installation_bound
        || (program.machine_specialization(machine.symbol).is_none() && machine.is_public)
    {
        return Err(
            "descriptor operation primitive requires its private bodyless core declaration",
        );
    }
    let [state] = program.machine_states(machine) else {
        return Err("descriptor operation declaration must have one entry signature");
    };
    if !state.contracts.is_empty() {
        return Err("descriptor operation primitive must not add entry contracts");
    }
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
    {
        return Err("descriptor operation primitive must retain every runtime operand");
    }
    Ok(state)
}

pub(super) fn machine_control_service(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<ServiceReachId, &'static str> {
    let [service] = program
        .service_reach_rows
        .services(machine.service_reach_row)
    else {
        return Err("descriptor operation primitive must reach exactly MachineControl");
    };
    let definition = program
        .service_reaches
        .definition(*service)
        .ok_or("descriptor operation MachineControl service has no declaration")?;
    if program.symbol_toolchain_relative_path(definition.symbol)
        != Some(std::path::Path::new("assembly.omg"))
        || program
            .normalized_hermetic_symbol_identity(definition.symbol)
            .ok()
            .as_deref()
            != Some("toolchain::MachineControl")
    {
        return Err(
            "descriptor operation primitive must reach the exact core MachineControl service",
        );
    }
    Ok(*service)
}

pub(super) fn plain_primitive(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    primitive: PrimitiveType,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    let expected = match primitive {
        PrimitiveType::U8 => symbols::BuiltinTypeAtom::U8,
        PrimitiveType::U16 => symbols::BuiltinTypeAtom::U16,
        PrimitiveType::U64 => symbols::BuiltinTypeAtom::U64,
        _ => return false,
    };
    program.symbols.builtin_type_atom(*symbol) == Some(expected)
}

pub(super) fn array_loan(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    access: ReferenceAccess,
    element: PrimitiveType,
) -> Result<(TypeReferenceHandle, &FixedArrayLength), &'static str> {
    let TypeReferenceNode::Reference {
        referee,
        access: actual,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return Err("descriptor operation storage must be an ordinary reference");
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(*referee)
    else {
        return Err("descriptor operation storage must be an unqualified fixed array");
    };
    if *actual != access || !plain_primitive(program, *element_type, element) {
        return Err("descriptor operation storage has different element or access authority");
    }
    Ok((*referee, length))
}
pub(super) fn fixed_array_length(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    access: ReferenceAccess,
    element: PrimitiveType,
) -> Result<usize, &'static str> {
    let (array, _) = array_loan(program, reference, access, element)?;
    program
        .closed_fixed_array_length(array)
        .ok_or("descriptor operation storage extent is not closed")
}

// First specialization may reuse the template symbol. Compare the current
// extent with its retained argument instead of assuming the template stayed open.
pub(super) fn extent_matches_application(
    program: &dyn CheckingInput,
    machine: &Machine,
    extent: SymbolHandle,
    length: &FixedArrayLength,
) -> Result<bool, &'static str> {
    let substitution = if let Some(application) = program.machine_specialization(machine.symbol) {
        let Some(typed_trees::static_applications::StaticMachineBindingValue::Const(actual)) =
            program.specialization_binding(application, extent)
        else {
            return Err("descriptor operation extent has no exact retained const argument");
        };
        Some((extent, *actual))
    } else {
        None
    };
    let expected = FixedArrayLength::ConstParameter {
        symbol: extent,
        name: Default::default(),
    };
    let expected = program.fixed_array_length_identity_with_simultaneous_substitutions(
        &expected,
        substitution.as_slice(),
    );
    let actual = program.fixed_array_length_identity(length, &[], &[]);
    Ok(expected.is_some() && expected == actual)
}
