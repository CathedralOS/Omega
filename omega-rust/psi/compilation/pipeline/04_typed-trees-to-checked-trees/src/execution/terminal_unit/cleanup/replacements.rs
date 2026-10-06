//! Select an assignment's old-value death before its establishing store.
//!
//! The ownership producer has already chosen the exact live subtrees. Resolve
//! their ordinary state storage once and reuse the same action selection as
//! state exits and temporary continuations. A borrowed replacement displaces
//! the old value into a result; that changes its executable storage namespace,
//! not which original death occurrence supplies its contracts or action order.

use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallableEntry,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn actions(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    operation_index: usize,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let CheckedUnitEffectOperationPlan::ContinuationCleanup {
        boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { statement_index },
        cleanup_actions,
    } = operations.get(operation_index)?
    else {
        return None;
    };
    let [marker] = cleanup_actions.as_slice() else {
        return None;
    };
    let (store_statement, destination) = match operations.get(operation_index.checked_add(1)?)? {
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index,
            destination,
            ..
        }
        | CheckedUnitEffectOperationPlan::RebindStructuralLocal {
            statement_index,
            destination,
            ..
        } => (statement_index, destination),
        _ => return None,
    };
    if statement_index != store_statement {
        return None;
    }
    let source = &destination.source;
    let root = match *source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            PlaceRoot::Symbol(symbol)
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = parameters.get(parameter_index as usize)?;
            let declaration = crate::semantic::calls::find_state(program, state)?;
            PlaceRoot::Symbol(
                program
                    .state_parameters(declaration)
                    .get(parameter.position as usize)?
                    .symbol,
            )
        }
        _ => return None,
    };
    let displaced = marker.source() != source;
    if displaced {
        let CheckedUnitEffectOperationPlan::MoveStructuralField {
            result,
            source: moved,
        } = operations.get(operation_index.checked_sub(1)?)?
        else {
            return None;
        };
        if moved != destination
            || *marker.source()
                != (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                })
        {
            return None;
        }
    }
    let mut actions = Vec::new();
    for (_, disposition) in facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| {
            row.machine == machine
                && row.state == state
                && row.root == root
                && row.source
                    == PermissionEventSource::Statement {
                        statement_index: *statement_index as usize,
                    }
        })
    {
        let place = crate::flow::CanonicalPlace {
            root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(disposition.path)?
                .to_vec(),
        };
        let (_, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            program,
            state,
            *statement_index as usize,
            &place,
        )?;
        if !path.starts_with(&destination.path) {
            continue;
        }
        let mut selected = super::actions::disposition_actions(
            program,
            facts,
            targets,
            shapes,
            disposition,
            source.clone(),
        )?;
        if displaced {
            for action in &mut selected {
                let (source, path) = match action {
                    CheckedNormalCleanupAction::Discard(action) => {
                        (&mut action.source, &mut action.path)
                    }
                    CheckedNormalCleanupAction::InvokeNominal(action) => {
                        (&mut action.source, &mut action.path)
                    }
                };
                *path = path.strip_prefix(destination.path.as_slice())?.to_vec();
                *source = marker.source().clone();
            }
        }
        actions.extend(selected);
    }
    // An occupied affine value cannot silently become a raw discard when its
    // source death is absent. The producer must supply that occurrence first.
    (!actions.is_empty()).then_some(actions)
}
