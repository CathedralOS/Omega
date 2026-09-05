//! Root joins and ABI facts reconstructed without an exit-record constructor.

use super::super::{
    WholeFunctionEntryAssumption, WholeFunctionExitContractError, WholeFunctionExitLayoutCustody,
    WholeFunctionExitPolicy, WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
    validation_rules::{EntryAssumptionKind, target_contract_inputs, view},
};
use super::{Inputs, require};
use register_model::{RegisterUnitId, RegisterViewId};
use std::collections::BTreeSet;

pub(super) struct Context {
    pub stack_pointer: RegisterViewId,
    pub result_view: RegisterViewId,
    pub link_register: Option<RegisterViewId>,
    pub stack_units: BTreeSet<RegisterUnitId>,
    pub link_units: BTreeSet<RegisterUnitId>,
    pub callee_saved: BTreeSet<RegisterUnitId>,
}

pub(super) fn check(
    inputs: &Inputs<'_>,
    custody: WholeFunctionExitLayoutCustody,
) -> Result<Context, WholeFunctionExitContractError> {
    let Inputs {
        selected,
        machine,
        physical,
        encoding,
        layout,
        frame,
        contract,
    } = inputs;
    if encoding.selected() != machine.selected
        || layout.selected() != machine.selected
        || encoding.machine() != machine.identity
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity()
        || selected.target != machine.target
        || layout.target() != machine.target
        || physical.identity() != machine.physical_register_model
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let (mut policy, convention, stack_name, link_name, entry) =
        target_contract_inputs(physical, machine.target)?;
    if convention.result_views.len() != 1 || convention.stack_alignment == 0 {
        return Err(WholeFunctionExitContractError::InvalidConvention);
    }
    let stack = view(physical, stack_name)?;
    let link = link_name.map(|name| view(physical, name)).transpose()?;
    let entry = match (entry, link) {
        (EntryAssumptionKind::ActivationStack, None) => {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
        }
        (EntryAssumptionKind::LinkRegister, Some(link)) => {
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
                link_register: link.id,
            }
        }
        _ => return Err(WholeFunctionExitContractError::InvalidConvention),
    };
    let disposition = if let Some((frame, protocol)) = frame {
        if frame.receipt().post_allocation_machine() != machine.identity
            || frame.plan().register_environment != machine.register_environment
            || frame.plan().physical_register_model != machine.physical_register_model
            || frame.receipt().target() != machine.target
            || protocol.receipt().frame_layout() != frame.receipt().identity()
            || protocol.receipt().target() != machine.target
        {
            return Err(WholeFunctionExitContractError::RootMismatch);
        }
        policy = match policy {
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => {
                WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1
            }
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => {
                WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1
            }
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => {
                WholeFunctionExitPolicy::DarwinAapcs64CanonicalFixedFrameV1
            }
            _ => return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
        };
        WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
            layout: frame.receipt().identity(),
            protocol: protocol.receipt().identity(),
        }
    } else {
        WholeFunctionFrameDisposition::FramelessV1
    };
    if !selected.structural_unit_functions.is_empty() {
        if policy != WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
            || frame.is_some() || custody != WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
            || !selected.functions.is_empty() || !machine.functions.is_empty()
            || !encoding.rows().is_empty() || !layout.functions().is_empty()
            || layout.policy() != machine_code::SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
        { return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy); }
        policy = if selected
            .structural_unit_functions
            .iter()
            .any(|function| function.call.is_some())
        {
            WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1
        } else {
            WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1
        };
    }
    require(
        contract.selected == machine.selected
            && contract.post_allocation_manifest == machine.post_allocation_manifest
            && contract.post_allocation_machine == machine.identity
            && contract.register_environment == machine.register_environment
            && contract.physical_register_model == machine.physical_register_model
            && contract.pre_layout == encoding.identity()
            && contract.resolved_layout == layout.identity()
            && contract.layout_custody == custody
            && contract.target == machine.target
            && contract.policy == policy
            && contract.frame == disposition
            && contract.hardening == WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1
            && contract.entry_assumption == entry
            && contract.stack_pointer == stack.id
            && contract.stack_alignment == convention.stack_alignment
            && contract.red_zone_bytes == convention.red_zone_bytes
            && contract.result_view == convention.result_views[0]
            && contract.callee_saved_units == convention.callee_saved,
    )?;
    Ok(Context {
        stack_pointer: stack.id,
        result_view: convention.result_views[0],
        link_register: link.map(|link| link.id),
        stack_units: stack.units.iter().copied().collect(),
        link_units: link
            .map(|link| link.units.iter().copied().collect())
            .unwrap_or_default(),
        callee_saved: convention.callee_saved.iter().copied().collect(),
    })
}
