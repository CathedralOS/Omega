use super::calling::{encode_call_plan, encode_placement};
use super::encoding::{encode_fuel, encode_ids, encode_len};
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
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
    optimization_core::encode_option_u64(bytes, function.attachment.map(|value| value.get()));
    optimization_core::encode_option_u64(
        bytes,
        function.nominal_cleanup_subject.map(|value| value.get()),
    );
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
        crate::codec_bytes::push_encoded(bytes, parameter.value.get().to_le_bytes());
        encode_scalar_type(bytes, parameter.scalar_type);
        encode_value_definition_site_identity(bytes, parameter.definition_site);
        encode_placement(bytes, &parameter.placement);
    }
    crate::codec_bytes::push_encoded(bytes, function.entry_block.get().to_le_bytes());
    encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        crate::codec_bytes::push_encoded(bytes, block.id.get().to_le_bytes());
        encode_len(bytes, block.structural_parameters.len());
        for parameter in &block.structural_parameters {
            super::structural_types::encode_structural_parameter(bytes, parameter);
        }
        encode_len(bytes, block.parameters.len());
        for parameter in &block.parameters {
            crate::codec_bytes::push_encoded(bytes, parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
            encode_value_definition_site_identity(bytes, parameter.site);
        }
        encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            crate::codec_bytes::push_encoded(bytes, instruction.operation.get().to_le_bytes());
            match instruction.result {
                Some(result) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, result.value.get().to_le_bytes());
                    encode_scalar_type(bytes, result.scalar_type);
                    encode_value_definition_site_identity(bytes, result.definition_site);
                }
                None => bytes.push(0),
            }
            match &instruction.kind {
                LegalizedScalarInstructionKind::PortWrite {
                    service,
                    port,
                    value,
                } => {
                    bytes.push(246);
                    crate::codec_bytes::push_encoded(bytes, service.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, port.to_le_bytes());
                    bytes.push(*value);
                }

                LegalizedScalarInstructionKind::SelectImplementation { table } => {
                    bytes.push(172);
                    crate::codec_bytes::push_encoded(bytes, table.to_le_bytes());
                }
                LegalizedScalarInstructionKind::FloatResultPolicy { format, policy } => {
                    bytes.push(170);
                    encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(*format));
                    match policy {
                        terminal_psi::FloatResultPolicy::Trapping { .. } => bytes.push(0),
                        terminal_psi::FloatResultPolicy::Saturating { .. } => bytes.push(1),
                    };
                    for operand in policy.operands() {
                        crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                    }
                }
                LegalizedScalarInstructionKind::AtomicFence { ordering } => {
                    bytes.push(244);
                    bytes.push(match ordering {
                        terminal_psi::FenceOrdering::Receive => 1,
                        terminal_psi::FenceOrdering::Publish => 2,
                        terminal_psi::FenceOrdering::ReceivePublish => 3,
                    });
                }
                LegalizedScalarInstructionKind::PlacedScalarAccess {
                    boundary,
                    realization,
                    location,
                    interpretation,
                    operand,
                    indices,
                } => {
                    bytes.push(245);
                    encode_runtime_indices(bytes, indices);
                    bytes.extend_from_slice(&boundary.get().to_le_bytes());
                    bytes.extend_from_slice(&realization.contract);
                    super::boundary_invocations::encode_placed_scalar_demand(
                        bytes,
                        realization.demand,
                    );
                    super::structural_types::encode_structural_argument(bytes, location);
                    super::placed_bindings::encode_interpretation(bytes, interpretation);
                    bytes.push(u8::from(operand.is_some()));
                    if let Some(value) = operand {
                        bytes.extend_from_slice(&value.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
                    encode_integer_type(bytes, *scalar_type);
                    crate::codec_bytes::push_encoded(bytes, field_byte_offset.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::IeeeFloatClassify {
                    boundary,
                    format,
                    argument,
                    result,
                    layout,
                } => {
                    bytes.push(242);
                    crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
                    encode_scalar_type(bytes, semantic_vocabulary::ScalarType::IeeeFloat(*format));
                    crate::codec_bytes::push_encoded(bytes, argument.get().to_le_bytes());
                    super::structural_result::encode_operation_result(bytes, result);
                    super::read_byte::encode_payload(bytes, *boundary, result, layout);
                }
                LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
                    boundary,
                    realization,
                    arguments,
                } => {
                    bytes.push(TAG_NEAREST_IEEE_FLOAT_ARITHMETIC);
                    crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
                    bytes.push(realization.operation.ordinal());
                    encode_scalar_type(
                        bytes,
                        semantic_vocabulary::ScalarType::IeeeFloat(realization.format),
                    );
                    // Present only on an x86 FMA; appending it stays injective.
                    if let Some(provider) = realization.x86_scalar_fma {
                        bytes.extend_from_slice(&provider.identity());
                    }
                    crate::codec_bytes::push_encoded(bytes, (arguments.len() as u64).to_le_bytes());
                    for argument in arguments {
                        crate::codec_bytes::push_encoded(bytes, argument.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(
                        bytes,
                        capacity_obligation.get().to_le_bytes(),
                    );
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::EstablishStructuralArray {
                    result,
                    elements,
                    shape,
                } => {
                    bytes.push(243);
                    super::structural_result::encode_operation_result(bytes, result);
                    encode_len(bytes, elements.len());
                    for element in elements {
                        super::structural_types::encode_structural_argument(bytes, element);
                    }
                    super::calling::encode_shape(bytes, *shape);
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
                        crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
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
                    super::structural_types::encode_record_initializers(bytes, fields);
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
                    crate::codec_bytes::push_encoded(bytes, result_case.get().to_le_bytes());
                    super::structural_types::encode_record_initializers(bytes, fields);
                    super::read_byte::encode_layout(bytes, layout);
                }
                LegalizedScalarInstructionKind::EstablishPlacedReference {
                    result,
                    source,
                    correspondence,
                    indices,
                    shape,
                } => {
                    bytes.push(94);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::structural_types::encode_structural_argument(bytes, source);
                    crate::codec_bytes::push_encoded(
                        bytes,
                        correspondence.schema.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        bytes,
                        correspondence.carrier.get().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        bytes,
                        correspondence.source_byte_length.to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        bytes,
                        correspondence.required_base.modulus().to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        bytes,
                        correspondence.required_base.residue().to_le_bytes(),
                    );
                    encode_len(bytes, correspondence.fields.len());
                    for field in &correspondence.fields {
                        crate::codec_bytes::push_encoded(bytes, field.access_slot.to_le_bytes());
                        crate::codec_bytes::push_encoded(
                            bytes,
                            field.source_field.get().to_le_bytes(),
                        );
                        crate::codec_bytes::push_encoded(bytes, field.byte_offset.to_le_bytes());
                        crate::codec_bytes::push_encoded(
                            bytes,
                            field.transfer_width_bits.to_le_bytes(),
                        );
                        encode_scalar_type(bytes, field.logical_scalar);
                        encode_scalar_type(bytes, field.stored_scalar);
                    }
                    super::calling::encode_shape(bytes, *shape);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::EstablishReference {
                    result,
                    source,
                    storage,
                    indices,
                    shape,
                } => {
                    bytes.push(29);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::structural_types::encode_structural_argument(bytes, source);
                    if let Some(storage) = storage {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(
                            bytes,
                            storage.byte_offset.get().to_le_bytes(),
                        );
                        if let Some(stored_scalar) = storage.stored_scalar {
                            bytes.push(1);
                            encode_scalar_type(bytes, stored_scalar);
                        } else {
                            bytes.push(0);
                        }
                    } else {
                        bytes.push(0);
                    }
                    super::calling::encode_shape(bytes, *shape);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::ReleaseReference { source, path } => {
                    bytes.push(30);
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                }
                LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                    result,
                    value,
                    shape,
                } => {
                    bytes.push(16);
                    super::structural_result::encode_operation_result(bytes, result);
                    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    super::calling::encode_shape(bytes, *shape);
                }
                LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
                    bytes.push(17);
                    crate::codec_bytes::push_encoded(bytes, destination.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                    crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, case_tag.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, tag_byte_offset.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
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
                                crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                            }
                        }
                    }
                    crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
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
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
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
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, destination.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
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
                    // Zero, which no field identity takes, names the whole slot.
                    bytes.extend_from_slice(&field.map_or(0, |field| field.get()).to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    bytes.push(*byte_size);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source } => {
                    bytes.push(11);
                    crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, value.value.get().to_le_bytes());
                    encode_scalar_type(bytes, value.scalar_type);
                    crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
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
                    indices,
                    start,
                    end,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(9);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::structural_types::encode_structural_argument(bytes, source);
                    encode_runtime_indices(bytes, indices);
                    for identity in [start.get(), end.get(), length.get(), obligation.get()] {
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceLength {
                    source,
                    indices,
                    length_byte_offset,
                } => {
                    bytes.push(7);
                    super::structural_types::encode_structural_argument(bytes, source);
                    encode_runtime_indices(bytes, indices);
                    crate::codec_bytes::push_encoded(bytes, length_byte_offset.to_le_bytes());
                }
                LegalizedScalarInstructionKind::ByteSequencePredicate { operand, predicate } => {
                    bytes.push(45);
                    encode_byte_operand(bytes, operand);
                    bytes.push(predicate.mask_bit());
                }
                LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } => {
                    bytes.push(46);
                    encode_byte_operand(bytes, left);
                    encode_byte_operand(bytes, right);
                }
                LegalizedScalarInstructionKind::EstablishElementView {
                    indices,
                    result,
                    destination,
                    source,
                    element,
                    region,
                    accepted_fact,
                } => {
                    bytes.push(40);
                    encode_runtime_indices(bytes, indices);
                    super::structural_result::encode_operation_result(bytes, result);
                    crate::codec_bytes::push_encoded(bytes, destination.get().to_le_bytes());
                    super::structural_types::encode_structural_argument(bytes, source);
                    crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
                    match region {
                        terminal_psi::ElementViewRegion::WholeCollection => bytes.push(0),
                        terminal_psi::ElementViewRegion::RepresentationBytes {
                            start,
                            end,
                            obligation,
                        } => {
                            bytes.push(1);
                            for id in [start.get(), end.get(), obligation.get()] {
                                crate::codec_bytes::push_encoded(bytes, id.to_le_bytes());
                            }
                        }
                    }
                    match accepted_fact {
                        Some(fact) => {
                            bytes.push(1);
                            bytes.extend_from_slice(&fact.bytes());
                        }
                        None => bytes.push(0),
                    }
                }
                LegalizedScalarInstructionKind::ElementViewWrite {
                    path,
                    destination,
                    indices,
                    index,
                    value,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(44);
                    super::structural_types::encode_structural_path(bytes, path);
                    super::structural_types::encode_structural_argument(bytes, destination);
                    encode_runtime_indices(bytes, indices);
                    for identity in [index.get(), value.get(), length.get(), obligation.get()] {
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ElementViewSubslice {
                    result,
                    source,
                    indices,
                    start,
                    end,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(41);
                    super::structural_result::encode_operation_result(bytes, result);
                    super::structural_types::encode_structural_argument(bytes, source);
                    encode_runtime_indices(bytes, indices);
                    for identity in [start.get(), end.get(), length.get(), obligation.get()] {
                        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                    indices,
                } => {
                    bytes.push(42);
                    crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                    super::structural_types::encode_structural_path(bytes, path);
                    encode_runtime_indices(bytes, indices);
                }
                LegalizedScalarInstructionKind::ElementViewLength {
                    source,
                    indices,
                    length_byte_offset,
                } => {
                    bytes.push(43);
                    super::structural_types::encode_structural_argument(bytes, source);
                    encode_runtime_indices(bytes, indices);
                    crate::codec_bytes::push_encoded(bytes, length_byte_offset.to_le_bytes());
                }
                LegalizedScalarInstructionKind::BooleanNot { operand } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } => {
                    bytes.push(5);
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                    encode_integer_type(bytes, *source_type);
                }
                LegalizedScalarInstructionKind::ScalarRepresentationView {
                    operand,
                    source_type,
                } => {
                    bytes.push(171);
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                    encode_scalar_type(bytes, *source_type);
                }
                LegalizedScalarInstructionKind::IntegerExactCast {
                    operand,
                    source_type,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(20);
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                    encode_integer_type(bytes, *source_type);
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
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
                LegalizedScalarInstructionKind::SaturatingAdd {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Add, *carrier));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingSubtract {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Subtract, *carrier));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingDivide {
                    carrier,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Divide, *carrier));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::SaturatingMultiply {
                    carrier,
                    left,
                    right,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Multiply, *carrier));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::TrappingBinary { form, left, right } => {
                    bytes.push(trapping_tag(*form));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::TrappingConvert {
                    form,
                    source,
                    operand,
                } => {
                    bytes.push(trapping_tag(*form));
                    bytes.push(source.ordinal());
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::SaturatingRemainder {
                    carrier,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(saturating_tag(SaturatingOperation::Remainder, *carrier));
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingRemainder {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(37);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingDivide {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(70);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::WrappingAdd { left, right } => {
                    bytes.push(38);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingSubtract { left, right } => {
                    bytes.push(68);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingMultiply { left, right } => {
                    bytes.push(69);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingShiftLeft { value, count } => {
                    bytes.push(81);
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, count.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::WrappingShiftRight { value, count } => {
                    bytes.push(82);
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, count.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::ExactShiftLeft {
                    value,
                    count,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(83);
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, count.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ExactShiftRight {
                    value,
                    count,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(84);
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, count.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::BitwiseAnd { left, right } => {
                    bytes.push(24);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseOr { left, right } => {
                    bytes.push(71);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
                    bytes.push(26);
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::BitwiseNot { operand } => {
                    bytes.push(72);
                    crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, left.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, right.get().to_le_bytes());
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
        crate::codec_bytes::push_encoded(bytes, operand.get().to_le_bytes());
    }
}

fn encode_structural_source(bytes: &mut Vec<u8>, source: &crate::LegalizedStructuralCaseSource) {
    match source {
        crate::LegalizedStructuralCaseSource::OperationResult { operation, result } => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            super::structural_result::encode_operation_result(bytes, result);
        }
        crate::LegalizedStructuralCaseSource::BlockParameter { block, declaration } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
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
    encode_cleanup_calls(bytes, &successor.cleanup_calls);
    crate::codec_bytes::push_encoded(bytes, successor.edge.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, successor.target.get().to_le_bytes());
    encode_bindings(bytes, &successor.bindings);
    encode_len(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        crate::codec_bytes::push_encoded(bytes, binding.parameter.get().to_le_bytes());
        super::structural_types::encode_structural_argument(bytes, &binding.argument);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_cleanup_calls(
    bytes: &mut Vec<u8>,
    calls: &[(target_operations::CallSiteOwner, crate::LegalizedScalarCall)],
) {
    encode_len(bytes, calls.len());
    for (owner, call) in calls {
        match owner {
            target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
            target_operations::CallSiteOwner::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            target_operations::CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, action_ordinal.to_le_bytes());
            }
        }
        let encoded = call.canonical_bytes();
        encode_len(bytes, encoded.len());
        bytes.extend_from_slice(&encoded);
    }
}

fn encode_terminator(bytes: &mut Vec<u8>, terminator: &LegalizedScalarTerminator) {
    match terminator {
        LegalizedScalarTerminator::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            arguments,
            binding,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
            fuel,
            effect,
            ownership,
        } => {
            bytes.push(6);
            crate::codec_bytes::push_encoded(bytes, psi_edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            match kind {
                semantic_vocabulary::ExternalCompletionKind::ProcessExit => bytes.push(1),
            }
            super::boundary_invocations::encode_binding(bytes, binding);
            encode_len(bytes, arguments.len());
            for argument in arguments {
                crate::codec_bytes::push_encoded(bytes, argument.parameter_index.to_le_bytes());
                super::normalized_foreign::encode_scalar_source(bytes, argument.source);
                encode_placement(bytes, &argument.placement);
            }
            encode_len(bytes, structural_arguments.len());
            for argument in structural_arguments {
                super::structural_types::encode_structural_argument(bytes, argument);
            }
            encode_ids(
                bytes,
                requirement_obligations
                    .iter()
                    .map(|identity| identity.get()),
            );
            encode_ids(
                bytes,
                frontier_lower_bound.iter().map(|identity| identity.get()),
            );
            encode_fuel(bytes, fuel);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::ReturnCleanup { returned, calls } => {
            bytes.push(5);
            encode_terminator(bytes, &LegalizedScalarTerminator::Return(returned.clone()));
            encode_cleanup_calls(bytes, calls);
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
            crate::codec_bytes::push_encoded(bytes, psi_edge.get().to_le_bytes());
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
            path,
            root_shape,
            byte_offset,
            layout,
            cases,
            effect,
            ownership,
        } => {
            bytes.push(3);
            encode_structural_source(bytes, source);
            super::structural_types::encode_structural_path(bytes, path);
            super::calling::encode_shape(bytes, *root_shape);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            super::read_byte::encode_layout(bytes, layout);
            encode_len(bytes, cases.len());
            for case in cases {
                for identity in [case.edge.get(), case.target.get(), case.case.get()] {
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                }
                crate::codec_bytes::push_encoded(bytes, case.case_tag.to_le_bytes());
                encode_len(bytes, case.payloads.len());
                for payload in &case.payloads {
                    crate::codec_bytes::push_encoded(bytes, payload.field.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(
                        bytes,
                        payload.field_byte_offset.to_le_bytes(),
                    );
                    crate::codec_bytes::push_encoded(
                        bytes,
                        payload.parameter.value.get().to_le_bytes(),
                    );
                    encode_scalar_type(bytes, payload.parameter.scalar_type);
                    encode_value_definition_site_identity(bytes, payload.parameter.definition_site);
                }
                encode_len(bytes, case.cleanup_actions.len());
                for action in &case.cleanup_actions {
                    super::structural::encode_cleanup_action(bytes, action);
                }
                encode_cleanup_calls(bytes, &case.cleanup_calls);
                encode_fuel(bytes, &case.fuel);
            }
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::Return(returned) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, returned.edge.get().to_le_bytes());
            match &returned.value {
                LegalizedScalarReturnValue::Unit => bytes.push(0),
                LegalizedScalarReturnValue::StructuralParameter { place } => {
                    bytes.push(3);
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
                LegalizedScalarReturnValue::SharedDescriptor { place } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
                LegalizedScalarReturnValue::Structural { source } => {
                    bytes.push(2);
                    encode_structural_source(bytes, source);
                }
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
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
            crate::codec_bytes::push_encoded(bytes, condition.get().to_le_bytes());
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
            crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            super::structural_result::encode_operation_result(bytes, result);
        }
        Destination::OwnedBlockParameter { block, parameter } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            super::structural_types::encode_structural_parameter(bytes, parameter);
        }
    }
}

pub(super) fn encode_runtime_indices(
    bytes: &mut Vec<u8>,
    indices: &[crate::legalized_operations::LegalizedRuntimeIndexOperand],
) {
    crate::codec_bytes::push_encoded(bytes, (indices.len() as u32).to_le_bytes());
    for index in indices {
        crate::codec_bytes::push_encoded(bytes, index.operand.value.get().to_le_bytes());
        encode_scalar_type(bytes, index.operand.scalar_type);
        crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
        match index.extent {
            Some(extent) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, extent.to_le_bytes());
            }
            None => bytes.push(0),
        }
        crate::codec_bytes::push_encoded(bytes, index.obligation.get().to_le_bytes());
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
            super::structural_types::encode_structural_argument(bytes, source);
            crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
        }
        terminal_psi::control_flow::ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            bytes.push(1);
            super::structural_types::encode_structural_argument(bytes, source);
            crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
        }
    }
}
