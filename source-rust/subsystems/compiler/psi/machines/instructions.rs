use crate::*;
use semantic_vocabulary::{MachineId};

impl Psi {
    pub fn blocks(&self) -> impl Iterator<Item = &Block> + '_ {
        self.machines.iter().flat_map(|machine| &machine.blocks)
    }

    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.machines
            .iter()
            .flat_map(|machine| machine.operations())
    }

    pub fn machine(&self, id: MachineId) -> Option<&TerminalMachine> {
        self.machines.iter().find(|machine| machine.id == id)
    }

    /// Whether any block can end the whole process (`ProcessExit`) instead of
    /// returning to its caller. A native host sees such an end only as an exit
    /// status, which a normal return can also produce.
    pub fn can_end_the_process(&self) -> bool {
        self.blocks().any(|block| {
            matches!(
                block.terminator,
                crate::Terminator::ExternalCompletion { .. }
            )
        })
    }

    pub fn unoptimized_machine_behavior(
        &self,
        id: MachineId,
    ) -> Option<&crate::UnoptimizedMachineBehavior> {
        self.execution.unoptimized_behavior
            .iter()
            .find(|body| body.machine == id)
    }
}
