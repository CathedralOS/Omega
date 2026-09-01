//! Optimizer module role: stage group. Atomic projected structural selection carrier.

mod model;

pub use model::{
    SelectedProjectedStructuralCallReturn, SelectedProjectedStructuralCallReturnRecipe,
    SelectedStructuralCallConstraint, SelectedStructuralCopyConstraint,
    SelectedStructuralCopyOperand, SelectedStructuralFixedOperand,
    SelectedStructuralFragmentConstraint, SelectedStructuralFragmentSite,
    SelectedStructuralReturnConstraint, SelectedStructuralTransfer,
};
