//! Input-only ranked authority replay. The destination remains an ordinary instruction graph.
use super::*;
use legalized_operations::{LegalizedCallUnitParameter, LegalizedStructuralContract};
mod correspondence;
mod layout;
mod proof;
mod semantic_graph;

pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetOperation::RankedU32Countdown(ranked) = &target.operation else {
        return Err(invalid);
    };
    if native.target != ::target::NativeTarget::linux_x64()
        && native.target != ::target::NativeTarget::linux_arm64()
    {
        return Err(invalid);
    }
    if proof::replay_verifier_custody(&ranked.custody).is_none()
        || abstract_operations::encode_ranked_u32_countdown_custody(&ranked.custody).is_err()
    {
        return Err(invalid);
    }
    let replay = &ranked.custody.semantic_replay;
    let [machine] = replay.machines.as_slice() else {
        return Err(invalid);
    };
    if terminal_codec::terminal_psi_identity(replay).map_err(|_| invalid.clone())? != plan.psi
        || native.functions.len() != 1
        || unit.functions.len() != 1
        || target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.machine != machine.id
        || target.attachment != machine.attachment
        || target.attachment.is_none()
        || ranked
            .structural_types
            .iter()
            .filter(|declaration| Some(declaration.id) == target.attachment)
            .count()
            != 1
        || target.scalar_abi.is_some()
        || target.mixed_structural_scalar_abi.is_some()
        || plan.structural_types != replay.structural_types
        || unit.structural_types != replay.structural_types
        || unit.fuel_schedule != ranked.custody.fixed_fuel.schedule()
        || abstracted.structural_parameters != machine.structural_parameters
        || optimized.structural_parameters != machine.structural_parameters
        || optimized.structural_places != machine.structural_places
        || abstracted.entry_claims != machine.entry_claims
        || optimized.entry_claim_declarations != machine.entry_claims
        || abstracted.published_service_ceiling != machine.published_service_ceiling
        || optimized.published_service_ceiling != machine.published_service_ceiling
        || optimized.result != abstracted.result
        || optimized.parameters.len() != abstracted.parameters.len()
        || optimized
            .parameters
            .iter()
            .zip(&abstracted.parameters)
            .enumerate()
            .any(|(index, (actual, expected))| {
                actual.value != expected.value
                    || actual.scalar_type != expected.scalar_type
                    || actual.site != ValueDefinitionSite::FunctionParameter(index as u32)
            })
        || optimized.declared_places
            != abstracted
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect()
        || optimized.entry_claims
            != abstracted
                .entry_claims
                .iter()
                .map(|claim| claim.claim)
                .collect()
        || !optimized.content_entry_claims.is_empty()
    {
        return Err(invalid);
    }
    correspondence::validate(target, native.target, plan, ranked)
}

pub(in crate::legalization) fn structural_contract(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
) -> Option<LegalizedStructuralContract> {
    let TargetOperation::RankedU32Countdown(ranked) = &target.operation else {
        return None;
    };
    Some(LegalizedStructuralContract {
        result: None,
        structural_types: ranked.structural_types.clone(),
        parameters: abstracted
            .structural_parameters
            .iter()
            .zip(&ranked.structural_parameters)
            .map(|(semantic, target)| LegalizedCallUnitParameter {
                semantic: semantic.clone(),
                target: target.clone(),
            })
            .collect(),
        structural_places: optimized.structural_places.clone(),
        entry_claims: abstracted.entry_claims.clone(),
        published_service_ceiling: abstracted.published_service_ceiling.clone(),
    })
}
