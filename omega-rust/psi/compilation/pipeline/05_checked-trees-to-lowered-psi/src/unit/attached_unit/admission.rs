//! Admit the complete operation-body roster before allocating shared identities.
//!
//! The result retains each checked source alongside its admitted body. Emission
//! consumes that roster in order instead of searching and removing composed bodies.

use super::super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    unsupported,
};
mod calls;
mod operations;
pub(super) use calls::{CallerView, admit as admit_call};

use super::bodies::{UnitBody, UnitPlans};
use super::composed_control::{self, callable::CallableBody};
use super::{
    retain_admitted_unit_boundary, retain_exact_unit_boundary, structural_completion,
    validate_unit_operation_sequence,
};
use checked_trees::{CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan};
use symbols::SymbolHandle;

pub(super) enum AdmittedBody<'a> {
    Ordinary(&'a CheckedUnitEffectMachinePlan),
    Composed {
        source: &'a CheckedComposedUnitControlMachinePlan,
        body: CallableBody<'a>,
    },
}

impl<'a> AdmittedBody<'a> {
    pub(super) fn source(&self) -> UnitBody<'a> {
        match self {
            Self::Ordinary(plan) => UnitBody::Ordinary(plan),
            Self::Composed { source, .. } => UnitBody::Composed(source),
        }
    }
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    closure: &[SymbolHandle],
    scalar_closure: &[SymbolHandle],
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<Vec<AdmittedBody<'a>>, LoweringError> {
    // Computation roots belong to both operation bodies and scalar graphs.
    // Both use the same exact boundary roster; the body's return shape must
    // not decide whether a nested application can resolve its declaration.
    for source in closure.iter().chain(scalar_closure) {
        retain_nested_boundary_applications(checked, plans, *source, boundaries)?;
    }
    let mut bodies = Vec::with_capacity(closure.len());
    for source in closure {
        let admitted = match UnitBody::find(plans, *source)? {
            UnitBody::Composed(plan) => {
                let body = composed_control::admit_callable(checked, plan)?;
                // `admit_callable` matched each call against its exact
                // target; the closure roster retains those targets.
                for (boundary, _) in body.boundaries() {
                    retain_admitted_unit_boundary(checked, plans, boundaries, boundary)?;
                }
                AdmittedBody::Composed { source: plan, body }
            }
            UnitBody::Ordinary(plan) => {
                validate_ordinary(checked, plan)?;
                operations::validate(checked, plans, plan, closure, scalar_closure, boundaries)?;
                AdmittedBody::Ordinary(plan)
            }
        };
        bodies.push(admitted);
    }
    Ok(bodies)
}

/// A boundary application nested under another operand is a computation
/// node, not a statement operation, so the operation scan never names its
/// target. Register each such target in the closure's boundary roster with the
/// same exact target, contract, result and reach checks a statement call gets;
/// emission then resolves the lowered declaration by that target.
fn retain_nested_boundary_applications<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    machine: SymbolHandle,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
        .map(|(_, root)| root.root)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Ok(());
    }
    for node in crate::expression_preparation::computation_graph::reachable_nodes(checked, &roots)?
    {
        let checked_trees::CheckedScalarComputationKind::Call {
            source_call,
            callee:
                checked_trees::CheckedScalarCallee::Boundary {
                    machine: target_machine,
                    state: target_state,
                },
            ..
        } = computations.nodes.get(node).kind
        else {
            continue;
        };
        let control = &checked.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return unsupported("nested boundary application lost its checked invocation");
        }
        // A declared boundary machine owns its machine contract. A requirement
        // retains its declared contract owner, which may be a boundary trait.
        let boundary = super::unique_unit_boundary(plans, target_machine)?;
        let fingerprint = checked
            .facts
            .contract_plans
            .for_machine(target_machine)
            .map(|contract| contract.report_fingerprint)
            .or_else(|| {
                checked
                    .facts
                    .contract_plans
                    .crash_capsule(boundary.contract_owner, target_state)
                    .map(|capsule| capsule.target_contract_report_fingerprint())
            })
            .ok_or(LoweringError::Unsupported(
                "nested boundary application target has no checked contract",
            ))?;
        retain_exact_unit_boundary(
            checked,
            plans,
            boundaries,
            target_machine,
            target_state,
            fingerprint,
            control.calls.get(source_call).service_reach,
            checked_trees::CheckedBoundaryMachineResultPlan::Scalar(
                computations.nodes.get(node).primitive_type,
            ),
        )?;
    }
    Ok(())
}

fn validate_ordinary(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    if machine.contract_report_fingerprint == 0 {
        return unsupported("Unit closure contains a null checked contract fingerprint");
    }
    if checked
        .facts
        .contract_plans
        .for_machine(machine.machine)
        .is_none()
    {
        return unsupported("Unit closure is missing its canonical checked contract");
    }
    validate_unit_operation_sequence(checked, machine)?;
    // The checked nominal plan owns its empty completion wherever the machine
    // occurs in a closure. Its exact plan identity, not entry selection, grants
    // the shared cleanup finalizer authority to install the return actions.
    let synthetic_cleanup = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .iter()
        .any(|nominal| nominal.machine == *machine);
    if !synthetic_cleanup {
        if machine.scalar_result.is_some() || machine.scalar_control.is_some() {
        } else {
            structural_completion::validate(checked, machine)?;
        }
    }
    validate_linear_transition_calls(checked, machine)?;

    Ok(())
}

/// A linear roster's own call at a named transition is only the unconditional
/// final invocation. A guarded arm's call runs on its selected arm alone, so a
/// linear roster holding one would run it whichever arm is taken.
fn validate_linear_transition_calls(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    use checked_trees::statement::{StatementNode, TransitionTargetNode};
    let (_, state) = crate::declarations::state_declaration(checked, machine.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    for call in machine
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
    {
        let Some(StatementNode::Transition(transition)) =
            statements.get(call.coordinate.statement_index as usize)
        else {
            continue;
        };
        if call.coordinate.call_ordinal == 0
            && matches!(
                checked.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { .. }
            )
            && !crate::call_sites::linear_terminal_invocation(
                checked,
                machine.state,
                call.coordinate.statement_index,
            )?
        {
            return unsupported("Unit body sequences a guarded arm's terminal call");
        }
    }
    Ok(())
}
