use crate::{
    BoundNominalCallbackPlacement, CallbackPlacementBindingIdentity, CallbackThunkPlan,
    callback_placement_binding_identity, canonical_callback_private_symbol,
    replay_callback_root_schedule, validate_bound_nominal_callback_placement,
};
use omega_calling_conventions::{
    CallbackRequirementId, NativePlace, PlanDiagnostic, StaticMachineBinderId,
};
use omega_control_flow::MachineFunctionIdentity;
use std::sync::Arc;

/// Address-free demand to place one emitted private callback function into the
/// exact outbound registrar binder destination selected during checking.
///
/// This carrier owns no target operation, physical offset, bytes, object
/// relocation, runtime storage, native address, registration authority, or
/// lease. Those require later joins that this row cannot authorize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackPrivateRelocationDemand {
    pub placement_index: usize,
    pub placement_identity: CallbackPlacementBindingIdentity,
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
    pub function_identity: MachineFunctionIdentity,
    pub private_symbol: Arc<str>,
}

/// Independently replay one address-free demand against its exact checked
/// placement and emitted thunk/root schedule.
pub fn replay_callback_private_relocation_demand(
    demand: &CallbackPrivateRelocationDemand,
    placement_index: usize,
    placement: &BoundNominalCallbackPlacement,
    thunk: &CallbackThunkPlan,
) -> Result<(), PlanDiagnostic> {
    validate_bound_nominal_callback_placement(placement)?;
    let Some(materialization) = &placement.private_materialization else {
        return Err(PlanDiagnostic(
            "callback private relocation demand names a placement without a target-closed materialization"
                .into(),
        ));
    };
    let expected_identity = callback_placement_binding_identity(placement);
    if demand.placement_index != placement_index
        || demand.placement_identity != expected_identity
        || demand.binder != materialization.binder
        || demand.destination != materialization.destination
        || demand.requirement != materialization.requirement
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact placement row".into(),
        ));
    }
    if thunk.placement_index != placement_index
        || thunk.placement_identity != expected_identity
        || demand.function_identity != thunk.function_identity
        || demand.private_symbol != thunk.private_symbol
        || demand.private_symbol != canonical_callback_private_symbol(placement)
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact thunk identity".into(),
        ));
    }
    replay_callback_root_schedule(&thunk.root_schedule, placement)?;
    if thunk.root_schedule.placement_index() != placement_index
        || thunk.root_schedule.placement_identity() != &expected_identity
        || thunk.root_schedule.function_identity() != demand.function_identity
        || thunk.root_schedule.private_symbol() != &demand.private_symbol
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact root schedule".into(),
        ));
    }
    Ok(())
}

/// Replay the complete ordered address-free demand catalog. Each retained
/// private materialization must have exactly one same-index thunk and demand;
/// non-materializing placements must have no demand.
pub fn replay_callback_private_relocation_demands(
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
) -> Result<(), PlanDiagnostic> {
    if thunks.len() != placements.len() {
        return Err(PlanDiagnostic(format!(
            "callback private relocation replay requires one thunk per placement, but retained {} placements and {} thunks",
            placements.len(),
            thunks.len()
        )));
    }
    let expected_count = placements
        .iter()
        .filter(|placement| placement.private_materialization.is_some())
        .count();
    if demands.len() != expected_count {
        return Err(PlanDiagnostic(format!(
            "callback private relocation replay requires {expected_count} exact demands, but retained {}",
            demands.len()
        )));
    }

    let mut next_demand = 0usize;
    for (placement_index, placement) in placements.iter().enumerate() {
        let matching_thunks = thunks
            .iter()
            .filter(|thunk| thunk.placement_index == placement_index)
            .collect::<Vec<_>>();
        let [thunk] = matching_thunks.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback placement {placement_index} resolves to {} thunks; exactly one is required",
                matching_thunks.len()
            )));
        };
        if placement.private_materialization.is_none() {
            if demands
                .iter()
                .any(|demand| demand.placement_index == placement_index)
            {
                return Err(PlanDiagnostic(format!(
                    "callback placement {placement_index} has a private relocation demand without a target-closed materialization"
                )));
            }
            continue;
        }
        let demand = demands.get(next_demand).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback placement {placement_index} is missing its ordered private relocation demand"
            ))
        })?;
        if demand.placement_index != placement_index {
            return Err(PlanDiagnostic(format!(
                "callback private relocation demand order drifted at placement {placement_index}"
            )));
        }
        replay_callback_private_relocation_demand(demand, placement_index, placement, thunk)?;
        next_demand += 1;
    }
    if next_demand != demands.len() {
        return Err(PlanDiagnostic(
            "callback private relocation replay retained an unowned trailing demand".into(),
        ));
    }
    Ok(())
}
