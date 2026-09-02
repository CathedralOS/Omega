use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::RegisterViewId;
use omega_selected_instructions::VirtualRegisterId;
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocatedCalleeSavedRequirementError {
    Upstream(crate::OptimizedRegisterHomeCustodyError),
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

impl std::fmt::Display for AllocatedCalleeSavedRequirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "allocated callee-saved requirement derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AllocatedCalleeSavedRequirementError {}
