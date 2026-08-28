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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalLivenessPosition(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalLivenessIdentity(pub(crate) [u8; 32]);

impl TerminalLivenessIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLivenessPlan {
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<TerminalFunctionLiveness>,
    /// Structural-ABI Unit functions retain their own roster even though the
    /// current exact form has no allocator-managed virtual registers. Keeping
    /// this separate prevents a zero-VReg result from erasing function, call,
    /// return, or architectural-unit custody.
    pub structural_unit_functions: Vec<TerminalFunctionLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionLiveness {
    pub machine: MachineId,
    pub entry_definitions: Vec<TerminalEntryDefinition>,
    pub operand_positions: Vec<TerminalOperandPosition>,
    pub blocks: Vec<TerminalBlockLiveness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEntryDefinition {
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalOperandPosition {
    pub position: TerminalLivenessPosition,
    pub instruction: TerminalSelectedInstructionId,
    pub operand: u16,
    pub virtual_register: TerminalVirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
    pub tied_to: Option<u16>,
    pub early_clobber: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBlockLiveness {
    pub block: TerminalSelectedBlockId,
    pub source_block: BlockId,
    pub virtual_live_in: Vec<TerminalVirtualRegisterId>,
    pub virtual_live_out: Vec<TerminalVirtualRegisterId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
    pub instructions: Vec<TerminalInstructionLiveness>,
    pub successors: Vec<TerminalSuccessorLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInstructionLiveness {
    pub position: TerminalLivenessPosition,
    pub instruction: TerminalSelectedInstructionId,
    pub virtual_uses: Vec<TerminalVirtualRegisterId>,
    pub virtual_defs: Vec<TerminalVirtualRegisterId>,
    pub virtual_live_in: Vec<TerminalVirtualRegisterId>,
    pub virtual_live_out: Vec<TerminalVirtualRegisterId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSuccessorLiveness {
    pub terminator: TerminalSelectedInstructionId,
    /// Canonical branch polarity: zero is nonzero/true, one is zero/false.
    pub polarity_ordinal: u8,
    pub psi_edge: EdgeId,
    pub target: TerminalSelectedBlockId,
    pub virtual_live: Vec<TerminalVirtualRegisterId>,
    pub unit_live: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLivenessValidationReceipt {
    pub(crate) identity: TerminalLivenessIdentity,
    pub(crate) selected: TerminalSelectedInstructionPlanIdentity,
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

impl TerminalLivenessValidationReceipt {
    pub const fn identity(self) -> TerminalLivenessIdentity {
        self.identity
    }

    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
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
pub struct ValidatedTerminalLiveness {
    pub(crate) plan: TerminalLivenessPlan,
    pub(crate) receipt: TerminalLivenessValidationReceipt,
}

impl ValidatedTerminalLiveness {
    pub const fn plan(&self) -> &TerminalLivenessPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalLivenessValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLivenessError {
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

impl std::fmt::Display for TerminalLivenessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal selected liveness failed: {self:?}")
    }
}

impl std::error::Error for TerminalLivenessError {}
