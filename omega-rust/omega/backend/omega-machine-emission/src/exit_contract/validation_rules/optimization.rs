use omega_post_allocation_machine_to_optimized_machine::StagedOptimizedPostAllocationMachineOptimization;
use omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::super::{error::WholeFunctionExitContractError, model::WholeFunctionExitLayoutCustody};

pub(in crate::exit_contract) fn post_allocation_layout_custody(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<WholeFunctionExitLayoutCustody, WholeFunctionExitContractError> {
    let normalized = optimization
        .custody()
        .ok_or(WholeFunctionExitContractError::OptimizationCustodyMismatch)?;
    if normalized.source() != machine.machine().receipt().identity()
        || normalized.optimization() != optimization.optimization()
        || normalized.selections() != optimization.selections()
        || normalized.action_count() != optimization.action_count()
        || encoding.post_allocation_machine_optimization() != Some(normalized)
        || layout.post_allocation_machine_optimization() != Some(normalized)
    {
        return Err(WholeFunctionExitContractError::OptimizationCustodyMismatch);
    }
    Ok(generic_layout_custody(
        normalized.optimization(),
        normalized.artifact_identity(),
    ))
}

fn generic_layout_custody(
    optimization: omega_optimization_core::Optimization,
    artifact_identity: [u8; 32],
) -> WholeFunctionExitLayoutCustody {
    WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
        optimization,
        artifact_identity,
    }
}

pub(in crate::exit_contract) fn validate_layout_custody(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    custody: WholeFunctionExitLayoutCustody,
) -> Result<(), WholeFunctionExitContractError> {
    let encoding_optimization = encoding.post_allocation_machine_optimization();
    let layout_optimization = layout.post_allocation_machine_optimization();
    let valid = match custody {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        | WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. } => {
            encoding_optimization.is_none() && layout_optimization.is_none()
        }
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization,
            artifact_identity,
        } => encoding_optimization.is_some_and(|normalized| {
            layout_optimization == Some(normalized)
                && normalized.optimization() == optimization
                && normalized.artifact_identity() == artifact_identity
                && normalized.source() == machine.machine().receipt().identity()
        }),
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion,
        } => encoding_optimization.is_some_and(|normalized| {
            layout_optimization == Some(normalized)
                && normalized.optimization()
                    == omega_optimization_core::Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                && normalized.artifact_identity() == fusion.bytes()
                && normalized.source() == machine.machine().receipt().identity()
        }),
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization,
        } => encoding_optimization.is_some_and(|normalized| {
            layout_optimization == Some(normalized)
                && normalized.optimization()
                    == omega_optimization_core::Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                && normalized.artifact_identity() == materialization.bytes()
                && normalized.source() == machine.machine().receipt().identity()
        }),
    };
    if !valid {
        return Err(WholeFunctionExitContractError::OptimizationCustodyMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::Optimization;

    use super::{WholeFunctionExitLayoutCustody, generic_layout_custody};

    #[test]
    fn generic_custody_binds_the_exact_rule_and_typed_leaf_identity() {
        let artifact_identity = [0x5a; 32];
        assert_eq!(
            generic_layout_custody(
                Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity,
            ),
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity,
            }
        );
        assert_ne!(
            generic_layout_custody(
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                artifact_identity,
            ),
            generic_layout_custody(
                Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity,
            )
        );
    }
}
