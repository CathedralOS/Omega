//! Propositions, binders and evidence applications named by a callable.
use terminal_psi::{
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ScalarPredicateDefinition, TerminalIdentityIndex,
};

use super::CodecError;
use super::wire::{Reader, Writer};
use crate::fields::{Field, Id, Index, List, Optional, Proposition, Text, wire_enum, wire_record};

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

// An absent declaration index introduces a telescope with erased types.
// Scalar-only definitions retain their original bytes and fingerprints.
pub struct ScalarPredicate;
impl Field<ScalarPredicateDefinition> for ScalarPredicate {
    fn write(writer: &mut Writer, value: &ScalarPredicateDefinition) -> Result<(), CodecError> {
        use terminal_psi::MathematicalPredicateBody;
        if !value.type_parameters.is_empty() {
            Index::write(writer, &TerminalIdentityIndex::ABSENT)?;
            List::<crate::fields::U32>::write(writer, &value.type_parameters)?;
        }
        Index::write(writer, &value.declaration)?;
        List::<crate::signatures::Declaration>::write(writer, &value.parameters)?;
        if !value.type_parameters.is_empty() {
            writer.boolean(matches!(
                value.body,
                MathematicalPredicateBody::TypeParameter(_)
            ));
        }
        match &value.body {
            MathematicalPredicateBody::Proposition(body) => Proposition::write(writer, body),
            MathematicalPredicateBody::TypeParameter(slot) if !value.type_parameters.is_empty() => {
                writer.u32(*slot);
                Ok(())
            }
            _ => Err(CodecError::MalformedStructuralFoundation(
                "type projection without a formal",
            )),
        }
    }
    fn read(reader: &mut Reader<'_>) -> Result<ScalarPredicateDefinition, CodecError> {
        use terminal_psi::MathematicalPredicateBody;
        let mut declaration = Index::read(reader)?;
        let mut type_parameters = Vec::new();
        if declaration.is_absent() {
            type_parameters = List::<crate::fields::U32>::read(reader)?;
            if type_parameters.is_empty() {
                return Err(CodecError::MalformedStructuralFoundation(
                    "empty erased type telescope extension",
                ));
            }
            declaration = Index::read(reader)?;
        }
        let parameters = List::<crate::signatures::Declaration>::read(reader)?;
        let body = if !type_parameters.is_empty() && reader.boolean()? {
            MathematicalPredicateBody::TypeParameter(reader.u32()?)
        } else {
            MathematicalPredicateBody::Proposition(Proposition::read(reader)?)
        };
        Ok(ScalarPredicateDefinition {
            declaration,
            parameters,
            type_parameters,
            body,
        })
    }
}

pub struct BinderArgument;
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
