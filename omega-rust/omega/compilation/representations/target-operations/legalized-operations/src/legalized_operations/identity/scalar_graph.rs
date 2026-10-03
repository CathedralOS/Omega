use super::calling::{encode_call_plan, encode_placement};
use super::encoding::{encode_fuel, encode_ids, encode_len, encode_option_id};
use super::scalar::{encode_bindings, encode_integer, encode_integer_type, encode_scalar_type};
use super::structural::{encode_effect, encode_ownership_roster};
use crate::legalized_operations::{
    LegalizedExactIntegerOperator, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstructionKind, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator,
};
use crate::{SaturatingCarrier, SaturatingOperation};
use optimization_unit::encode_value_definition_site_identity;
pub(super) fn encode(bytes: &mut Vec<u8>, function: &LegalizedScalarFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(bytes, function.attachment.map(|value| value.get()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    encode_call_plan(bytes, &function.call_plan);
    match &function.structural {
        Some(signature) => {
            bytes.push(1);
            super::plan::encode_structural_contract(bytes, signature);
        }
        None => bytes.push(0),
    }

    encode_len(bytes, function.parameters.len());
    for parameter in &function.parameters {
        bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
        encode_scalar_type(bytes, parameter.scalar_type);
        encode_value_definition_site_identity(bytes, parameter.definition_site);
        encode_placement(bytes, &parameter.placement);
    }
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        bytes.extend_from_slice(&block.id.get().to_le_bytes());
        encode_len(bytes, block.structural_parameters.len());
        for parameter in &block.structural_parameters {
            super::structural_types::encode_structural_parameter(bytes, parameter);
        }
        encode_len(bytes, block.parameters.len());
        for parameter in &block.parameters {
            bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
            encode_value_definition_site_identity(bytes, parameter.site);
        }
        encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            bytes.extend_from_slice(&instruction.operation.get().to_le_bytes());
            match instruction.result {
                Some(result) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&result.value.get().to_le_bytes());
                    encode_scalar_type(bytes, result.scalar_type);
                    encode_value_definition_site_identity(bytes, result.definition_site);
                }
                None => bytes.push(0),
            }
            match &instruction.kind {
                LegalizedScalarInstructionKind::FloatResultPolicy { format, policy } => {
                    bytes.push(170);
                    encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(*format));
                    bytes.push(match policy {
                        terminal_psi::FloatResultPolicy::Trapping { .. } => 0,
                        terminal_psi::FloatResultPolicy::Saturating { rule, .. } => match rule {
                            terminal_psi::FloatSaturationRule::FiniteOperandOverflow => 1,
                            terminal_psi::FloatSaturationRule::FiniteDivisionOverflow => 2,
                        },
                    });
                    for operand in policy.operands() {
                        bytes.extend_from_slice(&operand.get().to_le_bytes());
                    }
                }
                LegalizedScalarInstructionKind::AtomicAccess {
                    location,
                    field,
                    scalar_type,
                    field_byte_offset,
                    event,
                } => {
                    // 95..166 are the existing trapping forms.
                    bytes.push(167);
                    super::structural_types::encode_structural_argument(bytes, location);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                    encode_integer_type(bytes, *scalar_type);
                    bytes.extend_from_slice(&field_byte_offset.to_le_bytes());
                    encode_atomic_event(bytes, *event);
                }
                LegalizedScalarInstructionKind::IeeeFloatCompare {
                    comparison,
                    format,
                    left,
                    right,
                } => {
                    bytes.push(23);
                    bytes.push(match comparison {
                        semantic_vocabulary::IeeeFloatComparisonOperation::Equal => 0,
                        semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual => 1,
                        semantic_vocabulary::IeeeFloatComparisonOperation::Less => 2,
                        semantic_vocabulary::IeeeFloatComparisonOperation::LessOrEqual => 3,
                        semantic_vocabulary::IeeeFloatComparisonOperation::Greater => 4,
                        semantic_vocabulary::IeeeFloatComparisonOperation::GreaterOrEqual => 5,
                    });
                    encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(*format));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
                    boundary,
                    operation,
                    format,
                    arguments,
                } => {
                    bytes.push(TAG_NEAREST_IEEE_FLOAT_ARITHMETIC);
                    bytes.extend_from_slice(&boundary.get().to_le_bytes());
                    bytes.push(operation.ordinal());
                    encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(*format));
                    bytes.extend_from_slice(&(arguments.len() as u64).to_le_bytes());
                    for argument in arguments {
                        bytes.extend_from_slice(&argument.get().to_le_bytes());
                    }
                }
                LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                    result,
                    left,
                    right,
                    capacity_obligation,
                    accepted_fact,
                } => {
                    bytes.push(240);
                    super::structural_result::encode_operation_result(bytes, result);
                    encode_byte_operand(bytes, left);
                    encode_byte_operand(bytes, right);
                    bytes.extend_from_slice(&capacity_obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::EstablishScalarArray {
                    result,
                    elements,
                    shape,
                } => {
                    bytes.push(22);
                    super::structural_result::encode_operation_result(bytes, result);
                    encode_len(bytes, elements.len());
                    for element in elements {
                        bytes.extend_from_slice(&element.get().to_le_bytes());
                    }
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::EstablishRecord {
                    result,
                    fields,
                    shape,
                } => {
                    bytes.push(34);
                    super::structural_result::encode_operation_result(bytes, result);
                    encode_len(bytes, fields.len());
                    for field in fields {
                        bytes.extend_from_slice(&field.field.get().to_le_bytes());
                        match &field.value {
                            terminal_psi::RecordFieldValue::Scalar {
                                value,
                                range_obligation,
                            } => {
                                bytes.push(0);
                                bytes.extend_from_slice(&value.get().to_le_bytes());
                                encode_option_id(bytes, range_obligation.map(|id| id.get()));
                            }
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                bytes.push(1);
                                super::structural_types::encode_structural_argument(
                                    bytes, argument,
                                );
                            }
                        }
                    }
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::EstablishCase {
                    result,
                    result_case,
                    fields,
                    layout,
                } => {
                    bytes.push(21);
                    super::structural_result::encode_operation_result(bytes, result);
                    bytes.extend_from_slice(&result_case.get().to_le_bytes());
                    encode_len(bytes, fields.len());
                    for field in fields {
                        bytes.extend_from_slice(&field.field.get().to_le_bytes());
                        match &field.value {
                            terminal_psi::RecordFieldValue::Scalar {
                                value,
                                range_obligation,
                            } => {
                                bytes.push(0);
                                bytes.extend_from_slice(&value.get().to_le_bytes());
                                encode_option_id(bytes, range_obligation.map(|id| id.get()));
                            }
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                bytes.push(1);
                                super::structural_types::encode_structural_argument(
                                    bytes, argument,
                                );
                            }
                        }
                    }
                    super::read_byte::encode_layout(bytes, layout);
                }
                LegalizedScalarInstructionKind::EstablishReference {
                    result,
                    source,
                    shape,
                } => {
                    bytes.push(29);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::structural_types::encode_structural_argument(bytes, source);
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::ReleaseReference { source } => {
                    bytes.push(30);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                    result,
                    value,
                    shape,
                } => {
                    bytes.push(16);
                    super::structural_result::encode_operation_result(bytes, result);
                    bytes.extend_from_slice(&value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
                    bytes.push(17);
                    bytes.extend_from_slice(&destination.get().to_le_bytes());
                    bytes.extend_from_slice(&value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                }
                LegalizedScalarInstructionKind::StructuralCaseMembership {
                    source,
                    path,
                    case,
                    case_tag,
                    tag_byte_offset,
                    indices,
                } => {
                    bytes.push(24);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                    bytes.extend_from_slice(&case.get().to_le_bytes());
                    bytes.extend_from_slice(&case_tag.to_le_bytes());
                    bytes.extend_from_slice(&tag_byte_offset.to_le_bytes());
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::StructuralLeafCopy {
                    result,
                    source,
                    path,
                    byte_offset,
                    shape,
                    indices,
                } => {
                    bytes.push(match path {
                        abstract_operations::StructuralCopyPath::Ordinary(_) => 86,
                        abstract_operations::StructuralCopyPath::Case(_) => 92,
                    });
                    super::structural_result::encode_operation_result(bytes, result);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    match path {
                        abstract_operations::StructuralCopyPath::Ordinary(path) => {
                            super::structural_types::encode_structural_path(bytes, path);
                        }
                        abstract_operations::StructuralCopyPath::Case(path) => {
                            encode_len(bytes, path.len());
                            for segment in path {
                                use semantic_vocabulary::CanonicalStructuralPathSegment;
                                let (tag, identity) = match segment {
                                    CanonicalStructuralPathSegment::Field(field) => {
                                        (1, field.get())
                                    }
                                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                                        (2, *index)
                                    }
                                    CanonicalStructuralPathSegment::Case(case) => (3, case.get()),
                                };
                                bytes.push(tag);
                                bytes.extend_from_slice(&identity.to_le_bytes());
                            }
                        }
                    }
                    bytes.extend_from_slice(&byte_offset.to_le_bytes());
                    super::calling::encode_shape(bytes, *shape);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    byte_offset,
                    shape,
                    indices,
                } => {
                    bytes.push(88);
                    super::structural_types::encode_window_root(bytes, destination);
                    super::structural_types::encode_structural_path(bytes, path);
                    // A root window spells field zero, which no declared
                    // field identity takes.
                    bytes.extend_from_slice(
                        &field
                            .map_or(0, semantic_vocabulary::StructuralFieldId::get)
                            .to_le_bytes(),
                    );
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    bytes.extend_from_slice(&byte_offset.to_le_bytes());
                    super::calling::encode_shape(bytes, *shape);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape } => {
                    bytes.push(91);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::EstablishStructuralLocal { result, shape } => {
                    bytes.push(93);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                    place,
                    structural_type,
                } => {
                    bytes.push(89);
                    super::structural_types::encode_structural_place(bytes, *place);
                    super::structural_types::encode_structural_type(bytes, structural_type);
                }
                LegalizedScalarInstructionKind::PrimitiveScalarRead {
                    source,
                    path,
                    indices,
                } => {
                    bytes.push(18);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source,
                    case: None,
                    field,
                } => {
                    bytes.push(25);
                    super::structural_types::encode_structural_argument(bytes, source);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                }
                // A case-qualified read takes its own tag, so record reads keep
                // their existing encoding and no two shapes share bytes.
                LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source,
                    case: Some(case),
                    field,
                } => {
                    bytes.push(TAG_CASE_PAYLOAD_FIELD_READ);
                    super::structural_types::encode_structural_argument(bytes, source);
                    bytes.extend_from_slice(&case.get().to_le_bytes());
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                    source,
                    field,
                    index,
                    length,
                    obligation,
                    accepted_fact,
                    indices,
                } => {
                    bytes.push(90);
                    encode_runtime_indices(bytes, indices);
                    super::structural_types::encode_structural_argument(bytes, source);
                    for identity in [field.get(), index.get(), length.get(), obligation.get()] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                    source,
                    field,
                    indices,
                } => {
                    bytes.push(27);
                    encode_runtime_indices(bytes, indices);
                    super::structural_types::encode_structural_argument(bytes, source);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(39);
                    super::structural_types::encode_structural_argument(bytes, destination);
                    for identity in [
                        field.get(),
                        index.get(),
                        value.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceStore {
                    destination,
                    source,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(241);
                    bytes.extend_from_slice(&destination.get().to_le_bytes());
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&length.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                    destination,
                    field,
                    source,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(28);
                    super::structural_types::encode_structural_argument(bytes, destination);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&length.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                    destination,
                    field,
                    source,
                    indices,
                } => {
                    bytes.push(92);
                    encode_runtime_indices(bytes, indices);
                    super::structural_types::encode_structural_argument(bytes, destination);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::HostedReadByte {
                    boundary,
                    result,
                    layout,
                } => {
                    bytes.push(15);
                    super::read_byte::encode_payload(bytes, *boundary, result, layout);
                }
                LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    destination,
                    path,
                    value,
                    byte_offset,
                    byte_size,
                    indices,
                } => {
                    bytes.push(13);
                    encode_store_destination(bytes, destination);
                    super::structural_types::encode_structural_path(bytes, path);
                    bytes.extend_from_slice(&byte_offset.to_le_bytes());
                    bytes.extend_from_slice(&value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    bytes.push(*byte_size);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::HostedExitProcessI32 { boundary, source } => {
                    bytes.push(14);
                    bytes.extend_from_slice(&boundary.get().to_le_bytes());
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source } => {
                    bytes.push(11);
                    bytes.extend_from_slice(&boundary.get().to_le_bytes());
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    value,
                    byte_offset,
                    byte_size,
                    indices,
                } => {
                    bytes.push(12);
                    encode_store_destination(bytes, destination);
                    super::structural_types::encode_structural_path(bytes, path);
                    bytes.extend_from_slice(&field.get().to_le_bytes());
                    bytes.extend_from_slice(&value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    bytes.extend_from_slice(&byte_offset.to_le_bytes());
                    bytes.push(*byte_size);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                    destination,
                    structural_type,
                    bytes: literal_bytes,
                } => {
                    bytes.push(10);
                    super::structural_types::encode_structural_place(bytes, *destination);
                    super::structural_types::encode_structural_type(bytes, structural_type);
                    encode_len(bytes, literal_bytes.len());
                    bytes.extend_from_slice(literal_bytes);
                }
                LegalizedScalarInstructionKind::ByteSequenceSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(9);
                    super::structural_result::encode_operation_result(bytes, result);
                    for identity in [
                        source.get(),
                        start.get(),
                        end.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceWrite {
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(19);
                    for identity in [
                        destination.get(),
                        index.get(),
                        value.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceRead {
                    source,
                    index,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(8);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&index.get().to_le_bytes());
                    bytes.extend_from_slice(&length.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceLength {
                    source,
                    length_byte_offset,
                } => {
                    bytes.push(7);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&length_byte_offset.to_le_bytes());
                }
                LegalizedScalarInstructionKind::ByteSequencePredicate { operand, predicate } => {
                    bytes.push(45);
                    encode_byte_operand(bytes, operand);
                    bytes.push(predicate.mask_bit());
                }
                LegalizedScalarInstructionKind::EstablishElementView {
                    result,
                    destination,
                    source,
                    element,
                } => {
                    bytes.push(40);
                    super::structural_result::encode_operation_result(bytes, result);
                    bytes.extend_from_slice(&destination.get().to_le_bytes());
                    super::structural_types::encode_structural_argument(bytes, source);
                    bytes.extend_from_slice(&element.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::ElementViewWrite {
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(44);
                    for identity in [
                        destination.get(),
                        index.get(),
                        value.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ElementViewSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(41);
                    super::structural_result::encode_operation_result(bytes, result);
                    for identity in [
                        source.get(),
                        start.get(),
                        end.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ElementViewRead {
                    source,
                    index,
                    length,
                    obligation,
                    accepted_fact,
                    path,
                } => {
                    bytes.push(42);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&index.get().to_le_bytes());
                    bytes.extend_from_slice(&length.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                }
                LegalizedScalarInstructionKind::ElementViewLength {
                    source,
                    length_byte_offset,
                } => {
                    bytes.push(43);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&length_byte_offset.to_le_bytes());
                }
                LegalizedScalarInstructionKind::BooleanNot { operand } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } => {
                    bytes.push(5);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                    encode_integer_type(bytes, *source_type);
                }
                LegalizedScalarInstructionKind::ScalarRepresentationView {
                    operand,
                    source_type,
                } => {
                    bytes.push(171);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                    encode_scalar_type(bytes, *source_type);
                }
                LegalizedScalarInstructionKind::IntegerExactCast {
                    operand,
                    source_type,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(20);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                    encode_integer_type(bytes, *source_type);
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::Constant(value) => {
                    bytes.push(0);
                    encode_integer(bytes, *value);
                }
                LegalizedScalarInstructionKind::Call(call) => {
                    bytes.push(1);
                    super::ordinary_calls::encode_call(bytes, call);
                }
                LegalizedScalarInstructionKind::BoundarySettlement(settlement) => {
                    bytes.push(6);
                    super::structural::encode_boundary_settlement(bytes, settlement);
                }
                LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
                    bytes.push(67);
                    super::normalized_foreign::encode(bytes, call);
                }
                LegalizedScalarInstructionKind::DynamicParameterCall(call) => {
                    bytes.push(73);
                    super::dynamic_calls::encode_dynamic_parameter_call(bytes, call);
                }
                LegalizedScalarInstructionKind::SaturatingAdd {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Add, *carrier));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingSubtract {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Subtract, *carrier));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingDivide {
                    carrier,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Divide, *carrier));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::SaturatingMultiply {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Multiply, *carrier));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::TrappingBinary { form, left, right } => {
                    bytes.push(trapping_tag(*form));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::TrappingConvert {
                    form,
                    source,
                    operand,
                } => {
                    bytes.push(trapping_tag(*form));
                    bytes.push(source.ordinal());
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingRemainder {
                    carrier,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Remainder, *carrier));
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingRemainder {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(37);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingDivide {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(70);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingAdd { left, right } => {
                    bytes.push(38);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingSubtract { left, right } => {
                    bytes.push(68);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingMultiply { left, right } => {
                    bytes.push(69);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingShiftLeft { value, count } => {
                    bytes.push(81);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    bytes.extend_from_slice(&count.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingShiftRight { value, count } => {
                    bytes.push(82);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    bytes.extend_from_slice(&count.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::ExactShiftLeft {
                    value,
                    count,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(83);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    bytes.extend_from_slice(&count.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ExactShiftRight {
                    value,
                    count,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(84);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    bytes.extend_from_slice(&count.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ExactBinary {
                    operator,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(2);
                    bytes.push(match operator {
                        LegalizedExactIntegerOperator::Add => 0,
                        LegalizedExactIntegerOperator::Subtract => 1,
                        LegalizedExactIntegerOperator::Divide => 2,
                        LegalizedExactIntegerOperator::Multiply => 3,
                        LegalizedExactIntegerOperator::Remainder => 4,
                    });
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::BitwiseAnd { left, right } => {
                    bytes.push(24);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseOr { left, right } => {
                    bytes.push(71);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
                    bytes.push(26);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseNot { operand } => {
                    bytes.push(72);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    right_operand_type,
                    left,
                    right,
                } => {
                    bytes.push(3);
                    bytes.push(match predicate {
                        LegalizedScalarComparison::Equal => 0,
                        LegalizedScalarComparison::LessThan => 1,
                        LegalizedScalarComparison::LessOrEqual => 2,
                    });
                    encode_scalar_type(bytes, *operand_type);
                    encode_scalar_type(bytes, *right_operand_type);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
            }
            encode_fuel(bytes, &instruction.fuel);
            encode_effect(bytes, instruction.effect);
            encode_ownership_roster(bytes, &instruction.ownership);
        }
        encode_terminator(bytes, &block.terminator);
    }
    // The retained frontier catalog binds by its canonical fact identities:
    // each identity already names the verifier artifact, machine, site, and
    // snapshot the row spells, so the bytes the roster replay binds are the
    // same tokens the source unit retains.
    encode_len(bytes, function.ownership_frontier_facts.len());
    for fact in &function.ownership_frontier_facts {
        bytes.extend_from_slice(&fact.identity.bytes());
    }
}

fn encode_atomic_event(bytes: &mut Vec<u8>, event: terminal_psi::AtomicAccessEvent) {
    use terminal_psi::{
        AtomicAccessEvent as Event, AtomicReadModifyWrite as Fetch, MemoryOrdering,
    };
    fn ordering(bytes: &mut Vec<u8>, value: MemoryOrdering) {
        bytes.push(match value {
            MemoryOrdering::NoOrdering => 0,
            MemoryOrdering::Receive => 1,
            MemoryOrdering::Publish => 2,
            MemoryOrdering::ReceivePublish => 3,
            MemoryOrdering::GlobalOrder => 4,
        });
    }
    match event {
        Event::Load { ordering: order } => {
            bytes.push(0);
            ordering(bytes, order);
        }
        Event::Store {
            ordering: order, ..
        } => {
            bytes.push(1);
            ordering(bytes, order);
        }
        Event::ReadModifyWrite {
            ordering: order,
            operation,
            ..
        } => {
            bytes.push(2);
            bytes.push(match operation {
                Fetch::FetchAdd => 0,
                Fetch::FetchSub => 1,
                Fetch::FetchAnd => 2,
                Fetch::FetchOr => 3,
                Fetch::FetchXor => 4,
            });
            ordering(bytes, order);
        }
        Event::Swap {
            ordering: order, ..
        } => {
            bytes.push(3);
            ordering(bytes, order);
        }
        Event::CompareExchange {
            success, failure, ..
        } => {
            bytes.push(4);
            ordering(bytes, success);
            ordering(bytes, failure);
        }
    }
    for operand in event.operands() {
        bytes.extend_from_slice(&operand.get().to_le_bytes());
    }
}

fn encode_structural_source(bytes: &mut Vec<u8>, source: &crate::LegalizedStructuralCaseSource) {
    match source {
        crate::LegalizedStructuralCaseSource::OperationResult { operation, result } => {
            bytes.push(0);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
            super::structural_result::encode_operation_result(bytes, result);
        }
        crate::LegalizedStructuralCaseSource::BlockParameter { block, declaration } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            super::structural_types::encode_structural_parameter(bytes, declaration);
        }
        crate::LegalizedStructuralCaseSource::Parameter { declaration } => {
            bytes.push(2);
            super::structural_types::encode_structural_parameter(bytes, declaration);
        }
        crate::LegalizedStructuralCaseSource::BorrowedParameter { declaration } => {
            bytes.push(3);
            super::structural_types::encode_structural_parameter(bytes, declaration);
        }
    }
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &LegalizedScalarSuccessor) {
    bytes.extend_from_slice(&successor.edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.target.get().to_le_bytes());
    encode_bindings(bytes, &successor.bindings);
    encode_len(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        super::structural_types::encode_structural_argument(bytes, &binding.argument);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_terminator(bytes: &mut Vec<u8>, terminator: &LegalizedScalarTerminator) {
    match terminator {
        LegalizedScalarTerminator::ReturnCleanup { returned, calls } => {
            bytes.push(5);
            encode_terminator(bytes, &LegalizedScalarTerminator::Return(returned.clone()));
            encode_len(bytes, calls.len());
            for (owner, call) in calls {
                match owner {
                    target_operations::CallSiteOwner::Operation(operation) => {
                        bytes.push(0);
                        bytes.extend_from_slice(&operation.get().to_le_bytes());
                    }
                    target_operations::CallSiteOwner::CleanupAction {
                        edge,
                        action_ordinal,
                    } => {
                        bytes.push(1);
                        bytes.extend_from_slice(&edge.get().to_le_bytes());
                        bytes.extend_from_slice(&action_ordinal.to_le_bytes());
                    }
                }
                let encoded = call.canonical_bytes();
                encode_len(bytes, encoded.len());
                bytes.extend_from_slice(&encoded);
            }
        }
        LegalizedScalarTerminator::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
            fuel,
            effect,
            ownership,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&psi_edge.get().to_le_bytes());
            bytes.push(match cause {
                terminal_psi::CrashCause::Trap => 1,
                terminal_psi::CrashCause::Abort => 2,
            });
            encode_len(bytes, site_guard.len());
            for predicate in site_guard {
                let encoded =
                    terminal_codec::canonical_proposition_order_key(predicate.proposition())
                        .expect("validated crash predicate");
                encode_len(bytes, encoded.len());
                bytes.extend_from_slice(&encoded);
            }
            encode_ids(bytes, frontier_lower_bound.iter().map(|claim| claim.get()));
            encode_fuel(bytes, fuel);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::StructuralCase {
            source,
            layout,
            cases,
            effect,
            ownership,
        } => {
            bytes.push(3);
            encode_structural_source(bytes, source);
            super::read_byte::encode_layout(bytes, layout);
            encode_len(bytes, cases.len());
            for case in cases {
                for identity in [case.edge.get(), case.target.get(), case.case.get()] {
                    bytes.extend_from_slice(&identity.to_le_bytes());
                }
                bytes.extend_from_slice(&case.case_tag.to_le_bytes());
                encode_len(bytes, case.payloads.len());
                for payload in &case.payloads {
                    bytes.extend_from_slice(&payload.field.get().to_le_bytes());
                    bytes.extend_from_slice(&payload.field_byte_offset.to_le_bytes());
                    bytes.extend_from_slice(&payload.parameter.value.get().to_le_bytes());
                    encode_scalar_type(bytes, payload.parameter.scalar_type);
                    encode_value_definition_site_identity(bytes, payload.parameter.definition_site);
                }
                encode_ids(
                    bytes,
                    case.trivial_affine_discards.iter().map(|place| place.get()),
                );
                encode_fuel(bytes, &case.fuel);
            }
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::Return(returned) => {
            bytes.push(0);
            bytes.extend_from_slice(&returned.edge.get().to_le_bytes());
            match &returned.value {
                LegalizedScalarReturnValue::Unit => bytes.push(0),
                LegalizedScalarReturnValue::StructuralParameter { place } => {
                    bytes.push(3);
                    bytes.extend_from_slice(&place.get().to_le_bytes());
                }
                LegalizedScalarReturnValue::Structural { source } => {
                    bytes.push(2);
                    encode_structural_source(bytes, source);
                }
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    encode_scalar_type(bytes, *scalar_type);
                }
            }
            encode_fuel(bytes, &returned.fuel);
            encode_effect(bytes, returned.effect);
            encode_ownership_roster(bytes, &returned.ownership);
        }
        LegalizedScalarTerminator::Jump {
            successor,
            effect,
            ownership,
        } => {
            bytes.push(1);
            encode_successor(bytes, successor);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::Conditional {
            condition,
            when_true,
            when_false,
            effect,
            ownership,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&condition.get().to_le_bytes());
            encode_successor(bytes, when_true);
            encode_successor(bytes, when_false);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
    }
}

/// A case-qualified scalar field read: above the trapping forms (95..166) and
/// the atomic access tag (167).
const TAG_CASE_PAYLOAD_FIELD_READ: u8 = 168;
const TAG_NEAREST_IEEE_FLOAT_ARITHMETIC: u8 = 169;

/// One tag per Trapping form, above every other instruction tag: 95 plus the
/// form's dense ordinal (95 through 166).
const fn trapping_tag(form: crate::TrappingForm) -> u8 {
    95 + form.ordinal()
}

/// One tag per saturating operation and carrier. The u64 and i32 forms keep
/// the tags they carried before the family was widened (35, 36, 40, 41, 42);
/// every other carrier takes its ordinal above a per-operation base, and no
/// tag is ever reused.
const fn saturating_tag(operation: SaturatingOperation, carrier: SaturatingCarrier) -> u8 {
    match (operation, carrier) {
        (SaturatingOperation::Subtract, SaturatingCarrier::U64) => 35,
        (SaturatingOperation::Add, SaturatingCarrier::U64) => 36,
        (SaturatingOperation::Add, SaturatingCarrier::I32) => 40,
        (SaturatingOperation::Subtract, SaturatingCarrier::I32) => 41,
        (SaturatingOperation::Divide, SaturatingCarrier::I32) => 42,
        (SaturatingOperation::Add, carrier) => 43 + carrier.ordinal(),
        (SaturatingOperation::Subtract, carrier) => 51 + carrier.ordinal(),
        (SaturatingOperation::Divide, carrier) => 59 + carrier.ordinal(),
        (SaturatingOperation::Remainder, carrier) => 73 + carrier.ordinal(),
        (SaturatingOperation::Multiply, carrier) => 87 + carrier.ordinal(),
    }
}

/// Each runtime traversal's operand, stride, and bounds evidence, in path
/// order, so identity separates two accesses that differ only in the element
/// a selector or its certificate names.
/// Both scalar store rows share one destination encoding: a substituted
/// owned block home or loan changes the legalized identity.
fn encode_store_destination(
    bytes: &mut Vec<u8>,
    destination: &abstract_operations::StructuralScalarStoreDestination,
) {
    use abstract_operations::StructuralScalarStoreDestination as Destination;
    match destination {
        Destination::Parameter(parameter) => {
            bytes.push(0);
            super::structural_types::encode_structural_parameter(bytes, parameter);
        }
        Destination::OwnedResult { producer, result } => {
            bytes.push(1);
            bytes.extend_from_slice(&producer.get().to_le_bytes());
            super::structural_result::encode_operation_result(bytes, result);
        }
        Destination::OwnedBlockParameter { block, parameter } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            super::structural_types::encode_structural_parameter(bytes, parameter);
        }
    }
}

fn encode_runtime_indices(
    bytes: &mut Vec<u8>,
    indices: &[crate::legalized_operations::LegalizedRuntimeIndexOperand],
) {
    bytes.extend_from_slice(&(indices.len() as u32).to_le_bytes());
    for index in indices {
        bytes.extend_from_slice(&index.operand.value.get().to_le_bytes());
        encode_scalar_type(bytes, index.operand.scalar_type);
        bytes.extend_from_slice(&index.stride.to_le_bytes());
        match index.extent {
            Some(extent) => {
                bytes.push(1);
                bytes.extend_from_slice(&extent.to_le_bytes());
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&index.obligation.get().to_le_bytes());
        bytes.extend_from_slice(&index.accepted_fact.bytes());
    }
}

fn encode_byte_operand(
    bytes: &mut Vec<u8>,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
) {
    match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } => {
            bytes.push(0);
            bytes.extend_from_slice(&source.get().to_le_bytes());
            bytes.extend_from_slice(&length.get().to_le_bytes());
        }
        terminal_psi::control_flow::ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            bytes.push(1);
            super::structural_types::encode_structural_argument(bytes, source);
            bytes.extend_from_slice(&field.get().to_le_bytes());
            bytes.extend_from_slice(&length.get().to_le_bytes());
        }
    }
}
