use omega_core::arena::HandleSpan;

/// Relocation-free checked-assembly instructions whose complete final encoding
/// and architectural footprint are closed in the compiler catalog.
///
/// This tag is retained beside the encoded byte span so final-image validation
/// never has to rediscover instruction boundaries by scanning arbitrary bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedCheckedInstructionKind {
    MachineHalt,
    LoadFence,
    StoreFence,
    FullFence,
    InterruptDisable,
    InterruptEnable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
    pub fixed_checked_kind: Option<FixedCheckedInstructionKind>,
}
