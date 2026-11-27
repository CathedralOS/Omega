/*
  Intra-block promotion of memory-based `Packed*` selected instructions to
  128-bit `VECTOR128` register operations (`Packed*Reg`, `PackedLoad*`,
  `PackedStore*`, `PackedMaterializeZero`, `PackedFromGpr64*`), paired with
  single-block local-slot scalar replacement, external 16-byte store
  coalescing, dead local-store elimination, and pure `CopyI64`/DCE cleanup.

  All transformations are intra-block and strictly preserve non-empty
  `provenance.work`, `provenance.obligations`, and `provenance.edges`.
*/

use crate::instruction_selection::SelectedInstructionError;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedCaseDispatchSource,
    SelectedCasePayloadTransport, SelectedConstraintKeys, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedMemoryAccessRole, SelectedOperand, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin, terminator_instruction, terminator_instruction_mut,
    terminator_successors, terminator_successors_mut,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use std::collections::{HashMap, HashSet};

const MAX_TRACKED_SLOT_BYTES: usize = 512;
const MAX_EXTERNAL_COALESCE_WINDOW: usize = 24;

/// The scalar type of every `VECTOR128` register this pass creates. A promoted
/// register holds 16 raw bytes of lanes, so it carries an unsigned 128-bit type:
/// Stage 07 reads the width from this type when it spills the register, and a
/// 128-bit width selects a 16-byte packed store and reload. An 8-byte type here
/// would let a spill keep only the low half of the vector.
fn packed_vector_scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 128).expect("u128 integer type"))
}

pub(crate) fn promote_and_simplify(
    functions: &mut [SelectedFunction],
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let keys = environment.selected_keys();
    let Some(packed_keys) = PackedRegKeys::from_keys(&keys, environment) else {
        return Ok(());
    };
    let copy_row = constraint_row(environment, keys.copy_i64)?;
    if copy_row.operands.len() != 2 {
        return Err(SelectedInstructionError::MissingConstraint(keys.copy_i64));
    }
    let vec128_class = constraint_row(environment, packed_keys.add_reg)?.operands[2].class;
    let gpr64_class = copy_row.operands[0].class;

    for function in functions.iter_mut() {
        promote_function(
            function,
            environment,
            &keys,
            &packed_keys,
            vec128_class,
            gpr64_class,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PackedRegKeys {
    load_128: RegisterConstraintKey,
    store_128: RegisterConstraintKey,
    load_64: RegisterConstraintKey,
    store_64: RegisterConstraintKey,
    load_32: RegisterConstraintKey,
    store_32: RegisterConstraintKey,
    materialize_zero: RegisterConstraintKey,
    from_gpr64_zero_extend: RegisterConstraintKey,
    from_gpr64_pair: RegisterConstraintKey,
    add_reg: RegisterConstraintKey,
    sub_reg: RegisterConstraintKey,
    shift_right_reg: RegisterConstraintKey,
    shuffle_reg: RegisterConstraintKey,
    widen_u8x8_reg: RegisterConstraintKey,
    narrow_u8x8_reg: RegisterConstraintKey,
    add_u8x16_reg: RegisterConstraintKey,
    min_i16x8_reg: RegisterConstraintKey,
    cmpgt_i16x8_reg: RegisterConstraintKey,
    bit_and_i16x8_reg: RegisterConstraintKey,
    bit_andnot_i16x8_reg: RegisterConstraintKey,
    abs_i16x8_reg: RegisterConstraintKey,
}

impl PackedRegKeys {
    fn from_keys(
        keys: &SelectedConstraintKeys,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Option<Self> {
        let out = Self {
            load_128: keys.packed_load_128?,
            store_128: keys.packed_store_128?,
            load_64: keys.packed_load_64_zero_extend?,
            store_64: keys.packed_store_64?,
            load_32: keys.packed_load_32_zero_extend?,
            store_32: keys.packed_store_32?,
            materialize_zero: keys.packed_materialize_zero?,
            from_gpr64_zero_extend: keys.packed_from_gpr64_zero_extend?,
            from_gpr64_pair: keys.packed_from_gpr64_pair?,
            add_reg: keys.packed_add_i16x8_reg?,
            sub_reg: keys.packed_sub_i16x8_reg?,
            shift_right_reg: keys.packed_shift_right_i16x8_reg?,
            shuffle_reg: keys.packed_shuffle_i16x8_reg?,
            widen_u8x8_reg: keys.packed_widen_u8x8_to_i16x8_reg?,
            narrow_u8x8_reg: keys.packed_narrow_i16x8_to_u8x8_saturating_reg?,
            add_u8x16_reg: keys.packed_add_u8x16_reg?,
            min_i16x8_reg: keys.packed_min_i16x8_reg?,
            cmpgt_i16x8_reg: keys.packed_cmpgt_i16x8_reg?,
            bit_and_i16x8_reg: keys.packed_bit_and_i16x8_reg?,
            bit_andnot_i16x8_reg: keys.packed_bit_andnot_i16x8_reg?,
            abs_i16x8_reg: keys.packed_abs_i16x8_reg?,
        };
        for key in [
            out.load_128,
            out.store_128,
            out.load_64,
            out.store_64,
            out.load_32,
            out.store_32,
            out.materialize_zero,
            out.from_gpr64_zero_extend,
            out.from_gpr64_pair,
            out.add_reg,
            out.sub_reg,
            out.shift_right_reg,
            out.shuffle_reg,
            out.widen_u8x8_reg,
            out.narrow_u8x8_reg,
            out.add_u8x16_reg,
            out.min_i16x8_reg,
            out.cmpgt_i16x8_reg,
            out.bit_and_i16x8_reg,
            out.bit_andnot_i16x8_reg,
            out.abs_i16x8_reg,
        ] {
            environment.constraint(key)?;
        }
        Some(out)
    }
}

fn constraint_row(
    environment: &ValidatedTargetRegisterEnvironment,
    key: RegisterConstraintKey,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    environment
        .constraint(key)
        .ok_or(SelectedInstructionError::MissingConstraint(key))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PtrOrigin {
    Local {
        slot: LocalStorageSlotId,
        offset: u32,
    },
    External {
        base: VirtualRegisterId,
        offset: u32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymByte {
    Unknown,
    Const(u8),
    VecLaneByte {
        vreg: VirtualRegisterId,
        byte_index: u8,
    },
    GprByte {
        vreg: VirtualRegisterId,
        byte_index: u8,
    },
}

#[derive(Clone, Copy)]
struct PendingVecWrite {
    offset: u32,
    len: u32,
    vreg: VirtualRegisterId,
}

#[derive(Clone, Copy)]
enum ExtStore8Source {
    Zero,
    VecLow8(VirtualRegisterId),
    VecHigh8(VirtualRegisterId),
}

#[derive(Clone, Copy)]
struct ExtStore8Info {
    base: VirtualRegisterId,
    offset: u32,
    source: ExtStore8Source,
}

#[derive(Clone, Copy)]
struct ExtLoad8Origin {
    base: VirtualRegisterId,
    offset: u32,
    inst_id: SelectedInstructionId,
}

#[derive(Clone, Copy)]
struct FoldedExtLoadGroup {
    lo_id: SelectedInstructionId,
    hi_id: Option<SelectedInstructionId>,
    replacement_id: SelectedInstructionId,
}

#[derive(Clone, Copy)]
struct PackedOpSpec {
    in_widths: [u32; 2],
    in_count: usize,
    out_bytes: u32,
}

fn packed_op_spec(kind: &SelectedInstructionKind) -> Option<PackedOpSpec> {
    match kind {
        SelectedInstructionKind::PackedAddI16x8
        | SelectedInstructionKind::PackedSubI16x8
        | SelectedInstructionKind::PackedShuffleI16x8 { .. }
        | SelectedInstructionKind::PackedAddU8x16
        | SelectedInstructionKind::PackedMinI16x8
        | SelectedInstructionKind::PackedCmpGtI16x8
        | SelectedInstructionKind::PackedBitAndI16x8
        | SelectedInstructionKind::PackedBitAndNotI16x8 => Some(PackedOpSpec {
            in_widths: [16, 16],
            in_count: 2,
            out_bytes: 16,
        }),
        SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { .. }
        | SelectedInstructionKind::PackedAbsI16x8 => Some(PackedOpSpec {
            in_widths: [16, 0],
            in_count: 1,
            out_bytes: 16,
        }),
        SelectedInstructionKind::PackedWidenU8x8ToI16x8
        | SelectedInstructionKind::PackedLoadI16x4ZeroExtend => Some(PackedOpSpec {
            in_widths: [8, 0],
            in_count: 1,
            out_bytes: 16,
        }),
        SelectedInstructionKind::PackedWidenU8x4ToI16x8 => Some(PackedOpSpec {
            in_widths: [4, 0],
            in_count: 1,
            out_bytes: 16,
        }),
        SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating => Some(PackedOpSpec {
            in_widths: [16, 0],
            in_count: 1,
            out_bytes: 8,
        }),
        SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating => Some(PackedOpSpec {
            in_widths: [16, 0],
            in_count: 1,
            out_bytes: 4,
        }),
        _ => None,
    }
}

fn promote_function(
    function: &mut SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
    keys: &SelectedConstraintKeys,
    packed_keys: &PackedRegKeys,
    vec128_class: RegisterClassId,
    gpr64_class: RegisterClassId,
) -> Result<(), SelectedInstructionError> {
    let slot_sizes: HashMap<LocalStorageSlotId, usize> = function
        .local_storage_slots
        .iter()
        .map(|slot| (slot.id, slot.byte_size as usize))
        .collect();
    if function.blocks.is_empty() {
        return Ok(());
    }

    let multi_def_vregs = collect_multi_def_vregs(function);
    let non_escaping_by_block = analyze_non_escaping_slots(function, &slot_sizes, &multi_def_vregs);

    let mut next_inst_id = function
        .blocks
        .iter()
        .flat_map(|b| {
            b.instructions
                .iter()
                .map(|i| i.id.0)
                .chain(std::iter::once(terminator_instruction(&b.terminator).id.0))
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut next_vreg_id = function
        .virtual_registers
        .iter()
        .map(|v| v.id.0)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    let mut mem_remap: HashMap<SelectedInstructionId, Vec<(bool, SelectedInstructionId)>> =
        HashMap::new();
    let mut mem_delete: HashSet<SelectedInstructionId> = HashSet::new();
    let mut folded_ext_load_groups: Vec<FoldedExtLoadGroup> = Vec::new();

    // Extract virtual registers mutably around block iteration.
    let mut new_vregs = Vec::new();
    let block_count = function.blocks.len();

    for block_idx in 0..block_count {
        let local_slots = non_escaping_by_block
            .get(&block_idx)
            .cloned()
            .unwrap_or_default();

        // Only run the full pass if this block has promotable packed ops, coalescible external
        // 8B stores, or tracked non-escaping local slots with packed ops elsewhere? Wait: even if
        // a function only has external zero stores (like array clears), running per block is fast!
        let mut ctx = BlockCtx {
            environment,
            keys,
            packed_keys,
            vec128_class,
            gpr64_class,
            local_slots: &local_slots,
            multi_def_vregs: &multi_def_vregs,
            next_inst_id: &mut next_inst_id,
            next_vreg_id: &mut next_vreg_id,
            added_vregs: &mut new_vregs,
            slot_bytes: local_slots
                .iter()
                .map(|&s| (s, vec![SymByte::Unknown; slot_sizes[&s]]))
                .collect(),
            pending_vec_writes: HashMap::new(),
            ptr_origin: HashMap::new(),
            gpr_const: HashMap::new(),
            gpr_bytes: HashMap::new(),
            vec_high8_zero: HashMap::new(),
            cached_zero_xmm: None,
            cached_const_xmm: HashMap::new(),
            cached_const_gpr: HashMap::new(),
            cached_local_addr: HashMap::new(),
            gpr_linear_offset: HashMap::new(),
            gpr_scaled: HashMap::new(),
            cached_scaled_mul: HashMap::new(),
            cached_indexed_base: HashMap::new(),
            ext_store8_info: HashMap::new(),
            ext_load8_origin: HashMap::new(),
            ext_load_pair_vec: HashMap::new(),
            ext_load_single_vec: HashMap::new(),
            folded_ext_load_groups: &mut folded_ext_load_groups,
            mem_remap: &mut mem_remap,
            mem_delete: &mut mem_delete,
        };

        let mut block = function.blocks[block_idx].clone();
        ctx.promote_block(&mut block)?;
        coalesce_external_stores(
            &mut block.instructions,
            &mut ctx.ext_store8_info,
            environment,
            packed_keys,
            vec128_class,
            &local_slots,
            &multi_def_vregs,
            ctx.next_inst_id,
            ctx.next_vreg_id,
            ctx.added_vregs,
            ctx.mem_remap,
            ctx.mem_delete,
        )?;
        function.blocks[block_idx] = block;
    }

    function.virtual_registers.extend(new_vregs);

    let folded_ext_load_ids: HashSet<SelectedInstructionId> = folded_ext_load_groups
        .iter()
        .flat_map(|g| std::iter::once(g.lo_id).chain(g.hi_id))
        .collect();

    // Fixpoint CopyI64 alias propagation and pure dead-code elimination per block.
    dce_and_copy_prop(
        function,
        &slot_sizes,
        &non_escaping_by_block,
        &multi_def_vregs,
        &folded_ext_load_ids,
    );

    // Map any deleted external Load64 halves onto their replacement PackedLoad* instruction.
    if !folded_ext_load_groups.is_empty() {
        let mut surviving_insts = HashSet::new();
        for block in &function.blocks {
            for inst in &block.instructions {
                surviving_insts.insert(inst.id);
            }
            surviving_insts.insert(terminator_instruction(&block.terminator).id);
        }
        let mut remapped_replacements: HashSet<SelectedInstructionId> = HashSet::new();
        for group in &folded_ext_load_groups {
            if !surviving_insts.contains(&group.replacement_id) {
                continue;
            }
            for old_id in [Some(group.lo_id), group.hi_id].into_iter().flatten() {
                if surviving_insts.contains(&old_id)
                    || mem_remap.contains_key(&old_id)
                    || mem_delete.contains(&old_id)
                {
                    continue;
                }
                if remapped_replacements.insert(group.replacement_id) {
                    mem_remap
                        .entry(old_id)
                        .or_default()
                        .push((true, group.replacement_id));
                } else {
                    mem_delete.insert(old_id);
                }
            }
        }
    }

    // Reconcile memory_accesses after rewrites/deletions.
    reconcile_memory_accesses(function, &mem_remap, &mem_delete);

    // Prune dead virtual registers (keeping all parameter registers) and unused local slots,
    // ensuring every VirtualRegisterOrigin instruction still exists before renumber_function.
    prune_dead_metadata(function);

    super::selection_inlining::renumber_function(function);
    Ok(())
}

fn collect_multi_def_vregs(function: &SelectedFunction) -> HashSet<VirtualRegisterId> {
    let mut def_counts: HashMap<VirtualRegisterId, usize> = HashMap::new();
    for reg in &function.virtual_registers {
        if matches!(
            reg.origin,
            VirtualRegisterOrigin::EntryParameter { .. }
                | VirtualRegisterOrigin::StructuralParameter { .. }
                | VirtualRegisterOrigin::BlockParameter { .. }
        ) {
            *def_counts.entry(reg.id).or_default() += 1;
        }
    }
    for inst in function.blocks.iter().flat_map(block_instructions) {
        for op in &inst.operands {
            if matches!(
                op.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            ) {
                *def_counts.entry(op.virtual_register).or_default() += 1;
            }
        }
    }
    def_counts
        .into_iter()
        .filter_map(|(id, count)| (count > 1).then_some(id))
        .collect()
}

fn analyze_non_escaping_slots(
    function: &SelectedFunction,
    slot_sizes: &HashMap<LocalStorageSlotId, usize>,
    multi_def_vregs: &HashSet<VirtualRegisterId>,
) -> HashMap<usize, HashSet<LocalStorageSlotId>> {
    let mut slot_blocks: HashMap<LocalStorageSlotId, HashSet<usize>> = HashMap::new();
    let mut escaped: HashSet<LocalStorageSlotId> = HashSet::new();

    for (slot, &size) in slot_sizes {
        let candidate = matches!(
            slot,
            LocalStorageSlotId::Structural { .. } | LocalStorageSlotId::Inlined { .. }
        );
        if !candidate || size == 0 || size > MAX_TRACKED_SLOT_BYTES {
            escaped.insert(*slot);
        }
    }

    // A foreign call's result home is checked store-for-store against its
    // evaluated plan when frames are built (`structural_result_capture`), so
    // even a descriptor half the block never reads, such as a scalar
    // reference's codec forwarded only as an address, must stay written.
    for record in &function.normalized_foreign_calls {
        if let Some((operation, result)) = record
            .call
            .result
            .structural_home()
            .and_then(|home| home.operation_result())
        {
            escaped.insert(LocalStorageSlotId::Structural {
                operation,
                place: result.place,
            });
        }
    }

    // Track blocks mentioning each slot directly and disallow terminator/hosted uses.
    for (block_idx, block) in function.blocks.iter().enumerate() {
        for inst in &block.instructions {
            match &inst.kind {
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s),
                    byte_offset,
                } => {
                    slot_blocks.entry(*s).or_default().insert(block_idx);
                    let size = slot_sizes.get(s).copied().unwrap_or(0);
                    if (*byte_offset as usize) > size
                        || inst.operands.len() != 1
                        || multi_def_vregs.contains(&inst.operands[0].virtual_register)
                    {
                        escaped.insert(*s);
                    }
                }
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(s),
                    byte_offset,
                } => {
                    slot_blocks.entry(*s).or_default().insert(block_idx);
                    let size = slot_sizes.get(s).copied().unwrap_or(0);
                    if (*byte_offset as usize).saturating_add(8) > size {
                        escaped.insert(*s);
                    }
                }
                SelectedInstructionKind::HostedReadByte { slot }
                | SelectedInstructionKind::HostedWriteByteI32 { slot }
                | SelectedInstructionKind::SaveFloatingControl { slot }
                | SelectedInstructionKind::RestoreFloatingControl { slot } => {
                    escaped.insert(*slot);
                }
                _ => {}
            }
        }
        let term = terminator_instruction(&block.terminator);
        if let SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(s),
            ..
        }
        | SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(s),
            ..
        }
        | SelectedInstructionKind::HostedReadByte { slot: s }
        | SelectedInstructionKind::HostedWriteByteI32 { slot: s }
        | SelectedInstructionKind::SaveFloatingControl { slot: s }
        | SelectedInstructionKind::RestoreFloatingControl { slot: s } = &term.kind
        {
            escaped.insert(*s);
        }
        for succ in terminator_successors(&block.terminator) {
            for sb in &succ.structural_bindings {
                match sb.transport {
                    selected_instructions::SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Local(s),
                        destination,
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Local(s),
                        destination,
                        ..
                    } => {
                        escaped.insert(s);
                        escaped.insert(destination);
                    }
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        destination,
                        ..
                    } => {
                        escaped.insert(destination);
                    }
                    selected_instructions::SelectedStructuralTransport::Address {
                        destination,
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        destination,
                        ..
                    } => {
                        escaped.insert(destination);
                    }
                    selected_instructions::SelectedStructuralTransport::Unused => {}
                }
            }
        }
    }

    // Track local pointers across each single block and ensure no pointer escapes to another
    // block, terminator, stored value, call, or unsupported instruction.
    let mut ptr_to_slot_global: HashMap<VirtualRegisterId, (usize, LocalStorageSlotId, u32)> =
        HashMap::new();
    for (block_idx, block) in function.blocks.iter().enumerate() {
        for inst in &block.instructions {
            match &inst.kind {
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s),
                    byte_offset,
                } => {
                    if !escaped.contains(s)
                        && inst.operands.len() == 1
                        && !multi_def_vregs.contains(&inst.operands[0].virtual_register)
                    {
                        ptr_to_slot_global.insert(
                            inst.operands[0].virtual_register,
                            (block_idx, *s, *byte_offset),
                        );
                    }
                }
                SelectedInstructionKind::CopyI64 => {
                    if inst.operands.len() == 2 {
                        let src = inst.operands[0].virtual_register;
                        let dst = inst.operands[1].virtual_register;
                        if let Some(&(b, s, off)) = ptr_to_slot_global.get(&src) {
                            if b == block_idx && !multi_def_vregs.contains(&dst) {
                                ptr_to_slot_global.insert(dst, (b, s, off));
                            } else {
                                escaped.insert(s);
                            }
                        }
                    }
                }
                SelectedInstructionKind::AddressOffset { byte_offset }
                    if inst.operands.len() == 2 =>
                {
                    let src = inst.operands[0].virtual_register;
                    let dst = inst.operands[1].virtual_register;
                    if let Some(&(b, s, off)) = ptr_to_slot_global.get(&src) {
                        let size = slot_sizes.get(&s).copied().unwrap_or(0);
                        if b == block_idx
                            && !multi_def_vregs.contains(&dst)
                            && let Some(sum) = off.checked_add(*byte_offset)
                            && (sum as usize) <= size
                        {
                            ptr_to_slot_global.insert(dst, (b, s, sum));
                        } else {
                            escaped.insert(s);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Verify EVERY use of every local pointer across all blocks & terminators.
    for (block_idx, block) in function.blocks.iter().enumerate() {
        for inst in &block.instructions {
            for (op_pos, op) in inst.operands.iter().enumerate() {
                let vreg = op.virtual_register;
                if matches!(op.access, RegisterOperandAccess::Def) {
                    continue;
                }
                let Some(&(def_block, s, off)) = ptr_to_slot_global.get(&vreg) else {
                    continue;
                };
                if def_block != block_idx {
                    escaped.insert(s);
                    continue;
                }
                let size = slot_sizes.get(&s).copied().unwrap_or(0);
                let ok = match &inst.kind {
                    SelectedInstructionKind::CopyI64
                    | SelectedInstructionKind::AddressOffset { .. } => op_pos == 0,
                    SelectedInstructionKind::Load8 { byte_offset } => {
                        op_pos == 0 && (off as usize) + (*byte_offset as usize) < size
                    }
                    SelectedInstructionKind::Load16 { byte_offset } => {
                        op_pos == 0 && (off as usize) + (*byte_offset as usize) + 2 <= size
                    }
                    SelectedInstructionKind::Load32 { byte_offset } => {
                        op_pos == 0 && (off as usize) + (*byte_offset as usize) + 4 <= size
                    }
                    SelectedInstructionKind::Load64 { byte_offset } => {
                        op_pos == 0 && (off as usize) + (*byte_offset as usize) + 8 <= size
                    }
                    SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    } => {
                        op_pos == 0
                            && *byte_size > 0
                            && (off as usize) + (*byte_offset as usize) + (*byte_size as usize)
                                <= size
                    }
                    _ => {
                        if let Some(spec) = packed_op_spec(&inst.kind) {
                            if op_pos < spec.in_count {
                                (off as usize) + (spec.in_widths[op_pos] as usize) <= size
                            } else if op_pos == spec.in_count {
                                (off as usize) + (spec.out_bytes as usize) <= size
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                };
                if !ok {
                    escaped.insert(s);
                }
            }
        }

        let term = terminator_instruction(&block.terminator);
        for op in &term.operands {
            if let Some(&(_, s, _)) = ptr_to_slot_global.get(&op.virtual_register) {
                escaped.insert(s);
            }
        }
        for succ in terminator_successors(&block.terminator) {
            successor_registers(succ, |role, register| {
                if matches!(role, EdgeRegister::Argument | EdgeRegister::Structural)
                    && let Some(&(_, s, _)) = ptr_to_slot_global.get(&register)
                {
                    escaped.insert(s);
                }
            });
        }
    }

    let mut per_block: HashMap<usize, HashSet<LocalStorageSlotId>> = HashMap::new();
    for (slot, blocks) in slot_blocks {
        if blocks.len() == 1 && !escaped.contains(&slot) {
            let block_idx = *blocks.iter().next().unwrap();
            per_block.entry(block_idx).or_default().insert(slot);
        }
    }
    per_block
}

struct BlockCtx<'a> {
    environment: &'a ValidatedTargetRegisterEnvironment,
    keys: &'a SelectedConstraintKeys,
    packed_keys: &'a PackedRegKeys,
    vec128_class: RegisterClassId,
    gpr64_class: RegisterClassId,
    local_slots: &'a HashSet<LocalStorageSlotId>,
    multi_def_vregs: &'a HashSet<VirtualRegisterId>,
    next_inst_id: &'a mut u32,
    next_vreg_id: &'a mut u32,
    added_vregs: &'a mut Vec<VirtualRegister>,
    slot_bytes: HashMap<LocalStorageSlotId, Vec<SymByte>>,
    pending_vec_writes: HashMap<LocalStorageSlotId, Vec<PendingVecWrite>>,
    ptr_origin: HashMap<VirtualRegisterId, PtrOrigin>,
    gpr_const: HashMap<VirtualRegisterId, u64>,
    gpr_bytes: HashMap<VirtualRegisterId, [SymByte; 8]>,
    vec_high8_zero: HashMap<VirtualRegisterId, bool>,
    cached_zero_xmm: Option<VirtualRegisterId>,
    cached_const_xmm: HashMap<(u64, u64), VirtualRegisterId>,
    cached_const_gpr: HashMap<u64, VirtualRegisterId>,
    cached_local_addr: HashMap<LocalStorageSlotId, VirtualRegisterId>,
    gpr_linear_offset: HashMap<VirtualRegisterId, (VirtualRegisterId, u64)>,
    gpr_scaled: HashMap<VirtualRegisterId, (VirtualRegisterId, u64, u64)>,
    cached_scaled_mul: HashMap<(VirtualRegisterId, u64), VirtualRegisterId>,
    cached_indexed_base: HashMap<(VirtualRegisterId, VirtualRegisterId), VirtualRegisterId>,
    ext_store8_info: HashMap<SelectedInstructionId, ExtStore8Info>,
    ext_load8_origin: HashMap<VirtualRegisterId, ExtLoad8Origin>,
    ext_load_pair_vec: HashMap<(VirtualRegisterId, VirtualRegisterId), VirtualRegisterId>,
    ext_load_single_vec: HashMap<VirtualRegisterId, VirtualRegisterId>,
    folded_ext_load_groups: &'a mut Vec<FoldedExtLoadGroup>,
    mem_remap: &'a mut HashMap<SelectedInstructionId, Vec<(bool, SelectedInstructionId)>>,
    mem_delete: &'a mut HashSet<SelectedInstructionId>,
}

impl<'a> BlockCtx<'a> {
    fn invalidate_ext_loads(&mut self) {
        self.ext_load8_origin.clear();
        self.ext_load_pair_vec.clear();
        self.ext_load_single_vec.clear();
    }

    fn alloc_vreg(
        &mut self,
        class: RegisterClassId,
        scalar_type: ScalarType,
        inst_id: SelectedInstructionId,
        operand: u16,
    ) -> VirtualRegisterId {
        let id = VirtualRegisterId(*self.next_vreg_id);
        *self.next_vreg_id += 1;
        self.added_vregs.push(VirtualRegister {
            id,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction: inst_id,
                operand,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        id
    }

    fn alloc_vec_vreg(
        &mut self,
        inst_id: SelectedInstructionId,
        operand: u16,
    ) -> VirtualRegisterId {
        self.alloc_vreg(
            self.vec128_class,
            packed_vector_scalar_type(),
            inst_id,
            operand,
        )
    }

    fn alloc_gpr_vreg(
        &mut self,
        inst_id: SelectedInstructionId,
        operand: u16,
    ) -> VirtualRegisterId {
        let u64_ty = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 integer type");
        self.alloc_vreg(
            self.gpr64_class,
            ScalarType::Integer(u64_ty),
            inst_id,
            operand,
        )
    }

    fn make_inst(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        vregs: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<SelectedInstruction, SelectedInstructionError> {
        let id = SelectedInstructionId(*self.next_inst_id);
        *self.next_inst_id += 1;
        let row = constraint_row(self.environment, key)?;
        Ok(SelectedInstruction {
            id,
            kind,
            constraint: key,
            operands: make_operands(row, vregs),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance,
        })
    }

    fn emit_def_inst(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        uses: &[VirtualRegisterId],
        is_vec_def: bool,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(SelectedInstructionId, VirtualRegisterId), SelectedInstructionError> {
        let id = SelectedInstructionId(*self.next_inst_id);
        *self.next_inst_id += 1;
        let row = constraint_row(self.environment, key)?;
        let def_idx = row
            .operands
            .iter()
            .position(|o| matches!(o.access, RegisterOperandAccess::Def))
            .ok_or(SelectedInstructionError::MissingConstraint(key))?;
        let def_vreg = if is_vec_def {
            self.alloc_vec_vreg(id, def_idx as u16)
        } else {
            self.alloc_gpr_vreg(id, def_idx as u16)
        };
        let mut vregs = Vec::with_capacity(row.operands.len());
        let mut use_iter = uses.iter().copied();
        for (idx, op) in row.operands.iter().enumerate() {
            if idx == def_idx {
                vregs.push(def_vreg);
            } else {
                let u = use_iter
                    .next()
                    .ok_or(SelectedInstructionError::MissingConstraint(key))?;
                let _ = op;
                vregs.push(u);
            }
        }
        out.push(SelectedInstruction {
            id,
            kind,
            constraint: key,
            operands: make_operands(row, &vregs),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance,
        });
        Ok((id, def_vreg))
    }

    fn local_addr_vreg(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        slot: LocalStorageSlotId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if let Some(&addr) = self.cached_local_addr.get(&slot) {
            return Ok(addr);
        }
        let key = self
            .keys
            .frame_address
            .ok_or_else(SelectedInstructionError::custody)?;
        let (_, addr) = self.emit_def_inst(
            out,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            key,
            &[],
            false,
            SelectedInstructionProvenance::default(),
        )?;
        self.cached_local_addr.insert(slot, addr);
        self.ptr_origin
            .insert(addr, PtrOrigin::Local { slot, offset: 0 });
        Ok(addr)
    }

    fn resolve_ptr(&self, vreg: VirtualRegisterId) -> Option<PtrOrigin> {
        if let Some(&orig) = self.ptr_origin.get(&vreg) {
            return Some(orig);
        }
        if !self.multi_def_vregs.contains(&vreg) {
            return Some(PtrOrigin::External {
                base: vreg,
                offset: 0,
            });
        }
        None
    }

    fn gpr_bytes_or_symbolic(&self, vreg: VirtualRegisterId) -> [SymByte; 8] {
        if let Some(&bytes) = self.gpr_bytes.get(&vreg) {
            return bytes;
        }
        if !self.multi_def_vregs.contains(&vreg) {
            return std::array::from_fn(|i| SymByte::GprByte {
                vreg,
                byte_index: i as u8,
            });
        }
        [SymByte::Unknown; 8]
    }

    fn flush_pending_overlap(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        slot: LocalStorageSlotId,
        write_off: u32,
        write_len: u32,
        drop_fully_covered: bool,
    ) -> Result<(), SelectedInstructionError> {
        let Some(entries) = self.pending_vec_writes.get_mut(&slot) else {
            return Ok(());
        };
        let w_end = write_off.saturating_add(write_len);
        let mut to_flush = Vec::new();
        let mut kept = Vec::with_capacity(entries.len());
        for entry in entries.drain(..) {
            let e_end = entry.offset.saturating_add(entry.len);
            let overlaps = entry.offset < w_end && write_off < e_end;
            if !overlaps {
                kept.push(entry);
                continue;
            }
            let covered = drop_fully_covered && write_off <= entry.offset && e_end <= w_end;
            if !covered {
                to_flush.push(entry);
            }
        }
        *entries = kept;

        for entry in to_flush {
            let addr = self.local_addr_vreg(out, slot)?;
            let (kind, key) = match entry.len {
                16 => (
                    SelectedInstructionKind::PackedStore128 {
                        byte_offset: entry.offset,
                    },
                    self.packed_keys.store_128,
                ),
                8 => (
                    SelectedInstructionKind::PackedStore64 {
                        byte_offset: entry.offset,
                    },
                    self.packed_keys.store_64,
                ),
                4 => (
                    SelectedInstructionKind::PackedStore32 {
                        byte_offset: entry.offset,
                    },
                    self.packed_keys.store_32,
                ),
                _ => return Err(SelectedInstructionError::custody()),
            };
            let inst = self.make_inst(
                kind,
                key,
                &[addr, entry.vreg],
                SelectedInstructionProvenance::default(),
            )?;
            out.push(inst);
        }
        Ok(())
    }

    fn record_local_vec_write(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        slot: LocalStorageSlotId,
        offset: u32,
        len: u32,
        vreg: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        self.flush_pending_overlap(out, slot, offset, len, true)?;
        let bytes = self.slot_bytes.get_mut(&slot).unwrap();
        for i in 0..(len as usize) {
            bytes[(offset as usize) + i] = SymByte::VecLaneByte {
                vreg,
                byte_index: i as u8,
            };
        }
        self.pending_vec_writes
            .entry(slot)
            .or_default()
            .push(PendingVecWrite { offset, len, vreg });
        Ok(())
    }

    fn get_or_materialize_zero(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        prov: Option<SelectedInstructionProvenance>,
    ) -> Result<(Option<SelectedInstructionId>, VirtualRegisterId), SelectedInstructionError> {
        if prov.as_ref().is_none_or(is_pure_provenance)
            && let Some(z) = self.cached_zero_xmm
        {
            return Ok((None, z));
        }
        let p = prov.unwrap_or_default();
        let is_pure = is_pure_provenance(&p);
        let (id, z) = self.emit_def_inst(
            out,
            SelectedInstructionKind::PackedMaterializeZero,
            self.packed_keys.materialize_zero,
            &[],
            true,
            p,
        )?;
        self.vec_high8_zero.insert(z, true);
        if is_pure {
            self.cached_zero_xmm = Some(z);
        }
        Ok((Some(id), z))
    }

    fn get_or_materialize_gpr_const(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        val: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if let Some(&r) = self.cached_const_gpr.get(&val) {
            return Ok(r);
        }
        let (_, r) = self.emit_def_inst(
            out,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(val)),
            },
            self.keys.materialize_i64,
            &[],
            false,
            SelectedInstructionProvenance::default(),
        )?;
        self.cached_const_gpr.insert(val, r);
        self.gpr_const.insert(r, val);
        self.gpr_bytes.insert(r, const_bytes_u64(val));
        Ok(r)
    }

    fn get_or_materialize_vec_const(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        lo: u64,
        hi: u64,
        prov: Option<SelectedInstructionProvenance>,
    ) -> Result<(Option<SelectedInstructionId>, VirtualRegisterId), SelectedInstructionError> {
        if lo == 0 && hi == 0 {
            return self.get_or_materialize_zero(out, prov);
        }
        if prov.as_ref().is_none_or(is_pure_provenance)
            && let Some(&v) = self.cached_const_xmm.get(&(lo, hi))
        {
            return Ok((None, v));
        }
        let p = prov.unwrap_or_default();
        let is_pure = is_pure_provenance(&p);
        let (id, v) = if hi == 0 {
            let g_lo = self.get_or_materialize_gpr_const(out, lo)?;
            let (id, v) = self.emit_def_inst(
                out,
                SelectedInstructionKind::PackedFromGpr64ZeroExtend,
                self.packed_keys.from_gpr64_zero_extend,
                &[g_lo],
                true,
                p,
            )?;
            self.vec_high8_zero.insert(v, true);
            (id, v)
        } else {
            let g_lo = self.get_or_materialize_gpr_const(out, lo)?;
            let g_hi = self.get_or_materialize_gpr_const(out, hi)?;
            let (id, v) = self.emit_def_inst(
                out,
                SelectedInstructionKind::PackedFromGpr64Pair,
                self.packed_keys.from_gpr64_pair,
                &[g_lo, g_hi],
                true,
                p,
            )?;
            self.vec_high8_zero.insert(v, false);
            (id, v)
        };
        if is_pure {
            self.cached_const_xmm.insert((lo, hi), v);
        }
        Ok((Some(id), v))
    }

    /// Resolve an N-byte (`4`, `8`, or `16`) packed input operand into a `VECTOR128` register.
    /// Returns `(optional_read_inst_id_for_mem_access, vreg)`.
    fn resolve_packed_input(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        ptr_vreg: VirtualRegisterId,
        width: u32,
        need_zero_ext_to_16: bool,
        prov_for_only_op: Option<SelectedInstructionProvenance>,
    ) -> Result<(Option<SelectedInstructionId>, VirtualRegisterId), SelectedInstructionError> {
        let Some(ptr) = self.resolve_ptr(ptr_vreg) else {
            return Err(SelectedInstructionError::custody());
        };
        let p_pure = prov_for_only_op.as_ref().is_none_or(is_pure_provenance);
        match ptr {
            PtrOrigin::External { base, offset } => {
                let (kind, key, high_zero) = match width {
                    16 => (
                        SelectedInstructionKind::PackedLoad128 {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_128,
                        false,
                    ),
                    8 => (
                        SelectedInstructionKind::PackedLoad64ZeroExtend {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_64,
                        true,
                    ),
                    4 => (
                        SelectedInstructionKind::PackedLoad32ZeroExtend {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_32,
                        true,
                    ),
                    _ => return Err(SelectedInstructionError::custody()),
                };
                let (id, v) = self.emit_def_inst(
                    out,
                    kind,
                    key,
                    &[base],
                    true,
                    prov_for_only_op.unwrap_or_default(),
                )?;
                self.vec_high8_zero.insert(v, high_zero);
                Ok((Some(id), v))
            }
            PtrOrigin::Local { slot, offset } => {
                let slice: Vec<SymByte> = self.slot_bytes[&slot]
                    [(offset as usize)..(offset as usize + width as usize)]
                    .to_vec();

                // 1. All constant bytes -> zero or GPR constant materialization.
                if let Some((lo, hi)) = extract_const_pair(&slice) {
                    let (id_opt, v) =
                        self.get_or_materialize_vec_const(out, lo, hi, prov_for_only_op.clone())?;
                    return Ok((id_opt, v));
                }

                // 2. Exact full or low VecLaneByte match.
                if let Some(v) = match_vec_lanes(&slice, 0) {
                    let high_ok = !need_zero_ext_to_16
                        || width == 16
                        || (width == 8 && self.vec_high8_zero.get(&v).copied().unwrap_or(false));
                    if high_ok && p_pure {
                        return Ok((None, v));
                    }
                }

                // 3. Exact GprByte match (only when upper bytes zero if width == 4).
                if width == 8
                    && let Some(g) = match_gpr_bytes(&slice[..8])
                {
                    if let Some(&orig) = self.ext_load8_origin.get(&g) {
                        if p_pure && let Some(&v) = self.ext_load_single_vec.get(&g) {
                            return Ok((None, v));
                        }
                        let (id, v) = self.emit_def_inst(
                            out,
                            SelectedInstructionKind::PackedLoad64ZeroExtend {
                                byte_offset: orig.offset,
                            },
                            self.packed_keys.load_64,
                            &[orig.base],
                            true,
                            prov_for_only_op.unwrap_or_default(),
                        )?;
                        self.vec_high8_zero.insert(v, true);
                        if p_pure {
                            self.ext_load_single_vec.insert(g, v);
                        }
                        self.folded_ext_load_groups.push(FoldedExtLoadGroup {
                            lo_id: orig.inst_id,
                            hi_id: None,
                            replacement_id: id,
                        });
                        return Ok((Some(id), v));
                    }
                    let (id, v) = self.emit_def_inst(
                        out,
                        SelectedInstructionKind::PackedFromGpr64ZeroExtend,
                        self.packed_keys.from_gpr64_zero_extend,
                        &[g],
                        true,
                        prov_for_only_op.unwrap_or_default(),
                    )?;
                    self.vec_high8_zero.insert(v, true);
                    return Ok((Some(id), v));
                }
                if width == 4
                    && let Some(g) = match_gpr_bytes(&slice[..4])
                    && self
                        .gpr_bytes
                        .get(&g)
                        .is_some_and(|b| b[4..8] == [SymByte::Const(0); 4])
                {
                    let (id, v) = self.emit_def_inst(
                        out,
                        SelectedInstructionKind::PackedFromGpr64ZeroExtend,
                        self.packed_keys.from_gpr64_zero_extend,
                        &[g],
                        true,
                        prov_for_only_op.unwrap_or_default(),
                    )?;
                    self.vec_high8_zero.insert(v, true);
                    return Ok((Some(id), v));
                }
                if width == 16
                    && let Some(g_lo) = match_gpr_bytes(&slice[0..8])
                {
                    if slice[8..16].iter().all(|b| matches!(b, SymByte::Const(0))) {
                        if let Some(&orig) = self.ext_load8_origin.get(&g_lo) {
                            if p_pure && let Some(&v) = self.ext_load_single_vec.get(&g_lo) {
                                return Ok((None, v));
                            }
                            let (id, v) = self.emit_def_inst(
                                out,
                                SelectedInstructionKind::PackedLoad64ZeroExtend {
                                    byte_offset: orig.offset,
                                },
                                self.packed_keys.load_64,
                                &[orig.base],
                                true,
                                prov_for_only_op.unwrap_or_default(),
                            )?;
                            self.vec_high8_zero.insert(v, true);
                            if p_pure {
                                self.ext_load_single_vec.insert(g_lo, v);
                            }
                            self.folded_ext_load_groups.push(FoldedExtLoadGroup {
                                lo_id: orig.inst_id,
                                hi_id: None,
                                replacement_id: id,
                            });
                            return Ok((Some(id), v));
                        }
                        let (id, v) = self.emit_def_inst(
                            out,
                            SelectedInstructionKind::PackedFromGpr64ZeroExtend,
                            self.packed_keys.from_gpr64_zero_extend,
                            &[g_lo],
                            true,
                            prov_for_only_op.unwrap_or_default(),
                        )?;
                        self.vec_high8_zero.insert(v, true);
                        return Ok((Some(id), v));
                    }
                    if let Some(g_hi) = match_gpr_bytes(&slice[8..16]) {
                        if let (Some(&orig_lo), Some(&orig_hi)) = (
                            self.ext_load8_origin.get(&g_lo),
                            self.ext_load8_origin.get(&g_hi),
                        ) && orig_lo.base == orig_hi.base
                            && orig_lo.offset.checked_add(8) == Some(orig_hi.offset)
                        {
                            if p_pure && let Some(&v) = self.ext_load_pair_vec.get(&(g_lo, g_hi)) {
                                return Ok((None, v));
                            }
                            let (id, v) = self.emit_def_inst(
                                out,
                                SelectedInstructionKind::PackedLoad128 {
                                    byte_offset: orig_lo.offset,
                                },
                                self.packed_keys.load_128,
                                &[orig_lo.base],
                                true,
                                prov_for_only_op.unwrap_or_default(),
                            )?;
                            self.vec_high8_zero.insert(v, false);
                            if p_pure {
                                self.ext_load_pair_vec.insert((g_lo, g_hi), v);
                            }
                            self.folded_ext_load_groups.push(FoldedExtLoadGroup {
                                lo_id: orig_lo.inst_id,
                                hi_id: Some(orig_hi.inst_id),
                                replacement_id: id,
                            });
                            return Ok((Some(id), v));
                        }
                        let (id, v) = self.emit_def_inst(
                            out,
                            SelectedInstructionKind::PackedFromGpr64Pair,
                            self.packed_keys.from_gpr64_pair,
                            &[g_lo, g_hi],
                            true,
                            prov_for_only_op.unwrap_or_default(),
                        )?;
                        self.vec_high8_zero.insert(v, false);
                        return Ok((Some(id), v));
                    }
                }

                // 4. Memory load from Local(slot): flush any deferred overlapping vec writes first.
                self.flush_pending_overlap(out, slot, offset, width, false)?;
                let addr = self.local_addr_vreg(out, slot)?;
                let (kind, key, high_zero) = match width {
                    16 => (
                        SelectedInstructionKind::PackedLoad128 {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_128,
                        false,
                    ),
                    8 => (
                        SelectedInstructionKind::PackedLoad64ZeroExtend {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_64,
                        true,
                    ),
                    4 => (
                        SelectedInstructionKind::PackedLoad32ZeroExtend {
                            byte_offset: offset,
                        },
                        self.packed_keys.load_32,
                        true,
                    ),
                    _ => return Err(SelectedInstructionError::custody()),
                };
                let (id, v) = self.emit_def_inst(
                    out,
                    kind,
                    key,
                    &[addr],
                    true,
                    prov_for_only_op.unwrap_or_default(),
                )?;
                self.vec_high8_zero.insert(v, high_zero);
                Ok((Some(id), v))
            }
        }
    }

    fn promote_block(&mut self, block: &mut SelectedBlock) -> Result<(), SelectedInstructionError> {
        let orig_insts = std::mem::take(&mut block.instructions);
        let block_local_ptrs =
            collect_block_local_ptrs(&orig_insts, self.local_slots, self.multi_def_vregs);
        let mut out: Vec<SelectedInstruction> = Vec::with_capacity(orig_insts.len());

        for idx in 0..orig_insts.len() {
            let inst = &orig_insts[idx];
            match &inst.kind {
                SelectedInstructionKind::MaterializeI64 { value } => {
                    if let Some(dst) = inst.operands.first().map(|o| o.virtual_register)
                        && !self.multi_def_vregs.contains(&dst)
                        && let Some(c) = integer_value_to_u64(*value)
                    {
                        self.gpr_const.insert(dst, c);
                        self.gpr_bytes.insert(dst, const_bytes_u64(c));
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset,
                } => {
                    if self.local_slots.contains(slot)
                        && let Some(dst) = inst.operands.first().map(|o| o.virtual_register)
                        && !self.multi_def_vregs.contains(&dst)
                    {
                        self.ptr_origin.insert(
                            dst,
                            PtrOrigin::Local {
                                slot: *slot,
                                offset: *byte_offset,
                            },
                        );
                        if *byte_offset == 0 {
                            self.cached_local_addr.entry(*slot).or_insert(dst);
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::CopyI64 => {
                    if inst.operands.len() == 2 {
                        let src = inst.operands[0].virtual_register;
                        let dst = inst.operands[1].virtual_register;
                        if !self.multi_def_vregs.contains(&dst) {
                            if let Some(orig) = self.resolve_ptr(src) {
                                self.ptr_origin.insert(dst, orig);
                            }
                            if let Some(&c) = self.gpr_const.get(&src) {
                                self.gpr_const.insert(dst, c);
                            }
                            if let Some(&b) = self.gpr_bytes.get(&src) {
                                self.gpr_bytes.insert(dst, b);
                            } else {
                                self.gpr_bytes.insert(dst, self.gpr_bytes_or_symbolic(src));
                            }
                            if let Some(&lin) = self.gpr_linear_offset.get(&src) {
                                self.gpr_linear_offset.insert(dst, lin);
                            }
                            if let Some(&sc) = self.gpr_scaled.get(&src) {
                                self.gpr_scaled.insert(dst, sc);
                            }
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::WrappingAddI64 => {
                    if inst.operands.len() == 3 {
                        let lhs = inst.operands[0].virtual_register;
                        let rhs = inst.operands[1].virtual_register;
                        let dst = inst.operands[2].virtual_register;
                        if !self.multi_def_vregs.contains(&dst) {
                            let matched = if let Some(&k) = self.gpr_const.get(&rhs) {
                                (!self.multi_def_vregs.contains(&lhs)).then_some((lhs, k))
                            } else if let Some(&k) = self.gpr_const.get(&lhs) {
                                (!self.multi_def_vregs.contains(&rhs)).then_some((rhs, k))
                            } else {
                                None
                            };
                            if let Some((var, k)) = matched {
                                let (root, prev_k) = self
                                    .gpr_linear_offset
                                    .get(&var)
                                    .copied()
                                    .unwrap_or((var, 0));
                                self.gpr_linear_offset
                                    .insert(dst, (root, prev_k.wrapping_add(k)));
                            }
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
                    if inst.operands.len() == 2 {
                        let var = inst.operands[0].virtual_register;
                        let dst = inst.operands[1].virtual_register;
                        if !self.multi_def_vregs.contains(&dst)
                            && !self.multi_def_vregs.contains(&var)
                            && let Some(k) = integer_value_to_u64(*immediate)
                        {
                            let (root, prev_k) = self
                                .gpr_linear_offset
                                .get(&var)
                                .copied()
                                .unwrap_or((var, 0));
                            self.gpr_linear_offset
                                .insert(dst, (root, prev_k.wrapping_add(k)));
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::WrappingMultiplyI64 => {
                    if inst.operands.len() == 3 {
                        let lhs = inst.operands[0].virtual_register;
                        let rhs = inst.operands[1].virtual_register;
                        let dst = inst.operands[2].virtual_register;
                        if !self.multi_def_vregs.contains(&dst) {
                            let matched = if let Some(&stride) = self.gpr_const.get(&rhs) {
                                (!self.multi_def_vregs.contains(&lhs)).then_some((lhs, stride))
                            } else if let Some(&stride) = self.gpr_const.get(&lhs) {
                                (!self.multi_def_vregs.contains(&rhs)).then_some((rhs, stride))
                            } else {
                                None
                            };
                            if let Some((idx, stride)) = matched {
                                let (root_idx, k) = self
                                    .gpr_linear_offset
                                    .get(&idx)
                                    .copied()
                                    .unwrap_or((idx, 0));
                                let extra = k.wrapping_mul(stride);
                                self.gpr_scaled.insert(dst, (root_idx, stride, extra));
                                if k == 0 {
                                    self.cached_scaled_mul
                                        .entry((root_idx, stride))
                                        .or_insert(dst);
                                }
                            }
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::ByteViewAddress => {
                    out.push(inst.clone());
                    if inst.operands.len() == 3 {
                        let lhs = inst.operands[0].virtual_register;
                        let rhs = inst.operands[1].virtual_register;
                        let dst = inst.operands[2].virtual_register;
                        if !self.multi_def_vregs.contains(&dst) {
                            let matched = [(lhs, rhs), (rhs, lhs)].into_iter().find_map(
                                |(ptr_op, idx_op)| {
                                    let Some(PtrOrigin::External {
                                        base,
                                        offset: off_l,
                                    }) = self.resolve_ptr(ptr_op)
                                    else {
                                        return None;
                                    };
                                    if self.multi_def_vregs.contains(&base) {
                                        return None;
                                    }
                                    let &(root_idx, stride, extra_off) =
                                        self.gpr_scaled.get(&idx_op)?;
                                    let total_u64 = u64::from(off_l).checked_add(extra_off)?;
                                    if total_u64 > i32::MAX as u64 {
                                        return None;
                                    }
                                    Some((
                                        base,
                                        off_l,
                                        root_idx,
                                        stride,
                                        extra_off,
                                        total_u64 as u32,
                                    ))
                                },
                            );
                            if let Some((base, off_l, root_idx, stride, extra_off, total_off)) =
                                matched
                            {
                                let scaled_vreg = if let Some(&m) =
                                    self.cached_scaled_mul.get(&(root_idx, stride))
                                {
                                    m
                                } else {
                                    let c_stride =
                                        self.get_or_materialize_gpr_const(&mut out, stride)?;
                                    let (_, m) = self.emit_def_inst(
                                        &mut out,
                                        SelectedInstructionKind::WrappingMultiplyI64,
                                        self.keys.multiply_i64,
                                        &[c_stride, root_idx],
                                        false,
                                        SelectedInstructionProvenance::default(),
                                    )?;
                                    self.cached_scaled_mul.insert((root_idx, stride), m);
                                    m
                                };
                                let indexed_base = if let Some(&b) =
                                    self.cached_indexed_base.get(&(base, scaled_vreg))
                                {
                                    b
                                } else if off_l == 0 && extra_off == 0 {
                                    self.cached_indexed_base.insert((base, scaled_vreg), dst);
                                    dst
                                } else {
                                    let (_, b) = self.emit_def_inst(
                                        &mut out,
                                        SelectedInstructionKind::ByteViewAddress,
                                        inst.constraint,
                                        &[base, scaled_vreg],
                                        false,
                                        SelectedInstructionProvenance::default(),
                                    )?;
                                    self.cached_indexed_base.insert((base, scaled_vreg), b);
                                    b
                                };
                                self.ptr_origin.insert(
                                    dst,
                                    PtrOrigin::External {
                                        base: indexed_base,
                                        offset: total_off,
                                    },
                                );
                            }
                        }
                    }
                }
                SelectedInstructionKind::AddressOffset { byte_offset } => {
                    if inst.operands.len() == 2 {
                        let src = inst.operands[0].virtual_register;
                        let dst = inst.operands[1].virtual_register;
                        if !self.multi_def_vregs.contains(&dst)
                            && let Some(orig) = self.resolve_ptr(src)
                        {
                            match orig {
                                PtrOrigin::Local { slot, offset } => {
                                    if let Some(sum) = offset.checked_add(*byte_offset) {
                                        self.ptr_origin
                                            .insert(dst, PtrOrigin::Local { slot, offset: sum });
                                    }
                                }
                                PtrOrigin::External { base, offset } => {
                                    if let Some(sum) = offset.checked_add(*byte_offset)
                                        && sum <= i32::MAX as u32
                                    {
                                        self.ptr_origin
                                            .insert(dst, PtrOrigin::External { base, offset: sum });
                                    }
                                }
                            }
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::Load8 { byte_offset }
                | SelectedInstructionKind::Load16 { byte_offset }
                | SelectedInstructionKind::Load32 { byte_offset }
                | SelectedInstructionKind::Load64 { byte_offset } => {
                    let width: u32 = match inst.kind {
                        SelectedInstructionKind::Load8 { .. } => 1,
                        SelectedInstructionKind::Load16 { .. } => 2,
                        SelectedInstructionKind::Load32 { .. } => 4,
                        SelectedInstructionKind::Load64 { .. } => 8,
                        _ => unreachable!(),
                    };
                    if inst.operands.len() == 2
                        && let Some(PtrOrigin::Local { slot, offset }) =
                            self.resolve_ptr(inst.operands[0].virtual_register)
                    {
                        let dst = inst.operands[1].virtual_register;
                        let total_off = offset + *byte_offset;
                        let can_elide_flush = self.load_overlaps_pending(slot, total_off, width)
                            && self.dst_consumed_only_symbolically(
                                &orig_insts[(idx + 1)..],
                                block,
                                &block_local_ptrs,
                                dst,
                                &self.slot_bytes[&slot]
                                    [(total_off as usize)..(total_off as usize + width as usize)],
                            );
                        if !can_elide_flush {
                            self.flush_pending_overlap(&mut out, slot, total_off, width, false)?;
                        }
                        if !self.multi_def_vregs.contains(&dst) {
                            let mut bytes = [SymByte::Const(0); 8];
                            let src_slice = &self.slot_bytes[&slot]
                                [(total_off as usize)..(total_off as usize + width as usize)];
                            bytes[..(width as usize)].copy_from_slice(src_slice);
                            self.gpr_bytes.insert(dst, bytes);
                            if let Some((c, _)) = extract_const_pair(&bytes) {
                                self.gpr_const.insert(dst, c);
                            }
                        }
                    } else if matches!(inst.kind, SelectedInstructionKind::Load64 { .. })
                        && inst.operands.len() == 2
                        && is_pure_provenance(&inst.provenance)
                        && let Some(PtrOrigin::External { base, offset }) =
                            self.resolve_ptr(inst.operands[0].virtual_register)
                    {
                        let dst = inst.operands[1].virtual_register;
                        if !self.multi_def_vregs.contains(&base)
                            && !self.multi_def_vregs.contains(&dst)
                            && let Some(eff_off) = offset.checked_add(*byte_offset)
                            && eff_off <= i32::MAX as u32
                        {
                            self.ext_load8_origin.insert(
                                dst,
                                ExtLoad8Origin {
                                    base,
                                    offset: eff_off,
                                    inst_id: inst.id,
                                },
                            );
                        }
                    }
                    out.push(inst.clone());
                }
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset,
                } if self.local_slots.contains(slot) => {
                    let val = inst.operands[0].virtual_register;
                    self.flush_pending_overlap(&mut out, *slot, *byte_offset, 8, true)?;
                    let bytes = self.gpr_bytes_or_symbolic(val);
                    let target = self.slot_bytes.get_mut(slot).unwrap();
                    target[(*byte_offset as usize)..(*byte_offset as usize + 8)]
                        .copy_from_slice(&bytes);
                    out.push(inst.clone());
                }
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                } => {
                    let ptr = inst.operands[0].virtual_register;
                    let val = inst.operands[1].virtual_register;
                    match self.resolve_ptr(ptr) {
                        Some(PtrOrigin::Local { slot, offset }) => {
                            let off = offset + *byte_offset;
                            let sz = *byte_size as u32;
                            self.flush_pending_overlap(&mut out, slot, off, sz, true)?;
                            let src_bytes = self.gpr_bytes_or_symbolic(val);
                            let target = self.slot_bytes.get_mut(&slot).unwrap();
                            target[(off as usize)..(off as usize + sz as usize)]
                                .copy_from_slice(&src_bytes[..(sz as usize)]);
                            out.push(inst.clone());
                        }
                        Some(PtrOrigin::External { base, offset })
                            if offset
                                .checked_add(*byte_offset)
                                .is_some_and(|s| s <= i32::MAX as u32) =>
                        {
                            let eff_off = offset + *byte_offset;
                            let src_bytes = self.gpr_bytes_or_symbolic(val);
                            if *byte_size == 4
                                && let Some(v) = match_vec_lanes(&src_bytes[0..4], 0)
                            {
                                let store_inst = self.make_inst(
                                    SelectedInstructionKind::PackedStore32 {
                                        byte_offset: eff_off,
                                    },
                                    self.packed_keys.store_32,
                                    &[base, v],
                                    inst.provenance.clone(),
                                )?;
                                self.mem_remap
                                    .entry(inst.id)
                                    .or_default()
                                    .push((false, store_inst.id));
                                out.push(store_inst);
                            } else if *byte_size == 8
                                && let Some(v) = match_vec_lanes(&src_bytes[0..8], 0)
                            {
                                let store_inst = self.make_inst(
                                    SelectedInstructionKind::PackedStore64 {
                                        byte_offset: eff_off,
                                    },
                                    self.packed_keys.store_64,
                                    &[base, v],
                                    inst.provenance.clone(),
                                )?;
                                self.ext_store8_info.insert(
                                    store_inst.id,
                                    ExtStore8Info {
                                        base,
                                        offset: eff_off,
                                        source: ExtStore8Source::VecLow8(v),
                                    },
                                );
                                self.mem_remap
                                    .entry(inst.id)
                                    .or_default()
                                    .push((false, store_inst.id));
                                out.push(store_inst);
                            } else {
                                if *byte_size == 8 {
                                    if src_bytes.iter().all(|b| matches!(b, SymByte::Const(0))) {
                                        self.ext_store8_info.insert(
                                            inst.id,
                                            ExtStore8Info {
                                                base,
                                                offset: eff_off,
                                                source: ExtStore8Source::Zero,
                                            },
                                        );
                                    } else if let Some(v) = match_vec_lanes(&src_bytes[0..8], 8) {
                                        self.ext_store8_info.insert(
                                            inst.id,
                                            ExtStore8Info {
                                                base,
                                                offset: eff_off,
                                                source: ExtStore8Source::VecHigh8(v),
                                            },
                                        );
                                    }
                                }
                                out.push(inst.clone());
                            }
                            self.invalidate_ext_loads();
                        }
                        _ => {
                            self.invalidate_ext_loads();
                            out.push(inst.clone());
                        }
                    }
                }
                _ => {
                    if let Some(spec) = packed_op_spec(&inst.kind) {
                        self.promote_packed_inst(&mut out, inst, spec)?;
                    } else {
                        self.invalidate_ext_loads();
                        out.push(inst.clone());
                    }
                }
            }
        }

        // Remaining deferred Local writes are never read inside the block (and non-escaping).
        self.pending_vec_writes.clear();
        block.instructions = out;
        Ok(())
    }

    fn load_overlaps_pending(&self, slot: LocalStorageSlotId, off: u32, len: u32) -> bool {
        let Some(entries) = self.pending_vec_writes.get(&slot) else {
            return false;
        };
        let end = off.saturating_add(len);
        entries
            .iter()
            .any(|e| e.offset < end && off < e.offset.saturating_add(e.len))
    }

    /// Conservatively check whether every downstream use of `loaded_vreg` (within this block)
    /// consumes `loaded_slice` purely through Local copies or promotable External stores (including
    /// high-8-byte partner stores on the same base), meaning the memory load can avoid flushing.
    fn dst_consumed_only_symbolically(
        &self,
        rest: &[SelectedInstruction],
        block: &SelectedBlock,
        block_local_ptrs: &HashMap<VirtualRegisterId, PtrOrigin>,
        loaded_vreg: VirtualRegisterId,
        loaded_slice: &[SymByte],
    ) -> bool {
        if self.multi_def_vregs.contains(&loaded_vreg) {
            return false;
        }
        let term = terminator_instruction(&block.terminator);
        if term
            .operands
            .iter()
            .any(|op| op.virtual_register == loaded_vreg)
        {
            return false;
        }
        for succ in terminator_successors(&block.terminator) {
            if succ.bindings.iter().any(|b| {
                matches!(
                    b.transport,
                    selected_instructions::SelectedValueTransport::Registers { argument, .. }
                        if argument == loaded_vreg
                )
            }) {
                return false;
            }
            for sb in &succ.structural_bindings {
                let uses_vreg = match sb.transport {
                    selected_instructions::SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(r),
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Register(r),
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::WholeValue {
                        argument: r,
                        ..
                    } => r == loaded_vreg,
                    _ => false,
                };
                if uses_vreg {
                    return false;
                }
            }
        }

        // Only permit CopyI64, Store64(Local), Store(Local), or Store(External) matching a
        // promotable pattern; no chained external reads or arithmetic.
        let mut tracked: HashSet<VirtualRegisterId> = HashSet::new();
        tracked.insert(loaded_vreg);
        let is_low_vec = (loaded_slice.len() == 4 || loaded_slice.len() == 8)
            && match_vec_lanes(loaded_slice, 0).is_some();
        let is_high_vec = loaded_slice.len() == 8 && match_vec_lanes(loaded_slice, 8).is_some();
        let is_const = extract_const_pair(loaded_slice).is_some();

        // Check in forward order while also tracking CopyI64 derivatives ONLY if transitive
        // CopyI64 chains don't leak to terminator (check conservatively: disallow CopyI64 unless
        // single-use? Actually check every instruction against tracked!).
        let mut copy_origin_local = HashMap::new();
        for (at, inst) in rest.iter().enumerate() {
            match &inst.kind {
                SelectedInstructionKind::CopyI64
                    if inst.operands.len() == 2
                        && tracked.contains(&inst.operands[0].virtual_register) =>
                {
                    let dst = inst.operands[1].virtual_register;
                    if self.multi_def_vregs.contains(&dst) {
                        return false;
                    }
                    if !self.dst_only_local_or_promotable_store(
                        rest,
                        block,
                        block_local_ptrs,
                        dst,
                        is_low_vec,
                        is_high_vec,
                        is_const,
                    ) {
                        return false;
                    }
                    tracked.insert(dst);
                    copy_origin_local.insert(dst, true);
                }
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                } if self.local_slots.contains(s) => {
                    if tracked.contains(&inst.operands[0].virtual_register)
                        && slot_read_later(&rest[(at + 1)..], block_local_ptrs, *s)
                    {
                        return false;
                    }
                }
                SelectedInstructionKind::Store { byte_size, .. } => {
                    if inst.operands[0].virtual_register != loaded_vreg
                        && tracked.contains(&inst.operands[0].virtual_register)
                    {
                        return false;
                    }
                    if tracked.contains(&inst.operands[1].virtual_register) {
                        let ptr = inst.operands[0].virtual_register;
                        match block_local_ptrs
                            .get(&ptr)
                            .copied()
                            .or_else(|| self.resolve_ptr(ptr))
                        {
                            Some(PtrOrigin::Local { slot, .. }) => {
                                if slot_read_later(&rest[(at + 1)..], block_local_ptrs, slot) {
                                    return false;
                                }
                            }
                            Some(PtrOrigin::External { .. }) => {
                                if (*byte_size == 4 && loaded_slice.len() == 4 && is_low_vec)
                                    || (*byte_size == 8 && loaded_slice.len() == 8 && is_low_vec)
                                {
                                    // Will emit PackedStore32 / PackedStore64 directly.
                                } else if *byte_size == 8 && is_const {
                                    // Wait: external Store(Const) still uses loaded_vreg unless
                                    // coalesced with zero partner! Conservatively flush if Const.
                                    return false;
                                } else if *byte_size == 8 && is_high_vec {
                                    // High half cannot be emitted without partner coalescing ->
                                    // conservatively require flush unless coalesced cleanly.
                                    return false;
                                } else {
                                    return false;
                                }
                            }
                            None => return false,
                        }
                    }
                }
                _ => {
                    if inst
                        .operands
                        .iter()
                        .any(|op| tracked.contains(&op.virtual_register))
                    {
                        return false;
                    }
                }
            }
        }
        let _ = copy_origin_local;
        true
    }

    fn dst_only_local_or_promotable_store(
        &self,
        rest: &[SelectedInstruction],
        block: &SelectedBlock,
        block_local_ptrs: &HashMap<VirtualRegisterId, PtrOrigin>,
        vreg: VirtualRegisterId,
        is_low_vec: bool,
        is_high_vec: bool,
        is_const: bool,
    ) -> bool {
        let term = terminator_instruction(&block.terminator);
        if term.operands.iter().any(|op| op.virtual_register == vreg) {
            return false;
        }
        for succ in terminator_successors(&block.terminator) {
            if succ.bindings.iter().any(|b| {
                matches!(
                    b.transport,
                    selected_instructions::SelectedValueTransport::Registers { argument, .. }
                        if argument == vreg
                )
            }) {
                return false;
            }
        }
        for (at, inst) in rest.iter().enumerate() {
            for (pos, op) in inst.operands.iter().enumerate() {
                if op.virtual_register != vreg || matches!(op.access, RegisterOperandAccess::Def) {
                    continue;
                }
                match &inst.kind {
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(s),
                        ..
                    } if self.local_slots.contains(s) && pos == 0 => {
                        if slot_read_later(&rest[(at + 1)..], block_local_ptrs, *s) {
                            return false;
                        }
                    }
                    SelectedInstructionKind::Store { byte_size, .. } if pos == 1 => {
                        let ptr = inst.operands[0].virtual_register;
                        match block_local_ptrs
                            .get(&ptr)
                            .copied()
                            .or_else(|| self.resolve_ptr(ptr))
                        {
                            Some(PtrOrigin::Local { slot, .. }) => {
                                if slot_read_later(&rest[(at + 1)..], block_local_ptrs, slot) {
                                    return false;
                                }
                            }
                            Some(PtrOrigin::External { .. }) => {
                                let ok = (*byte_size == 4 || *byte_size == 8) && is_low_vec;
                                let _ = (is_high_vec, is_const);
                                if !ok {
                                    return false;
                                }
                            }
                            None => return false,
                        }
                    }
                    _ => return false,
                }
            }
        }
        true
    }

    fn promote_packed_inst(
        &mut self,
        out: &mut Vec<SelectedInstruction>,
        inst: &SelectedInstruction,
        spec: PackedOpSpec,
    ) -> Result<(), SelectedInstructionError> {
        let mut remapped_reads: Vec<SelectedInstructionId> = Vec::new();
        let mut inputs = Vec::with_capacity(spec.in_count);

        // For single-instruction LoadI16x4ZeroExtend, pass provenance directly to input resolve if
        // we can produce the zero-extended vector in one instruction.
        let is_zero_ext_load = matches!(
            inst.kind,
            SelectedInstructionKind::PackedLoadI16x4ZeroExtend
        );
        for i in 0..spec.in_count {
            let need_zext = is_zero_ext_load
                || matches!(inst.kind, SelectedInstructionKind::PackedWidenU8x4ToI16x8);
            let prov_for_only = if is_zero_ext_load {
                Some(inst.provenance.clone())
            } else {
                None
            };
            let (read_id, v) = self.resolve_packed_input(
                out,
                inst.operands[i].virtual_register,
                spec.in_widths[i],
                need_zext,
                prov_for_only,
            )?;
            if let Some(id) = read_id {
                remapped_reads.push(id);
            }
            inputs.push(v);
        }

        let (main_id_opt, v_out) = match &inst.kind {
            SelectedInstructionKind::PackedAddI16x8 => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedAddI16x8Reg,
                    self.packed_keys.add_reg,
                    &[inputs[0], inputs[1]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedSubI16x8 => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedSubI16x8Reg,
                    self.packed_keys.sub_reg,
                    &[inputs[0], inputs[1]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedShuffleI16x8 { lanes } => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedShuffleI16x8Reg { lanes: *lanes },
                    self.packed_keys.shuffle_reg,
                    &[inputs[0], inputs[1]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count } => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count: *count },
                    self.packed_keys.shift_right_reg,
                    &[inputs[0]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedWidenU8x8ToI16x8
            | SelectedInstructionKind::PackedWidenU8x4ToI16x8 => {
                let hz = matches!(inst.kind, SelectedInstructionKind::PackedWidenU8x4ToI16x8);
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg,
                    self.packed_keys.widen_u8x8_reg,
                    &[inputs[0]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, hz);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating
            | SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg,
                    self.packed_keys.narrow_u8x8_reg,
                    &[inputs[0]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedAddU8x16
            | SelectedInstructionKind::PackedMinI16x8
            | SelectedInstructionKind::PackedCmpGtI16x8
            | SelectedInstructionKind::PackedBitAndI16x8
            | SelectedInstructionKind::PackedBitAndNotI16x8 => {
                let (reg_kind, reg_key) = match inst.kind {
                    SelectedInstructionKind::PackedAddU8x16 => (
                        SelectedInstructionKind::PackedAddU8x16Reg,
                        self.packed_keys.add_u8x16_reg,
                    ),
                    SelectedInstructionKind::PackedMinI16x8 => (
                        SelectedInstructionKind::PackedMinI16x8Reg,
                        self.packed_keys.min_i16x8_reg,
                    ),
                    SelectedInstructionKind::PackedCmpGtI16x8 => (
                        SelectedInstructionKind::PackedCmpGtI16x8Reg,
                        self.packed_keys.cmpgt_i16x8_reg,
                    ),
                    SelectedInstructionKind::PackedBitAndI16x8 => (
                        SelectedInstructionKind::PackedBitAndI16x8Reg,
                        self.packed_keys.bit_and_i16x8_reg,
                    ),
                    SelectedInstructionKind::PackedBitAndNotI16x8 => (
                        SelectedInstructionKind::PackedBitAndNotI16x8Reg,
                        self.packed_keys.bit_andnot_i16x8_reg,
                    ),
                    _ => unreachable!(),
                };
                let (id, v) = self.emit_def_inst(
                    out,
                    reg_kind,
                    reg_key,
                    &[inputs[0], inputs[1]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedAbsI16x8 => {
                let (id, v) = self.emit_def_inst(
                    out,
                    SelectedInstructionKind::PackedAbsI16x8Reg,
                    self.packed_keys.abs_i16x8_reg,
                    &[inputs[0]],
                    true,
                    inst.provenance.clone(),
                )?;
                self.vec_high8_zero.insert(v, false);
                (Some(id), v)
            }
            SelectedInstructionKind::PackedLoadI16x4ZeroExtend => {
                // resolve_packed_input already got prov_for_only and returned zero-extended v.
                if remapped_reads.is_empty() && !is_pure_provenance(&inst.provenance) {
                    // Rare case: input resolved via reused register/cached const while provenance
                    // had non-empty work/obligations/edges; force an explicit instruction to carry it.
                    let (id, v) = if let Some(&v_in) = inputs.first() {
                        let ptr_origin = self.resolve_ptr(inst.operands[0].virtual_register);
                        let _ = v_in;
                        if let Some(PtrOrigin::Local { slot, offset }) = ptr_origin {
                            self.flush_pending_overlap(out, slot, offset, 8, false)?;
                            let addr = self.local_addr_vreg(out, slot)?;
                            self.emit_def_inst(
                                out,
                                SelectedInstructionKind::PackedLoad64ZeroExtend {
                                    byte_offset: offset,
                                },
                                self.packed_keys.load_64,
                                &[addr],
                                true,
                                inst.provenance.clone(),
                            )?
                        } else {
                            return Err(SelectedInstructionError::custody());
                        }
                    } else {
                        return Err(SelectedInstructionError::custody());
                    };
                    self.vec_high8_zero.insert(v, true);
                    remapped_reads.push(id);
                    (None, v)
                } else {
                    (None, inputs[0])
                }
            }
            _ => unreachable!(),
        };

        let dst_ptr = inst.operands[spec.in_count].virtual_register;
        let mut store_id_opt = None;
        match self.resolve_ptr(dst_ptr) {
            Some(PtrOrigin::Local { slot, offset }) => {
                self.record_local_vec_write(out, slot, offset, spec.out_bytes, v_out)?;
            }
            Some(PtrOrigin::External { base, offset }) => {
                let (kind, key) = match spec.out_bytes {
                    16 => (
                        SelectedInstructionKind::PackedStore128 {
                            byte_offset: offset,
                        },
                        self.packed_keys.store_128,
                    ),
                    8 => (
                        SelectedInstructionKind::PackedStore64 {
                            byte_offset: offset,
                        },
                        self.packed_keys.store_64,
                    ),
                    4 => (
                        SelectedInstructionKind::PackedStore32 {
                            byte_offset: offset,
                        },
                        self.packed_keys.store_32,
                    ),
                    _ => return Err(SelectedInstructionError::custody()),
                };
                let s_inst = self.make_inst(
                    kind,
                    key,
                    &[base, v_out],
                    if main_id_opt.is_none() && remapped_reads.is_empty() {
                        inst.provenance.clone()
                    } else {
                        SelectedInstructionProvenance::default()
                    },
                )?;
                store_id_opt = Some(s_inst.id);
                out.push(s_inst);
                self.invalidate_ext_loads();
            }
            None => return Err(SelectedInstructionError::custody()),
        }

        // Update memory_accesses mapping for `inst.id`.
        if remapped_reads.is_empty() && store_id_opt.is_none() {
            self.mem_delete.insert(inst.id);
        } else {
            let entry = self.mem_remap.entry(inst.id).or_default();
            for r in remapped_reads {
                entry.push((true, r));
            }
            if let Some(s) = store_id_opt {
                entry.push((false, s));
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn coalesce_external_stores(
    insts: &mut Vec<SelectedInstruction>,
    ext_store8_info: &mut HashMap<SelectedInstructionId, ExtStore8Info>,
    environment: &ValidatedTargetRegisterEnvironment,
    packed_keys: &PackedRegKeys,
    vec128_class: RegisterClassId,
    local_slots: &HashSet<LocalStorageSlotId>,
    multi_def_vregs: &HashSet<VirtualRegisterId>,
    next_inst_id: &mut u32,
    next_vreg_id: &mut u32,
    added_vregs: &mut Vec<VirtualRegister>,
    mem_remap: &mut HashMap<SelectedInstructionId, Vec<(bool, SelectedInstructionId)>>,
    mem_delete: &mut HashSet<SelectedInstructionId>,
) -> Result<(), SelectedInstructionError> {
    let block_local_ptrs = collect_block_local_ptrs(insts, local_slots, multi_def_vregs);
    let mut last_zero_store: Option<(usize, VirtualRegisterId)> = None;
    let mut removed = vec![false; insts.len()];

    let mut i = 0;
    while i < insts.len() {
        if removed[i] {
            i += 1;
            continue;
        }
        let Some(info1) = ext_store8_info.get(&insts[i].id).copied() else {
            i += 1;
            continue;
        };
        if !is_pure_provenance(&insts[i].provenance) {
            i += 1;
            continue;
        }

        // Scan forward over instructions that do not touch memory or redefine `info1.base`.
        let mut match_idx = None;
        let limit = (i + 1 + MAX_EXTERNAL_COALESCE_WINDOW).min(insts.len());
        for (j, mid) in insts.iter().enumerate().take(limit).skip(i + 1) {
            if removed[j] {
                continue;
            }
            if let Some(info2) = ext_store8_info.get(&mid.id).copied()
                && info2.base == info1.base
                && (info1.offset.saturating_add(8) == info2.offset
                    || info2.offset.saturating_add(8) == info1.offset)
            {
                let compatible = match (info1.source, info2.source) {
                    (ExtStore8Source::Zero, ExtStore8Source::Zero) => Some(None),
                    (ExtStore8Source::VecLow8(v1), ExtStore8Source::VecHigh8(v2))
                        if v1 == v2 && info1.offset.saturating_add(8) == info2.offset =>
                    {
                        Some(Some(v1))
                    }
                    (ExtStore8Source::VecHigh8(v1), ExtStore8Source::VecLow8(v2))
                        if v1 == v2 && info2.offset.saturating_add(8) == info1.offset =>
                    {
                        Some(Some(v1))
                    }
                    _ => None,
                };
                if let Some(vec_opt) = compatible
                    && is_pure_provenance(&mid.provenance)
                {
                    match_idx = Some((j, info1.offset.min(info2.offset), vec_opt));
                    break;
                }
            }
            if !is_memory_barrier_free_for_coalesce(mid, local_slots, &block_local_ptrs)
                || mid.operands.iter().any(|o| {
                    matches!(
                        o.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    ) && o.virtual_register == info1.base
                })
                || multi_def_vregs.contains(&info1.base)
            {
                break;
            }
        }

        if let Some((j, low_off, vec_opt)) = match_idx {
            let old1 = insts[i].id;
            removed[i] = true;
            let store_vreg = match vec_opt {
                Some(v) => v,
                None => {
                    // Keep the zero in a vector register only across a store run that
                    // the selector can cross. An intervening call can clobber every lane.
                    let reusable = last_zero_store.filter(|&(last, _)| {
                        last < i
                            && insts[last + 1..i].iter().all(|instruction| {
                                is_memory_barrier_free_for_coalesce(
                                    instruction,
                                    local_slots,
                                    &block_local_ptrs,
                                )
                            })
                    });
                    let zero = if let Some((_, zero)) = reusable {
                        zero
                    } else {
                        let z_id = SelectedInstructionId(*next_inst_id);
                        *next_inst_id += 1;
                        let z_reg = VirtualRegisterId(*next_vreg_id);
                        *next_vreg_id += 1;
                        added_vregs.push(VirtualRegister {
                            id: z_reg,
                            scalar_type: packed_vector_scalar_type(),
                            class: vec128_class,
                            origin: VirtualRegisterOrigin::InstructionScratch {
                                instruction: z_id,
                                operand: 0,
                            },
                            definition_site: None,
                            entry_fixed_view: None,
                        });
                        let z_row = constraint_row(environment, packed_keys.materialize_zero)?;
                        let z_inst = SelectedInstruction {
                            id: z_id,
                            kind: SelectedInstructionKind::PackedMaterializeZero,
                            constraint: packed_keys.materialize_zero,
                            operands: make_operands(z_row, &[z_reg]),
                            implicit_uses: z_row.implicit_uses.clone(),
                            implicit_defs: z_row.implicit_defs.clone(),
                            clobbers: z_row.clobbers.clone(),
                            provenance: SelectedInstructionProvenance::default(),
                        };
                        insts[i] = z_inst;
                        removed[i] = false;
                        z_reg
                    };
                    last_zero_store = Some((j, zero));
                    zero
                }
            };
            let coalesced_id = SelectedInstructionId(*next_inst_id);
            *next_inst_id += 1;
            let store_row = constraint_row(environment, packed_keys.store_128)?;

            let old2 = insts[j].id;
            let prov = insts[j].provenance.clone();
            insts[j] = SelectedInstruction {
                id: coalesced_id,
                kind: SelectedInstructionKind::PackedStore128 {
                    byte_offset: low_off,
                },
                constraint: packed_keys.store_128,
                operands: make_operands(store_row, &[info1.base, store_vreg]),
                implicit_uses: store_row.implicit_uses.clone(),
                implicit_defs: store_row.implicit_defs.clone(),
                clobbers: store_row.clobbers.clone(),
                provenance: prov,
            };
            mem_delete.insert(old1);
            mem_remap
                .entry(old2)
                .or_default()
                .push((false, coalesced_id));
        }
        i += 1;
    }

    if removed.iter().any(|&r| r) {
        let mut idx = 0usize;
        insts.retain(|_| {
            let keep = !removed[idx];
            idx += 1;
            keep
        });
    }
    Ok(())
}

/// Whether any of `later` may read `slot`'s memory. A load whose flush is
/// elided leaves its destination register without the slot's bytes, so a copy
/// of it stored to another local slot leaves that slot's memory stale too; the
/// elision is sound only while nothing reads that slot's memory afterwards.
/// Writing through a pointer and deriving a pointer are not reads; every other
/// use of a pointer into the slot counts as one.
fn slot_read_later(
    later: &[SelectedInstruction],
    block_local_ptrs: &HashMap<VirtualRegisterId, PtrOrigin>,
    slot: LocalStorageSlotId,
) -> bool {
    later.iter().any(|inst| {
        inst.operands.iter().enumerate().any(|(pos, op)| {
            if matches!(op.access, RegisterOperandAccess::Def) {
                return false;
            }
            let Some(&PtrOrigin::Local { slot: s, .. }) =
                block_local_ptrs.get(&op.virtual_register)
            else {
                return false;
            };
            if s != slot {
                return false;
            }
            let writes_through =
                pos == 0 && pointer_access(&inst.kind).is_some_and(|(writes, ..)| writes);
            let derives = pos == 0
                && matches!(
                    inst.kind,
                    SelectedInstructionKind::CopyI64
                        | SelectedInstructionKind::AddressOffset { .. }
                );
            !(writes_through || derives)
        })
    })
}

fn collect_block_local_ptrs(
    insts: &[SelectedInstruction],
    local_slots: &HashSet<LocalStorageSlotId>,
    multi_def_vregs: &HashSet<VirtualRegisterId>,
) -> HashMap<VirtualRegisterId, PtrOrigin> {
    let mut ptrs = HashMap::new();
    for inst in insts {
        match &inst.kind {
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset,
            } if local_slots.contains(slot) => {
                if let Some(dst) = inst.operands.first().map(|o| o.virtual_register)
                    && !multi_def_vregs.contains(&dst)
                {
                    ptrs.insert(
                        dst,
                        PtrOrigin::Local {
                            slot: *slot,
                            offset: *byte_offset,
                        },
                    );
                }
            }
            SelectedInstructionKind::CopyI64 if inst.operands.len() == 2 => {
                let dst = inst.operands[1].virtual_register;
                if !multi_def_vregs.contains(&dst)
                    && let Some(&orig) = ptrs.get(&inst.operands[0].virtual_register)
                {
                    ptrs.insert(dst, orig);
                }
            }
            SelectedInstructionKind::AddressOffset { byte_offset } if inst.operands.len() == 2 => {
                let dst = inst.operands[1].virtual_register;
                if !multi_def_vregs.contains(&dst)
                    && let Some(PtrOrigin::Local { slot, offset }) =
                        ptrs.get(&inst.operands[0].virtual_register).copied()
                    && let Some(sum) = offset.checked_add(*byte_offset)
                {
                    ptrs.insert(dst, PtrOrigin::Local { slot, offset: sum });
                }
            }
            _ => {}
        }
    }
    ptrs
}

fn is_memory_barrier_free_for_coalesce(
    inst: &SelectedInstruction,
    local_slots: &HashSet<LocalStorageSlotId>,
    block_local_ptrs: &HashMap<VirtualRegisterId, PtrOrigin>,
) -> bool {
    match &inst.kind {
        _ if register_op_purity(&inst.kind).is_some() => true,
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(s),
            ..
        } => local_slots.contains(s),
        _ if pointer_access(&inst.kind).is_some() => {
            !inst.operands.is_empty()
                && block_local_ptrs.contains_key(&inst.operands[0].virtual_register)
        }
        _ => false,
    }
}

/// Register-only instructions that dead-code elimination may drop and store
/// coalescing may move across. `Some(true)` marks the scalar address and
/// arithmetic forms, which elimination drops when their provenance carries no
/// work or edges; `Some(false)` marks packed register forms, whose provenance
/// must also carry no obligations.
fn register_op_purity(kind: &SelectedInstructionKind) -> Option<bool> {
    match kind {
        SelectedInstructionKind::MaterializeI64 { .. }
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingMultiplyI64
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactAddI64Immediate { .. } => Some(true),
        SelectedInstructionKind::PackedMaterializeZero
        | SelectedInstructionKind::PackedFromGpr64ZeroExtend
        | SelectedInstructionKind::PackedFromGpr64Pair
        | SelectedInstructionKind::PackedAddI16x8Reg
        | SelectedInstructionKind::PackedSubI16x8Reg
        | SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { .. }
        | SelectedInstructionKind::PackedShuffleI16x8Reg { .. }
        | SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg
        | SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg
        | SelectedInstructionKind::PackedAddU8x16Reg
        | SelectedInstructionKind::PackedMinI16x8Reg
        | SelectedInstructionKind::PackedCmpGtI16x8Reg
        | SelectedInstructionKind::PackedBitAndI16x8Reg
        | SelectedInstructionKind::PackedBitAndNotI16x8Reg
        | SelectedInstructionKind::PackedAbsI16x8Reg => Some(false),
        _ => None,
    }
}

/// The byte window a load or store reaches through its pointer operand
/// (operand 0): whether it writes, its offset from the pointer and its width.
fn pointer_access(kind: &SelectedInstructionKind) -> Option<(bool, u32, usize)> {
    Some(match *kind {
        SelectedInstructionKind::Load8 { byte_offset } => (false, byte_offset, 1),
        SelectedInstructionKind::Load16 { byte_offset } => (false, byte_offset, 2),
        SelectedInstructionKind::Load32 { byte_offset } => (false, byte_offset, 4),
        SelectedInstructionKind::Load64 { byte_offset } => (false, byte_offset, 8),
        SelectedInstructionKind::PackedLoad128 { byte_offset } => (false, byte_offset, 16),
        SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset } => (false, byte_offset, 8),
        SelectedInstructionKind::PackedLoad32ZeroExtend { byte_offset } => (false, byte_offset, 4),
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => (true, byte_offset, byte_size as usize),
        SelectedInstructionKind::PackedStore128 { byte_offset } => (true, byte_offset, 16),
        SelectedInstructionKind::PackedStore64 { byte_offset } => (true, byte_offset, 8),
        SelectedInstructionKind::PackedStore32 { byte_offset } => (true, byte_offset, 4),
        _ => return None,
    })
}

fn eliminate_dead_local_stores(
    insts: &mut Vec<SelectedInstruction>,
    slot_sizes: &HashMap<LocalStorageSlotId, usize>,
    local_slots: &HashSet<LocalStorageSlotId>,
    multi_def_vregs: &HashSet<VirtualRegisterId>,
) -> bool {
    if local_slots.is_empty() {
        return false;
    }
    let ptr_origin = collect_block_local_ptrs(insts, local_slots, multi_def_vregs);

    let mut live_bytes: HashMap<LocalStorageSlotId, Vec<bool>> = local_slots
        .iter()
        .map(|&s| (s, vec![false; slot_sizes[&s]]))
        .collect();
    let mut removed = vec![false; insts.len()];

    for (idx, inst) in insts.iter().enumerate().rev() {
        let (writes, off, width) = match inst.kind {
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset,
            } if local_slots.contains(&slot) => (true, (slot, byte_offset as usize), 8),
            _ => {
                let Some((writes, byte_offset, width)) = pointer_access(&inst.kind) else {
                    continue;
                };
                let Some(&PtrOrigin::Local { slot, offset }) =
                    ptr_origin.get(&inst.operands[0].virtual_register)
                else {
                    continue;
                };
                (writes, (slot, (offset + byte_offset) as usize), width)
            }
        };
        let (slot, off) = off;
        let live = &mut live_bytes.get_mut(&slot).unwrap()[off..(off + width)];
        if !writes {
            live.fill(true);
        } else if !live.iter().any(|&b| b) && is_pure_provenance(&inst.provenance) {
            removed[idx] = true;
        } else {
            live.fill(false);
        }
    }

    let any_removed = removed.iter().any(|&r| r);
    if any_removed {
        let mut idx = 0usize;
        insts.retain(|_| {
            let keep = !removed[idx];
            idx += 1;
            keep
        });
    }
    any_removed
}

fn dce_and_copy_prop(
    function: &mut SelectedFunction,
    slot_sizes: &HashMap<LocalStorageSlotId, usize>,
    non_escaping_by_block: &HashMap<usize, HashSet<LocalStorageSlotId>>,
    multi_def_vregs: &HashSet<VirtualRegisterId>,
    folded_ext_loads: &HashSet<SelectedInstructionId>,
) {
    let reg_class_map: HashMap<VirtualRegisterId, RegisterClassId> = function
        .virtual_registers
        .iter()
        .map(|r| (r.id, r.class))
        .collect();
    let mut fixed_entry_vregs: HashSet<VirtualRegisterId> = function
        .virtual_registers
        .iter()
        .filter(|r| r.entry_fixed_view.is_some())
        .map(|r| r.id)
        .collect();
    for inst in function.blocks.iter().flat_map(block_instructions) {
        for op in &inst.operands {
            if op.fixed_view.is_some() || op.tied_to.is_some() {
                fixed_entry_vregs.insert(op.virtual_register);
            }
        }
    }

    loop {
        let mut changed = false;

        // Collect block occurrence sets so we only propagate CopyI64 when dst is local to 1 block.
        let mut vreg_blocks: HashMap<VirtualRegisterId, HashSet<usize>> = HashMap::new();
        for (b_idx, block) in function.blocks.iter().enumerate() {
            for inst in block_instructions(block) {
                for op in &inst.operands {
                    vreg_blocks
                        .entry(op.virtual_register)
                        .or_default()
                        .insert(b_idx);
                }
            }
            for succ in terminator_successors(&block.terminator) {
                successor_registers(succ, |role, register| {
                    if matches!(
                        role,
                        EdgeRegister::Argument | EdgeRegister::Parameter | EdgeRegister::Structural
                    ) {
                        vreg_blocks.entry(register).or_default().insert(b_idx);
                    }
                });
            }
        }

        // Propagate CopyI64 within each single block.
        for (b_idx, block) in function.blocks.iter_mut().enumerate() {
            let mut copy_map: HashMap<VirtualRegisterId, VirtualRegisterId> = HashMap::new();
            for inst in &block.instructions {
                if matches!(inst.kind, SelectedInstructionKind::CopyI64)
                    && inst.operands.len() == 2
                    && inst.operands[0].fixed_view.is_none()
                    && inst.operands[1].fixed_view.is_none()
                    && inst.operands[1].tied_to.is_none()
                    && is_pure_provenance(&inst.provenance)
                {
                    let src = inst.operands[0].virtual_register;
                    let dst = inst.operands[1].virtual_register;
                    if src != dst
                        && !multi_def_vregs.contains(&src)
                        && !multi_def_vregs.contains(&dst)
                        && !fixed_entry_vregs.contains(&src)
                        && !fixed_entry_vregs.contains(&dst)
                        && reg_class_map.get(&src) == reg_class_map.get(&dst)
                        && vreg_blocks
                            .get(&dst)
                            .is_some_and(|s| s.len() == 1 && s.contains(&b_idx))
                    {
                        let root = copy_map.get(&src).copied().unwrap_or(src);
                        copy_map.insert(dst, root);
                    }
                }
            }
            if !copy_map.is_empty() {
                for inst in &mut block.instructions {
                    for op in &mut inst.operands {
                        if matches!(op.access, RegisterOperandAccess::Use)
                            && op.fixed_view.is_none()
                            && op.tied_to.is_none()
                            && let Some(&rep) = copy_map.get(&op.virtual_register)
                        {
                            op.virtual_register = rep;
                            changed = true;
                        }
                    }
                }
            }
        }

        // Compute global Use set across all instructions, terminators, and successor transports.
        let used = collect_used_vregs(function);

        for (b_idx, block) in function.blocks.iter_mut().enumerate() {
            let local_slots = non_escaping_by_block.get(&b_idx);
            let mut local_addrs: HashSet<VirtualRegisterId> = HashSet::new();
            for inst in &block.instructions {
                if let SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                } = inst.kind
                    && local_slots.is_some_and(|ls| ls.contains(&s))
                    && let Some(op) = inst.operands.first()
                {
                    local_addrs.insert(op.virtual_register);
                } else if matches!(
                    inst.kind,
                    SelectedInstructionKind::CopyI64
                        | SelectedInstructionKind::AddressOffset { .. }
                ) && inst.operands.len() == 2
                    && local_addrs.contains(&inst.operands[0].virtual_register)
                {
                    local_addrs.insert(inst.operands[1].virtual_register);
                }
            }

            let orig_len = block.instructions.len();
            block.instructions.retain(|inst| {
                let removable = match register_op_purity(&inst.kind) {
                    Some(true) => is_pure_scalar_provenance(&inst.provenance),
                    Some(false) => is_pure_provenance(&inst.provenance),
                    None => {
                        pointer_access(&inst.kind).is_some_and(|(writes, ..)| !writes)
                            && is_pure_provenance(&inst.provenance)
                            && ((!inst.operands.is_empty()
                                && local_addrs.contains(&inst.operands[0].virtual_register))
                                || folded_ext_loads.contains(&inst.id))
                    }
                };
                if !removable {
                    return true;
                }
                inst.operands.iter().any(|op| {
                    matches!(
                        op.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    ) && used.contains(&op.virtual_register)
                })
            });
            if block.instructions.len() != orig_len {
                changed = true;
            }
            if let Some(local_slots) = non_escaping_by_block.get(&b_idx)
                && eliminate_dead_local_stores(
                    &mut block.instructions,
                    slot_sizes,
                    local_slots,
                    multi_def_vregs,
                )
            {
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }
}

fn collect_used_vregs(function: &SelectedFunction) -> HashSet<VirtualRegisterId> {
    let mut used = HashSet::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            for op in &inst.operands {
                if matches!(
                    op.access,
                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                ) {
                    used.insert(op.virtual_register);
                }
            }
        }
        let term = terminator_instruction(&block.terminator);
        for op in &term.operands {
            used.insert(op.virtual_register);
        }
        for succ in terminator_successors(&block.terminator) {
            successor_registers(succ, |role, register| {
                if !matches!(role, EdgeRegister::Parameter | EdgeRegister::CaseParameter) {
                    used.insert(register);
                }
            });
        }
    }
    used
}

fn reconcile_memory_accesses(
    function: &mut SelectedFunction,
    mem_remap: &HashMap<SelectedInstructionId, Vec<(bool, SelectedInstructionId)>>,
    mem_delete: &HashSet<SelectedInstructionId>,
) {
    let mut expanded = Vec::with_capacity(function.memory_accesses.len());
    for access in function.memory_accesses.drain(..) {
        if mem_delete.contains(&access.instruction) {
            continue;
        }
        if let Some(targets) = mem_remap.get(&access.instruction) {
            let is_write = matches!(
                access.role,
                SelectedMemoryAccessRole::WritePlace
                    | SelectedMemoryAccessRole::WriteBacking
                    | SelectedMemoryAccessRole::WriteBackingSpan
                    | SelectedMemoryAccessRole::WriteLocal { .. }
                    | SelectedMemoryAccessRole::WriteOutgoing { .. }
                    | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
                    | SelectedMemoryAccessRole::WriteIndexedStructural { .. }
                    | SelectedMemoryAccessRole::WriteByteSpan { .. }
                    | SelectedMemoryAccessRole::WriteByteSequence { .. }
                    | SelectedMemoryAccessRole::WriteElementView { .. }
            );
            let mut assigned = false;
            for &(target_is_read, new_id) in targets {
                if target_is_read != is_write {
                    let mut cloned = access.clone();
                    cloned.instruction = new_id;
                    expanded.push(cloned);
                    assigned = true;
                    break;
                }
            }
            let _ = assigned;
        } else {
            expanded.push(access);
        }
    }

    let mut surviving_insts = HashSet::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            surviving_insts.insert(inst.id);
        }
        surviving_insts.insert(terminator_instruction(&block.terminator).id);
    }
    expanded.retain(|access| surviving_insts.contains(&access.instruction));
    function.memory_accesses = expanded;
}

pub(crate) fn prune_dead_metadata(function: &mut SelectedFunction) {
    let mut live_insts = HashSet::new();
    let mut live_vregs = HashSet::new();
    let mut live_slots = HashSet::new();

    for block in &function.blocks {
        for inst in block_instructions(block) {
            live_insts.insert(inst.id);
            for op in &inst.operands {
                live_vregs.insert(op.virtual_register);
            }
            match &inst.kind {
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                }
                | SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(s),
                    ..
                }
                | SelectedInstructionKind::HostedReadByte { slot: s }
                | SelectedInstructionKind::HostedWriteByteI32 { slot: s }
                | SelectedInstructionKind::SaveFloatingControl { slot: s }
                | SelectedInstructionKind::RestoreFloatingControl { slot: s } => {
                    live_slots.insert(*s);
                }
                _ => {}
            }
        }
        for succ in terminator_successors(&block.terminator) {
            successor_registers(succ, |_, register| {
                live_vregs.insert(register);
            });
            for sb in &succ.structural_bindings {
                match sb.transport {
                    selected_instructions::SelectedStructuralTransport::Address {
                        base,
                        destination,
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        base,
                        destination,
                        ..
                    } => {
                        live_slots.insert(destination);
                        if let selected_instructions::SelectedAddressBase::Local(s) = base {
                            live_slots.insert(s);
                        }
                    }
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        destination,
                        ..
                    } => {
                        live_slots.insert(destination);
                    }
                    selected_instructions::SelectedStructuralTransport::Unused => {}
                }
            }
            if let Some(case) = &succ.structural_case
                && let SelectedCaseDispatchSource::Local { slot } = case.source
            {
                live_slots.insert(slot);
            }
        }
    }

    for access in &function.memory_accesses {
        if let SelectedMemoryAccessRole::ReadLocal { slot }
        | SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } = access.role
        {
            live_slots.insert(slot);
        }
    }

    function.local_storage_slots.retain(|slot| {
        !matches!(
            slot.id,
            LocalStorageSlotId::Structural { .. } | LocalStorageSlotId::Inlined { .. }
        ) || live_slots.contains(&slot.id)
    });

    // Also retain any register referenced by a surviving SpillAddress origin.
    let mut extra_vregs = Vec::new();
    for reg in &function.virtual_registers {
        if let VirtualRegisterOrigin::SpillAddress {
            instruction,
            register,
        } = reg.origin
            && (live_vregs.contains(&reg.id) || live_insts.contains(&instruction))
        {
            extra_vregs.push(register);
        }
    }
    live_vregs.extend(extra_vregs);

    function.virtual_registers.retain(|reg| match reg.origin {
        VirtualRegisterOrigin::EntryParameter { .. }
        | VirtualRegisterOrigin::StructuralParameter { .. }
        | VirtualRegisterOrigin::BlockParameter { .. } => true,
        VirtualRegisterOrigin::InstructionScratch { instruction, .. }
        | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
        | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
        | VirtualRegisterOrigin::SpillAddress { instruction, .. }
        | VirtualRegisterOrigin::AbiTransport { instruction, .. }
        | VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
            live_vregs.contains(&reg.id) && live_insts.contains(&instruction)
        }
    });

    // Renumber virtual registers densely to 0..len so `ids_are_positions()` holds.
    let mut vreg_map: HashMap<VirtualRegisterId, VirtualRegisterId> =
        HashMap::with_capacity(function.virtual_registers.len());
    for (idx, reg) in function.virtual_registers.iter_mut().enumerate() {
        let new_id = VirtualRegisterId(idx as u32);
        vreg_map.insert(reg.id, new_id);
        reg.id = new_id;
    }
    for reg in &mut function.virtual_registers {
        if let VirtualRegisterOrigin::SpillAddress { register, .. } = &mut reg.origin {
            *register = vreg_map[register];
        }
    }
    for block in &mut function.blocks {
        for inst in &mut block.instructions {
            for op in &mut inst.operands {
                op.virtual_register = vreg_map[&op.virtual_register];
            }
        }
        let term = terminator_instruction_mut(&mut block.terminator);
        for op in &mut term.operands {
            op.virtual_register = vreg_map[&op.virtual_register];
        }
        for succ in terminator_successors_mut(&mut block.terminator) {
            for b in &mut succ.bindings {
                if let selected_instructions::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut b.transport
                {
                    *argument = vreg_map[argument];
                    *parameter = vreg_map[parameter];
                }
            }
            for sb in &mut succ.structural_bindings {
                match &mut sb.transport {
                    selected_instructions::SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(r),
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Register(r),
                        ..
                    }
                    | selected_instructions::SelectedStructuralTransport::WholeValue {
                        argument: r,
                        ..
                    } => {
                        *r = vreg_map[r];
                    }
                    _ => {}
                }
            }
            if let Some(case) = &mut succ.structural_case {
                if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = &mut case.source {
                    *pointer = vreg_map[pointer];
                }
                for payload in &mut case.payloads {
                    match &mut payload.transport {
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            *parameter = vreg_map[parameter];
                        }
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            *argument = vreg_map[argument];
                            *parameter = vreg_map[parameter];
                        }
                        SelectedCasePayloadTransport::Unused => {}
                    }
                }
            }
        }
    }
}

/// A block's instructions followed by its terminator instruction.
fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// Where on a successor edge a register appears. A value binding reads its
/// `Argument` in this block and defines its `Parameter` in the successor.
/// `Structural` covers transport bases and whole-value arguments. `Case` covers
/// the borrowed dispatch pointer, payload arguments and unmaterialized payload
/// parameters; `CaseParameter` is a materialized payload's parameter.
#[derive(Clone, Copy)]
enum EdgeRegister {
    Argument,
    Parameter,
    Structural,
    Case,
    CaseParameter,
}

/// Visit every virtual register a successor edge names. Each pass filters
/// the roles it needs instead of walking the edge itself.
fn successor_registers(
    succ: &selected_instructions::SelectedSuccessor,
    mut visit: impl FnMut(EdgeRegister, VirtualRegisterId),
) {
    for b in &succ.bindings {
        if let selected_instructions::SelectedValueTransport::Registers {
            argument,
            parameter,
        } = b.transport
        {
            visit(EdgeRegister::Argument, argument);
            visit(EdgeRegister::Parameter, parameter);
        }
    }
    for sb in &succ.structural_bindings {
        match sb.transport {
            selected_instructions::SelectedStructuralTransport::Address {
                base: selected_instructions::SelectedAddressBase::Register(r),
                ..
            }
            | selected_instructions::SelectedStructuralTransport::ScalarReference {
                base: selected_instructions::SelectedAddressBase::Register(r),
                ..
            }
            | selected_instructions::SelectedStructuralTransport::WholeValue {
                argument: r, ..
            } => visit(EdgeRegister::Structural, r),
            _ => {}
        }
    }
    if let Some(case) = &succ.structural_case {
        if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source {
            visit(EdgeRegister::Case, pointer);
        }
        for payload in &case.payloads {
            match payload.transport {
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    visit(EdgeRegister::Case, argument);
                    visit(EdgeRegister::CaseParameter, parameter);
                }
                SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                    visit(EdgeRegister::Case, parameter)
                }
                SelectedCasePayloadTransport::Unused => {}
            }
        }
    }
}

fn make_operands(
    row: &RegisterInstructionConstraint,
    vregs: &[VirtualRegisterId],
) -> Vec<SelectedOperand> {
    assert_eq!(row.operands.len(), vregs.len());
    row.operands
        .iter()
        .zip(vregs.iter().copied())
        .map(|(c, vreg)| SelectedOperand {
            operand: c.operand,
            virtual_register: vreg,
            access: c.access,
            class: c.class,
            fixed_view: c.fixed_view,
            tied_to: c.tied_to,
            early_clobber: c.early_clobber,
        })
        .collect()
}

fn is_pure_provenance(p: &SelectedInstructionProvenance) -> bool {
    p.work.is_empty() && p.obligations.is_empty() && p.edges.is_empty()
}

fn is_pure_scalar_provenance(p: &SelectedInstructionProvenance) -> bool {
    p.work.is_empty() && p.edges.is_empty()
}

fn integer_value_to_u64(v: IntegerValue) -> Option<u64> {
    match v {
        IntegerValue::Signed(x) => i64::try_from(x).ok().map(|n| n as u64),
        IntegerValue::Unsigned(x) => u64::try_from(x).ok(),
    }
}

fn const_bytes_u64(v: u64) -> [SymByte; 8] {
    v.to_le_bytes().map(SymByte::Const)
}

fn extract_const_pair(slice: &[SymByte]) -> Option<(u64, u64)> {
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    for (idx, b) in slice.iter().enumerate() {
        let SymByte::Const(val) = *b else {
            return None;
        };
        if idx < 8 {
            lo[idx] = val;
        } else {
            hi[idx - 8] = val;
        }
    }
    Some((u64::from_le_bytes(lo), u64::from_le_bytes(hi)))
}

fn match_vec_lanes(slice: &[SymByte], start_byte: u8) -> Option<VirtualRegisterId> {
    let SymByte::VecLaneByte { vreg, byte_index } = slice.first().copied()? else {
        return None;
    };
    if byte_index != start_byte {
        return None;
    }
    for (offset, &b) in slice.iter().enumerate() {
        let SymByte::VecLaneByte {
            vreg: cand,
            byte_index: idx,
        } = b
        else {
            return None;
        };
        if cand != vreg || idx != start_byte + offset as u8 {
            return None;
        }
    }
    Some(vreg)
}

fn match_gpr_bytes(slice: &[SymByte]) -> Option<VirtualRegisterId> {
    let SymByte::GprByte {
        vreg,
        byte_index: 0,
    } = slice.first().copied()?
    else {
        return None;
    };
    for (offset, &b) in slice.iter().enumerate() {
        let SymByte::GprByte {
            vreg: cand,
            byte_index,
        } = b
        else {
            return None;
        };
        if cand != vreg || byte_index as usize != offset {
            return None;
        }
    }
    Some(vreg)
}

#[cfg(test)]
mod tests {
    use super::promote_and_simplify;
    use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId,
        SelectedBlockOrigin, SelectedFunction, SelectedFunctionContracts, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
        SelectedLocalStorageSlot, SelectedOperand, SelectedTerminator, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType, ValueId,
    };
    use std::sync::Arc;
    use target::NativeTarget;
    use target_vocabulary::TerminalPsiProvenance;

    fn i64_ty() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
    }

    fn op_from(
        row: &RegisterInstructionConstraint,
        idx: usize,
        vreg: VirtualRegisterId,
    ) -> SelectedOperand {
        let c = &row.operands[idx];
        SelectedOperand {
            operand: c.operand,
            virtual_register: vreg,
            access: c.access,
            class: c.class,
            fixed_view: c.fixed_view,
            tied_to: c.tied_to,
            early_clobber: c.early_clobber,
        }
    }

    fn base_function(
        slots: Vec<SelectedLocalStorageSlot>,
        vregs: Vec<VirtualRegister>,
        insts: Vec<SelectedInstruction>,
        term_id: SelectedInstructionId,
        ret_key: register_model::RegisterConstraintKey,
    ) -> SelectedFunction {
        SelectedFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                structural_block_parameters: Vec::new(),
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![],
                    edges: vec![],
                },
                structural: None,
                local_storage_slots: slots,
                outgoing_arguments: vec![],
                calls: vec![],
                normalized_foreign_calls: vec![],
                memory_accesses: vec![],
                boundary_settlements: vec![],
                virtual_registers: vregs,
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: insts,
                terminator: SelectedTerminator::Return {
                    instruction: SelectedInstruction {
                        id: term_id,
                        kind: SelectedInstructionKind::ReturnUnit,
                        constraint: ret_key,
                        operands: vec![],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }]
            .into(),
        }
    }

    #[test]
    fn promotes_chained_packed_ops_across_local_slots_eliminating_stack_temporaries() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let add_key = keys.packed_add_i16x8.unwrap();
        let sra_key = keys.packed_shift_right_i16x8.unwrap();
        let narrow_key = keys.packed_narrow_i16x8_to_u8x4_saturating.unwrap();
        let load64_key = keys.load64.unwrap();
        let load32_key = keys.load32.unwrap();
        let store64_key = keys.store64.unwrap();
        let store_key = keys.store.unwrap();
        let ret_key = keys.return_unit;

        let fa_row = env.constraint(fa_key).unwrap();
        let add_row = env.constraint(add_key).unwrap();
        let sra_row = env.constraint(sra_key).unwrap();
        let narrow_row = env.constraint(narrow_key).unwrap();
        let load64_row = env.constraint(load64_key).unwrap();
        let load32_row = env.constraint(load32_key).unwrap();
        let store64_row = env.constraint(store64_key).unwrap();
        let store_row = env.constraint(store_key).unwrap();

        let ext_in = VirtualRegisterId(0);
        let ext_out = VirtualRegisterId(1);
        let p_const = VirtualRegisterId(2);
        let g_const = VirtualRegisterId(3);
        let p_in = VirtualRegisterId(4);
        let p_add = VirtualRegisterId(5);
        let p_sra = VirtualRegisterId(6);
        let p_narrow = VirtualRegisterId(7);
        let g_px = VirtualRegisterId(8);
        let g_zero = VirtualRegisterId(9);

        let slot_const = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 0,
        };
        let slot_in = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 1,
        };
        let slot_add = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 2,
        };
        let slot_sra = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 3,
        };
        let slot_narrow = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 4,
        };

        let gpr = fa_row.operands[0].class;
        let mut vregs = vec![
            VirtualRegister {
                id: ext_in,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: ext_out,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(2).unwrap(),
                    parameter_index: 1,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ];
        let scratch_insts = [
            (g_const, SelectedInstructionId(0)),
            (g_zero, SelectedInstructionId(1)),
            (p_const, SelectedInstructionId(2)),
            (p_in, SelectedInstructionId(5)),
            (p_add, SelectedInstructionId(7)),
            (p_sra, SelectedInstructionId(9)),
            (p_narrow, SelectedInstructionId(11)),
            (g_px, SelectedInstructionId(13)),
        ];
        for (vid, iid) in scratch_insts {
            vregs.push(VirtualRegister {
                id: vid,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: iid,
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        }

        let insts = vec![
            // Build [32, 32, 32, 32, 0, 0, 0, 0] in slot_const.
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0x0020_0020_0020_0020),
                },
                constraint: keys.materialize_i64,
                operands: vec![SelectedOperand {
                    operand: 0,
                    virtual_register: g_const,
                    access: RegisterOperandAccess::Def,
                    class: gpr,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                },
                constraint: keys.materialize_i64,
                operands: vec![SelectedOperand {
                    operand: 0,
                    virtual_register: g_zero,
                    access: RegisterOperandAccess::Def,
                    class: gpr,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(2),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_const),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_const)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(3),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot_const),
                    byte_offset: 0,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, g_const)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(4),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot_const),
                    byte_offset: 8,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, g_zero)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // Load [i16; 4] zero-extended from ext_in into slot_in.
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_in),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_in)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(6),
                kind: SelectedInstructionKind::PackedLoadI16x4ZeroExtend,
                constraint: keys.packed_load_i16x4_zero_extend.unwrap(),
                operands: vec![
                    op_from(load64_row, 0, ext_in),
                    SelectedOperand {
                        operand: 1,
                        virtual_register: p_in,
                        access: RegisterOperandAccess::Use,
                        class: gpr,
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    },
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // Add slot_in + slot_const -> slot_add.
            SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_add),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_add)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(8),
                kind: SelectedInstructionKind::PackedAddI16x8,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, p_in),
                    op_from(add_row, 1, p_const),
                    op_from(add_row, 2, p_add),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // Shift right arithmetic -> slot_sra.
            SelectedInstruction {
                id: SelectedInstructionId(9),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_sra),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_sra)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(10),
                kind: SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count: 6 },
                constraint: sra_key,
                operands: vec![op_from(sra_row, 0, p_add), op_from(sra_row, 1, p_sra)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // Narrow to u8x4 -> slot_narrow, then store 4 bytes to ext_out.
            SelectedInstruction {
                id: SelectedInstructionId(11),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_narrow),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_narrow)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(12),
                kind: SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating,
                constraint: narrow_key,
                operands: vec![
                    op_from(narrow_row, 0, p_sra),
                    op_from(narrow_row, 1, p_narrow),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(13),
                kind: SelectedInstructionKind::Load32 { byte_offset: 0 },
                constraint: load32_key,
                operands: vec![
                    op_from(load32_row, 0, p_narrow),
                    op_from(load32_row, 1, g_px),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(14),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 4,
                    byte_size: 4,
                },
                constraint: store_key,
                operands: vec![op_from(store_row, 0, ext_out), op_from(store_row, 1, g_px)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];

        let slots = vec![slot_const, slot_in, slot_add, slot_sra, slot_narrow]
            .into_iter()
            .map(|id| SelectedLocalStorageSlot {
                id,
                byte_size: 16,
                alignment: 16,
            })
            .collect();
        let mut func = base_function(slots, vregs, insts, SelectedInstructionId(15), ret_key);
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        assert!(
            func.local_storage_slots.is_empty(),
            "all temporary slots should be eliminated, remaining: {:?}",
            func.local_storage_slots
        );
        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        assert!(
            kinds.contains(&SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset: 0 })
        );
        assert!(kinds.contains(&SelectedInstructionKind::PackedFromGpr64ZeroExtend));
        assert!(kinds.contains(&SelectedInstructionKind::PackedAddI16x8Reg));
        assert!(
            kinds.contains(
                &SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count: 6 }
            )
        );
        assert!(kinds.contains(&SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg));
        assert!(kinds.contains(&SelectedInstructionKind::PackedStore32 { byte_offset: 4 }));
        assert!(!kinds.iter().any(|k| matches!(
            k,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::Store { .. }
        )));
        // Each vector register the pass creates states its whole 128-bit width;
        // a Stage 07 spill reads that width to choose a 16-byte slot.
        let vector_class = env
            .constraint(keys.packed_add_i16x8_reg.unwrap())
            .unwrap()
            .operands[2]
            .class;
        let promoted: Vec<_> = func
            .virtual_registers
            .iter()
            .filter(|register| register.class == vector_class)
            .collect();
        assert!(!promoted.is_empty());
        assert!(promoted.iter().all(|register| {
            register.scalar_type
                == ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 128).unwrap())
        }));
    }

    #[test]
    fn preserves_local_slot_when_address_escapes_to_return_or_call() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let sra_key = keys.packed_shift_right_i16x8.unwrap();
        let store64_key = keys.store64.unwrap();
        let ret_key = keys.return_i64;

        let fa_row = env.constraint(fa_key).unwrap();
        let sra_row = env.constraint(sra_key).unwrap();
        let store64_row = env.constraint(store64_key).unwrap();
        let ret_row = env.constraint(ret_key).unwrap();

        let slot = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 0,
        };
        let gpr = fa_row.operands[0].class;
        let g0 = VirtualRegisterId(0);
        let p0 = VirtualRegisterId(1);
        let p1 = VirtualRegisterId(2);

        let vregs = vec![
            VirtualRegister {
                id: g0,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(0),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: p0,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(1),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: p1,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(4),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ];

        let mut func = SelectedFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                structural_block_parameters: Vec::new(),
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![],
                    edges: vec![],
                },
                structural: None,
                local_storage_slots: vec![SelectedLocalStorageSlot {
                    id: slot,
                    byte_size: 16,
                    alignment: 16,
                }],
                outgoing_arguments: vec![],
                calls: vec![],
                normalized_foreign_calls: vec![],
                memory_accesses: vec![],
                boundary_settlements: vec![],
                virtual_registers: vregs,
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    SelectedInstruction {
                        id: SelectedInstructionId(0),
                        kind: SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(0),
                        },
                        constraint: keys.materialize_i64,
                        operands: vec![SelectedOperand {
                            operand: 0,
                            virtual_register: g0,
                            access: RegisterOperandAccess::Def,
                            class: gpr,
                            fixed_view: None,
                            tied_to: None,
                            early_clobber: false,
                        }],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    SelectedInstruction {
                        id: SelectedInstructionId(1),
                        kind: SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Local(slot),
                            byte_offset: 0,
                        },
                        constraint: fa_key,
                        operands: vec![op_from(fa_row, 0, p0)],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    SelectedInstruction {
                        id: SelectedInstructionId(2),
                        kind: SelectedInstructionKind::Store64 {
                            slot: FrameStorageSlotId::Local(slot),
                            byte_offset: 0,
                        },
                        constraint: store64_key,
                        operands: vec![op_from(store64_row, 0, g0)],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    SelectedInstruction {
                        id: SelectedInstructionId(3),
                        kind: SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count: 1 },
                        constraint: sra_key,
                        operands: vec![op_from(sra_row, 0, p0), op_from(sra_row, 1, p0)],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    SelectedInstruction {
                        id: SelectedInstructionId(4),
                        kind: SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Local(slot),
                            byte_offset: 0,
                        },
                        constraint: fa_key,
                        operands: vec![op_from(fa_row, 0, p1)],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                ],
                terminator: SelectedTerminator::Return {
                    instruction: SelectedInstruction {
                        id: SelectedInstructionId(5),
                        kind: SelectedInstructionKind::ReturnScalar,
                        constraint: ret_key,
                        operands: vec![op_from(ret_row, 0, p1)],
                        implicit_uses: vec![],
                        implicit_defs: vec![],
                        clobbers: vec![],
                        provenance: SelectedInstructionProvenance::default(),
                    },
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }]
            .into(),
        };

        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        assert_eq!(func.local_storage_slots.len(), 1);
        assert!(func.blocks[0].instructions.iter().any(|i| matches!(
            i.kind,
            SelectedInstructionKind::PackedStore128 { byte_offset: 0 }
        )));
    }

    #[test]
    fn coalesces_external_zero_stores_without_carrying_a_vector_across_a_call() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let store_key = keys.store.unwrap();
        let ret_key = keys.return_unit;
        let store_row = env.constraint(store_key).unwrap();
        let gpr = store_row.operands[0].class;

        let ext_ptr = VirtualRegisterId(0);
        let g_zero = VirtualRegisterId(1);
        let vregs = vec![
            VirtualRegister {
                id: ext_ptr,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: g_zero,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(0),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ];
        let store = |id, byte_offset| SelectedInstruction {
            id: SelectedInstructionId(id),
            kind: SelectedInstructionKind::Store {
                byte_offset,
                byte_size: 8,
            },
            constraint: store_key,
            operands: vec![
                op_from(store_row, 0, ext_ptr),
                op_from(store_row, 1, g_zero),
            ],
            implicit_uses: vec![],
            implicit_defs: vec![],
            clobbers: vec![],
            provenance: SelectedInstructionProvenance::default(),
        };
        let call_target = selected_instructions::SelectedCallTarget::Machine(
            semantic_vocabulary::MachineId::new(7).unwrap(),
        );
        let call = SelectedInstructionKind::CallUnit {
            target: call_target,
        };
        let insts = vec![
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                },
                constraint: keys.materialize_i64,
                operands: vec![SelectedOperand {
                    operand: 0,
                    virtual_register: g_zero,
                    access: RegisterOperandAccess::Def,
                    class: gpr,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            store(1, 0),
            store(2, 8),
            store(3, 16),
            store(4, 24),
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: call,
                constraint: env.selected_keys().call_unit[0],
                operands: vec![],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: env
                    .constraint(env.selected_keys().call_unit[0])
                    .unwrap()
                    .clobbers
                    .clone(),
                provenance: SelectedInstructionProvenance::default(),
            },
            store(6, 32),
            store(7, 40),
        ];

        let mut func = base_function(vec![], vregs, insts, SelectedInstructionId(8), ret_key);
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SelectedInstructionKind::PackedMaterializeZero,
                SelectedInstructionKind::PackedStore128 { byte_offset: 0 },
                SelectedInstructionKind::PackedStore128 { byte_offset: 16 },
                call,
                SelectedInstructionKind::PackedMaterializeZero,
                SelectedInstructionKind::PackedStore128 { byte_offset: 32 },
            ]
        );
    }

    #[test]
    fn preserves_boundary_floating_control_slots_around_foreign_calls() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let save_key = keys.save_floating_control.unwrap();
        let restore_key = keys.restore_floating_control.unwrap();
        let ret_key = keys.return_unit;
        let boundary_slot = LocalStorageSlotId::Boundary {
            operation: semantic_vocabulary::OperationId::new(9).unwrap(),
        };
        let insts = vec![
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::SaveFloatingControl {
                    slot: boundary_slot,
                },
                constraint: save_key,
                operands: vec![],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::RestoreFloatingControl {
                    slot: boundary_slot,
                },
                constraint: restore_key,
                operands: vec![],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];
        let mut func = base_function(
            vec![SelectedLocalStorageSlot {
                id: boundary_slot,
                byte_size: 8,
                alignment: 8,
            }],
            vec![],
            insts,
            SelectedInstructionId(2),
            ret_key,
        );
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        assert_eq!(func.local_storage_slots.len(), 1);
        assert_eq!(func.local_storage_slots[0].id, boundary_slot);
        assert_eq!(func.blocks[0].instructions.len(), 2);
    }

    #[test]
    fn eliminates_multi_hop_local_slot_copy_chains_feeding_packed_ops() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let add_key = keys.packed_add_i16x8.unwrap();
        let sra_key = keys.packed_shift_right_i16x8.unwrap();
        let load64_key = keys.load64.unwrap();
        let store_key = keys.store.unwrap();
        let ret_key = keys.return_unit;

        let fa_row = env.constraint(fa_key).unwrap();
        let add_row = env.constraint(add_key).unwrap();
        let sra_row = env.constraint(sra_key).unwrap();
        let load64_row = env.constraint(load64_key).unwrap();
        let store_row = env.constraint(store_key).unwrap();

        let ext_in = VirtualRegisterId(0);
        let ext_out = VirtualRegisterId(1);
        let p1 = VirtualRegisterId(2);
        let p2 = VirtualRegisterId(3);
        let p3 = VirtualRegisterId(4);
        let p_out = VirtualRegisterId(5);
        let g1_lo = VirtualRegisterId(6);
        let g1_hi = VirtualRegisterId(7);
        let g2_lo = VirtualRegisterId(8);
        let g2_hi = VirtualRegisterId(9);
        let g_out_lo = VirtualRegisterId(10);
        let g_out_hi = VirtualRegisterId(11);

        let s1 = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 0,
        };
        let s2 = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 1,
        };
        let s3 = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 2,
        };
        let s_out = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 3,
        };

        let gpr = fa_row.operands[0].class;
        let mut vregs = vec![
            VirtualRegister {
                id: ext_in,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: ext_out,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(2).unwrap(),
                    parameter_index: 1,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ];
        for (vid, iid) in [
            (p1, 0),
            (p2, 2),
            (g1_lo, 3),
            (g1_hi, 5),
            (p3, 7),
            (g2_lo, 8),
            (g2_hi, 10),
            (p_out, 12),
            (g_out_lo, 14),
            (g_out_hi, 15),
        ] {
            vregs.push(VirtualRegister {
                id: vid,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(iid),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        }

        let insts = vec![
            // 0: p1 = &s1
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s1),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // 1: s1 = add(ext_in, ext_in)
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::PackedAddI16x8,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, ext_in),
                    op_from(add_row, 1, ext_in),
                    op_from(add_row, 2, p1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // 2..6: s2 = copy(s1)
            SelectedInstruction {
                id: SelectedInstructionId(2),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s2),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(3),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p1), op_from(load64_row, 1, g1_lo)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(4),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![op_from(store_row, 0, p2), op_from(store_row, 1, g1_lo)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: SelectedInstructionKind::Load64 { byte_offset: 8 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p1), op_from(load64_row, 1, g1_hi)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(6),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![op_from(store_row, 0, p2), op_from(store_row, 1, g1_hi)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // 7..11: s3 = copy(s2)
            SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s3),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p3)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(8),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p2), op_from(load64_row, 1, g2_lo)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(9),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![op_from(store_row, 0, p3), op_from(store_row, 1, g2_lo)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(10),
                kind: SelectedInstructionKind::Load64 { byte_offset: 8 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p2), op_from(load64_row, 1, g2_hi)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(11),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![op_from(store_row, 0, p3), op_from(store_row, 1, g2_hi)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // 12..13: s_out = sra(s3, 1)
            SelectedInstruction {
                id: SelectedInstructionId(12),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(s_out),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_out)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(13),
                kind: SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count: 1 },
                constraint: sra_key,
                operands: vec![op_from(sra_row, 0, p3), op_from(sra_row, 1, p_out)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            // 14..17: store s_out to ext_out
            SelectedInstruction {
                id: SelectedInstructionId(14),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![
                    op_from(load64_row, 0, p_out),
                    op_from(load64_row, 1, g_out_lo),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(15),
                kind: SelectedInstructionKind::Load64 { byte_offset: 8 },
                constraint: load64_key,
                operands: vec![
                    op_from(load64_row, 0, p_out),
                    op_from(load64_row, 1, g_out_hi),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(16),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![
                    op_from(store_row, 0, ext_out),
                    op_from(store_row, 1, g_out_lo),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(17),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![
                    op_from(store_row, 0, ext_out),
                    op_from(store_row, 1, g_out_hi),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];

        let slots = [s1, s2, s3, s_out]
            .into_iter()
            .map(|id| SelectedLocalStorageSlot {
                id,
                byte_size: 16,
                alignment: 16,
            })
            .collect();
        let mut func = base_function(slots, vregs, insts, SelectedInstructionId(18), ret_key);
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        assert!(
            func.local_storage_slots.is_empty(),
            "all multi-hop copy slots should be eliminated, remaining: {:?}",
            func.local_storage_slots
        );
        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&SelectedInstructionKind::PackedAddI16x8Reg));
        assert!(
            kinds.contains(
                &SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count: 1 }
            )
        );
        assert!(kinds.contains(&SelectedInstructionKind::PackedStore128 { byte_offset: 0 }));
        assert!(
            !kinds.iter().any(|k| matches!(
                k,
                SelectedInstructionKind::Load64 { .. } | SelectedInstructionKind::Store { .. }
            )),
            "expected all scalar Load64/Store hops eliminated, got: {kinds:?}"
        );
    }

    #[test]
    fn coalesces_external_zero_stores_across_intervening_local_slot_load() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let store64_key = keys.store64.unwrap();
        let load64_key = keys.load64.unwrap();
        let store_key = keys.store.unwrap();
        let ret_key = keys.return_unit;

        let fa_row = env.constraint(fa_key).unwrap();
        let store64_row = env.constraint(store64_key).unwrap();
        let load64_row = env.constraint(load64_key).unwrap();
        let store_row = env.constraint(store_key).unwrap();
        let gpr = fa_row.operands[0].class;

        let ext_ptr = VirtualRegisterId(0);
        let g_zero = VirtualRegisterId(1);
        let p_local = VirtualRegisterId(2);
        let g_loaded1 = VirtualRegisterId(3);
        let g_loaded2 = VirtualRegisterId(4);
        let slot = LocalStorageSlotId::Inlined {
            call_site: 0,
            slot_index: 0,
        };

        let mut vregs = vec![VirtualRegister {
            id: ext_ptr,
            scalar_type: i64_ty(),
            class: gpr,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: None,
            entry_fixed_view: None,
        }];
        for (vid, iid) in [(g_zero, 0), (p_local, 1), (g_loaded1, 4), (g_loaded2, 6)] {
            vregs.push(VirtualRegister {
                id: vid,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(iid),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        }

        let insts = vec![
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                },
                constraint: keys.materialize_i64,
                operands: vec![SelectedOperand {
                    operand: 0,
                    virtual_register: g_zero,
                    access: RegisterOperandAccess::Def,
                    class: gpr,
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                }],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_local)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(2),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, g_zero)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(3),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 8,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, g_zero)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(4),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![
                    op_from(load64_row, 0, p_local),
                    op_from(load64_row, 1, g_loaded1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![
                    op_from(store_row, 0, ext_ptr),
                    op_from(store_row, 1, g_loaded1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(6),
                kind: SelectedInstructionKind::Load64 { byte_offset: 8 },
                constraint: load64_key,
                operands: vec![
                    op_from(load64_row, 0, p_local),
                    op_from(load64_row, 1, g_loaded2),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                },
                constraint: store_key,
                operands: vec![
                    op_from(store_row, 0, ext_ptr),
                    op_from(store_row, 1, g_loaded2),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];

        let mut func = base_function(
            vec![SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 16,
            }],
            vregs,
            insts,
            SelectedInstructionId(8),
            ret_key,
        );
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        assert!(func.local_storage_slots.is_empty());
        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SelectedInstructionKind::PackedMaterializeZero,
                SelectedInstructionKind::PackedStore128 { byte_offset: 0 },
            ]
        );
    }

    #[test]
    fn folds_repeated_indexed_element_addresses_for_adjacent_blocks() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let ao_key = keys.address_offset.unwrap();
        let add_key = keys.add_i64;
        let mul_key = keys.multiply_i64;
        let pack_load_key = keys.packed_load_i16x4_zero_extend.unwrap();
        let pack_store32_key = keys.packed_narrow_i16x8_to_u8x4_saturating.unwrap();
        let ret_key = keys.return_unit;

        let fa_row = env.constraint(fa_key).unwrap();
        let ao_row = env.constraint(ao_key).unwrap();
        let add_row = env.constraint(add_key).unwrap();
        let mul_row = env.constraint(mul_key).unwrap();
        let pack_load_row = env.constraint(pack_load_key).unwrap();
        let pack_store32_row = env.constraint(pack_store32_key).unwrap();

        let ext_base = VirtualRegisterId(0);
        let idx = VirtualRegisterId(1);
        let c1 = VirtualRegisterId(2);
        let idx1 = VirtualRegisterId(3);
        let c64a = VirtualRegisterId(4);
        let off_a = VirtualRegisterId(5);
        let base_a8 = VirtualRegisterId(6);
        let ptr_a = VirtualRegisterId(7);
        let c64b = VirtualRegisterId(8);
        let off_b = VirtualRegisterId(9);
        let base_b8 = VirtualRegisterId(10);
        let ptr_b = VirtualRegisterId(11);
        let p_tmp_a = VirtualRegisterId(12);
        let p_tmp_b = VirtualRegisterId(13);

        let slot_a = LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(1).unwrap(),
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
        };
        let slot_b = LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(2).unwrap(),
            place: semantic_vocabulary::PlaceId::new(2).unwrap(),
        };
        let gpr = fa_row.operands[0].class;
        let mut vregs = vec![
            VirtualRegister {
                id: ext_base,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: idx,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(2).unwrap(),
                    parameter_index: 1,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
        ];
        for (vid, iid) in [
            (c1, 0),
            (idx1, 1),
            (c64a, 2),
            (off_a, 3),
            (base_a8, 4),
            (ptr_a, 5),
            (c64b, 6),
            (off_b, 7),
            (base_b8, 8),
            (ptr_b, 9),
            (p_tmp_a, 10),
            (p_tmp_b, 12),
        ] {
            vregs.push(VirtualRegister {
                id: vid,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(iid),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        }

        let ob = semantic_vocabulary::ObligationId::new(1).unwrap();
        let ob_fact =
            optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(b"test");
        let ob_prov = SelectedInstructionProvenance {
            obligations: vec![ob],
            ..Default::default()
        };

        let insts = vec![
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(1),
                },
                constraint: keys.materialize_i64,
                operands: vec![op_from(
                    env.constraint(keys.materialize_i64).unwrap(),
                    0,
                    c1,
                )],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov.clone(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::ExactAddI64 {
                    obligation: ob,
                    accepted_fact: ob_fact,
                },
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, idx),
                    op_from(add_row, 1, c1),
                    op_from(add_row, 2, idx1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov.clone(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(2),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(64),
                },
                constraint: keys.materialize_i64,
                operands: vec![op_from(
                    env.constraint(keys.materialize_i64).unwrap(),
                    0,
                    c64a,
                )],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov.clone(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(3),
                kind: SelectedInstructionKind::WrappingMultiplyI64,
                constraint: mul_key,
                operands: vec![
                    op_from(mul_row, 0, idx),
                    op_from(mul_row, 1, c64a),
                    op_from(mul_row, 2, off_a),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov.clone(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(4),
                kind: SelectedInstructionKind::AddressOffset { byte_offset: 8 },
                constraint: ao_key,
                operands: vec![op_from(ao_row, 0, ext_base), op_from(ao_row, 1, base_a8)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: SelectedInstructionKind::ByteViewAddress,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, base_a8),
                    op_from(add_row, 1, off_a),
                    op_from(add_row, 2, ptr_a),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(6),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(64),
                },
                constraint: keys.materialize_i64,
                operands: vec![op_from(
                    env.constraint(keys.materialize_i64).unwrap(),
                    0,
                    c64b,
                )],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov.clone(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::WrappingMultiplyI64,
                constraint: mul_key,
                operands: vec![
                    op_from(mul_row, 0, idx1),
                    op_from(mul_row, 1, c64b),
                    op_from(mul_row, 2, off_b),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: ob_prov,
            },
            SelectedInstruction {
                id: SelectedInstructionId(8),
                kind: SelectedInstructionKind::AddressOffset { byte_offset: 8 },
                constraint: ao_key,
                operands: vec![op_from(ao_row, 0, ext_base), op_from(ao_row, 1, base_b8)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(9),
                kind: SelectedInstructionKind::ByteViewAddress,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, base_b8),
                    op_from(add_row, 1, off_b),
                    op_from(add_row, 2, ptr_b),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(10),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_a),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_tmp_a)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(11),
                kind: SelectedInstructionKind::PackedLoadI16x4ZeroExtend,
                constraint: pack_load_key,
                operands: vec![
                    op_from(pack_load_row, 0, ptr_a),
                    op_from(pack_load_row, 1, p_tmp_a),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(12),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot_b),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, p_tmp_b)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(13),
                kind: SelectedInstructionKind::PackedLoadI16x4ZeroExtend,
                constraint: pack_load_key,
                operands: vec![
                    op_from(pack_load_row, 0, ptr_b),
                    op_from(pack_load_row, 1, p_tmp_b),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(14),
                kind: SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating,
                constraint: pack_store32_key,
                operands: vec![
                    op_from(pack_store32_row, 0, p_tmp_a),
                    op_from(pack_store32_row, 1, ptr_a),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(15),
                kind: SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating,
                constraint: pack_store32_key,
                operands: vec![
                    op_from(pack_store32_row, 0, p_tmp_b),
                    op_from(pack_store32_row, 1, ptr_b),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];

        let mut func = base_function(
            vec![
                SelectedLocalStorageSlot {
                    id: slot_a,
                    byte_size: 16,
                    alignment: 16,
                },
                SelectedLocalStorageSlot {
                    id: slot_b,
                    byte_size: 16,
                    alignment: 16,
                },
            ],
            vregs,
            insts,
            SelectedInstructionId(16),
            ret_key,
        );
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        let mul_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::WrappingMultiplyI64))
            .count();
        let bva_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::ByteViewAddress))
            .count();
        let add_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::ExactAddI64 { .. }))
            .count();
        assert_eq!(mul_count, 1, "kinds={kinds:?}");
        assert_eq!(bva_count, 1, "kinds={kinds:?}");
        assert_eq!(add_count, 0, "kinds={kinds:?}");
        assert!(
            kinds.contains(&SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset: 8 })
        );
        assert!(
            kinds.contains(&SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset: 72 })
        );
    }

    #[test]
    fn folds_paired_external_load64_through_local_slot_to_packed_load128_and_drops_dead_loads() {
        let env =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = env.selected_keys();
        let fa_key = keys.frame_address.unwrap();
        let ao_key = keys.address_offset.unwrap();
        let add_key = keys.add_i64;
        let mul_key = keys.multiply_i64;
        let load64_key = keys.load64.unwrap();
        let store64_key = keys.store64.unwrap();
        let pack_add_key = keys.packed_add_u8x16.unwrap();
        let ret_key = keys.return_unit;

        let fa_row = env.constraint(fa_key).unwrap();
        let ao_row = env.constraint(ao_key).unwrap();
        let add_row = env.constraint(add_key).unwrap();
        let mul_row = env.constraint(mul_key).unwrap();
        let load64_row = env.constraint(load64_key).unwrap();
        let store64_row = env.constraint(store64_key).unwrap();
        let pack_add_row = env.constraint(pack_add_key).unwrap();

        let src1 = VirtualRegisterId(0);
        let src2 = VirtualRegisterId(1);
        let dst = VirtualRegisterId(2);
        let idx = VirtualRegisterId(3);
        let c16 = VirtualRegisterId(4);
        let off1 = VirtualRegisterId(5);
        let p1 = VirtualRegisterId(6);
        let p1_8 = VirtualRegisterId(7);
        let lo1 = VirtualRegisterId(8);
        let hi1 = VirtualRegisterId(9);
        let lp1 = VirtualRegisterId(10);
        let off2 = VirtualRegisterId(11);
        let p2 = VirtualRegisterId(12);
        let p2_8 = VirtualRegisterId(13);
        let lo2 = VirtualRegisterId(14);
        let hi2 = VirtualRegisterId(15);
        let lp2 = VirtualRegisterId(16);
        let lp_out = VirtualRegisterId(17);

        let slot1 = LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(1).unwrap(),
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
        };
        let slot2 = LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(2).unwrap(),
            place: semantic_vocabulary::PlaceId::new(2).unwrap(),
        };
        let gpr = fa_row.operands[0].class;
        let mut vregs: Vec<VirtualRegister> = [src1, src2, dst, idx]
            .into_iter()
            .enumerate()
            .map(|(i, id)| VirtualRegister {
                id,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(i as u64 + 1).unwrap(),
                    parameter_index: i,
                },
                definition_site: None,
                entry_fixed_view: None,
            })
            .collect();
        for (vid, iid) in [
            (c16, 0),
            (off1, 1),
            (p1, 2),
            (p1_8, 3),
            (lo1, 4),
            (hi1, 5),
            (lp1, 6),
            (off2, 9),
            (p2, 10),
            (p2_8, 11),
            (lo2, 12),
            (hi2, 13),
            (lp2, 14),
            (lp_out, 17),
        ] {
            vregs.push(VirtualRegister {
                id: vid,
                scalar_type: i64_ty(),
                class: gpr,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction: SelectedInstructionId(iid),
                    operand: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        }

        let insts = vec![
            SelectedInstruction {
                id: SelectedInstructionId(0),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(16),
                },
                constraint: keys.materialize_i64,
                operands: vec![op_from(
                    env.constraint(keys.materialize_i64).unwrap(),
                    0,
                    c16,
                )],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(1),
                kind: SelectedInstructionKind::WrappingMultiplyI64,
                constraint: mul_key,
                operands: vec![
                    op_from(mul_row, 0, idx),
                    op_from(mul_row, 1, c16),
                    op_from(mul_row, 2, off1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(2),
                kind: SelectedInstructionKind::ByteViewAddress,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, src1),
                    op_from(add_row, 1, off1),
                    op_from(add_row, 2, p1),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(3),
                kind: SelectedInstructionKind::AddressOffset { byte_offset: 8 },
                constraint: ao_key,
                operands: vec![op_from(ao_row, 0, p1), op_from(ao_row, 1, p1_8)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(4),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p1), op_from(load64_row, 1, lo1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(5),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p1_8), op_from(load64_row, 1, hi1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(6),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot1),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, lp1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(7),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot1),
                    byte_offset: 0,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, lo1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(8),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot1),
                    byte_offset: 8,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, hi1)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(9),
                kind: SelectedInstructionKind::WrappingMultiplyI64,
                constraint: mul_key,
                operands: vec![
                    op_from(mul_row, 0, idx),
                    op_from(mul_row, 1, c16),
                    op_from(mul_row, 2, off2),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(10),
                kind: SelectedInstructionKind::ByteViewAddress,
                constraint: add_key,
                operands: vec![
                    op_from(add_row, 0, src2),
                    op_from(add_row, 1, off2),
                    op_from(add_row, 2, p2),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(11),
                kind: SelectedInstructionKind::AddressOffset { byte_offset: 8 },
                constraint: ao_key,
                operands: vec![op_from(ao_row, 0, p2), op_from(ao_row, 1, p2_8)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(12),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p2), op_from(load64_row, 1, lo2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(13),
                kind: SelectedInstructionKind::Load64 { byte_offset: 0 },
                constraint: load64_key,
                operands: vec![op_from(load64_row, 0, p2_8), op_from(load64_row, 1, hi2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(14),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot2),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, lp2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(15),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot2),
                    byte_offset: 0,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, lo2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(16),
                kind: SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot2),
                    byte_offset: 8,
                },
                constraint: store64_key,
                operands: vec![op_from(store64_row, 0, hi2)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(17),
                kind: SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot1),
                    byte_offset: 0,
                },
                constraint: fa_key,
                operands: vec![op_from(fa_row, 0, lp_out)],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
            SelectedInstruction {
                id: SelectedInstructionId(18),
                kind: SelectedInstructionKind::PackedAddU8x16,
                constraint: pack_add_key,
                operands: vec![
                    op_from(pack_add_row, 0, lp1),
                    op_from(pack_add_row, 1, lp2),
                    op_from(pack_add_row, 2, dst),
                ],
                implicit_uses: vec![],
                implicit_defs: vec![],
                clobbers: vec![],
                provenance: SelectedInstructionProvenance::default(),
            },
        ];

        let mut func = base_function(
            vec![
                SelectedLocalStorageSlot {
                    id: slot1,
                    byte_size: 16,
                    alignment: 16,
                },
                SelectedLocalStorageSlot {
                    id: slot2,
                    byte_size: 16,
                    alignment: 16,
                },
            ],
            vregs,
            insts,
            SelectedInstructionId(19),
            ret_key,
        );
        promote_and_simplify(std::slice::from_mut(&mut func), &env).unwrap();

        let kinds: Vec<_> = func.blocks[0].instructions.iter().map(|i| i.kind).collect();
        let load128_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::PackedLoad128 { byte_offset: 0 }))
            .count();
        let scalar_load64_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::Load64 { .. }))
            .count();
        let pair_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::PackedFromGpr64Pair))
            .count();
        let mul_count = kinds
            .iter()
            .filter(|k| matches!(k, SelectedInstructionKind::WrappingMultiplyI64))
            .count();
        assert_eq!(load128_count, 2, "kinds={kinds:?}");
        assert_eq!(scalar_load64_count, 0, "kinds={kinds:?}");
        assert_eq!(pair_count, 0, "kinds={kinds:?}");
        assert_eq!(mul_count, 2, "kinds={kinds:?}");
        assert!(kinds.contains(&SelectedInstructionKind::PackedAddU8x16Reg));
        assert!(kinds.contains(&SelectedInstructionKind::PackedStore128 { byte_offset: 0 }));
    }
}
