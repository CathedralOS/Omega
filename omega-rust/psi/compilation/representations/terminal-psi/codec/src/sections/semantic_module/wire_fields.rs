//! Wire spellings of the recorded fields an operation row carries.
//!
//! Each marker names one field spelling; `operation_wire!` in `block_wire` pairs
//! an operation's fields with these markers once, and both directions read
//! that pairing. Decoding labels name the identity type, which is all a zero
//! identity error reports.

use super::CodecError;
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
use super::wire::{
    Reader, Writer, decode_bytes, decode_counted, decode_ids, decode_optional_id,
    encode_optional_id,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentProjectionIdentity, IeeeFloatComparisonOperation,
    IeeeFloatValue, IntegerValue, ProofTerm, PsiSemanticId, ScalarTerm, StructuralFieldId,
};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequencePredicate, ClaimTransfer, CompletionReceipt, ElementViewRegion,
    OutcomeSpecificCallArgumentSubstitution, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEvidenceUse, RecordFieldInitializer, RecordFieldValue, ResidentReferenceSource,
    ScalarCaseField, StructuralArgument, StructuralContentProjection, StructuralPathQualification,
    StructuralPathSegment, StructuralReferentIdentity, StructuralResultClaimBinding,
};

pub(super) trait Field<T> {
    fn write(writer: &mut Writer, value: &T) -> Result<(), CodecError>;
    fn read(reader: &mut Reader<'_>) -> Result<T, CodecError>;
}

fn label<T>() -> &'static str {
    let name = std::any::type_name::<T>();
    name.rsplit("::").next().unwrap_or(name)
}

pub(super) struct Id;
impl<T: PsiSemanticId> Field<T> for Id {
    fn write(writer: &mut Writer, value: &T) -> Result<(), CodecError> {
        writer.id(*value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<T, CodecError> {
        reader.id(label::<T>())
    }
}

pub(super) struct OptionalId;
impl<T: PsiSemanticId> Field<Option<T>> for OptionalId {
    fn write(writer: &mut Writer, value: &Option<T>) -> Result<(), CodecError> {
        encode_optional_id(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<T>, CodecError> {
        decode_optional_id(reader, label::<T>())
    }
}

pub(super) struct Ids;
impl<T: PsiSemanticId> Field<Vec<T>> for Ids {
    fn write(writer: &mut Writer, value: &Vec<T>) -> Result<(), CodecError> {
        writer.ids("operation identities", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<T>, CodecError> {
        decode_ids(reader, label::<T>())
    }
}

/// A field identity, or zero for the whole projected place. Nonzero
/// identities keep their ordinary spelling; zero is the one value a field
/// identity never takes.
pub(super) struct OptionalField;
impl Field<Option<StructuralFieldId>> for OptionalField {
    fn write(writer: &mut Writer, value: &Option<StructuralFieldId>) -> Result<(), CodecError> {
        writer.u64(value.map_or(0, StructuralFieldId::get));
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<StructuralFieldId>, CodecError> {
        Ok(StructuralFieldId::new(reader.u64()?))
    }
}

macro_rules! integer_fields {
    ($($marker:ident: $ty:ty => $write:ident / $read:ident),* $(,)?) => {$(
        pub(super) struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                writer.$write(*value);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                reader.$read()
            }
        }
    )*};
}
integer_fields! {
    U8: u8 => u8 / u8,
    U16: u16 => u16 / u16,
    U32: u32 => u32 / u32,
    U64: u64 => u64 / u64,
    Flag: bool => boolean / boolean,
}

/// An operand path; projected and whole spellings share one roster.
pub(super) struct Path;
impl Field<Vec<StructuralPathSegment>> for Path {
    fn write(writer: &mut Writer, value: &Vec<StructuralPathSegment>) -> Result<(), CodecError> {
        encode_structural_path(writer, "operation path", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<StructuralPathSegment>, CodecError> {
        decode_structural_path(reader)
    }
}

pub(super) struct Argument;
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

/// A scalar field's carrier path. One literal index may end the carrier when
/// the element record owns the observed field (`maps[1]` for `maps[1].value`);
/// a final case step names the payload namespace a case-qualified observation
/// reads. Foundation validation decides whether that carrier is admitted.
pub(super) struct FieldPath;
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

pub(super) struct Bytes;
impl Field<Vec<u8>> for Bytes {
    fn write(writer: &mut Writer, value: &Vec<u8>) -> Result<(), CodecError> {
        writer.len("byte-sequence literal bytes", value.len())?;
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
        decode_bytes(reader)
    }
}

pub(super) struct Integer;
impl Field<IntegerValue> for Integer {
    fn write(writer: &mut Writer, value: &IntegerValue) -> Result<(), CodecError> {
        encode_integer_value(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
        decode_integer_value(reader)
    }
}

pub(super) struct Float;
impl Field<IeeeFloatValue> for Float {
    fn write(writer: &mut Writer, value: &IeeeFloatValue) -> Result<(), CodecError> {
        encode_ieee_float_value(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<IeeeFloatValue, CodecError> {
        decode_ieee_float_value(reader)
    }
}

/// Closed tables of one-byte spellings; an unknown byte is an invalid tag.
macro_rules! byte_enums {
    ($($marker:ident: $ty:ty, $name:literal { $($variant:ident = $byte:literal),* $(,)? })*) => {$(
        pub(super) struct $marker;
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

pub(super) struct Resident;
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

pub(super) struct Region;
impl Field<ElementViewRegion> for Region {
    fn write(writer: &mut Writer, value: &ElementViewRegion) -> Result<(), CodecError> {
        match value {
            ElementViewRegion::WholeCollection => writer.u8(0),
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
            1 => Ok(ElementViewRegion::RepresentationBytes {
                start: reader.id("ValueId")?,
                end: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            }),
            tag => Err(CodecError::InvalidTag("element view region", tag)),
        }
    }
}

pub(super) struct ByteOperand;
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

pub(super) struct ScalarCaseFields;
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

pub(super) struct RecordFields;
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
        pub(super) struct $marker;
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
pub(super) use delegated_fields;
delegated_fields! {
    Arguments: Vec<StructuralArgument> => encode_structural_arguments, decode_structural_arguments;
    ScalarTerms: Vec<ScalarTerm> => encode_scalar_terms, decode_scalar_terms;
    ProofTerms: Vec<ProofTerm> => encode_proof_terms, decode_proof_terms;
}

/// Claims crossing a call or settled by a boundary completion: the claim, the
/// argument that carries it, and the path inside that argument.
macro_rules! argument_claims {
    ($($marker:ident: $ty:ident, $label:literal;)*) => {$(
        pub(super) struct $marker;
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

// Outcome-specific selections of the structural call encodings.
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
pub(super) type SelectedEvidence = List<SelectedEvidenceRow>;

/// By-value spellings owned by sibling wire modules.
macro_rules! copied_fields {
    ($($marker:ident: $ty:ty => $encode:path, $decode:path;)*) => {$(
        pub(super) struct $marker;
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
    Scalar: semantic_vocabulary::ScalarType
        => super::scalar_wire::encode_scalar_type, super::scalar_wire::decode_scalar_type;
}

/// A module identity-table index.
pub(super) struct Index;
impl Field<terminal_psi::TerminalIdentityIndex> for Index {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::TerminalIdentityIndex,
    ) -> Result<(), CodecError> {
        writer.u32(value.0);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::TerminalIdentityIndex, CodecError> {
        Ok(terminal_psi::TerminalIdentityIndex(reader.u32()?))
    }
}

pub(super) struct Text;
impl Field<String> for Text {
    fn write(writer: &mut Writer, value: &String) -> Result<(), CodecError> {
        writer.string("wire text", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<String, CodecError> {
        reader.string("wire text")
    }
}

/// A counted roster of one element spelling.
pub(super) struct List<K>(std::marker::PhantomData<K>);
impl<T, K: Field<T>> Field<Vec<T>> for List<K> {
    fn write(writer: &mut Writer, value: &Vec<T>) -> Result<(), CodecError> {
        writer.len("wire roster", value.len())?;
        for element in value {
            K::write(writer, element)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<T>, CodecError> {
        decode_counted(reader, K::read)
    }
}

/// An optional value behind a 0/1 presence byte.
pub(super) struct Optional<K>(std::marker::PhantomData<K>);
impl<T, K: Field<T>> Field<Option<T>> for Optional<K> {
    fn write(writer: &mut Writer, value: &Option<T>) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some(value) = value {
            K::write(writer, value)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<T>, CodecError> {
        Ok(if reader.boolean()? {
            Some(K::read(reader)?)
        } else {
            None
        })
    }
}

/// A record spelled as its fields in wire order. The destructuring names every
/// field, so a new field fails to compile until its spelling is chosen.
macro_rules! wire_record {
    ($($marker:ident: $ty:ident { $($field:ident: $kind:ty),* $(,)? })*) => {$(
        pub(super) struct $marker;
        impl $crate::sections::semantic_module::wire_fields::Field<$ty> for $marker {
            fn write(
                writer: &mut $crate::sections::semantic_module::wire::Writer,
                value: &$ty,
            ) -> Result<(), $crate::sections::semantic_module::CodecError> {
                let $ty { $($field),* } = value;
                $(<$kind as $crate::sections::semantic_module::wire_fields::Field<_>>::write(
                    writer, $field,
                )?;)*
                Ok(())
            }
            fn read(
                reader: &mut $crate::sections::semantic_module::wire::Reader<'_>,
            ) -> Result<$ty, $crate::sections::semantic_module::CodecError> {
                Ok($ty {
                    $($field: <$kind as $crate::sections::semantic_module::wire_fields::Field<_>>::read(
                        reader,
                    )?),*
                })
            }
        }
    )*};
}
pub(super) use wire_record;

pub(super) struct Strings;
impl Field<Vec<String>> for Strings {
    fn write(writer: &mut Writer, value: &Vec<String>) -> Result<(), CodecError> {
        writer.strings("wire texts", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<String>, CodecError> {
        reader.strings("wire texts")
    }
}

delegated_fields! {
    CallResult: terminal_psi::BoundaryMachineResult
        => super::structural_signature_wire::encode_call_result,
           super::structural_signature_wire::decode_call_result;
}

/// A closed enum spelled as one tag byte, then its variant's fields in wire
/// order. Each arm names every field of its variant; a one-field tuple
/// variant names its binding as `Variant(binding: Kind)`.
macro_rules! wire_enum {
    ($($marker:ident: $ty:ident, $name:literal {
        $($tag:literal => $variant:ident
            $(( $tuple_field:ident: $tuple_kind:ty ))?
            $({ $($field:ident: $kind:ty),* $(,)? })?
        ),* $(,)?
    })*) => {$(
        pub(super) struct $marker;
        impl $crate::sections::semantic_module::wire_fields::Field<$ty> for $marker {
            fn write(
                writer: &mut $crate::sections::semantic_module::wire::Writer,
                value: &$ty,
            ) -> Result<(), $crate::sections::semantic_module::CodecError> {
                match value {
                    $($ty::$variant $(($tuple_field))? $({ $($field),* })? => {
                        writer.u8($tag);
                        $(<$tuple_kind as $crate::sections::semantic_module::wire_fields::Field<_>>::write(writer, $tuple_field)?;)?
                        $($(<$kind as $crate::sections::semantic_module::wire_fields::Field<_>>::write(writer, $field)?;)*)?
                    })*
                }
                Ok(())
            }
            fn read(
                reader: &mut $crate::sections::semantic_module::wire::Reader<'_>,
            ) -> Result<$ty, $crate::sections::semantic_module::CodecError> {
                Ok(match reader.u8()? {
                    $($tag => $ty::$variant
                        $((<$tuple_kind as $crate::sections::semantic_module::wire_fields::Field<_>>::read(reader)?))?
                        $({ $($field: <$kind as $crate::sections::semantic_module::wire_fields::Field<_>>::read(reader)?),* })?,
                    )*
                    tag => return Err($crate::sections::semantic_module::CodecError::InvalidTag(
                        $name, tag,
                    )),
                })
            }
        }
    )*};
}
pub(super) use wire_enum;

pub(super) type EvidenceInterface = super::proof_declaration_wire::Interface;

/// The digest committing a closed conformance application.
pub(super) struct Commitment;
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

pub(super) struct InvocationSite;
impl Field<semantic_vocabulary::InvocationSite> for InvocationSite {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::InvocationSite,
    ) -> Result<(), CodecError> {
        super::wire::encode_invocation_site(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::InvocationSite, CodecError> {
        super::wire::decode_invocation_site(reader)
    }
}

wire_record! {
    ArgumentSubstitution: OutcomeSpecificCallArgumentSubstitution {
        argument_position: U32,
        parameter_position: U32,
        structural_path: Optional<Path>,
    }
}

/// One-field `u32` newtypes spelled as their index.
macro_rules! u32_newtypes {
    ($($marker:ident: $ty:path;)*) => {$(
        pub(super) struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                writer.u32(value.0);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                Ok($ty(reader.u32()?))
            }
        }
    )*};
}
pub(super) use u32_newtypes;

/// A fixed 32-byte commitment digest.
pub(super) struct Digest;
impl Field<[u8; 32]> for Digest {
    fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
        reader.array()
    }
}

/// A nominal declaration, spelled as its identity-table index.
pub(super) struct Nominal;
impl Field<terminal_psi::NominalDeclarationIdentity> for Nominal {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::NominalDeclarationIdentity,
    ) -> Result<(), CodecError> {
        writer.u32(value.identity().0);
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::NominalDeclarationIdentity, CodecError> {
        Ok(terminal_psi::NominalDeclarationIdentity::new(
            terminal_psi::TerminalIdentityIndex(reader.u32()?),
        ))
    }
}

/// A top-level proposition or content projection; nested depth starts at zero.
pub(super) struct Proposition;
impl Field<semantic_vocabulary::Proposition> for Proposition {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::Proposition,
    ) -> Result<(), CodecError> {
        super::proposition_wire::encode_proposition(writer, value, 0)
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::Proposition, CodecError> {
        super::proposition_wire::decode_proposition(reader, 0)
    }
}

pub(super) struct ContentExpression;
impl Field<semantic_vocabulary::ContentProjectionExpression> for ContentExpression {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ContentProjectionExpression,
    ) -> Result<(), CodecError> {
        super::structural_signature_wire::encode_content_projection_expression(writer, value)
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::ContentProjectionExpression, CodecError> {
        super::structural_signature_wire::decode_content_projection_expression(reader, 0)
    }
}

delegated_fields! {
    ContentAlgebra: semantic_vocabulary::ContentAlgebra
        => super::content_wire::encode_content_algebra, super::content_wire::decode_content_algebra;
}

pub(super) struct ReachArgument;
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

/// A qualification path: a structural path whose projections may name a case.
pub(super) struct QualificationPath;
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
    ProjectionIdentity: ContentProjectionIdentity { domain: Id, projection_report_fingerprint: U64 }
    ContentProjection: StructuralContentProjection {
        identity: ProjectionIdentity,
        algebra: ContentAlgebra,
        expression: ContentExpression,
    }
    PathQualification: StructuralPathQualification { path: QualificationPath, domain: Id }
}

delegated_fields! {
    StructuralType: terminal_psi::StructuralTypeDeclaration
        => super::structural_type_wire::encode_structural_type,
           super::structural_type_wire::decode_structural_type;
}

wire_record! {
    Referent: StructuralReferentIdentity { id: Id, identity: Index, foreign_descriptor: Flag }
}

/// A scalar qualification set, spelled as its identity; zero is the empty set.
pub(super) struct QualificationSet;
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

/// Crash continuations: buckets of guarded routes, one cause per bucket.
pub(super) type CrashRoutes = List<super::contract_wire::Bucket>;

delegated_fields! {
    CleanupAction: terminal_psi::TerminalAffineCleanupAction
        => super::structural_place_wire::encode_affine_cleanup_action,
           super::structural_place_wire::decode_affine_cleanup_action;
}

// Operation results, returning edges and loan completions carry the same
// path-based claim join. One row keeps every envelope on the Case guard and
// the typed source path, never machine-local identity order.
wire_record! {
    ClaimBinding: StructuralResultClaimBinding { claim: Id, path: Path }
}

/// An optional package identity behind a presence flag; the zero digest names
/// no package, so each owner refuses it with its own reason.
macro_rules! optional_packages {
    ($($marker:ident: $empty:literal;)*) => {$(
        pub(super) struct $marker;
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
