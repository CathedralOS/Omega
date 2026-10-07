//! Emit the prefix effects already retained by the checked scalar-return plan.
//! The producer owns statement coverage, signature and permission admission;
//! lowering consumes that plan rather than recognizing its source body again.
use super::super::{CheckedUnitEffectOperationPlan, StructuralParameterDeclaration};
use super::{
    CheckedStructuralScalarReturnMachinePlan, LoweringError, Operation, StructuralTypeDeclaration,
    ValueDeclaration, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;

pub(super) fn emit(
    plan: &CheckedStructuralScalarReturnMachinePlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    for effect in &plan.effects {
        let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            destination: checked_trees::CheckedStoreRoot::Parameter { position },
            value,
            ..
        } = effect
        else {
            return unsupported("scalar return acquired an unsupported prefix effect");
        };
        let checked_trees::CheckedCallScalarArgument::Pure(value) = value else {
            return unsupported(
                "scalar return prefix computation requires ordinary operation emission",
            );
        };
        // `parameters` lowers the retained structural roster densely; the
        // store's authored root selects its entry by that roster's position.
        let parameter_index = plan
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position == *position)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or(LoweringError::Unsupported(
                "write-only store names an unknown structural parameter",
            ))?;
        let kind = crate::emission::primitive_store::emit(
            parameter_index,
            value,
            parameters,
            structural_types,
            plan.scalar_parameters.len(),
            scalar_values,
            next_value,
            operations,
        )?;
        let operation = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: terminal_psi::OperationResult::Unit,
            kind,
        });
    }
    Ok(())
}
