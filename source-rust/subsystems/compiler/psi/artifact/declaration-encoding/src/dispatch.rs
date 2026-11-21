//! Callable slots, implementation tables and reference regions retained at dispatch.
use crate::CodecError;
use crate::fields::Field;
use crate::wire::{Writer, decode_canonical};

use crate::fields::{
    Access, CallResult, Flag, I128, Id, Ids, Index, List, Multiplicity, OptionalId, Path, Scalar,
    U32, wire_enum, wire_record,
};
use application_identity::const_value::FiniteScalarValue;
use terminal_psi::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};

wire_record! {
    Catalog: TerminalDynamicDispatchCatalog {
        interfaces: List<Interface, 12>,
        tables: List<Table, 16>,
    }
    Interface: TerminalDynamicInterface { id: U32, identity: Index, slots: List<Slot, 37> }
    Slot: TerminalDynamicSlot {
        slot: U32,
        declaring_trait_identity: Index,
        requirement_identity: Index,
        family_tuple: List<FamilyScalar, 2>,
        scalar_parameters: List<Scalar>,
        structural_parameters: List<Formal, 18>,
        entry_claim_paths: List<ClaimPath, 8>,
        result: CallResult,
        result_claim_paths: List<Path, 4>,
        result_reference_regions: List<Region, 14>,
    }
    Formal: TerminalDynamicStructuralFormal {
        position: U32,
        structural_type: Id,
        access: Access,
        multiplicity: Multiplicity,
        qualifications: Ids,
    }
    ClaimPath: TerminalDynamicClaimPath { argument_index: U32, path: Path }
    Region: TerminalDynamicReferenceRegion {
        path: Path,
        origin: Origin,
        source_path: Path,
        access: Access,
    }
    Table: TerminalDynamicTable {
        id: U32,
        interface: U32,
        implementation_identity: Index,
        rows: List<OptionalId>,
    }
}

wire_enum! {
    FamilyScalar: FiniteScalarValue, "finite family scalar value" {
        1 => Integer(value: I128),
        2 => Boolean(value: Flag),
    }
    Origin: TerminalDynamicReferenceOrigin, "TerminalDynamicReferenceOrigin" {
        1 => Parameter { argument_index: U32 },
        2 => Invocation { carrier_type: Id },
    }
}

/// Encode the catalog with the same fields used inside a semantic module.
pub fn encode_dynamic_dispatch_catalog(
    catalog: &TerminalDynamicDispatchCatalog,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    Catalog::write(&mut writer, catalog)?;
    Ok(writer.finish())
}

pub fn decode_dynamic_dispatch_catalog(
    bytes: &[u8],
) -> Result<TerminalDynamicDispatchCatalog, CodecError> {
    decode_canonical(bytes, Catalog::read, encode_dynamic_dispatch_catalog)
}

pub fn encode_reference_regions(
    writer: &mut crate::wire::Writer,
    value: &Vec<TerminalDynamicReferenceRegion>,
) -> Result<(), crate::CodecError> {
    <List<Region> as crate::fields::Field<_>>::write(writer, value)
}

pub fn decode_reference_regions(
    reader: &mut crate::wire::Reader<'_>,
) -> Result<Vec<TerminalDynamicReferenceRegion>, crate::CodecError> {
    <List<Region> as crate::fields::Field<_>>::read(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::Field;
    use crate::wire::{Reader, Writer};

    #[test]
    fn standalone_dispatch_catalog_uses_the_module_fields_exactly() {
        let catalog = TerminalDynamicDispatchCatalog {
            interfaces: vec![TerminalDynamicInterface {
                id: 0,
                identity: terminal_psi::TerminalIdentityIndex(2),
                slots: Vec::new(),
            }],
            tables: vec![TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: terminal_psi::TerminalIdentityIndex(3),
                rows: Vec::new(),
            }],
        };
        let mut writer = Writer::default();
        Catalog::write(&mut writer, &catalog).unwrap();
        let bytes = writer.finish();
        assert_eq!(encode_dynamic_dispatch_catalog(&catalog).unwrap(), bytes);
        assert_eq!(decode_dynamic_dispatch_catalog(&bytes).unwrap(), catalog);
        for prefix in 0..bytes.len() {
            assert!(decode_dynamic_dispatch_catalog(&bytes[..prefix]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            decode_dynamic_dispatch_catalog(&trailing),
            Err(CodecError::TrailingBytes(1))
        ));
    }

    #[test]
    fn standalone_dispatch_catalog_refuses_a_symbolic_scalar_width() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, MachineId, ScalarType, ValueId, WidthParameter,
        };
        let catalog = TerminalDynamicDispatchCatalog {
            interfaces: vec![TerminalDynamicInterface {
                id: 0,
                identity: terminal_psi::TerminalIdentityIndex(2),
                slots: vec![TerminalDynamicSlot {
                    slot: 0,
                    declaring_trait_identity: terminal_psi::TerminalIdentityIndex(3),
                    requirement_identity: terminal_psi::TerminalIdentityIndex(4),
                    family_tuple: Vec::new(),
                    scalar_parameters: vec![ScalarType::Integer(IntegerType::parameterized(
                        IntegerSign::Unsigned,
                        WidthParameter {
                            owner: MachineId::new(1).unwrap(),
                            value: ValueId::new(2).unwrap(),
                        },
                    ))],
                    structural_parameters: Vec::new(),
                    entry_claim_paths: Vec::new(),
                    result: terminal_psi::BoundaryMachineResult::Unit,
                    result_claim_paths: Vec::new(),
                    result_reference_regions: Vec::new(),
                }],
            }],
            tables: Vec::new(),
        };
        assert!(matches!(
            encode_dynamic_dispatch_catalog(&catalog),
            Err(CodecError::IntegerWidthClosure(_))
        ));
    }

    #[test]
    fn finite_family_scalar_wire_preserves_values_and_rejects_bad_tags() {
        let values = vec![
            FiniteScalarValue::Integer(i128::MIN),
            FiniteScalarValue::Integer(i128::MAX),
            FiniteScalarValue::Boolean(false),
            FiniteScalarValue::Boolean(true),
        ];
        let mut writer = Writer::default();
        <List<FamilyScalar> as Field<_>>::write(&mut writer, &values).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            <List<FamilyScalar> as Field<_>>::read(&mut reader).unwrap(),
            values
        );
        assert_eq!(reader.remaining(), 0);
        for bytes in [&[3][..], &[2, 2][..], &[1, 0][..]] {
            assert!(FamilyScalar::read(&mut Reader::new(bytes)).is_err());
        }
    }
}
