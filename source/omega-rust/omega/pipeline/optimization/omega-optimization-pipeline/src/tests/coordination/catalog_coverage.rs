use omega_machine_optimizer::ORDERED_POST_ALLOCATION_MACHINE_RULES;
use omega_optimization_core::{Optimization, OptimizationExecutionPhase};
use omega_psi_optimizer::ORDERED_PSI_PASSES;
use omega_regalloc::{ORDERED_ALLOCATION_RECOVERY_RULES, ORDERED_SELECTED_LOWERING_RULES};

use crate::ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES;

#[test]
fn every_exact_optimization_has_one_rule_stage_disposition() {
    let catalogs: [(OptimizationExecutionPhase, &[Optimization]); 5] = [
        (OptimizationExecutionPhase::Psi, &ORDERED_PSI_PASSES),
        (
            OptimizationExecutionPhase::SelectedLowering,
            &ORDERED_SELECTED_LOWERING_RULES,
        ),
        (
            OptimizationExecutionPhase::AllocationRecovery,
            &ORDERED_ALLOCATION_RECOVERY_RULES,
        ),
        (
            OptimizationExecutionPhase::PostAllocationMachine,
            &ORDERED_POST_ALLOCATION_MACHINE_RULES,
        ),
        (
            OptimizationExecutionPhase::FunctionRelativeLayout,
            &ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES,
        ),
    ];

    for optimization in Optimization::ALL {
        let dispositions = catalogs
            .iter()
            .flat_map(|(phase, catalog)| {
                catalog
                    .iter()
                    .copied()
                    .filter(move |scheduled| *scheduled == optimization)
                    .map(move |scheduled| (*phase, scheduled))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dispositions,
            vec![(optimization.execution_phase(), optimization)],
            "{optimization:?} must occur once in its declared phase catalog"
        );
    }

    let scheduled_count = catalogs
        .iter()
        .map(|(_, catalog)| catalog.len())
        .sum::<usize>();
    assert_eq!(scheduled_count, Optimization::ALL.len());
}
