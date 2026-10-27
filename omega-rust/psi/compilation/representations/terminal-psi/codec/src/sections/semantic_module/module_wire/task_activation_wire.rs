//! Task activation demands on the wire.
//!
//! The bundle is a self-contained evidence section: demands plus the
//! bundle-local structural-type and target-layout tables their value types
//! reference. Identity indices reference the module identity table and are
//! encoded as raw `u32`s, exactly like structural referent identities.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::structural_type_wire::{decode_structural_type, encode_structural_type};
use super::super::wire::{Reader, Writer, decode_counted};
use super::carry_and_suspension_wire::{decode_carry_policy, encode_carry_policy};
use super::target_layout_wire;
use terminal_psi::{
    TerminalIdentityIndex, TerminalTaskActivationDemand, TerminalTaskActivationDemands,
    TerminalTaskCarryObligations, TerminalTaskCarryStorage, TerminalTaskFrame,
    TerminalTaskFrameCall, TerminalTaskLiveCarry, TerminalTaskResidentValue,
    TerminalTaskSignatureValue, TerminalTaskStartOperation, TerminalTaskSuspensionCrossing,
    TerminalTaskUnresolvedCall, TerminalTaskUnresolvedCallKind, TerminalTaskValueType,
};

fn encode_identity_index(writer: &mut Writer, index: TerminalIdentityIndex) {
    writer.u32(index.0);
}

fn decode_identity_index(reader: &mut Reader<'_>) -> Result<TerminalIdentityIndex, CodecError> {
    Ok(TerminalIdentityIndex(reader.u32()?))
}

fn encode_value_type(writer: &mut Writer, value_type: TerminalTaskValueType) {
    match value_type {
        TerminalTaskValueType::Scalar(scalar_type) => {
            writer.u8(1);
            encode_scalar_type(writer, scalar_type);
        }
        TerminalTaskValueType::Structural(structural_type) => {
            writer.u8(2);
            writer.id(structural_type);
        }
        TerminalTaskValueType::TargetLayout(application) => {
            writer.u8(3);
            writer.u32(application.0);
        }
    }
}

fn decode_value_type(reader: &mut Reader<'_>) -> Result<TerminalTaskValueType, CodecError> {
    Ok(match reader.u8()? {
        1 => TerminalTaskValueType::Scalar(decode_scalar_type(reader)?),
        2 => TerminalTaskValueType::Structural(reader.id("StructuralTypeId")?),
        3 => TerminalTaskValueType::TargetLayout(terminal_psi::LayoutApplicationId(reader.u32()?)),
        tag => return Err(CodecError::InvalidTag("TerminalTaskValueType", tag)),
    })
}

fn encode_signature_value(writer: &mut Writer, value: &TerminalTaskSignatureValue) {
    encode_value_type(writer, value.value_type);
    encode_identity_index(writer, value.normalized_identity);
    writer.boolean(value.is_mutable);
}

fn decode_signature_value(
    reader: &mut Reader<'_>,
) -> Result<TerminalTaskSignatureValue, CodecError> {
    Ok(TerminalTaskSignatureValue {
        value_type: decode_value_type(reader)?,
        normalized_identity: decode_identity_index(reader)?,
        is_mutable: reader.boolean()?,
    })
}

fn encode_live_carry(writer: &mut Writer, live: &TerminalTaskLiveCarry) -> Result<(), CodecError> {
    writer.u64(live.place);
    writer.u64(live.value_type);
    writer.u8(match live.storage {
        TerminalTaskCarryStorage::Persistent => 1,
        TerminalTaskCarryStorage::Parameter => 2,
        TerminalTaskCarryStorage::Local => 3,
        TerminalTaskCarryStorage::CallArgument => 4,
    });
    writer.len("task live carry claims", live.claims.len())?;
    for claim in &live.claims {
        writer.u64(*claim);
    }
    encode_carry_policy(writer, live.effective);
    Ok(())
}

fn decode_live_carry(reader: &mut Reader<'_>) -> Result<TerminalTaskLiveCarry, CodecError> {
    let place = reader.u64()?;
    let value_type = reader.u64()?;
    let storage = match reader.u8()? {
        1 => TerminalTaskCarryStorage::Persistent,
        2 => TerminalTaskCarryStorage::Parameter,
        3 => TerminalTaskCarryStorage::Local,
        4 => TerminalTaskCarryStorage::CallArgument,
        tag => return Err(CodecError::InvalidTag("TerminalTaskCarryStorage", tag)),
    };
    let claims = decode_counted(reader, |reader| reader.u64())?;
    let effective = decode_carry_policy(reader)?;
    Ok(TerminalTaskLiveCarry {
        place,
        value_type,
        storage,
        claims,
        effective,
    })
}

fn encode_crossing(
    writer: &mut Writer,
    crossing: &TerminalTaskSuspensionCrossing,
) -> Result<(), CodecError> {
    writer.u64(crossing.identity);
    writer.boolean(crossing.suspension_allowed);
    writer.boolean(crossing.preserve_cpu);
    writer.boolean(crossing.preserve_host_thread);
    writer.counted(
        "task crossing live carry",
        &crossing.live_carry,
        encode_live_carry,
    )?;
    Ok(())
}

fn decode_crossing(reader: &mut Reader<'_>) -> Result<TerminalTaskSuspensionCrossing, CodecError> {
    Ok(TerminalTaskSuspensionCrossing {
        identity: reader.u64()?,
        suspension_allowed: reader.boolean()?,
        preserve_cpu: reader.boolean()?,
        preserve_host_thread: reader.boolean()?,
        live_carry: decode_counted(reader, decode_live_carry)?,
    })
}

fn encode_frame(writer: &mut Writer, frame: &TerminalTaskFrame) -> Result<(), CodecError> {
    writer.u64(frame.identity);
    encode_identity_index(writer, frame.machine_callable_identity);
    encode_identity_index(writer, frame.entry);
    writer.len("task frame resident values", frame.resident.len())?;
    for resident in &frame.resident {
        encode_value_type(writer, resident.value_type);
        encode_identity_index(writer, resident.normalized_identity);
    }
    writer.len("task frame calls", frame.calls.len())?;
    for call in &frame.calls {
        writer.u64(call.callee);
    }
    writer.len("task frame unresolved calls", frame.unresolved_calls.len())?;
    for site in &frame.unresolved_calls {
        encode_identity_index(writer, site.state);
        writer.u32(site.statement_index);
        writer.u32(site.call_ordinal);
        writer.u8(match site.kind {
            TerminalTaskUnresolvedCallKind::UnresolvedTarget => 1,
            TerminalTaskUnresolvedCallKind::NonCheckedSupply => 2,
        });
    }
    Ok(())
}

fn decode_frame(reader: &mut Reader<'_>) -> Result<TerminalTaskFrame, CodecError> {
    let identity = reader.u64()?;
    let machine_callable_identity = decode_identity_index(reader)?;
    let entry = decode_identity_index(reader)?;
    let resident = decode_counted(reader, |reader| {
        Ok(TerminalTaskResidentValue {
            value_type: decode_value_type(reader)?,
            normalized_identity: decode_identity_index(reader)?,
        })
    })?;
    let calls = decode_counted(reader, |reader| {
        Ok(TerminalTaskFrameCall {
            callee: reader.u64()?,
        })
    })?;
    let unresolved_calls = decode_counted(reader, |reader| {
        Ok(TerminalTaskUnresolvedCall {
            state: decode_identity_index(reader)?,
            statement_index: reader.u32()?,
            call_ordinal: reader.u32()?,
            kind: match reader.u8()? {
                1 => TerminalTaskUnresolvedCallKind::UnresolvedTarget,
                2 => TerminalTaskUnresolvedCallKind::NonCheckedSupply,
                tag => {
                    return Err(CodecError::InvalidTag(
                        "TerminalTaskUnresolvedCallKind",
                        tag,
                    ));
                }
            },
        })
    })?;
    Ok(TerminalTaskFrame {
        identity,
        machine_callable_identity,
        entry,
        resident,
        calls,
        unresolved_calls,
    })
}

fn encode_demand(
    writer: &mut Writer,
    demand: &TerminalTaskActivationDemand,
) -> Result<(), CodecError> {
    writer.u8(match demand.operation {
        TerminalTaskStartOperation::Start => 1,
        TerminalTaskStartOperation::TryStart => 2,
    });
    writer.u64(demand.report_fingerprint);
    encode_identity_index(writer, demand.requirement_callable_identity);
    encode_identity_index(writer, demand.requirement_owner_path);
    encode_identity_index(writer, demand.requirement_owner_package);
    writer.boolean(demand.requirement_in_scope);
    encode_identity_index(writer, demand.authored_requirement_identity);
    writer.id(demand.machine);
    encode_identity_index(writer, demand.machine_callable_identity);
    encode_identity_index(writer, demand.entry);
    writer.u64(demand.entry_report_fingerprint);
    writer.u64(demand.contract_report_fingerprint);
    writer.bytes(&demand.specialization_commitment);
    writer.u64(demand.specialization_report_fingerprint);
    writer.boolean(demand.may_suspend);
    writer.boolean(demand.may_block);
    writer.boolean(demand.cancellation_required);
    writer.len("task signature arguments", demand.signature_arguments.len())?;
    for argument in &demand.signature_arguments {
        encode_signature_value(writer, argument);
    }
    encode_signature_value(writer, &demand.signature_outcome);
    writer.counted("task frames", &demand.frames, encode_frame)?;
    writer.counted("task crossings", &demand.crossings, encode_crossing)?;
    writer.boolean(demand.carry_obligations.preserve_cpu);
    writer.boolean(demand.carry_obligations.preserve_host_thread);
    Ok(())
}

fn decode_demand(reader: &mut Reader<'_>) -> Result<TerminalTaskActivationDemand, CodecError> {
    let operation = match reader.u8()? {
        1 => TerminalTaskStartOperation::Start,
        2 => TerminalTaskStartOperation::TryStart,
        tag => return Err(CodecError::InvalidTag("TerminalTaskStartOperation", tag)),
    };
    let report_fingerprint = reader.u64()?;
    let requirement_callable_identity = decode_identity_index(reader)?;
    let requirement_owner_path = decode_identity_index(reader)?;
    let requirement_owner_package = decode_identity_index(reader)?;
    let requirement_in_scope = reader.boolean()?;
    let authored_requirement_identity = decode_identity_index(reader)?;
    let machine = reader.id("MachineId")?;
    let machine_callable_identity = decode_identity_index(reader)?;
    let entry = decode_identity_index(reader)?;
    let entry_report_fingerprint = reader.u64()?;
    let contract_report_fingerprint = reader.u64()?;
    let specialization_commitment = reader.array::<32>()?;
    let specialization_report_fingerprint = reader.u64()?;
    let may_suspend = reader.boolean()?;
    let may_block = reader.boolean()?;
    let cancellation_required = reader.boolean()?;
    let signature_arguments = decode_counted(reader, decode_signature_value)?;
    let signature_outcome = decode_signature_value(reader)?;
    let frames = decode_counted(reader, decode_frame)?;
    let crossings = decode_counted(reader, decode_crossing)?;
    let carry_obligations = TerminalTaskCarryObligations {
        preserve_cpu: reader.boolean()?,
        preserve_host_thread: reader.boolean()?,
    };
    Ok(TerminalTaskActivationDemand {
        operation,
        report_fingerprint,
        requirement_callable_identity,
        requirement_owner_path,
        requirement_owner_package,
        requirement_in_scope,
        authored_requirement_identity,
        machine,
        machine_callable_identity,
        entry,
        entry_report_fingerprint,
        contract_report_fingerprint,
        specialization_commitment,
        specialization_report_fingerprint,
        may_suspend,
        may_block,
        cancellation_required,
        signature_arguments,
        signature_outcome,
        frames,
        crossings,
        carry_obligations,
    })
}

pub(super) fn encode_demands(
    writer: &mut Writer,
    demands: &TerminalTaskActivationDemands,
) -> Result<(), CodecError> {
    writer.counted("task activation demands", &demands.demands, encode_demand)?;
    writer.counted(
        "task activation structural types",
        &demands.structural_types,
        encode_structural_type,
    )?;
    writer.counted(
        "task activation target layout applications",
        &demands.target_layout_applications,
        target_layout_wire::encode,
    )?;
    Ok(())
}

pub(super) fn decode_demands(
    reader: &mut Reader<'_>,
) -> Result<TerminalTaskActivationDemands, CodecError> {
    Ok(TerminalTaskActivationDemands {
        demands: decode_counted(reader, decode_demand)?,
        structural_types: decode_counted(reader, decode_structural_type)?,
        target_layout_applications: decode_counted(reader, target_layout_wire::decode)?,
    })
}
