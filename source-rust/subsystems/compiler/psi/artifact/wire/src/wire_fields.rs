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
    IeeeFloatComparisonOperation, IeeeFloatValue, IntegerValue, ProofTerm, ScalarTerm,
};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequencePredicate, ClaimTransfer, CompletionReceipt, ElementViewRegion,
    OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificCallResultSubstitution, OutcomeSpecificEvidenceUse, RecordFieldInitializer,
    RecordFieldValue, ResidentReferenceSource, ScalarCaseField, StructuralArgument,
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

pub(crate) use psi_declaration_encoding::structural_fields::FieldPath;

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

pub(crate) struct PackedIntegerBinary;
impl Field<semantic_vocabulary::PackedIntegerBinaryOperation> for PackedIntegerBinary {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::PackedIntegerBinaryOperation,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::PackedIntegerBinaryOperation::*;
        match value {
            WrappingAddI16x8 => writer.u8(0),
            WrappingSubI16x8 => writer.u8(1),
            ShuffleI16x8 { lanes } => {
                writer.u8(2);
                writer.bytes(lanes);
            }
        }
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::PackedIntegerBinaryOperation, CodecError> {
        use semantic_vocabulary::PackedIntegerBinaryOperation::*;
        Ok(match reader.u8()? {
            0 => WrappingAddI16x8,
            1 => WrappingSubI16x8,
            2 => {
                let lanes = <[u8; 8]>::try_from(reader.take(8)?).expect("eight wire bytes");
                if lanes.iter().any(|&index| index >= 16) {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "packed shuffle index exceeds both input arrays",
                    ));
                }
                ShuffleI16x8 { lanes }
            }
            tag => return Err(CodecError::InvalidTag("PackedIntegerBinaryOperation", tag)),
        })
    }
}

pub(crate) struct PackedIntegerUnary;
impl Field<semantic_vocabulary::PackedIntegerUnaryOperation> for PackedIntegerUnary {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::PackedIntegerUnaryOperation,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::PackedIntegerUnaryOperation::*;
        match value {
            ShiftRightArithmeticI16x8 { count } => {
                writer.u8(0);
                writer.u8(*count);
            }
            WidenU8x8ToI16x8 => writer.u8(1),
            NarrowI16x8ToU8x8Saturating => writer.u8(2),
            LoadI16x4ZeroExtend => writer.u8(3),
            WidenU8x4ToI16x8 => writer.u8(4),
            NarrowI16x8ToU8x4Saturating => writer.u8(5),
        }
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::PackedIntegerUnaryOperation, CodecError> {
        use semantic_vocabulary::PackedIntegerUnaryOperation::*;
        Ok(match reader.u8()? {
            0 => {
                let count = reader.u8()?;
                if count >= 16 {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "packed shift count exceeds the lane width",
                    ));
                }
                ShiftRightArithmeticI16x8 { count }
            }
            1 => WidenU8x8ToI16x8,
            2 => NarrowI16x8ToU8x8Saturating,
            3 => LoadI16x4ZeroExtend,
            4 => WidenU8x4ToI16x8,
            5 => NarrowI16x8ToU8x4Saturating,
            tag => return Err(CodecError::InvalidTag("PackedIntegerUnaryOperation", tag)),
        })
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

pub(crate) type EvidenceInterface = super::proof_declaration_wire::Interface;

pub(crate) use psi_callable_encoding::signatures::{ArgumentSubstitution, Commitment};

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

pub(crate) use psi_callable_encoding::contracts::MachineDeclaration;

delegated_fields! {
    StructuralType: terminal_psi::StructuralTypeDeclaration
        => super::structural_type_wire::encode_structural_type,
           super::structural_type_wire::decode_structural_type;
}

wire_record! {
    Referent: StructuralReferentIdentity { id: Id, identity: Index, foreign_descriptor: Flag, abstract_opaque: Flag }
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
