//! Acknowledgements belong to source calls, while suspension crossings belong
//! to execution. A redundant marker creates no pause. Tail-call frame reuse
//! must retain the called interface even when execution uses a jump.

use semantic_vocabulary::{EdgeId, MachineId, OperationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallSite {
    Operation(OperationId),
    Edge(EdgeId),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallOrigin {
    #[default]
    Authored,
    CompilerGenerated,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallAcknowledgement {
    pub origin: CallOrigin,
    pub acknowledges_suspend: bool,
    pub acknowledges_block: bool,
    /// The original call occupies a position in which suspension is legal.
    /// Lowering arguments to separate operations does not create that position.
    pub suspension_position: bool,
}

impl CallAcknowledgement {
    /// An internal call acknowledges the complete envelope. Its caller's
    /// contract, access and suspension obligations remain independently checked.
    pub const fn generated() -> Self {
        Self {
            origin: CallOrigin::CompilerGenerated,
            acknowledges_suspend: true,
            acknowledges_block: true,
            suspension_position: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallFormation {
    pub site: CallSite,
    pub acknowledgement: CallAcknowledgement,
    /// Only a call lowered to a jump needs to name its otherwise erased callee.
    pub tail_callee: Option<MachineId>,
}
