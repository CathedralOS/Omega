use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{LivenessError, LivenessIdentity, LivenessPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LiveRangePoint(pub u32);

pub use register_homes::LiveRangeIdentity;

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
    pub early_clobbers: Vec<EarlyClobberConstraint>,
    pub architectural_units: Vec<ArchitecturalUnitLiveRange>,
    pub interference: Vec<VirtualInterference>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeValidationReceipt {
    pub(crate) identity: LiveRangeIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) liveness: LivenessIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) block_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) virtual_occurrence_count: usize,
    pub(crate) fixed_constraint_count: usize,
    pub(crate) virtual_fragment_count: usize,
    pub(crate) architectural_unit_count: usize,
    pub(crate) architectural_action_count: usize,
    pub(crate) architectural_fragment_count: usize,
    pub(crate) virtual_edge_connector_count: usize,
    pub(crate) architectural_edge_connector_count: usize,
    pub(crate) interference_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) tied_component_count: usize,
    pub(crate) early_clobber_count: usize,
    pub(crate) early_clobber_use_count: usize,
}

impl LiveRangeValidationReceipt {
    pub const fn identity(self) -> LiveRangeIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn block_count(self) -> usize {
        self.block_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn virtual_fragment_count(self) -> usize {
        self.virtual_fragment_count
    }
    pub const fn virtual_occurrence_count(self) -> usize {
        self.virtual_occurrence_count
    }
    pub const fn fixed_constraint_count(self) -> usize {
        self.fixed_constraint_count
    }
    pub const fn architectural_unit_count(self) -> usize {
        self.architectural_unit_count
    }
    pub const fn architectural_fragment_count(self) -> usize {
        self.architectural_fragment_count
    }
    pub const fn architectural_action_count(self) -> usize {
        self.architectural_action_count
    }
    pub const fn virtual_edge_connector_count(self) -> usize {
        self.virtual_edge_connector_count
    }
    pub const fn architectural_edge_connector_count(self) -> usize {
        self.architectural_edge_connector_count
    }
    pub const fn interference_count(self) -> usize {
        self.interference_count
    }
    pub const fn tied_pair_count(self) -> usize {
        self.tied_pair_count
    }
    pub const fn tied_component_count(self) -> usize {
        self.tied_component_count
    }
    pub const fn early_clobber_count(self) -> usize {
        self.early_clobber_count
    }
    pub const fn early_clobber_use_count(self) -> usize {
        self.early_clobber_use_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiveRanges {
    pub(crate) plan: std::sync::Arc<LiveRangePlan>,
    pub(crate) receipt: LiveRangeValidationReceipt,
}

impl ValidatedLiveRanges {
    pub fn plan(&self) -> &LiveRangePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> LiveRangeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveRangeError {
    LivenessRevalidation(LivenessError),
    LivenessReceiptMismatch,
    RootMismatch,
    UnsupportedUseDef {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedTiedOperand {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedEarlyClobber {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockDomainMismatch {
        function: usize,
        block: u32,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    ArchitecturalUnitMismatch {
        function: usize,
        unit: u16,
    },
    InterferenceMismatch {
        function: usize,
    },
    TiedPairMismatch {
        function: usize,
    },
    EarlyClobberMismatch {
        function: usize,
    },
    NonCanonicalRows {
        function: usize,
    },
}

impl std::fmt::Display for LiveRangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal live-range derivation failed: {self:?}")
    }
}

impl std::error::Error for LiveRangeError {}
