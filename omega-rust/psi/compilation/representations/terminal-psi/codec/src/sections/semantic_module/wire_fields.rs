//! Wire spellings of the recorded fields an operation row carries.
//!
//! Each marker names one field spelling; `operation_wire!` in `block_wire` pairs
//! an operation's fields with these markers once, and both directions read
//! that pairing. Decoding labels name the identity type, which is all a zero
//! identity error reports.

use super::CodecError;
use super::contract_wire::{decode_crash_routes, encode_crash_routes};
use super::proof_declaration_wire::{decode_evidence_interface, encode_evidence_interface};
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
    CanonicalStructuralPathSegment, IeeeFloatComparisonOperation, IeeeFloatValue, IntegerValue,
    ProofTerm, PsiSemanticId, ScalarTerm, StructuralFieldId,
};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequencePredicate, ClaimTransfer, CompletionReceipt, CrashRouteBucket, ElementViewRegion,
    OutcomeSpecificCallArgumentSubstitution, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEvidenceUse, OutcomeSpecificGuard, RecordFieldInitializer, RecordFieldValue,
    ResidentReferenceSource, ScalarCaseField, StructuralArgument, StructuralPathSegment,
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
delegated_fields! {
    Arguments: Vec<StructuralArgument> => encode_structural_arguments, decode_structural_arguments;
    ScalarTerms: Vec<ScalarTerm> => encode_scalar_terms, decode_scalar_terms;
    ProofTerms: Vec<ProofTerm> => encode_proof_terms, decode_proof_terms;
    CrashRoutes: Vec<CrashRouteBucket> => encode_crash_routes, decode_crash_routes;
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

/// Outcome-specific selections of the structural call encodings.
pub(super) struct SelectedEvidence;
impl Field<Vec<OutcomeSpecificCallEvidence>> for SelectedEvidence {
    fn write(
        writer: &mut Writer,
        value: &Vec<OutcomeSpecificCallEvidence>,
    ) -> Result<(), CodecError> {
        writer.len("guarded call selected evidence", value.len())?;
        for binding in value {
            writer.id(binding.guard.result_type);
            writer.id(binding.guard.result_case);
            writer.u32(binding.position);
            writer.id(binding.callee_obligation);
            writer.id(binding.callee_term);
            writer.string("guarded call output field", &binding.output_field)?;
            writer.id(binding.callee_proposition);
            writer.id(binding.instantiated_proposition);
            writer.id(binding.output);
            match &binding.result_substitution {
                None => writer.u8(0),
                Some(substitution) => {
                    writer.u8(1);
                    writer.u32(substitution.argument_position);
                    writer.id(substitution.callee_result);
                    writer.id(substitution.caller_result);
                }
            }
            writer.len(
                "guarded call argument substitutions",
                binding.argument_substitutions.len(),
            )?;
            for substitution in &binding.argument_substitutions {
                writer.u32(substitution.argument_position);
                writer.u32(substitution.parameter_position);
                match &substitution.structural_path {
                    None => writer.u8(0),
                    Some(path) => {
                        writer.u8(1);
                        encode_structural_path(writer, "guarded call argument projection", path)?;
                    }
                }
            }
            writer.id(binding.validity.result);
            writer.len(
                "guarded call proposition dependencies",
                binding.validity.proposition_dependencies.len(),
            )?;
            for dependency in &binding.validity.proposition_dependencies {
                writer.id(*dependency);
            }
            encode_evidence_interface(writer, &binding.validity.evidence_interface)?;
            writer.len(
                "guarded call interface dependencies",
                binding.validity.interface_dependencies.len(),
            )?;
            for dependency in &binding.validity.interface_dependencies {
                writer.id(*dependency);
            }
            writer.u32(binding.expected_use_count);
            writer.len("guarded selected evidence uses", binding.uses.len())?;
            for use_ in &binding.uses {
                writer.id(use_.target);
                writer.u32(use_.input_position);
                writer.id(use_.target_requirement);
                writer.id(use_.target_term);
                writer.id(use_.source);
                writer.id(use_.instantiated_proposition);
                writer.id(use_.target_parameter);
                writer.id(use_.caller_result);
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<OutcomeSpecificCallEvidence>, CodecError> {
        decode_counted(reader, |reader| {
            Ok(OutcomeSpecificCallEvidence {
                guard: OutcomeSpecificGuard {
                    result_type: reader.id("StructuralTypeId")?,
                    result_case: reader.id("StructuralCaseId")?,
                },
                position: reader.u32()?,
                callee_obligation: reader.id("ObligationId")?,
                callee_term: reader.id("EvidenceTermId")?,
                output_field: reader.string("guarded call output field")?,
                callee_proposition: reader.id("PropositionId")?,
                instantiated_proposition: reader.id("PropositionId")?,
                output: reader.id("EvidenceTermId")?,
                result_substitution: match reader.u8()? {
                    0 => None,
                    1 => Some(OutcomeSpecificCallResultSubstitution {
                        argument_position: reader.u32()?,
                        callee_result: reader.id("PlaceId")?,
                        caller_result: reader.id("PlaceId")?,
                    }),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "OutcomeSpecificCallResultSubstitution",
                            tag,
                        ));
                    }
                },
                argument_substitutions: decode_counted(reader, |reader| {
                    Ok(OutcomeSpecificCallArgumentSubstitution {
                        argument_position: reader.u32()?,
                        parameter_position: reader.u32()?,
                        structural_path: match reader.u8()? {
                            0 => None,
                            1 => Some(decode_structural_path(reader)?),
                            tag => {
                                return Err(CodecError::InvalidTag(
                                    "OutcomeSpecificCallArgumentSubstitution",
                                    tag,
                                ));
                            }
                        },
                    })
                })?,
                validity: OutcomeSpecificCallEvidenceValidity {
                    result: reader.id("PlaceId")?,
                    proposition_dependencies: decode_ids(reader, "PlaceId")?,
                    evidence_interface: decode_evidence_interface(reader)?,
                    interface_dependencies: decode_ids(reader, "PlaceId")?,
                },
                expected_use_count: reader.u32()?,
                uses: decode_counted(reader, |reader| {
                    Ok(OutcomeSpecificEvidenceUse {
                        target: reader.id("MachineId")?,
                        input_position: reader.u32()?,
                        target_requirement: reader.id("PropositionId")?,
                        target_term: reader.id("EvidenceTermId")?,
                        source: reader.id("EvidenceTermId")?,
                        instantiated_proposition: reader.id("PropositionId")?,
                        target_parameter: reader.id("PlaceId")?,
                        caller_result: reader.id("PlaceId")?,
                    })
                })?,
            })
        })
    }
}
