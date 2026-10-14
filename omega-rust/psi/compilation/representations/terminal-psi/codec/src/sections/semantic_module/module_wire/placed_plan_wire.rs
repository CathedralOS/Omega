//! Complete normalized placement transport; no address or host grant is encoded.
use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_counted};
use access_plans::{
    AccessExposure, AtomicPermissions, BoundaryReach, BoundaryServiceReachId, ExternalRead,
    FieldAccess, PlacementPlanSnapshot,
};
use layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
use terminal_psi::{
    TerminalIdentityIndex, TerminalPlacedAccessor, TerminalPlacedAccessorKind,
    TerminalPlacedFieldPlan, TerminalPlacedViewPlan,
};

pub(super) fn encode(writer: &mut Writer, plan: &TerminalPlacedViewPlan) -> Result<(), CodecError> {
    writer.id(plan.carrier);
    writer.id(plan.schema);
    for identity in [
        plan.view_identity,
        plan.policy_identity,
        plan.policy_plan_machine_identity,
        plan.schema_identity,
    ] {
        writer.u32(identity.0);
    }
    let snapshot = plan.placement.snapshot();
    writer.u64(snapshot.layout.schema_report_fingerprint);
    optional_u64(writer, snapshot.layout.size);
    writer.u64(snapshot.layout.align);
    writer.boolean(snapshot.layout.offsets.is_some());
    if let Some(offsets) = &snapshot.layout.offsets {
        writer.len("placement offsets", offsets.len())?;
        for offset in offsets {
            writer.u64(*offset);
        }
    }
    writer.len("placement layout entries", snapshot.layout.entries.len())?;
    for entry in &snapshot.layout.entries {
        writer.string("normalized placement field", &entry.field)?;
        optional_u64(writer, entry.member_identity);
        match entry.placement {
            LayoutPlacementReport::At { offset } => {
                writer.u8(0);
                writer.u64(offset);
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                writer.u8(1);
                writer.u64(offset);
                writer.u64(stored_width);
                writer.u8(match interpretation {
                    IntegerInterpretation::Signed => 0,
                    IntegerInterpretation::Unsigned => 1,
                });
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                writer.u8(2);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    writer.u64(value);
                }
            }
        }
    }
    writer.len("placement access entries", snapshot.access.len())?;
    for access in &snapshot.access {
        encode_access(writer, access);
    }
    writer.len("placement reach", snapshot.reach.services().len())?;
    for service in snapshot.reach.services() {
        writer.u64(service.normalized_identity());
    }
    writer.len("placed field contracts", plan.fields.len())?;
    for field in &plan.fields {
        writer.u32(field.access_slot);
        writer.u32(field.view_field.0);
        writer.u32(field.schema_field.0);
        writer.id(field.accessor);
        writer.len("placed accessor contracts", field.operations.len())?;
        for operation in &field.operations {
            writer.id(operation.boundary);
            writer.u8(match operation.kind {
                TerminalPlacedAccessorKind::Read => 0,
                TerminalPlacedAccessorKind::Take => 1,
                TerminalPlacedAccessorKind::Write => 2,
            });
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<TerminalPlacedViewPlan, CodecError> {
    let carrier = reader.id("placed carrier")?;
    let schema = reader.id("placed schema")?;
    let view_identity = TerminalIdentityIndex(reader.u32()?);
    let policy_identity = TerminalIdentityIndex(reader.u32()?);
    let policy_plan_machine_identity = TerminalIdentityIndex(reader.u32()?);
    let schema_identity = TerminalIdentityIndex(reader.u32()?);
    let schema_report_fingerprint = reader.u64()?;
    let size = decode_optional_u64(reader)?;
    let align = reader.u64()?;
    let offsets = if reader.boolean()? {
        Some(decode_counted(reader, |reader| reader.u64())?)
    } else {
        None
    };
    let entries = decode_counted(reader, |reader| {
        let field = reader.string("normalized placement field")?;
        let member_identity = decode_optional_u64(reader)?;
        let placement = match reader.u8()? {
            0 => LayoutPlacementReport::At {
                offset: reader.u64()?,
            },
            1 => LayoutPlacementReport::IntegerAt {
                offset: reader.u64()?,
                stored_width: reader.u64()?,
                interpretation: match reader.u8()? {
                    0 => IntegerInterpretation::Signed,
                    1 => IntegerInterpretation::Unsigned,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "placement integer interpretation",
                            tag,
                        ));
                    }
                },
            },
            2 => LayoutPlacementReport::Bits {
                container: reader.u64()?,
                container_width: reader.u64()?,
                destination_lsb: reader.u64()?,
                source_lsb: reader.u64()?,
                width: reader.u64()?,
            },
            tag => return Err(CodecError::InvalidTag("placement layout", tag)),
        };
        Ok(LayoutFieldEntryReport {
            field,
            member_identity,
            placement,
        })
    })?;
    let access = decode_counted(reader, decode_access)?;
    let services = decode_counted(reader, |reader| {
        BoundaryServiceReachId::from_normalized_identity(reader.u64()?)
            .map_err(|error| CodecError::PlacementPlan(error.to_string()))
    })?;
    // Foreign ingress rejoins canonical normalized keys/geometry. This does
    // not validate or acquire actual backing, provider custody or rights.
    let placement = PlacementPlanSnapshot {
        layout: LayoutPlanReport {
            schema_report_fingerprint,
            entries,
            offsets,
            size,
            align,
        },
        access,
        reach: BoundaryReach::from_services(services),
    }
    .validate()
    .map_err(|error| CodecError::PlacementPlan(error.to_string()))?;
    let fields = decode_counted(reader, |reader| {
        let access_slot = reader.u32()?;
        let view_field = TerminalIdentityIndex(reader.u32()?);
        let schema_field = TerminalIdentityIndex(reader.u32()?);
        let accessor = reader.id("placed accessor carrier")?;
        let operations = decode_counted(reader, |reader| {
            let boundary = reader.id("placed accessor boundary")?;
            let kind = match reader.u8()? {
                0 => TerminalPlacedAccessorKind::Read,
                1 => TerminalPlacedAccessorKind::Take,
                2 => TerminalPlacedAccessorKind::Write,
                tag => return Err(CodecError::InvalidTag("placed accessor operation", tag)),
            };
            Ok(TerminalPlacedAccessor { boundary, kind })
        })?;
        Ok(TerminalPlacedFieldPlan {
            access_slot,
            view_field,
            schema_field,
            accessor,
            operations,
        })
    })?;
    Ok(TerminalPlacedViewPlan {
        carrier,
        schema,
        view_identity,
        policy_identity,
        policy_plan_machine_identity,
        schema_identity,
        placement,
        fields,
    })
}

fn optional_u64(writer: &mut Writer, value: Option<u64>) {
    writer.boolean(value.is_some());
    if let Some(value) = value {
        writer.u64(value);
    }
}
fn decode_optional_u64(reader: &mut Reader<'_>) -> Result<Option<u64>, CodecError> {
    if reader.boolean()? {
        Ok(Some(reader.u64()?))
    } else {
        Ok(None)
    }
}
fn exposure(writer: &mut Writer, value: AccessExposure) {
    writer.u8(match value {
        AccessExposure::Exported => 0,
        AccessExposure::BindingPrivate => 1,
    });
}
fn decode_exposure(reader: &mut Reader<'_>) -> Result<AccessExposure, CodecError> {
    match reader.u8()? {
        0 => Ok(AccessExposure::Exported),
        1 => Ok(AccessExposure::BindingPrivate),
        tag => Err(CodecError::InvalidTag("placed access exposure", tag)),
    }
}
fn encode_access(writer: &mut Writer, access: &FieldAccess) {
    match access {
        FieldAccess::Inaccessible => writer.u8(0),
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            exposure: scope,
        } => {
            writer.u8(1);
            writer.u16(*transfer_width_bits);
            writer.boolean(*read);
            writer.boolean(*write);
            exposure(writer, *scope);
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            exposure: scope,
        } => {
            writer.u8(2);
            writer.u16(*transfer_width_bits);
            writer.u8(match read {
                ExternalRead::None => 0,
                ExternalRead::Read => 1,
                ExternalRead::Take => 2,
            });
            writer.boolean(*write);
            exposure(writer, *scope);
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            exposure: scope,
        } => {
            writer.u8(3);
            writer.u16(*transfer_width_bits);
            for value in [
                operations.load,
                operations.store,
                operations.fetch_add,
                operations.fetch_sub,
                operations.fetch_xor,
                operations.fetch_or,
                operations.fetch_and,
                operations.swap,
                operations.compare_exchange,
                operations.compare_exchange_once,
                operations.try_exchange,
                operations.try_exchange_once,
            ] {
                writer.boolean(value);
            }
            exposure(writer, *scope);
        }
    }
}
fn decode_access(reader: &mut Reader<'_>) -> Result<FieldAccess, CodecError> {
    Ok(match reader.u8()? {
        0 => FieldAccess::Inaccessible,
        1 => FieldAccess::Stable {
            transfer_width_bits: reader.u16()?,
            read: reader.boolean()?,
            write: reader.boolean()?,
            exposure: decode_exposure(reader)?,
        },
        2 => FieldAccess::External {
            transfer_width_bits: reader.u16()?,
            read: match reader.u8()? {
                0 => ExternalRead::None,
                1 => ExternalRead::Read,
                2 => ExternalRead::Take,
                tag => return Err(CodecError::InvalidTag("placed external read", tag)),
            },
            write: reader.boolean()?,
            exposure: decode_exposure(reader)?,
        },
        3 => FieldAccess::Atomic {
            transfer_width_bits: reader.u16()?,
            operations: AtomicPermissions {
                load: reader.boolean()?,
                store: reader.boolean()?,
                fetch_add: reader.boolean()?,
                fetch_sub: reader.boolean()?,
                fetch_xor: reader.boolean()?,
                fetch_or: reader.boolean()?,
                fetch_and: reader.boolean()?,
                swap: reader.boolean()?,
                compare_exchange: reader.boolean()?,
                compare_exchange_once: reader.boolean()?,
                try_exchange: reader.boolean()?,
                try_exchange_once: reader.boolean()?,
            },
            exposure: decode_exposure(reader)?,
        },
        tag => return Err(CodecError::InvalidTag("placed field access", tag)),
    })
}
