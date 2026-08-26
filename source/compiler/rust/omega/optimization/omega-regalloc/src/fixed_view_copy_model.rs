use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionPlan,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::{MachineId, ValueId};

use crate::{
    TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity,
    TerminalVirtualFixedConstraintSite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalFixedViewCopyIdentity(pub(crate) [u8; 32]);

impl TerminalFixedViewCopyIdentity {
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact, deliberately narrow policy for materializing entry-to-fixed-use
/// transitions. This is a stable named transformation, not an allocator mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalFixedViewCopyPolicy {
    LeafLocalBeforeFixedUseV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFixedViewCopyPlan {
    pub source_selected: TerminalSelectedInstructionPlanIdentity,
    pub source_ranges: TerminalLiveRangeIdentity,
    pub source_legality: TerminalAllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub policy: TerminalFixedViewCopyPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub copies: Vec<TerminalFixedViewCopy>,
    pub transformed: TerminalSelectedInstructionPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalFixedViewCopy {
    pub function: u32,
    pub machine: MachineId,
    pub source_virtual_register: TerminalVirtualRegisterId,
    pub source_value: ValueId,
    pub source_definition_site: ValueDefinitionSite,
    pub from_view: RegisterViewId,
    pub destination_site: TerminalVirtualFixedConstraintSite,
    pub to_view: RegisterViewId,
    pub block: TerminalSelectedBlockId,
    pub before_instruction: TerminalSelectedInstructionId,
    pub copy_instruction: TerminalSelectedInstructionId,
    pub result_virtual_register: TerminalVirtualRegisterId,
    pub copy_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalFixedViewCopyValidationReceipt {
    pub(crate) identity: TerminalFixedViewCopyIdentity,
    pub(crate) source_selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) source_ranges: TerminalLiveRangeIdentity,
    pub(crate) source_legality: TerminalAllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) transformed_selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    pub(crate) fuel_schedule: psi_core::FuelScheduleIdentity,
    pub(crate) policy: TerminalFixedViewCopyPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) copy_count: usize,
}

impl TerminalFixedViewCopyValidationReceipt {
    pub const fn identity(self) -> TerminalFixedViewCopyIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> TerminalLiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> TerminalAllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn transformed_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn policy(self) -> TerminalFixedViewCopyPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn copy_count(self) -> usize {
        self.copy_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalFixedViewCopies {
    pub(crate) plan: TerminalFixedViewCopyPlan,
    pub(crate) receipt: TerminalFixedViewCopyValidationReceipt,
}

impl ValidatedTerminalFixedViewCopies {
    pub const fn plan(&self) -> &TerminalFixedViewCopyPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalFixedViewCopyValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalFixedViewCopyError {
    RootMismatch,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    UnsupportedPolicy,
    UnsupportedTransitionSite {
        function: usize,
        register: u32,
    },
    UnsupportedSourceRegister {
        function: usize,
        register: u32,
    },
    MissingDestination {
        function: usize,
        instruction: u32,
    },
    NonLeafDestination {
        function: usize,
        instruction: u32,
    },
    CopyConstraintMismatch,
    IdentifierOverflow {
        function: usize,
    },
    NonCanonicalCopies,
    CopyMismatch {
        index: usize,
    },
    TransformedPlanMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for TerminalFixedViewCopyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal fixed-view copy materialization failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalFixedViewCopyError {}
