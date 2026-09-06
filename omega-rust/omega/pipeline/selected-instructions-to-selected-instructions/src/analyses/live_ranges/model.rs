use crate::LivenessError;
use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    LiveRangeIdentity, LiveRangePlan, LivenessIdentity, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

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
