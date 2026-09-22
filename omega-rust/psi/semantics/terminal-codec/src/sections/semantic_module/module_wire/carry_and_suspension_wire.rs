//! Carry policies and suspension call plans, sites and targets on the
//! wire.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::scalar_wire::{decode_scalar_type, encode_scalar_type};
use crate::sections::semantic_module::wire::{Reader, Writer, decode_counted, decode_ids};
use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use terminal_psi::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionPlace, TerminalSuspensionStorage,
    TerminalSuspensionValueType,
};

fn encode_carry_policy(writer: &mut Writer, policy: CarryPolicy) {
    writer.u8(match policy.suspension {
        CarrySuspension::Forbidden => 1,
        CarrySuspension::Allowed => 2,
    });
    writer.u8(match policy.cpu {
        CarryCpu::Origin => 1,
        CarryCpu::Any => 2,
    });
    writer.u8(match policy.host_thread {
        CarryHostThread::Origin => 1,
        CarryHostThread::Any => 2,
    });
    writer.u8(match policy.address {
        CarryAddress::Stable => 1,
        CarryAddress::Movable => 2,
    });
}

fn decode_carry_policy(reader: &mut Reader<'_>) -> Result<CarryPolicy, CodecError> {
    Ok(CarryPolicy {
        suspension: match reader.u8()? {
            1 => CarrySuspension::Forbidden,
            2 => CarrySuspension::Allowed,
            tag => return Err(CodecError::InvalidTag("CarrySuspension", tag)),
        },
        cpu: match reader.u8()? {
            1 => CarryCpu::Origin,
            2 => CarryCpu::Any,
            tag => return Err(CodecError::InvalidTag("CarryCpu", tag)),
        },
        host_thread: match reader.u8()? {
            1 => CarryHostThread::Origin,
            2 => CarryHostThread::Any,
            tag => return Err(CodecError::InvalidTag("CarryHostThread", tag)),
        },
        address: match reader.u8()? {
            1 => CarryAddress::Stable,
            2 => CarryAddress::Movable,
            tag => return Err(CodecError::InvalidTag("CarryAddress", tag)),
        },
    })
}

pub(crate) fn encode_suspension_call_plan(
    writer: &mut Writer,
    plan: &TerminalSuspensionCallPlan,
) -> Result<(), CodecError> {
    writer.id(plan.operation);
    writer.id(plan.crossing);
    encode_suspension_call_target(writer, plan.target);
    encode_carry_policy(writer, plan.effective);
    writer.u32(plan.live_value_count);
    writer.len("suspension live values", plan.live_values.len())?;
    for live in &plan.live_values {
        match &live.place {
            TerminalSuspensionPlace::Scalar(value) => {
                writer.u8(1);
                writer.id(*value);
            }
            TerminalSuspensionPlace::Structural { place, path } => {
                writer.u8(2);
                writer.id(*place);
                writer.len("suspension structural path", path.len())?;
                for segment in path {
                    match segment {
                        terminal_psi::StructuralPathSegment::Field(field) => {
                            writer.u8(1);
                            writer.string("suspension structural field", field)?;
                        }
                        terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                            writer.u8(2);
                            writer.u64(*index);
                        }
                        terminal_psi::StructuralPathSegment::Referent => writer.u8(3),
                        terminal_psi::StructuralPathSegment::FixedByteRange { start, end } => {
                            writer.u8(4);
                            writer.u64(*start);
                            writer.u64(*end);
                        }
                    }
                }
            }
        }
        match live.value_type {
            TerminalSuspensionValueType::Scalar(scalar_type) => {
                writer.u8(1);
                encode_scalar_type(writer, scalar_type);
            }
            TerminalSuspensionValueType::Structural(structural_type) => {
                writer.u8(2);
                writer.id(structural_type);
            }
        }
        writer.u8(match live.storage {
            TerminalSuspensionStorage::Persistent => 1,
            TerminalSuspensionStorage::Parameter => 2,
            TerminalSuspensionStorage::Local => 3,
            TerminalSuspensionStorage::CallArgument => 4,
        });
        writer.u32(live.claim_count);
        writer.len("suspension live claims", live.claims.len())?;
        for claim in &live.claims {
            writer.id(*claim);
        }
        encode_carry_policy(writer, live.effective);
    }
    Ok(())
}

pub(crate) fn encode_suspension_call_site(writer: &mut Writer, site: &TerminalSuspensionCallSite) {
    writer.id(site.operation);
    writer.id(site.crossing);
    encode_suspension_call_target(writer, site.target);
    writer.bytes(&site.frontier_commitment);
}

pub(crate) fn decode_suspension_call_site(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallSite, CodecError> {
    Ok(TerminalSuspensionCallSite {
        operation: reader.id("OperationId")?,
        crossing: reader.id("SuspensionCrossingId")?,
        target: decode_suspension_call_target(reader)?,
        frontier_commitment: reader.array()?,
    })
}

fn encode_suspension_call_target(writer: &mut Writer, target: TerminalSuspensionCallTarget) {
    match target {
        TerminalSuspensionCallTarget::Machine(machine) => {
            writer.u8(1);
            writer.id(machine);
        }
        TerminalSuspensionCallTarget::Boundary(boundary) => {
            writer.u8(2);
            writer.id(boundary);
        }
        TerminalSuspensionCallTarget::DynamicDescriptor { ordinal } => {
            writer.u8(3);
            writer.u32(ordinal);
        }
        TerminalSuspensionCallTarget::DynamicParameter {
            parameter_ordinal,
            requirement_slot,
        } => {
            writer.u8(4);
            writer.u32(parameter_ordinal);
            writer.u32(requirement_slot);
        }
    }
}

fn decode_suspension_call_target(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallTarget, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalSuspensionCallTarget::Machine(
            reader.id("MachineId")?,
        )),
        2 => Ok(TerminalSuspensionCallTarget::Boundary(
            reader.id("BoundaryMachineId")?,
        )),
        3 => Ok(TerminalSuspensionCallTarget::DynamicDescriptor {
            ordinal: reader.u32()?,
        }),
        4 => Ok(TerminalSuspensionCallTarget::DynamicParameter {
            parameter_ordinal: reader.u32()?,
            requirement_slot: reader.u32()?,
        }),
        tag => Err(CodecError::InvalidTag("TerminalSuspensionCallTarget", tag)),
    }
}

pub(crate) fn decode_suspension_call_plan(
    reader: &mut Reader<'_>,
) -> Result<TerminalSuspensionCallPlan, CodecError> {
    let operation = reader.id("OperationId")?;
    let crossing = reader.id("SuspensionCrossingId")?;
    let target = decode_suspension_call_target(reader)?;
    let effective = decode_carry_policy(reader)?;
    let live_value_count = reader.u32()?;
    let live_values = decode_counted(reader, |reader| {
        let place = match reader.u8()? {
            1 => TerminalSuspensionPlace::Scalar(reader.id("ValueId")?),
            2 => TerminalSuspensionPlace::Structural {
                place: reader.id("PlaceId")?,
                path: decode_counted(reader, |reader| match reader.u8()? {
                    1 => Ok(terminal_psi::StructuralPathSegment::Field(
                        reader.string("suspension structural field")?,
                    )),
                    2 => Ok(terminal_psi::StructuralPathSegment::FixedIndex(
                        reader.u64()?,
                    )),
                    3 => Ok(terminal_psi::StructuralPathSegment::Referent),
                    4 => Ok(terminal_psi::StructuralPathSegment::FixedByteRange {
                        start: reader.u64()?,
                        end: reader.u64()?,
                    }),
                    tag => Err(CodecError::InvalidTag("StructuralPathSegment", tag)),
                })?,
            },
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionPlace", tag)),
        };
        let value_type = match reader.u8()? {
            1 => TerminalSuspensionValueType::Scalar(decode_scalar_type(reader)?),
            2 => TerminalSuspensionValueType::Structural(reader.id("StructuralTypeId")?),
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionValueType", tag)),
        };
        let storage = match reader.u8()? {
            1 => TerminalSuspensionStorage::Persistent,
            2 => TerminalSuspensionStorage::Parameter,
            3 => TerminalSuspensionStorage::Local,
            4 => TerminalSuspensionStorage::CallArgument,
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionStorage", tag)),
        };
        Ok(TerminalSuspensionLiveValue {
            place,
            value_type,
            storage,
            claim_count: reader.u32()?,
            claims: decode_ids(reader, "ClaimId")?,
            effective: decode_carry_policy(reader)?,
        })
    })?;
    Ok(TerminalSuspensionCallPlan {
        operation,
        crossing,
        target,
        effective,
        live_value_count,
        live_values,
    })
}
