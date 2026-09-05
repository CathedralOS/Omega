//! Optimizer module role: stage group. Structural-Unit ABI, call, and return selection carriers.

mod model;

pub use model::{
    SelectedBoundarySettlement, SelectedMicrosoftX64OwnedIndirectPairLayout,
    SelectedStructuralUnitAbi, SelectedStructuralUnitAbiRecipe, SelectedStructuralUnitCallArgument,
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitCallSource,
    SelectedStructuralUnitFunction, SelectedStructuralUnitIndirectBinding,
    SelectedStructuralUnitParameter, SelectedStructuralUnitReturn,
};
