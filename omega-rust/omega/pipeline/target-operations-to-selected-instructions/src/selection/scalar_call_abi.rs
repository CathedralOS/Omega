//! Input-only join from the admitted register CallPlan to target constraint rows.

use super::shared::*;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction};
use register_environment::ValidatedTargetRegisterEnvironment;

pub(super) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    key: RegisterConstraintKey,
    row: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    call.validate_shape().map_err(|_| invalid())?;
    if call
        .arguments
        .iter()
        .any(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
    {
        if source.call_plan.policy != CallingPolicy::native_for_target(environment.target()) {
            return Err(invalid());
        }
        validate_borrowed_argument(source, call).ok_or_else(invalid)?;
    }
    let count = call.arguments.len();
    if environment.selected_keys().call_i64.get(count) != Some(&key)
        || environment.constraint(key) != Some(row)
        || row.key != key
        || call.call_plan.parameters.len() != count
        || row.operands.len() != count + 1
    {
        return Err(invalid());
    }
    let result = call.call_plan.result.as_ref().ok_or_else(invalid)?;
    if Some(result) != call.result_placement.as_ref() {
        return Err(invalid());
    }
    for (index, (placement, operand)) in call
        .call_plan
        .parameters
        .iter()
        .chain(std::iter::once(result))
        .zip(&row.operands)
        .enumerate()
    {
        let register = match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] => register,
            [
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    copy_stack_byte_offset: None,
                    byte_size: 16,
                    alignment: 8,
                },
            ] if placement.shape == ValueShape::borrowed_reference(16, 8)
                && matches!(
                    call.arguments.get(index),
                    Some(LegalizedScalarArgument::Structural { .. })
                ) =>
            {
                register
            }
            _ => return Err(invalid()),
        };
        if operand.fixed_view.is_none()
            || operand.fixed_view != environment.fixed_register_view(*register)
            || operand.access
                != if index == count {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                }
        {
            return Err(invalid());
        }
        if let Some(argument) = call.arguments.get(index)
            && argument.placement() != placement
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Rejoin the original descriptor parameter before permitting pointer transport.
/// Callee declarations and source-call correspondence remain legalization inputs.
fn validate_borrowed_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
) -> Option<()> {
    let signature = source.structural.as_ref()?;
    let [parameter] = signature.parameters.as_slice() else {
        return None;
    };
    let [LegalizedScalarArgument::Structural { semantic, target }] = call.arguments.as_slice()
    else {
        return None;
    };
    let parameters = [crate::structural_unit_input::Parameter {
        semantic: &parameter.semantic,
        target: &parameter.target,
    }];
    let shape = ValueShape::borrowed_reference(16, 8);
    let expected = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .ok()?;
    if source.attachment.is_some()
        || source.ranked.is_some()
        || !signature.entry_claims.is_empty()
        || !signature.published_service_ceiling.is_empty()
        || !crate::structural_unit_input::accepts_borrowed_view(
            &source.call_plan,
            &parameters,
            &signature.structural_types,
        )
        || call.source != legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
        || !call.claim_transfers.is_empty()
        || !call.requirement_obligations.is_empty()
        || !call.crash_continuations.is_empty()
        || call.call_plan != expected
        || semantic.place != parameter.semantic.place
        || semantic.access != StructuralAccess::SharedBorrow
        || !semantic.path.is_empty()
        || target.place != semantic.place
        || target.access != semantic.access
        || !target.path.is_empty()
        || target.root_structural_type != parameter.semantic.structural_type
        || target.structural_type != parameter.semantic.structural_type
        || target.shape != shape
        || target.source_byte_offset != 0
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || target.source != parameter.target.placement
        || target.destination != expected.parameters[0]
    {
        return None;
    }
    Some(())
}
