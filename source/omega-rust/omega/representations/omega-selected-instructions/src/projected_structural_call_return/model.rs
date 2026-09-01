use omega_calling_conventions::ValuePlacement;
use omega_legalized_operations::LegalizedOperationPlanIdentity;
use omega_register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
};
use omega_target_operations::MachineRegister;
use psi_core::MachineId;
use psi_terminal::StructuralPathQualification;

/// The first selected projected-qualification family. It retains ABI
/// constraints but creates no machine instruction or allocator-managed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedProjectedStructuralCallReturnRecipe {
    OwnedLinearIntegerFragmentV1,
}

/// Canonical semantic/ABI occurrence order for the exact two-function closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectedStructuralFragmentSite {
    CallerParameter,
    CallerArgumentSource,
    CallerArgumentDestination,
    CallerOperationResult,
    CallerFunctionResult,
    CalleeParameter,
    CalleeReturnSource,
    CalleeFunctionResult,
}

impl SelectedStructuralFragmentSite {
    pub const ALL: [Self; 8] = [
        Self::CallerParameter,
        Self::CallerArgumentSource,
        Self::CallerArgumentDestination,
        Self::CallerOperationResult,
        Self::CallerFunctionResult,
        Self::CalleeParameter,
        Self::CalleeReturnSource,
        Self::CalleeFunctionResult,
    ];
}

/// One target placement constrained to the exact direct integer fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralFragmentConstraint {
    pub site: SelectedStructuralFragmentSite,
    pub placement: ValuePlacement,
}

/// One fixed explicit operand retained from a target register-constraint row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedStructuralFixedOperand {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: RegisterViewId,
}

/// One operand of a structural copy row plus the concrete view selected for
/// this closure. The row may itself be allocation-flexible (`fixed_view=None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedStructuralCopyOperand {
    pub operand: u16,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub row_fixed_view: Option<RegisterViewId>,
    pub selected_view: RegisterViewId,
    pub tied_to: Option<u16>,
    pub early_clobber: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCopyConstraint {
    pub key: RegisterConstraintKey,
    pub source: SelectedStructuralCopyOperand,
    pub destination: SelectedStructuralCopyOperand,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

/// Full target register effects for the selected structural call row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralCallConstraint {
    pub key: RegisterConstraintKey,
    pub argument: SelectedStructuralFixedOperand,
    pub result: SelectedStructuralFixedOperand,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

/// Full target register effects for one selected structural return row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralReturnConstraint {
    pub key: RegisterConstraintKey,
    pub value: SelectedStructuralFixedOperand,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
}

/// Exact target-dependent transfer needed inside the callee before return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedStructuralTransfer {
    SameViewNoCopy {
        register: MachineRegister,
    },
    FixedViewCopy {
        source: MachineRegister,
        destination: MachineRegister,
        constraint: SelectedStructuralCopyConstraint,
    },
}

/// Atomic caller/callee custody and target constraint projection. The
/// legalized identity binds the complete source closure; these fields retain
/// every selected fact without inventing allocator-managed virtual values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProjectedStructuralCallReturn {
    pub recipe: SelectedProjectedStructuralCallReturnRecipe,
    pub legalized_plan: LegalizedOperationPlanIdentity,
    pub caller: MachineId,
    pub callee: MachineId,
    pub projected_qualifications: Vec<StructuralPathQualification>,
    pub fragments: Vec<SelectedStructuralFragmentConstraint>,
    pub call: SelectedStructuralCallConstraint,
    pub caller_return: SelectedStructuralReturnConstraint,
    pub callee_return: SelectedStructuralReturnConstraint,
    pub caller_argument_transfer: SelectedStructuralTransfer,
    pub callee_return_transfer: SelectedStructuralTransfer,
    pub caller_return_transfer: SelectedStructuralTransfer,
}
