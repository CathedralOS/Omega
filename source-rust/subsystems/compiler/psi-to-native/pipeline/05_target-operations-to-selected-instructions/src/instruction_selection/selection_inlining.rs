/*
  Bottom-up acyclic internal-call inlining and single-predecessor fallthrough
  block contraction over `SelectedFunction`s in Stage 05.

  Inlining runs strictly after instruction selection and strictly before sorting
  functions by `MachineId`, keeping Stage 15 publication provenance intact by
  attributing all cloned callee blocks, edges, and instructions to the
  caller's own `CallSiteOwner::Operation(call_op)` and mapping cloned callee
  stack slots to `LocalStorageSlotId::Inlined { call_site, slot_index }`.
*/

use crate::instruction_selection::{SelectedInstructionError, selection::constraints};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{
    RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess, RegisterViewId,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedCallTarget, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedLocalStorageSlot,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
    terminator_instruction, terminator_instruction_mut, terminator_successors,
    terminator_successors_mut,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use target_vocabulary::CallSiteOwner;

const MAX_INLINED_CALLEE_INSTRUCTIONS: usize = 16384;
const MAX_INLINED_CALLEE_BLOCKS: usize = 1024;
const MAX_CALLER_INSTRUCTIONS_AFTER_INLINE: usize = 65536;

pub(crate) fn inline_and_simplify(
    functions: &mut [SelectedFunction],
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    if functions.is_empty() {
        return Ok(());
    }
    let keys = environment.selected_keys();
    let copy_row = environment
        .constraint(keys.copy_i64)
        .ok_or(SelectedInstructionError::MissingConstraint(keys.copy_i64))?;
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(SelectedInstructionError::MissingConstraint(keys.jump))?;
    let copy_class = validate_copy_row(copy_row, keys.copy_i64)?;
    validate_jump_row(jump_row, keys.jump)?;

    let (order, in_cycle) = compute_bottom_up_order(functions);

    for caller_idx in order {
        let mut next_call_site: u32 = 0;
        let mut block_idx = 0;
        while block_idx < functions[caller_idx].blocks.len() {
            let mut inlined_any = false;
            let inst_count = functions[caller_idx].blocks[block_idx].instructions.len();
            for inst_pos in 0..inst_count {
                let Some(SelectedCallTarget::Machine(callee_machine)) =
                    call_target_at(&functions[caller_idx].blocks[block_idx].instructions[inst_pos])
                else {
                    continue;
                };
                let Some(callee_idx) = functions.iter().position(|f| f.machine == callee_machine)
                else {
                    continue;
                };
                if callee_idx == caller_idx || in_cycle[callee_idx] {
                    continue;
                }
                let (caller, callee) = borrow_caller_and_callee(functions, caller_idx, callee_idx);
                if try_inline_call_site(
                    caller,
                    callee,
                    block_idx,
                    inst_pos,
                    &mut next_call_site,
                    copy_row,
                    jump_row,
                    copy_class,
                )? {
                    inlined_any = true;
                    break;
                }
            }
            if !inlined_any {
                block_idx += 1;
            }
        }
        fold_edge_transfer_chains(&mut functions[caller_idx], copy_row)?;
        merge_single_predecessor_blocks(&mut functions[caller_idx]);
        renumber_function(&mut functions[caller_idx]);
    }
    Ok(())
}

fn validate_copy_row(
    row: &RegisterInstructionConstraint,
    key: register_model::RegisterConstraintKey,
) -> Result<RegisterClassId, SelectedInstructionError> {
    if row.operands.len() != 2
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].access != RegisterOperandAccess::Def
        || row.operands[0].class != row.operands[1].class
    {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(row.operands[0].class)
}

fn validate_jump_row(
    row: &RegisterInstructionConstraint,
    key: register_model::RegisterConstraintKey,
) -> Result<(), SelectedInstructionError> {
    if !row.operands.is_empty() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(())
}

fn call_target_at(instruction: &SelectedInstruction) -> Option<SelectedCallTarget> {
    match instruction.kind {
        SelectedInstructionKind::CallUnit { target }
        | SelectedInstructionKind::CallScalar { target }
        | SelectedInstructionKind::CallAggregate { target } => Some(target),
        _ => None,
    }
}

fn borrow_caller_and_callee(
    functions: &mut [SelectedFunction],
    caller_idx: usize,
    callee_idx: usize,
) -> (&mut SelectedFunction, &SelectedFunction) {
    assert_ne!(caller_idx, callee_idx);
    if caller_idx < callee_idx {
        let (left, right) = functions.split_at_mut(callee_idx);
        (&mut left[caller_idx], &right[0])
    } else {
        let (left, right) = functions.split_at_mut(caller_idx);
        (&mut right[0], &left[callee_idx])
    }
}

/// Computes a bottom-up topological order of SCCs (callees before callers) via
/// Tarjan's algorithm and marks every function belonging to a cycle or self-loop.
fn compute_bottom_up_order(functions: &[SelectedFunction]) -> (Vec<usize>, Vec<bool>) {
    let mut index_by_machine: Vec<(MachineId, usize)> = functions
        .iter()
        .enumerate()
        .map(|(idx, function)| (function.machine, idx))
        .collect();
    index_by_machine.sort_by_key(|&(machine, _)| machine);
    let lookup_machine = |machine: MachineId| -> Option<usize> {
        index_by_machine
            .binary_search_by_key(&machine, |&(entry, _)| entry)
            .ok()
            .map(|pos| index_by_machine[pos].1)
    };
    let n = functions.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut has_self_loop = vec![false; n];
    for (idx, function) in functions.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for inst in function.instructions() {
            if let Some(SelectedCallTarget::Machine(target)) = inst.kind.call_target()
                && let Some(target_idx) = lookup_machine(target)
            {
                if target_idx == idx {
                    has_self_loop[idx] = true;
                } else if seen.insert(target_idx) {
                    adj[idx].push(target_idx);
                }
            }
        }
        for block in &function.blocks {
            if let Some(SelectedCallTarget::Machine(target)) =
                terminator_instruction(&block.terminator).kind.call_target()
                && let Some(target_idx) = lookup_machine(target)
            {
                if target_idx == idx {
                    has_self_loop[idx] = true;
                } else if seen.insert(target_idx) {
                    adj[idx].push(target_idx);
                }
            }
        }
    }

    struct TarjanState<'a> {
        adj: &'a [Vec<usize>],
        index: usize,
        indices: Vec<Option<usize>>,
        lowlink: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        sccs: Vec<Vec<usize>>,
    }

    fn strongconnect(v: usize, state: &mut TarjanState<'_>) {
        let v_idx = state.index;
        state.indices[v] = Some(v_idx);
        state.lowlink[v] = v_idx;
        state.index += 1;
        state.stack.push(v);
        state.on_stack[v] = true;

        for &w in &state.adj[v] {
            if state.indices[w].is_none() {
                strongconnect(w, state);
                state.lowlink[v] = state.lowlink[v].min(state.lowlink[w]);
            } else if state.on_stack[w] {
                state.lowlink[v] = state.lowlink[v].min(state.indices[w].unwrap());
            }
        }

        if state.lowlink[v] == v_idx {
            let mut scc = Vec::new();
            while let Some(w) = state.stack.pop() {
                state.on_stack[w] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            state.sccs.push(scc);
        }
    }

    let mut state = TarjanState {
        adj: &adj,
        index: 0,
        indices: vec![None; n],
        lowlink: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        sccs: Vec::new(),
    };
    for v in 0..n {
        if state.indices[v].is_none() {
            strongconnect(v, &mut state);
        }
    }

    // Tarjan produces SCCs in reverse topological order: leaves (callees) come first.
    let mut order = Vec::with_capacity(n);
    let mut in_cycle = vec![false; n];
    for scc in state.sccs {
        let cyclic = scc.len() > 1 || (scc.len() == 1 && has_self_loop[scc[0]]);
        for v in scc {
            in_cycle[v] = cyclic;
            order.push(v);
        }
    }
    (order, in_cycle)
}

#[derive(Clone, Copy)]
struct ParamCopySpec {
    caller_arg: VirtualRegisterId,
    callee_param: VirtualRegisterId,
    new_origin: ParamOriginKind,
}

#[derive(Clone, Copy)]
enum ParamOriginKind {
    EntryParameter {
        source_value: semantic_vocabulary::ValueId,
    },
    AbiTransport {
        place: semantic_vocabulary::PlaceId,
        byte_offset: u32,
    },
}

struct InlinedPlan {
    call_op: OperationId,
    caller_src_block: BlockId,
    slot_map: HashMap<LocalStorageSlotId, LocalStorageSlotId>,
    reg_map: HashMap<VirtualRegisterId, VirtualRegisterId>,
    arg_copies: Vec<ParamCopySpec>,
    ret_copies: Vec<(VirtualRegisterId, VirtualRegisterId)>,
    call_work_provenance: Option<SelectedInstructionProvenance>,
}

fn total_instructions(function: &SelectedFunction) -> usize {
    function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + 1)
        .sum()
}

fn try_inline_call_site(
    caller: &mut SelectedFunction,
    callee: &SelectedFunction,
    caller_block_idx: usize,
    call_inst_pos: usize,
    next_call_site: &mut u32,
    copy_row: &RegisterInstructionConstraint,
    jump_row: &RegisterInstructionConstraint,
    copy_class: RegisterClassId,
) -> Result<bool, SelectedInstructionError> {
    let Some(plan) = check_inline_eligibility(
        caller,
        callee,
        caller_block_idx,
        call_inst_pos,
        *next_call_site,
        copy_class,
    ) else {
        return Ok(false);
    };

    *next_call_site += 1;
    apply_inline(
        caller,
        callee,
        caller_block_idx,
        call_inst_pos,
        plan,
        copy_row,
        jump_row,
    )?;
    Ok(true)
}

fn check_inline_eligibility(
    caller: &SelectedFunction,
    callee: &SelectedFunction,
    caller_block_idx: usize,
    call_inst_pos: usize,
    call_site_id: u32,
    copy_class: RegisterClassId,
) -> Option<InlinedPlan> {
    let callee_inst_count = total_instructions(callee);
    if callee_inst_count > MAX_INLINED_CALLEE_INSTRUCTIONS
        || callee.blocks.len() > MAX_INLINED_CALLEE_BLOCKS
        || total_instructions(caller) + callee_inst_count > MAX_CALLER_INSTRUCTIONS_AFTER_INLINE
    {
        return None;
    }
    if !callee.calls.is_empty()
        || !callee.outgoing_arguments.is_empty()
        || !callee.normalized_foreign_calls.is_empty()
        || !callee.boundary_settlements.is_empty()
    {
        return None;
    }

    let caller_block = &caller.blocks[caller_block_idx];
    if matches!(
        caller_block.origin,
        SelectedBlockOrigin::EdgeTransfer { .. } | SelectedBlockOrigin::CaseDispatch { .. }
    ) || matches!(
        caller_block.terminator,
        SelectedTerminator::OperationTrap { .. }
            | SelectedTerminator::OperationInvokeNoNormal { .. }
    ) {
        return None;
    }

    let call_inst = &caller_block.instructions[call_inst_pos];
    if caller.boundary_settlements.iter().any(|settlement| {
        settlement.block == caller_block.id && settlement.instruction_index == call_inst_pos as u32
    }) {
        return None;
    }

    let contract = caller
        .calls
        .iter()
        .find(|contract| contract.instruction == call_inst.id)?;
    let CallSiteOwner::Operation(call_op) = contract.owner else {
        return None;
    };
    // Arguments passed on the stack are stored into outgoing slots before the
    // call. Inlining removes the call but would leave those slots and stores,
    // and a frame without a call cannot hold outgoing arguments.
    if caller.outgoing_arguments.iter().any(|slot| {
        slot.id.invocation_site == semantic_vocabulary::InvocationSite::Operation(call_op)
    }) {
        return None;
    }

    // Preserve any work settled on the call instruction only when edges are empty
    // and all settled sites belong to `call_op`.
    let call_work_provenance =
        if call_inst.provenance.work.is_empty() && call_inst.provenance.obligations.is_empty() {
            None
        } else {
            if !call_inst.provenance.edges.is_empty() {
                return None;
            }
            let candidate = SelectedInstructionProvenance {
                operations: vec![call_op],
                obligations: call_inst.provenance.obligations.clone(),
                work: call_inst.provenance.work.clone(),
                ..Default::default()
            };
            if !candidate.work_settles_only_claimed_sites() {
                return None;
            }
            Some(candidate)
        };

    // Index call operands by fixed_view.
    let mut call_use_by_view: HashMap<RegisterViewId, &SelectedOperand> = HashMap::new();
    let mut call_defs: Vec<&SelectedOperand> = Vec::new();
    let mut seen_def_views = BTreeSet::new();
    for operand in &call_inst.operands {
        let view = operand.fixed_view?;
        match operand.access {
            RegisterOperandAccess::Use => {
                if call_use_by_view.insert(view, operand).is_some() {
                    return None;
                }
            }
            RegisterOperandAccess::Def => {
                if !seen_def_views.insert(view) {
                    return None;
                }
                call_defs.push(operand);
            }
            RegisterOperandAccess::UseDef => return None,
        }
    }

    // Callee entry block must be Source(_) (though it will be rewritten to Operation { .. }).
    let callee_entry = callee.block(callee.entry_block)?;
    if !matches!(callee_entry.origin, SelectedBlockOrigin::Source(_)) {
        return None;
    }

    // Validate all blocks/instructions/terminators/edges in callee.
    let mut return_block_indices = Vec::new();
    let slot_map: HashMap<LocalStorageSlotId, LocalStorageSlotId> = callee
        .local_storage_slots
        .iter()
        .enumerate()
        .map(|(slot_index, slot)| {
            (
                slot.id,
                LocalStorageSlotId::Inlined {
                    call_site: call_site_id,
                    slot_index: slot_index as u32,
                },
            )
        })
        .collect();

    for (b_idx, block) in callee.blocks.iter().enumerate() {
        for inst in &block.instructions {
            if !instruction_is_inline_safe(inst, &slot_map) {
                return None;
            }
        }
        if !instruction_is_inline_safe(terminator_instruction(&block.terminator), &slot_map) {
            return None;
        }
        match &block.terminator {
            SelectedTerminator::Return { .. } => return_block_indices.push(b_idx),
            SelectedTerminator::Jump { .. }
            | SelectedTerminator::ConditionalBranch { .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { .. }
            | SelectedTerminator::OperationTrap { .. } => {}
            SelectedTerminator::Crash { .. }
            | SelectedTerminator::EliminateEmpty { .. }
            | SelectedTerminator::InvokeNoNormal { .. }
            | SelectedTerminator::OperationInvokeNoNormal { .. }
            | SelectedTerminator::HostedExitProcess { .. } => return None,
        }
        // Cloned branches become continuations of the caller's operation.
        // A callee block parameter or case payload belongs to its original Psi edge.
        if terminator_successors(&block.terminator).iter().any(|edge| {
            !edge.bindings.is_empty()
                || !edge.structural_bindings.is_empty()
                || edge.structural_case.is_some()
        }) {
            return None;
        }
    }
    for access in &callee.memory_accesses {
        if let SelectedMemoryAccessRole::ReadLocal { slot }
        | SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } = access.role
            && !slot_map.contains_key(&slot)
        {
            return None;
        }
    }

    if return_block_indices.is_empty() {
        return None;
    }
    if !call_defs.is_empty() && return_block_indices.len() != 1 {
        return None;
    }

    // Match return values by fixed_view.
    let mut ret_copies = Vec::with_capacity(call_defs.len());
    if !call_defs.is_empty() {
        let ret_inst = terminator_instruction(&callee.blocks[return_block_indices[0]].terminator);
        let mut ret_uses: HashMap<RegisterViewId, &SelectedOperand> = HashMap::new();
        for op in &ret_inst.operands {
            if op.access == RegisterOperandAccess::Use {
                let view = op.fixed_view?;
                if ret_uses.insert(view, op).is_some() {
                    return None;
                }
            }
        }
        for call_def in &call_defs {
            if call_def.class != copy_class {
                return None;
            }
            let view = call_def.fixed_view?;
            let ret_op = ret_uses.get(&view)?;
            if ret_op.class != copy_class {
                return None;
            }
            ret_copies.push((ret_op.virtual_register, call_def.virtual_register));
        }
    }

    // Match entry parameter registers and verify all entry-fixed registers are accounted for.
    let mut arg_copies = Vec::new();
    let mut used_param_views = BTreeSet::new();
    for reg in &callee.virtual_registers {
        let is_entry_origin = matches!(
            reg.origin,
            VirtualRegisterOrigin::EntryParameter { .. }
                | VirtualRegisterOrigin::StructuralParameter { .. }
        );
        if reg.entry_fixed_view.is_some() || is_entry_origin {
            let view = reg.entry_fixed_view?;
            if reg.class != copy_class || !used_param_views.insert(view) {
                return None;
            }
            let caller_arg_op = call_use_by_view.get(&view)?;
            if caller_arg_op.class != copy_class {
                return None;
            }
            let new_origin = match reg.origin {
                VirtualRegisterOrigin::EntryParameter { source_value, .. } => {
                    ParamOriginKind::EntryParameter { source_value }
                }
                VirtualRegisterOrigin::StructuralParameter { place, .. } => {
                    ParamOriginKind::AbiTransport {
                        place,
                        byte_offset: 0,
                    }
                }
                VirtualRegisterOrigin::AbiTransport {
                    place, byte_offset, ..
                } => ParamOriginKind::AbiTransport { place, byte_offset },
                _ => return None,
            };
            arg_copies.push(ParamCopySpec {
                caller_arg: caller_arg_op.virtual_register,
                callee_param: reg.id,
                new_origin,
            });
        }
    }

    if call_work_provenance.is_some()
        && arg_copies.is_empty()
        && callee.blocks.iter().all(|b| b.instructions.is_empty())
    {
        return None;
    }

    Some(InlinedPlan {
        call_op,
        caller_src_block: caller_block.source_block(),
        slot_map,
        reg_map: HashMap::with_capacity(callee.virtual_registers.len()),
        arg_copies,
        ret_copies,
        call_work_provenance,
    })
}

fn instruction_is_inline_safe(
    inst: &SelectedInstruction,
    slot_map: &HashMap<LocalStorageSlotId, LocalStorageSlotId>,
) -> bool {
    match inst.kind {
        SelectedInstructionKind::SaveFloatingControl { .. }
        | SelectedInstructionKind::RestoreFloatingControl { .. }
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. } => false,
        SelectedInstructionKind::Store64 { slot, .. }
        | SelectedInstructionKind::FrameAddress { slot, .. } => match slot {
            FrameStorageSlotId::Local(local) => slot_map.contains_key(&local),
            FrameStorageSlotId::ActivationBase => true,
            FrameStorageSlotId::Incoming { .. } | FrameStorageSlotId::Outgoing(_) => false,
        },
        _ => true,
    }
}

fn apply_inline(
    caller: &mut SelectedFunction,
    callee: &SelectedFunction,
    caller_block_idx: usize,
    call_inst_pos: usize,
    mut plan: InlinedPlan,
    copy_row: &RegisterInstructionConstraint,
    jump_row: &RegisterInstructionConstraint,
) -> Result<(), SelectedInstructionError> {
    let call_op = plan.call_op;
    let caller_src_block = plan.caller_src_block;
    let caller_block_id = caller.blocks[caller_block_idx].id;

    // 1. Add mapped local storage slots to caller.
    let mut new_slots: Vec<SelectedLocalStorageSlot> = callee
        .local_storage_slots
        .iter()
        .map(|slot| SelectedLocalStorageSlot {
            id: plan.slot_map[&slot.id],
            byte_size: slot.byte_size,
            alignment: slot.alignment,
        })
        .collect();
    caller.local_storage_slots.append(&mut new_slots);

    // 2. Allocate fresh virtual registers for all callee registers in caller.
    let reg_base = caller.virtual_registers.len() as u32;
    for (idx, reg) in callee.virtual_registers.iter().enumerate() {
        let new_id = VirtualRegisterId(reg_base + idx as u32);
        plan.reg_map.insert(reg.id, new_id);
        let mut cloned = reg.clone();
        cloned.id = new_id;
        cloned.entry_fixed_view = None;
        caller.virtual_registers.push(cloned);
    }

    // Allocate fresh block IDs for cloned callee blocks + continuation block.
    let block_base = caller.blocks.len() as u32;
    let mut block_map: HashMap<SelectedBlockId, SelectedBlockId> =
        HashMap::with_capacity(callee.blocks.len());
    for (idx, block) in callee.blocks.iter().enumerate() {
        block_map.insert(block.id, SelectedBlockId(block_base + idx as u32));
    }
    let cont_block_id = SelectedBlockId(block_base + callee.blocks.len() as u32);
    let cloned_callee_entry = block_map[&callee.entry_block];

    // Pick next temporary instruction ID safely above any currently used ID in caller.
    let mut next_inst_id = caller
        .blocks
        .iter()
        .flat_map(|b| {
            b.instructions
                .iter()
                .map(|i| i.id.0)
                .chain(std::iter::once(terminator_instruction(&b.terminator).id.0))
        })
        .max()
        .map_or(0, |m| m + 1);
    let mut alloc_inst_id = || {
        let id = SelectedInstructionId(next_inst_id);
        next_inst_id += 1;
        id
    };

    // 3. Split caller_block at call_inst_pos.
    let (mut tail_instructions, orig_terminator) = {
        let orig_caller_block = &mut caller.blocks[caller_block_idx];
        let tail_instructions = orig_caller_block.instructions.split_off(call_inst_pos);
        let orig_terminator = std::mem::replace(
            &mut orig_caller_block.terminator,
            // Placeholder; replaced right below.
            SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(0),
                    kind: SelectedInstructionKind::Jump,
                    constraint: jump_row.key,
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance::default(),
                },
                successor: SelectedSuccessor {
                    role: SelectedSuccessorRole::OperationContinuation { operation: call_op },
                    psi_edge: None,
                    block: cloned_callee_entry,
                    source_target: caller_src_block,
                    bindings: Vec::new(),
                    structural_bindings: Vec::new(),
                    structural_case: None,
                    work: Vec::new(),
                },
            },
        );
        (tail_instructions, orig_terminator)
    };
    let call_inst = tail_instructions.remove(0);

    let op_provenance = || SelectedInstructionProvenance {
        operations: vec![call_op],
        ..Default::default()
    };
    let mut work_provenance = plan.call_work_provenance.take();

    // Emit parameter copies at the end of caller_block's prefix.
    for arg in &plan.arg_copies {
        let dst = plan.reg_map[&arg.callee_param];
        let copy_id = alloc_inst_id();
        let prov = work_provenance.take().unwrap_or_else(op_provenance);
        let copy_inst = constraints::instruction_from_row(
            copy_id,
            SelectedInstructionKind::CopyI64,
            &[arg.caller_arg, dst],
            prov,
            copy_row,
        )?;
        caller.blocks[caller_block_idx].instructions.push(copy_inst);
        caller.virtual_registers[dst.0 as usize].origin = match arg.new_origin {
            ParamOriginKind::EntryParameter { source_value } => {
                VirtualRegisterOrigin::InstructionResult {
                    instruction: copy_id,
                    source_value,
                }
            }
            ParamOriginKind::AbiTransport { place, byte_offset } => {
                VirtualRegisterOrigin::AbiTransport {
                    instruction: copy_id,
                    place,
                    byte_offset,
                }
            }
        };
    }

    // Never attach `work` to an OperationContinuation Jump; `liveness.rs:1770`
    // requires empty `work` on OperationContinuation terminators.
    let entry_jump_inst = constraints::instruction_from_row(
        alloc_inst_id(),
        SelectedInstructionKind::Jump,
        &[],
        op_provenance(),
        jump_row,
    )?;
    caller.blocks[caller_block_idx].terminator = SelectedTerminator::Jump {
        instruction: entry_jump_inst,
        successor: SelectedSuccessor {
            role: SelectedSuccessorRole::OperationContinuation { operation: call_op },
            psi_edge: None,
            block: cloned_callee_entry,
            source_target: caller_src_block,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: None,
            work: Vec::new(),
        },
    };

    // 4. Clone and remap callee blocks + instructions + memory accesses.
    let mut inst_map: HashMap<SelectedInstructionId, SelectedInstructionId> = HashMap::new();
    let mut cloned_blocks = Vec::with_capacity(callee.blocks.len() + 1);
    let mut ret_copies = Some(plan.ret_copies);

    for block in &callee.blocks {
        let new_block_id = block_map[&block.id];
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for inst in &block.instructions {
            let new_id = alloc_inst_id();
            inst_map.insert(inst.id, new_id);
            let prov = work_provenance.take().unwrap_or_else(op_provenance);
            instructions.push(remap_callee_instruction(
                inst,
                new_id,
                prov,
                &plan.reg_map,
                &plan.slot_map,
            ));
        }

        let old_term_id = terminator_instruction(&block.terminator).id;
        let new_terminator = match &block.terminator {
            SelectedTerminator::Return { .. } => {
                if let Some(copies) = ret_copies.take() {
                    for (callee_ret_src, caller_def_dst) in copies {
                        let mapped_src = plan.reg_map[&callee_ret_src];
                        let copy_id = alloc_inst_id();
                        let copy_inst = constraints::instruction_from_row(
                            copy_id,
                            SelectedInstructionKind::CopyI64,
                            &[mapped_src, caller_def_dst],
                            op_provenance(),
                            copy_row,
                        )?;
                        instructions.push(copy_inst);
                        remap_origin_instruction(
                            &mut caller.virtual_registers[caller_def_dst.0 as usize].origin,
                            copy_id,
                        );
                    }
                }
                let ret_jump_id = alloc_inst_id();
                inst_map.insert(old_term_id, ret_jump_id);
                let ret_jump_inst = constraints::instruction_from_row(
                    ret_jump_id,
                    SelectedInstructionKind::Jump,
                    &[],
                    op_provenance(),
                    jump_row,
                )?;
                SelectedTerminator::Jump {
                    instruction: ret_jump_inst,
                    successor: SelectedSuccessor {
                        role: SelectedSuccessorRole::OperationContinuation { operation: call_op },
                        psi_edge: None,
                        block: cont_block_id,
                        source_target: caller_src_block,
                        bindings: Vec::new(),
                        structural_bindings: Vec::new(),
                        structural_case: None,
                        work: Vec::new(),
                    },
                }
            }
            other => {
                let new_term_id = alloc_inst_id();
                inst_map.insert(old_term_id, new_term_id);
                let mut cloned_term = other.clone();
                {
                    let t_inst = terminator_instruction_mut(&mut cloned_term);
                    *t_inst = remap_callee_instruction(
                        t_inst,
                        new_term_id,
                        op_provenance(),
                        &plan.reg_map,
                        &plan.slot_map,
                    );
                }
                if let SelectedTerminator::OperationTrap { operation, .. } = &mut cloned_term {
                    *operation = call_op;
                }
                for succ in terminator_successors_mut(&mut cloned_term) {
                    succ.role = SelectedSuccessorRole::OperationContinuation { operation: call_op };
                    succ.psi_edge = None;
                    succ.block = block_map[&succ.block];
                    succ.source_target = caller_src_block;
                    succ.work.clear();
                }
                cloned_term
            }
        };

        cloned_blocks.push(SelectedBlock {
            id: new_block_id,
            origin: SelectedBlockOrigin::Operation {
                source: caller_src_block,
                operation: call_op,
            },
            instructions,
            terminator: new_terminator,
        });
    }

    // Continuation block holds caller suffix instructions + original terminator.
    cloned_blocks.push(SelectedBlock {
        id: cont_block_id,
        origin: SelectedBlockOrigin::Operation {
            source: caller_src_block,
            operation: call_op,
        },
        instructions: tail_instructions,
        terminator: orig_terminator,
    });
    caller.blocks.extend(cloned_blocks);

    // 5. Remap remaining VirtualRegister origins for cloned callee registers.
    for (idx, reg) in callee.virtual_registers.iter().enumerate() {
        let new_id = VirtualRegisterId(reg_base + idx as u32);
        let is_param_copy = plan.arg_copies.iter().any(|arg| arg.callee_param == reg.id);
        if is_param_copy {
            continue;
        }
        let target_reg = &mut caller.virtual_registers[new_id.0 as usize];
        target_reg.origin = match reg.origin {
            VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand,
            } => VirtualRegisterOrigin::InstructionScratch {
                instruction: inst_map[&instruction],
                operand,
            },
            VirtualRegisterOrigin::StructuralObservation {
                instruction,
                place,
                byte_offset,
            } => VirtualRegisterOrigin::StructuralObservation {
                instruction: inst_map[&instruction],
                place,
                byte_offset,
            },
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction,
                source_value,
            } => VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: inst_map[&instruction],
                source_value,
            },
            VirtualRegisterOrigin::SpillAddress {
                instruction,
                register,
            } => VirtualRegisterOrigin::SpillAddress {
                instruction: inst_map[&instruction],
                register: plan.reg_map[&register],
            },
            VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            } => VirtualRegisterOrigin::AbiTransport {
                instruction: inst_map[&instruction],
                place,
                byte_offset,
            },
            VirtualRegisterOrigin::BlockParameter {
                source_value,
                block,
                parameter_index,
            } => VirtualRegisterOrigin::BlockParameter {
                source_value,
                block: block_map[&block],
                parameter_index,
            },
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            } => VirtualRegisterOrigin::InstructionResult {
                instruction: inst_map[&instruction],
                source_value,
            },
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => {
                return Err(SelectedInstructionError::custody_because(
                    "unmatched callee entry parameter escaped eligibility check",
                ));
            }
        };
    }

    // 6. Update caller calls, memory accesses, and boundary settlements.
    caller.calls.retain(|c| c.instruction != call_inst.id);
    caller
        .memory_accesses
        .retain(|m| m.instruction != call_inst.id);
    for access in &callee.memory_accesses {
        let mut cloned = access.clone();
        cloned.instruction = inst_map[&access.instruction];
        cloned.origin = SelectedMemoryAccessOrigin::Operation(call_op);
        match &mut cloned.role {
            SelectedMemoryAccessRole::ReadLocal { slot }
            | SelectedMemoryAccessRole::WriteLocal { slot }
            | SelectedMemoryAccessRole::AddressLocal { slot } => {
                *slot = plan.slot_map[slot];
            }
            _ => {}
        }
        caller.memory_accesses.push(cloned);
    }
    for settlement in &mut caller.boundary_settlements {
        if settlement.block == caller_block_id
            && settlement.instruction_index > call_inst_pos as u32
        {
            settlement.block = cont_block_id;
            settlement.instruction_index -= call_inst_pos as u32 + 1;
        }
    }

    Ok(())
}

fn remap_origin_instruction(origin: &mut VirtualRegisterOrigin, new_inst: SelectedInstructionId) {
    match origin {
        VirtualRegisterOrigin::InstructionScratch { instruction, .. }
        | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
        | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
        | VirtualRegisterOrigin::SpillAddress { instruction, .. }
        | VirtualRegisterOrigin::AbiTransport { instruction, .. }
        | VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
            *instruction = new_inst;
        }
        VirtualRegisterOrigin::EntryParameter { .. }
        | VirtualRegisterOrigin::StructuralParameter { .. }
        | VirtualRegisterOrigin::BlockParameter { .. } => {}
    }
}

fn remap_callee_instruction(
    inst: &SelectedInstruction,
    new_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    reg_map: &HashMap<VirtualRegisterId, VirtualRegisterId>,
    slot_map: &HashMap<LocalStorageSlotId, LocalStorageSlotId>,
) -> SelectedInstruction {
    let mut cloned = inst.clone();
    cloned.id = new_id;
    cloned.provenance = provenance;
    for op in &mut cloned.operands {
        op.virtual_register = reg_map[&op.virtual_register];
    }
    match &mut cloned.kind {
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(local),
            ..
        }
        | SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(local),
            ..
        } => {
            *local = slot_map[local];
        }
        _ => {}
    }
    cloned
}

/// Folds `A -> E -> B` into `A` when `A` ends in a plain jump to the edge
/// transfer block `E`, `E` is `A`'s only successor and `B`'s only predecessor
/// chain, and `B` is entered only through `E`. The source compiler gives every
/// sub-expression its own block, so straight-line code otherwise reaches native
/// code as a chain of jumps through out-of-line edge blocks. `B`'s parameters
/// become results of explicit copies, which allocation usually coalesces into
/// nothing, and the edges' work settlements move to those copies.
fn fold_edge_transfer_chains(
    function: &mut SelectedFunction,
    copy_row: &RegisterInstructionConstraint,
) -> Result<(), SelectedInstructionError> {
    loop {
        let index_by_id: HashMap<SelectedBlockId, usize> = function
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut pred_counts = vec![0usize; function.blocks.len()];
        for block in function.blocks.iter() {
            for succ in terminator_successors(&block.terminator) {
                if let Some(&dst) = index_by_id.get(&succ.block) {
                    pred_counts[dst] += 1;
                }
            }
        }
        let mut found = None;
        for (a_idx, a) in function.blocks.iter().enumerate() {
            if matches!(
                a.origin,
                SelectedBlockOrigin::EdgeTransfer { .. } | SelectedBlockOrigin::CaseDispatch { .. }
            ) {
                continue;
            }
            let SelectedTerminator::Jump {
                instruction: a_jump,
                successor: a_succ,
            } = &a.terminator
            else {
                continue;
            };
            // The jump into an edge transfer block only names the bindings;
            // the edge block's own successor carries their registers.
            if !a_jump.provenance.work.is_empty()
                || a_succ.bindings.iter().any(|binding| {
                    binding.transport != selected_instructions::SelectedValueTransport::Unused
                })
                || !a_succ.structural_bindings.is_empty()
                || a_succ.structural_case.is_some()
            {
                continue;
            }
            let Some(&e_idx) = index_by_id.get(&a_succ.block) else {
                continue;
            };
            let e = &function.blocks[e_idx];
            if e_idx == a_idx
                || pred_counts[e_idx] != 1
                || !matches!(e.origin, SelectedBlockOrigin::EdgeTransfer { .. })
            {
                continue;
            }
            let SelectedTerminator::Jump {
                instruction: e_jump,
                successor: e_succ,
            } = &e.terminator
            else {
                continue;
            };
            // The jump's work settles on the first parameter copy, so an
            // edge with work and no bound parameter has nowhere to put it.
            if (!a_succ.work.is_empty() && e_succ.bindings.is_empty())
                || !e_jump.provenance.work.is_empty()
                || !e_succ.structural_bindings.is_empty()
                || e_succ.structural_case.is_some()
                || !e_succ.work.is_empty()
                || e_succ.bindings.iter().any(|binding| {
                    !matches!(
                        binding.transport,
                        selected_instructions::SelectedValueTransport::Registers { .. }
                    )
                })
            {
                continue;
            }
            let Some(&b_idx) = index_by_id.get(&e_succ.block) else {
                continue;
            };
            let b = &function.blocks[b_idx];
            if b_idx == a_idx
                || b_idx == e_idx
                || b.id == function.entry_block
                || pred_counts[b_idx] != 1
                || !matches!(
                    b.origin,
                    SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. }
                )
                || matches!(
                    b.terminator,
                    SelectedTerminator::OperationTrap { .. }
                        | SelectedTerminator::OperationInvokeNoNormal { .. }
                )
                // An operation's continuation edge names the source block
                // that owns it, which must stay the merged block's own.
                || (b.source_block() != a.source_block()
                    && terminator_successors(&b.terminator).iter().any(|next| {
                        matches!(next.role, SelectedSuccessorRole::OperationContinuation { .. })
                    }))
            {
                continue;
            }
            // Every parameter of `B` must be one the edge binds.
            let bound: BTreeSet<VirtualRegisterId> = e_succ
                .bindings
                .iter()
                .filter_map(|binding| match binding.transport {
                    selected_instructions::SelectedValueTransport::Registers {
                        parameter, ..
                    } => Some(parameter),
                    selected_instructions::SelectedValueTransport::Unused => None,
                })
                .collect();
            if function.virtual_registers.iter().enumerate().any(|(index, reg)| {
                matches!(reg.origin, VirtualRegisterOrigin::BlockParameter { block, .. } if block == b.id)
                    && !bound.contains(&VirtualRegisterId(index as u32))
            }) {
                continue;
            }
            found = Some((a_idx, e_idx, b_idx));
            break;
        }
        let Some((a_idx, e_idx, b_idx)) = found else {
            return Ok(());
        };
        let first_id = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|inst| inst.id.0)
                    .chain(std::iter::once(
                        terminator_instruction(&block.terminator).id.0,
                    ))
            })
            .max()
            .unwrap_or(0)
            + 1;
        let a_id = function.blocks[a_idx].id;
        let b_id = function.blocks[b_idx].id;
        let e_id = function.blocks[e_idx].id;
        let (a_edge, a_work) = match &function.blocks[a_idx].terminator {
            SelectedTerminator::Jump { successor, .. } => {
                (successor.psi_edge, successor.work.clone())
            }
            _ => unreachable!("checked above"),
        };
        let (e_edge, bindings) = match &function.blocks[e_idx].terminator {
            SelectedTerminator::Jump { successor, .. } => {
                (successor.psi_edge, successor.bindings.clone())
            }
            _ => unreachable!("checked above"),
        };
        let e_instructions = function.blocks[e_idx].instructions.clone();
        let mut copies = Vec::with_capacity(bindings.len());
        let mut pending_work = Some(a_work);
        for (id, binding) in (first_id..).zip(&bindings) {
            let selected_instructions::SelectedValueTransport::Registers {
                argument,
                parameter,
            } = binding.transport
            else {
                unreachable!("checked above");
            };
            let id = SelectedInstructionId(id);
            let mut provenance = SelectedInstructionProvenance {
                values: vec![binding.semantic.parameter],
                ..Default::default()
            };
            if let Some(work) = pending_work.take().filter(|work| !work.is_empty()) {
                provenance.edges.extend(a_edge);
                provenance.work = work;
            }
            provenance.edges.extend(e_edge);
            provenance.edges.sort();
            provenance.edges.dedup();
            copies.push(constraints::instruction_from_row(
                id,
                SelectedInstructionKind::CopyI64,
                &[argument, parameter],
                provenance,
                copy_row,
            )?);
            function.virtual_registers[parameter.0 as usize].origin =
                VirtualRegisterOrigin::InstructionResult {
                    instruction: id,
                    source_value: binding.semantic.parameter,
                };
        }
        debug_assert!(pending_work.is_none_or(|work| work.is_empty()));
        let a_len = (function.blocks[a_idx].instructions.len()
            + e_instructions.len()
            + copies.len()) as u32;
        let b_block = function.blocks[b_idx].clone();
        let a_block = &mut function.blocks[a_idx];
        a_block.instructions.extend(e_instructions);
        a_block.instructions.extend(copies);
        a_block.instructions.extend(b_block.instructions);
        a_block.terminator = b_block.terminator;
        for settlement in &mut function.boundary_settlements {
            if settlement.block == b_id {
                settlement.block = a_id;
                settlement.instruction_index += a_len;
            }
        }
        let mut blocks: Vec<SelectedBlock> = function.blocks.iter().cloned().collect();
        blocks.retain(|block| block.id != e_id && block.id != b_id);
        function.blocks = blocks.into();
    }
}

fn merge_single_predecessor_blocks(function: &mut SelectedFunction) {
    if function.blocks.len() <= 1 {
        return;
    }

    let mut block_param_blocks: BTreeSet<SelectedBlockId> = BTreeSet::new();
    for reg in &function.virtual_registers {
        if let VirtualRegisterOrigin::BlockParameter { block, .. } = reg.origin {
            block_param_blocks.insert(block);
        }
    }

    let mut removed = vec![false; function.blocks.len()];
    let mut index_by_id: HashMap<SelectedBlockId, usize> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();

    loop {
        let mut pred_counts = vec![0usize; function.blocks.len()];
        for (i, block) in function.blocks.iter().enumerate() {
            if removed[i] {
                continue;
            }
            for succ in terminator_successors(&block.terminator) {
                if let Some(&dst) = index_by_id.get(&succ.block) {
                    pred_counts[dst] += 1;
                }
            }
        }

        let mut merged_any = false;
        for b1_idx in 0..function.blocks.len() {
            if removed[b1_idx] {
                continue;
            }
            let (b2_idx, b1_id, b2_id) = {
                let b1 = &function.blocks[b1_idx];
                if matches!(
                    b1.origin,
                    SelectedBlockOrigin::EdgeTransfer { .. }
                        | SelectedBlockOrigin::CaseDispatch { .. }
                ) {
                    continue;
                }
                let SelectedTerminator::Jump {
                    instruction: jump_inst,
                    successor,
                } = &b1.terminator
                else {
                    continue;
                };
                if !jump_inst.provenance.work.is_empty()
                    || !successor.bindings.is_empty()
                    || !successor.structural_bindings.is_empty()
                    || !successor.work.is_empty()
                    || successor.structural_case.is_some()
                {
                    continue;
                }
                let Some(&b2_idx) = index_by_id.get(&successor.block) else {
                    continue;
                };
                if b1_idx == b2_idx || removed[b2_idx] {
                    continue;
                }
                let b2 = &function.blocks[b2_idx];
                if b2.id == function.entry_block
                    || pred_counts[b2_idx] != 1
                    || b1.source_block() != b2.source_block()
                    || block_param_blocks.contains(&b2.id)
                    || !matches!(
                        b2.origin,
                        SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. }
                    )
                    || matches!(
                        b2.terminator,
                        SelectedTerminator::OperationTrap { .. }
                            | SelectedTerminator::OperationInvokeNoNormal { .. }
                    )
                    || terminator_successors(&b2.terminator).iter().any(|next| {
                        matches!(
                            next.role,
                            SelectedSuccessorRole::EdgeTransferContinuation
                                | SelectedSuccessorRole::CaseDispatchContinuation
                        )
                    })
                {
                    continue;
                }
                (b2_idx, b1.id, b2.id)
            };

            let b2_taken = function.blocks[b2_idx].clone();
            let b1_orig_len = function.blocks[b1_idx].instructions.len() as u32;
            let b1_mut = &mut function.blocks[b1_idx];
            b1_mut.instructions.extend(b2_taken.instructions);
            b1_mut.terminator = b2_taken.terminator;

            for settlement in &mut function.boundary_settlements {
                if settlement.block == b2_id {
                    settlement.block = b1_id;
                    settlement.instruction_index += b1_orig_len;
                }
            }
            removed[b2_idx] = true;
            // Recompute predecessors carefully after contraction so chains collapse
            // safely without dropping any multi-predecessor guard.
            merged_any = true;
            break;
        }
        if !merged_any {
            break;
        }
    }

    if removed.iter().any(|&r| r) {
        let kept: Vec<SelectedBlock> = function
            .blocks
            .iter()
            .enumerate()
            .filter(|&(i, _)| !removed[i])
            .map(|(_, b)| b.clone())
            .collect();
        function.blocks = kept.into();
    }
    let _ = &mut index_by_id;
}

pub(crate) fn renumber_function(function: &mut SelectedFunction) {
    let block_map: BTreeMap<SelectedBlockId, SelectedBlockId> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.id, SelectedBlockId(idx as u32)))
        .collect();

    function.entry_block = block_map[&function.entry_block];

    let mut inst_map: HashMap<SelectedInstructionId, SelectedInstructionId> = HashMap::new();
    let mut next_inst_id = 0u32;

    for (idx, block) in function.blocks.iter_mut().enumerate() {
        block.id = SelectedBlockId(idx as u32);
        for inst in &mut block.instructions {
            let new_id = SelectedInstructionId(next_inst_id);
            next_inst_id += 1;
            inst_map.insert(inst.id, new_id);
            inst.id = new_id;
        }
        let term_inst = terminator_instruction_mut(&mut block.terminator);
        let new_term_id = SelectedInstructionId(next_inst_id);
        next_inst_id += 1;
        inst_map.insert(term_inst.id, new_term_id);
        term_inst.id = new_term_id;

        for succ in terminator_successors_mut(&mut block.terminator) {
            succ.block = block_map[&succ.block];
        }
    }

    for reg in &mut function.virtual_registers {
        match &mut reg.origin {
            VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::SpillAddress { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. }
            | VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
                *instruction = inst_map[instruction];
            }
            VirtualRegisterOrigin::BlockParameter { block, .. } => {
                *block = block_map[block];
            }
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => {}
        }
    }

    for call in &mut function.calls {
        call.instruction = inst_map[&call.instruction];
    }
    for foreign in &mut function.normalized_foreign_calls {
        foreign.instruction = inst_map[&foreign.instruction];
    }
    for access in &mut function.memory_accesses {
        access.instruction = inst_map[&access.instruction];
    }
    for settlement in &mut function.boundary_settlements {
        settlement.block = block_map[&settlement.block];
    }

    debug_assert!(function.ids_are_positions());
}

#[cfg(test)]
mod tests {
    use super::inline_and_simplify;
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use operation_evidence::provenance::{EffectLink, PsiProvenance, WorkSettlement};
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedBlock, SelectedBlockId,
        SelectedBlockOrigin, SelectedCallContract, SelectedCallTarget, SelectedFunction,
        SelectedFunctionContracts, SelectedInstruction, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionProvenance, SelectedLocalStorageSlot,
        SelectedOperand, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
        VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ScalarType, ValueId};
    use std::sync::Arc;
    use target::NativeTarget;
    use target_vocabulary::{
        CallSiteOwner, LegalizedScalarCall, NativeCallOrigin, NativeCallTarget,
        TerminalPsiProvenance,
    };

    fn i64_type() -> ScalarType {
        ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
                .unwrap(),
        )
    }

    #[test]
    fn inlines_acyclic_callee_and_contracts_single_predecessor_fallthrough_blocks() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let keys = environment.selected_keys();
        let gp_class = environment.constraint(keys.copy_i64).unwrap().operands[0].class;
        let caller_machine = MachineId::new(1).unwrap();
        let callee_machine = MachineId::new(2).unwrap();
        let caller_op = OperationId::new(10).unwrap();
        let caller_ret_edge = EdgeId::new(100).unwrap();
        let callee_ret_edge = EdgeId::new(200).unwrap();
        let caller_b0 = BlockId::new(1).unwrap();
        let caller_b1 = BlockId::new(2).unwrap();
        let callee_b0 = BlockId::new(1).unwrap();
        let callee_op = OperationId::new(20).unwrap();

        let call_row = keys
            .call_scalar
            .iter()
            .filter_map(|key| environment.constraint(*key))
            .find(|row| {
                row.operands
                    .iter()
                    .filter(|op| op.access == RegisterOperandAccess::Use)
                    .count()
                    == 1
                    && row
                        .operands
                        .iter()
                        .filter(|op| op.access == RegisterOperandAccess::Def)
                        .count()
                        == 1
            })
            .unwrap();
        let arg_view = call_row
            .operands
            .iter()
            .find(|op| op.access == RegisterOperandAccess::Use)
            .unwrap()
            .fixed_view
            .unwrap();
        let ret_view = call_row
            .operands
            .iter()
            .find(|op| op.access == RegisterOperandAccess::Def)
            .unwrap()
            .fixed_view
            .unwrap();

        let caller_arg = VirtualRegisterId(0);
        let caller_ret = VirtualRegisterId(1);
        let call_id = SelectedInstructionId(0);
        let caller = SelectedFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                machine: caller_machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![caller_op],
                    edges: vec![caller_ret_edge, EdgeId::new(101).unwrap()],
                },
                structural: None,
                structural_block_parameters: Vec::new(),
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: vec![SelectedCallContract {
                    instruction: call_id,
                    owner: CallSiteOwner::Operation(caller_op),
                    call: LegalizedScalarCall {
                        source: NativeCallOrigin::Authored,
                        target: NativeCallTarget::Machine(callee_machine),
                        arguments: Vec::new(),
                        result_placement: None,
                        structural_result: None,
                        call_plan: evaluate_call_plan(
                            CallingPolicy::MicrosoftX64,
                            &CallSignature {
                                parameters: Vec::new(),
                                result: None,
                            },
                        )
                        .unwrap(),
                        descriptor_origins: Vec::new(),
                        access_bindings: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    effect: EffectLink {
                        input: 0,
                        output: 1,
                    },
                    ownership: Vec::new(),
                }],
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                virtual_registers: vec![
                    VirtualRegister {
                        id: caller_arg,
                        scalar_type: i64_type(),
                        class: gp_class,
                        origin: VirtualRegisterOrigin::EntryParameter {
                            source_value: ValueId::new(1).unwrap(),
                            parameter_index: 0,
                        },
                        definition_site: None,
                        entry_fixed_view: Some(arg_view),
                    },
                    VirtualRegister {
                        id: caller_ret,
                        scalar_type: i64_type(),
                        class: gp_class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: call_id,
                            source_value: ValueId::new(2).unwrap(),
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    },
                ],
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(caller_b0),
                    instructions: vec![SelectedInstruction {
                        id: call_id,
                        kind: SelectedInstructionKind::CallScalar {
                            target: SelectedCallTarget::Machine(callee_machine),
                        },
                        constraint: call_row.key,
                        operands: vec![
                            SelectedOperand {
                                operand: 0,
                                virtual_register: caller_arg,
                                access: RegisterOperandAccess::Use,
                                class: gp_class,
                                fixed_view: Some(arg_view),
                                tied_to: None,
                                early_clobber: false,
                            },
                            SelectedOperand {
                                operand: 1,
                                virtual_register: caller_ret,
                                access: RegisterOperandAccess::Def,
                                class: gp_class,
                                fixed_view: Some(ret_view),
                                tied_to: None,
                                early_clobber: true,
                            },
                        ],
                        implicit_uses: Vec::new(),
                        implicit_defs: Vec::new(),
                        clobbers: Vec::new(),
                        provenance: SelectedInstructionProvenance {
                            operations: vec![caller_op],
                            ..Default::default()
                        },
                    }],
                    terminator: SelectedTerminator::Jump {
                        instruction: SelectedInstruction {
                            id: SelectedInstructionId(1),
                            kind: SelectedInstructionKind::Jump,
                            constraint: keys.jump,
                            operands: Vec::new(),
                            implicit_uses: Vec::new(),
                            implicit_defs: Vec::new(),
                            clobbers: Vec::new(),
                            provenance: SelectedInstructionProvenance::default(),
                        },
                        successor: SelectedSuccessor {
                            role: SelectedSuccessorRole::Semantic,
                            psi_edge: Some(EdgeId::new(101).unwrap()),
                            block: SelectedBlockId(1),
                            source_target: caller_b1,
                            bindings: Vec::new(),
                            structural_bindings: Vec::new(),
                            structural_case: None,
                            work: Vec::new(),
                        },
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    origin: SelectedBlockOrigin::Source(caller_b1),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: SelectedInstruction {
                            id: SelectedInstructionId(2),
                            kind: SelectedInstructionKind::ReturnScalar,
                            constraint: keys.return_i64,
                            operands: vec![SelectedOperand {
                                operand: 0,
                                virtual_register: caller_ret,
                                access: RegisterOperandAccess::Use,
                                class: gp_class,
                                fixed_view: Some(ret_view),
                                tied_to: None,
                                early_clobber: false,
                            }],
                            implicit_uses: Vec::new(),
                            implicit_defs: Vec::new(),
                            clobbers: Vec::new(),
                            provenance: SelectedInstructionProvenance::default(),
                        },
                        psi_return_edge: caller_ret_edge,
                    },
                },
            ]
            .into(),
        };

        let callee_slot = LocalStorageSlotId::Boundary {
            operation: callee_op,
        };
        let callee_param = VirtualRegisterId(0);
        let callee_addr = VirtualRegisterId(1);
        let callee = SelectedFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                machine: callee_machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![callee_op],
                    edges: vec![callee_ret_edge],
                },
                structural: None,
                structural_block_parameters: Vec::new(),
                local_storage_slots: vec![SelectedLocalStorageSlot {
                    id: callee_slot,
                    byte_size: 8,
                    alignment: 8,
                }],
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                virtual_registers: vec![
                    VirtualRegister {
                        id: callee_param,
                        scalar_type: i64_type(),
                        class: gp_class,
                        origin: VirtualRegisterOrigin::EntryParameter {
                            source_value: ValueId::new(10).unwrap(),
                            parameter_index: 0,
                        },
                        definition_site: None,
                        entry_fixed_view: Some(arg_view),
                    },
                    VirtualRegister {
                        id: callee_addr,
                        scalar_type: i64_type(),
                        class: gp_class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: SelectedInstructionId(0),
                            source_value: ValueId::new(11).unwrap(),
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    },
                ],
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(callee_b0),
                    instructions: vec![SelectedInstruction {
                        id: SelectedInstructionId(0),
                        kind: SelectedInstructionKind::FrameAddress {
                            slot: FrameStorageSlotId::Local(callee_slot),
                            byte_offset: 0,
                        },
                        constraint: keys.copy_i64,
                        operands: vec![SelectedOperand {
                            operand: 0,
                            virtual_register: callee_addr,
                            access: RegisterOperandAccess::Def,
                            class: gp_class,
                            fixed_view: None,
                            tied_to: None,
                            early_clobber: false,
                        }],
                        implicit_uses: Vec::new(),
                        implicit_defs: Vec::new(),
                        clobbers: Vec::new(),
                        provenance: SelectedInstructionProvenance {
                            operations: vec![callee_op],
                            ..Default::default()
                        },
                    }],
                    terminator: SelectedTerminator::Jump {
                        instruction: SelectedInstruction {
                            id: SelectedInstructionId(1),
                            kind: SelectedInstructionKind::Jump,
                            constraint: keys.jump,
                            operands: Vec::new(),
                            implicit_uses: Vec::new(),
                            implicit_defs: Vec::new(),
                            clobbers: Vec::new(),
                            provenance: SelectedInstructionProvenance {
                                edges: vec![EdgeId::new(201).unwrap()],
                                ..Default::default()
                            },
                        },
                        successor: SelectedSuccessor {
                            role: SelectedSuccessorRole::Semantic,
                            psi_edge: Some(EdgeId::new(201).unwrap()),
                            block: SelectedBlockId(1),
                            source_target: BlockId::new(2).unwrap(),
                            bindings: Vec::new(),
                            structural_bindings: Vec::new(),
                            structural_case: None,
                            work: vec![WorkSettlement {
                                site: PsiProvenance::Edge(EdgeId::new(201).unwrap()),
                                units: 1,
                            }],
                        },
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: SelectedInstruction {
                            id: SelectedInstructionId(2),
                            kind: SelectedInstructionKind::ReturnScalar,
                            constraint: keys.return_i64,
                            operands: vec![SelectedOperand {
                                operand: 0,
                                virtual_register: callee_param,
                                access: RegisterOperandAccess::Use,
                                class: gp_class,
                                fixed_view: Some(ret_view),
                                tied_to: None,
                                early_clobber: false,
                            }],
                            implicit_uses: Vec::new(),
                            implicit_defs: Vec::new(),
                            clobbers: Vec::new(),
                            provenance: SelectedInstructionProvenance::default(),
                        },
                        psi_return_edge: callee_ret_edge,
                    },
                },
            ]
            .into(),
        };

        let mut with_binding = [caller.clone(), callee.clone()];
        let SelectedTerminator::Jump { successor, .. } = &mut with_binding[1].blocks[0].terminator
        else {
            unreachable!()
        };
        let binding = selected_instructions::SelectedValueBinding {
            semantic: operation_operands::values::ValueBinding {
                parameter: ValueId::new(3).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: i64_type(),
            },
            transport: selected_instructions::SelectedValueTransport::Unused,
        };
        successor.bindings.push(binding.clone());
        inline_and_simplify(&mut with_binding, &environment).unwrap();
        assert_eq!(with_binding[0].calls.as_slice(), caller.calls.as_slice());
        let SelectedTerminator::Jump { successor, .. } = &with_binding[1].blocks[0].terminator
        else {
            panic!("callee lost its branch")
        };
        assert_eq!(successor.bindings, [binding]);
        assert_eq!(successor.role, SelectedSuccessorRole::Semantic);

        // A call that passes an argument on the stack stays a call: inlining
        // would remove it but leave the outgoing slot, which frame layout
        // refuses in a function without calls.
        let mut stack_passed = [caller.clone(), callee.clone()];
        stack_passed[0].outgoing_arguments.push(
            selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    invocation_site: semantic_vocabulary::InvocationSite::Operation(caller_op),
                    argument_index: 4,
                    role: selected_instructions::OutgoingArgumentSlotRole::ValueCopy,
                },
                byte_size: 48,
                alignment: 16,
                abi_stack_byte_offset: 32,
            },
        );
        inline_and_simplify(&mut stack_passed, &environment).unwrap();
        assert_eq!(stack_passed[0].calls.as_slice(), caller.calls.as_slice());
        assert_eq!(stack_passed[0].outgoing_arguments.len(), 1);

        let mut functions = vec![caller, callee];
        inline_and_simplify(&mut functions, &environment).unwrap();

        let inlined_caller = &functions[0];
        assert!(inlined_caller.ids_are_positions());
        assert!(inlined_caller.calls.is_empty());
        assert_eq!(inlined_caller.blocks.len(), 2);
        assert_eq!(
            inlined_caller.blocks[0].origin,
            SelectedBlockOrigin::Source(caller_b0)
        );
        assert_eq!(
            inlined_caller.blocks[1].origin,
            SelectedBlockOrigin::Source(caller_b1)
        );
        assert_eq!(
            inlined_caller.local_storage_slots[0].id,
            LocalStorageSlotId::Inlined {
                call_site: 0,
                slot_index: 0,
            }
        );
        assert!(
            inlined_caller
                .instructions()
                .all(|inst| !matches!(inst.kind, SelectedInstructionKind::CallScalar { .. }))
        );
        assert!(inlined_caller.instructions().any(|inst| matches!(
            inst.kind,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Inlined {
                    call_site: 0,
                    slot_index: 0,
                }),
                ..
            }
        )));
    }
}
