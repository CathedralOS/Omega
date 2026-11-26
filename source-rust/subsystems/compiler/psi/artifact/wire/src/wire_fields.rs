//! Fields used by the complete Psi artifact. Shared declaration fields come from the declaration encoder.

pub(crate) use crate::semantic_module::CodecError;

use super::proof_term_wire::{decode_proof_terms, encode_proof_terms};
use super::scalar_term_wire::{decode_scalar_terms, encode_scalar_terms};
use super::scalar_wire::{
    decode_ieee_float_value, decode_integer_value, encode_ieee_float_value, encode_integer_value,
};
use super::structural_place_wire::{decode_structural_arguments, encode_structural_arguments};
use super::wire::{Reader, Writer};
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

wire_record! {
    Argument: StructuralArgument { place: Id, access: Access, path: Path }
    ResidentSource: ResidentReferenceSource { base: Id, length: Id, granted: Id, resident: Id }
    ScalarCaseFieldRow: ScalarCaseField { field: Id, value: Id, range_obligation: OptionalId }
    RecordFieldInitializerRow: RecordFieldInitializer { field: Id, value: RecordFieldValueRow }
    ClaimTransferRow: ClaimTransfer { claim: Id, argument_index: U32, path: Path }
    CompletionReceiptRow: CompletionReceipt { claim: Id, argument_index: U32, path: Path }
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
    Referent: StructuralReferentIdentity {
        id: Id,
        identity: Index,
        foreign_descriptor: Flag,
        abstract_opaque: Flag,
    }
    ClaimBinding: StructuralResultClaimBinding { claim: Id, path: Path }
}

pub(crate) type Resident = Optional<ResidentSource>;
pub(crate) type ScalarCaseFields = List<ScalarCaseFieldRow>;
pub(crate) type RecordFields = List<RecordFieldInitializerRow>;
pub(crate) type ClaimTransfers = List<ClaimTransferRow>;
pub(crate) type CompletionReceipts = List<CompletionReceiptRow>;
pub(crate) type SelectedEvidence = List<SelectedEvidenceRow>;
pub(crate) type EvidenceInterface = super::proof_declaration_wire::Interface;
pub(crate) type CrashRoutes = List<super::contract_wire::Bucket>;

pub(crate) use psi_callable_encoding::contracts::MachineDeclaration;
pub(crate) use psi_callable_encoding::signatures::{ArgumentSubstitution, Commitment};
pub(crate) use psi_declaration_encoding::structural_fields::FieldPath;

pub(crate) struct I64;
impl Field<i64> for I64 {
    fn write(writer: &mut Writer, value: &i64) -> Result<(), CodecError> {
        writer.bytes(&value.to_le_bytes());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<i64, CodecError> {
        Ok(i64::from_le_bytes(reader.array()?))
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

pub(crate) struct IntegerTypeField;
impl Field<semantic_vocabulary::IntegerType> for IntegerTypeField {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::IntegerType,
    ) -> Result<(), CodecError> {
        super::scalar_wire::encode_integer_type(writer, *value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::IntegerType, CodecError> {
        super::scalar_wire::decode_integer_type(reader)
    }
}

pub(crate) struct FloatFormatField;
impl Field<semantic_vocabulary::IeeeFloatFormat> for FloatFormatField {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::IeeeFloatFormat,
    ) -> Result<(), CodecError> {
        super::structural_field_wire::encode_ieee_float_format(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::IeeeFloatFormat, CodecError> {
        super::structural_field_wire::decode_ieee_float_format(reader)
    }
}

pub(crate) struct OptionalAccess;
impl Field<Option<terminal_psi::StructuralAccess>> for OptionalAccess {
    fn write(
        writer: &mut Writer,
        value: &Option<terminal_psi::StructuralAccess>,
    ) -> Result<(), CodecError> {
        match value {
            Some(access) => {
                super::structural_signature_wire::encode_structural_access(writer, *access)
            }
            None => writer.u8(0),
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<terminal_psi::StructuralAccess>, CodecError> {
        super::structural_signature_wire::decode_optional_structural_access(reader)
    }
}

wire_enum! {
    FloatComparison: IeeeFloatComparisonOperation, "IeeeFloatComparisonOperation" {
        0 => Equal, 1 => NotEqual, 2 => Less, 3 => LessOrEqual, 4 => Greater, 5 => GreaterOrEqual,
    }
    Predicate: ByteSequencePredicate, "ByteSequencePredicate" {
        1 => ValidUtf8, 2 => NoNul, 4 => AsciiOnly, 8 => NonEmpty,
    }
    Region: ElementViewRegion, "element view region" {
        0 => WholeCollection,
        2 => WholeRepresentation { source_length: Id, obligation: Id },
        1 => RepresentationBytes { start: Id, end: Id, obligation: Id },
    }
    ByteOperand: ByteSequenceOperand, "byte sequence operand" {
        2 => Whole { source: Argument, length: Id },
        1 => Field { source: Argument, field: Id, length: Id },
    }
    RecordFieldValueRow: RecordFieldValue, "RecordFieldValue" {
        1 => Scalar { value: Id, range_obligation: OptionalId },
        2 => Structural(argument: Argument),
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
            WrappingAddU8x16 => writer.u8(3),
            MinI16x8 => writer.u8(4),
            CmpGtI16x8 => writer.u8(5),
            BitAndI16x8 => writer.u8(6),
            BitAndNotI16x8 => writer.u8(7),
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
                let lanes = reader.array::<8>()?;
                if lanes.iter().any(|&index| index >= 16) {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "packed shuffle index exceeds both input arrays",
                    ));
                }
                ShuffleI16x8 { lanes }
            }
            3 => WrappingAddU8x16,
            4 => MinI16x8,
            5 => CmpGtI16x8,
            6 => BitAndI16x8,
            7 => BitAndNotI16x8,
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
            AbsI16x8 => writer.u8(6),
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
            6 => AbsI16x8,
            tag => return Err(CodecError::InvalidTag("PackedIntegerUnaryOperation", tag)),
        })
    }
}

delegated_fields! {
    Arguments: Vec<StructuralArgument> => encode_structural_arguments, decode_structural_arguments;
    ScalarTermField: ScalarTerm
        => super::scalar_term_wire::encode_scalar_term, super::scalar_term_wire::decode_scalar_term;
    ScalarTerms: Vec<ScalarTerm> => encode_scalar_terms, decode_scalar_terms;
    ProofTerms: Vec<ProofTerm> => encode_proof_terms, decode_proof_terms;
    StructuralType: terminal_psi::StructuralTypeDeclaration
        => super::structural_type_wire::encode_structural_type,
           super::structural_type_wire::decode_structural_type;
    CleanupAction: terminal_psi::TerminalAffineCleanupAction
        => super::structural_place_wire::encode_affine_cleanup_action,
           super::structural_place_wire::decode_affine_cleanup_action;
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
