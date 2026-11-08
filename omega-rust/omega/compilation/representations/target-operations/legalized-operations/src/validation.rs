// Representation-owned validation for retained legalized-operation values.
//
// `LegalizedScalarCall::validate_source` checks call-origin, provider-completion, and ownership
// event consistency. Installation already joined completion receipts to candidate entry claims;
// caller entry provenance describes historical ingress rather than current custody, so a fresh
// local or transferred result need not carry an entry row. We therefore validate the installed
// transfers and ownership event directly without reconstructing a narrower entry-only custody
// judgement.
//
// `LegalizedScalarCall::validate_shape` and `validate_no_normal_shape` check raw ABI placement
// consistency (`CallReturn` entry control, argument count/placement, access-binding ordering and
// carrier/view/policy/schema/commitment agreement, and direct/indirect result placement). Empty
// aggregate results (`ValueShape::integer(0, 1)` with empty locations) are valid structural
// results that require no physical register.
//
// `LegalizedScalarInstruction::has_valid_hosted_read_byte_shape` checks the representation shape
// of a `HostedReadByte` sum result (`ValueShape::integer(8, 4)`, 4-byte tag at offset 0, and
// 4-byte payload at offset 4 on case 1).

use crate::{
    LegalizedCallSourceError, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, NativeCallOrigin,
};
use calling_conventions::{EntryControl, ValueLocation, ValuePlacement, ValueShape};
use optimization_unit::OwnershipEvent;
use terminal_psi::{ClaimTransfer, StructuralMultiplicity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarCallShapeError {
    ArgumentCount,
    ArgumentPlacement { argument: usize },
    Result,
    CallControl,
    AccessBinding { binding: usize },
}

impl LegalizedScalarCall {
    pub fn validate_source(
        &self,
        ownership: &[OwnershipEvent],
    ) -> Result<(), LegalizedCallSourceError> {
        validate_call_source(self, ownership)
    }

    pub fn validate_shape(&self) -> Result<(), LegalizedScalarCallShapeError> {
        self.validate_invocation_shape(false)
    }

    pub fn validate_no_normal_shape(&self) -> Result<(), LegalizedScalarCallShapeError> {
        self.validate_invocation_shape(true)
    }

    fn validate_invocation_shape(
        &self,
        no_normal: bool,
    ) -> Result<(), LegalizedScalarCallShapeError> {
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
        for (index, binding) in self.access_bindings.iter().enumerate() {
            let position = usize::try_from(binding.argument_index)
                .map_err(|_| Error::AccessBinding { binding: index })?;
            let Some(argument) = structural.get(position) else {
                return Err(Error::AccessBinding { binding: index });
            };
            if previous.is_some_and(|previous| previous >= binding.argument_index)
                || binding.argument_index != binding.input.runtime_position
                || self.target
                    != target_operations::NativeCallTarget::Machine(binding.input.machine)
                || argument.access != binding.input.access
                || argument.access == terminal_psi::StructuralAccess::Owned
                || match &binding.association {
                    target_operations::TargetAccessCallAssociation::Captured {
                        carrier, ..
                    } => *carrier != argument.structural_type,
                    target_operations::TargetAccessCallAssociation::CallerFormal(input) => {
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
                return Err(Error::AccessBinding { binding: index });
            }
            previous = Some(binding.argument_index);
        }
        if self.result_placement != self.call_plan.result
            || self.result_placement.as_ref().is_some_and(|placement| {
                if no_normal {
                    !direct_scalar_register(placement)
                        && !direct_aggregate_registers(placement)
                        && !indirect_aggregate_result(placement)
                } else if self.structural_result.is_some() {
                    !direct_aggregate_registers(placement) && !indirect_aggregate_result(placement)
                } else {
                    !direct_scalar_register(placement)
                }
            })
            || self.structural_result.is_some() && self.result_placement.is_none()
            || self.structural_result.is_none() && !self.descriptor_origins.is_empty()
            || no_normal && self.structural_result.is_some()
        {
            return Err(Error::Result);
        }
        Ok(())
    }
}

fn validate_call_source(
    call: &LegalizedScalarCall,
    ownership: &[OwnershipEvent],
) -> Result<(), LegalizedCallSourceError> {
    match &call.source {
        NativeCallOrigin::ProgramEntry {
            entry,
            source_signature,
            completion_claim_sources: claim_sources,
            completion_receipts: receipts,
            ..
        } => {
            if call.target.machine() != Some(*entry) || *source_signature == [0; 32] {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            validate_completion(call, ownership, claim_sources, receipts)
        }
        NativeCallOrigin::Authored => {
            let claims = call
                .claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<Vec<_>>();
            ((claims.is_empty() && ownership.is_empty())
                || ownership == [OwnershipEvent::ClaimTransfer(claims)])
            .then_some(())
            .ok_or(LegalizedCallSourceError::OwnershipMismatch)
        }
        NativeCallOrigin::InstalledProvider {
            boundary,
            provider,
            assumption,
            completion_claim_sources: claim_sources,
            completion_receipts: receipts,
        } => {
            if assumption
                .as_ref()
                .map_or(provider.boundary != *boundary, |row| {
                    !row.matches(*boundary, provider)
                })
                || call.target.machine() != Some(provider.candidate)
            {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            let structural_arguments = call
                .arguments
                .iter()
                .filter_map(|argument| match argument {
                    LegalizedScalarArgument::Structural {
                        semantic, target, ..
                    } => Some((semantic, target)),
                    LegalizedScalarArgument::Scalar { .. } => None,
                })
                .collect::<Vec<_>>();
            if provider.signature.parameters.len() != structural_arguments.len()
                || call
                    .arguments
                    .iter()
                    .skip_while(|argument| {
                        matches!(argument, LegalizedScalarArgument::Scalar { .. })
                    })
                    .any(|argument| matches!(argument, LegalizedScalarArgument::Scalar { .. }))
                || structural_arguments
                    .iter()
                    .zip(&provider.signature.parameters)
                    .any(|((semantic, target), parameter)| {
                        semantic.access != parameter.access
                            || target.place != semantic.place
                            || target.path != semantic.path
                            || target.access != parameter.access
                            || target.structural_type != parameter.structural_type
                    })
            {
                return Err(LegalizedCallSourceError::ArgumentSignatureMismatch);
            }
            validate_completion(call, ownership, claim_sources, receipts)
        }
        NativeCallOrigin::TargetFamily {
            member,
            target,
            completion_claim_sources: claim_sources,
            completion_receipts: receipts,
            ..
        } => {
            if call.target.machine() != Some(*member) || target.is_empty() {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            validate_completion(call, ownership, claim_sources, receipts)
        }
    }
}

fn validate_completion(
    call: &LegalizedScalarCall,
    ownership: &[OwnershipEvent],
    claim_sources: &[abstract_operations::CompletionClaimSource],
    receipts: &[terminal_psi::CompletionReceipt],
) -> Result<(), LegalizedCallSourceError> {
    let structural_count = call
        .arguments
        .iter()
        .filter(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
        .count();
    let transfers = receipts
        .iter()
        .map(|receipt| ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
            path: receipt.path.clone(),
        })
        .collect::<Vec<_>>();
    let unique_source_claims = claim_sources
        .iter()
        .map(|source| source.claim)
        .collect::<std::collections::BTreeSet<_>>();
    if unique_source_claims.len() != claim_sources.len()
        || transfers != call.claim_transfers
        || receipts.windows(2).any(|pair| pair[0] >= pair[1])
        || receipts
            .iter()
            .any(|receipt| receipt.argument_index as usize >= structural_count)
    {
        return Err(LegalizedCallSourceError::CompletionEvidenceMismatch);
    }
    let completed = receipts.to_vec();
    ((completed.is_empty() && ownership.is_empty())
        || ownership == [OwnershipEvent::ClaimCompletion(completed)])
    .then_some(())
    .ok_or(LegalizedCallSourceError::OwnershipMismatch)
}

fn direct_aggregate_registers(placement: &ValuePlacement) -> bool {
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

impl LegalizedScalarInstruction {
    pub fn has_valid_hosted_read_byte_shape(&self) -> bool {
        let LegalizedScalarInstructionKind::HostedReadByte { result, layout, .. } = &self.kind
        else {
            return false;
        };
        self.result.is_none()
            && self.ownership
                == [optimization_unit::OwnershipEvent::ClaimCompletion(
                    Vec::new(),
                )]
            && result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && layout.shape == ValueShape::integer(8, 4)
            && layout.tag_byte_offset == 0
            && layout.tag_shape == ValueShape::integer(4, 4)
            && layout.common_fields.is_empty()
            && layout.payload_byte_offset == 4
            && layout.cases.len() == 2
            && layout.cases[0].fields.is_empty()
            && layout.cases[1].fields.as_slice()
                == [calling_conventions::PackedFieldLayout {
                    shape: ValueShape::integer(4, 4),
                    byte_offset: 4,
                }]
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
                        result: Some(ValueShape::float(width as u16)),
                    },
                )
                .unwrap();
                let mut call = LegalizedScalarCall {
                    descriptor_origins: Vec::new(),
                    access_bindings: Vec::new(),
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
