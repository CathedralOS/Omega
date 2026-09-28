//! Borrow boundaries, owner paths, places, accesses and reborrow handoffs
//! on the wire.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::wire::{Reader, Writer, decode_counted};
use terminal_psi::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowRootHandoff, TerminalReborrowRootHandoffStep,
    TerminalReborrowSharedCohortMember,
};

fn encode_borrow_boundary(
    writer: &mut Writer,
    source: &TerminalBorrowBoundarySource,
) -> Result<(), CodecError> {
    match source {
        TerminalBorrowBoundarySource::Statement { statement_index } => {
            writer.u8(1);
            writer.u64(*statement_index);
        }
        TerminalBorrowBoundarySource::Call {
            statement_index,
            call_ordinal,
            target_identity,
        } => {
            writer.u8(2);
            writer.u64(*statement_index);
            writer.u64(*call_ordinal);
            writer.string("reborrow call target identity", target_identity)?;
        }
    }
    Ok(())
}

fn decode_borrow_boundary(
    reader: &mut Reader<'_>,
) -> Result<TerminalBorrowBoundarySource, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalBorrowBoundarySource::Statement {
            statement_index: reader.u64()?,
        }),
        2 => Ok(TerminalBorrowBoundarySource::Call {
            statement_index: reader.u64()?,
            call_ordinal: reader.u64()?,
            target_identity: reader.string("reborrow call target identity")?,
        }),
        tag => Err(CodecError::InvalidTag("TerminalBorrowBoundarySource", tag)),
    }
}

fn encode_owner_path(
    writer: &mut Writer,
    path: &[TerminalBorrowOwnerSegment],
) -> Result<(), CodecError> {
    writer.len("reborrow owner path", path.len())?;
    for segment in path {
        match segment {
            TerminalBorrowOwnerSegment::Field(identity) => {
                writer.u8(1);
                writer.string("reborrow owner field", identity)?;
            }
            TerminalBorrowOwnerSegment::Case(identity) => {
                writer.u8(2);
                writer.string("reborrow owner case", identity)?;
            }
            TerminalBorrowOwnerSegment::FixedIndex(index) => {
                writer.u8(3);
                writer.u64(*index);
            }
            TerminalBorrowOwnerSegment::DynamicIndex => writer.u8(4),
        }
    }
    Ok(())
}

fn decode_owner_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalBorrowOwnerSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(TerminalBorrowOwnerSegment::Field(
            reader.string("reborrow owner field")?,
        )),
        2 => Ok(TerminalBorrowOwnerSegment::Case(
            reader.string("reborrow owner case")?,
        )),
        3 => Ok(TerminalBorrowOwnerSegment::FixedIndex(reader.u64()?)),
        4 => Ok(TerminalBorrowOwnerSegment::DynamicIndex),
        tag => Err(CodecError::InvalidTag("TerminalBorrowOwnerSegment", tag)),
    })
}

fn encode_place_segments(
    writer: &mut Writer,
    segments: &[TerminalBorrowPlaceSegment],
) -> Result<(), CodecError> {
    writer.len("reborrow place segments", segments.len())?;
    for segment in segments {
        match segment {
            TerminalBorrowPlaceSegment::Field(identity) => {
                writer.u8(1);
                writer.string("reborrow place field", identity)?;
            }
            TerminalBorrowPlaceSegment::Case(identity) => {
                writer.u8(2);
                writer.string("reborrow place case", identity)?;
            }
            TerminalBorrowPlaceSegment::FixedIndex(index) => {
                writer.u8(3);
                writer.u64(*index);
            }
            TerminalBorrowPlaceSegment::FixedRange { start, end } => {
                writer.u8(4);
                writer.u64(*start);
                writer.u64(*end);
            }
        }
    }
    Ok(())
}

fn decode_place_segments(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalBorrowPlaceSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(TerminalBorrowPlaceSegment::Field(
            reader.string("reborrow place field")?,
        )),
        2 => Ok(TerminalBorrowPlaceSegment::Case(
            reader.string("reborrow place case")?,
        )),
        3 => Ok(TerminalBorrowPlaceSegment::FixedIndex(reader.u64()?)),
        4 => Ok(TerminalBorrowPlaceSegment::FixedRange {
            start: reader.u64()?,
            end: reader.u64()?,
        }),
        tag => Err(CodecError::InvalidTag("TerminalBorrowPlaceSegment", tag)),
    })
}

fn encode_place(writer: &mut Writer, place: &TerminalBorrowPlace) -> Result<(), CodecError> {
    writer.string("reborrow place root", &place.root_identity)?;
    encode_place_segments(writer, &place.segments)
}

fn decode_place(reader: &mut Reader<'_>) -> Result<TerminalBorrowPlace, CodecError> {
    Ok(TerminalBorrowPlace {
        root_identity: reader.string("reborrow place root")?,
        segments: decode_place_segments(reader)?,
    })
}

fn encode_borrow_access(writer: &mut Writer, access: StructuralAccess) {
    writer.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

fn decode_borrow_access(reader: &mut Reader<'_>) -> Result<StructuralAccess, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
    }
}

pub(crate) fn encode_reborrow_root_handoff(
    writer: &mut Writer,
    handoff: &TerminalReborrowRootHandoff,
) -> Result<(), CodecError> {
    writer.id(handoff.machine);
    writer.string(
        "reborrow machine identity",
        &handoff.source_machine_identity,
    )?;
    writer.string("reborrow state identity", &handoff.source_state_identity)?;
    writer.string("reborrow direct owner", &handoff.direct_root_owner_identity)?;
    encode_owner_path(writer, &handoff.direct_root_owner_path)?;
    encode_place(writer, &handoff.direct_root_place)?;
    encode_borrow_access(writer, handoff.direct_root_access);
    encode_borrow_boundary(writer, &handoff.direct_root_activation)?;
    encode_borrow_boundary(writer, &handoff.direct_root_weakening)?;
    writer.string(
        "reborrow direct-root lifetime",
        &handoff.direct_root_lifetime_identity,
    )?;
    writer.len("reborrow root-handoff lineage", handoff.lineage.len())?;
    for step in &handoff.lineage {
        writer.string("reborrow child owner", &step.child_owner_identity)?;
        encode_owner_path(writer, &step.child_owner_path)?;
        encode_place(writer, &step.child_place)?;
        encode_place_segments(writer, &step.projection_remainder)?;
        encode_borrow_access(writer, step.child_access);
        encode_borrow_boundary(writer, &step.child_activation)?;
        encode_borrow_boundary(writer, &step.formation_boundary)?;
        encode_borrow_boundary(writer, &step.child_weakening)?;
    }
    Ok(())
}

pub(crate) fn decode_reborrow_root_handoff(
    reader: &mut Reader<'_>,
) -> Result<TerminalReborrowRootHandoff, CodecError> {
    Ok(TerminalReborrowRootHandoff {
        machine: reader.id("MachineId")?,
        source_machine_identity: reader.string("reborrow machine identity")?,
        source_state_identity: reader.string("reborrow state identity")?,
        direct_root_owner_identity: reader.string("reborrow direct owner")?,
        direct_root_owner_path: decode_owner_path(reader)?,
        direct_root_place: decode_place(reader)?,
        direct_root_access: decode_borrow_access(reader)?,
        direct_root_activation: decode_borrow_boundary(reader)?,
        direct_root_weakening: decode_borrow_boundary(reader)?,
        direct_root_lifetime_identity: reader.string("reborrow direct-root lifetime")?,
        lineage: decode_counted(reader, |reader| {
            Ok(TerminalReborrowRootHandoffStep {
                child_owner_identity: reader.string("reborrow child owner")?,
                child_owner_path: decode_owner_path(reader)?,
                child_place: decode_place(reader)?,
                projection_remainder: decode_place_segments(reader)?,
                child_access: decode_borrow_access(reader)?,
                child_activation: decode_borrow_boundary(reader)?,
                formation_boundary: decode_borrow_boundary(reader)?,
                child_weakening: decode_borrow_boundary(reader)?,
            })
        })?,
    })
}

pub(crate) fn encode_reborrow_restored_call_use(
    writer: &mut Writer,
    use_row: &TerminalReborrowRestoredCallUse,
) -> Result<(), CodecError> {
    writer.id(use_row.machine);
    writer.id(use_row.operation);
    writer.u8(match use_row.restoration_class {
        TerminalReborrowRestorationClass::ExclusiveReactivation => 1,
        TerminalReborrowRestorationClass::SharedFreezeRestoration => 2,
    });
    encode_borrow_boundary(writer, &use_row.call_boundary)?;
    writer.id(use_row.call_target_machine);
    writer.string(
        "restored-use machine identity",
        &use_row.source_machine_identity,
    )?;
    writer.string(
        "restored-use state identity",
        &use_row.source_state_identity,
    )?;
    writer.string(
        "restored-use direct owner",
        &use_row.direct_root_owner_identity,
    )?;
    encode_owner_path(writer, &use_row.direct_root_owner_path)?;
    encode_place(writer, &use_row.direct_root_place)?;
    encode_borrow_boundary(writer, &use_row.direct_root_activation)?;
    encode_borrow_boundary(writer, &use_row.direct_root_weakening)?;
    writer.string(
        "restored-use direct-root lifetime",
        &use_row.direct_root_lifetime_identity,
    )?;
    writer.string("restored-use child owner", &use_row.child_owner_identity)?;
    encode_owner_path(writer, &use_row.child_owner_path)?;
    encode_place(writer, &use_row.child_place)?;
    encode_place_segments(writer, &use_row.projection_remainder)?;
    encode_borrow_access(writer, use_row.child_access);
    encode_borrow_boundary(writer, &use_row.child_activation)?;
    encode_borrow_boundary(writer, &use_row.formation_boundary)?;
    encode_borrow_boundary(writer, &use_row.child_weakening)?;
    writer.len("restored-use shared cohort", use_row.shared_cohort.len())?;
    for member in &use_row.shared_cohort {
        writer.string(
            "restored-use cohort child owner",
            &member.child_owner_identity,
        )?;
        encode_owner_path(writer, &member.child_owner_path)?;
        encode_place(writer, &member.child_place)?;
        encode_borrow_access(writer, member.child_access);
        encode_borrow_boundary(writer, &member.child_activation)?;
        encode_borrow_boundary(writer, &member.child_weakening)?;
    }
    Ok(())
}

pub(crate) fn decode_reborrow_restored_call_use(
    reader: &mut Reader<'_>,
) -> Result<TerminalReborrowRestoredCallUse, CodecError> {
    Ok(TerminalReborrowRestoredCallUse {
        machine: reader.id("MachineId")?,
        operation: reader.id("OperationId")?,
        restoration_class: match reader.u8()? {
            1 => TerminalReborrowRestorationClass::ExclusiveReactivation,
            2 => TerminalReborrowRestorationClass::SharedFreezeRestoration,
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalReborrowRestorationClass",
                    tag,
                ));
            }
        },
        call_boundary: decode_borrow_boundary(reader)?,
        call_target_machine: reader.id("MachineId")?,
        source_machine_identity: reader.string("restored-use machine identity")?,
        source_state_identity: reader.string("restored-use state identity")?,
        direct_root_owner_identity: reader.string("restored-use direct owner")?,
        direct_root_owner_path: decode_owner_path(reader)?,
        direct_root_place: decode_place(reader)?,
        direct_root_activation: decode_borrow_boundary(reader)?,
        direct_root_weakening: decode_borrow_boundary(reader)?,
        direct_root_lifetime_identity: reader.string("restored-use direct-root lifetime")?,
        child_owner_identity: reader.string("restored-use child owner")?,
        child_owner_path: decode_owner_path(reader)?,
        child_place: decode_place(reader)?,
        projection_remainder: decode_place_segments(reader)?,
        child_access: decode_borrow_access(reader)?,
        child_activation: decode_borrow_boundary(reader)?,
        formation_boundary: decode_borrow_boundary(reader)?,
        child_weakening: decode_borrow_boundary(reader)?,
        shared_cohort: decode_counted(reader, |reader| {
            Ok(TerminalReborrowSharedCohortMember {
                child_owner_identity: reader.string("restored-use cohort child owner")?,
                child_owner_path: decode_owner_path(reader)?,
                child_place: decode_place(reader)?,
                child_access: decode_borrow_access(reader)?,
                child_activation: decode_borrow_boundary(reader)?,
                child_weakening: decode_borrow_boundary(reader)?,
            })
        })?,
    })
}
