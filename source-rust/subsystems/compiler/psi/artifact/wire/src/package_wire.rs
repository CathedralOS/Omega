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
