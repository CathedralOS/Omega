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
    /// One entry conversion per internal parameter. `Some(0)` is direct
    /// transport; another word is the existing runtime scalar codec. `None`
    /// means a closed scalar referent has no supported entry conversion, not
    /// that its type was missing. Internal calls need no external conversion.
    pub entry_scalar_codecs: Vec<Option<u64>>,
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
    /// Every retained scalar lane, including the specialized result views.
    pub fn scalar_values(&self) -> impl Iterator<Item = &ScalarAbiValue> {
        self.scalar_parameters
            .iter()
            .chain(
                self.scalar
                    .iter()
                    .flat_map(|abi| abi.parameters.iter().chain(abi.result.iter())),
            )
            .chain(self.mixed_structural_scalar.iter().flat_map(|abi| {
                abi.scalar_parameters
                    .iter()
                    .chain(std::iter::once(&abi.result))
            }))
    }

    pub fn scalar_values_mut(&mut self) -> impl Iterator<Item = &mut ScalarAbiValue> {
        self.scalar_parameters
            .iter_mut()
            .chain(
                self.scalar
                    .iter_mut()
                    .flat_map(|abi| abi.parameters.iter_mut().chain(abi.result.iter_mut())),
            )
            .chain(self.mixed_structural_scalar.iter_mut().flat_map(|abi| {
                abi.scalar_parameters
                    .iter_mut()
                    .chain(std::iter::once(&mut abi.result))
            }))
    }

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
