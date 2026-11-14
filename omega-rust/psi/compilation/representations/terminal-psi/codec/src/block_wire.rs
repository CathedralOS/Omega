/*
Canonical wire encoding and decoding for `TerminalMachine` control-flow blocks (`Block`,
`Operation`, `OperationKind`, `Terminator`), atomic and storage operations, call sites, and
terminator edges.
*/

pub(crate) mod atomic_operations {

    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::operation_tags;
    use crate::sections::semantic_module::structural_place_wire::{
        decode_structural_path, encode_structural_path,
    };
    use language_core::atomic::{
        AtomicCompareExchangeAttemptAxis, AtomicCompareExchangeObservationAxis,
        AtomicCompareExchangeOnceResultCustody, AtomicCompareExchangeOutcomeIdentity,
    };
    use semantic_vocabulary::PlaceId;
    use terminal_psi::{
        AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, OperationKind,
        StructuralPathSegment,
    };

    const LOAD: u8 = 1;
    const STORE: u8 = 2;
    const READ_MODIFY_WRITE: u8 = 3;
    const SWAP: u8 = 4;
    const COMPARE_EXCHANGE: u8 = 5;
    const COMPARE_EXCHANGE_ONCE: u8 = 6;

    pub(crate) fn encode_atomic_access(
        writer: &mut Writer,
        place: PlaceId,
        path: &[StructuralPathSegment],
        event: AtomicAccessEvent,
        resident_obligation: Option<semantic_vocabulary::ObligationId>,
    ) -> Result<(), CodecError> {
        writer.u8(if resident_obligation.is_some() {
            operation_tags::RESTRICTED_ATOMIC_ACCESS
        } else {
            operation_tags::ATOMIC_ACCESS
        });
        if let Some(obligation) = resident_obligation {
            writer.id(obligation);
        }
        writer.id(place);
        encode_structural_path(writer, "atomic access path", path)?;
        match event {
            AtomicAccessEvent::Load { ordering } => {
                writer.u8(LOAD);
                encode_ordering(writer, ordering);
            }
            AtomicAccessEvent::Store { value, ordering } => {
                writer.u8(STORE);
                writer.id(value);
                encode_ordering(writer, ordering);
            }
            AtomicAccessEvent::ReadModifyWrite {
                operation,
                operand,
                ordering,
            } => {
                writer.u8(READ_MODIFY_WRITE);
                writer.u8(match operation {
                    AtomicReadModifyWrite::FetchAdd => 1,
                    AtomicReadModifyWrite::FetchSub => 2,
                    AtomicReadModifyWrite::FetchAnd => 3,
                    AtomicReadModifyWrite::FetchOr => 4,
                    AtomicReadModifyWrite::FetchXor => 5,
                });
                writer.id(operand);
                encode_ordering(writer, ordering);
            }
            AtomicAccessEvent::Swap { value, ordering } => {
                writer.u8(SWAP);
                writer.id(value);
                encode_ordering(writer, ordering);
            }
            AtomicAccessEvent::CompareExchange {
                expected,
                replacement,
                success,
                failure,
            } => {
                writer.u8(COMPARE_EXCHANGE);
                writer.id(expected);
                writer.id(replacement);
                encode_ordering(writer, success);
                encode_ordering(writer, failure);
            }
            AtomicAccessEvent::CompareExchangeOnce {
                custody,
                expected,
                replacement,
                success,
                failure,
            } => {
                writer.u8(COMPARE_EXCHANGE_ONCE);
                encode_once_custody(writer, custody)?;
                writer.id(expected);
                writer.id(replacement);
                encode_ordering(writer, success);
                encode_ordering(writer, failure);
            }
        }
        Ok(())
    }

    pub(crate) fn decode_atomic_access(
        reader: &mut Reader<'_>,
        restricted: bool,
    ) -> Result<OperationKind, CodecError> {
        let resident_obligation = if restricted {
            Some(reader.id("ObligationId")?)
        } else {
            None
        };
        let place = reader.id("PlaceId")?;
        let path = decode_structural_path(reader)?;
        let event = match reader.u8()? {
            LOAD => AtomicAccessEvent::Load {
                ordering: decode_ordering(reader)?,
            },
            STORE => AtomicAccessEvent::Store {
                value: reader.id("ValueId")?,
                ordering: decode_ordering(reader)?,
            },
            READ_MODIFY_WRITE => {
                let operation = match reader.u8()? {
                    1 => AtomicReadModifyWrite::FetchAdd,
                    2 => AtomicReadModifyWrite::FetchSub,
                    3 => AtomicReadModifyWrite::FetchAnd,
                    4 => AtomicReadModifyWrite::FetchOr,
                    5 => AtomicReadModifyWrite::FetchXor,
                    tag => return Err(CodecError::InvalidTag("AtomicReadModifyWrite", tag)),
                };
                AtomicAccessEvent::ReadModifyWrite {
                    operation,
                    operand: reader.id("ValueId")?,
                    ordering: decode_ordering(reader)?,
                }
            }
            SWAP => AtomicAccessEvent::Swap {
                value: reader.id("ValueId")?,
                ordering: decode_ordering(reader)?,
            },
            COMPARE_EXCHANGE => AtomicAccessEvent::CompareExchange {
                expected: reader.id("ValueId")?,
                replacement: reader.id("ValueId")?,
                success: decode_ordering(reader)?,
                failure: decode_ordering(reader)?,
            },
            COMPARE_EXCHANGE_ONCE => AtomicAccessEvent::CompareExchangeOnce {
                custody: decode_once_custody(reader)?,
                expected: reader.id("ValueId")?,
                replacement: reader.id("ValueId")?,
                success: decode_ordering(reader)?,
                failure: decode_ordering(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("AtomicAccessEvent", tag)),
        };
        Ok(OperationKind::AtomicAccess {
            place,
            path,
            event,
            resident_obligation,
        })
    }

    fn encode_once_custody(
        writer: &mut Writer,
        custody: AtomicCompareExchangeOnceResultCustody,
    ) -> Result<(), CodecError> {
        if !custody.is_canonical() {
            return Err(CodecError::MalformedStructuralFoundation(
                "atomic attempt has inconsistent result custody",
            ));
        }
        writer.u8(match custody.attempt {
            AtomicCompareExchangeAttemptAxis::Decisive => 1,
            AtomicCompareExchangeAttemptAxis::SingleAttempt => 2,
        });
        writer.u8(match custody.observation {
            AtomicCompareExchangeObservationAxis::Observing => 1,
            AtomicCompareExchangeObservationAxis::NonObserving => 2,
        });
        writer.u8(match custody.outcome_identity {
            AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome => 1,
            AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome => 2,
            AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOutcome => 3,
            AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOnceOutcome => 4,
        });
        Ok(())
    }

    fn decode_once_custody(
        reader: &mut Reader<'_>,
    ) -> Result<AtomicCompareExchangeOnceResultCustody, CodecError> {
        let attempt = match reader.u8()? {
            1 => AtomicCompareExchangeAttemptAxis::Decisive,
            2 => AtomicCompareExchangeAttemptAxis::SingleAttempt,
            tag => {
                return Err(CodecError::InvalidTag(
                    "AtomicCompareExchangeAttemptAxis",
                    tag,
                ));
            }
        };
        let observation = match reader.u8()? {
            1 => AtomicCompareExchangeObservationAxis::Observing,
            2 => AtomicCompareExchangeObservationAxis::NonObserving,
            tag => {
                return Err(CodecError::InvalidTag(
                    "AtomicCompareExchangeObservationAxis",
                    tag,
                ));
            }
        };
        let outcome_identity = match reader.u8()? {
            1 => AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome,
            2 => AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome,
            3 => AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOutcome,
            4 => AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOnceOutcome,
            tag => {
                return Err(CodecError::InvalidTag(
                    "AtomicCompareExchangeOutcomeIdentity",
                    tag,
                ));
            }
        };
        let custody = AtomicCompareExchangeOnceResultCustody {
            attempt,
            observation,
            outcome_identity,
        };
        if !custody.is_canonical() {
            return Err(CodecError::MalformedStructuralFoundation(
                "atomic attempt has inconsistent result custody",
            ));
        }
        Ok(custody)
    }

    fn encode_ordering(writer: &mut Writer, ordering: MemoryOrdering) {
        writer.u8(match ordering {
            MemoryOrdering::NoOrdering => 1,
            MemoryOrdering::Receive => 2,
            MemoryOrdering::Publish => 3,
            MemoryOrdering::ReceivePublish => 4,
            MemoryOrdering::GlobalOrder => 5,
        });
    }

    fn decode_ordering(reader: &mut Reader<'_>) -> Result<MemoryOrdering, CodecError> {
        Ok(match reader.u8()? {
            1 => MemoryOrdering::NoOrdering,
            2 => MemoryOrdering::Receive,
            3 => MemoryOrdering::Publish,
            4 => MemoryOrdering::ReceivePublish,
            5 => MemoryOrdering::GlobalOrder,
            tag => return Err(CodecError::InvalidTag("MemoryOrdering", tag)),
        })
    }

    pub(crate) fn encode_atomic_fence(writer: &mut Writer, ordering: terminal_psi::FenceOrdering) {
        use terminal_psi::FenceOrdering;
        writer.u8(operation_tags::ATOMIC_FENCE);
        writer.u8(match ordering {
            FenceOrdering::Receive => 1,
            FenceOrdering::Publish => 2,
            FenceOrdering::ReceivePublish => 3,
        });
    }

    pub(crate) fn decode_atomic_fence(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        use terminal_psi::FenceOrdering;
        let ordering = match reader.u8()? {
            1 => FenceOrdering::Receive,
            2 => FenceOrdering::Publish,
            3 => FenceOrdering::ReceivePublish,
            tag => return Err(CodecError::InvalidTag("FenceOrdering", tag)),
        };
        Ok(OperationKind::AtomicFence { ordering })
    }
}
pub(crate) mod call_operations {

    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::operation_tags;
    use semantic_vocabulary::ServiceId;
    use terminal_psi::OperationKind;

    pub(crate) fn encode_register_access(
        writer: &mut Writer,
        service: Option<ServiceId>,
        event: terminal_psi::RegisterAccessEvent,
    ) -> Result<(), CodecError> {
        if !event.operation().is_admitted()
            || service.is_some() != event.operation().requires_service_authority()
        {
            return Err(CodecError::MalformedStructuralFoundation(
                "register access disagrees with its catalog service authority",
            ));
        }
        if let Some(service) = service {
            writer.u8(operation_tags::REGISTER_ACCESS);
            writer.id(service);
        } else {
            writer.u8(operation_tags::REGISTER_OBSERVATION);
        }
        writer.u16(event.operation().catalog_tag());
        for value in event.inputs() {
            writer.id(*value);
        }
        Ok(())
    }

    pub(crate) fn decode_register_access(
        reader: &mut Reader<'_>,
        requires_service: bool,
    ) -> Result<OperationKind, CodecError> {
        let service = if requires_service {
            Some(reader.id("ServiceId")?)
        } else {
            None
        };
        let tag = reader.u16()?;
        let operation = terminal_psi::RegisterOperation::from_catalog_tag(tag).ok_or(
            CodecError::MalformedStructuralFoundation("unknown register access catalog identity"),
        )?;
        if service.is_some() != operation.requires_service_authority() {
            return Err(CodecError::MalformedStructuralFoundation(
                "register access wire tag disagrees with catalog service authority",
            ));
        }
        let event = match operation {
            terminal_psi::RegisterOperation::GlobalDescriptorTable(_)
            | terminal_psi::RegisterOperation::TaskRegisterActivate
            | terminal_psi::RegisterOperation::TaskRegisterRead
            | terminal_psi::RegisterOperation::SegmentReload(_)
            | terminal_psi::RegisterOperation::SegmentRead(_) => {
                return Err(CodecError::MalformedStructuralFoundation(
                    "descriptor-table operations require a checked structural operand",
                ));
            }
            terminal_psi::RegisterOperation::MemoryFence(kind) => {
                terminal_psi::RegisterAccessEvent::MemoryFence { kind }
            }
            terminal_psi::RegisterOperation::ProcessorHalt => {
                terminal_psi::RegisterAccessEvent::ProcessorHalt
            }
            terminal_psi::RegisterOperation::InterruptControl(kind) => {
                terminal_psi::RegisterAccessEvent::InterruptControl { kind }
            }
            terminal_psi::RegisterOperation::SystemRead(register) => {
                terminal_psi::RegisterAccessEvent::Read { register }
            }
            terminal_psi::RegisterOperation::SystemWrite(register) => {
                terminal_psi::RegisterAccessEvent::Write {
                    register,
                    value: reader.id("ValueId")?,
                }
            }
            terminal_psi::RegisterOperation::PortRead => {
                terminal_psi::RegisterAccessEvent::PortRead {
                    port: reader.id("ValueId")?,
                }
            }
            terminal_psi::RegisterOperation::PortWrite => {
                terminal_psi::RegisterAccessEvent::PortWrite {
                    port: reader.id("ValueId")?,
                    value: reader.id("ValueId")?,
                }
            }
            terminal_psi::RegisterOperation::ModelSpecificRead => {
                terminal_psi::RegisterAccessEvent::ModelSpecificRead {
                    index: reader.id("ValueId")?,
                }
            }
            terminal_psi::RegisterOperation::ModelSpecificWrite => {
                terminal_psi::RegisterAccessEvent::ModelSpecificWrite {
                    index: reader.id("ValueId")?,
                    value: reader.id("ValueId")?,
                }
            }
        };
        Ok(OperationKind::RegisterAccess { service, event })
    }
}
pub(crate) mod operation_tags {

    pub(crate) const INTEGER_CONSTANT: u8 = 1;
    pub(crate) const BOOLEAN_CONSTANT: u8 = 2;
    pub(crate) const WRAPPING_INTEGER_ADD: u8 = 3;
    pub(crate) const SATURATING_INTEGER_ADD: u8 = 4;
    pub(crate) const WRAPPING_INTEGER_SUBTRACT: u8 = 5;
    pub(crate) const SATURATING_INTEGER_SUBTRACT: u8 = 6;
    pub(crate) const WRAPPING_INTEGER_MULTIPLY: u8 = 7;
    pub(crate) const SATURATING_INTEGER_MULTIPLY: u8 = 8;
    pub(crate) const BOOLEAN_NOT: u8 = 9;
    pub(crate) const BOOLEAN_EQUAL: u8 = 10;
    pub(crate) const INTEGER_EQUAL: u8 = 11;
    pub(crate) const INTEGER_LESS_THAN: u8 = 12;
    pub(crate) const INTEGER_LESS_OR_EQUAL: u8 = 13;
    pub(crate) const INTEGER_BITWISE_AND: u8 = 14;
    pub(crate) const INTEGER_BITWISE_OR: u8 = 15;
    pub(crate) const INTEGER_BITWISE_XOR: u8 = 16;
    pub(crate) const WRAPPING_INTEGER_SHIFT_LEFT: u8 = 17;
    pub(crate) const WRAPPING_INTEGER_SHIFT_RIGHT: u8 = 18;
    pub(crate) const INTEGER_BITWISE_NOT: u8 = 19;
    pub(crate) const INTEGER_WIDEN: u8 = 20;
    pub(crate) const INTEGER_EXACT_CAST: u8 = 21;
    pub(crate) const EXACT_INTEGER_SHIFT_RIGHT: u8 = 22;
    pub(crate) const EXACT_INTEGER_SHIFT_LEFT: u8 = 23;
    pub(crate) const EXACT_INTEGER_ADD: u8 = 24;
    pub(crate) const EXACT_INTEGER_SUBTRACT: u8 = 25;
    pub(crate) const EXACT_INTEGER_MULTIPLY: u8 = 26;
    pub(crate) const EXACT_INTEGER_DIVIDE: u8 = 27;
    pub(crate) const EXACT_INTEGER_REMAINDER: u8 = 28;
    pub(crate) const WRAPPING_INTEGER_DIVIDE: u8 = 29;
    pub(crate) const WRAPPING_INTEGER_REMAINDER: u8 = 30;
    pub(crate) const SATURATING_INTEGER_DIVIDE: u8 = 31;
    pub(crate) const SATURATING_INTEGER_REMAINDER: u8 = 32;
    pub(crate) const CALL: u8 = 33;
    pub(crate) const CALL_UNIT: u8 = 34;
    pub(crate) const BOUNDARY_CALL: u8 = 35;
    pub(crate) const PORT_WRITE: u8 = 36;
    pub(crate) const ESTABLISH_TRIVIAL_AFFINE_LOCAL: u8 = 37;
    pub(crate) const BOOLEAN_STRUCTURAL_FIELD: u8 = 38;
    pub(crate) const CALL_STRUCTURAL_SCALAR: u8 = 39;
    pub(crate) const ESTABLISH_BYTE_SEQUENCE_LITERAL: u8 = 40;
    pub(crate) const CALL_STRUCTURAL: u8 = 41;
    pub(crate) const ESTABLISH_SCALAR_CASE: u8 = 42;
    pub(crate) const WRITE_ONLY_PRIMITIVE_STORE: u8 = 43;
    pub(crate) const IEEE_FLOAT_CONSTANT: u8 = 44;
    pub(crate) const NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD: u8 = 45;
    pub(crate) const STRUCTURAL_SCALAR_FIELD_STORE: u8 = 46;
    pub(crate) const SCALAR_STRUCTURAL_FIELD: u8 = 47;
    pub(crate) const CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS: u8 = 50;
    pub(crate) const BYTE_SEQUENCE_LENGTH: u8 = 115;
    pub(crate) const BYTE_SEQUENCE_READ: u8 = 56;
    pub(crate) const BYTE_SEQUENCE_SUBSLICE: u8 = 113;
    pub(crate) const STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE: u8 = 58;
    pub(crate) const STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH: u8 = 59;
    pub(crate) const STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE: u8 = 60;
    pub(crate) const ESTABLISH_PRIMITIVE_LOCAL: u8 = 117;
    pub(crate) const PRIMITIVE_SCALAR_READ: u8 = 62;
    pub(crate) const BYTE_SEQUENCE_WRITE: u8 = 63;
    pub(crate) const ESTABLISH_SCALAR_ARRAY: u8 = 64;
    pub(crate) const IEEE_FLOAT_COMPARE: u8 = 65;
    pub(crate) const STRUCTURAL_CASE_MEMBERSHIP: u8 = 66;
    pub(crate) const ESTABLISH_RECORD: u8 = 68;
    pub(crate) const ESTABLISH_REFERENCE: u8 = 69;
    pub(crate) const RELEASE_REFERENCE: u8 = 70;
    pub(crate) const RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE: u8 = 75;
    pub(crate) const MOVE_STRUCTURAL_FIELD: u8 = 77;
    pub(crate) const STORE_STRUCTURAL_FIELD: u8 = 78;
    pub(crate) const ESTABLISH_ELEMENT_VIEW: u8 = 79;
    pub(crate) const ELEMENT_VIEW_READ: u8 = 81;
    pub(crate) const ELEMENT_VIEW_SUBSLICE: u8 = 114;
    pub(crate) const ESTABLISH_STRUCTURAL_CASE: u8 = 83;
    pub(crate) const STRUCTURAL_LEAF_COPY: u8 = 84;

    pub(crate) const STRUCTURAL_CASE_LEAF_COPY: u8 = 85;

    pub(crate) const STRUCTURAL_BYTE_SEQUENCE_FIELD_READ: u8 = 86;

    pub(crate) const PROJECTED_PRIMITIVE_SCALAR_READ: u8 = 89;

    pub(crate) const PROJECTED_WRITE_ONLY_PRIMITIVE_STORE: u8 = 90;

    pub(crate) const TRAPPING_INTEGER: u8 = 91;

    pub(crate) const PROJECTED_ELEMENT_VIEW_READ: u8 = 92;

    pub(crate) const ATOMIC_ACCESS: u8 = 93;

    pub(crate) const ESTABLISH_ZEROED_STRUCTURAL: u8 = 94;

    pub(crate) const ESTABLISH_BYTE_SEQUENCE_CONCATENATION: u8 = 95;

    pub(crate) const BYTE_SEQUENCE_STORE: u8 = 96;

    pub(crate) const ELEMENT_VIEW_WRITE: u8 = 97;

    pub(crate) const ESTABLISH_MUTABLE_ELEMENT_VIEW: u8 = 98;

    pub(crate) const FLOAT_RESULT_POLICY: u8 = 99;

    pub(crate) const STRUCTURAL_BYTE_VIEW_FIELD_STORE: u8 = 100;

    pub(crate) const SCALAR_REPRESENTATION_VIEW: u8 = 101;

    pub(crate) const LANDED_SCALAR_REPRESENTATION_VIEW: u8 = 102;

    pub(crate) const ESTABLISH_STRUCTURAL_LOCAL: u8 = 103;
    pub(crate) const BYTE_SEQUENCE_PREDICATE: u8 = 104;

    pub(crate) const BYTE_SEQUENCE_EQUAL: u8 = 105;
    pub(crate) const PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE: u8 = 106;

    pub(crate) const ESTABLISH_STRUCTURAL_ARRAY: u8 = 107;

    pub(crate) const PROJECTED_RELEASE_REFERENCE: u8 = 108;

    pub(crate) const SELECT_IMPLEMENTATION: u8 = 109;
    pub(crate) const CALL_DYNAMIC: u8 = 110;

    pub(crate) const ESTABLISH_STORAGE_VIEW_REFERENCE: u8 = 111;

    pub(crate) const ELEMENT_VIEW_LENGTH: u8 = 112;
    pub(crate) const RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE: u8 = 116;

    pub(crate) const REPLACE_DISCARDABLE_STRUCTURAL_FIELD: u8 = 118;

    pub(crate) const ATOMIC_FENCE: u8 = 119;

    pub(crate) const ESTABLISH_ACCESS_REFERENCE: u8 = 120;

    pub(crate) const REGISTER_ACCESS: u8 = 121;

    pub(crate) const ESTABLISH_NATIVE_CODE_LAYOUT: u8 = 122;
    pub(crate) const STORAGE_ADDRESS: u8 = 123;

    pub(crate) const MOVE_STRUCTURAL_VALUE: u8 = 124;

    pub(crate) const TRANSFORM_CONTENT: u8 = 125;

    pub(crate) const REGISTER_OBSERVATION: u8 = 126;
    pub(crate) const ESTABLISH_ACTIVATION_CLAIM: u8 = 127;
    pub(crate) const RELEASE_ACTIVATION_CLAIM: u8 = 128;
    pub(crate) const SPLIT_ACTIVATION_CLAIM: u8 = 129;
    pub(crate) const MERGE_ACTIVATION_CLAIM: u8 = 130;

    pub(crate) const RESTRICTED_ATOMIC_ACCESS: u8 = 131;

    pub(crate) const READ_SCALAR_APPLICATION: u8 = 132;

    pub(crate) const GLOBAL_DESCRIPTOR_TABLE: u8 = 133;

    pub(crate) const ACTIVATE_TASK_REGISTER: u8 = 134;
    pub(crate) const READ_TASK_REGISTER: u8 = 135;

    pub(crate) const RELOAD_SEGMENT_DESCRIPTORS: u8 = 136;
    pub(crate) const READ_SEGMENT_SELECTOR: u8 = 137;
}
pub(crate) mod scalar_operations {

    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::operation_tags;
    use semantic_vocabulary::ValueId;
    use terminal_psi::FloatResultPolicy;
    use terminal_psi::{OperationKind, TrappingIntegerOperation};

    pub(crate) fn encode_trapping_integer(
        writer: &mut Writer,
        operation: TrappingIntegerOperation,
    ) -> Result<(), CodecError> {
        writer.u8(operation_tags::TRAPPING_INTEGER);
        let (primitive, operands): (u8, &[ValueId]) = match &operation {
            TrappingIntegerOperation::Add { left, right } => (1, &[*left, *right]),
            TrappingIntegerOperation::Subtract { left, right } => (2, &[*left, *right]),
            TrappingIntegerOperation::Multiply { left, right } => (3, &[*left, *right]),
            TrappingIntegerOperation::Divide { left, right } => (4, &[*left, *right]),
            TrappingIntegerOperation::Remainder { left, right } => (5, &[*left, *right]),
            TrappingIntegerOperation::ShiftLeft { value, count } => (6, &[*value, *count]),
            TrappingIntegerOperation::ShiftRight { value, count } => (7, &[*value, *count]),
            TrappingIntegerOperation::Convert { operand } => (8, &[*operand]),
        };
        writer.u8(primitive);
        for operand in operands {
            writer.id(*operand);
        }
        Ok(())
    }

    pub(crate) fn decode_trapping_integer(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        let primitive = reader.u8()?;
        let binary = |reader: &mut Reader<'_>| -> Result<(ValueId, ValueId), CodecError> {
            Ok((reader.id("ValueId")?, reader.id("ValueId")?))
        };
        let operation = match primitive {
            1 => {
                let (left, right) = binary(reader)?;
                TrappingIntegerOperation::Add { left, right }
            }
            2 => {
                let (left, right) = binary(reader)?;
                TrappingIntegerOperation::Subtract { left, right }
            }
            3 => {
                let (left, right) = binary(reader)?;
                TrappingIntegerOperation::Multiply { left, right }
            }
            4 => {
                let (left, right) = binary(reader)?;
                TrappingIntegerOperation::Divide { left, right }
            }
            5 => {
                let (left, right) = binary(reader)?;
                TrappingIntegerOperation::Remainder { left, right }
            }
            6 => {
                let (value, count) = binary(reader)?;
                TrappingIntegerOperation::ShiftLeft { value, count }
            }
            7 => {
                let (value, count) = binary(reader)?;
                TrappingIntegerOperation::ShiftRight { value, count }
            }
            8 => TrappingIntegerOperation::Convert {
                operand: reader.id("ValueId")?,
            },
            tag => return Err(CodecError::InvalidTag("TrappingIntegerOperation", tag)),
        };
        Ok(OperationKind::TrappingInteger { operation })
    }

    pub(crate) fn encode_float_result_policy(
        writer: &mut Writer,
        policy: &FloatResultPolicy,
    ) -> Result<(), CodecError> {
        writer.u8(operation_tags::FLOAT_RESULT_POLICY);
        match policy {
            FloatResultPolicy::Trapping { raw_result } => {
                writer.u8(1);
                writer.id(*raw_result);
            }
            FloatResultPolicy::Saturating {
                raw_result,
                eligible,
            } => {
                writer.u8(4);
                writer.id(*raw_result);
                writer.id(*eligible);
            }
        }
        Ok(())
    }
    pub(crate) fn decode_float_result_policy(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        let policy = match reader.u8()? {
            1 => FloatResultPolicy::Trapping {
                raw_result: reader.id("ValueId")?,
            },
            4 => FloatResultPolicy::Saturating {
                raw_result: reader.id("ValueId")?,
                eligible: reader.id("ValueId")?,
            },
            tag => return Err(CodecError::InvalidTag("FloatResultPolicy", tag)),
        };
        Ok(OperationKind::FloatResultPolicy { policy })
    }
}
pub(crate) mod storage_operations {

    use super::super::CodecError;
    use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{Argument, Field, OptionalField};
    use super::operation_tags;
    use crate::sections::semantic_module::structural_place_wire::{
        decode_structural_path, encode_structural_path,
    };
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    use semantic_vocabulary::{ObligationId, PlaceId, StructuralFieldId, ValueId};
    use terminal_psi::OperationKind;
    use terminal_psi::ReferenceStorageView;
    use terminal_psi::{StructuralArgument, StructuralPathSegment};

    pub(crate) fn encode_establish_reference(
        writer: &mut Writer,
        source: StructuralArgument,
        storage: Option<ReferenceStorageView>,
    ) -> Result<(), CodecError> {
        writer.u8(if storage.is_some() {
            operation_tags::ESTABLISH_STORAGE_VIEW_REFERENCE
        } else {
            operation_tags::ESTABLISH_REFERENCE
        });
        writer.id(source.place);
        super::super::structural_signature_wire::encode_structural_access(writer, source.access);
        encode_structural_path(writer, "reference source path", &source.path)?;
        if let Some(storage) = storage {
            writer.id(storage.byte_offset);
            writer.boolean(storage.stored_scalar.is_some());
            if let Some(stored_scalar) = storage.stored_scalar {
                encode_scalar_type(writer, stored_scalar)?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode_establish_reference(
        reader: &mut Reader<'_>,
        has_storage_view: bool,
    ) -> Result<OperationKind, CodecError> {
        Ok(OperationKind::EstablishReference {
            source: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            storage: if has_storage_view {
                Some(ReferenceStorageView {
                    byte_offset: reader.id("ValueId")?,
                    stored_scalar: if reader.boolean()? {
                        Some(decode_scalar_type(reader)?)
                    } else {
                        None
                    },
                })
            } else {
                None
            },
        })
    }

    pub(crate) fn encode_release_reference(
        writer: &mut Writer,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    ) -> Result<(), CodecError> {
        writer.u8(if path.is_empty() {
            operation_tags::RELEASE_REFERENCE
        } else {
            operation_tags::PROJECTED_RELEASE_REFERENCE
        });
        writer.id(source);
        if !path.is_empty() {
            encode_structural_path(writer, "reference release path", &path)?;
        }
        Ok(())
    }

    pub(crate) fn decode_release_reference(
        reader: &mut Reader<'_>,
        projected: bool,
    ) -> Result<OperationKind, CodecError> {
        let source = reader.id("PlaceId")?;
        let path = if projected {
            decode_structural_path(reader)?
        } else {
            Vec::new()
        };
        if projected && path.is_empty() {
            return crate::codec_error::malformed("projected release requires a nonempty path");
        }
        Ok(OperationKind::ReleaseReference { source, path })
    }

    pub(crate) fn encode_primitive_scalar_read(
        writer: &mut Writer,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    ) -> Result<(), CodecError> {
        if path.is_empty() {
            writer.u8(operation_tags::PRIMITIVE_SCALAR_READ);
            writer.id(source);
        } else {
            writer.u8(operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ);
            writer.id(source);
            encode_structural_path(writer, "primitive source path", &path)?;
        }
        Ok(())
    }

    pub(crate) fn decode_projected_primitive_scalar_read(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        let source = reader.id("PlaceId")?;
        let path = decode_structural_path(reader)?;
        if path.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty projected primitive path",
            ));
        }
        Ok(OperationKind::PrimitiveScalarRead { source, path })
    }

    pub(crate) fn encode_structural_case_leaf_copy(
        writer: &mut Writer,
        source: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(), CodecError> {
        writer.u8(operation_tags::STRUCTURAL_CASE_LEAF_COPY);
        super::super::structural_field_wire::encode_canonical_structural_field(
            writer,
            source,
            path,
            "case leaf copy path",
        )
    }

    pub(crate) fn encode_element_view_read(
        writer: &mut Writer,
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
        path: Vec<StructuralPathSegment>,
    ) -> Result<(), CodecError> {
        writer.u8(if path.is_empty() {
            operation_tags::ELEMENT_VIEW_READ
        } else {
            operation_tags::PROJECTED_ELEMENT_VIEW_READ
        });
        writer.id(source);
        writer.id(index);
        writer.id(length);
        writer.id(obligation);
        if !path.is_empty() {
            encode_structural_path(writer, "element view read path", &path)?;
        }
        Ok(())
    }

    pub(crate) fn decode_projected_element_view_read(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        let source = reader.id("PlaceId")?;
        let index = reader.id("ValueId")?;
        let length = reader.id("ValueId")?;
        let obligation = reader.id("ObligationId")?;
        let path = decode_structural_path(reader)?;
        if path.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty projected element view read path",
            ));
        }
        Ok(OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        })
    }

    pub(crate) fn encode_write_only_primitive_store(
        writer: &mut Writer,
        destination: PlaceId,
        value: ValueId,
        path: Vec<StructuralPathSegment>,
        range_obligation: Option<ObligationId>,
    ) -> Result<(), CodecError> {
        if let Some(obligation) = range_obligation {
            writer.u8(operation_tags::RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE);
            writer.id(destination);
            encode_structural_path(writer, "restricted primitive destination path", &path)?;
            writer.id(value);
            writer.id(obligation);
            return Ok(());
        }
        if path.is_empty() {
            writer.u8(operation_tags::WRITE_ONLY_PRIMITIVE_STORE);
            writer.id(destination);
        } else {
            writer.u8(operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE);
            writer.id(destination);
            encode_structural_path(writer, "primitive destination path", &path)?;
        }
        writer.id(value);
        Ok(())
    }

    pub(crate) fn decode_projected_write_only_primitive_store(
        reader: &mut Reader<'_>,
    ) -> Result<OperationKind, CodecError> {
        let destination = reader.id("PlaceId")?;
        let path = decode_structural_path(reader)?;
        if path.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty projected primitive path",
            ));
        }
        Ok(OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value: reader.id("ValueId")?,
            range_obligation: None,
        })
    }

    pub(crate) fn encode_store_structural_field(
        writer: &mut Writer,
        destination: PlaceId,
        disposition: terminal_psi::StructuralStoreDisposition,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: StructuralArgument,
    ) -> Result<(), CodecError> {
        writer.u8(match disposition {
            terminal_psi::StructuralStoreDisposition::Vacant => {
                operation_tags::STORE_STRUCTURAL_FIELD
            }
            terminal_psi::StructuralStoreDisposition::ReplaceDiscardable => {
                operation_tags::REPLACE_DISCARDABLE_STRUCTURAL_FIELD
            }
        });
        writer.id(destination);
        encode_structural_path(writer, "structural field store path", &path)?;
        OptionalField::write(writer, &field)?;
        Argument::write(writer, &value)
    }

    pub(crate) fn decode_store_structural_field(
        reader: &mut Reader<'_>,
        disposition: terminal_psi::StructuralStoreDisposition,
    ) -> Result<OperationKind, CodecError> {
        Ok(OperationKind::StoreStructuralField {
            disposition,
            destination: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
            field: OptionalField::read(reader)?,
            value: Argument::read(reader)?,
        })
    }
}
pub(crate) mod terminator_wire {

    use super::super::CodecError;
    use super::super::contract_wire::{CrashPredicate, Successor};
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{
        Arguments, ClaimBinding, ClaimTransfers, CleanupAction, CrashRoutes, Field, Id, Ids, List,
        Optional, Path, ProofTerms, ScalarTerms, U32, wire_enum, wire_record,
    };
    use terminal_psi::{ExternalCompletionKind, StructuralCaseSuccessorEdge, Terminator};

    wire_record! {
        CaseSuccessor: StructuralCaseSuccessorEdge {
            edge: Id,
            target: Id,
            case: Id,
            payload_fields: Ids,
            structural_arguments: Arguments,
            cleanup_actions: List<CleanupAction>,
        }
    }

    wire_enum! {
        TerminatorRow: Terminator, "Terminator" {
            19 => InvokeNoNormal {
                edge: Id,
                static_reach_binding: Optional<U32>,
                callee: Id,
                arguments: Ids,
                erased_arguments: ScalarTerms,
                erased_proof_arguments: ProofTerms,
                structural_arguments: Arguments,
                claim_transfers: ClaimTransfers,
                requirement_obligations: Ids,
                crash_continuations: CrashRoutes,
            },
            17 => ExternalCompletion {
                edge: Id,
                boundary: Id,
                kind: CompletionKind,
                arguments: Ids,
                structural_arguments: Arguments,
                requirement_obligations: Ids,
                frontier_lower_bound: Ids,
            },
            14 => Jump {
                edge: Id,
                target: Id,
                arguments: Ids,
                erased_arguments: ScalarTerms,
                erased_proof_arguments: ProofTerms,
                structural_arguments: Arguments,
                cleanup_actions: List<CleanupAction>,
            },
            2 => Return { edge: Id, value: Id, cleanup_actions: List<CleanupAction> },
            13 => ReturnUnit { edge: Id, cleanup_actions: List<CleanupAction> },
            18 => ReturnStructural {
                edge: Id,
                source: Id,
                returned_claims: List<ClaimBinding>,
                cleanup_actions: List<CleanupAction>,
            },
            15 => Conditional { condition: Id, when_true: Successor, when_false: Successor },
            16 => StructuralCase { source: Id, path: Path, cases: List<CaseSuccessor> },
            4 => Crash {
                edge: Id,
                cause: super::super::contract_wire::Cause,
                site_guard: List<CrashPredicate>,
                frontier_lower_bound: Ids,
            },
        }
        CompletionKind: ExternalCompletionKind, "ExternalCompletionKind" { 1 => ProcessExit }
    }

    pub(crate) fn encode_terminator(
        writer: &mut Writer,
        terminator: &Terminator,
    ) -> Result<(), CodecError> {
        TerminatorRow::write(writer, terminator)
    }

    pub(crate) fn decode_terminator(reader: &mut Reader<'_>) -> Result<Terminator, CodecError> {
        TerminatorRow::read(reader)
    }
}

#[cfg(test)]
mod owned_value_tests {
    use super::{Reader, Writer, decode_operation, encode_operation};
    use semantic_vocabulary::{
        ClaimId, OperationId, PlaceId, StructuralDomainId, StructuralTypeId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralOperationResult, StructuralPathQualification,
        StructuralPathSegment, StructuralResultClaimBinding, TerminalIdentityIndex,
    };

    #[test]
    fn placed_reference_wire_preserves_owner_roles_and_backing_kind() {
        let source = StructuralArgument {
            place: PlaceId::new(2).unwrap(),
            access: StructuralAccess::MutableBorrow,
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        };
        let resident = terminal_psi::ResidentReferenceSource {
            base: semantic_vocabulary::StructuralFieldId::new(7).unwrap(),
            length: semantic_vocabulary::StructuralFieldId::new(8).unwrap(),
            granted: StructuralDomainId::new(9).unwrap(),
            resident: StructuralDomainId::new(10).unwrap(),
        };
        let mut encodings = Vec::new();
        for roles in [
            None,
            Some(resident),
            Some(terminal_psi::ResidentReferenceSource {
                base: resident.length,
                length: resident.base,
                ..resident
            }),
        ] {
            let kind = OperationKind::EstablishAccessReference {
                source: source.clone(),
                resident: roles,
            };
            let mut writer = Writer::default();
            super::encode_operation_kind(&mut writer, kind.clone()).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[0], super::operation_tags::ESTABLISH_ACCESS_REFERENCE);
            let mut reader = Reader::new(&bytes);
            assert_eq!(super::decode_operation_kind(&mut reader).unwrap(), kind);
            assert_eq!(reader.remaining(), 0);
            encodings.push(bytes);
        }
        assert_ne!(encodings[0], encodings[1]);
        assert_ne!(encodings[1], encodings[2]);
    }

    #[test]
    fn owned_value_wire_preserves_projected_source_and_claim_identity() {
        let path = vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))];
        let operation = Operation {
            id: OperationId::new(9).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::MoveStructuralValue {
                source: StructuralArgument {
                    place: PlaceId::new(2).unwrap(),
                    path,
                    access: StructuralAccess::Owned,
                },
            },
            result: OperationResult::Structural(StructuralOperationResult {
                place: PlaceId::new(3).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![StructuralDomainId::new(5).unwrap()],
                projected_qualifications: vec![StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(6))],
                    domain: StructuralDomainId::new(7).unwrap(),
                }],
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: vec![StructuralResultClaimBinding {
                    claim: ClaimId::new(8).unwrap(),
                    path: Vec::new(),
                }],
            }),
        };
        let mut writer = Writer::default();
        encode_operation(&mut writer, &operation).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_operation(&mut reader).unwrap(), operation);
        assert_eq!(reader.remaining(), 0);

        let mut copy = operation.clone();
        copy.kind = OperationKind::StructuralLeafCopy {
            source: PlaceId::new(2).unwrap(),
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        };
        let mut writer = Writer::default();
        encode_operation(&mut writer, &copy).unwrap();
        assert_ne!(
            bytes,
            writer.finish(),
            "copy and move must have distinct wire meanings"
        );
    }
}
#[cfg(test)]
mod unknown_tag_tests {

    use super::super::target_family_tests::unit_machine;
    use super::{Writer, encode_operation, operation_tags};
    use crate::{CodecError, decode_module, encode_module};
    use semantic_vocabulary::{
        MachineId, OperationId, ScalarQualificationSetId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, TerminalModule, ValueDeclaration,
    };

    const TAGS_WITHOUT_TABLE_ROWS: [u8; 19] = [
        0, 48, 49, 51, 52, 53, 54, 55, 57, 61, 67, 71, 72, 73, 74, 76, 80, 82, 87,
    ];

    fn module_with_one_operation() -> (TerminalModule, Vec<u8>, usize) {
        let machine_id = MachineId::new(1).unwrap();
        let operation = Operation {
            id: OperationId::new(1).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        };
        let mut machine = unit_machine(machine_id);
        machine.blocks[0].operations.push(operation.clone());
        let mut module = TerminalModule::for_entry(machine_id);
        module.machines.push(machine);
        let bytes = encode_module(&module).unwrap();

        let mut writer = Writer::default();
        encode_operation(&mut writer, &operation).unwrap();
        let row = writer.finish();
        let start = bytes
            .windows(row.len())
            .position(|window| window == row.as_slice())
            .expect("the encoded module contains the operation row");
        let tag_offset = start + row.len() - 3;
        assert_eq!(bytes[tag_offset], operation_tags::BOOLEAN_CONSTANT);
        (module, bytes, tag_offset)
    }

    #[test]
    fn an_operation_tag_without_a_table_row_rejects_the_module() {
        let (module, bytes, tag_offset) = module_with_one_operation();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let unknown = TAGS_WITHOUT_TABLE_ROWS.into_iter().chain(138..=u8::MAX);
        for tag in unknown {
            let mut changed = bytes.clone();
            changed[tag_offset] = tag;
            assert!(
                matches!(
                    decode_module(&changed),
                    Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
                ),
                "operation tag {tag} has no table row and must reject as an unknown operation tag"
            );
        }
    }

    #[test]
    fn unassigned_operation_tags_71_and_72_reject() {
        let (_, bytes, tag_offset) = module_with_one_operation();
        for tag in [71, 72] {
            let mut changed = bytes.clone();
            changed[tag_offset] = tag;
            assert!(matches!(
                decode_module(&changed),
                Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
            ));
        }
    }
}

use terminal_psi::{Block, Operation, OperationKind, OperationResult};

use super::CodecError;
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::proof_term_wire::{decode_proof_formals, encode_proof_formals};
use super::structural_result_wire::{decode_operation_result, encode_operation_result};
use super::structural_signature_wire::{
    decode_structural_parameters, encode_structural_parameters,
};
use super::wire::{Reader, Writer};
use super::wire_fields;
use crate::sections::semantic_module::wire::decode_counted;

pub(crate) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    encode_declarations(
        writer,
        "block erased scalar formals",
        &block.erased_scalar_formals,
    )?;
    encode_proof_formals(writer, &block.erased_proof_formals)?;
    encode_structural_parameters(writer, &block.structural_parameters)?;
    writer.counted("operations", &block.operations, encode_operation)?;
    terminator_wire::encode_terminator(writer, &block.terminator)
}

fn encode_operation(writer: &mut Writer, operation: &Operation) -> Result<(), CodecError> {
    writer.id(operation.id);
    writer.boolean(operation.static_reach_binding.is_some());
    if let Some(binder) = operation.static_reach_binding {
        writer.u32(binder);
    }
    match &operation.result {
        OperationResult::Unit => writer.u8(0),
        OperationResult::Scalar(result) => {
            writer.u8(1);
            encode_declaration(writer, *result)?;
        }
        OperationResult::Structural(result) => {
            writer.u8(2);
            encode_operation_result(writer, result)?;
        }
    }
    encode_operation_kind(writer, operation.kind.clone())?;
    writer.boolean(operation.suspension_crossing.is_some());
    if let Some(crossing) = operation.suspension_crossing {
        writer.id(crossing);
    }
    Ok(())
}

pub(crate) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let erased_scalar_formals = decode_declarations(reader)?;
    let erased_proof_formals = decode_proof_formals(reader)?;
    let structural_parameters = decode_structural_parameters(reader)?;
    let operations = decode_counted(reader, decode_operation)?;
    let terminator = terminator_wire::decode_terminator(reader)?;
    Ok(Block {
        id,
        parameters,
        erased_scalar_formals,
        erased_proof_formals,
        structural_parameters,
        operations,
        terminator,
    })
}

fn decode_operation(reader: &mut Reader<'_>) -> Result<Operation, CodecError> {
    let operation_id = reader.id("OperationId")?;
    let static_reach_binding = if reader.boolean()? {
        Some(reader.u32()?)
    } else {
        None
    };
    let result = match reader.u8()? {
        0 => OperationResult::Unit,
        1 => OperationResult::Scalar(decode_declaration(reader)?),
        2 => OperationResult::Structural(decode_operation_result(reader)?),
        tag => return Err(CodecError::InvalidTag("OperationResult", tag)),
    };
    let kind = decode_operation_kind(reader)?;
    let suspension_crossing = if reader.boolean()? {
        Some(reader.id("SuspensionCrossingId")?)
    } else {
        None
    };
    Ok(Operation {
        static_reach_binding,
        suspension_crossing,
        id: operation_id,
        result,
        kind,
    })
}

macro_rules! operation_wire {
    (
        rows { $($tag:ident => $variant:ident { $($field:ident: $kind:ident),* $(,)? }),* $(,)? }
        paired {
            $($absent:ident | $present:ident => $paired:ident {
                $($paired_field:ident: $paired_kind:ident),* ; $optional:ident
            }),* $(,)?
        }
        encode($writer:ident) { $($encode_pattern:pat => $encode_body:expr),* $(,)? }
        decode($reader:ident) { $($decode_tag:pat => $decode_body:expr),* $(,)? }
    ) => {
        fn encode_operation_kind(
            $writer: &mut Writer,
            kind: OperationKind,
        ) -> Result<(), CodecError> {
            match kind {
                $(OperationKind::$variant { $($field),* } => {
                    $writer.u8(operation_tags::$tag);
                    $(<wire_fields::$kind as wire_fields::Field<_>>::write($writer, &$field)?;)*
                })*
                $(OperationKind::$paired { $($paired_field,)* $optional } => {
                    $writer.u8(if $optional.is_some() {
                        operation_tags::$present
                    } else {
                        operation_tags::$absent
                    });
                    $(<wire_fields::$paired_kind as wire_fields::Field<_>>::write(
                        $writer,
                        &$paired_field,
                    )?;)*
                    if let Some(obligation) = $optional {
                        $writer.id(obligation);
                    }
                })*
                $($encode_pattern => {
                    $encode_body;
                })*
            }
            Ok(())
        }

        fn decode_operation_kind($reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
            Ok(match $reader.u8()? {
                $(operation_tags::$tag => OperationKind::$variant {
                    $($field: <wire_fields::$kind as wire_fields::Field<_>>::read($reader)?),*
                },)*
                $(operation_tags::$absent => OperationKind::$paired {
                    $($paired_field: <wire_fields::$paired_kind as wire_fields::Field<_>>::read($reader)?,)*
                    $optional: None,
                },
                operation_tags::$present => OperationKind::$paired {
                    $($paired_field: <wire_fields::$paired_kind as wire_fields::Field<_>>::read($reader)?,)*
                    $optional: Some($reader.id("ObligationId")?),
                },)*
                $($decode_tag => $decode_body,)*
                tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
            })
        }
    };
}

operation_wire! {
    rows {
        READ_SCALAR_APPLICATION => ReadScalarApplication { application: Id },
        ESTABLISH_NATIVE_CODE_LAYOUT => EstablishNativeCodeLayout { machine: Id },
        STORAGE_ADDRESS => StorageAddress { source: Argument },
        ACTIVATE_TASK_REGISTER => ActivateTaskRegister { service: Id, entries: U16, gdt: Argument, task_state: Argument, selector: Id, scratch: Argument },
        READ_TASK_REGISTER => ReadTaskRegister { service: Id },
        RELOAD_SEGMENT_DESCRIPTORS => ReloadSegmentDescriptors { service: Id, entries: U16, kind: SegmentReloadKind, gdt: Argument, selector: Id, scratch: Argument },
        READ_SEGMENT_SELECTOR => ReadSegmentSelector { service: Id, register: SegmentSelectorRegister },
        GLOBAL_DESCRIPTOR_TABLE => GlobalDescriptorTable { service: Id, operation: GlobalDescriptorTable, source: Argument },
        ESTABLISH_ACCESS_REFERENCE => EstablishAccessReference {
            source: Argument,
            resident: Resident,
        },
        ESTABLISH_PRIMITIVE_LOCAL => EstablishPrimitiveLocal {
            value: Id,
            range_obligation: OptionalId,
        },
        STRUCTURAL_CASE_MEMBERSHIP => StructuralCaseMembership { source: Id, path: Path, case: Id },
        STRUCTURAL_LEAF_COPY => StructuralLeafCopy { source: Id, path: Path },
        MOVE_STRUCTURAL_VALUE => MoveStructuralValue { source: Argument },
        BYTE_SEQUENCE_SUBSLICE => ByteSequenceSubslice {
            source: Argument,
            start: Id,
            end: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_WRITE => ByteSequenceWrite {
            destination: Id,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        ELEMENT_VIEW_WRITE => ElementViewWrite {
            destination: Argument,
            path: Path,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_READ => ByteSequenceRead {
            source: Id,
            index: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_LENGTH => ByteSequenceLength { source: Argument },
        BYTE_SEQUENCE_STORE => ByteSequenceStore {
            destination: Id,
            source: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE => StructuralByteSequenceFieldStore {
            destination: Id,
            path: Path,
            field: Id,
            source: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH => StructuralByteSequenceFieldLength {
            source: Id,
            path: Path,
            field: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE => StructuralByteSequenceFieldByteStore {
            destination: Id,
            path: Path,
            field: Id,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_READ => StructuralByteSequenceFieldRead {
            source: Id,
            path: Path,
            field: Id,
            index: Id,
            length: Id,
            obligation: Id,
        },
        MOVE_STRUCTURAL_FIELD => MoveStructuralField { source: Id, path: Path, field: OptionalField },
        ESTABLISH_STRUCTURAL_ARRAY => EstablishStructuralArray { elements: Arguments },
        ESTABLISH_ACTIVATION_CLAIM => EstablishActivationClaim {
            bound: U64,
            committed: Id,
            backing_type: Id,
            established_case: Id,
            rejected_case: Id,
        },
        SPLIT_ACTIVATION_CLAIM => SplitActivationClaim {
            source: Argument,
            claim: Id,
            bound_at: U64,
            committed_at: Id,
            covered_case: Id,
            rejected_case: Id,
        },
        MERGE_ACTIVATION_CLAIM => MergeActivationClaim {
            lower: Argument,
            upper: Argument,
            lower_claim: Id,
            upper_claim: Id,
            covered_case: Id,
            rejected_case: Id,
        },
        RELEASE_ACTIVATION_CLAIM => ReleaseActivationClaim { source: Argument, claim: Id },
        ESTABLISH_SCALAR_ARRAY => EstablishScalarArray { elements: Ids },
        ESTABLISH_SCALAR_CASE => EstablishScalarCase { result_case: Id, fields: ScalarCaseFields },
        ESTABLISH_BYTE_SEQUENCE_CONCATENATION => EstablishByteSequenceConcatenation {
            left: ByteOperand,
            right: ByteOperand,
            capacity_obligation: Id,
        },
        BYTE_SEQUENCE_PREDICATE => ByteSequencePredicate { operand: ByteOperand, predicate: Predicate },
        BYTE_SEQUENCE_EQUAL => ByteSequenceEqual { left: ByteOperand, right: ByteOperand },
        ESTABLISH_BYTE_SEQUENCE_LITERAL => EstablishByteSequenceLiteral {
            destination: Id,
            bytes: Bytes,
            qualifications: Ids,
        },
        ESTABLISH_ELEMENT_VIEW => EstablishElementView {
            destination: Id,
            source: Argument,
            element: Id,
            region: Region,
        },
        ESTABLISH_MUTABLE_ELEMENT_VIEW => EstablishMutableElementView {
            destination: Id,
            source: Argument,
            element: Id,
            region: Region,
        },
        ELEMENT_VIEW_LENGTH => ElementViewLength { source: Argument },
        ELEMENT_VIEW_SUBSLICE => ElementViewSubslice {
            source: Argument,
            start: Id,
            end: Id,
            length: Id,
            obligation: Id,
        },
        ESTABLISH_ZEROED_STRUCTURAL => EstablishZeroedStructural {},
        ESTABLISH_STRUCTURAL_LOCAL => EstablishStructuralLocal {},
        ESTABLISH_TRIVIAL_AFFINE_LOCAL => EstablishTrivialAffineLocal { destination: Id },
        ESTABLISH_RECORD => EstablishRecord { fields: RecordFields },
        ESTABLISH_STRUCTURAL_CASE => EstablishStructuralCase {
            result_case: Id,
            fields: RecordFields,
        },
        CALL => Call {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_UNIT => CallUnit {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_STRUCTURAL_SCALAR => CallStructuralScalar {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS => CallStructuralWithScalarArguments {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        CALL_STRUCTURAL => CallStructural {
            callee: Id,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        CALL_DYNAMIC => CallDynamic {
            selector: Id,
            interface: U32,
            slot: U32,
            arguments: Ids,
            family_actuals: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        BOUNDARY_CALL => BoundaryCall {
            boundary: Id,
            arguments: Ids,
            structural_arguments: Arguments,
            completion_receipts: CompletionReceipts,
            requirement_obligations: Ids,
        },
        SELECT_IMPLEMENTATION => SelectImplementation { table: U32 },
        PORT_WRITE => PortWrite { service: Id, port: U16, value: U8 },
        INTEGER_CONSTANT => IntegerConstant { value: Integer },
        BOOLEAN_CONSTANT => BooleanConstant { value: Flag },
        IEEE_FLOAT_CONSTANT => IeeeFloatConstant { value: Float },
        IEEE_FLOAT_COMPARE => IeeeFloatCompare {
            comparison: FloatComparison,
            left: Id,
            right: Id,
        },
        NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD => NearestIeeeFloatFusedMultiplyAdd {
            left: Id,
            right: Id,
            addend: Id,
        },
        BOOLEAN_STRUCTURAL_FIELD => BooleanStructuralField { source: Id, path: FieldPath, field: Id },
        SCALAR_STRUCTURAL_FIELD => ScalarStructuralField { source: Id, path: FieldPath, field: Id },
        BOOLEAN_NOT => BooleanNot { operand: Id },
        BOOLEAN_EQUAL => BooleanEqual { left: Id, right: Id },
        INTEGER_EQUAL => IntegerEqual { left: Id, right: Id },
        INTEGER_LESS_THAN => IntegerLessThan { left: Id, right: Id },
        INTEGER_LESS_OR_EQUAL => IntegerLessOrEqual { left: Id, right: Id },
        INTEGER_BITWISE_NOT => IntegerBitwiseNot { operand: Id },
        INTEGER_WIDEN => IntegerWiden { operand: Id },
        INTEGER_EXACT_CAST => IntegerExactCast { operand: Id, obligation: Id },
        INTEGER_BITWISE_AND => IntegerBitwiseAnd { left: Id, right: Id },
        INTEGER_BITWISE_OR => IntegerBitwiseOr { left: Id, right: Id },
        INTEGER_BITWISE_XOR => IntegerBitwiseXor { left: Id, right: Id },
        WRAPPING_INTEGER_SHIFT_LEFT => WrappingIntegerShiftLeft { value: Id, count: Id },
        WRAPPING_INTEGER_SHIFT_RIGHT => WrappingIntegerShiftRight { value: Id, count: Id },
        EXACT_INTEGER_SHIFT_LEFT => ExactIntegerShiftLeft { value: Id, count: Id, obligation: Id },
        EXACT_INTEGER_SHIFT_RIGHT => ExactIntegerShiftRight { value: Id, count: Id, obligation: Id },
        EXACT_INTEGER_ADD => ExactIntegerAdd { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_SUBTRACT => ExactIntegerSubtract { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_MULTIPLY => ExactIntegerMultiply { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_DIVIDE => ExactIntegerDivide { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_REMAINDER => ExactIntegerRemainder { left: Id, right: Id, obligation: Id },
        WRAPPING_INTEGER_DIVIDE => WrappingIntegerDivide { left: Id, right: Id, obligation: Id },
        WRAPPING_INTEGER_REMAINDER => WrappingIntegerRemainder { left: Id, right: Id, obligation: Id },
        SATURATING_INTEGER_DIVIDE => SaturatingIntegerDivide { left: Id, right: Id, obligation: Id },
        SATURATING_INTEGER_REMAINDER => SaturatingIntegerRemainder {
            left: Id,
            right: Id,
            obligation: Id,
        },
        WRAPPING_INTEGER_ADD => WrappingIntegerAdd { left: Id, right: Id },
        SATURATING_INTEGER_ADD => SaturatingIntegerAdd { left: Id, right: Id },
        WRAPPING_INTEGER_SUBTRACT => WrappingIntegerSubtract { left: Id, right: Id },
        SATURATING_INTEGER_SUBTRACT => SaturatingIntegerSubtract { left: Id, right: Id },
        WRAPPING_INTEGER_MULTIPLY => WrappingIntegerMultiply { left: Id, right: Id },
        SATURATING_INTEGER_MULTIPLY => SaturatingIntegerMultiply { left: Id, right: Id },
    }
    paired {
        STRUCTURAL_BYTE_VIEW_FIELD_STORE | PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE
            => StructuralByteViewFieldStore {
                destination: Id, path: Path, field: OptionalField, source: Id; predicate_obligation
            },
        STRUCTURAL_SCALAR_FIELD_STORE | RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE
            => StructuralScalarFieldStore {
                destination: Id, path: Path, field: Id, value: Id; range_obligation
            },
        SCALAR_REPRESENTATION_VIEW | LANDED_SCALAR_REPRESENTATION_VIEW
            => ScalarRepresentationView { operand: Id; landing_obligation },
    }
    encode(writer) {
        OperationKind::EstablishReference { source, storage } => {
            storage_operations::encode_establish_reference(writer, source, storage)?
        },
        OperationKind::ReleaseReference { source, path } => {
            storage_operations::encode_release_reference(writer, source, path)?
        },
        OperationKind::PrimitiveScalarRead { source, path } => {
            storage_operations::encode_primitive_scalar_read(writer, source, path)?
        },
        OperationKind::TransformContent {
            source,
            parents,
            derivations,
        } => {
            writer.u8(operation_tags::TRANSFORM_CONTENT);
            super::structural_place_wire::encode_structural_arguments(writer, &[source])?;
            super::structural_place_wire::encode_structural_arguments(writer, &parents)?;
            super::content_wire::encode_content_transformations(writer, &derivations)?;
        },
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            storage_operations::encode_structural_case_leaf_copy(writer, source, &path)?
        },
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            path,
            range_obligation,
        } => storage_operations::encode_write_only_primitive_store(
            writer,
            destination,
            value,
            path,
            range_obligation,
        )?,
        OperationKind::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
        } => storage_operations::encode_store_structural_field(
            writer,
            destination,
            disposition,
            path,
            field,
            value,
        )?,
        OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } => storage_operations::encode_element_view_read(
            writer, source, index, length, obligation, path,
        )?,
        OperationKind::RegisterAccess { service, event } => {
            call_operations::encode_register_access(writer, service, event)?
        },
        OperationKind::TrappingInteger { operation } => {
            scalar_operations::encode_trapping_integer(writer, operation)?
        },
        OperationKind::FloatResultPolicy { policy } => {
            scalar_operations::encode_float_result_policy(writer, &policy)?
        },
        OperationKind::AtomicFence { ordering } => {
            atomic_operations::encode_atomic_fence(writer, ordering)
        },
        OperationKind::AtomicAccess {
            place,
            path,
            event,
            resident_obligation,
        } => atomic_operations::encode_atomic_access(
            writer,
            place,
            &path,
            event,
            resident_obligation,
        )?,
    }
    decode(reader) {
        operation_tags::ESTABLISH_REFERENCE => {
            storage_operations::decode_establish_reference(reader, false)?
        },
        operation_tags::ESTABLISH_STORAGE_VIEW_REFERENCE => {
            storage_operations::decode_establish_reference(reader, true)?
        },
        operation_tags::RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, false)?
        },
        operation_tags::PROJECTED_RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, true)?
        },
        operation_tags::PRIMITIVE_SCALAR_READ => OperationKind::PrimitiveScalarRead {
            path: Vec::new(),
            source: reader.id("PlaceId")?,
        },
        operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_projected_primitive_scalar_read(reader)?
        },
        operation_tags::TRANSFORM_CONTENT => {
            let mut sources = super::structural_place_wire::decode_structural_arguments(reader)?;
            if sources.len() != 1 {
                return Err(CodecError::MalformedStructuralFoundation(
                    "content transformation requires one payload",
                ));
            }
            let source = sources.remove(0);
            OperationKind::TransformContent {
                source,
                parents: super::structural_place_wire::decode_structural_arguments(reader)?,
                derivations: super::content_wire::decode_content_transformations(reader)?,
            }
        },
        operation_tags::STRUCTURAL_CASE_LEAF_COPY => {
            let (source, path) =
                super::structural_field_wire::decode_canonical_structural_field(reader)?;
            OperationKind::StructuralCaseLeafCopy { source, path }
        },
        operation_tags::WRITE_ONLY_PRIMITIVE_STORE => OperationKind::WriteOnlyPrimitiveStore {
            path: Vec::new(),
            destination: reader.id("PlaceId")?,
            value: reader.id("ValueId")?,
            range_obligation: None,
        },
        operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_projected_write_only_primitive_store(reader)?
        },
        operation_tags::RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE => {
            OperationKind::WriteOnlyPrimitiveStore {
                destination: reader.id("PlaceId")?,
                path: super::structural_place_wire::decode_structural_path(reader)?,
                value: reader.id("ValueId")?,
                range_obligation: Some(reader.id("ObligationId")?),
            }
        },
        operation_tags::STORE_STRUCTURAL_FIELD => storage_operations::decode_store_structural_field(
            reader,
            terminal_psi::StructuralStoreDisposition::Vacant,
        )?,
        operation_tags::REPLACE_DISCARDABLE_STRUCTURAL_FIELD => {
            storage_operations::decode_store_structural_field(
                reader,
                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            )?
        },
        operation_tags::ELEMENT_VIEW_READ => OperationKind::ElementViewRead {
            source: reader.id("PlaceId")?,
            index: reader.id("ValueId")?,
            length: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
            path: Vec::new(),
        },
        operation_tags::PROJECTED_ELEMENT_VIEW_READ => {
            storage_operations::decode_projected_element_view_read(reader)?
        },
        operation_tags::REGISTER_ACCESS => call_operations::decode_register_access(reader, true)?,
        operation_tags::REGISTER_OBSERVATION => {
            call_operations::decode_register_access(reader, false)?
        },
        operation_tags::TRAPPING_INTEGER => scalar_operations::decode_trapping_integer(reader)?,
        operation_tags::FLOAT_RESULT_POLICY => {
            scalar_operations::decode_float_result_policy(reader)?
        },
        operation_tags::ATOMIC_FENCE => atomic_operations::decode_atomic_fence(reader)?,
        operation_tags::ATOMIC_ACCESS => atomic_operations::decode_atomic_access(reader, false)?,
        operation_tags::RESTRICTED_ATOMIC_ACCESS => {
            atomic_operations::decode_atomic_access(reader, true)?
        },
    }
}
