//! Structural establishment and write-only storage tags.

use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(65);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            encode_abstract_result(bytes, *value);
        }
        O::PrimitiveLocalStore {
            psi_operation,
            destination,
            value,
        } => {
            bytes.u8(66);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            encode_abstract_result(bytes, *value);
        }
        O::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
        } => {
            bytes.u8(67);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
        }
        O::ByteSequenceSubslice {
            psi_operation,
            result,
            source,
            start,
            end,
            length,
            obligation,
        } => {
            bytes.u8(64);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*source);
            bytes.id(*start);
            bytes.id(*end);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            value,
        } => {
            bytes.u8(49);
            bytes.id(*psi_operation);
            encode_structural_parameter(bytes, destination);
            encode_abstract_result(bytes, *value);
        }
        O::StructuralScalarFieldStore {
            psi_operation,
            destination,
            path,
            field,
            value,
        } => {
            bytes.u8(50);
            bytes.id(*psi_operation);
            encode_structural_parameter(bytes, destination);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            encode_abstract_result(bytes, *value);
        }
        O::EstablishScalarCase {
            psi_operation,
            result,
            result_case,
            fields,
        } => {
            bytes.u8(48);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*result_case);
            bytes.slice(fields, |bytes, field| {
                bytes.id(field.field);
                bytes.id(field.value);
                match field.range_obligation {
                    Some(obligation) => {
                        bytes.u8(1);
                        bytes.id(obligation);
                    }
                    None => bytes.u8(0),
                }
            });
        }
        O::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes: literal,
        } => {
            bytes.u8(1);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
            bytes.len(literal.len());
            bytes.bytes(literal);
        }
        O::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } => {
            bytes.u8(2);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
        }
        O::EstablishAffineScalarRecord {
            psi_operation,
            result,
            field,
            value,
        } => {
            bytes.u8(56);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*field);
            encode_integer_value(bytes, *value);
        }
        _ => unreachable!("operation family routing admitted a non-structural operation"),
    }
}
