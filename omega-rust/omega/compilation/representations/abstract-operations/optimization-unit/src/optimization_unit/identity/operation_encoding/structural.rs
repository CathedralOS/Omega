//! Structural establishment and write-only storage tags.

use crate::optimization_unit::identity::carrier_encoding::{
    encode_abstract_result, encode_ids, encode_structural_argument, encode_structural_parameter,
    encode_structural_path_segment, encode_window_root,
};
use crate::optimization_unit::identity::structural_encoding::{
    encode_place_declaration, encode_structural_operation_result, encode_structural_type,
};

use super::{AbstractOperation, CanonicalBytes};
pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::EstablishByteSequenceConcatenation {
            psi_operation,
            result,
            left,
            right,
            capacity_obligation,
        } => {
            bytes.u8(92);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            for operand in [left, right] {
                match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } => {
                        bytes.u8(0);
                        bytes.id(*source);
                        bytes.id(*length);
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field {
                        source,
                        field,
                        length,
                    } => {
                        bytes.u8(1);
                        encode_structural_argument(bytes, source);
                        bytes.id(*field);
                        bytes.id(*length);
                    }
                }
            }
            bytes.id(*capacity_obligation);
        }
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
            // have a distinct tag and retain every spelled step, including
            // each runtime element's selector and obligation.
            bytes.u8(if path.is_empty() { 67 } else { 73 });
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            if !path.is_empty() {
                bytes.slice(path, encode_structural_path_segment);
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
        O::ElementViewSubslice {
            psi_operation,
            result,
            source,
            start,
            end,
            length,
            obligation,
        } => {
            bytes.u8(84);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*source);
            bytes.id(*start);
            bytes.id(*end);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::EstablishElementView {
            psi_operation,
            result,
            destination,
            source,
            element,
            region,
        } => {
            bytes.u8(85);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*destination);
            encode_structural_argument(bytes, source);
            bytes.id(*element);
            match region {
                terminal_psi::ElementViewRegion::WholeCollection => bytes.u8(0),
                terminal_psi::ElementViewRegion::RepresentationBytes {
                    start,
                    end,
                    obligation,
                } => {
                    bytes.u8(1);
                    bytes.id(*start);
                    bytes.id(*end);
                    bytes.id(*obligation);
                }
            }
        }
        O::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            path,
            value,
        } => {
            bytes.u8(if path.is_empty() { 49 } else { 74 });
            bytes.id(*psi_operation);
            encode_store_destination(bytes, destination);
            if !path.is_empty() {
                bytes.slice(path, encode_structural_path_segment);
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
            encode_store_destination(bytes, destination);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            encode_abstract_result(bytes, *value);
            if let Some(obligation) = range_obligation {
                bytes.id(*obligation);
            }
        }
        O::EstablishCase {
            psi_operation,
            result,
            result_case,
            fields,
        } => {
            bytes.u8(48);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*result_case);
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
        O::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes: literal,
            qualifications,
        } => {
            bytes.u8(1);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
            bytes.len(literal.len());
            bytes.bytes(literal);
            encode_ids(bytes, qualifications);
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
        O::EstablishZeroedStructural {
            psi_operation,
            result,
        } => {
            bytes.u8(90);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
        }
        O::EstablishStructuralLocal {
            psi_operation,
            result,
        } => {
            bytes.u8(103);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
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
        O::MoveStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            bytes.u8(81);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            encode_window_root(bytes, source);
            bytes.slice(path, encode_structural_path_segment);
            encode_window_field(bytes, *field);
        }
        O::StoreStructuralField {
            psi_operation,
            destination,
            path,
            field,
            value,
        } => {
            bytes.u8(82);
            bytes.id(*psi_operation);
            encode_window_root(bytes, destination);
            bytes.slice(path, encode_structural_path_segment);
            encode_window_field(bytes, *field);
            encode_structural_argument(bytes, value);
        }
        O::StructuralLeafCopy {
            psi_operation,
            result,
            source,
            path,
        } => {
            bytes.u8(match path {
                abstract_operations::StructuralCopyPath::Ordinary(_) => 88,
                abstract_operations::StructuralCopyPath::Case(_) => 91,
            });
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*source);
            match path {
                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                    bytes.slice(path, encode_structural_path_segment);
                }
                abstract_operations::StructuralCopyPath::Case(path) => {
                    crate::optimization_unit::identity::proposition_encoding::encode_canonical_path(
                        bytes, path,
                    );
                }
            }
        }
        _ => unreachable!("operation family routing admitted a non-structural operation"),
    }
}

/// A window's field identity, or zero for a root window: no declared field
/// takes identity zero, so every field window keeps its previous spelling.
/// Both scalar store forms share one destination encoding, so a rewrite
/// that substitutes an owned block home for a loan, or one block's home for
/// another's, changes the canonical identity.
fn encode_store_destination(
    bytes: &mut CanonicalBytes,
    destination: &abstract_operations::StructuralScalarStoreDestination,
) {
    use abstract_operations::StructuralScalarStoreDestination as Destination;
    match destination {
        Destination::Parameter(parameter) => {
            bytes.u8(0);
            encode_structural_parameter(bytes, parameter);
        }
        Destination::OwnedResult { producer, result } => {
            bytes.u8(1);
            bytes.id(*producer);
            encode_structural_operation_result(bytes, result);
        }
        Destination::OwnedBlockParameter { block, parameter } => {
            bytes.u8(2);
            bytes.id(*block);
            encode_structural_parameter(bytes, parameter);
        }
    }
}

fn encode_window_field(
    bytes: &mut CanonicalBytes,
    field: Option<semantic_vocabulary::StructuralFieldId>,
) {
    bytes.u64(field.map_or(0, semantic_vocabulary::StructuralFieldId::get));
}
