use crate::CodecError;
use crate::fields::{Field, Id, Index, List, Text, U32, wire_enum, wire_record};
use crate::wire::{Writer, decode_canonical};
use terminal_psi::quotients::QuotientPositionalRelation;
use terminal_psi::quotients::QuotientRelationIdentity;
use terminal_psi::types::quotients::{
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
    TerminalQuotientOperation, TerminalQuotientRepresentation,
};

wire_record! {
    Relation: QuotientRelationIdentity {
        quotient_declaration: Text,
        quotient_type: Text,
        carrier_type: Text,
        relation: Text,
    }
    Catalog: TerminalQuotientCatalog {
        representations: List<Representation>,
        operations: List<Operation>,
        adaptations: List<Adaptation>,
    }
    Representation: TerminalQuotientRepresentation {
        quotient: Id,
        carrier: Id,
        quotient_storage_identity: Index,
        carrier_storage_identity: Index,
        formation: Relation,
    }
    Operation: TerminalQuotientOperation {
        public_machine: Id,
        representative_machine: Id,
        call: Id,
        input_relations: List<Positional>,
        result_relation: Positional,
    }
    Adaptation: TerminalQuotientAdaptation {
        machine: Id,
        copy: Id,
        source: Id,
        result: Id,
        source_type: Id,
        result_type: Id,
        representation: U32,
        license: License,
    }
}

wire_enum! {
    Positional: QuotientPositionalRelation, "QuotientPositionalRelation" {
        1 => Quotient(identity: Relation),
        2 => ExactEquality { public_type: Text, representative_type: Text },
    }
    License: TerminalQuotientAdaptationLicense, "TerminalQuotientAdaptationLicense" {
        1 => Mint,
        2 => Input { operation: U32, position: U32 },
        3 => Result { operation: U32 },
    }
}

/// Keeps representations, operations and adaptations in their declared order.
pub fn encode_quotient_catalog(catalog: &TerminalQuotientCatalog) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    Catalog::write(&mut writer, catalog)?;
    Ok(writer.finish())
}
pub fn decode_quotient_catalog(bytes: &[u8]) -> Result<TerminalQuotientCatalog, CodecError> {
    decode_canonical(bytes, Catalog::read, encode_quotient_catalog)
}
