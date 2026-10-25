//! Connected physical-home identities, shared by placement and pressure recovery.
//!
//! A tie or live edge transfer joins disjoint fragments into one home. Pressure
//! must follow that relation at both ends: a retired edge reload may interfere
//! here while its still-rewritable destination lives in another block. This
//! partition describes identity only; callers retain class, interference and
//! rewrite admission. Registers absent from the relations are singletons.

use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

pub(super) struct SameHomeComponents {
    registers: Vec<VirtualRegisterId>,
    roots: Vec<usize>,
    leader_by_id: Vec<u32>,
    member_offsets: Vec<u32>,
    member_targets: Vec<VirtualRegisterId>,
}

impl SameHomeComponents {
    pub(super) fn from_ranges(ranges: &FunctionLiveRanges) -> Self {
        let relations = || {
            ranges
                .tied_pairs
                .iter()
                .map(|tie| (tie.use_virtual_register, tie.def_virtual_register))
                .chain(
                    ranges
                        .edge_transfers
                        .iter()
                        .map(|edge| (edge.argument, edge.parameter)),
                )
        };
        let mut registers: Vec<_> = relations()
            .flat_map(|(left, right)| [left, right])
            .collect();
        registers.sort_unstable();
        registers.dedup();
        let mut roots: Vec<_> = (0..registers.len()).collect();
        for (left, right) in relations() {
            // Every endpoint was collected above, without filtering.
            let left = registers.partition_point(|register| *register < left);
            let right = registers.partition_point(|register| *register < right);
            let left = root(&roots, left);
            let right = root(&roots, right);
            roots[right] = left;
        }
        let max_id = registers.last().map_or(0, |reg| reg.0 as usize + 1);
        let mut leader_by_id = vec![u32::MAX; max_id];
        let mut counts = vec![0u32; registers.len()];
        for position in 0..roots.len() {
            let r = root(&roots, position);
            roots[position] = r;
            leader_by_id[registers[position].0 as usize] = registers[r].0;
            counts[r] += 1;
        }
        let mut member_offsets = vec![0u32; registers.len() + 1];
        for i in 0..registers.len() {
            member_offsets[i + 1] = member_offsets[i] + counts[i];
        }
        let mut cursors = member_offsets.clone();
        let mut member_targets = vec![VirtualRegisterId(0); registers.len()];
        for (position, &reg) in registers.iter().enumerate() {
            let r = roots[position];
            let cursor = &mut cursors[r];
            member_targets[*cursor as usize] = reg;
            *cursor += 1;
        }
        Self {
            registers,
            roots,
            leader_by_id,
            member_offsets,
            member_targets,
        }
    }

    #[inline]
    pub(super) fn leader(&self, register: VirtualRegisterId) -> VirtualRegisterId {
        self.leader_by_id
            .get(register.0 as usize)
            .copied()
            .filter(|&leader| leader != u32::MAX)
            .map_or(register, VirtualRegisterId)
    }

    pub(super) fn members(&self, register: VirtualRegisterId) -> Vec<VirtualRegisterId> {
        let Ok(position) = self.registers.binary_search(&register) else {
            return vec![register];
        };
        let r = self.roots[position];
        let start = self.member_offsets[r] as usize;
        let end = self.member_offsets[r + 1] as usize;
        self.member_targets[start..end].to_vec()
    }

    #[inline]
    pub(super) fn visit_members(
        &self,
        register: VirtualRegisterId,
        mut f: impl FnMut(VirtualRegisterId),
    ) {
        let Ok(position) = self.registers.binary_search(&register) else {
            f(register);
            return;
        };
        let r = self.roots[position];
        let start = self.member_offsets[r] as usize;
        let end = self.member_offsets[r + 1] as usize;
        for &member in &self.member_targets[start..end] {
            f(member);
        }
    }
}

fn root(parents: &[usize], mut position: usize) -> usize {
    while parents[position] != position {
        position = parents[position];
    }
    position
}
