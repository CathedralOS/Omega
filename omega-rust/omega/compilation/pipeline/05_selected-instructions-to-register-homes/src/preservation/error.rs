use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterViewId;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocatedCalleeSavedRequirementError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTargetConvention,
    FunctionRosterMismatch,
    HomeRosterMismatch,
    DuplicateHome {
        function: MachineId,
        virtual_register: VirtualRegisterId,
    },
    MissingHome {
        function: MachineId,
        virtual_register: VirtualRegisterId,
    },
    UnknownOrIncompatibleView {
        function: MachineId,
        virtual_register: VirtualRegisterId,
        view: RegisterViewId,
    },
    NonCanonicalRequirements,
    UsageMismatch,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

error_report!(
    AllocatedCalleeSavedRequirementError,
    "allocated callee-saved requirement derivation failed"
);
