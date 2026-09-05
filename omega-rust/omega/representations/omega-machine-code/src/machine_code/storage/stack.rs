//! Function and call stack usage, native adjustments, and return-link storage.

use crate::{ScalarCleanupPreservationEvidence, ScalarControlFlowEvidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitStackEvidence {
    /// Exact function-lifetime stack allocation and matching release. The
    /// object boundary validates both target encodings before deriving any
    /// numeric stack demand. `None` is valid only for an x86-64 Unit leaf with
    /// no parameter-home frame.
    pub frame: Option<StackAdjustmentPair>,
    /// AArch64 Unit functions retain the incoming link register in their
    /// function-lifetime frame. Both accesses are validated against the exact
    /// encoded instructions; x86-64 uses the implicit CALL/RET stack link.
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitCallStackEvidence {
    /// Exact outgoing argument/shadow allocation and matching release around
    /// this call. The object boundary derives the transient contribution from
    /// these validated target instructions plus architecture-owned linkage.
    pub outbound: Option<StackAdjustmentPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCallStackEvidence {
    pub outbound: Option<StackAdjustmentPair>,
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackAdjustmentPair {
    pub byte_size: u32,
    pub allocation_offset: usize,
    pub allocation_byte_count: usize,
    pub release_offset: usize,
    pub release_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ReturnLinkEvidence {
    pub frame_byte_offset: u32,
    pub store_offset: usize,
    pub load_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStackEvidence {
    pub mutations: Vec<ScalarStackMutation>,
    pub control_flow: ScalarControlFlowEvidence,
    pub stack_alignment: u32,
    /// Exact ABI-result preservation around an appended scalar-return cleanup
    /// suffix. Ordinary scalar functions have no such suffix and retain
    /// `None`; object construction validates every named access independently
    /// from the generic mutation trace.
    pub cleanup_preservation: Option<ScalarCleanupPreservationEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarStackMutation {
    pub offset: usize,
    pub byte_count: usize,
    pub kind: ScalarStackMutationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStackMutationKind {
    Allocate { byte_size: u32 },
    Release { byte_size: u32 },
    X86ReleasePreservingFlags { byte_size: u32 },
    X86Push,
    X86Pop,
}
