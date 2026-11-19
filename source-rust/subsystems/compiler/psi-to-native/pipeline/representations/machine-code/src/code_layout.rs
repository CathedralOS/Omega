//! Final code attribution carried into object metadata and image placement.

use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCodeRegion {
    pub section_offset: u64, // Byte offset of the attributed span from the start of the text section.
    pub length: u64,         // Non-zero byte length of the coalesced attributed span.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodeLayout {
    pub machine: MachineId, // Specialized machine identity described by this layout.
    pub entry_section_offset: u64, // Section-relative byte offset of the machine's entry point.
    pub regions: Vec<NativeCodeRegion>, // Sorted, non-overlapping attributed instruction regions.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCodeLayoutError {
    UnavailableRealization(MachineId),
    MissingFrameAttribution(MachineId),
    AmbiguousRealization(MachineId),
    InvalidInstructionSpan(MachineId),
    MissingEntryInstruction(MachineId),
}
