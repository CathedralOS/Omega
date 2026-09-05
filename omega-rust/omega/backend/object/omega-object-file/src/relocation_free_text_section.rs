//! Compatibility exports; section data and identity belong to machine-code.
pub use omega_machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState, PlacedBlockSpan,
    PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements, relocation_free_text_section_identity,
};
