use crate::LegalizedScalarCall;
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

#[cfg(test)]
mod tests {
    use super::*;

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
                    source: crate::LegalizedCallUnitSource::AuthoredCallUnit,
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
