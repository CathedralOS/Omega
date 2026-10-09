//! Carry policies and suspension call plans, sites and targets on the
//! wire. Structural frontiers use the ordinary storage operand vocabulary:
//! neither serialization nor the frontier commitment may erase loan access.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::scalar_wire::{decode_scalar_type, encode_scalar_type};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use crate::sections::semantic_module::structural_signature_wire::{
    decode_structural_access, encode_structural_access,
};
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
            TerminalSuspensionPlace::Structural(argument) => {
                writer.u8(2);
                writer.id(argument.place);
                encode_structural_access(writer, argument.access);
                encode_structural_path(writer, "suspension structural path", &argument.path)?;
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
            2 => TerminalSuspensionPlace::Structural(terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            }),
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionPlace", tag)),
        };
        let value_type = match reader.u8()? {
            1 => TerminalSuspensionValueType::Scalar(decode_scalar_type(reader)?),
            2 => TerminalSuspensionValueType::Structural(reader.id("StructuralTypeId")?),
            tag => return Err(CodecError::InvalidTag("TerminalSuspensionValueType", tag)),
        };
        let storage = match reader.u8()? {
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

#[cfg(test)]
mod tests {
    use super::{decode_suspension_call_plan, encode_suspension_call_plan};
    use crate::codec_error::CodecError;
    use crate::sections::semantic_module::wire::{Reader, Writer};
    use language_semantics::CarryPolicy;
    use semantic_vocabulary::{
        MachineId, ObligationId, OperationId, PlaceId, StructuralCaseId, StructuralTypeId,
        SuspensionCrossingId, ValueId,
    };
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralPathSegment, TerminalSuspensionCallPlan,
        TerminalSuspensionCallTarget, TerminalSuspensionLiveValue, TerminalSuspensionPlace,
        TerminalSuspensionStorage, TerminalSuspensionValueType, suspension_frontier_commitment,
    };

    #[test]
    fn suspension_storage_access_survives_wire_and_changes_identity() {
        let mut encodings = Vec::new();
        let mut commitments = Vec::new();
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let selector = ValueId::new(7).expect("nonzero selector");
            let plan = TerminalSuspensionCallPlan {
                operation: OperationId::new(1).expect("nonzero operation"),
                crossing: SuspensionCrossingId::new(2).expect("nonzero crossing"),
                target: TerminalSuspensionCallTarget::Machine(
                    MachineId::new(3).expect("nonzero machine"),
                ),
                effective: CarryPolicy::STRICT,
                live_value_count: 1,
                live_values: vec![TerminalSuspensionLiveValue {
                    place: TerminalSuspensionPlace::Structural(StructuralArgument {
                        place: PlaceId::new(4).expect("nonzero place"),
                        access,
                        path: vec![
                            StructuralPathSegment::Case(
                                StructuralCaseId::new(5).expect("nonzero case"),
                            ),
                            StructuralPathSegment::Field("values".to_owned()),
                            StructuralPathSegment::Referent,
                            StructuralPathSegment::FixedIndex(2),
                            StructuralPathSegment::FixedByteRange { start: 1, end: 4 },
                            StructuralPathSegment::RuntimeIndex {
                                index: selector,
                                obligation: ObligationId::new(8).expect("nonzero obligation"),
                            },
                        ],
                    }),
                    value_type: TerminalSuspensionValueType::Structural(
                        StructuralTypeId::new(6).expect("nonzero structural type"),
                    ),
                    storage: TerminalSuspensionStorage::Local,
                    claim_count: 0,
                    claims: Vec::new(),
                    effective: CarryPolicy::STRICT,
                }],
            };
            let mut writer = Writer::default();
            encode_suspension_call_plan(&mut writer, &plan).expect("encode frontier");
            let encoded = writer.finish();
            let mut reader = Reader::new(&encoded);
            let decoded = decode_suspension_call_plan(&mut reader).expect("decode frontier");
            assert_eq!(decoded, plan);
            assert_eq!(reader.remaining(), 0);

            let mut retained_values = Vec::new();
            decoded.live_values[0]
                .place
                .visit_value_ids(|value| retained_values.push(value));
            assert_eq!(retained_values, vec![selector]);
            let commitment = suspension_frontier_commitment(&decoded);
            assert!(!encodings.contains(&encoded));
            assert!(!commitments.contains(&commitment));
            encodings.push(encoded);
            commitments.push(commitment);
        }
    }

    #[test]
    fn suspension_access_wire_rejects_accessless_module_format() {
        let mut encoded = super::super::super::MAGIC.to_vec();
        encoded.extend_from_slice(&130_u16.to_le_bytes());
        assert!(matches!(
            super::super::super::decode_module(&encoded),
            Err(CodecError::UnsupportedFormatMarker(130))
        ));
    }
}
