use crate::CodecError;
use crate::fields::{Field, Flag, Id, Index, List, Optional, Text, U16, U32, U64};
use crate::wire::{Reader, Writer, decode_canonical, decode_counted};
use crate::{wire_enum, wire_record};
use access_admission::{PlacementSnapshot, ValidatedPlacement};
use access_vocabulary::{
    AccessExposure, AccessPermissions, AtomicPermissions, BoundaryReach, BoundaryServiceReachId,
    ExternalRead, FieldAccess, ObservationModel,
};
use layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
use terminal_psi::{
    TerminalAccessFieldPlan, TerminalAccessViewPlan, TerminalAccessor, TerminalAccessorKind,
};

pub fn encode_access_view_catalog(plans: &[TerminalAccessViewPlan]) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.counted("access view plans", plans, encode)?;
    Ok(writer.finish())
}

pub fn decode_access_view_catalog(bytes: &[u8]) -> Result<Vec<TerminalAccessViewPlan>, CodecError> {
    decode_canonical(
        bytes,
        |reader| decode_counted(reader, decode),
        |plans| encode_access_view_catalog(plans),
    )
}

pub fn encode(writer: &mut Writer, plan: &TerminalAccessViewPlan) -> Result<(), CodecError> {
    ViewPlan::write(writer, plan)
}

pub fn decode(reader: &mut Reader<'_>) -> Result<TerminalAccessViewPlan, CodecError> {
    ViewPlan::read(reader)
}

pub fn encode_placement(
    writer: &mut Writer,
    placement: &ValidatedPlacement,
) -> Result<(), CodecError> {
    ValidatedLayout::write(writer, placement)
}

pub fn decode_placement(reader: &mut Reader<'_>) -> Result<ValidatedPlacement, CodecError> {
    ValidatedLayout::read(reader)
}

wire_record! {
    ViewPlan: TerminalAccessViewPlan {
        carrier: Id,
        schema: Id,
        view_identity: Index,
        policy_identity: Index,
        policy_plan_machine_identity: Index,
        schema_identity: Index,
        placement: ValidatedLayout,
        fields: List<FieldPlan>,
    }
    FieldPlan: TerminalAccessFieldPlan {
        access_slot: U32,
        view_field: Index,
        schema_field: Index,
        accessor: Id,
        operations: List<Accessor>,
    }
    Accessor: TerminalAccessor { boundary: Id, kind: AccessorKind }
    Layout: LayoutPlanReport {
        schema_report_fingerprint: U64,
        size: Optional<U64>,
        align: U64,
        offsets: Optional<List<U64>>,
        entries: List<LayoutEntry>,
    }
    LayoutEntry: LayoutFieldEntryReport {
        field: Text,
        member_identity: Optional<U64>,
        placement: LayoutSlot,
    }
    Atomics: AtomicPermissions {
        load: Flag,
        store: Flag,
        fetch_add: Flag,
        fetch_sub: Flag,
        fetch_xor: Flag,
        fetch_or: Flag,
        fetch_and: Flag,
        swap: Flag,
        compare_exchange: Flag,
        compare_exchange_once: Flag,
        try_exchange: Flag,
        try_exchange_once: Flag,
    }
}

wire_enum! {
    AccessorKind: TerminalAccessorKind, "accessor operation" { 0 => Read, 1 => Take, 2 => Write }
    Interpretation: IntegerInterpretation, "placement integer interpretation" {
        0 => Signed,
        1 => Unsigned,
    }
    LayoutSlot: LayoutPlacementReport, "placement layout" {
        0 => At { offset: U64 },
        1 => IntegerAt { offset: U64, stored_width: U64, interpretation: Interpretation },
        2 => Bits {
            container: U64,
            container_width: U64,
            destination_lsb: U64,
            source_lsb: U64,
            width: U64,
        },
    }
    Exposure: AccessExposure, "placed access exposure" { 0 => Exported, 1 => BindingPrivate }
    ExternalReadKind: ExternalRead, "placed external read" { 0 => None, 1 => Read, 2 => Take }
    // A validated placement never gives a repeated region atomic observation.
    RegionObservation: ObservationModel, "placed repeated observation" {
        0 => Stable,
        1 => External;
        refuse ObservationModel::Atomic => CodecError::NonCanonicalEncoding,
    }
    FieldAccessKind: FieldAccess, "access field access" {
        0 => Inaccessible,
        5 => Region {
            byte_length: U64,
            byte_alignment: U64,
            transfer_width_bits: U16,
            observation: RegionObservation,
            permissions: RegionPermissions,
            exposure: Exposure,
        },
        1 => Stable { transfer_width_bits: U16, read: Flag, write: Flag, exposure: Exposure },
        2 => External {
            transfer_width_bits: U16,
            read: ExternalReadKind,
            write: Flag,
            exposure: Exposure,
        },
        3 => Atomic { transfer_width_bits: U16, operations: Atomics, exposure: Exposure },
    }
}

/// A region carries only ordinary permissions; placement validation refuses
/// atomic ones on a region, so the atomic set is empty on both sides.
struct RegionPermissions;
impl Field<AccessPermissions> for RegionPermissions {
    fn write(writer: &mut Writer, value: &AccessPermissions) -> Result<(), CodecError> {
        if value.atomic.any() {
            return Err(CodecError::NonCanonicalEncoding);
        }
        writer.boolean(value.read);
        writer.boolean(value.take);
        writer.boolean(value.write);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<AccessPermissions, CodecError> {
        Ok(AccessPermissions {
            read: reader.boolean()?,
            take: reader.boolean()?,
            write: reader.boolean()?,
            ..Default::default()
        })
    }
}

/// A placement is decoded through its snapshot so the receiver re-validates it.
pub struct ValidatedLayout;
impl Field<ValidatedPlacement> for ValidatedLayout {
    fn write(writer: &mut Writer, placement: &ValidatedPlacement) -> Result<(), CodecError> {
        let snapshot = placement.snapshot();
        Layout::write(writer, &snapshot.layout)?;
        List::<FieldAccessKind>::write(writer, &snapshot.access)?;
        let services = snapshot.reach.services();
        writer.len("placement reach", services.len())?;
        for service in services {
            writer.u64(service.normalized_identity());
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ValidatedPlacement, CodecError> {
        let layout = Layout::read(reader)?;
        let access = List::<FieldAccessKind>::read(reader)?;
        let services = decode_counted(reader, |reader| {
            BoundaryServiceReachId::from_normalized_identity(reader.u64()?)
                .map_err(|error| CodecError::AccessPlan(error.to_string()))
        })?;
        PlacementSnapshot {
            layout,
            access,
            reach: BoundaryReach::from_services(services),
        }
        .validate()
        .map_err(|error| CodecError::AccessPlan(error.to_string()))
    }
}
