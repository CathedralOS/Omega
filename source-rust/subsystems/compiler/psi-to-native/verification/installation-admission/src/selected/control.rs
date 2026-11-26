use super::Error;
use selected_instructions::{SelectedBlockId, SelectedFunction, SelectedInstructionId};

pub struct Graph {
    positions: Vec<(SelectedInstructionId, (usize, usize))>,
    dominators: Vec<Vec<bool>>,
    reachable: Vec<bool>,
}
impl Graph {
    pub fn new(function: &SelectedFunction) -> Result<Self, Error> {
        let count = function.blocks.len();
        let mut positions = Vec::new();
        let mut successors = vec![Vec::new(); count];
        for (index, block) in function.blocks.iter().enumerate() {
            if block.id.0 as usize != index {
                return Err(Error::Mismatch(
                    "installation replay requires the selected block arena",
                ));
            }
            for (position, row) in block
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                ))
                .enumerate()
            {
                positions.push((row.id, (index, position)));
            }
            for edge in selected_instructions::terminator_successors(&block.terminator) {
                if edge.block.0 as usize >= count {
                    return Err(Error::Mismatch("installation branch leaves its function"));
                }
                successors[index].push(edge.block.0 as usize);
            }
        }
        positions.sort_unstable_by_key(|row| row.0);
        if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::Mismatch(
                "installation has duplicated selected instruction IDs",
            ));
        }
        let entry = function.entry_block.0 as usize;
        if entry >= count {
            return Err(Error::Mismatch("installation function has no entry"));
        }
        let mut reachable = vec![false; count];
        let mut pending = vec![entry];
        while let Some(block) = pending.pop() {
            if reachable[block] {
                continue;
            }
            reachable[block] = true;
            pending.extend(successors[block].iter().copied());
        }
        let mut predecessors = vec![Vec::new(); count];
        for (from, edges) in successors.iter().enumerate() {
            for &to in edges {
                predecessors[to].push(from);
            }
        }
        let mut dominators = vec![reachable.clone(); count];
        dominators[entry].fill(false);
        dominators[entry][entry] = true;
        loop {
            let mut changed = false;
            for block in 0..count {
                if block == entry || !reachable[block] {
                    continue;
                }
                let mut next = reachable.clone();
                for &predecessor in &predecessors[block] {
                    if reachable[predecessor] {
                        for (value, previous) in next.iter_mut().zip(&dominators[predecessor]) {
                            *value &= *previous;
                        }
                    }
                }
                next[block] = true;
                if next != dominators[block] {
                    dominators[block] = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(Self {
            positions,
            dominators,
            reachable,
        })
    }
    pub fn position(&self, id: SelectedInstructionId) -> Result<(usize, usize), Error> {
        self.positions
            .binary_search_by_key(&id, |row| row.0)
            .ok()
            .map(|index| self.positions[index].1)
            .ok_or(Error::Mismatch(
                "installation instruction is outside its CFG",
            ))
    }
    pub fn precedes(
        &self,
        first: SelectedInstructionId,
        last: SelectedInstructionId,
    ) -> Result<bool, Error> {
        let (a, ai) = self.position(first)?;
        let (b, bi) = self.position(last)?;
        if !self.reachable[a] || !self.reachable[b] {
            return Ok(false);
        }
        Ok(if a == b {
            ai < bi
        } else {
            self.dominators[b][a]
        })
    }

    pub fn block_dominates(
        &self,
        block: SelectedBlockId,
        instruction: SelectedInstructionId,
    ) -> Result<bool, Error> {
        let source = block.0 as usize;
        let (destination, _) = self.position(instruction)?;
        Ok(self.reachable.get(source).copied().unwrap_or(false)
            && self.reachable[destination]
            && self.dominators[destination][source])
    }
}
