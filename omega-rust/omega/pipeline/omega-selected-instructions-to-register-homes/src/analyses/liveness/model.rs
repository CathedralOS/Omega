use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::NativeTarget;
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LivenessPosition(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivenessIdentity(pub(crate) [u8; 32]);

impl LivenessIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<FunctionLiveness>,
    /// Structural-ABI Unit functions retain their own roster even though the
    /// current exact form has no allocator-managed virtual registers. Keeping
    /// this separate prevents a zero-VReg result from erasing function, call,
    /// return, or architectural-unit custody.
    pub structural_unit_functions: Vec<FunctionLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveness {
    pub machine: MachineId,
    pub entry_definitions: Vec<EntryDefinition>,
    pub operand_positions: Vec<OperandPosition>,
    pub blocks: Vec<BlockLiveness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryDefinition {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandPosition {
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
    pub tied_to: Option<u16>,
    pub early_clobber: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLiveness {
    pub block: SelectedBlockId,
    pub source_block: BlockId,
    pub virtual_live_in: Vec<VirtualRegisterId>,
    pub virtual_live_out: Vec<VirtualRegisterId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
    pub instructions: Vec<InstructionLiveness>,
    pub successors: Vec<SuccessorLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionLiveness {
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub virtual_uses: Vec<VirtualRegisterId>,
    pub virtual_defs: Vec<VirtualRegisterId>,
    pub virtual_live_in: Vec<VirtualRegisterId>,
    pub virtual_live_out: Vec<VirtualRegisterId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorLiveness {
    pub terminator: SelectedInstructionId,
    /// Canonical branch polarity: zero is nonzero/true, one is zero/false.
    pub polarity_ordinal: u8,
    pub psi_edge: EdgeId,
    pub target: SelectedBlockId,
    pub virtual_live: Vec<VirtualRegisterId>,
    pub unit_live: Vec<RegisterUnitId>,
}

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
