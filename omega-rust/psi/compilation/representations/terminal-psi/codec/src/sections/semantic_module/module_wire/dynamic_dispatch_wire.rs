//! Dynamic-dispatch catalog on the wire: selectable interfaces with their
//! slot call shapes, and one row table per selected named implementation.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::structural_signature_wire::{decode_structural_access, encode_structural_access};
use super::super::wire::{Reader, Writer, decode_counted};
use terminal_psi::{
    StructuralMultiplicity, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicSlot, TerminalDynamicStructuralFormal, TerminalDynamicTable,
};

pub(super) fn encode_dynamic_dispatch(
    writer: &mut Writer,
    catalog: &TerminalDynamicDispatchCatalog,
) -> Result<(), CodecError> {
    writer.len("dynamic interfaces", catalog.interfaces.len())?;
    for interface in &catalog.interfaces {
        writer.u32(interface.id);
        writer.string("dynamic interface identity", &interface.identity)?;
        writer.len("dynamic interface slots", interface.slots.len())?;
        for slot in &interface.slots {
            writer.u32(slot.slot);
            writer.string(
                "dynamic slot declaring trait",
                &slot.declaring_trait_identity,
            )?;
            writer.string("dynamic slot requirement", &slot.requirement_identity)?;
            writer.strings("dynamic slot family tuple", &slot.family_tuple)?;
            writer.len(
                "dynamic slot scalar parameters",
                slot.scalar_parameters.len(),
            )?;
            for scalar_type in &slot.scalar_parameters {
                encode_scalar_type(writer, *scalar_type);
            }
            writer.len(
                "dynamic slot structural parameters",
                slot.structural_parameters.len(),
            )?;
            for formal in &slot.structural_parameters {
                writer.u32(formal.position);
                writer.id(formal.structural_type);
                encode_structural_access(writer, formal.access);
                writer.u8(match formal.multiplicity {
                    StructuralMultiplicity::Unrestricted => 1,
                    StructuralMultiplicity::Affine => 2,
                    StructuralMultiplicity::Linear => 3,
                });
            }
            match slot.result {
                None => writer.u8(0),
                Some(result) => {
                    writer.u8(1);
                    encode_scalar_type(writer, result);
                }
            }
        }
    }
    writer.len("dynamic tables", catalog.tables.len())?;
    for table in &catalog.tables {
        writer.u32(table.id);
        writer.u32(table.interface);
        writer.string(
            "dynamic table implementation",
            &table.implementation_identity,
        )?;
        writer.len("dynamic table rows", table.rows.len())?;
        for row in &table.rows {
            match row {
                Some(machine) => {
                    writer.u8(1);
                    writer.id(*machine);
                }
                None => writer.u8(0),
            }
        }
    }
    Ok(())
}

pub(super) fn decode_dynamic_dispatch(
    reader: &mut Reader<'_>,
) -> Result<TerminalDynamicDispatchCatalog, CodecError> {
    let interfaces = decode_counted(reader, |reader| {
        Ok(TerminalDynamicInterface {
            id: reader.u32()?,
            identity: reader.string("dynamic interface identity")?,
            slots: decode_counted(reader, |reader| {
                Ok(TerminalDynamicSlot {
                    slot: reader.u32()?,
                    declaring_trait_identity: reader.string("dynamic slot declaring trait")?,
                    requirement_identity: reader.string("dynamic slot requirement")?,
                    family_tuple: reader.strings("dynamic slot family tuple")?,
                    scalar_parameters: decode_counted(reader, decode_scalar_type)?,
                    structural_parameters: decode_counted(reader, |reader| {
                        Ok(TerminalDynamicStructuralFormal {
                            position: reader.u32()?,
                            structural_type: reader.id("StructuralTypeId")?,
                            access: decode_structural_access(reader)?,
                            multiplicity: match reader.u8()? {
                                1 => StructuralMultiplicity::Unrestricted,
                                2 => StructuralMultiplicity::Affine,
                                3 => StructuralMultiplicity::Linear,
                                tag => {
                                    return Err(CodecError::InvalidTag(
                                        "StructuralMultiplicity",
                                        tag,
                                    ));
                                }
                            },
                        })
                    })?,
                    result: match reader.u8()? {
                        0 => None,
                        1 => Some(decode_scalar_type(reader)?),
                        tag => {
                            return Err(CodecError::InvalidTag("TerminalDynamicSlot result", tag));
                        }
                    },
                })
            })?,
        })
    })?;
    let tables = decode_counted(reader, |reader| {
        Ok(TerminalDynamicTable {
            id: reader.u32()?,
            interface: reader.u32()?,
            implementation_identity: reader.string("dynamic table implementation")?,
            rows: decode_counted(reader, |reader| match reader.u8()? {
                0 => Ok(None),
                1 => Ok(Some(reader.id("MachineId")?)),
                tag => Err(CodecError::InvalidTag("TerminalDynamicTable row", tag)),
            })?,
        })
    })?;
    Ok(TerminalDynamicDispatchCatalog { interfaces, tables })
}
