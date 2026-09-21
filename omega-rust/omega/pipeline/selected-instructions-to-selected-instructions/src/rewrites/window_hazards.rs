//! Hazard audit shared by the in-block relocation rewrites: which
//! instructions may trade order at all, which pairs are coupled by a
//! register or condition-state hazard, and whether a boundary settlement
//! sits inside a window. Each rewrite locates its own window; the audit of
//! that window is one owner.
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};

use crate::rewrites::block_edges::{
    RelocationCrossing, edge_accounted, plain_edge, transport_conflict,
};

/// Register locations an operand reads or writes. `UseDef` participates in
/// both directions, matching the access marks the constraint rows publish.
fn reads(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

fn writes(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

pub(super) fn register_reads(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| reads(operand.access))
        .map(|operand| operand.virtual_register)
}

pub(super) fn register_writes(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| writes(operand.access))
        .map(|operand| operand.virtual_register)
}

/// Whether `writer`'s writes meet `reader`'s reads on a register or a
/// condition-state unit. Implicit uses read units; implicit definitions and
/// clobbers write them, so flag publishers and flag consumers couple the
/// same way explicit operands do.
fn writes_meet_reads(writer: &SelectedInstruction, reader: &SelectedInstruction) -> bool {
    register_writes(writer)
        .any(|register| register_reads(reader).any(|candidate| candidate == register))
        || writer
            .implicit_defs
            .iter()
            .chain(writer.clobbers.iter())
            .any(|unit| reader.implicit_uses.contains(unit))
}

/// Whether two instructions can trade order. The three hazards cover every
/// pair a move passes: `earlier` defining a location `later` reads (RAW)
/// would starve the consumer or hand it a different value, `earlier`
/// reading a location `later` writes (WAR) would hand it the new value, and
/// a shared written location (WAW) would change which definition later
/// positions observe. Units and registers participate identically.
pub(super) fn coupled(earlier: &SelectedInstruction, later: &SelectedInstruction) -> bool {
    writes_meet_reads(earlier, later)
        || writes_meet_reads(later, earlier)
        || register_writes(earlier)
            .any(|register| register_writes(later).any(|candidate| candidate == register))
        || earlier
            .implicit_defs
            .iter()
            .chain(earlier.clobbers.iter())
            .any(|unit| later.implicit_defs.contains(unit) || later.clobbers.contains(unit))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can observe or expose reachable state regardless of roster rows, and a
/// terminator kind never belongs in a block body. Same boundary the
/// memory-motion rules enforce.
pub(super) fn is_barrier(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    matches!(
        instruction.kind,
        CallUnit { .. }
            | CallScalar { .. }
            | CallAggregate { .. }
            | HostedReadByte { .. }
            | HostedWriteByteI32 { .. }
            | SaveFloatingControl { .. }
            | RestoreFloatingControl { .. }
            | HostedExitProcessI32
            | ReturnScalar
            | ReturnAggregate { .. }
            | ReturnUnit
            | Jump
            | ConditionalBranchNonZero
            | ConditionalBranchU64LessThan
            | ConditionalBranchI64LessThan
    )
}

/// An instruction without a roster row must be unable to reach any semantic
/// or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access whose reach the relocation
/// cannot prove.
fn unaccounted_kind(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        Store64 {
            slot: FrameStorageSlotId::Local(
                LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
            ),
            ..
        } | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Load64 { .. }
            | Load8Indexed
            | LoadPacked { .. }
            | FrameAddress { .. }
            | AddressOffset { .. }
            | ByteViewAddress
            | CopyI64
            | MaterializeI64 { .. }
            | CompareI64
            | CompareI64Zero
            | CompareI64Immediate { .. }
            | ExactAddI64 { .. }
            | ExactSubtractI64 { .. }
            | ExactMultiplyI64 { .. }
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | SaturatingAdd { .. }
            | WrappingAddI64
            | WrappingSubtractI64
            | WrappingMultiplyI64
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
            | ExactDivideU64 { .. }
            | ExactRemainderU64 { .. }
            | WrappingRemainderI64 { .. }
            | WrappingDivideI64 { .. }
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
            | BitwiseAndI64
            | BitwiseOrI64
            | BitwiseNotI64
            | BitwiseXorI64
            | ZeroExtendU8
            | ZeroExtendU16
            | ZeroExtendU32
            | SignExtendI8
            | SignExtendI16
            | SignExtendI32
            | Float32ToBits
            | Float64ToBits
            | BitsToFloat32
            | BitsToFloat64
            | MaterializeBooleanEqual
            | MaterializeBooleanU64LessThan
            | MaterializeBooleanI64LessThan
            | MaterializeBooleanU64LessOrEqual
            | MaterializeBooleanI64LessOrEqual
    )
}

/// Whether an instruction's effect is pure register and condition-state
/// work that adds no observable execution on arrivals it never ran on.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: sinking it would add the
/// access — and any fault or slot write it carried — to every traversal
/// entering the relocation's landing through the other inflows. The same
/// holds for kinds whose target encoding may architecturally fault: their
/// proof obligations establish definedness for the source operation, but
/// this audit runs at the selected level where the encoded trap behavior
/// is the honest bound — an execution that could fault must still run
/// only on the paths that ran it before.
pub(super) fn speculatable(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        CopyBytes
            | LoadPacked { .. }
            | StorePacked { .. }
            | Store { .. }
            | Load8Indexed
            | Load64 { .. }
            | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Store64 { .. }
            | ExactDivideU64 { .. }
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
    )
}

/// Whether the roster accounts for the instruction's memory reach. Rows
/// name the instruction by identity, so the relocation retains them
/// unchanged.
pub(super) fn has_memory_rows(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
) -> bool {
    function
        .memory_accesses
        .iter()
        .any(|access| access.instruction == instruction)
}

/// A call contract row makes the instruction an effect barrier even when
/// its kind survived the kind check.
pub(super) fn has_call_contract(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
) -> bool {
    function
        .calls
        .iter()
        .any(|call| call.instruction == instruction)
}

/// A boundary settlement at `position` sits before that body ordinal: any
/// position after the window's first index through its last observes a
/// different executed prefix once the member lands on the other side of
/// the crossed run, while positions at or outside the window's span see
/// the same executed set on either order.
pub(super) fn interior_settlement(
    function: &SelectedFunction,
    block: SelectedBlockId,
    window: std::ops::RangeInclusive<usize>,
) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && window.contains(&(settlement.instruction_index as usize))
    })
}

/// Whether one instruction may trade order with a crossed instruction at
/// all: not a barrier kind, not named by the call roster, and either
/// roster-accounted or unable to reach storage the roster covers.
/// `None` when the instruction can never trade order; otherwise whether the
/// roster accounts for its memory reach.
pub(super) fn schedulable(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
) -> Option<bool> {
    if is_barrier(instruction) || has_call_contract(function, instruction.id) {
        return None;
    }
    let accounted = has_memory_rows(function, instruction.id);
    if !accounted && unaccounted_kind(instruction) {
        return None;
    }
    Some(accounted)
}

/// One instruction's hazard-audit surface: the operand, implicit-use,
/// implicit-definition, and clobber rows `coupled` walks.
pub(super) fn surface(instruction: &SelectedInstruction) -> usize {
    instruction.operands.len()
        + instruction.implicit_uses.len()
        + instruction.implicit_defs.len()
        + instruction.clobbers.len()
}

/// Why the run-level relocation audit refused a window — one kind per
/// crossed contract so a caller can keep reporting its own typed errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunRelocationRejection {
    /// No acyclic path joins the run block to the destination block, or
    /// the run is empty.
    UnreachableDestination,
    /// A member or a crossed position can never trade order: a barrier
    /// kind, a call-roster entry, or an unaccounted memory reach.
    Unschedulable,
    /// A member shares a register or condition-state unit with a crossed
    /// position or with a crossed edge's terminator instruction.
    Coupled,
    /// Both sides of a crossing carry memory-roster rows, so their order
    /// is observable through the roster.
    MemoryOrdering,
    /// A member interferes with a crossed edge's register transports.
    TransportConflict,
    /// A crossed edge carries boundary effects — case dispatch,
    /// continuation, structural transfer, or per-edge fuel — the move
    /// cannot preserve.
    NonPlainEdge,
    /// A boundary settlement sits inside the moved span or a crossed
    /// block, so the executed prefix it observes changes.
    Settlement,
}

/// Applies the whole window audit to a member run in one pass — the check
/// every per-shape relocation admission spells out itself.
/// `crossed_window` in `block_edges` derives the positions and edges every
/// acyclic path between the run and the destination crosses; this audit
/// proves the window independent once: every member schedulable, no member
/// coupled with any crossed position or crossed edge's terminator
/// instruction, roster-accounted members crossing only unaccounted
/// positions, every crossed edge plain and free of transport conflicts,
/// and no boundary settlement inside the moved span or a crossed block.
/// Hazards between the run's own members are not re-checked: the members
/// keep their relative order.
pub(super) fn admit_run_relocation(
    function: &SelectedFunction,
    members: &[&SelectedInstruction],
    crossing: &RelocationCrossing<'_>,
) -> Result<(), RunRelocationRejection> {
    if members.is_empty() || !crossing.reachable {
        return Err(RunRelocationRejection::UnreachableDestination);
    }
    let mut any_member_accounted = false;
    for member in members {
        match schedulable(function, member) {
            None => return Err(RunRelocationRejection::Unschedulable),
            Some(accounted) => any_member_accounted |= accounted,
        }
    }
    for (block_index, positions) in &crossing.positions {
        let block = &function.blocks[*block_index];
        for position in positions {
            let crossed = &block.instructions[*position];
            let Some(crossed_accounted) = schedulable(function, crossed) else {
                return Err(RunRelocationRejection::Unschedulable);
            };
            if any_member_accounted && crossed_accounted {
                return Err(RunRelocationRejection::MemoryOrdering);
            }
            for member in members {
                if coupled(member, crossed) {
                    return Err(RunRelocationRejection::Coupled);
                }
            }
        }
    }
    for edge in &crossing.edges {
        if !plain_edge(edge.successor) {
            return Err(RunRelocationRejection::NonPlainEdge);
        }
        if has_call_contract(function, edge.instruction.id) {
            return Err(RunRelocationRejection::Unschedulable);
        }
        if any_member_accounted && edge_accounted(function, edge.instruction, edge.successor) {
            return Err(RunRelocationRejection::MemoryOrdering);
        }
        for member in members {
            if coupled(member, edge.instruction) {
                return Err(RunRelocationRejection::Coupled);
            }
            if transport_conflict(member, edge.successor) {
                return Err(RunRelocationRejection::TransportConflict);
            }
        }
    }
    let run_block = function.blocks[crossing.run_block].id;
    let destination_block = function.blocks[crossing.destination_block].id;
    for settlement in &function.boundary_settlements {
        let index = settlement.instruction_index as usize;
        let refused = if settlement.block == run_block && run_block == destination_block {
            // In-block: the window's span runs from just after the earlier
            // of the run's start and the landing index through the later of
            // the run's end and the landing index. The earlier endpoint
            // itself observes an unchanged prefix — a settlement at it sees
            // only positions before the window, identical on either order.
            index > crossing.run_start.min(crossing.landing_index)
                && index <= crossing.run_end.max(crossing.landing_index)
        } else if settlement.block == run_block {
            // Cross-block: the run vacates from `run_start` on, so any
            // settlement positioned after it observes a changed prefix.
            index > crossing.run_start
        } else if settlement.block == destination_block {
            // Positions past the landing index observe the run inside the
            // prefix.
            index > crossing.landing_index
        } else {
            // An intermediate block's whole body is crossed — any
            // settlement in it observes a changed executed prefix. The
            // block key decides rather than its position list: an
            // empty-bodied intermediate records no ordinals, yet a
            // settlement at index 0 still trades which side of that
            // point the member executes on.
            crossing
                .positions
                .iter()
                .any(|(block_index, _)| function.blocks[*block_index].id == settlement.block)
        };
        if refused {
            return Err(RunRelocationRejection::Settlement);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    };
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
        SelectedBoundarySettlementPayload, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
        SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
        SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
        VirtualRegisterId,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, EdgeId, MachineId, OperationId, PlaceId, ValueId,
    };

    use super::{RunRelocationRejection, admit_run_relocation};
    use crate::rewrites::block_edges::{CrossingDirection, crossed_window};

    const BLOCK_A: SelectedBlockId = SelectedBlockId(0);
    const BLOCK_B: SelectedBlockId = SelectedBlockId(1);
    const R_MEMBER: VirtualRegisterId = VirtualRegisterId(10);
    const R_OTHER: VirtualRegisterId = VirtualRegisterId(11);

    fn instruction(
        id: u64,
        access: RegisterOperandAccess,
        register: VirtualRegisterId,
    ) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(id.try_into().unwrap()),
            kind: SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 0,
            },
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register: register,
                access,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: Default::default(),
        }
    }

    fn successor(block: SelectedBlockId, edge: u64) -> selected_instructions::SelectedSuccessor {
        selected_instructions::SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(1).unwrap(),
            bindings: Vec::new(),
            fuel: Vec::new(),
        }
    }

    /// `A = [1(member);2(member);3(tail)] -> jump -> B = [4;5] -> return`.
    /// The run is A's first two members landing at B index 1.
    fn function(
        member_access: RegisterOperandAccess,
        tail_access: RegisterOperandAccess,
    ) -> SelectedFunction {
        let jump_instruction = instruction(60, RegisterOperandAccess::Use, R_OTHER);
        let return_instruction = instruction(61, RegisterOperandAccess::Use, R_OTHER);
        SelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: BLOCK_A,
            virtual_registers: Vec::new(),
            blocks: vec![
                SelectedBlock {
                    id: BLOCK_A,
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        instruction(1, member_access, R_MEMBER),
                        instruction(2, member_access, R_MEMBER),
                        instruction(3, tail_access, R_OTHER),
                    ],
                    terminator: SelectedTerminator::Jump {
                        instruction: jump_instruction,
                        successor: successor(BLOCK_B, 10),
                    },
                },
                SelectedBlock {
                    id: BLOCK_B,
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: vec![
                        instruction(4, RegisterOperandAccess::Use, R_OTHER),
                        instruction(5, RegisterOperandAccess::Use, R_OTHER),
                    ],
                    terminator: SelectedTerminator::Return {
                        instruction: return_instruction,
                        psi_return_edge: EdgeId::new(12).unwrap(),
                    },
                },
            ],
        }
    }

    fn admit(function: &SelectedFunction) -> Result<(), RunRelocationRejection> {
        let crossing =
            crossed_window(function, 0, 0, 1, 1, 1, CrossingDirection::Forward, 64).unwrap();
        let members: Vec<&SelectedInstruction> =
            function.blocks[0].instructions[0..=1].iter().collect();
        admit_run_relocation(function, &members, &crossing)
    }

    #[test]
    fn clean_run_crossing_admits() {
        let function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        assert_eq!(admit(&function), Ok(()));
    }

    #[test]
    fn member_writing_what_a_crossed_position_reads_is_coupled() {
        // Member defines R_MEMBER; the tail instruction in the run block
        // reads it — moving the run past it hands the reader a different
        // value.
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Use);
        function.blocks[0].instructions[2].operands[0].virtual_register = R_MEMBER;
        assert_eq!(admit(&function), Err(RunRelocationRejection::Coupled));
    }

    #[test]
    fn member_reading_what_a_crossed_position_writes_is_coupled() {
        let mut function = function(RegisterOperandAccess::Use, RegisterOperandAccess::Def);
        function.blocks[0].instructions[0].operands[0].virtual_register = R_OTHER;
        assert_eq!(admit(&function), Err(RunRelocationRejection::Coupled));
    }

    #[test]
    fn a_barrier_member_is_unschedulable() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::ReturnUnit;
        assert_eq!(admit(&function), Err(RunRelocationRejection::Unschedulable));
    }

    #[test]
    fn a_barrier_inside_the_window_is_unschedulable() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::ReturnUnit;
        assert_eq!(admit(&function), Err(RunRelocationRejection::Unschedulable));
    }

    #[test]
    fn roster_carrying_sides_do_not_cross() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        for instruction in [1u64, 3] {
            function.memory_accesses.push(SelectedMemoryAccess {
                instruction: SelectedInstructionId(instruction.try_into().unwrap()),
                origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(31).unwrap()),
                place: PlaceId::new(3).unwrap(),
                byte_offset: 0,
                byte_count: 8,
                role: SelectedMemoryAccessRole::WritePlace,
            });
        }
        assert_eq!(
            admit(&function),
            Err(RunRelocationRejection::MemoryOrdering)
        );
    }

    #[test]
    fn a_crossed_edge_transport_conflict_refuses() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            panic!()
        };
        successor.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(20).unwrap(),
                argument: ValueId::new(21).unwrap(),
                scalar_type: semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: R_MEMBER,
                parameter: R_OTHER,
            },
        });
        assert_eq!(
            admit(&function),
            Err(RunRelocationRejection::TransportConflict)
        );
    }

    #[test]
    fn a_non_plain_crossed_edge_refuses() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            panic!()
        };
        successor.role = SelectedSuccessorRole::CaseDispatchContinuation;
        assert_eq!(admit(&function), Err(RunRelocationRejection::NonPlainEdge));
    }

    #[test]
    fn a_settlement_past_the_vacated_run_refuses() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: BLOCK_A,
                instruction_index: 2,
                settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    operation: OperationId::new(30).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(9).unwrap(),
                },
            });
        assert_eq!(admit(&function), Err(RunRelocationRejection::Settlement));
    }

    #[test]
    fn a_settlement_at_the_run_start_is_retained() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: BLOCK_A,
                instruction_index: 0,
                settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    operation: OperationId::new(30).unwrap(),
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    source: ValueId::new(9).unwrap(),
                },
            });
        assert_eq!(admit(&function), Ok(()));
    }

    #[test]
    fn an_unreachable_destination_refuses() {
        let mut function = function(RegisterOperandAccess::Def, RegisterOperandAccess::Def);
        function.blocks[1].id = SelectedBlockId(9); // sever the edge target
        let crossing =
            crossed_window(&function, 0, 0, 1, 1, 1, CrossingDirection::Forward, 64).unwrap();
        let members: Vec<&SelectedInstruction> =
            function.blocks[0].instructions[0..=1].iter().collect();
        assert_eq!(
            admit_run_relocation(&function, &members, &crossing),
            Err(RunRelocationRejection::UnreachableDestination)
        );
    }
}
