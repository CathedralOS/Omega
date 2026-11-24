/*
A stack unwinder (Windows exception dispatch, a debugger's stack walk) undoes
a function's frame from the outside: it needs to know how far the prologue has
moved the stack pointer and where it stored each register the function must
preserve. The frame protocol encoder is the only owner of the instruction
boundaries in a prologue or epilogue, so it records one `FrameProtocolStep` per
emitted instruction. Image writers turn those steps into a container's unwind
format (PE `.pdata`/`.xdata`) without decoding the bytes again.
*/

use crate::MachineRegister;

/// What one frame-protocol instruction changes in the state an unwinder restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProtocolEffect {
    /// The instruction moves the stack pointer by `bytes`: down in a prologue,
    /// up in an epilogue.
    AdjustStack { bytes: u32 },
    /// The instruction stores (prologue) or reloads (epilogue) the 8-byte
    /// `register` at `frame_offset_bytes` above the post-prologue stack pointer.
    Register {
        register: MachineRegister,
        frame_offset_bytes: u32,
    },
    /// The instruction leaves the stack pointer and preserved registers as
    /// they were: a stack-commit probe or an address computation.
    Unchanged,
}

/// One instruction of a prologue or epilogue, ending `end_offset` bytes after
/// the start of that prologue or epilogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameProtocolStep {
    pub end_offset: u32,
    pub effect: FrameProtocolEffect,
}

/// The frame a function establishes, positioned inside its placed bytes: the
/// prologue at offset 0 and one epilogue before every return instruction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionFrameUnwind {
    pub prologue: Vec<FrameProtocolStep>,
    pub epilogue: Vec<FrameProtocolStep>,
    /// Function-relative offset of each epilogue's first byte, ascending.
    pub epilogue_offsets: Vec<u32>,
}

impl FunctionFrameUnwind {
    /// A frameless function leaves the stack pointer and every preserved
    /// register untouched, so an unwinder needs no record for it.
    pub fn is_frameless(&self) -> bool {
        self.prologue
            .iter()
            .all(|step| step.effect == FrameProtocolEffect::Unchanged)
    }

    pub fn prologue_byte_count(&self) -> u32 {
        self.prologue.last().map_or(0, |step| step.end_offset)
    }

    pub fn epilogue_byte_count(&self) -> u32 {
        self.epilogue.last().map_or(0, |step| step.end_offset)
    }
}

/// Collects steps while an encoder appends instruction bytes to one buffer.
#[derive(Debug, Default)]
pub struct FrameProtocolSteps {
    steps: Vec<FrameProtocolStep>,
}

impl FrameProtocolSteps {
    /// Record that the bytes appended since the previous step form one
    /// instruction with `effect`. Returns `None` when the buffer outgrows u32.
    pub fn push(&mut self, bytes: &[u8], effect: FrameProtocolEffect) -> Option<()> {
        let end_offset = u32::try_from(bytes.len()).ok()?;
        self.steps.push(FrameProtocolStep { end_offset, effect });
        Some(())
    }

    pub fn into_steps(self) -> Vec<FrameProtocolStep> {
        self.steps
    }
}

/// A frame protocol's bytes with the instruction steps that produced them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncodedFrameProtocol {
    pub prologue: Vec<u8>,
    pub epilogue: Vec<u8>,
    pub prologue_steps: Vec<FrameProtocolStep>,
    pub epilogue_steps: Vec<FrameProtocolStep>,
}
