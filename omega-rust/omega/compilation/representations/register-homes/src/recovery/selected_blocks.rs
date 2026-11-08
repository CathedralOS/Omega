/*
We encode and decode `SelectedBlock`, `SelectedTerminator`, `SelectedSuccessor`,
and `SelectedStructuralCaseEdge` rows inside the `FixedViewCopyPlan` selected
payload.

Each block carries its origin (`Source`, `EdgeTransfer`, `CaseDispatch`, or
`Operation`), its instruction slice, and its terminator. `encode_terminator` is
delegated to `selected_instructions::terminator_encoding::encode_terminator` so
the wire tag layout is stated once in the crate that owns `SelectedTerminator`.
On decode, we verify that `OperationContinuation` successors omit a source
`psi_edge` while semantic/transfer/case successors require one, and that
`ScalarReference`, `WholeValue`, and `Address` structural transports target
valid `StructuralBlockParameter` slots with non-zero aligned extents.
*/

use ::selected_instructions::{
    SelectedAddressBase, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedScalarReferenceSource, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport, VirtualRegisterId,
};
use abstract_operations::ValueBinding;
use semantic_vocabulary::{BlockId, EdgeId, OperationId, ValueId};

use crate::FixedViewCopyDecodeError;

use super::fixed_view_copy_codec::{
    Cursor, decode_definition_site, decode_id, decode_list, decode_scalar, decode_work,
    encode_scalar, encode_work, length,
};
use super::selected_instructions::{decode_instruction, encode_instruction};
use super::structural_contracts::{
    decode_cleanup, decode_local_slot, decode_runtime_indices, decode_semantic_argument,
    encode_runtime_indices, encode_semantic_argument,
};

pub(super) fn encode_block(bytes: &mut Vec<u8>, block: &SelectedBlock) {
    crate::codec_bytes::push_encoded(bytes, block.id.0.to_le_bytes());
    match block.origin {
        SelectedBlockOrigin::Source(source) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
        }
        SelectedBlockOrigin::Operation { source, operation } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        SelectedBlockOrigin::CaseDispatch {
            source,
            case_ordinal,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, case_ordinal.to_le_bytes());
        }
        SelectedBlockOrigin::EdgeTransfer { edge, target } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, target.get().to_le_bytes());
        }
    }
    length(bytes, block.instructions.len());
    for instruction in &block.instructions {
        encode_instruction(bytes, instruction);
    }
    ::selected_instructions::terminator_encoding::encode_terminator(
        bytes,
        &block.terminator,
        encode_instruction,
        encode_successor,
    );
}

pub(super) fn decode_block(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedBlock, FixedViewCopyDecodeError> {
    let id = SelectedBlockId(cursor.u32()?);
    let origin = match cursor.byte()? {
        0 => SelectedBlockOrigin::Source(decode_id(cursor, BlockId::new)?),
        1 => SelectedBlockOrigin::EdgeTransfer {
            edge: decode_id(cursor, EdgeId::new)?,
            target: decode_id(cursor, BlockId::new)?,
        },
        2 => SelectedBlockOrigin::CaseDispatch {
            source: decode_id(cursor, BlockId::new)?,
            case_ordinal: cursor.u32()?,
        },
        3 => SelectedBlockOrigin::Operation {
            source: decode_id(cursor, BlockId::new)?,
            operation: decode_id(cursor, OperationId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownBlockOrigin(tag)),
    };
    let instructions = decode_list(cursor, decode_instruction)?;
    let terminator = match cursor.byte()? {
        9 => SelectedTerminator::OperationInvokeNoNormal {
            instruction: decode_instruction(cursor)?,
            operation: decode_id(cursor, OperationId::new)?,
        },
        7 => SelectedTerminator::OperationTrap {
            instruction: decode_instruction(cursor)?,
            operation: decode_id(cursor, OperationId::new)?,
        },
        6 => {
            let instruction = decode_instruction(cursor)?;
            let psi_edge = decode_id(cursor, EdgeId::new)?;
            let cause = match cursor.byte()? {
                1 => terminal_psi::CrashCause::Trap,
                2 => terminal_psi::CrashCause::Abort,
                tag => return Err(FixedViewCopyDecodeError::UnknownCrashCause(tag)),
            };
            let site_guard = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
                let length = cursor.length()?;
                let proposition =
                    terminal_codec::decode_canonical_proposition(cursor.take(length)?)
                        .map_err(|_| FixedViewCopyDecodeError::InvalidCrashPredicate)?;
                Ok(terminal_psi::CrashPredicateTerm::new(proposition))
            })?;
            let frontier_lower_bound = decode_list(cursor, |cursor| {
                decode_id(cursor, semantic_vocabulary::ClaimId::new)
            })?;
            SelectedTerminator::Crash {
                instruction,
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            }
        }
        8 => SelectedTerminator::InvokeNoNormal {
            instruction: decode_instruction(cursor)?,
            psi_edge: decode_id(cursor, EdgeId::new)?,
        },
        5 => SelectedTerminator::HostedExitProcess {
            instruction: decode_instruction(cursor)?,
            completion_edge: decode_id(cursor, EdgeId::new)?,
        },
        4 => SelectedTerminator::Jump {
            instruction: decode_instruction(cursor)?,
            successor: decode_successor(cursor)?,
        },
        0 => SelectedTerminator::ConditionalBranch {
            instruction: decode_instruction(cursor)?,
            when_nonzero: decode_successor(cursor)?,
            when_zero: decode_successor(cursor)?,
        },
        1 => SelectedTerminator::Return {
            instruction: decode_instruction(cursor)?,
            psi_return_edge: decode_id(cursor, EdgeId::new)?,
        },
        2 => SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        3 => SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownTerminator(tag)),
    };
    Ok(SelectedBlock {
        id,
        origin,
        instructions,
        terminator,
    })
}

pub(super) fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        SelectedSuccessorRole::Semantic => 0,
        SelectedSuccessorRole::EdgeTransferContinuation => 1,
        SelectedSuccessorRole::CaseDispatchContinuation => 2,
        SelectedSuccessorRole::OperationContinuation { .. } => 3,
    });
    if let SelectedSuccessorRole::OperationContinuation { operation } = successor.role {
        crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
    }
    crate::codec_bytes::push_encoded(
        bytes,
        successor
            .psi_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, successor.block.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, successor.source_target.get().to_le_bytes());
    length(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, binding.semantic.argument.get().to_le_bytes());
        encode_scalar(bytes, binding.semantic.scalar_type);
        match binding.transport {
            SelectedValueTransport::Unused => bytes.push(0),
            SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, parameter.0.to_le_bytes());
            }
        }
    }
    match &successor.structural_case {
        None => bytes.push(0),
        Some(case) => {
            bytes.push(1);
            case.encode_identity(bytes);
        }
    }
    length(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        encode_runtime_indices(bytes, &binding.source_indices);
        crate::codec_bytes::push_encoded(bytes, binding.semantic.parameter.get().to_le_bytes());
        encode_semantic_argument(bytes, &binding.semantic.argument);
        match binding.transport {
            ::selected_instructions::SelectedStructuralTransport::Unused => bytes.push(0),
            ::selected_instructions::SelectedStructuralTransport::ScalarReference {
                base,
                byte_offset,
                source,
                destination,
            } => {
                bytes.push(4);
                match base {
                    SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                match source {
                    SelectedScalarReferenceSource::Canonical { codec } => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, codec.to_le_bytes());
                    }
                    SelectedScalarReferenceSource::Descriptor => bytes.push(1),
                }
                destination.encode_identity(bytes);
            }
            ::selected_instructions::SelectedStructuralTransport::WholeValue {
                argument,
                source_byte_offset,
                destination,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, argument.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, source_byte_offset.to_le_bytes());
                destination.encode_identity(bytes);
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
            ::selected_instructions::SelectedStructuralTransport::Address {
                base,
                byte_offset,
                byte_count,
                destination,
            } => {
                bytes.push(3);
                match base {
                    SelectedAddressBase::Register(register) => {
                        bytes.push(0);
                        crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                    }
                    SelectedAddressBase::Local(slot) => {
                        bytes.push(1);
                        slot.encode_identity(bytes);
                    }
                }
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_count.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_work(bytes, &successor.work);
}

pub(super) fn decode_successor(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedSuccessor, FixedViewCopyDecodeError> {
    let role = match cursor.byte()? {
        0 => SelectedSuccessorRole::Semantic,
        1 => SelectedSuccessorRole::EdgeTransferContinuation,
        2 => SelectedSuccessorRole::CaseDispatchContinuation,
        3 => SelectedSuccessorRole::OperationContinuation {
            operation: decode_id(cursor, OperationId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownSuccessorRole(tag)),
    };
    let psi_edge = EdgeId::new(cursor.u64()?);
    if psi_edge.is_none() != matches!(role, SelectedSuccessorRole::OperationContinuation { .. }) {
        return Err(FixedViewCopyDecodeError::InvalidSuccessorLineage);
    }
    let block = SelectedBlockId(cursor.u32()?);
    let source_target = decode_id(cursor, BlockId::new)?;
    let bindings = decode_list(cursor, |cursor| {
        let semantic = ValueBinding {
            parameter: decode_id(cursor, ValueId::new)?,
            argument: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => SelectedValueTransport::Unused,
            1 => SelectedValueTransport::Registers {
                argument: VirtualRegisterId(cursor.u32()?),
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        Ok(SelectedValueBinding {
            semantic,
            transport,
        })
    })?;
    let structural_case = decode_case(cursor)?;
    let structural_bindings = decode_list(cursor, |cursor| {
        let source_indices = decode_runtime_indices(cursor)?;
        let semantic = abstract_operations::AbstractStructuralBinding {
            parameter: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            argument: decode_semantic_argument(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => ::selected_instructions::SelectedStructuralTransport::Unused,
            4 => {
                let base = match cursor.byte()? {
                    0 => SelectedAddressBase::Register(VirtualRegisterId(cursor.u32()?)),
                    1 => SelectedAddressBase::Local(decode_local_slot(cursor)?),
                    _ => return Err(FixedViewCopyDecodeError::InvalidStructuralTransport),
                };
                let byte_offset = cursor.u32()?;
                let source = match cursor.byte()? {
                    0 => {
                        let codec = cursor.u64()?;
                        if runtime_abi::ScalarStorageCodec::decode(codec).is_none() {
                            return Err(FixedViewCopyDecodeError::InvalidStructuralTransport);
                        }
                        SelectedScalarReferenceSource::Canonical { codec }
                    }
                    1 => SelectedScalarReferenceSource::Descriptor,
                    _ => return Err(FixedViewCopyDecodeError::InvalidStructuralTransport),
                };
                let destination = decode_local_slot(cursor)?;
                if !matches!(
                    destination,
                    ::selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }
                ) {
                    return Err(FixedViewCopyDecodeError::InvalidStructuralTransport);
                }
                ::selected_instructions::SelectedStructuralTransport::ScalarReference {
                    base,
                    byte_offset,
                    source,
                    destination,
                }
            }
            2 => {
                let argument = VirtualRegisterId(cursor.u32()?);
                let source_byte_offset = cursor.u32()?;
                let destination = decode_local_slot(cursor)?;
                let byte_size = cursor.u32()?;
                let alignment = cursor.u16()?;
                if byte_size == 0
                    || !alignment.is_power_of_two()
                    || byte_size % u32::from(alignment) != 0
                    || source_byte_offset % u32::from(alignment) != 0
                    || source_byte_offset.checked_add(byte_size).is_none()
                    || !matches!(
                        destination,
                        ::selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }
                    )
                {
                    return Err(FixedViewCopyDecodeError::InvalidStructuralTransport);
                }
                ::selected_instructions::SelectedStructuralTransport::WholeValue {
                    argument,
                    source_byte_offset,
                    destination,
                    byte_size,
                    alignment,
                }
            }
            3 => {
                let base = match cursor.byte()? {
                    0 => SelectedAddressBase::Register(VirtualRegisterId(cursor.u32()?)),
                    1 => SelectedAddressBase::Local(decode_local_slot(cursor)?),
                    _ => return Err(FixedViewCopyDecodeError::InvalidStructuralTransport),
                };
                let byte_offset = cursor.u32()?;
                let byte_count = cursor.u32()?;
                let destination = decode_local_slot(cursor)?;
                if byte_count == 0
                    || !matches!(
                        destination,
                        ::selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }
                    )
                {
                    return Err(FixedViewCopyDecodeError::InvalidStructuralTransport);
                }
                ::selected_instructions::SelectedStructuralTransport::Address {
                    base,
                    byte_offset,
                    byte_count,
                    destination,
                }
            }
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        Ok(::selected_instructions::SelectedStructuralBinding {
            semantic,
            transport,
            source_indices,
        })
    })?;
    Ok(SelectedSuccessor {
        role,
        psi_edge,
        block,
        source_target,
        bindings,
        structural_bindings,
        structural_case,
        work: decode_work(cursor)?,
    })
}

fn decode_case(
    cursor: &mut Cursor<'_>,
) -> Result<Option<::selected_instructions::SelectedStructuralCaseEdge>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => return Ok(None),
        1 => {}
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
    let source = match cursor.byte()? {
        0 => ::selected_instructions::SelectedCaseDispatchSource::Local {
            slot: decode_local_slot(cursor)?,
        },
        1 => ::selected_instructions::SelectedCaseDispatchSource::Borrowed {
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            pointer: VirtualRegisterId(cursor.u32()?),
            byte_size: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
    };
    let case = decode_id(cursor, semantic_vocabulary::StructuralCaseId::new)?;
    let case_tag = cursor.i32()?;
    let payloads = decode_list(cursor, |cursor| {
        let field = decode_id(cursor, semantic_vocabulary::StructuralFieldId::new)?;
        let field_byte_offset = cursor.u32()?;
        let parameter = legalized_operations::LegalizedValueDefinition {
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
            definition_site: decode_definition_site(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => ::selected_instructions::SelectedCasePayloadTransport::Unused,
            1 => ::selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            2 => ::selected_instructions::SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(cursor.u32()?),
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        Ok(::selected_instructions::SelectedCasePayloadBinding {
            semantic: legalized_operations::LegalizedStructuralCasePayload {
                field,
                field_byte_offset,
                parameter,
            },
            transport,
        })
    })?;
    let cleanup_actions = decode_list(cursor, decode_cleanup)?;
    Ok(Some(::selected_instructions::SelectedStructuralCaseEdge {
        source,
        case,
        case_tag,
        payloads,
        cleanup_actions,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        BlockId, Cursor, EdgeId, SelectedBlockId, SelectedBlockOrigin, SelectedSuccessor,
        SelectedSuccessorRole, SelectedValueBinding, SelectedValueTransport, ValueBinding, ValueId,
        VirtualRegisterId, decode_block, decode_instruction, decode_successor, encode_block,
        encode_successor,
    };
    use crate::FixedViewCopyDecodeError;
    use crate::recovery::codec_tests::function_with_operand;
    use optimization_core::AcceptedObligationFactIdentity;
    use semantic_vocabulary::{IntegerType, ScalarType};

    fn successor() -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1),
            block: SelectedBlockId(2),
            source_target: BlockId::new(3).unwrap(),
            bindings: vec![SelectedValueBinding {
                semantic: ValueBinding {
                    parameter: ValueId::new(4).unwrap(),
                    argument: ValueId::new(5).unwrap(),
                    scalar_type: ScalarType::Boolean,
                },
                transport: SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(17),
                    parameter: VirtualRegisterId(29),
                },
            }],
            work: vec![],
        }
    }

    #[test]
    fn successor_scalar_reference_preserves_both_storage_interpretations() {
        use ::selected_instructions::{
            LocalStorageSlotId, SelectedAddressBase, SelectedScalarReferenceSource,
            SelectedStructuralBinding, SelectedStructuralTransport,
        };
        use semantic_vocabulary::PlaceId;
        let mut original = successor();
        let destination = LocalStorageSlotId::StructuralBlockParameter {
            block: original.source_target,
            place: PlaceId::new(31).unwrap(),
        };
        original
            .structural_bindings
            .push(SelectedStructuralBinding {
                source_indices: vec![legalized_operations::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: ValueId::new(14).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                                .unwrap(),
                        ),
                    },
                    stride: 16,
                    extent: None,
                    obligation: semantic_vocabulary::ObligationId::new(15).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([16; 32]),
                }],
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(31).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(9).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    },
                },
                transport: SelectedStructuralTransport::Unused,
            });
        for (base, source) in [
            (
                SelectedAddressBase::Register(VirtualRegisterId(41)),
                SelectedScalarReferenceSource::Canonical { codec: 257 },
            ),
            (
                SelectedAddressBase::Local(destination),
                SelectedScalarReferenceSource::Descriptor,
            ),
        ] {
            original.structural_bindings[0].transport =
                SelectedStructuralTransport::ScalarReference {
                    base,
                    byte_offset: 0,
                    source,
                    destination,
                };
            let mut bytes = Vec::new();
            encode_successor(&mut bytes, &original);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_successor(&mut cursor).unwrap(), original);
            assert_eq!(cursor.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode_successor(&mut Cursor::new(&bytes[..length])).is_err());
            }
        }
        original.structural_bindings[0].transport = SelectedStructuralTransport::ScalarReference {
            base: SelectedAddressBase::Register(VirtualRegisterId(41)),
            byte_offset: 0,
            source: SelectedScalarReferenceSource::Canonical { codec: 9 },
            destination,
        };
        let mut invalid = Vec::new();
        encode_successor(&mut invalid, &original);
        assert_eq!(
            decode_successor(&mut Cursor::new(&invalid)),
            Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
        );
    }

    #[test]
    fn successor_register_transport_has_exact_canonical_bytes() {
        let original = successor();
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        let golden = "0001000000000000000200000003000000000000000100000000000000040000000000000005000000000000000001110000001d0000000000000000000000000000000000000000";
        let expected = (0..golden.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&golden[offset..offset + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(encoded, expected);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
        let mut unused = original;
        unused.bindings[0].transport = SelectedValueTransport::Unused;
        let mut unused_encoded = Vec::new();
        encode_successor(&mut unused_encoded, &unused);
        assert_eq!(unused_encoded.len(), encoded.len() - 8);
        assert_eq!(unused_encoded[46], 0);
        assert_eq!(
            decode_successor(&mut Cursor::new(&unused_encoded)).unwrap(),
            unused
        );
    }

    #[test]
    fn operation_trap_codec_retains_its_owner_without_an_authored_crash_edge() {
        use ::selected_instructions::{
            SelectedBlock, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedTerminator,
        };
        use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
        let operation = semantic_vocabulary::OperationId::new(9).unwrap();
        let mut block = SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Operation {
                source: BlockId::new(1).unwrap(),
                operation,
            },
            instructions: Vec::new(),
            terminator: SelectedTerminator::OperationTrap {
                operation,
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(3),
                    kind: SelectedInstructionKind::Crash,
                    constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 780,
                    },
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance {
                        operations: vec![operation],
                        ..Default::default()
                    },
                },
            },
        };
        let mut encoded = Vec::new();
        encode_block(&mut encoded, &block);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_block(&mut cursor).unwrap(), block);
        assert_eq!(cursor.remaining(), 0);
        let SelectedTerminator::OperationTrap { operation, .. } = &mut block.terminator else {
            panic!("trap");
        };
        *operation = semantic_vocabulary::OperationId::new(10).unwrap();
        let mut changed = Vec::new();
        encode_block(&mut changed, &block);
        assert_ne!(encoded, changed);
    }

    #[test]
    fn operation_invocation_codec_keeps_nonreturn_and_exact_source_operation() {
        use ::selected_instructions::{
            SelectedBlock, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedTerminator,
        };
        use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
        let operation = semantic_vocabulary::OperationId::new(9).unwrap();
        let mut block = SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Operation {
                source: BlockId::new(1).unwrap(),
                operation,
            },
            instructions: Vec::new(),
            terminator: SelectedTerminator::OperationInvokeNoNormal {
                operation,
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(3),
                    kind: SelectedInstructionKind::InvokeNoNormal {
                        target: ::selected_instructions::SelectedCallTarget::Machine(
                            semantic_vocabulary::MachineId::new(4).unwrap(),
                        ),
                    },
                    constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 0,
                    },
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance {
                        operations: vec![operation],
                        ..Default::default()
                    },
                },
            },
        };
        let mut encoded = Vec::new();
        encode_block(&mut encoded, &block);
        let mut cursor = Cursor::new(&encoded);
        let decoded = decode_block(&mut cursor).unwrap();
        assert_eq!(decoded, block);
        assert_eq!(cursor.remaining(), 0);
        assert!(::selected_instructions::terminator_successors(&decoded.terminator).is_empty());
        let SelectedTerminator::OperationInvokeNoNormal { operation, .. } = &mut block.terminator
        else {
            panic!("work call");
        };
        *operation = semantic_vocabulary::OperationId::new(10).unwrap();
        let mut changed = Vec::new();
        encode_block(&mut changed, &block);
        assert_ne!(encoded, changed);
    }

    #[test]
    fn crash_block_codec_retains_exact_cause_guards_frontier_and_work() {
        use ::selected_instructions::{
            SelectedBlock, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedTerminator,
        };
        use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
        use semantic_vocabulary::{ClaimId, Proposition};
        use terminal_psi::{CrashCause, CrashPredicateTerm};

        for cause in [CrashCause::Trap, CrashCause::Abort] {
            let edge = EdgeId::new(7).unwrap();
            let block = SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Crash {
                    instruction: SelectedInstruction {
                        id: SelectedInstructionId(0),
                        kind: SelectedInstructionKind::Crash,
                        constraint: RegisterConstraintKey {
                            family: RegisterConstraintFamily::Instruction,
                            variant: 780,
                        },
                        operands: Vec::new(),
                        implicit_uses: Vec::new(),
                        implicit_defs: Vec::new(),
                        clobbers: Vec::new(),
                        provenance: SelectedInstructionProvenance {
                            edges: vec![edge],
                            work: vec![optimization_unit::WorkSettlement {
                                site: optimization_unit::PsiProvenance::Edge(edge),
                                units: 2,
                            }],
                            ..Default::default()
                        },
                    },
                    psi_edge: edge,
                    cause,
                    site_guard: vec![CrashPredicateTerm::new(Proposition::Truth)],
                    frontier_lower_bound: vec![ClaimId::new(3).unwrap(), ClaimId::new(5).unwrap()],
                },
            };
            let mut encoded = Vec::new();
            encode_block(&mut encoded, &block);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_block(&mut cursor).unwrap(), block);
            assert_eq!(cursor.remaining(), 0);
            for mutation in 0..4 {
                let mut changed = block.clone();
                let SelectedTerminator::Crash {
                    psi_edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } = &mut changed.terminator
                else {
                    unreachable!()
                };
                match mutation {
                    0 => *psi_edge = EdgeId::new(9).unwrap(),
                    1 => {
                        *cause = if *cause == CrashCause::Trap {
                            CrashCause::Abort
                        } else {
                            CrashCause::Trap
                        }
                    }
                    2 => site_guard.clear(),
                    3 => frontier_lower_bound.reverse(),
                    _ => unreachable!(),
                }
                let mut changed_bytes = Vec::new();
                encode_block(&mut changed_bytes, &changed);
                assert_ne!(changed_bytes, encoded);
                assert_eq!(
                    decode_block(&mut Cursor::new(&changed_bytes)).unwrap(),
                    changed
                );
            }
            let mut cursor = Cursor::new(&encoded);
            cursor.take(4 + 1 + 8 + 8 + 1).unwrap();
            decode_instruction(&mut cursor).unwrap();
            cursor.take(8).unwrap();
            let cause_offset = encoded.len() - cursor.remaining();
            let mut corrupt = encoded.clone();
            corrupt[cause_offset] = 255;
            assert_eq!(
                decode_block(&mut Cursor::new(&corrupt)),
                Err(FixedViewCopyDecodeError::UnknownCrashCause(255))
            );
            assert!(decode_block(&mut Cursor::new(&encoded[..encoded.len() - 1])).is_err());
        }
    }

    #[test]
    fn borrowed_value_binding_retains_semantic_place_and_exact_storage() {
        use ::selected_instructions::{
            LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
        };
        use semantic_vocabulary::PlaceId;
        let mut original = successor();
        original
            .structural_bindings
            .push(SelectedStructuralBinding {
                source_indices: Vec::new(),
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(31).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(37).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: VirtualRegisterId(41),
                    source_byte_offset: 0,
                    destination: LocalStorageSlotId::StructuralBlockParameter {
                        block: original.source_target,
                        place: PlaceId::new(31).unwrap(),
                    },
                    byte_size: 16,
                    alignment: 8,
                },
            });
        let mut stored_bytes = Vec::new();
        encode_successor(&mut stored_bytes, &original);
        let mut unused = original.clone();
        unused.structural_bindings[0].transport = SelectedStructuralTransport::Unused;
        let mut unused_bytes = Vec::new();
        encode_successor(&mut unused_bytes, &unused);
        let transport_offset = stored_bytes
            .iter()
            .zip(&unused_bytes)
            .position(|(stored, unused)| stored != unused)
            .unwrap();
        assert_eq!(stored_bytes[transport_offset], 2);
        assert_eq!(unused_bytes[transport_offset], 0);
        stored_bytes[transport_offset] = 1;
        assert_eq!(
            decode_successor(&mut Cursor::new(&stored_bytes)),
            Err(FixedViewCopyDecodeError::UnknownValueTransport(1))
        );
        for transport in [
            original.structural_bindings[0].transport,
            SelectedStructuralTransport::Unused,
        ] {
            original.structural_bindings[0].transport = transport;
            let mut bytes = Vec::new();
            encode_successor(&mut bytes, &original);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_successor(&mut cursor).unwrap(), original);
            assert_eq!(cursor.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode_successor(&mut Cursor::new(&bytes[..length])).is_err());
            }
        }
    }

    #[test]
    fn successor_rejects_unknown_transport_and_incomplete_register_pair() {
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &successor());
        encoded[46] = 2;
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::UnknownValueTransport(2))
        );
        encoded[46] = 1;
        for length in 47..55 {
            assert_eq!(
                decode_successor(&mut Cursor::new(&encoded[..length])),
                Err(FixedViewCopyDecodeError::Truncated)
            );
        }
    }

    #[test]
    fn successor_role_round_trips_and_unknown_roles_reject() {
        let mut original = successor();
        original.role = SelectedSuccessorRole::EdgeTransferContinuation;
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        assert_eq!(encoded[0], 1);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)).unwrap(),
            original
        );
        original.role = SelectedSuccessorRole::CaseDispatchContinuation;
        encoded.clear();
        encode_successor(&mut encoded, &original);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)).unwrap(),
            original
        );
        encoded[0] = 4;
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::UnknownSuccessorRole(4))
        );
    }

    #[test]
    fn operation_successor_codec_keeps_absent_edges_and_rejects_forged_lineage() {
        let operation = semantic_vocabulary::OperationId::new(71).unwrap();
        let mut original = successor();
        original.role = SelectedSuccessorRole::OperationContinuation { operation };
        original.psi_edge = None;
        original.bindings.clear();
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)).unwrap(),
            original
        );
        encoded[9..17].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::InvalidSuccessorLineage)
        );
        original.role = SelectedSuccessorRole::Semantic;
        encoded.clear();
        encode_successor(&mut encoded, &original);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::InvalidSuccessorLineage)
        );
    }

    #[test]
    fn implementation_block_origin_round_trips_without_a_fabricated_source_block() {
        let mut block =
            function_with_operand(register_model::RegisterOperandAccess::Use).blocks[0].clone();
        block.origin = SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(71).unwrap(),
            target: BlockId::new(91).unwrap(),
        };
        let mut encoded = Vec::new();
        encode_block(&mut encoded, &block);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_block(&mut cursor).unwrap(), block);
        assert_eq!(cursor.remaining(), 0);
        block.origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(91).unwrap(),
            case_ordinal: 2,
        };
        encoded.clear();
        encode_block(&mut encoded, &block);
        assert_eq!(decode_block(&mut Cursor::new(&encoded)).unwrap(), block);
        block.origin = SelectedBlockOrigin::Operation {
            source: BlockId::new(91).unwrap(),
            operation: semantic_vocabulary::OperationId::new(71).unwrap(),
        };
        encoded.clear();
        encode_block(&mut encoded, &block);
        assert_eq!(decode_block(&mut Cursor::new(&encoded)).unwrap(), block);
        encoded[4] = 4;
        assert_eq!(
            decode_block(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::UnknownBlockOrigin(4))
        );
    }

    #[test]
    fn case_payload_codec_retains_every_semantic_and_transport_field() {
        use ::selected_instructions::{
            LocalStorageSlotId, SelectedCaseDispatchSource, SelectedCasePayloadBinding,
            SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
        };
        use semantic_vocabulary::{OperationId, PlaceId, StructuralCaseId, StructuralFieldId};
        use terminal_psi::TerminalAffineCleanupAction;
        let mut original = successor();
        original.structural_case = Some(SelectedStructuralCaseEdge {
            source: SelectedCaseDispatchSource::Local {
                slot: LocalStorageSlotId::Structural {
                    operation: OperationId::new(7).unwrap(),
                    place: PlaceId::new(11).unwrap(),
                },
            },
            case: StructuralCaseId::new(2).unwrap(),
            case_tag: 1,
            payloads: vec![SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: StructuralFieldId::new(3).unwrap(),
                    field_byte_offset: 4,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(9).unwrap(),
                        scalar_type: ScalarType::Boolean,
                        definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                            block: original.source_target,
                            position: 1,
                        },
                    },
                },
                transport: SelectedCasePayloadTransport::Unused,
            }],
            cleanup_actions: vec![
                TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(11).unwrap()),
                TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place: PlaceId::new(12).unwrap(),
                        path: vec![terminal_psi::StructuralPathSegment::Field(
                            terminal_psi::TerminalIdentityIndex(1),
                        )],
                        structural_type: semantic_vocabulary::StructuralTypeId::new(4).unwrap(),
                    },
                ),
            ],
        });
        let mut encodings = Vec::new();
        for transport in [
            SelectedCasePayloadTransport::Unused,
            SelectedCasePayloadTransport::Unmaterialized {
                parameter: VirtualRegisterId(29),
            },
            SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(17),
                parameter: VirtualRegisterId(29),
            },
        ] {
            original.structural_case.as_mut().unwrap().payloads[0].transport = transport;
            let mut encoded = Vec::new();
            encode_successor(&mut encoded, &original);
            assert_eq!(
                decode_successor(&mut Cursor::new(&encoded)).unwrap(),
                original
            );
            for end in 0..encoded.len() {
                assert!(decode_successor(&mut Cursor::new(&encoded[..end])).is_err());
            }
            encodings.push(encoded);
        }
        assert_ne!(encodings[0], encodings[1]);
        assert_ne!(encodings[1], encodings[2]);
        for mutation in 0..7 {
            let mut changed = original.clone();
            let case = changed.structural_case.as_mut().unwrap();
            match mutation {
                0 => case.case_tag = 0,
                1 => case.payloads[0].semantic.field_byte_offset += 4,
                2 => case.payloads[0].semantic.parameter.value = ValueId::new(10).unwrap(),
                3 => case.cleanup_actions.retain(|action| {
                    !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_))
                }),
                6 => case.cleanup_actions.retain(|action| {
                    !matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
                }),
                4 => {
                    case.source = SelectedCaseDispatchSource::Local {
                        slot: LocalStorageSlotId::Boundary {
                            operation: OperationId::new(7).unwrap(),
                        },
                    }
                }
                _ => {
                    case.payloads[0].semantic.parameter.definition_site =
                        optimization_unit::ValueDefinitionSite::FunctionParameter(1)
                }
            }
            let mut encoded = Vec::new();
            encode_successor(&mut encoded, &changed);
            assert_ne!(encoded, encodings[2], "mutation {mutation}");
            assert_eq!(
                decode_successor(&mut Cursor::new(&encoded)).unwrap(),
                changed
            );
        }
    }

    #[test]
    fn whole_value_transport_round_trips_exact_storage_and_rejects_invalid_extents() {
        use ::selected_instructions::{
            LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
        };
        let mut original = successor();
        original
            .structural_bindings
            .push(SelectedStructuralBinding {
                source_indices: Vec::new(),
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: semantic_vocabulary::PlaceId::new(12).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                        access: terminal_psi::StructuralAccess::Owned,
                        path: Vec::new(),
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: VirtualRegisterId(30),
                    source_byte_offset: 16,
                    destination: LocalStorageSlotId::StructuralBlockParameter {
                        block: original.source_target,
                        place: semantic_vocabulary::PlaceId::new(12).unwrap(),
                    },
                    byte_size: 12,
                    alignment: 4,
                },
            });
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
        for (byte_size, alignment, source_offset) in [
            (0, 4, 16),
            (12, 0, 16),
            (12, 3, 16),
            (12, 8, 16),
            (12, 4, 1),
            (12, 4, u32::MAX - 3),
        ] {
            let mut forged = original.clone();
            let SelectedStructuralTransport::WholeValue {
                byte_size: bytes,
                alignment: align,
                source_byte_offset,
                ..
            } = &mut forged.structural_bindings[0].transport
            else {
                unreachable!()
            };
            *bytes = byte_size;
            *align = alignment;
            *source_byte_offset = source_offset;
            let mut encoded = Vec::new();
            encode_successor(&mut encoded, &forged);
            assert_eq!(
                decode_successor(&mut Cursor::new(&encoded)),
                Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
            );
        }
        let mut forged = original.clone();
        let SelectedStructuralTransport::WholeValue { destination, .. } =
            &mut forged.structural_bindings[0].transport
        else {
            unreachable!()
        };
        *destination = LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(12).unwrap(),
        };
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &forged);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
        );
    }

    #[test]
    fn address_transport_round_trips_both_bases_and_rejects_forged_carriers() {
        use ::selected_instructions::{
            LocalStorageSlotId, SelectedAddressBase, SelectedStructuralBinding,
            SelectedStructuralTransport,
        };
        let join = semantic_vocabulary::PlaceId::new(12).unwrap();
        for base in [
            SelectedAddressBase::Register(VirtualRegisterId(30)),
            SelectedAddressBase::Local(LocalStorageSlotId::Structural {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            }),
        ] {
            let mut original = successor();
            original
                .structural_bindings
                .push(SelectedStructuralBinding {
                    source_indices: Vec::new(),
                    semantic: abstract_operations::AbstractStructuralBinding {
                        parameter: join,
                        argument: terminal_psi::StructuralArgument {
                            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                            path: vec![terminal_psi::StructuralPathSegment::Field(
                                terminal_psi::TerminalIdentityIndex(2),
                            )],
                        },
                    },
                    transport: SelectedStructuralTransport::Address {
                        base,
                        byte_offset: 8,
                        byte_count: 8,
                        destination: LocalStorageSlotId::StructuralBlockParameter {
                            block: original.source_target,
                            place: join,
                        },
                    },
                });
            let mut encoded = Vec::new();
            encode_successor(&mut encoded, &original);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_successor(&mut cursor).unwrap(), original);
            assert_eq!(cursor.remaining(), 0);
            for mutation in 0..2 {
                let mut forged = original.clone();
                let SelectedStructuralTransport::Address {
                    byte_count,
                    destination,
                    ..
                } = &mut forged.structural_bindings[0].transport
                else {
                    unreachable!()
                };
                if mutation == 0 {
                    *byte_count = 0;
                } else {
                    *destination = LocalStorageSlotId::Structural {
                        operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                        place: join,
                    };
                }
                let mut encoded = Vec::new();
                encode_successor(&mut encoded, &forged);
                assert_eq!(
                    decode_successor(&mut Cursor::new(&encoded)),
                    Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
                );
            }
        }
    }

    #[test]
    fn successor_case_round_trip_retains_ordered_cleanup_actions() {
        use ::selected_instructions::{SelectedCaseDispatchSource, SelectedStructuralCaseEdge};
        use semantic_vocabulary::{
            ClaimId, MachineId, ObligationId, PlaceId, StructuralCaseId, StructuralTypeId,
        };
        use terminal_psi::{
            NominalAffineCleanup, StructuralAffineDiscard, StructuralPathSegment,
            TerminalAffineCleanupAction,
        };

        let mut original = successor();
        original.structural_case = Some(SelectedStructuralCaseEdge {
            source: SelectedCaseDispatchSource::Borrowed {
                place: PlaceId::new(10).unwrap(),
                pointer: VirtualRegisterId(11),
                byte_size: 16,
            },
            case: StructuralCaseId::new(12).unwrap(),
            case_tag: 3,
            payloads: Vec::new(),
            cleanup_actions: vec![
                TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(13).unwrap()),
                TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                    retains_descendants: false,
                    place: PlaceId::new(14).unwrap(),
                    path: vec![
                        StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(3)),
                        StructuralPathSegment::FixedIndex(1),
                    ],
                    structural_type: StructuralTypeId::new(15).unwrap(),
                    cleanup_machine: MachineId::new(16).unwrap(),
                    consumed_claims: vec![ClaimId::new(17).unwrap()],
                    cleanup_receiver: Some(PlaceId::new(18).unwrap()),
                    requirement_obligations: vec![ObligationId::new(19).unwrap()],
                }),
                TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                    place: PlaceId::new(20).unwrap(),
                    structural_type: StructuralTypeId::new(21).unwrap(),
                    path: vec![
                        StructuralPathSegment::Case(StructuralCaseId::new(22).unwrap()),
                        StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(4)),
                        StructuralPathSegment::Referent,
                        StructuralPathSegment::FixedIndex(1),
                        StructuralPathSegment::FixedRange { start: 2, end: 4 },
                        StructuralPathSegment::RuntimeIndex {
                            index: ValueId::new(23).unwrap(),
                            obligation: ObligationId::new(24).unwrap(),
                        },
                    ],
                }),
            ],
        });
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
    }
}
