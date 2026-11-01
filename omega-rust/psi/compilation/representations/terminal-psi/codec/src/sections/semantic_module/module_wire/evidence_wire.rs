//! Evidence terms and evidence contract lanes on the wire.

use super::super::wire_fields::{
    ArgumentSubstitution, EvidenceInterface, Id, List, Optional, Text, U32, wire_enum, wire_record,
};
use terminal_psi::{
    EvidenceArgumentSubject, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceTermDeclaration,
};

wire_record! {
    Term: EvidenceTermDeclaration { id: Id, proposition: Id, interface: EvidenceInterface }
    Lane: EvidenceContractLane {
        machine: Id,
        kind: LaneKind,
        position: U32,
        term: Id,
        output_field: Optional<Text>,
        argument_subjects: List<Subject>,
    }
}

wire_enum! {
    LaneKind: EvidenceContractLaneKind, "EvidenceContractLaneKind" {
        1 => Requires,
        2 => Ensures,
    }
    Subject: EvidenceArgumentSubject, "EvidenceArgumentSubject" {
        1 => Parameter(parameter: ArgumentSubstitution),
        2 => Result { argument_position: U32 },
        3 => Closed { argument_position: U32 },
        4 => Unknown { argument_position: U32 },
    }
}
