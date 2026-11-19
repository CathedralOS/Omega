use selected_instructions::{
    BlockSlotEvent, FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind as Kind,
};

mod flow {
    use selected_instructions::SelectedInstructionId;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Writer {
        Unreachable,
        Missing,
        Store(SelectedInstructionId),
        Different,
    }
    impl Writer {
        fn merge(self, incoming: Self) -> Self {
            if self == incoming || incoming == Self::Unreachable {
                self
            } else if self == Self::Unreachable {
                incoming
            } else {
                Self::Different
            }
        }
    }
    pub fn entries(
        entry: usize,
        last: &[Option<SelectedInstructionId>],
        successors: &[Vec<usize>],
    ) -> Vec<Writer> {
        let mut entries = vec![Writer::Unreachable; successors.len()];
        entries[entry] = Writer::Missing;
        let mut pending = std::collections::VecDeque::from([entry]);
        let mut queued = vec![false; entries.len()];
        queued[entry] = true;
        while let Some(block) = pending.pop_front() {
            queued[block] = false;
            let outgoing = last[block].map_or(entries[block], Writer::Store);
            for &next in &successors[block] {
                let merged = entries[next].merge(outgoing);
                if merged != entries[next] {
                    entries[next] = merged;
                    if !queued[next] {
                        queued[next] = true;
                        pending.push_back(next);
                    }
                }
            }
        }
        entries
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        const A: SelectedInstructionId = SelectedInstructionId(10);
        const B: SelectedInstructionId = SelectedInstructionId(20);
        #[test]
        fn initialized_word_reaches_straight_line_and_loop_reads() {
            let result = entries(
                0,
                &[Some(A), None, None, None],
                &[vec![1], vec![2, 3], vec![1], vec![]],
            );
            assert_eq!(
                result,
                vec![
                    Writer::Missing,
                    Writer::Store(A),
                    Writer::Store(A),
                    Writer::Store(A)
                ]
            );
        }
        #[test]
        fn a_branch_without_initialization_does_not_inherit_its_sibling_store() {
            let result = entries(
                0,
                &[None, Some(A), None, None],
                &[vec![1, 2], vec![3], vec![3], vec![]],
            );
            assert_eq!(result[3], Writer::Different);
        }
        #[test]
        fn different_branch_writers_remain_distinct() {
            let result = entries(
                0,
                &[None, Some(A), Some(B), None],
                &[vec![1, 2], vec![3], vec![3], vec![]],
            );
            assert_eq!(result[3], Writer::Different);
        }
        #[test]
        fn a_backedge_store_does_not_initialize_the_first_iteration() {
            let result = entries(0, &[None, None, Some(A)], &[vec![1], vec![2], vec![1]]);
            assert_eq!(result[1], Writer::Different);
        }
        #[test]
        fn unreachable_stores_do_not_publish_values() {
            let result = entries(0, &[None, Some(A), None], &[vec![], vec![2], vec![]]);
            assert_eq!(result[1], Writer::Unreachable);
            assert_eq!(result[2], Writer::Unreachable);
        }
        #[test]
        fn reentered_definition_cannot_bypass_its_store_before_a_reload() {
            let result = entries(
                0,
                &[None, None, Some(A), None],
                &[vec![1], vec![2, 3], vec![1], vec![]],
            );
            assert_eq!(result[3], Writer::Different);
        }
    }
}

use flow::Writer;
struct Slot {
    valid: bool,
    stores: Vec<Vec<(usize, SelectedInstructionId)>>,
    entries: Option<Vec<Writer>>,
}
pub struct SpillValues<'a> {
    function: &'a SelectedFunction,
    slots: Vec<Option<Slot>>,
    instructions: Vec<(SelectedInstructionId, usize, usize, &'a SelectedInstruction)>,
    successors: Vec<Vec<usize>>,
}
impl<'a> SpillValues<'a> {
    pub fn new(function: &'a SelectedFunction) -> Result<Self, &'static str> {
        let mut slots: Vec<Option<Slot>> = (0..function.virtual_registers.len())
            .map(|_| None)
            .collect();
        for home in &function.local_storage_slots {
            let LocalStorageSlotId::Spill { register } = home.id else {
                continue;
            };
            let entry = slots
                .get_mut(register.0 as usize)
                .ok_or("spill slot names a missing register")?;
            if entry.is_some() {
                return Err("spill slot has multiple allocations");
            }
            *entry = Some(Slot {
                valid: home.byte_size == 8 && home.alignment >= 8,
                stores: vec![Vec::new(); function.blocks.len()],
                entries: None,
            });
        }
        let mut instructions = Vec::new();
        let mut successors = vec![Vec::new(); function.blocks.len()];
        let mut address_slots = vec![None; function.virtual_registers.len()];
        let mut definition_counts = vec![0usize; function.virtual_registers.len()];
        let mut non_load_bits: Vec<u64> = Vec::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            if block.id.0 as usize != block_index {
                return Err("spill flow has noncanonical blocks");
            }
            for (position, row) in block
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                ))
                .enumerate()
            {
                instructions.push((row.id, block_index, position, row));
                for operand in &row.operands {
                    if operand.access != register_model::RegisterOperandAccess::Use {
                        let count = definition_counts
                            .get_mut(operand.virtual_register.0 as usize)
                            .ok_or("spill instruction defines a missing register")?;
                        *count += 1;
                    }
                }
                if let Kind::SaveFloatingControl { slot } | Kind::RestoreFloatingControl { slot } =
                    row.kind
                {
                    invalidate(&mut slots, slot);
                }
            }
            for edge in selected_instructions::terminator_successors(&block.terminator) {
                if edge.block.0 as usize >= function.blocks.len() {
                    return Err("spill edge leaves its function");
                }
                successors[block_index].push(edge.block.0 as usize);
            }
            let summary = function
                .blocks
                .block_slot_summary(block_index)
                .ok_or("spill block has no access roster")?;
            non_load_bits.resize(
                non_load_bits
                    .len()
                    .max(summary.non_load_register_bits().len()),
                0,
            );
            for (all, block) in non_load_bits
                .iter_mut()
                .zip(summary.non_load_register_bits())
            {
                *all |= block;
            }
            for event in &summary.events {
                match *event {
                    BlockSlotEvent::Invalidate { slot } => invalidate(&mut slots, slot),
                    BlockSlotEvent::Store {
                        slot: LocalStorageSlotId::Spill { register },
                        index,
                    } => {
                        if let Some(slot) =
                            slots.get_mut(register.0 as usize).and_then(Option::as_mut)
                        {
                            let row = block
                                .instructions
                                .get(index)
                                .ok_or("spill store is outside its block")?;
                            if row
                                .operands
                                .iter()
                                .filter(|operand| operand.operand == 0)
                                .count()
                                != 1
                                || row.operands.iter().any(|operand| {
                                    operand.access
                                        != if operand.operand == 0 {
                                            register_model::RegisterOperandAccess::Use
                                        } else {
                                            register_model::RegisterOperandAccess::Def
                                        }
                                })
                            {
                                slot.valid = false;
                            }
                            slot.stores[block_index].push((index, row.id));
                        }
                    }
                    BlockSlotEvent::FrameAddress {
                        slot: LocalStorageSlotId::Spill { register: slot },
                        register,
                    } => {
                        let owner = address_slots
                            .get_mut(register.0 as usize)
                            .ok_or("spill address register is missing")?;
                        if let Some(prior) = owner.replace(slot) {
                            invalidate(&mut slots, LocalStorageSlotId::Spill { register: prior });
                            invalidate(&mut slots, LocalStorageSlotId::Spill { register: slot });
                        }
                    }
                    _ => {}
                }
            }
        }
        for (register, owner) in address_slots.into_iter().enumerate() {
            let Some(slot) = owner else { continue };
            if definition_counts[register] != 1
                || non_load_bits
                    .get(register / 64)
                    .is_some_and(|bits| bits & (1u64 << (register % 64)) != 0)
            {
                invalidate(&mut slots, LocalStorageSlotId::Spill { register: slot });
            }
        }
        instructions.sort_unstable_by_key(|row| row.0);
        if instructions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("spill flow has duplicate instruction identities");
        }
        Ok(Self {
            function,
            slots,
            instructions,
            successors,
        })
    }
    pub fn private_store(&self, row: &SelectedInstruction) -> bool {
        match row.kind {
            Kind::Store64 {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill { register }),
                byte_offset: 0,
            } => self
                .slots
                .get(register.0 as usize)
                .and_then(Option::as_ref)
                .is_some_and(|slot| slot.valid),
            _ => false,
        }
    }
    pub fn private_address(&self, row: &SelectedInstruction) -> bool {
        match row.kind {
            Kind::FrameAddress {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill { register }),
                byte_offset: 0,
            } => self
                .slots
                .get(register.0 as usize)
                .and_then(Option::as_ref)
                .is_some_and(|slot| slot.valid),
            _ => false,
        }
    }
    pub fn writer(
        &mut self,
        load: SelectedInstructionId,
        home: LocalStorageSlotId,
    ) -> Result<&'a SelectedInstruction, &'static str> {
        let LocalStorageSlotId::Spill { register } = home else {
            return Err("word reload is not a private spill");
        };
        let at = self
            .instructions
            .binary_search_by_key(&load, |row| row.0)
            .map_err(|_| "spill reload is outside its function")?;
        let (_, block, position, row) = self.instructions[at];
        if row.kind != (Kind::Load64 { byte_offset: 0 }) {
            return Err("spill reload changes its width or offset");
        }
        let slot = self
            .slots
            .get_mut(register.0 as usize)
            .and_then(Option::as_mut)
            .filter(|slot| slot.valid)
            .ok_or("spill storage escapes or has incompatible accesses")?;
        let entry = self.function.entry_block.0 as usize;
        if entry >= self.function.blocks.len() {
            return Err("spill flow has no entry block");
        }
        let entries = slot.entries.get_or_insert_with(|| {
            let last = slot
                .stores
                .iter()
                .map(|stores| stores.last().map(|row| row.1))
                .collect::<Vec<_>>();
            flow::entries(entry, &last, &self.successors)
        });
        if entries[block] == Writer::Unreachable {
            return Err("spill reload is unreachable from function entry");
        }
        let stores = &slot.stores[block];
        let before = stores.partition_point(|row| row.0 < position);
        let reaching = before
            .checked_sub(1)
            .map_or(entries[block], |index| Writer::Store(stores[index].1));
        let Writer::Store(store) = reaching else {
            return Err("spill reload has no single initialized writer on every path");
        };
        let index = self
            .instructions
            .binary_search_by_key(&store, |row| row.0)
            .map_err(|_| "spill writer is outside its function")?;
        Ok(self.instructions[index].3)
    }
}
fn invalidate(slots: &mut [Option<Slot>], home: LocalStorageSlotId) {
    if let LocalStorageSlotId::Spill { register } = home
        && let Some(slot) = slots.get_mut(register.0 as usize).and_then(Option::as_mut)
    {
        slot.valid = false;
    }
}
