use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{LivenessIdentity, LivenessPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivenessValidationReceipt {
    pub(crate) identity: LivenessIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) block_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) instruction_count: usize,
    pub(crate) successor_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) early_clobber_count: usize,
}

impl LivenessValidationReceipt {
    pub const fn identity(self) -> LivenessIdentity {
        self.identity
    }

    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
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

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn successor_count(self) -> usize {
        self.successor_count
    }

    pub const fn tied_pair_count(self) -> usize {
        self.tied_pair_count
    }

    pub const fn early_clobber_count(self) -> usize {
        self.early_clobber_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiveness {
    pub(crate) plan: std::sync::Arc<LivenessPlan>,
    pub(crate) receipt: LivenessValidationReceipt,
}

impl ValidatedLiveness {
    pub fn plan(&self) -> &LivenessPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> LivenessValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessError {
    ProjectedStructuralCallReturnUnsupported,
    RootMismatch,
    DuplicateMachine {
        machine: u64,
    },
    StructuralFunctionMismatch {
        function: usize,
    },
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
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: u32,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorMismatch {
        function: usize,
        block: u32,
        ordinal: u8,
    },
    NonCanonicalSet {
        function: usize,
        instruction: Option<u32>,
    },
    NonDensePositions {
        function: usize,
    },
    TransferMismatch {
        function: usize,
        instruction: u32,
    },
    FixedConstraintMismatch {
        function: usize,
    },
}

impl std::fmt::Display for LivenessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal selected liveness failed: {self:?}")
    }
}

impl std::error::Error for LivenessError {}
