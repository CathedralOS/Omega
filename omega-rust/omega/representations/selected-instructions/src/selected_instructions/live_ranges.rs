//! Durable selected-instruction live-range facts and their canonical identity.
//!
//! Raw data carries no analysis admission authority; independent replay belongs
//! to the selected-instruction transformation stage.

mod identity;
pub use identity::live_range_identity;

use crate::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{LivenessIdentity, LivenessPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LiveRangePoint(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiveRangeIdentity(pub(crate) [u8; 32]);

impl LiveRangeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRangePlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub liveness: LivenessIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<FunctionLiveRanges>,
    /// Structural-signature Unit functions remain distinct from the ordinary
    /// VReg roster while retaining their exact architectural live ranges.
    pub structural_unit_functions: Vec<FunctionLiveRanges>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveRanges {
    pub machine: MachineId,
    pub block_domains: Vec<BlockPointDomain>,
    pub virtual_registers: Vec<VirtualLiveRange>,
    pub tied_pairs: Vec<DistinctUseDefTie>,
    /// Explicit same-home edge transfers for the transition-free allocator.
    /// These are not instruction use/def ties and retain the semantic edge.
    pub edge_transfers: Vec<EdgeRegisterTransfer>,
    pub early_clobbers: Vec<EarlyClobberConstraint>,
    pub architectural_units: Vec<ArchitecturalUnitLiveRange>,
    pub interference: Vec<VirtualInterference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRegisterTransfer {
    pub source: SelectedBlockId,
    pub target: SelectedBlockId,
    pub psi_edge: EdgeId,
    pub argument: VirtualRegisterId,
    pub parameter: VirtualRegisterId,
    pub class: RegisterClassId,
}

/// One exact instruction phase where a definition writes at the before point
/// while all listed unrelated virtual inputs must still be readable. A tied
/// source is represented only by [`DistinctUseDefTie`] and is not
/// duplicated in `uses`. This is allocation hazard evidence and does not make
/// the definition semantically live before its ordinary after-point definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberConstraint {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub early_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_class: RegisterClassId,
    pub def_point: LiveRangePoint,
    pub uses: Vec<EarlyClobberUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberUse {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
}

/// One exact same-home requirement between a distinct VReg use and definition.
/// The use is observed at the instruction's before point and the definition at
/// its after point; this is not `UseDef` and does not invent interference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DistinctUseDefTie {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub use_operand: u16,
    pub use_virtual_register: VirtualRegisterId,
    pub use_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_point: LiveRangePoint,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPointDomain {
    pub block: SelectedBlockId,
    pub source_block: BlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLiveRange {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub occurrences: Vec<VirtualOccurrence>,
    pub fixed_constraints: Vec<VirtualFixedConstraint>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualOccurrence {
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub access: RegisterOperandAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFixedConstraintSite {
    Entry,
    Operand {
        position: LivenessPosition,
        point: LiveRangePoint,
        instruction: SelectedInstructionId,
        operand: u16,
        access: RegisterOperandAccess,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualFixedConstraint {
    pub site: VirtualFixedConstraintSite,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeFragment {
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeEdgeConnector {
    pub source: SelectedBlockId,
    pub terminator: SelectedInstructionId,
    pub polarity_ordinal: u8,
    pub psi_edge: EdgeId,
    pub target: SelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitecturalUnitLiveRange {
    pub unit: RegisterUnitId,
    pub actions: Vec<ArchitecturalUnitAction>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchitecturalUnitAction {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub kind: ArchitecturalUnitActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchitecturalUnitActionKind {
    Use,
    Def,
    Clobber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualInterference {
    pub lower: VirtualRegisterId,
    pub higher: VirtualRegisterId,
}
