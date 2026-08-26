use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_target::NativeTarget;
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};

use crate::{TerminalLivenessError, TerminalLivenessIdentity, TerminalLivenessPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalLiveRangePoint(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalLiveRangeIdentity(pub(crate) [u8; 32]);

impl TerminalLiveRangeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLiveRangePlan {
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub liveness: TerminalLivenessIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<TerminalFunctionLiveRanges>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionLiveRanges {
    pub machine: MachineId,
    pub block_domains: Vec<TerminalBlockPointDomain>,
    pub virtual_registers: Vec<TerminalVirtualLiveRange>,
    pub architectural_units: Vec<TerminalArchitecturalUnitLiveRange>,
    pub interference: Vec<TerminalVirtualInterference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBlockPointDomain {
    pub block: TerminalSelectedBlockId,
    pub source_block: BlockId,
    pub start: TerminalLiveRangePoint,
    pub end: TerminalLiveRangePoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalVirtualLiveRange {
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub occurrences: Vec<TerminalVirtualOccurrence>,
    pub fixed_constraints: Vec<TerminalVirtualFixedConstraint>,
    pub fragments: Vec<TerminalLiveRangeFragment>,
    pub edge_connectors: Vec<TerminalLiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalVirtualOccurrence {
    pub position: TerminalLivenessPosition,
    pub point: TerminalLiveRangePoint,
    pub instruction: TerminalSelectedInstructionId,
    pub operand: u16,
    pub access: RegisterOperandAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalVirtualFixedConstraintSite {
    Entry,
    Operand {
        position: TerminalLivenessPosition,
        point: TerminalLiveRangePoint,
        instruction: TerminalSelectedInstructionId,
        operand: u16,
        access: RegisterOperandAccess,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalVirtualFixedConstraint {
    pub site: TerminalVirtualFixedConstraintSite,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLiveRangeFragment {
    pub block: TerminalSelectedBlockId,
    pub start: TerminalLiveRangePoint,
    pub end: TerminalLiveRangePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLiveRangeEdgeConnector {
    pub source: TerminalSelectedBlockId,
    pub terminator: TerminalSelectedInstructionId,
    pub polarity_ordinal: u8,
    pub psi_edge: EdgeId,
    pub target: TerminalSelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalArchitecturalUnitLiveRange {
    pub unit: RegisterUnitId,
    pub actions: Vec<TerminalArchitecturalUnitAction>,
    pub fragments: Vec<TerminalLiveRangeFragment>,
    pub edge_connectors: Vec<TerminalLiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalArchitecturalUnitAction {
    pub block: TerminalSelectedBlockId,
    pub position: TerminalLivenessPosition,
    pub point: TerminalLiveRangePoint,
    pub instruction: TerminalSelectedInstructionId,
    pub kind: TerminalArchitecturalUnitActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalArchitecturalUnitActionKind {
    Use,
    Def,
    Clobber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalVirtualInterference {
    pub lower: TerminalVirtualRegisterId,
    pub higher: TerminalVirtualRegisterId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLiveRangeValidationReceipt {
    pub(crate) identity: TerminalLiveRangeIdentity,
    pub(crate) selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) liveness: TerminalLivenessIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) function_count: usize,
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
}

impl TerminalLiveRangeValidationReceipt {
    pub const fn identity(self) -> TerminalLiveRangeIdentity {
        self.identity
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> TerminalLivenessIdentity {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalLiveRanges {
    pub(crate) plan: TerminalLiveRangePlan,
    pub(crate) receipt: TerminalLiveRangeValidationReceipt,
}

impl ValidatedTerminalLiveRanges {
    pub const fn plan(&self) -> &TerminalLiveRangePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalLiveRangeValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLiveRangeError {
    LivenessRevalidation(TerminalLivenessError),
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
    NonCanonicalRows {
        function: usize,
    },
}

impl std::fmt::Display for TerminalLiveRangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal live-range derivation failed: {self:?}")
    }
}

impl std::error::Error for TerminalLiveRangeError {}
