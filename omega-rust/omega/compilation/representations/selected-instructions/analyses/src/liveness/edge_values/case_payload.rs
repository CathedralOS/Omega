//! Rejoin a narrow signed presentation to the original exact payload load.
use super::FunctionIndex;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockOrigin, SelectedInstructionKind, SelectedSuccessor, VirtualRegister,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, ScalarType};

pub(super) fn observed_source<'a>(
    index: &FunctionIndex<'a>,
    successor: &SelectedSuccessor,
    value: &'a VirtualRegister,
) -> Option<&'a VirtualRegister> {
    let psi_edge = successor.psi_edge?;
    if matches!(
        value.origin,
        VirtualRegisterOrigin::StructuralObservation { .. }
    ) {
        return Some(value);
    }
    let VirtualRegisterOrigin::AbiTransport {
        instruction,
        place,
        byte_offset,
    } = value.origin
    else {
        return None;
    };
    let ScalarType::Integer(integer) = value.scalar_type else {
        return None;
    };
    if integer.sign() != IntegerSign::Signed {
        return None;
    }
    let kind = match integer.bits() {
        8 => SelectedInstructionKind::SignExtendI8,
        16 => SelectedInstructionKind::SignExtendI16,
        32 => SelectedInstructionKind::SignExtendI32,
        _ => return None,
    };
    let (owner, normalization, position) = index.unique_instruction(instruction)?;
    if normalization.kind != kind
        || !matches!(owner.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
            if edge == psi_edge && target == successor.source_target)
        || normalization.provenance.edges != [psi_edge]
        || !normalization.provenance.operations.is_empty()
        || !normalization.provenance.fuel.is_empty()
    {
        return None;
    }
    let [input, output] = normalization.operands.as_slice() else {
        return None;
    };
    if input.access != RegisterOperandAccess::Use
        || output.access != RegisterOperandAccess::Def
        || output.virtual_register != value.id
        || input.virtual_register == value.id
        || input.class != value.class
        || output.class != value.class
    {
        return None;
    }
    let observed = index.unique_register(input.virtual_register)?;
    if observed.scalar_type != value.scalar_type
        || observed.class != value.class
        || observed.definition_site.is_some()
    {
        return None;
    }
    let VirtualRegisterOrigin::StructuralObservation {
        instruction: load_id,
        place: load_place,
        byte_offset: load_offset,
    } = observed.origin
    else {
        return None;
    };
    if load_place != place || load_offset != byte_offset {
        return None;
    }
    let (load_owner, load, load_position) = index.unique_instruction(load_id)?;
    let [address, loaded] = load.operands.as_slice() else {
        return None;
    };
    if !std::ptr::eq(load_owner, owner)
        || load_position >= position
        || load.provenance.edges != [psi_edge]
        || !matches!(
            (integer.bits(), load.kind),
            (8, SelectedInstructionKind::Load8 { .. })
                | (16, SelectedInstructionKind::Load16 { .. })
                | (32, SelectedInstructionKind::Load32 { .. })
        )
        || address.access != RegisterOperandAccess::Use
        || loaded.access != RegisterOperandAccess::Def
        || loaded.virtual_register != observed.id
        || loaded.class != observed.class
    {
        return None;
    }
    Some(observed)
}
