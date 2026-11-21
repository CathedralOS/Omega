//! Callable evidence lanes and their parameter or result subjects.
use crate::fields::{Id, List, Optional, Text, U32, wire_enum, wire_record};
use crate::proof_declarations::Interface as EvidenceInterface;
use crate::signatures::ArgumentSubstitution;
use terminal_psi::{
    EvidenceArgumentSubject, EvidenceCallableOwner, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceTermDeclaration,
};

wire_record! {
    Term: EvidenceTermDeclaration { id: Id, proposition: Id, interface: EvidenceInterface }
    Lane: EvidenceContractLane {
        owner: CallableOwner,
        kind: LaneKind,
        position: U32,
        term: Id,
        output_field: Optional<Text>,
        argument_subjects: List<Subject>,
    }
}

wire_enum! {
    CallableOwner: EvidenceCallableOwner, "EvidenceCallableOwner" {
        1 => Machine(machine: Id),
        2 => Runtime(contract: Id),
    }
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
