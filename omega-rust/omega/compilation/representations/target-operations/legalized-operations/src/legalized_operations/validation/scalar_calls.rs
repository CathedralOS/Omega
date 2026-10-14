use crate::LegalizedScalarCall;
use calling_conventions::{EntryControl, ValueLocation, ValuePlacement, ValueShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarCallShapeError {
    ArgumentCount,
    ArgumentPlacement { argument: usize },
    Result,
    CallControl,
    PlacedBinding { binding: usize },
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
        let structural = self
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                crate::LegalizedScalarArgument::Structural { target, .. } => Some(target),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut previous = None;
        for (index, binding) in self.placed_bindings.iter().enumerate() {
            let position = usize::try_from(binding.argument_index)
                .map_err(|_| Error::PlacedBinding { binding: index })?;
            let Some(argument) = structural.get(position) else {
                return Err(Error::PlacedBinding { binding: index });
            };
            if previous.is_some_and(|previous| previous >= binding.argument_index)
                || binding.argument_index != binding.input.runtime_position
                || self.target
                    != target_operations::NativeCallTarget::Machine(binding.input.machine)
                || argument.access != binding.input.access
                || argument.access == terminal_psi::StructuralAccess::Owned
                || match &binding.association {
                    target_operations::TargetPlacedCallAssociation::Captured {
                        carrier, ..
                    } => *carrier != argument.structural_type,
                    target_operations::TargetPlacedCallAssociation::CallerFormal(input) => {
                        input.carrier_identity != binding.input.carrier_identity
                            || input.view_identity != binding.input.view_identity
                            || input.policy_identity != binding.input.policy_identity
                            || input.policy_plan_machine_identity
                                != binding.input.policy_plan_machine_identity
                            || input.schema_identity != binding.input.schema_identity
                            || input.placement_commitment != binding.input.placement_commitment
                            || !binding.input.access.admits(input.access)
                    }
                }
            {
                return Err(Error::PlacedBinding { binding: index });
            }
            previous = Some(binding.argument_index);
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
            || self.structural_result.is_none() && !self.descriptor_origins.is_empty()
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
        && (placement.shape == ValueShape::integer(width, width as u16)
            || matches!(width, 4 | 8) && placement.shape == ValueShape::float(width as u16))
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
        && (placement.shape == ValueShape::integer(width, width as u16)
            || matches!(width, 4 | 8) && placement.shape == ValueShape::float(width as u16))
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
                        result: Some(ValueShape::float(width as u16)),
                    },
                )
                .unwrap();
                let mut call = LegalizedScalarCall {
                    descriptor_origins: Vec::new(),
                    placed_bindings: Vec::new(),
                    structural_result: None,
                    source: crate::NativeCallOrigin::Authored,
                    claim_transfers: Vec::new(),
                    target: target_operations::NativeCallTarget::Machine(
                        semantic_vocabulary::MachineId::new(1).unwrap(),
                    ),
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
