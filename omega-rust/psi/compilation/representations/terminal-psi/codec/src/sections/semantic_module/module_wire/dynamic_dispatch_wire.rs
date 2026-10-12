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
    TerminalDynamicFamilyMember, TerminalDynamicFamilyValue, TerminalDynamicInterface,
    TerminalDynamicSlot, TerminalDynamicStructuralFormal, TerminalDynamicTable,
    TerminalIdentityIndex,
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
            writer.len("dynamic slot family tuple", slot.family_tuple.len())?;
            for member in &slot.family_tuple {
                writer.string("dynamic family member identity", &member.identity)?;
                match member.value {
                    TerminalDynamicFamilyValue::Integer(value) => {
                        writer.u8(0);
                        writer.bytes(&value.to_le_bytes());
                    }
                    TerminalDynamicFamilyValue::Boolean(value) => {
                        writer.u8(1);
                        writer.boolean(value);
                    }
                }
            }
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
                    family_tuple: decode_counted(reader, |reader| {
                        let identity = reader.string("dynamic family member identity")?;
                        let value = match reader.u8()? {
                            0 => TerminalDynamicFamilyValue::Integer(i128::from_le_bytes(
                                reader.array()?,
                            )),
                            1 => TerminalDynamicFamilyValue::Boolean(reader.boolean()?),
                            tag => {
                                return Err(CodecError::InvalidTag(
                                    "dynamic family member value",
                                    tag,
                                ));
                            }
                        };
                        Ok(TerminalDynamicFamilyMember { identity, value })
                    })?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_wire_retains_values_independently_of_identity_spelling() {
        let catalog = TerminalDynamicDispatchCatalog {
            interfaces: vec![TerminalDynamicInterface {
                id: 0,
                identity: TerminalIdentityIndex(0),
                slots: vec![TerminalDynamicSlot {
                    slot: 0,
                    declaring_trait_identity: TerminalIdentityIndex(1),
                    requirement_identity: TerminalIdentityIndex(2),
                    family_tuple: vec![
                        TerminalDynamicFamilyMember {
                            identity: "opaque signed member".to_owned(),
                            value: TerminalDynamicFamilyValue::Integer(i128::MIN),
                        },
                        TerminalDynamicFamilyMember {
                            identity: "opaque boolean member".to_owned(),
                            value: TerminalDynamicFamilyValue::Boolean(true),
                        },
                    ],
                    scalar_parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claim_paths: Vec::new(),
                    result: terminal_psi::BoundaryMachineResult::Unit,
                    result_claim_paths: Vec::new(),
                }],
            }],
            tables: Vec::new(),
        };
        let mut writer = Writer::default();
        encode_dynamic_dispatch(&mut writer, &catalog).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_dynamic_dispatch(&mut reader).unwrap(), catalog);
        assert_eq!(reader.remaining(), 0);
        // Every truncation of a published row must fail decoding.
        for length in 0..bytes.len() {
            assert!(decode_dynamic_dispatch(&mut Reader::new(&bytes[..length])).is_err());
        }
    }
}
