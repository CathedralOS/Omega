//! Structural establishment and write-only storage tags.

use super::super::{
    encode_abstract_result, encode_canonical_path, encode_place_declaration,
    encode_structural_argument, encode_structural_operation_result, encode_structural_parameter,
    encode_structural_path_segment, encode_structural_type,
};
use super::{AbstractOperation, CanonicalBytes};
pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } => {
            bytes.u8(69);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.slice(elements, |bytes, value| bytes.id(*value));
        }
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
        O::StructuralCaseMembership {
            psi_operation,
            result,
            source,
            path,
            case,
        } => {
            bytes.u8(71);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*case);
        }
        O::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
            path,
        } => {
            // Existing whole-root identities stay stable; projected subjects
            // have a distinct tag and retain every declaration-local step.
            bytes.u8(if path.is_empty() { 67 } else { 73 });
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            if !path.is_empty() {
                encode_canonical_path(bytes, path);
            }
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
            path,
            value,
        } => {
            bytes.u8(if path.is_empty() { 49 } else { 74 });
            bytes.id(*psi_operation);
            encode_structural_parameter(bytes, destination);
            if !path.is_empty() {
                encode_canonical_path(bytes, path);
            }
            encode_abstract_result(bytes, *value);
        }
        O::StructuralScalarFieldStore {
            psi_operation,
            destination,
            path,
            field,
            value,
            range_obligation,
        } => {
            bytes.u8(if range_obligation.is_some() { 75 } else { 50 });
            bytes.id(*psi_operation);
            encode_structural_parameter(bytes, destination);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            encode_abstract_result(bytes, *value);
            if let Some(obligation) = range_obligation {
                bytes.id(*obligation);
            }
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
        O::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => {
            bytes.u8(56);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.slice(fields, |bytes, initializer| {
                bytes.id(initializer.field);
                match &initializer.value {
                    terminal_psi::RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    } => {
                        bytes.u8(0);
                        bytes.id(*value);
                        match range_obligation {
                            Some(obligation) => {
                                bytes.u8(1);
                                bytes.id(*obligation);
                            }
                            None => bytes.u8(0),
                        }
                    }
                    terminal_psi::RecordFieldValue::Structural(argument) => {
                        bytes.u8(1);
                        encode_structural_argument(bytes, argument);
                    }
                }
            });
        }
        O::EstablishReference {
            psi_operation,
            result,
            source,
        } => {
            bytes.u8(79);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            encode_structural_argument(bytes, source);
        }
        O::ReleaseReference {
            psi_operation,
            source,
        } => {
            bytes.u8(80);
            bytes.id(*psi_operation);
            bytes.id(*source);
        }
        _ => unreachable!("operation family routing admitted a non-structural operation"),
    }
}
