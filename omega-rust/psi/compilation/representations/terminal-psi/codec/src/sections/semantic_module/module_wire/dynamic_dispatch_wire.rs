//! Dynamic-dispatch catalog on the wire: selectable interfaces with their
//! slot call shapes, and one row table per selected named implementation.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::structural_signature_wire::{
    decode_call_result, decode_structural_access, encode_call_result, encode_structural_access,
};
use super::super::wire::{Reader, Writer, decode_counted};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use terminal_psi::{
    StructuralMultiplicity, TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog,
    TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicStructuralFormal,
    TerminalDynamicTable, TerminalIdentityIndex,
};

pub(super) fn encode_dynamic_dispatch(
    writer: &mut Writer,
    catalog: &TerminalDynamicDispatchCatalog,
) -> Result<(), CodecError> {
    writer.len("dynamic interfaces", catalog.interfaces.len())?;
    for interface in &catalog.interfaces {
        writer.u32(interface.id);
        writer.u32(interface.identity.0);
        writer.len("dynamic interface slots", interface.slots.len())?;
        for slot in &interface.slots {
            writer.u32(slot.slot);
            writer.u32(slot.declaring_trait_identity.0);
            writer.u32(slot.requirement_identity.0);
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
            writer.len(
                "dynamic slot entry claim paths",
                slot.entry_claim_paths.len(),
            )?;
            for claim in &slot.entry_claim_paths {
                writer.u32(claim.argument_index);
                encode_structural_path(writer, "dynamic slot claim path", &claim.path)?;
            }
            encode_call_result(writer, &slot.result)?;
            writer.len("dynamic result claim paths", slot.result_claim_paths.len())?;
            for path in &slot.result_claim_paths {
                encode_structural_path(writer, "dynamic result claim path", path)?;
            }
            writer.len(
                "dynamic result reference regions",
                slot.result_reference_regions.len(),
            )?;
            for region in &slot.result_reference_regions {
                encode_structural_path(writer, "dynamic result reference path", &region.path)?;
                match region.origin {
                    terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } => {
                        writer.u8(1);
                        writer.u32(argument_index);
                    }
                    terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                        writer.u8(2);
                        writer.id(carrier_type);
                    }
                }
                encode_structural_path(writer, "dynamic result input region", &region.source_path)?;
                encode_structural_access(writer, region.access);
            }
        }
    }
    writer.len("dynamic tables", catalog.tables.len())?;
    for table in &catalog.tables {
        writer.u32(table.id);
        writer.u32(table.interface);
        writer.u32(table.implementation_identity.0);
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
            identity: TerminalIdentityIndex(reader.u32()?),
            slots: decode_counted(reader, |reader| {
                Ok(TerminalDynamicSlot {
                    slot: reader.u32()?,
                    declaring_trait_identity: TerminalIdentityIndex(reader.u32()?),
                    requirement_identity: TerminalIdentityIndex(reader.u32()?),
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
                    entry_claim_paths: decode_counted(reader, |reader| {
                        Ok(TerminalDynamicClaimPath {
                            argument_index: reader.u32()?,
                            path: decode_structural_path(reader)?,
                        })
                    })?,
                    result: decode_call_result(reader)?,
                    result_claim_paths: decode_counted(reader, decode_structural_path)?,
                    result_reference_regions: decode_counted(reader, |reader| {
                        Ok(terminal_psi::TerminalDynamicReferenceRegion {
                            path: decode_structural_path(reader)?,
                            origin: match reader.u8()? {
                                1 => terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                                    argument_index: reader.u32()?,
                                },
                                2 => terminal_psi::TerminalDynamicReferenceOrigin::Invocation {
                                    carrier_type: reader.id("StructuralTypeId")?,
                                },
                                tag => {
                                    return Err(CodecError::InvalidTag(
                                        "TerminalDynamicReferenceOrigin",
                                        tag,
                                    ));
                                }
                            },
                            source_path: decode_structural_path(reader)?,
                            access: decode_structural_access(reader)?,
                        })
                    })?,
                })
            })?,
        })
    })?;
    let tables = decode_counted(reader, |reader| {
        Ok(TerminalDynamicTable {
            id: reader.u32()?,
            interface: reader.u32()?,
            implementation_identity: TerminalIdentityIndex(reader.u32()?),
            rows: decode_counted(reader, |reader| match reader.u8()? {
                0 => Ok(None),
                1 => Ok(Some(reader.id("MachineId")?)),
                tag => Err(CodecError::InvalidTag("TerminalDynamicTable row", tag)),
            })?,
        })
    })?;
    Ok(TerminalDynamicDispatchCatalog { interfaces, tables })
}
