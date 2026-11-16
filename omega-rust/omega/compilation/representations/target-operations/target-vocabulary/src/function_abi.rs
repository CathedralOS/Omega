//! The calling contract native forms retain for each function.
//!
//! Image emission publishes each function's ABI with the object, and the
//! hosted entry bridge checks the entry's calling plan against it. Stage 02
//! decides that contract while it lowers. Each later form carries the same
//! record without depending on the earlier program that produced it.

use crate::{
    MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi, TargetStructuralParameter,
};
use calling_conventions::CallPlan;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedFunctionAbi {
    pub machine: MachineId,
    /// The function's own calling plan, whatever its result shape.
    pub call_plan: CallPlan,
    /// Scalar parameters in calling-plan order.
    pub scalar_parameters: Vec<ScalarAbiValue>,
    /// Structural parameters in calling-plan order.
    pub structural_parameters: Vec<TargetStructuralParameter>,
    /// Present when the function takes and returns scalars only.
    pub scalar: Option<ScalarFunctionAbi>,
    /// Present when the function takes structural inputs and returns a scalar.
    pub mixed_structural_scalar: Option<MixedStructuralScalarFunctionAbi>,
}

impl LegalizedFunctionAbi {
    /// The calling plan and parameters the function publishes as its
    /// parameter ABI, when [`publishes_parameter_abi`] says it has one.
    pub fn parameter_abi(
        &self,
    ) -> Option<(&CallPlan, &[ScalarAbiValue], &[TargetStructuralParameter])> {
        publishes_parameter_abi(
            &self.call_plan,
            &self.scalar_parameters,
            &self.structural_parameters,
            self.scalar.is_some() || self.mixed_structural_scalar.is_some(),
        )
        .then_some((
            &self.call_plan,
            self.scalar_parameters.as_slice(),
            self.structural_parameters.as_slice(),
        ))
    }
}

/// Whether a function publishes its calling plan and parameters as a
/// parameter ABI: a Unit function that takes scalar arguments or owned
/// structural inputs, or a function with a result that has neither a scalar
/// nor a mixed scalar ABI.
pub fn publishes_parameter_abi(
    call_plan: &CallPlan,
    scalar_parameters: &[ScalarAbiValue],
    structural_parameters: &[TargetStructuralParameter],
    has_scalar_or_mixed_abi: bool,
) -> bool {
    let takes_inputs = !scalar_parameters.is_empty()
        || structural_parameters
            .iter()
            .any(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned);
    match call_plan.result {
        None => takes_inputs,
        Some(_) => !has_scalar_or_mixed_abi,
    }
}
