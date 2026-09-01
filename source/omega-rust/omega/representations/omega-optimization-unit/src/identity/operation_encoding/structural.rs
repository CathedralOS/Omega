//! Structural establishment and write-only storage tags.

use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
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
        O::EstablishPayloadlessCase {
            psi_operation,
            result,
            result_case,
        } => {
            bytes.u8(48);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*result_case);
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
        _ => unreachable!("operation family routing admitted a non-structural operation"),
    }
}
