//! Rejoin the exact temporary owner and its normal call cleanup.

use super::*;
use checked_trees::CheckedStructuralAccess;

pub(crate) fn validate_cleanup(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation_index: usize,
) -> Result<(), LoweringError> {
    let Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards,
    }) = caller.operations.get(operation_index)
    else {
        return unsupported("call continuation cleanup entry is absent");
    };
    let consumer = operation_index
        .checked_sub(1)
        .and_then(|previous| caller.operations.get(previous))
        .ok_or(LoweringError::Unsupported(
            "call cleanup has no preceding consumer",
        ))?;
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate: call,
        structural_arguments,
        scalar_arguments,
        ..
    } = consumer
    else {
        return unsupported("call cleanup does not immediately follow its consumer");
    };
    let [argument] = structural_arguments.as_slice() else {
        return unsupported("call cleanup requires one exact temporary argument");
    };
    if call != coordinate || coordinate.call_ordinal != 0 || !scalar_arguments.is_empty() {
        return unsupported("call cleanup substituted its consumer");
    }
    let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
        return unsupported("call cleanup requires an expression-owned result");
    };
    let mut producers = caller.operations[..operation_index]
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => {
                Some((operation, *coordinate, result, *discard_result_on_return))
            }
            _ => None,
        });
    let (producer_operation, producer, result, discard_on_return) = producers.next().ok_or(
        LoweringError::Unsupported("call cleanup has no result producer"),
    )?;
    if producers.next().is_some()
        || discard_on_return
        || producer.call_ordinal != 1
        || producer.statement_index != coordinate.statement_index
        || result.statement_index != coordinate.statement_index
        || result.multiplicity != language_semantics::Multiplicity::Affine
    {
        return unsupported("call cleanup has no unique continuing owner");
    }
    match argument.access {
        CheckedStructuralAccess::SharedBorrow if argument.path.is_empty() => {
            let [discard] = affine_discards.as_slice() else {
                return unsupported("shared call cleanup requires the intact owner");
            };
            if discard.source != argument.source
                || !discard.path.is_empty()
                || discard.type_identity != result.type_identity
                || argument.type_identity != result.type_identity
            {
                return unsupported("shared call cleanup substituted its result");
            }
            let source = crate::call_source_custody::authored::locate_source(
                checked,
                caller.state,
                producer,
            )?;
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                source.source_site
            else {
                return unsupported("call cleanup lost its expression-owned source");
            };
            shared_temporary::validate(checked, caller, producer, *coordinate, expression)
        }
        CheckedStructuralAccess::Owned if !argument.path.is_empty() => {
            let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
            if !matches!(
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(coordinate.statement_index as usize),
                Some(StatementNode::Call(_))
            ) {
                return unsupported("partial cleanup has no authored call statement");
            }
            let expected = crate::unit_cleanup::checked_partial_affine_residuals(
                &checked.facts.flow.terminal_unit_effects.structural_types,
                &argument.source,
                &result.type_identity,
                &[(argument.path.as_slice(), argument.type_identity.as_str())],
                affine_discards.len(),
            )?;
            if expected != *affine_discards {
                return unsupported("partial call continuation residual partition drifted");
            }
            crate::unit_cleanup::validate_anonymous_partial_permissions(
                checked,
                caller,
                producer_operation,
                consumer,
                affine_discards,
            )
        }
        _ => unsupported("call cleanup has no owned projection or whole shared loan"),
    }
}
