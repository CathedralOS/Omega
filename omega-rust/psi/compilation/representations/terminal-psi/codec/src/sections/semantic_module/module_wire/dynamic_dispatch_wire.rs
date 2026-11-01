//! Dynamic-dispatch catalog on the wire: selectable interfaces with their
//! slot call shapes, and one row table per selected named implementation.

use super::super::wire_fields::{
    Access, CallResult, Id, Ids, Index, List, Multiplicity, OptionalId, Path, Scalar, Strings, U32,
    wire_enum, wire_record,
};
use terminal_psi::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};

wire_record! {
    Catalog: TerminalDynamicDispatchCatalog {
        interfaces: List<Interface>,
        tables: List<Table>,
    }
    Interface: TerminalDynamicInterface { id: U32, identity: Index, slots: List<Slot> }
    Slot: TerminalDynamicSlot {
        slot: U32,
        declaring_trait_identity: Index,
        requirement_identity: Index,
        family_tuple: Strings,
        scalar_parameters: List<Scalar>,
        structural_parameters: List<Formal>,
        entry_claim_paths: List<ClaimPath>,
        result: CallResult,
        result_claim_paths: List<Path>,
        result_reference_regions: List<Region>,
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
    Origin: TerminalDynamicReferenceOrigin, "TerminalDynamicReferenceOrigin" {
        1 => Parameter { argument_index: U32 },
        2 => Invocation { carrier_type: Id },
    }
}
