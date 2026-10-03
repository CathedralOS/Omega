//! Operations that establish a value on the wire: scalar arrays, cases and
//! records, byte-sequence literals, trivial affine and primitive locals, and
//! the integer, Boolean and IEEE float constants.

use super::super::CodecError;
use super::super::scalar_wire::{
    decode_ieee_float_value, decode_integer_value, encode_ieee_float_value, encode_integer_value,
};
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use crate::sections::semantic_module::wire::{
    decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};
use semantic_vocabulary::{
    IeeeFloatValue, IntegerValue, PlaceId, StructuralCaseId, StructuralDomainId, ValueId,
};
use terminal_psi::OperationKind;

pub(super) fn encode_establish_primitive_local(
    writer: &mut Writer,
    value: ValueId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_PRIMITIVE_LOCAL);
    writer.id(value);
    Ok(())
}

pub(super) fn decode_establish_primitive_local(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishPrimitiveLocal {
        value: reader.id("ValueId")?,
    })
}

pub(super) fn encode_establish_scalar_array(
    writer: &mut Writer,
    elements: Vec<ValueId>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_SCALAR_ARRAY);
    writer.len("scalar array elements", elements.len())?;
    for element in elements {
        writer.id(element);
    }
    Ok(())
}

pub(super) fn decode_establish_scalar_array(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishScalarArray {
        elements: decode_counted(reader, |reader| reader.id("ValueId"))?,
    })
}

pub(super) fn encode_establish_scalar_case(
    writer: &mut Writer,
    result_case: StructuralCaseId,
    fields: Vec<terminal_psi::ScalarCaseField>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_SCALAR_CASE);
    writer.id(result_case);
    writer.len("scalar case fields", fields.len())?;
    for field in fields {
        writer.id(field.field);
        writer.id(field.value);
        encode_optional_id(writer, field.range_obligation);
    }
    Ok(())
}

pub(super) fn decode_establish_scalar_case(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishScalarCase {
        result_case: reader.id("StructuralCaseId")?,
        fields: decode_counted(reader, |reader| {
            Ok(terminal_psi::ScalarCaseField {
                field: reader.id("StructuralFieldId")?,
                value: reader.id("ValueId")?,
                range_obligation: decode_optional_id(reader, "ObligationId")?,
            })
        })?,
    })
}

pub(super) fn encode_establish_byte_sequence_literal(
    writer: &mut Writer,
    destination: PlaceId,
    bytes: Vec<u8>,
    qualifications: &[StructuralDomainId],
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_BYTE_SEQUENCE_LITERAL);
    writer.id(destination);
    writer.len("byte-sequence literal bytes", bytes.len())?;
    writer.bytes(&bytes);
    writer.len("literal qualifications", qualifications.len())?;
    for qualification in qualifications {
        writer.id(*qualification);
    }
    Ok(())
}

pub(super) fn decode_establish_byte_sequence_literal(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishByteSequenceLiteral {
        destination: reader.id("PlaceId")?,
        bytes: {
            let len = usize::try_from(reader.count()?).map_err(|_| CodecError::UnexpectedEnd)?;
            reader.take(len)?.to_vec()
        },
        qualifications: decode_ids(reader, "StructuralDomainId")?,
    })
}

pub(super) fn encode_establish_zeroed_structural(writer: &mut Writer) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_ZEROED_STRUCTURAL);
    Ok(())
}

pub(super) fn decode_establish_zeroed_structural(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let _ = reader;
    Ok(OperationKind::EstablishZeroedStructural {})
}

pub(super) fn encode_establish_structural_local(writer: &mut Writer) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_STRUCTURAL_LOCAL);
    Ok(())
}

pub(super) fn decode_establish_structural_local(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let _ = reader;
    Ok(OperationKind::EstablishStructuralLocal {})
}

pub(super) fn encode_establish_trivial_affine_local(
    writer: &mut Writer,
    destination: PlaceId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_TRIVIAL_AFFINE_LOCAL);
    writer.id(destination);
    Ok(())
}

pub(super) fn decode_establish_trivial_affine_local(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishTrivialAffineLocal {
        destination: reader.id("PlaceId")?,
    })
}

pub(super) fn encode_establish_structural_case(
    writer: &mut Writer,
    result_case: StructuralCaseId,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_STRUCTURAL_CASE);
    writer.id(result_case);
    encode_record_field_initializers(writer, fields)
}

pub(super) fn decode_establish_structural_case(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishStructuralCase {
        result_case: reader.id("StructuralCaseId")?,
        fields: decode_record_field_initializers(reader)?,
    })
}

pub(super) fn encode_establish_record(
    writer: &mut Writer,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_RECORD);
    encode_record_field_initializers(writer, fields)
}

fn encode_record_field_initializers(
    writer: &mut Writer,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
) -> Result<(), CodecError> {
    writer.len("record fields", fields.len())?;
    for field in fields {
        writer.id(field.field);
        match field.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                writer.u8(1);
                writer.id(value);
                encode_optional_id(writer, range_obligation);
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                writer.u8(2);
                writer.id(argument.place);
                super::super::structural_signature_wire::encode_structural_access(
                    writer,
                    argument.access,
                );
                encode_structural_path(writer, "record child path", &argument.path)?;
            }
        }
    }
    Ok(())
}

pub(super) fn decode_establish_record(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishRecord {
        fields: decode_record_field_initializers(reader)?,
    })
}

fn decode_record_field_initializers(
    reader: &mut Reader<'_>,
) -> Result<Vec<terminal_psi::RecordFieldInitializer>, CodecError> {
    decode_counted(reader, |reader| {
        let field = reader.id("StructuralFieldId")?;
        let value = match reader.u8()? {
            1 => terminal_psi::RecordFieldValue::Scalar {
                value: reader.id("ValueId")?,
                range_obligation: decode_optional_id(reader, "ObligationId")?,
            },
            2 => terminal_psi::RecordFieldValue::Structural(terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            }),
            tag => return Err(CodecError::InvalidTag("RecordFieldValue", tag)),
        };
        Ok(terminal_psi::RecordFieldInitializer { field, value })
    })
}

pub(super) fn encode_integer_constant(
    writer: &mut Writer,
    value: IntegerValue,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_CONSTANT);
    encode_integer_value(writer, value);
    Ok(())
}

pub(super) fn decode_integer_constant(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerConstant {
        value: decode_integer_value(reader)?,
    })
}

pub(super) fn encode_boolean_constant(writer: &mut Writer, value: bool) -> Result<(), CodecError> {
    writer.u8(operation_tags::BOOLEAN_CONSTANT);
    writer.u8(u8::from(value));
    Ok(())
}

pub(super) fn decode_boolean_constant(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::BooleanConstant {
        value: reader.boolean()?,
    })
}

pub(super) fn encode_ieee_float_constant(
    writer: &mut Writer,
    value: IeeeFloatValue,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::IEEE_FLOAT_CONSTANT);
    encode_ieee_float_value(writer, value);
    Ok(())
}

pub(super) fn decode_ieee_float_constant(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IeeeFloatConstant {
        value: decode_ieee_float_value(reader)?,
    })
}

pub(super) fn encode_establish_byte_sequence_concatenation(
    writer: &mut Writer,
    left: terminal_psi::control_flow::ByteSequenceOperand,
    right: terminal_psi::control_flow::ByteSequenceOperand,
    capacity_obligation: semantic_vocabulary::ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_BYTE_SEQUENCE_CONCATENATION);
    encode_byte_sequence_operand(writer, left)?;
    encode_byte_sequence_operand(writer, right)?;
    writer.id(capacity_obligation);
    Ok(())
}

pub(super) fn decode_establish_byte_sequence_concatenation(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishByteSequenceConcatenation {
        left: decode_byte_sequence_operand(reader)?,
        right: decode_byte_sequence_operand(reader)?,
        capacity_obligation: reader.id("ObligationId")?,
    })
}

fn encode_byte_sequence_operand(
    writer: &mut Writer,
    operand: terminal_psi::control_flow::ByteSequenceOperand,
) -> Result<(), CodecError> {
    use terminal_psi::control_flow::ByteSequenceOperand;
    match operand {
        ByteSequenceOperand::Whole { source, length } => {
            writer.u8(0);
            writer.id(source);
            writer.id(length);
        }
        ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            writer.u8(1);
            writer.id(source.place);
            super::super::structural_signature_wire::encode_structural_access(
                writer,
                source.access,
            );
            encode_structural_path(writer, "byte sequence operand path", &source.path)?;
            writer.id(field);
            writer.id(length);
        }
    }
    Ok(())
}

fn decode_byte_sequence_operand(
    reader: &mut Reader<'_>,
) -> Result<terminal_psi::control_flow::ByteSequenceOperand, CodecError> {
    use terminal_psi::control_flow::ByteSequenceOperand;
    match reader.u8()? {
        0 => Ok(ByteSequenceOperand::Whole {
            source: reader.id("PlaceId")?,
            length: reader.id("ValueId")?,
        }),
        1 => Ok(ByteSequenceOperand::Field {
            source: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            field: reader.id("StructuralFieldId")?,
            length: reader.id("ValueId")?,
        }),
        tag => Err(CodecError::InvalidTag("byte sequence operand", tag)),
    }
}

pub(super) fn encode_byte_sequence_equal(
    writer: &mut Writer,
    left: terminal_psi::control_flow::ByteSequenceOperand,
    right: terminal_psi::control_flow::ByteSequenceOperand,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_EQUAL);
    encode_byte_sequence_operand(writer, left)?;
    encode_byte_sequence_operand(writer, right)?;
    Ok(())
}

pub(super) fn decode_byte_sequence_equal(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceEqual {
        left: decode_byte_sequence_operand(reader)?,
        right: decode_byte_sequence_operand(reader)?,
    })
}

pub(super) fn encode_byte_sequence_predicate(
    writer: &mut Writer,
    operand: terminal_psi::control_flow::ByteSequenceOperand,
    predicate: terminal_psi::ByteSequencePredicate,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_PREDICATE);
    encode_byte_sequence_operand(writer, operand)?;
    writer.u8(predicate.mask_bit());
    Ok(())
}

pub(super) fn decode_byte_sequence_predicate(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequencePredicate {
        operand: decode_byte_sequence_operand(reader)?,
        predicate: match reader.u8()? {
            1 => terminal_psi::ByteSequencePredicate::ValidUtf8,
            2 => terminal_psi::ByteSequencePredicate::NoNul,
            4 => terminal_psi::ByteSequencePredicate::AsciiOnly,
            8 => terminal_psi::ByteSequencePredicate::NonEmpty,
            tag => return Err(CodecError::InvalidTag("ByteSequencePredicate", tag)),
        },
    })
}

pub(super) fn encode_establish_structural_array(
    writer: &mut Writer,
    elements: Vec<terminal_psi::StructuralArgument>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_STRUCTURAL_ARRAY);
    writer.len("structural array elements", elements.len())?;
    for argument in elements {
        writer.id(argument.place);
        super::super::structural_signature_wire::encode_structural_access(writer, argument.access);
        encode_structural_path(writer, "array element path", &argument.path)?;
    }
    Ok(())
}

pub(super) fn decode_establish_structural_array(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishStructuralArray {
        elements: decode_counted(reader, |reader| {
            Ok(terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            })
        })?,
    })
}
