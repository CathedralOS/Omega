use crate::CodecError;
use crate::fields::{Field, Flag, Id, Ids, Index, List, Nominal, Optional, OptionalId, Text, U64};
use crate::wire::{Reader, Writer, decode_canonical, decode_counted};
use crate::{wire_enum, wire_record};
use terminal_psi::{
    TargetLayoutInput, TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalAccessOwner, TerminalAccessProfileRequirement,
    TerminalAccessStorage,
};

/// Writes each lifecycle with the placement and storage its operations use.
pub fn encode_access_lifecycle_catalog(
    plans: &[TerminalAccessLifecyclePlan],
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.counted("access lifecycle plans", plans, encode)?;
    Ok(writer.finish())
}

pub fn decode_access_lifecycle_catalog(
    bytes: &[u8],
) -> Result<Vec<TerminalAccessLifecyclePlan>, CodecError> {
    decode_canonical(
        bytes,
        |reader| decode_counted(reader, decode),
        |plans| encode_access_lifecycle_catalog(plans),
    )
}

wire_record! {
    Target: TargetLayoutInput {
        target: Text,
        pointer_size: U64,
        pointer_alignment: U64,
        case_tag_size: U64,
        case_tag_alignment: U64,
        little_endian: Flag,
    }
    ProfileRequirement: TerminalAccessProfileRequirement {
        boundary: Id,
        owner: Nominal,
        callable: Index,
    }
}

wire_enum! {
    Operation: TerminalAccessLifecycleOperation, "access lifecycle operation" {
        0 => Initialize,
        1 => MoveOut,
        2 => View,
        3 => Relocate,
    }
    Execution: TerminalAccessLifecycleExecution, "access lifecycle execution" {
        0 => SelectedProvider,
        1 => CompilerPrimitive {
            profile_type: OptionalId,
            profile_requirements: List<ProfileRequirement>,
        },
    }
    Owner: TerminalAccessOwner, "placement content owner" {
        0 => Extent,
        1 => Run { carrier: Id, storage: Id, count: Id, singleton: OptionalId },
    }
}

pub fn encode(writer: &mut Writer, plan: &TerminalAccessLifecyclePlan) -> Result<(), CodecError> {
    writer.id(plan.boundary);
    Optional::<Target>::write(writer, &plan.target)?;
    Operation::write(writer, &plan.operation)?;
    Execution::write(writer, &plan.execution)?;
    writer.id(plan.schema);
    Ids::write(writer, &plan.image_fields)?;
    let storage = &plan.storage;
    writer.id(storage.carrier);
    Owner::write(writer, &storage.owner)?;
    writer.id(storage.base);
    writer.id(storage.length);
    writer.id(storage.granted);
    // A view borrows content it does not own, so it names no content domains.
    if plan.operation == TerminalAccessLifecycleOperation::View {
        if storage.vacant.is_some() || storage.resident.is_some() {
            return Err(CodecError::NonCanonicalEncoding);
        }
    } else {
        let (vacant, resident) = storage
            .owned_content()
            .map_err(|_| CodecError::NonCanonicalEncoding)?;
        writer.id(vacant);
        writer.id(resident);
    }
    for identity in [
        plan.policy_identity,
        plan.policy_plan_machine_identity,
        plan.schema_identity,
    ] {
        Index::write(writer, &identity)?;
    }
    crate::access::views::encode_placement(writer, &plan.placement)
}

pub fn decode(reader: &mut Reader<'_>) -> Result<TerminalAccessLifecyclePlan, CodecError> {
    let boundary = Id::read(reader)?;
    let target = Optional::<Target>::read(reader)?;
    let operation = Operation::read(reader)?;
    let execution = Execution::read(reader)?;
    let schema = Id::read(reader)?;
    let image_fields = Ids::read(reader)?;
    let carrier = Id::read(reader)?;
    let owner = Owner::read(reader)?;
    let base = Id::read(reader)?;
    let length = Id::read(reader)?;
    let granted = Id::read(reader)?;
    let (vacant, resident) = if operation == TerminalAccessLifecycleOperation::View {
        (None, None)
    } else {
        (Some(Id::read(reader)?), Some(Id::read(reader)?))
    };
    Ok(TerminalAccessLifecyclePlan {
        boundary,
        target,
        operation,
        execution,
        schema,
        image_fields,
        storage: TerminalAccessStorage {
            carrier,
            owner,
            base,
            length,
            granted,
            vacant,
            resident,
        },
        policy_identity: Index::read(reader)?,
        policy_plan_machine_identity: Index::read(reader)?,
        schema_identity: Index::read(reader)?,
        placement: crate::access::views::decode_placement(reader)?,
    })
}
