//! Canonical proof declaration and evidence-interface wire format.
//!
//! This module owns proposition binder declarations, source-handle-free
//! application identities, optional evidence projections, and evidence
//! interface requirements. Recursive propositions and proof admission remain
//! outside the codec layer.

use terminal_psi::{
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, TerminalIdentityIndex,
};

use super::CodecError;
use super::wire::{Reader, Writer};
use super::wire_fields::{Field, Id, Index, List, Optional, Text, wire_enum, wire_record};

wire_record! {
    Declaration: PropositionDeclaration {
        id: Id,
        name: Text,
        binders: List<Binder>,
        parameter_types: List<Text>,
        evidence: Evidence,
    }
    Binder: PropositionBinderDeclaration { name: Text, kind: BinderKind }
    Application: PropositionApplicationIdentity {
        id: Id,
        declaration: Id,
        binder_arguments: List<BinderArgument>,
        arguments: List<Text>,
        evidence_interface: Optional<Interface>,
    }
    Projection: EvidenceProjectionIdentity {
        term: Id,
        declaring_trait_identity: Index,
        declaring_trait_arguments: List<Text>,
        requirement_identity: Index,
    }
    Interface: EvidenceInterfaceIdentity {
        trait_identity: Index,
        arguments: List<Text>,
        requirements: List<Requirement>,
    }
    Requirement: EvidenceRequirementIdentity {
        declaring_trait_identity: Index,
        declaring_trait_arguments: List<Text>,
        requirement_identity: Index,
    }
}

wire_enum! {
    BinderKind: PropositionBinderKind, "PropositionBinderKind" {
        1 => Type,
        2 => Const { type_identity: Index },
        3 => Machine,
    }
    Evidence: PropositionEvidence, "PropositionEvidence" {
        1 => FactOnly,
        2 => Witness { evidence_type: Text },
    }
    ArgumentKind: PropositionBinderArgumentKind, "PropositionBinderArgumentKind" {
        1 => Type,
        2 => Const,
        3 => Machine,
    }
}

/// A binder argument names either its identity or, through an evidence
/// projection, the requirement that supplies it; never both.
struct BinderArgument;
impl Field<PropositionBinderArgumentIdentity> for BinderArgument {
    fn write(
        writer: &mut Writer,
        value: &PropositionBinderArgumentIdentity,
    ) -> Result<(), CodecError> {
        ArgumentKind::write(writer, &value.kind)?;
        match &value.evidence_projection {
            None => {
                writer.u8(0);
                writer.u32(value.identity.0);
                Ok(())
            }
            Some(projection) => {
                writer.u8(1);
                Projection::write(writer, projection)
            }
        }
    }
    fn read(reader: &mut Reader<'_>) -> Result<PropositionBinderArgumentIdentity, CodecError> {
        let kind = ArgumentKind::read(reader)?;
        let (identity, evidence_projection) = match reader.u8()? {
            0 => (TerminalIdentityIndex(reader.u32()?), None),
            1 => (
                TerminalIdentityIndex::ABSENT,
                Some(Projection::read(reader)?),
            ),
            tag => return Err(CodecError::InvalidTag("PropositionBinderArgument", tag)),
        };
        Ok(PropositionBinderArgumentIdentity {
            kind,
            identity,
            evidence_projection,
        })
    }
}
