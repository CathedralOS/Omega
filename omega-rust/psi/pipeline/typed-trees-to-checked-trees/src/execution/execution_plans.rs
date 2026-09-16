//! Build dependent execution plans without publishing intermediate checked facts.

use crate::{SelectedIeeeFloatFmaUnitApplication, SelectedOperatorApplication};
use checked_trees::{
    CheckFacts, CheckedBoundaryScalarReturnPlans, CheckedStructuralScalarReturnPlans,
    CheckedUnitEffectPlans,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) struct ExecutionPlans {
    pub boundary_returns: CheckedBoundaryScalarReturnPlans,
    pub unit_effects: CheckedUnitEffectPlans,
    pub structural_scalar_returns: CheckedStructuralScalarReturnPlans,
    pub cleanup_diagnostics: Vec<Diagnostic>,
}

/// Independent returns precede Unit closure; complete structural returns depend
/// on that closure. Initial checking has no previous roster. Selected rebuilding
/// preserves nominal and selected callees while refreshing primitive bodies.
/// Diagnostics remain owned output so initial checking can aggregate its later
/// affine-cleanup failures before deciding whether to publish checked trees.
pub(crate) fn build_execution_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    previous_returns: Option<&CheckedStructuralScalarReturnPlans>,
    operator_applications: &[SelectedOperatorApplication],
    ieee_float_fma_applications: &[SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> ExecutionPlans {
    let boundary_returns =
        crate::execution::terminal_unit::returns::build_checked_boundary_scalar_return_plans(
            program, facts,
        );
    let primitive_returns =
        crate::execution::terminal_unit::returns::build_checked_primitive_store_scalar_return_plans(
            program, facts,
        );
    let structural_callees = match previous_returns {
        Some(previous) => {
            crate::execution::terminal_unit::returns::reconcile_primitive_store_scalar_returns(
                previous,
                primitive_returns,
            )
        }
        None => primitive_returns,
    };
    let scalar_callees = crate::execution::terminal_unit::ScalarCalleePlans {
        boundary_returns: &boundary_returns,
        structural_returns: &structural_callees,
    };
    let unit_effects =
        crate::execution::terminal_unit::build_checked_unit_effect_plans_with_call_frames(
            program,
            facts,
            scalar_callees,
            operator_applications,
            ieee_float_fma_applications,
            call_frames,
        );
    let mut cleanup_diagnostics = Vec::new();
    let structural_scalar_returns =
        crate::execution::terminal_unit::returns::build_checked_structural_scalar_return_plans(
            program,
            facts,
            &unit_effects,
            operator_applications,
            &mut cleanup_diagnostics,
        );
    ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    }
}
