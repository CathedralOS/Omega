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
        for position in 0..roots.len() {
            let r = root(&roots, position);
            roots[position] = r;
            leader_by_id[registers[position].0 as usize] = registers[r].0;
        }
        Self {
            registers,
            roots,
            leader_by_id,
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
        self.registers
            .iter()
            .copied()
            .zip(&self.roots)
            .filter_map(|(member, root)| (*root == self.roots[position]).then_some(member))
            .collect()
    }
}

fn root(parents: &[usize], mut position: usize) -> usize {
    while parents[position] != position {
        position = parents[position];
    }
    position
}
