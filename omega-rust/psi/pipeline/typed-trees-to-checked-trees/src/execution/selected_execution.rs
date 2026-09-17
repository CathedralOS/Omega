//! Rebuild checked execution plans after exact provider settlement.
//!
//! Scalar-callee plans are borrowed inputs to Unit planning, not provisional
//! mutations of published facts. Fallible rebuilds publish only on success.

use crate::execution::execution_plans::{ExecutionPlans, build_execution_plans};
use checked_trees::CheckedTrees;

/// Exact compiler-owned join from one authored operator use to the checked
/// machine selected to realize it. Selected execution supplies these rows only
/// after ProviderPlan settlement; ordinary checking supplies none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOperatorApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// One compiler-intrinsic nearest IEEE FMA selected for an attached Unit
/// local initializer. This is intentionally disjoint from checked-body
/// operator adapters: no bodyless call or fabricated realization machine is
/// introduced into checked Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIeeeFloatFmaUnitApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub format: semantic_vocabulary::IeeeFloatFormat,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// Rebuild every checked Terminal plan whose exact shape depends on selected
/// operator execution. Unit-local scalar calls and direct structural-scalar
/// returns are one transaction so neither selected family can erase the
/// other's custody.
pub fn rebuild_checked_terminal_plans_with_selected_execution(
    program: &mut CheckedTrees,
    operator_applications: &[SelectedOperatorApplication],
    ieee_float_fma_applications: &[SelectedIeeeFloatFmaUnitApplication],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let call_frames = validation::CallFrameResolver::new(&program.typed);
    let ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    } = build_execution_plans(
        &program.typed,
        &program.facts,
        Some(&program.facts.flow.terminal_structural_scalar_returns),
        operator_applications,
        ieee_float_fma_applications,
        call_frames.as_ref(),
    );
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    program.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    program.facts.flow.terminal_unit_effects = unit_effects;
    program.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(())
}

/// Re-derive the retained state write frames from the settled typed program
/// after settlement rewrote authored bodies in place.
///
/// Checking retains an opaque frame for a state whose body calls a bodyless
/// boundary declaration; settlement replaces such a call with its selected
/// realization (an adapter call, a builtin, or an arithmetic expression), so
/// the settled body carries a derivable frame that the retained one no longer
/// describes. Terminal store planning compares the retained frame with the one
/// re-inferred from the settled body, so the retained facts follow the
/// settled program. Every refreshed frame must equal the retained frame or
/// refine a retained opaque frame; any other change is a settlement fault.
pub fn refresh_settled_state_write_frames(
    program: &mut CheckedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let Some(call_frames) = validation::CallFrameResolver::new(&program.typed) else {
        return Err(vec![diagnostics::Diagnostic::error(
            "selected execution settlement cannot re-infer state write frames: the settled typed program has no call frame resolver",
        )]);
    };
    let refreshed = settled_mutation_facts(&program.typed, &call_frames);
    let mut diagnostics = Vec::new();
    for (retained, refreshed) in program
        .facts
        .mutation
        .machines
        .iter()
        .zip(refreshed.machines.iter())
    {
        for (retained_state, refreshed_state) in retained
            .state_write_frames
            .iter()
            .zip(refreshed.state_write_frames.iter())
        {
            let retained_opaque =
                retained_state.frame.completeness() == facts::WriteFrameCompleteness::Opaque;
            if retained_state.frame == refreshed_state.frame || retained_opaque {
                continue;
            }
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected execution settlement changed the complete write frame retained for state `{}`: retained {:?}, settled {:?}",
                program.typed.symbols.display_path(retained_state.state, "::"),
                retained_state.frame.paths(),
                refreshed_state.frame.paths(),
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    program.facts.mutation = refreshed;
    Ok(())
}

/// The settled program's write frames in the retained fact shape: one entry
/// per machine in machine-table order, one frame per state in state order.
fn settled_mutation_facts(
    program: &typed_trees::TypedTrees,
    call_frames: &validation::CallFrameResolver<'_>,
) -> checked_trees::MutationFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| checked_trees::MachineMutationFact {
            machine: machine.symbol,
            state_write_frames: program
                .machine_states(machine)
                .iter()
                .zip(call_frames.inferred_machine_state_write_frames(machine))
                .map(|(state, frame)| checked_trees::StateWriteFramePlan {
                    state: state.symbol,
                    frame,
                })
                .collect(),
        })
        .collect();
    checked_trees::MutationFacts { machines }
}
