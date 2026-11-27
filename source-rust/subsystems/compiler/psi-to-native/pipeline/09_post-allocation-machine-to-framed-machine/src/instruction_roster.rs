//! Find a function's selected instructions by id without rescanning its blocks.
//!
//! Frame planning and entry planning match every allocated row, call record and
//! foreign call back to the selected instruction it came from. Scanning every
//! block for each of those lookups made the checks quadratic in function size.
//! The roster lists the function's block bodies and terminators once, sorted by
//! instruction id, and answers each lookup by binary search. Rows that share an
//! id keep block order, so the first match is the row a front-to-back scan of
//! `instructions_and_terminators()` would have found.

use selected_instructions::{
    SelectedBlock, SelectedInstruction, SelectedInstructionId, terminator_instruction,
};

#[derive(Clone, Copy)]
struct RosterRow<'function> {
    block: &'function SelectedBlock,
    instruction: &'function SelectedInstruction,
    terminator: bool,
}

#[derive(Default)]
pub struct SelectedInstructionRoster<'function> {
    rows: Vec<RosterRow<'function>>,
}

impl<'function> SelectedInstructionRoster<'function> {
    pub fn new(blocks: impl IntoIterator<Item = &'function SelectedBlock>) -> Self {
        let mut rows = Vec::new();
        for block in blocks {
            rows.reserve(block.instructions.len() + 1);
            rows.extend(block.instructions.iter().map(|instruction| RosterRow {
                block,
                instruction,
                terminator: false,
            }));
            rows.push(RosterRow {
                block,
                instruction: terminator_instruction(&block.terminator),
                terminator: true,
            });
        }
        // `sort_by_key` is stable: rows with one id stay in block order.
        rows.sort_by_key(|row| row.instruction.id);
        Self { rows }
    }

    fn rows(&self, id: SelectedInstructionId) -> &[RosterRow<'function>] {
        let start = self.rows.partition_point(|row| row.instruction.id < id);
        let length = self.rows[start..].partition_point(|row| row.instruction.id == id);
        &self.rows[start..start + length]
    }

    /// The first body instruction or terminator with this id.
    pub fn instruction_or_terminator(
        &self,
        id: SelectedInstructionId,
    ) -> Option<&'function SelectedInstruction> {
        self.rows(id).first().map(|row| row.instruction)
    }

    /// The first body instruction with this id; terminators do not match.
    pub fn body_instruction(
        &self,
        id: SelectedInstructionId,
    ) -> Option<&'function SelectedInstruction> {
        self.rows(id)
            .iter()
            .find(|row| !row.terminator)
            .map(|row| row.instruction)
    }

    /// The only body instruction or terminator with this id and its block.
    /// An absent or repeated id has no unique owner and returns `None`.
    pub fn unique(
        &self,
        id: SelectedInstructionId,
    ) -> Option<(&'function SelectedBlock, &'function SelectedInstruction)> {
        match self.rows(id) {
            [row] => Some((row.block, row.instruction)),
            _ => None,
        }
    }

    /// The only body instruction with this id and its block; terminators are
    /// ignored. An absent or repeated id returns `None`.
    pub fn unique_body_instruction(
        &self,
        id: SelectedInstructionId,
    ) -> Option<(&'function SelectedBlock, &'function SelectedInstruction)> {
        let mut body = self.rows(id).iter().filter(|row| !row.terminator);
        match (body.next(), body.next()) {
            (Some(row), None) => Some((row.block, row.instruction)),
            _ => None,
        }
    }

    pub fn contains(&self, id: SelectedInstructionId) -> bool {
        !self.rows(id).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::SelectedInstructionRoster;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedCallTarget,
        SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId};

    fn instruction(id: u32, callee: u32) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(id),
            kind: SelectedInstructionKind::InvokeNoNormal {
                target: SelectedCallTarget::Machine(MachineId::new(u64::from(callee)).unwrap()),
            },
            constraint: register_model::RegisterConstraintKey::instruction(0),
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: Default::default(),
        }
    }

    fn block(id: u32, body: Vec<SelectedInstruction>, terminator: u32) -> SelectedBlock {
        SelectedBlock {
            id: SelectedBlockId(id),
            origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(id) + 1).unwrap()),
            instructions: body,
            terminator: SelectedTerminator::InvokeNoNormal {
                instruction: instruction(terminator, 90 + id),
                psi_edge: EdgeId::new(u64::from(id) + 1).unwrap(),
            },
        }
    }

    fn callee(instruction: &SelectedInstruction) -> Option<MachineId> {
        instruction.kind.machine_callee()
    }

    #[test]
    fn lookups_match_a_front_to_back_scan_of_bodies_and_terminators() {
        // Id 4 is a body instruction in block 1 and the terminator of block 0,
        // so block order puts the terminator first.
        let blocks = [
            block(0, vec![instruction(7, 1)], 4),
            block(1, vec![instruction(4, 2), instruction(2, 3)], 9),
        ];
        let roster = SelectedInstructionRoster::new(&blocks);

        let first = roster.instruction_or_terminator(SelectedInstructionId(4));
        assert_eq!(first.and_then(callee), MachineId::new(90));
        let body = roster.body_instruction(SelectedInstructionId(4));
        assert_eq!(body.and_then(callee), MachineId::new(2));
        assert!(roster.unique(SelectedInstructionId(4)).is_none());
        let (owner, row) = roster
            .unique_body_instruction(SelectedInstructionId(4))
            .unwrap();
        assert_eq!(
            (owner.id, callee(row)),
            (SelectedBlockId(1), MachineId::new(2))
        );

        let (owner, row) = roster.unique(SelectedInstructionId(9)).unwrap();
        assert_eq!(
            (owner.id, callee(row)),
            (SelectedBlockId(1), MachineId::new(91))
        );
        assert!(roster.body_instruction(SelectedInstructionId(9)).is_none());
        assert!(
            roster
                .unique_body_instruction(SelectedInstructionId(9))
                .is_none()
        );

        assert!(roster.contains(SelectedInstructionId(2)));
        assert!(!roster.contains(SelectedInstructionId(3)));
        assert!(
            roster
                .instruction_or_terminator(SelectedInstructionId(8))
                .is_none()
        );
        assert!(!SelectedInstructionRoster::default().contains(SelectedInstructionId(7)));
    }
}
