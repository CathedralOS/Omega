//! Schema and per-operation evidence for a value with hidden type parameters.

use crate::wire_fields::{Argument, Id, Ids, List, U32, wire_enum, wire_record};
use terminal_psi::{
    TerminalPackageCatalog, TerminalPackageEvent, TerminalPackageOpening, TerminalPackageSchema,
    TerminalPackageSite,
};

wire_record! {
    Catalog: TerminalPackageCatalog {
        schemas: List<Schema>,
        openings: List<Opening>,
        sites: List<Site>,
    }
    Schema: TerminalPackageSchema { record: Id, witnesses: Ids }
    Opening: TerminalPackageOpening { machine: Id, record: Id, subject: Argument, witnesses: Ids }
    Site: TerminalPackageSite { machine: Id, operation: Id, event: Event }
}

wire_enum! {
    Event: TerminalPackageEvent, "TerminalPackageEvent" {
        1 => Construct { witnesses: Ids },
        2 => Project { opening: U32, field: Id },
        3 => Invoke { opening: U32 },
        4 => Store { opening: U32, field: Id },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire_fields::Field;
    use crate::wire_fields::wire::{Reader, Writer};
    use semantic_vocabulary::{
        MachineId, OperationId, PlaceId, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{StructuralAccess, StructuralArgument};

    #[test]
    fn saved_program_must_own_each_package_operation() {
        use crate::semantic_module::entry_tests::{identity_table, unit_machine};

        let machine = MachineId::new(1).unwrap();
        let mut module = terminal_psi::TerminalModule::for_entry(machine);
        module.identities = identity_table(4);
        module.machines.push(unit_machine(machine));
        let original = crate::encode_module(&module).unwrap();
        assert_eq!(crate::decode_module(&original).unwrap(), module);

        module.execution.packages.sites.push(TerminalPackageSite {
            machine,
            operation: OperationId::new(99).unwrap(),
            event: TerminalPackageEvent::Construct { witnesses: vec![] },
        });
        let bytes = crate::module_wire::encode_raw(&module).unwrap();
        assert_ne!(original, bytes);
        assert!(matches!(
            crate::decode_module(&bytes),
            Err(crate::CodecError::MalformedStructuralFoundation(
                "package site names an absent operation"
            ))
        ));
    }

    #[test]
    fn package_catalog_preserves_witness_order_subject_and_operation_identity() {
        let ty = |n| StructuralTypeId::new(n).unwrap();
        let machine = MachineId::new(2).unwrap();
        let site = |n, event| TerminalPackageSite {
            machine,
            operation: OperationId::new(n).unwrap(),
            event,
        };
        let mut subject =
            StructuralArgument::whole(PlaceId::new(3).unwrap(), StructuralAccess::SharedBorrow);
        subject
            .path
            .push(terminal_psi::StructuralPathSegment::FixedIndex(1));
        let catalog = TerminalPackageCatalog {
            schemas: vec![TerminalPackageSchema {
                record: ty(1),
                witnesses: vec![ty(9), ty(8)],
            }],
            openings: vec![TerminalPackageOpening {
                machine,
                record: ty(1),
                subject,
                witnesses: vec![ty(7), ty(6)],
            }],
            sites: vec![
                site(
                    1,
                    TerminalPackageEvent::Construct {
                        witnesses: vec![ty(5), ty(4)],
                    },
                ),
                site(
                    2,
                    TerminalPackageEvent::Project {
                        opening: 0,
                        field: StructuralFieldId::new(3).unwrap(),
                    },
                ),
                site(3, TerminalPackageEvent::Invoke { opening: 0 }),
                site(
                    4,
                    TerminalPackageEvent::Store {
                        opening: 0,
                        field: StructuralFieldId::new(1).unwrap(),
                    },
                ),
            ],
        };
        let mut writer = Writer::default();
        Catalog::write(&mut writer, &catalog).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(Catalog::read(&mut reader).unwrap(), catalog);
        assert_eq!(reader.remaining(), 0);
        assert!(Catalog::read(&mut Reader::new(&bytes[..bytes.len() - 1])).is_err());
    }
}
