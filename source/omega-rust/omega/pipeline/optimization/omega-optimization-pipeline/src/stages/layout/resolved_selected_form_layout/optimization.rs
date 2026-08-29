use omega_optimization_core::Optimization;

use crate::{
    PostAllocationMachineOptimizationCustody, StagedOptimizedPostAllocationMachineOptimization,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedSelectedFormEncoding,
};

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::StagedOptimizedResolvedSelectedFormLayout;

pub(super) fn validate_optimization_custody(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<
    Option<PostAllocationMachineOptimizationCustody>,
    OptimizedResolvedSelectedFormLayoutError,
> {
    let Some(optimization) = optimization else {
        return if pre_layout.post_allocation_machine_optimization().is_none() {
            Ok(None)
        } else {
            Err(OptimizedResolvedSelectedFormLayoutError::OptimizationCustodyMismatch)
        };
    };
    let normalized = pre_layout
        .post_allocation_machine_optimization()
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OptimizationCustodyMismatch)?;
    let source = machine.machine().receipt().identity();
    let paired = match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(staged) => {
            let receipt = staged.custody();
            let actions = u64::try_from(receipt.action_count()).ok();
            normalized.optimization()
                == Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                && normalized.artifact_identity() == receipt.fusion().bytes()
                && normalized.selections() == receipt.selections()
                && normalized.post_allocation_machine_selections()
                    == receipt.post_allocation_machine_selections()
                && normalized.source() == receipt.source()
                && normalized.action_count() == receipt.action_count()
                && actions.and_then(|count| count.checked_mul(8))
                    == Some(normalized.baseline_bytes())
                && actions.and_then(|count| count.checked_mul(4))
                    == Some(normalized.selected_bytes())
        }
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(staged) => {
            let receipt = staged.custody();
            normalized.optimization()
                == Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                && normalized.artifact_identity() == receipt.materialization().bytes()
                && normalized.selections() == receipt.selections()
                && normalized.post_allocation_machine_selections()
                    == receipt.post_allocation_machine_selections()
                && normalized.source() == receipt.source()
                && normalized.action_count() == receipt.action_count()
                && receipt.baseline_words().checked_mul(4) == Some(normalized.baseline_bytes())
                && receipt.selected_words().checked_mul(4) == Some(normalized.selected_bytes())
        }
        StagedOptimizedPostAllocationMachineOptimization::X86XorZero(staged) => {
            let receipt = staged.custody();
            normalized.optimization() == Optimization::X86SelectXorZeroI64MaterializationV1
                && normalized.artifact_identity() == receipt.materialization().bytes()
                && normalized.selections() == receipt.selections()
                && normalized.post_allocation_machine_selections()
                    == receipt.post_allocation_machine_selections()
                && normalized.source() == receipt.source()
                && normalized.action_count() == receipt.action_count()
                && normalized.baseline_bytes() == receipt.baseline_bytes()
                && normalized.selected_bytes() == receipt.selected_bytes()
                && u64::try_from(receipt.action_count())
                    .ok()
                    .and_then(|count| count.checked_mul(7))
                    == normalized.expected_byte_savings()
        }
    };
    if !paired || normalized.source() != source {
        return Err(OptimizedResolvedSelectedFormLayoutError::OptimizationCustodyMismatch);
    }
    Ok(Some(normalized))
}

pub(super) fn validate_layout_byte_savings(
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    selected: &StagedOptimizedResolvedSelectedFormLayout,
    custody: PostAllocationMachineOptimizationCustody,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let baseline_bytes = layout_byte_count(baseline)?;
    let selected_bytes = layout_byte_count(selected)?;
    let expected = custody
        .expected_byte_savings()
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OptimizationByteSavingsMismatch)?;
    if baseline.selected() != selected.selected()
        || baseline.machine() != selected.machine()
        || baseline.target() != selected.target()
        || baseline.policy() != selected.policy()
        || baseline.functions().len() != selected.functions().len()
        || baseline.structural_unit_functions() != selected.structural_unit_functions()
        || baseline.post_allocation_machine_optimization().is_some()
        || selected.post_allocation_machine_optimization() != Some(custody)
        || !has_exact_byte_savings(baseline_bytes, selected_bytes, expected)
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::OptimizationByteSavingsMismatch);
    }
    Ok(())
}

fn layout_byte_count(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<u64, OptimizedResolvedSelectedFormLayoutError> {
    layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OptimizationByteSavingsMismatch)
        })
}

fn has_exact_byte_savings(baseline: u64, selected: u64, expected: u64) -> bool {
    baseline.checked_sub(selected) == Some(expected)
}

#[cfg(test)]
mod tests {
    use super::has_exact_byte_savings;

    #[test]
    fn x86_xor_zero_layout_accounts_for_exactly_seven_bytes_per_action() {
        assert!(has_exact_byte_savings(10, 3, 7));
        assert!(has_exact_byte_savings(23, 9, 14));
    }

    #[test]
    fn x86_xor_zero_layout_rejects_selected_byte_corruption() {
        assert!(!has_exact_byte_savings(10, 4, 7));
        assert!(!has_exact_byte_savings(10, 2, 7));
    }
}
