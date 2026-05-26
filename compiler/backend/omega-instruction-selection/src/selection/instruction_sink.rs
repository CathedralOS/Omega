use omega_abstract_operations::SelectedInstruction;
use omega_core::arena::{Arena, Handle, HandleSpan};

pub(super) struct SelectedInstructionSink<'arena> {
    instructions: &'arena mut Arena<SelectedInstruction>,
    start: Handle<SelectedInstruction>,
    count: u32,
}

impl<'arena> SelectedInstructionSink<'arena> {
    pub(super) fn new(instructions: &'arena mut Arena<SelectedInstruction>) -> Self {
        Self {
            instructions,
            start: Handle::invalid(),
            count: 0,
        }
    }

    pub(super) fn push(&mut self, instruction: SelectedInstruction) {
        let handle = self.instructions.append(instruction);
        if self.count == 0 {
            self.start = handle;
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("selected instruction span overflow");
    }

    pub(super) const fn len(&self) -> usize {
        self.count as usize
    }

    pub(super) fn pop(&mut self) -> Option<SelectedInstruction> {
        if self.count == 0 {
            return None;
        }

        let last_index = self
            .start
            .arena_index()
            .checked_add(self.count.checked_sub(1)?)
            .expect("selected instruction span overflow");
        let handle = Handle::from_parts(last_index, self.start.generation());
        let instruction = self.instructions.pop_last_appended(handle)?;

        self.count -= 1;
        if self.count == 0 {
            self.start = Handle::invalid();
        }

        Some(instruction)
    }

    pub(super) const fn finish(&self) -> HandleSpan<SelectedInstruction> {
        HandleSpan::from_parts(self.start, self.count)
    }
}
