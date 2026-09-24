//! Source-selected scalar helper closure for composed Unit operands.
use super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, unsupported};
use super::{CheckedTrees, LoweringError, catalogs};
use checked_trees::CheckedCallScalarArgument;

pub(crate) use crate::scalar_graph::scalar_call_closure::embedded::EmbeddedScalarCalls as ComposedScalarCalls;

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    internal_targets: &[catalogs::LoweredComposedInternalTarget],
) -> Result<ComposedScalarCalls, LoweringError> {
    let targets = selected_targets(checked, machine, states)?;
    let excluded_sources = std::iter::once(machine)
        .chain(internal_targets.iter().map(|target| target.source))
        .collect::<Vec<_>>();
    ComposedScalarCalls::prepare_targets(
        checked,
        &targets,
        &excluded_sources,
        internal_targets
            .len()
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "composed Unit machine count overflows usize",
            ))?,
    )
}

fn selected_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<checked_trees::CheckedScalarComputationHandle>, LoweringError> {
    let mut pending = Vec::new();
    for state in states {
        for edge in super::state_graph::successors(state) {
            for argument in &edge.scalar_arguments {
                if matches!(
                    argument.source,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                ) && let CheckedCallScalarArgument::Computation(root) =
                    super::state_graph::scalars::successor_value(checked, state, edge, argument)?
                {
                    pending.push(root);
                }
            }
        }
        if let CheckedComposedUnitControlTerminatorPlan::Guarded { arms, .. } = &state.terminator {
            let guards = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .guarded_exits
                .span(*arms)
                .ok_or(LoweringError::Unsupported(
                    "guarded computation roster is stale",
                ))?;
            for guard in guards {
                if let Some(root) = checked.facts.values.scalar_computations.root_at(
                    state.state,
                    guard.guard_statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                ) {
                    if root.machine != machine {
                        return unsupported("ordered guard changed its computation owner");
                    }
                    pending.push(root.root);
                }
            }
        }
        // Two-way and ordered graph guards name their computation root on the
        // plan; admission rejoins that handle to its unique Guard coordinate.
        match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                guard: CheckedCallScalarArgument::Computation(root),
                ..
            }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                guard: CheckedCallScalarArgument::Computation(root),
                ..
            } => pending.push(*root),
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } => {
                pending.extend(arms.iter().filter_map(|arm| match arm.guard {
                    CheckedCallScalarArgument::Computation(root) => Some(root),
                    CheckedCallScalarArgument::Pure(_) => None,
                }));
            }
            _ => {}
        }
        for operation in state.operation_dependencies() {
            if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result, calls, ..
            } = operation
            {
                crate::expression_preparation::source_custody::structural::validate(
                    checked,
                    machine,
                    state.state,
                    operation,
                )?;
                // A member call's scalar arguments are computations in the
                // same statement; only their roots join the pending set, the
                // member's statement custody belongs to the construction.
                for call in calls {
                    let CheckedUnitEffectOperationPlan::StructuralCall {
                        scalar_arguments, ..
                    } = call.operation()
                    else {
                        return unsupported("composed scalar selection lost its value call");
                    };
                    for argument in scalar_arguments {
                        let CheckedCallScalarArgument::Computation(handle) = argument else {
                            continue;
                        };
                        pending.push(*handle);
                    }
                }
                pending.extend(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .roots
                        .iter()
                        .map(|(_, root)| root)
                        .filter(|root| {
                            root.machine == machine
                                && root.state == state.state
                                && root.statement_ordinal == result.statement_index
                                && matches!(
                                    root.role,
                                    CheckedScalarExpressionRole::StructuralValueSubject { .. }
                                        | CheckedScalarExpressionRole::RecordField { .. }
                                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                                        | CheckedScalarExpressionRole::StructuralValueField { .. }
                                )
                        })
                        .map(|root| root.root),
                );
                continue;
            }
            if let CheckedUnitEffectOperationPlan::EstablishScalarLocal { value, .. } = operation {
                if let CheckedCallScalarArgument::Computation(handle) = value {
                    pending.push(*handle);
                }
                continue;
            }
            if let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = operation {
                if let Some((root, _)) =
                    crate::emission::structural_scalar_store_source::computation_root(
                        checked,
                        machine,
                        state.state,
                        store,
                    )?
                {
                    pending.push(root);
                }
                continue;
            }
            if let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
                value, ..
            } = operation
            {
                if let CheckedCallScalarArgument::Computation(handle) = value {
                    pending.push(*handle);
                }
                continue;
            }
            let arguments = match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    scalar_arguments, ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    scalar_arguments, ..
                }
                | CheckedUnitEffectOperationPlan::ScalarCall {
                    scalar_arguments, ..
                }
                | CheckedUnitEffectOperationPlan::CallUnit {
                    scalar_arguments, ..
                } => scalar_arguments,
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                // Whole structural moves and stores evaluate no scalar.
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                // A view-subslice local replays its own endpoints at emission.
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                    continue;
                }
                _ => return unsupported("composed scalar selection contains a non-call operation"),
            };
            crate::emission::call_source_custody::validate_operation(
                checked,
                machine,
                state.state,
                operation,
                &state.structural_parameters,
            )?;
            for argument in arguments {
                let CheckedCallScalarArgument::Computation(handle) = argument else {
                    continue;
                };
                pending.push(*handle);
            }
        }
    }
    Ok(pending)
}

pub(super) fn selected_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let roots = selected_roots(checked, machine, states)?;
    let mut targets =
        crate::scalar_graph::scalar_call_closure::embedded::computation_targets(checked, &roots)?;
    // A statement-level scalar call carries its callee on the operation, not
    // inside an operand computation.
    for operation in states
        .iter()
        .flat_map(|state| state.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        if let CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } = operation
            && !targets.contains(target_machine)
        {
            targets.push(*target_machine);
        }
    }
    Ok(targets)
}
