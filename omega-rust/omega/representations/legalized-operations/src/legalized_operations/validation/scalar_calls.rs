use crate::{LegalizedDynamicParameterCall, LegalizedScalarCall};
use calling_conventions::{EntryControl, ValueLocation, ValuePlacement, ValueShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarCallShapeError {
    ArgumentCount,
    ArgumentPlacement { argument: usize },
    Result,
    CallControl,
}
impl LegalizedScalarCall {
    /// Raw ABI consistency only; source, types and canonical target ABI require replay.
    pub fn validate_shape(&self) -> Result<(), LegalizedScalarCallShapeError> {
        use LegalizedScalarCallShapeError as Error;
        if self.call_plan.entry_control != EntryControl::CallReturn
            || !self.call_plan.callback_materializations.is_empty()
        {
            return Err(Error::CallControl);
        }
        if self.arguments.len() != self.call_plan.parameters.len() {
            return Err(Error::ArgumentCount);
        }
        for (index, (argument, placement)) in self
            .arguments
            .iter()
            .zip(&self.call_plan.parameters)
            .enumerate()
        {
            if argument.placement() != placement
                || matches!(argument, crate::LegalizedScalarArgument::Scalar { .. })
                    && !direct_scalar_placement(placement)
            {
                return Err(Error::ArgumentPlacement { argument: index });
            }
        }
        if self.result_placement != self.call_plan.result
            || self.result_placement.as_ref().is_some_and(|placement| {
                if self.structural_result.is_some() {
                    !direct_aggregate_registers(placement) && !indirect_aggregate_result(placement)
                } else {
                    !direct_scalar_register(placement)
                }
            })
            || self.structural_result.is_some() && self.result_placement.is_none()
        {
            return Err(Error::Result);
        }
        Ok(())
    }
}
/// Complete direct integer-bank aggregate result, with no gaps or hidden result pointer.
fn direct_aggregate_registers(placement: &ValuePlacement) -> bool {
    // Some(empty) preserves a structural result while requiring no register.
    if placement.shape == ValueShape::integer(0, 1) && placement.locations.is_empty() {
        return true;
    }
    if placement.shape.class != calling_conventions::ValueClass::Integer
        || placement.shape.byte_size == 0
        || placement.shape.byte_size > 16
        || placement.locations.is_empty()
        || placement.locations.len() > 2
    {
        return false;
    }
    let mut offset = 0;
    let mut registers = Vec::new();
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = location
        else {
            return false;
        };
        if *value_byte_offset != offset
            || *byte_size == 0
            || *byte_size > 8
            || registers.contains(register)
        {
            return false;
        }
        let Some(end) = offset.checked_add(*byte_size) else {
            return false;
        };
        offset = end;
        registers.push(*register);
    }
    offset == placement.shape.byte_size
}
fn direct_scalar_placement(placement: &ValuePlacement) -> bool {
    let width = placement.shape.byte_size;
    matches!(width, 1 | 2 | 4 | 8)
        && (placement.shape == ValueShape::integer(width, width)
            || matches!(width, 4 | 8) && placement.shape == ValueShape::float(width))
        && (matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            }] if *byte_size == width
        ) || matches!(placement.locations.as_slice(),
            [ValueLocation::Stack { stack_byte_offset, value_byte_offset: 0, byte_size, alignment }]
                if *byte_size == width
                    && *alignment >= placement.shape.alignment && alignment.is_power_of_two()
                    && stack_byte_offset.is_multiple_of(u32::from(*alignment))))
}
fn direct_scalar_register(placement: &ValuePlacement) -> bool {
    let width = placement.shape.byte_size;
    matches!(width, 1 | 2 | 4 | 8)
        && (placement.shape == ValueShape::integer(width, width)
            || matches!(width, 4 | 8) && placement.shape == ValueShape::float(width))
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            }] if *byte_size == width
        )
}

fn indirect_aggregate_result(placement: &ValuePlacement) -> bool {
    placement.shape.class == calling_conventions::ValueClass::Integer
        && placement.shape.byte_size > 0
        && placement.shape.alignment.is_power_of_two()
        && matches!(placement.locations.as_slice(), [ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(_),
            copy_stack_byte_offset: None, byte_size, alignment,
        }] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedDynamicParameterCallShapeError {
    CallControl,
    DispatchSignature,
    Custody,
    Requirement,
    Result,
    TableOffset,
}

impl LegalizedDynamicParameterCall {
    /// Raw contract consistency only: the erased adapter plan, the parameter
    /// ABI binding, the requirement row, and the slot offset must agree
    /// internally. Semantic join and roster membership require replay against
    /// the owning function's plan and abstract operation.
    pub fn validate_shape(&self) -> Result<(), LegalizedDynamicParameterCallShapeError> {
        use LegalizedDynamicParameterCallShapeError as Error;
        if self.dispatch_call_plan.entry_control != EntryControl::CallReturn
            || !self.dispatch_call_plan.callback_materializations.is_empty()
        {
            return Err(Error::CallControl);
        }
        // The erased adapter receives exactly the instance word; its byte
        // width is the pointer width and the only placement family admitted.
        let [instance] = self.dispatch_call_plan.parameters.as_slice() else {
            return Err(Error::DispatchSignature);
        };
        if !direct_scalar_placement(instance) {
            return Err(Error::DispatchSignature);
        }
        let dispatch = &self.dynamic_dispatch.dispatch;
        let parameter = &self.dynamic_dispatch.parameter;
        if self.parameter_abi.parameter != *parameter
            || dispatch.parameter_ordinal != parameter.ordinal
            || dispatch.owner != parameter.owner
        {
            return Err(Error::Custody);
        }
        let mut selected = parameter
            .requirements
            .iter()
            .filter(|row| row.slot == dispatch.requirement_slot);
        if selected.next() != Some(&self.requirement) || selected.next().is_some() {
            return Err(Error::Requirement);
        }
        match (&self.result_home, &self.dispatch_call_plan.result) {
            (Some(home), Some(placement)) if placement.shape == home.shape => {}
            (None, None) => {}
            _ => return Err(Error::Result),
        }
        if self.table_slot_byte_offset
            != dispatch
                .requirement_slot
                .checked_mul(u32::from(instance.shape.byte_size))
                .ok_or(Error::TableOffset)?
        {
            return Err(Error::TableOffset);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{LegalizedScalarCall, LegalizedScalarCallShapeError, ValueLocation, ValueShape};

    #[test]
    fn scalar_float_call_result_retains_exact_direct_width() {
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::windows_x64(),
            target::NativeTarget::macos_arm64(),
        ] {
            for width in [4, 8] {
                let call_plan = calling_conventions::evaluate_call_plan(
                    calling_conventions::CallingPolicy::native_for_target(target),
                    &calling_conventions::CallSignature {
                        parameters: Vec::new(),
                        result: Some(ValueShape::float(width)),
                    },
                )
                .unwrap();
                let mut call = LegalizedScalarCall {
                    structural_result: None,
                    source: crate::NativeCallOrigin::Authored,
                    claim_transfers: Vec::new(),
                    callee: semantic_vocabulary::MachineId::new(1).unwrap(),
                    arguments: Vec::new(),
                    result_placement: call_plan.result.clone(),
                    call_plan,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                };
                call.validate_shape().unwrap();
                let result = call.result_placement.as_mut().unwrap();
                let ValueLocation::Register { byte_size, .. } = &mut result.locations[0] else {
                    panic!("direct scalar float result");
                };
                *byte_size = if width == 4 { 8 } else { 4 };
                call.call_plan.result = call.result_placement.clone();
                assert_eq!(
                    call.validate_shape(),
                    Err(LegalizedScalarCallShapeError::Result)
                );
            }
        }
    }
}
