//! Borrow boundaries, owner paths, places, accesses and restored calls
//! on the wire.

use crate::sections::semantic_module::wire_fields::{
    Access, Id, Index, List, U64, wire_enum, wire_record,
};
use terminal_psi::{
    TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment, TerminalBorrowPlace,
    TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember,
};

wire_enum! {
    Boundary: TerminalBorrowBoundarySource, "TerminalBorrowBoundarySource" {
        1 => Statement { statement_index: U64 },
        2 => Call { statement_index: U64, call_ordinal: U64, target_identity: Index },
    }
    OwnerSegment: TerminalBorrowOwnerSegment, "TerminalBorrowOwnerSegment" {
        1 => Field(identity: Index),
        2 => Case(identity: Index),
        3 => FixedIndex(index: U64),
        4 => DynamicIndex,
    }
    PlaceSegment: TerminalBorrowPlaceSegment, "TerminalBorrowPlaceSegment" {
        1 => Field(identity: Index),
        2 => Case(identity: Index),
        3 => FixedIndex(index: U64),
        4 => FixedRange { start: U64, end: U64 },
        5 => CapturedSelector(identity: Index),
    }
    RestorationClass: TerminalReborrowRestorationClass, "TerminalReborrowRestorationClass" {
        1 => ExclusiveReactivation,
        2 => SharedFreezeRestoration,
    }
}

wire_record! {
    Place: TerminalBorrowPlace { root_identity: Index, segments: List<PlaceSegment> }
    CohortMember: TerminalReborrowSharedCohortMember {
        child_owner_identity: Index,
        child_owner_path: List<OwnerSegment>,
        child_place: Place,
        child_access: Access,
        child_activation: Boundary,
        child_weakening: Boundary,
    }
    RestoredCallUse: TerminalReborrowRestoredCallUse {
        machine: Id,
        operation: Id,
        restoration_class: RestorationClass,
        call_boundary: Boundary,
        call_target_machine: Id,
        source_machine_identity: Index,
        source_state_identity: Index,
        direct_root_owner_identity: Index,
        direct_root_owner_path: List<OwnerSegment>,
        direct_root_place: Place,
        direct_root_activation: Boundary,
        direct_root_weakening: Boundary,
        direct_root_lifetime_identity: Index,
        child_owner_identity: Index,
        child_owner_path: List<OwnerSegment>,
        child_place: Place,
        projection_remainder: List<PlaceSegment>,
        child_access: Access,
        child_activation: Boundary,
        formation_boundary: Boundary,
        child_weakening: Boundary,
        shared_cohort: List<CohortMember>,
    }
}
