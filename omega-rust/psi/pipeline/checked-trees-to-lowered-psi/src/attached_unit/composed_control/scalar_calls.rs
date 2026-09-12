//! Source-selected scalar helper closure for composed Unit operands.

use super::*;
use checked_trees::CheckedCallScalarArgument;

pub(crate) use crate::scalar_call_closure::embedded::EmbeddedScalarCalls as ComposedScalarCalls;

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    internal_targets: &[catalogs::LoweredComposedInternalTarget],
) -> Result<ComposedScalarCalls, LoweringError> {
    let roots = selected_roots(checked, machine, states)?;
    let excluded_sources = std::iter::once(machine)
        .chain(internal_targets.iter().map(|target| target.source))
        .collect::<Vec<_>>();
    ComposedScalarCalls::prepare_computations(
        checked,
        &roots,
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
        for operation in &state.operations {
            if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } =
                operation
            {
                crate::attached_unit::structural_values::source_custody::validate(
                    checked,
                    machine,
                    state.state,
                    operation,
                )?;
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
                                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
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
                if let Some(root) = crate::structural_scalar_store_source::computation_root(
                    checked,
                    machine,
                    state.state,
                    store,
                )? {
                    pending.push(root);
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
                | CheckedUnitEffectOperationPlan::CallUnit {
                    scalar_arguments, ..
                } => scalar_arguments,
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_) => {
                    continue;
                }
                _ => return unsupported("composed scalar selection contains a non-call operation"),
            };
            crate::call_source_custody::validate_operation(
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
    crate::scalar_call_closure::embedded::computation_targets(checked, &roots)
}
