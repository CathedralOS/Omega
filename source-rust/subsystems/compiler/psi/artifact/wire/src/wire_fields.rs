//! Fields used by the complete Psi artifact. Shared declaration fields come from the declaration encoder.

pub(crate) use crate::semantic_module::CodecError;

use super::proof_term_wire::{decode_proof_terms, encode_proof_terms};
use super::scalar_term_wire::{decode_scalar_terms, encode_scalar_terms};
use super::scalar_wire::{
    decode_ieee_float_value, decode_integer_value, encode_ieee_float_value, encode_integer_value,
};
use super::structural_place_wire::{
    decode_structural_arguments, decode_structural_path, encode_structural_arguments,
    encode_structural_path,
};
use super::structural_signature_wire::{decode_structural_access, encode_structural_access};
use super::wire::{Reader, Writer, decode_counted, decode_optional_id, encode_optional_id};
pub(crate) use psi_declaration_encoding::fields::*;
pub(crate) use psi_declaration_encoding::wire;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatComparisonOperation, IeeeFloatValue, IntegerValue,
    ProofTerm, ScalarTerm,
};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequencePredicate, ClaimTransfer, CompletionReceipt, ElementViewRegion,
    OutcomeSpecificCallArgumentSubstitution, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEvidenceUse, RecordFieldInitializer, RecordFieldValue, ResidentReferenceSource,
    ScalarCaseField, StructuralArgument, StructuralPathQualification, StructuralPathSegment,
    StructuralReferentIdentity, StructuralResultClaimBinding,
};

pub(crate) struct Argument;
impl Field<StructuralArgument> for Argument {
    fn write(writer: &mut Writer, value: &StructuralArgument) -> Result<(), CodecError> {
        writer.id(value.place);
        encode_structural_access(writer, value.access);
        encode_structural_path(writer, "operation argument path", &value.path)
    }
    fn read(reader: &mut Reader<'_>) -> Result<StructuralArgument, CodecError> {
        Ok(StructuralArgument {
            place: reader.id("PlaceId")?,
            access: decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        })
    }
}

pub(crate) struct FieldPath;
impl Field<Vec<CanonicalStructuralPathSegment>> for FieldPath {
    fn write(
        writer: &mut Writer,
        value: &Vec<CanonicalStructuralPathSegment>,
    ) -> Result<(), CodecError> {
        super::structural_field_wire::encode_canonical_structural_path(
            writer,
            value,
            "scalar field carrier path",
        )
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<CanonicalStructuralPathSegment>, CodecError> {
        decode_counted(reader, |reader| match reader.u8()? {
            1 => Ok(CanonicalStructuralPathSegment::Field(
                reader.id("StructuralFieldId")?,
            )),
            2 => Ok(CanonicalStructuralPathSegment::FixedIndex(reader.u64()?)),
            3 => Ok(CanonicalStructuralPathSegment::Case(
                reader.id("StructuralCaseId")?,
            )),
            4 => Ok(CanonicalStructuralPathSegment::Referent),
            tag => Err(CodecError::InvalidTag("scalar field carrier path", tag)),
        })
    }
}

pub(crate) struct Integer;
impl Field<IntegerValue> for Integer {
    fn write(writer: &mut Writer, value: &IntegerValue) -> Result<(), CodecError> {
        encode_integer_value(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
        decode_integer_value(reader)
    }
}

pub(crate) struct Float;
impl Field<IeeeFloatValue> for Float {
    fn write(writer: &mut Writer, value: &IeeeFloatValue) -> Result<(), CodecError> {
        encode_ieee_float_value(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<IeeeFloatValue, CodecError> {
        decode_ieee_float_value(reader)
    }
}

macro_rules! byte_enums {
    ($($marker:ident: $ty:ty, $name:literal { $($variant:ident = $byte:literal),* $(,)? })*) => {$(
        pub(crate) struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                writer.u8(match value { $(<$ty>::$variant => $byte,)* });
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                match reader.u8()? {
                    $($byte => Ok(<$ty>::$variant),)*
                    tag => Err(CodecError::InvalidTag($name, tag)),
                }
            }
        }
    )*};
}
byte_enums! {
    FloatComparison: IeeeFloatComparisonOperation, "IeeeFloatComparisonOperation" {
        Equal = 0, NotEqual = 1, Less = 2, LessOrEqual = 3, Greater = 4, GreaterOrEqual = 5,
    }
    Predicate: ByteSequencePredicate, "ByteSequencePredicate" {
        ValidUtf8 = 1, NoNul = 2, AsciiOnly = 4, NonEmpty = 8,
    }
}

pub(crate) struct Resident;
impl Field<Option<ResidentReferenceSource>> for Resident {
    fn write(
        writer: &mut Writer,
        value: &Option<ResidentReferenceSource>,
    ) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some(resident) = value {
            writer.id(resident.base);
            writer.id(resident.length);
            writer.id(resident.granted);
            writer.id(resident.resident);
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<ResidentReferenceSource>, CodecError> {
        if !reader.boolean()? {
            return Ok(None);
        }
        Ok(Some(ResidentReferenceSource {
            base: reader.id("StructuralFieldId")?,
            length: reader.id("StructuralFieldId")?,
            granted: reader.id("StructuralDomainId")?,
            resident: reader.id("StructuralDomainId")?,
        }))
    }
}

pub(crate) struct Region;
impl Field<ElementViewRegion> for Region {
    fn write(writer: &mut Writer, value: &ElementViewRegion) -> Result<(), CodecError> {
        match value {
            ElementViewRegion::WholeCollection => writer.u8(0),
            ElementViewRegion::WholeRepresentation {
                source_length,
                obligation,
            } => {
                writer.u8(2);
                writer.id(*source_length);
                writer.id(*obligation);
            }
            ElementViewRegion::RepresentationBytes {
                start,
                end,
                obligation,
            } => {
                writer.u8(1);
                writer.id(*start);
                writer.id(*end);
                writer.id(*obligation);
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ElementViewRegion, CodecError> {
        match reader.u8()? {
            0 => Ok(ElementViewRegion::WholeCollection),
            2 => Ok(ElementViewRegion::WholeRepresentation {
                source_length: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            }),
            1 => Ok(ElementViewRegion::RepresentationBytes {
                start: reader.id("ValueId")?,
                end: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            }),
            tag => Err(CodecError::InvalidTag("element view region", tag)),
        }
    }
}

pub(crate) struct ByteOperand;
impl Field<ByteSequenceOperand> for ByteOperand {
    fn write(writer: &mut Writer, value: &ByteSequenceOperand) -> Result<(), CodecError> {
        match value {
            ByteSequenceOperand::Whole { source, length } => {
                writer.u8(2);
                Argument::write(writer, source)?;
                writer.id(*length);
            }
            ByteSequenceOperand::Field {
                source,
                field,
                length,
            } => {
                writer.u8(1);
                Argument::write(writer, source)?;
                writer.id(*field);
                writer.id(*length);
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<ByteSequenceOperand, CodecError> {
        match reader.u8()? {
            2 => Ok(ByteSequenceOperand::Whole {
                source: Argument::read(reader)?,
                length: reader.id("ValueId")?,
            }),
            1 => Ok(ByteSequenceOperand::Field {
                source: Argument::read(reader)?,
                field: reader.id("StructuralFieldId")?,
                length: reader.id("ValueId")?,
            }),
            tag => Err(CodecError::InvalidTag("byte sequence operand", tag)),
        }
    }
}

pub(crate) struct ScalarCaseFields;
impl Field<Vec<ScalarCaseField>> for ScalarCaseFields {
    fn write(writer: &mut Writer, value: &Vec<ScalarCaseField>) -> Result<(), CodecError> {
        writer.len("scalar case fields", value.len())?;
        for field in value {
            writer.id(field.field);
            writer.id(field.value);
            encode_optional_id(writer, field.range_obligation);
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<ScalarCaseField>, CodecError> {
        decode_counted(reader, |reader| {
            Ok(ScalarCaseField {
                field: reader.id("StructuralFieldId")?,
                value: reader.id("ValueId")?,
                range_obligation: decode_optional_id(reader, "ObligationId")?,
            })
        })
    }
}

pub(crate) struct RecordFields;
impl Field<Vec<RecordFieldInitializer>> for RecordFields {
    fn write(writer: &mut Writer, value: &Vec<RecordFieldInitializer>) -> Result<(), CodecError> {
        writer.len("record fields", value.len())?;
        for field in value {
            writer.id(field.field);
            match &field.value {
                RecordFieldValue::Scalar {
                    value,
                    range_obligation,
                } => {
                    writer.u8(1);
                    writer.id(*value);
                    encode_optional_id(writer, *range_obligation);
                }
                RecordFieldValue::Structural(argument) => {
                    writer.u8(2);
                    Argument::write(writer, argument)?;
                }
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<RecordFieldInitializer>, CodecError> {
        decode_counted(reader, |reader| {
            let field = reader.id("StructuralFieldId")?;
            let value = match reader.u8()? {
                1 => RecordFieldValue::Scalar {
                    value: reader.id("ValueId")?,
                    range_obligation: decode_optional_id(reader, "ObligationId")?,
                },
                2 => RecordFieldValue::Structural(Argument::read(reader)?),
                tag => return Err(CodecError::InvalidTag("RecordFieldValue", tag)),
            };
            Ok(RecordFieldInitializer { field, value })
        })
    }
}

macro_rules! delegated_fields {
    ($($marker:ident: $ty:ty => $encode:path, $decode:path;)*) => {$(
        pub(crate) struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                $encode(writer, value)
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                $decode(reader)
            }
        }
    )*};
}
pub(crate) use delegated_fields;
delegated_fields! {
    Arguments: Vec<StructuralArgument> => encode_structural_arguments, decode_structural_arguments;
    ScalarTerms: Vec<ScalarTerm> => encode_scalar_terms, decode_scalar_terms;
    ProofTerms: Vec<ProofTerm> => encode_proof_terms, decode_proof_terms;
}

macro_rules! argument_claims {
    ($($marker:ident: $ty:ident, $label:literal;)*) => {$(
        pub(crate) struct $marker;
        impl Field<Vec<$ty>> for $marker {
            fn write(writer: &mut Writer, value: &Vec<$ty>) -> Result<(), CodecError> {
                writer.len($label, value.len())?;
                for row in value {
                    writer.id(row.claim);
                    writer.u32(row.argument_index);
                    encode_structural_path(writer, $label, &row.path)?;
                }
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<Vec<$ty>, CodecError> {
                decode_counted(reader, |reader| {
                    Ok($ty {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                        path: decode_structural_path(reader)?,
                    })
                })
            }
        }
    )*};
}
argument_claims! {
    ClaimTransfers: ClaimTransfer, "call claim transfers";
    CompletionReceipts: CompletionReceipt, "boundary claim settlements";
}

wire_record! {
    SelectedEvidenceRow: OutcomeSpecificCallEvidence {
        guard: super::contract_wire::Guard,
        position: U32,
        callee_obligation: Id,
        callee_term: Id,
        output_field: Text,
        callee_proposition: Id,
        instantiated_proposition: Id,
        output: Id,
        result_substitution: Optional<ResultSubstitution>,
        argument_substitutions: List<ArgumentSubstitution>,
        validity: EvidenceValidity,
        expected_use_count: U32,
        uses: List<EvidenceUse>,
    }
    ResultSubstitution: OutcomeSpecificCallResultSubstitution {
        argument_position: U32,
        callee_result: Id,
        caller_result: Id,
    }
    EvidenceValidity: OutcomeSpecificCallEvidenceValidity {
        result: Id,
        proposition_dependencies: Ids,
        evidence_interface: EvidenceInterface,
        interface_dependencies: Ids,
    }
    EvidenceUse: OutcomeSpecificEvidenceUse {
        target: Id,
        input_position: U32,
        target_requirement: Id,
        target_term: Id,
        source: Id,
        instantiated_proposition: Id,
        target_parameter: Id,
        caller_result: Id,
    }
}
pub(crate) type SelectedEvidence = List<SelectedEvidenceRow>;

macro_rules! copied_fields {
    ($($marker:ident: $ty:ty => $encode:path, $decode:path;)*) => {$(
        pub(crate) struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                $encode(writer, *value);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                $decode(reader)
            }
        }
    )*};
}
copied_fields! {
    Access: terminal_psi::StructuralAccess => encode_structural_access, decode_structural_access;
    Multiplicity: terminal_psi::StructuralMultiplicity
        => super::structural_result_wire::encode_multiplicity,
           super::structural_result_wire::decode_multiplicity;
}

pub(crate) struct Scalar;
impl Field<semantic_vocabulary::ScalarType> for Scalar {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ScalarType,
    ) -> Result<(), CodecError> {
        super::scalar_wire::encode_scalar_type(writer, *value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::ScalarType, CodecError> {
        super::scalar_wire::decode_scalar_type(reader)
    }
}

delegated_fields! {
    CallResult: terminal_psi::BoundaryMachineResult
        => super::structural_signature_wire::encode_call_result,
           super::structural_signature_wire::decode_call_result;
}

pub(crate) struct DeclaredLifetimeDependency;
impl Field<terminal_psi::BoundaryLifetimeDependency> for DeclaredLifetimeDependency {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::BoundaryLifetimeDependency,
    ) -> Result<(), CodecError> {
        super::structural_signature_wire::reference_outputs::encode_dependency(
            writer,
            value,
            super::structural_place_wire::encode_declared_lifetime_path,
        )
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::BoundaryLifetimeDependency, CodecError> {
        super::structural_signature_wire::reference_outputs::decode_dependency(
            reader,
            super::structural_place_wire::decode_declared_lifetime_path,
        )
    }
}

pub(crate) type EvidenceInterface = super::proof_declaration_wire::Interface;

pub(crate) struct Commitment;
impl Field<terminal_psi::ClosedConformanceApplicationCommitment> for Commitment {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::ClosedConformanceApplicationCommitment,
    ) -> Result<(), CodecError> {
        writer.bytes(&value.as_bytes());
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::ClosedConformanceApplicationCommitment, CodecError> {
        Ok(terminal_psi::ClosedConformanceApplicationCommitment::from_digest(reader.array()?))
    }
}

pub(crate) struct InvocationSite;
impl Field<semantic_vocabulary::InvocationSite> for InvocationSite {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::InvocationSite,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::InvocationSite as Site;

        let (tag, identity, action_ordinal) = match *value {
            Site::Operation(operation) => (0, operation.get(), None),
            Site::ExternalCompletion(edge) => (1, edge.get(), None),
            Site::InvokeNoNormal(edge) => (2, edge.get(), None),
            Site::Cleanup {
                edge,
                action_ordinal,
            } => (3, edge.get(), Some(action_ordinal)),
        };
        writer.u8(tag);
        writer.u64(identity);
        if let Some(action_ordinal) = action_ordinal {
            writer.u32(action_ordinal);
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::InvocationSite, CodecError> {
        use semantic_vocabulary::InvocationSite as Site;

        let tag = reader.u8()?;
        let identity = reader.u64()?;
        if tag == 3 {
            let edge = semantic_vocabulary::EdgeId::new(identity).ok_or(
                CodecError::MalformedStructuralFoundation("invalid cleanup invocation edge"),
            )?;
            return Ok(Site::Cleanup {
                edge,
                action_ordinal: reader.u32()?,
            });
        }
        Site::from_tagged_identity(tag, identity).ok_or(CodecError::MalformedStructuralFoundation(
            "invalid invocation identity",
        ))
    }
}

wire_record! {
    ArgumentSubstitution: OutcomeSpecificCallArgumentSubstitution {
        argument_position: U32,
        parameter_position: U32,
        structural_path: Optional<Path>,
    }
}

pub(crate) struct ReachArgument;
impl Field<terminal_psi::ClosedReachArgument> for ReachArgument {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::ClosedReachArgument,
    ) -> Result<(), CodecError> {
        super::reach_application_wire::encode_argument(writer, value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::ClosedReachArgument, CodecError> {
        super::reach_application_wire::decode_argument(reader)
    }
}

pub(crate) struct QualificationPath;
impl Field<Vec<StructuralPathSegment>> for QualificationPath {
    fn write(writer: &mut Writer, value: &Vec<StructuralPathSegment>) -> Result<(), CodecError> {
        super::structural_place_wire::encode_qualification_path(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<StructuralPathSegment>, CodecError> {
        super::structural_place_wire::decode_qualification_path(reader)
    }
}

delegated_fields! {
    MachineDeclaration: terminal_psi::CheckedMachineDeclaration
        => super::machine_declaration_wire::encode, super::machine_declaration_wire::decode;
}

wire_record! {
    PathQualification: StructuralPathQualification { path: QualificationPath, domain: Id }
}

delegated_fields! {
    StructuralType: terminal_psi::StructuralTypeDeclaration
        => super::structural_type_wire::encode_structural_type,
           super::structural_type_wire::decode_structural_type;
}

wire_record! {
    Referent: StructuralReferentIdentity { id: Id, identity: Index, foreign_descriptor: Flag, abstract_opaque: Flag }
}

pub(crate) struct QualificationSet;
impl Field<semantic_vocabulary::ScalarQualificationSetId> for QualificationSet {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ScalarQualificationSetId,
    ) -> Result<(), CodecError> {
        writer.u64(value.get());
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::ScalarQualificationSetId, CodecError> {
        Ok(semantic_vocabulary::ScalarQualificationSetId::new(
            reader.u64()?,
        ))
    }
}

pub(crate) type CrashRoutes = List<super::contract_wire::Bucket>;

delegated_fields! {
    CleanupAction: terminal_psi::TerminalAffineCleanupAction
        => super::structural_place_wire::encode_affine_cleanup_action,
           super::structural_place_wire::decode_affine_cleanup_action;
}

wire_record! {
    ClaimBinding: StructuralResultClaimBinding { claim: Id, path: Path }
}

macro_rules! optional_packages {
    ($($marker:ident: $empty:literal;)*) => {$(
        pub(crate) struct $marker;
        impl Field<Option<semantic_vocabulary::PackageKeyIdentity>> for $marker {
            fn write(
                writer: &mut Writer,
                value: &Option<semantic_vocabulary::PackageKeyIdentity>,
            ) -> Result<(), CodecError> {
                writer.boolean(value.is_some());
                if let Some(package) = value {
                    writer.bytes(&package.digest());
                }
                Ok(())
            }
            fn read(
                reader: &mut Reader<'_>,
            ) -> Result<Option<semantic_vocabulary::PackageKeyIdentity>, CodecError> {
                if !reader.boolean()? {
                    return Ok(None);
                }
                semantic_vocabulary::PackageKeyIdentity::from_digest(reader.array()?)
                    .map(Some)
                    .ok_or(CodecError::MalformedStructuralFoundation($empty))
            }
        }
    )*};
}
optional_packages! {
    TaskPackage: "task requirement has a zero package identity";
    ExternalPackage: "empty external provider package identity";
    ProviderPackage: "empty provider source package identity";
    BoundaryPackage: "empty boundary package identity";
}

pub(crate) struct Custody;
impl Field<semantic_vocabulary::compiler_evaluation::DeclarationCustody> for Custody {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::compiler_evaluation::DeclarationCustody,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
        writer.u8(match value {
            DeclarationCustody::Package(_) => 1,
            DeclarationCustody::Toolchain => 2,
            DeclarationCustody::UnownedUser => 3,
            DeclarationCustody::Missing => 4,
            DeclarationCustody::ToolchainContract => 5,
        });
        if let DeclarationCustody::Package(package) = value {
            ProviderPackage::write(writer, &Some(*package))?;
        }
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, CodecError> {
        use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
        Ok(match reader.u8()? {
            1 => DeclarationCustody::Package(ProviderPackage::read(reader)?.ok_or(
                CodecError::MalformedStructuralFoundation(
                    "compiler-evaluation package custody is absent",
                ),
            )?),
            2 => DeclarationCustody::Toolchain,
            3 => DeclarationCustody::UnownedUser,
            4 => DeclarationCustody::Missing,
            5 => DeclarationCustody::ToolchainContract,
            tag => return Err(CodecError::InvalidTag("compiler-evaluation custody", tag)),
        })
    }
}

use terminal_psi::GlobalDescriptorTableOperation;
wire_enum! {
    GlobalDescriptorTable: GlobalDescriptorTableOperation, "GlobalDescriptorTableOperation" {
        0 => Store,
        1 => Load,
    }
}

use terminal_psi::{SegmentRegister, SegmentReload};
wire_enum! {
    SegmentReloadKind: SegmentReload, "SegmentReload" {
        0 => Code,
        1 => Data,
    }
    SegmentSelectorRegister: SegmentRegister, "SegmentRegister" {
        0 => Code,
        1 => Data,
        2 => Extra,
        3 => Stack,
        4 => Fs,
        5 => Gs,
    }
}

#[cfg(test)]
mod invocation_site_tests {
    use super::wire::{Reader, Writer};
    use super::{Field, List, U32};
    use crate::CodecError;
    use semantic_vocabulary::{EdgeId, InvocationSite, OperationId};

    #[test]
    fn minimum_row_bytes_refuse_the_count_before_reading_any_row() {
        let bytes = [2, 0, 0, 0, 7, 0, 0, 0];
        let mut reader = Reader::new(&bytes);
        assert!(matches!(
            <List<U32, 4> as Field<Vec<u32>>>::read(&mut reader),
            Err(CodecError::UnexpectedEnd)
        ));
        assert_eq!(reader.remaining(), 4);
        let rows = vec![7, 9];
        let mut existing = Writer::default();
        <List<U32> as Field<Vec<u32>>>::write(&mut existing, &rows).unwrap();
        let mut bounded = Writer::default();
        <List<U32, 4> as Field<Vec<u32>>>::write(&mut bounded, &rows).unwrap();
        let bytes = bounded.finish();
        assert_eq!(bytes, existing.finish());
        assert_eq!(
            <List<U32, 4> as Field<Vec<u32>>>::read(&mut Reader::new(&bytes)).unwrap(),
            rows
        );
    }

    #[test]
    fn invocation_identity_retains_cleanup_edge_and_action_ordinal() {
        let mut encodings = Vec::new();
        for site in [
            InvocationSite::Operation(OperationId::new(1).unwrap()),
            InvocationSite::ExternalCompletion(EdgeId::new(2).unwrap()),
            InvocationSite::InvokeNoNormal(EdgeId::new(2).unwrap()),
            InvocationSite::Cleanup {
                edge: EdgeId::new(2).unwrap(),
                action_ordinal: 0,
            },
            InvocationSite::Cleanup {
                edge: EdgeId::new(2).unwrap(),
                action_ordinal: 1,
            },
            InvocationSite::Cleanup {
                edge: EdgeId::new(3).unwrap(),
                action_ordinal: 0,
            },
        ] {
            let mut writer = Writer::default();
            super::InvocationSite::write(&mut writer, &site).unwrap();
            let bytes = writer.finish();
            let mut expected = Vec::new();
            site.encode_identity(&mut expected);
            assert_eq!(bytes, expected);
            let mut reader = Reader::new(&bytes);
            assert_eq!(super::InvocationSite::read(&mut reader).unwrap(), site);
            assert_eq!(reader.remaining(), 0);
            assert!(!encodings.contains(&bytes));
            encodings.push(bytes);
        }
        let cleanup = encodings.last().unwrap();
        assert_eq!(cleanup.len(), 13);
        for length in 0..cleanup.len() {
            assert!(super::InvocationSite::read(&mut Reader::new(&cleanup[..length])).is_err());
        }
    }
}
