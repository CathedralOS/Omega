use omega_legalized_operations::LegalizedOperationPlanIdentity;
use omega_optimization_core::OptimizationValidatorIdentity;
use omega_register_model::RegisterConstraintKey;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSelectedInstructions {
    pub(super) plan: SelectedInstructionPlan,
    pub(super) receipt: SelectedInstructionValidationReceipt,
}

impl ValidatedSelectedInstructions {
    pub const fn plan(&self) -> &SelectedInstructionPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> SelectedInstructionValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedInstructionValidationReceipt {
    pub(super) identity: SelectedInstructionPlanIdentity,
    pub(super) legalized: LegalizedOperationPlanIdentity,
    pub(super) legalization_validator: OptimizationValidatorIdentity,
    pub(super) optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    pub(super) fuel_schedule: psi_core::FuelScheduleIdentity,
    pub(super) function_count: usize,
    pub(super) block_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) instruction_count: usize,
    pub(super) projected_structural_call_return_count: usize,
}

impl SelectedInstructionValidationReceipt {
    pub const fn identity(self) -> SelectedInstructionPlanIdentity {
        self.identity
    }

    pub const fn legalized(self) -> LegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
    }

    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
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

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn projected_structural_call_return_count(self) -> usize {
        self.projected_structural_call_return_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionError {
    UnsupportedProjectedStructuralShape,
    ProjectedStructuralRosterMismatch,
    ProjectedStructuralCustodyMismatch,
    ProjectedStructuralConstraintMismatch {
        site: omega_selected_instructions::SelectedStructuralFragmentSite,
    },
    MissingProjectedStructuralCallConstraint,
    ProjectedStructuralCatalogMismatch,
    SourceCustodyMismatch,
    TargetRegisterArchitectureMismatch,
    UnsupportedSourceShape {
        function: usize,
    },
    AmbiguousSourceShape {
        function: usize,
        first: &'static str,
        second: &'static str,
    },
    UnsupportedIntegerShape {
        function: usize,
    },
    UnsupportedCondition {
        function: usize,
    },
    MissingConstantDefinition {
        function: usize,
        arm_edge: psi_core::EdgeId,
    },
    MissingFuelProvenance {
        function: usize,
    },
    MissingConstraint(RegisterConstraintKey),
    MissingInputRegisterView {
        function: usize,
    },
    NonCanonicalVirtualRegisters {
        function: usize,
    },
    NonCanonicalBlocks {
        function: usize,
    },
    NonCanonicalInstructions {
        function: usize,
    },
    FunctionProjectionMismatch {
        function: usize,
    },
    VirtualRegisterProjectionMismatch {
        function: usize,
        register: u32,
    },
    BlockProjectionMismatch {
        function: usize,
        block: u32,
    },
    InstructionProjectionMismatch {
        function: usize,
        instruction: u32,
    },
    ConstraintOperandMismatch {
        function: usize,
        instruction: u32,
    },
    ConstraintEffectMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorProjectionMismatch {
        function: usize,
        block: u32,
    },
    UseBeforeDefinition {
        function: usize,
        instruction: u32,
        register: u32,
    },
    MultipleDefinitions {
        function: usize,
        register: u32,
    },
    ProvenancePartitionMismatch {
        function: usize,
    },
}

impl std::fmt::Display for SelectedInstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal instruction selection failed: {self:?}")
    }
}

impl std::error::Error for SelectedInstructionError {}
