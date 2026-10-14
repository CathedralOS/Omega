//! Canonical quotient catalog encoding. Field order and variant tags are explicit.
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_counted},
};
use language_semantics::quotient_correspondence::*;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use terminal_psi::terminal_module::quotients::*;

trait Wire: Sized {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError>;
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError>;
}
impl Wire for String {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.string("quotient identity", self)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.string("quotient identity")
    }
}
impl Wire for u32 {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.u32(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.u32()
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.len("quotient rows", self.len())?;
        for x in self {
            x.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        decode_counted(r, T::read)
    }
}

impl Wire for MachineId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient MachineId")
    }
}

impl Wire for OperationId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient OperationId")
    }
}

impl Wire for PlaceId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient PlaceId")
    }
}

impl Wire for StructuralTypeId {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        w.id(*self);
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        r.id("quotient StructuralTypeId")
    }
}

impl Wire for QuotientCallableIdentity {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.declaration.write(w)?;
        self.overload.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            declaration: Wire::read(r)?,
            overload: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientCorrespondenceOperationKind {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Lift => {
                w.u8(1);
            }
            Self::LiftWithForwardPreconditionTransport => {
                w.u8(2);
            }
            Self::Define => {
                w.u8(3);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Lift),
            2 => Ok(Self::LiftWithForwardPreconditionTransport),
            3 => Ok(Self::Define),
            tag => Err(CodecError::InvalidTag(
                "QuotientCorrespondenceOperationKind",
                tag,
            )),
        }
    }
}

impl Wire for QuotientStaticApplication {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.bindings.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            bindings: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientMachineApplication {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.callable.write(w)?;
        self.static_application.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            callable: Wire::read(r)?,
            static_application: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientRelationIdentity {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.quotient_declaration.write(w)?;
        self.quotient_type.write(w)?;
        self.carrier_type.write(w)?;
        self.relation.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            quotient_declaration: Wire::read(r)?,
            quotient_type: Wire::read(r)?,
            carrier_type: Wire::read(r)?,
            relation: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientPositionalRelation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Quotient(value) => {
                w.u8(1);
                value.write(w)?;
            }
            Self::ExactEquality {
                public_type,
                representative_type,
            } => {
                w.u8(2);
                public_type.write(w)?;
                representative_type.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Quotient(Wire::read(r)?)),
            2 => Ok(Self::ExactEquality {
                public_type: Wire::read(r)?,
                representative_type: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag("QuotientPositionalRelation", tag)),
        }
    }
}

impl Wire for QuotientDefineRuntimePosition {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.public_position.write(w)?;
        self.representative_position.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            public_position: Wire::read(r)?,
            representative_position: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientContractOwner {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Machine => {
                w.u8(1);
            }
            Self::State => {
                w.u8(2);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Machine),
            2 => Ok(Self::State),
            tag => Err(CodecError::InvalidTag("QuotientContractOwner", tag)),
        }
    }
}

impl Wire for QuotientContractFactCoordinate {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.owner.write(w)?;
        self.contract_position.write(w)?;
        self.fact_position.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            owner: Wire::read(r)?,
            contract_position: Wire::read(r)?,
            fact_position: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremApplicationSide {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Left => {
                w.u8(1);
            }
            Self::Right => {
                w.u8(2);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Left),
            2 => Ok(Self::Right),
            tag => Err(CodecError::InvalidTag(
                "QuotientTheoremApplicationSide",
                tag,
            )),
        }
    }
}

impl Wire for QuotientForwardPreconditionTransportFact {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.application.write(w)?;
        self.source.write(w)?;
        self.actual.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            application: Wire::read(r)?,
            source: Wire::read(r)?,
            actual: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremParameterRole {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::QuotientLeft { input_position } => {
                w.u8(1);
                input_position.write(w)?;
            }
            Self::QuotientRight { input_position } => {
                w.u8(2);
                input_position.write(w)?;
            }
            Self::Shared { input_position } => {
                w.u8(3);
                input_position.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::QuotientLeft {
                input_position: Wire::read(r)?,
            }),
            2 => Ok(Self::QuotientRight {
                input_position: Wire::read(r)?,
            }),
            3 => Ok(Self::Shared {
                input_position: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag("QuotientTheoremParameterRole", tag)),
        }
    }
}

impl Wire for QuotientTheoremParameter {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.theorem_position.write(w)?;
        self.role.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            theorem_position: Wire::read(r)?,
            role: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremRelationPremise {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.expected_position.write(w)?;
        self.actual.write(w)?;
        self.relation.write(w)?;
        self.left_parameter.write(w)?;
        self.right_parameter.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            expected_position: Wire::read(r)?,
            actual: Wire::read(r)?,
            relation: Wire::read(r)?,
            left_parameter: Wire::read(r)?,
            right_parameter: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientRepresentativeApplication {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.arguments.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            arguments: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientResultRelation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Quotient(value) => {
                w.u8(1);
                value.write(w)?;
            }
            Self::ExactEquality(value) => {
                w.u8(2);
                value.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Quotient(Wire::read(r)?)),
            2 => Ok(Self::ExactEquality(Wire::read(r)?)),
            tag => Err(CodecError::InvalidTag("QuotientResultRelation", tag)),
        }
    }
}

impl Wire for QuotientTheoremConclusion {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.actual.write(w)?;
        self.relation.write(w)?;
        self.left.write(w)?;
        self.right.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            actual: Wire::read(r)?,
            relation: Wire::read(r)?,
            left: Wire::read(r)?,
            right: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientCongruenceCorrespondence {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.parameters.write(w)?;
        self.relation_premises.write(w)?;
        self.legality_premises.write(w)?;
        self.conclusion.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            parameters: Wire::read(r)?,
            relation_premises: Wire::read(r)?,
            legality_premises: Wire::read(r)?,
            conclusion: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremRole {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Congruence => {
                w.u8(1);
            }
            Self::ForwardPreconditionTransport => {
                w.u8(2);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Congruence),
            2 => Ok(Self::ForwardPreconditionTransport),
            tag => Err(CodecError::InvalidTag("QuotientTheoremRole", tag)),
        }
    }
}

impl Wire for QuotientForwardPreconditionTransportCorrespondence {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.public_premises.write(w)?;
        self.representative_conclusions.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            public_premises: Wire::read(r)?,
            representative_conclusions: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremCorrespondence {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Congruence(value) => {
                w.u8(1);
                value.write(w)?;
            }
            Self::ForwardPreconditionTransport(value) => {
                w.u8(2);
                value.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Congruence(Wire::read(r)?)),
            2 => Ok(Self::ForwardPreconditionTransport(Wire::read(r)?)),
            tag => Err(CodecError::InvalidTag("QuotientTheoremCorrespondence", tag)),
        }
    }
}

impl Wire for QuotientPurityCertificate {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::PureClosure => {
                w.u8(1);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::PureClosure),
            tag => Err(CodecError::InvalidTag("QuotientPurityCertificate", tag)),
        }
    }
}

impl Wire for QuotientTerminationCertificate {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Unconditional => {
                w.u8(1);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Unconditional),
            tag => Err(CodecError::InvalidTag(
                "QuotientTerminationCertificate",
                tag,
            )),
        }
    }
}

impl Wire for QuotientCrashCertificate {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::CrashFree => {
                w.u8(1);
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::CrashFree),
            tag => Err(CodecError::InvalidTag("QuotientCrashCertificate", tag)),
        }
    }
}

impl Wire for QuotientRepresentativeEligibility {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.purity.write(w)?;
        self.termination.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            purity: Wire::read(r)?,
            termination: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremEligibility {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.purity.write(w)?;
        self.termination.write(w)?;
        self.crash.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            purity: Wire::read(r)?,
            termination: Wire::read(r)?,
            crash: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientTheoremEvidence {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.role.write(w)?;
        self.selected_application.write(w)?;
        self.correspondence.write(w)?;
        self.eligibility.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            role: Wire::read(r)?,
            selected_application: Wire::read(r)?,
            correspondence: Wire::read(r)?,
            eligibility: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientStateForwarding {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.source_position.write(w)?;
        self.target_position.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            source_position: Wire::read(r)?,
            target_position: Wire::read(r)?,
        })
    }
}

impl Wire for QuotientResultFlow {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Direct {
                statement_position,
                immutable_alias_count,
            } => {
                w.u8(1);
                statement_position.write(w)?;
                immutable_alias_count.write(w)?;
            }
            Self::Forwarded {
                machine_state_count,
                result_state_position,
                statement_position,
                immutable_alias_count,
                forwarding,
            } => {
                w.u8(2);
                machine_state_count.write(w)?;
                result_state_position.write(w)?;
                statement_position.write(w)?;
                immutable_alias_count.write(w)?;
                forwarding.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Direct {
                statement_position: Wire::read(r)?,
                immutable_alias_count: Wire::read(r)?,
            }),
            2 => Ok(Self::Forwarded {
                machine_state_count: Wire::read(r)?,
                result_state_position: Wire::read(r)?,
                statement_position: Wire::read(r)?,
                immutable_alias_count: Wire::read(r)?,
                forwarding: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag("QuotientResultFlow", tag)),
        }
    }
}

impl Wire for CanonicalQuotientCorrespondence {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.operation_kind.write(w)?;
        self.public_operation.write(w)?;
        self.representative.write(w)?;
        self.input_relations.write(w)?;
        self.result_relation.write(w)?;
        self.runtime_positions.write(w)?;
        self.theorem_evidence.write(w)?;
        self.representative_eligibility.write(w)?;
        self.result_flow.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            operation_kind: Wire::read(r)?,
            public_operation: Wire::read(r)?,
            representative: Wire::read(r)?,
            input_relations: Wire::read(r)?,
            result_relation: Wire::read(r)?,
            runtime_positions: Wire::read(r)?,
            theorem_evidence: Wire::read(r)?,
            representative_eligibility: Wire::read(r)?,
            result_flow: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientCatalog {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.representations.write(w)?;
        self.operations.write(w)?;
        self.adaptations.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            representations: Wire::read(r)?,
            operations: Wire::read(r)?,
            adaptations: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientRepresentation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.quotient.write(w)?;
        self.carrier.write(w)?;
        self.quotient_storage_identity.0.write(w)?;
        self.carrier_storage_identity.0.write(w)?;
        self.formation.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            quotient: Wire::read(r)?,
            carrier: Wire::read(r)?,
            quotient_storage_identity: terminal_psi::TerminalIdentityIndex(Wire::read(r)?),
            carrier_storage_identity: terminal_psi::TerminalIdentityIndex(Wire::read(r)?),
            formation: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientOperation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.public_machine.write(w)?;
        self.representative_machine.write(w)?;
        self.call.write(w)?;
        self.correspondence.write(w)?;
        self.theorems.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            public_machine: Wire::read(r)?,
            representative_machine: Wire::read(r)?,
            call: Wire::read(r)?,
            correspondence: Wire::read(r)?,
            theorems: Wire::read(r)?,
        })
    }
}

impl Wire for TerminalQuotientTheorem {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        super::super::machine_declaration_wire::encode(w, &self.declaration)?;
        match self.admission {
            TerminalQuotientTheoremAdmission::SourceCheckedBodyfulResultless => w.u8(1),
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        let declaration = super::super::machine_declaration_wire::decode(r)?;
        let admission = match r.u8()? {
            1 => TerminalQuotientTheoremAdmission::SourceCheckedBodyfulResultless,
            tag => return Err(CodecError::InvalidTag("quotient theorem admission", tag)),
        };
        Ok(Self {
            declaration,
            admission,
        })
    }
}

impl Wire for TerminalQuotientAdaptationLicense {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        match self {
            Self::Mint => {
                w.u8(1);
            }
            Self::Input {
                operation,
                position,
            } => {
                w.u8(2);
                operation.write(w)?;
                position.write(w)?;
            }
            Self::Result { operation } => {
                w.u8(3);
                operation.write(w)?;
            }
        };
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        match r.u8()? {
            1 => Ok(Self::Mint),
            2 => Ok(Self::Input {
                operation: Wire::read(r)?,
                position: Wire::read(r)?,
            }),
            3 => Ok(Self::Result {
                operation: Wire::read(r)?,
            }),
            tag => Err(CodecError::InvalidTag(
                "TerminalQuotientAdaptationLicense",
                tag,
            )),
        }
    }
}

impl Wire for TerminalQuotientAdaptation {
    fn write(&self, w: &mut Writer) -> Result<(), CodecError> {
        self.machine.write(w)?;
        self.copy.write(w)?;
        self.source.write(w)?;
        self.result.write(w)?;
        self.source_type.write(w)?;
        self.result_type.write(w)?;
        self.representation.write(w)?;
        self.license.write(w)?;
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            machine: Wire::read(r)?,
            copy: Wire::read(r)?,
            source: Wire::read(r)?,
            result: Wire::read(r)?,
            source_type: Wire::read(r)?,
            result_type: Wire::read(r)?,
            representation: Wire::read(r)?,
            license: Wire::read(r)?,
        })
    }
}

pub(super) fn encode(w: &mut Writer, catalog: &TerminalQuotientCatalog) -> Result<(), CodecError> {
    catalog.write(w)
}
pub(super) fn decode(r: &mut Reader<'_>) -> Result<TerminalQuotientCatalog, CodecError> {
    Wire::read(r)
}

/// Encode retained identity data; this does not admit its mathematical claims.
pub fn encode_quotient_catalog(catalog: &TerminalQuotientCatalog) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    catalog.write(&mut writer)?;
    Ok(writer.finish())
}
pub fn decode_quotient_catalog(bytes: &[u8]) -> Result<TerminalQuotientCatalog, CodecError> {
    let mut reader = Reader::new(bytes);
    let catalog = TerminalQuotientCatalog::read(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if encode_quotient_catalog(&catalog)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(catalog)
}
