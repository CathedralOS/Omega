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

pub(super) fn actions(
    program: &dyn ::validation::CheckingInput,
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
        boundary:
            checked_trees::CheckedUnitCleanupBoundary::Replacement {
                statement_index,
                destination,
                old_value,
                ..
            },
        ..
    } = operations.get(operation_index)?
    else {
        return None;
    };
    let source = &destination.source;
    let root = match *source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            PlaceRoot::Symbol(symbol)
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = parameters.get(parameter_index as usize)?;
            let declaration = program.state_by_symbol(state)?;
            PlaceRoot::Symbol(
                program
                    .state_parameters(declaration)
                    .get(parameter.position as usize)?
                    .symbol,
            )
        }
        _ => return None,
    };
    let displaced = old_value.source != *source;
    let replaced_place = match program.state_by_symbol(state).and_then(|declaration| {
        program
            .statement_table
            .statements(declaration.statement_nodes)
            .get(*statement_index as usize)
    }) {
        Some(typed_trees::statement::StatementNode::Assignment(assignment)) => {
            validation::canonical_place_from_expression_in_state(
                program,
                state,
                *statement_index as usize,
                assignment.target,
            )
        }
        _ => None,
    };
    // The operand keeps the immediate loan carrier, while the permission
    // producer may name its backing as the dying storage subject. These are
    // two coordinates of this one replacement, not alternative cleanup plans.
    let stored_place = replaced_place.as_ref().and_then(|place| {
        let declaration = program.state_by_symbol(state)?;
        let (root, segments) = super::super::borrowed_windows::resolve_storage_place(
            program,
            declaration,
            program
                .statement_table
                .statements(declaration.statement_nodes),
            *statement_index as usize,
            place.root.as_symbol()?,
            &place.segments,
        );
        Some(validation::CanonicalPlace { root, segments })
    });
    let mut actions = Vec::new();
    for (_, disposition) in facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| {
            row.machine == machine
                && row.state == state
                && (row.root == root
                    || replaced_place
                        .as_ref()
                        .is_some_and(|replaced| replaced.root == row.root)
                    || stored_place
                        .as_ref()
                        .is_some_and(|stored| stored.root == row.root))
                && row.source
                    == PermissionEventSource::Statement {
                        statement_index: *statement_index as usize,
                    }
        })
    {
        let place = validation::CanonicalPlace {
            root: disposition.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(disposition.path)?
                .to_vec(),
        };
        // The death of exactly the assigned place is the old value this store
        // displaces, whose path the store already names, including a
        // runtime-selected element that has no static argument path.
        // A store through a view writes the view's backing, while the old
        // element dies at the view's own place; only that exact death crosses
        // namespaces, as the store's destination.
        let replaced =
            replaced_place.as_ref() == Some(&place) || stored_place.as_ref() == Some(&place);
        if disposition.root != root && !replaced {
            continue;
        }
        let path = if replaced {
            destination.path.clone()
        } else {
            crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                state,
                *statement_index as usize,
                &place,
            )?
            .1
        };
        // A death can cover the replaced place or an enclosing value: a
        // whole-array store composes one displacing store per element, while
        // the old array dies once at the statement. Each store takes the
        // death's actions within its own place; sibling elements' actions
        // belong to their own stores.
        if !path.starts_with(&destination.path) && !destination.path.starts_with(&path) {
            continue;
        }
        for mut action in super::actions::disposition_actions(
            program,
            facts,
            targets,
            shapes,
            disposition,
            source.clone(),
            replaced.then_some(destination.path.as_slice()),
        )? {
            let action_path = match &action {
                CheckedNormalCleanupAction::Discard(action) => &action.path,
                CheckedNormalCleanupAction::InvokeNominal(action) => &action.path,
                CheckedNormalCleanupAction::RetireHome { .. } => return None,
            };
            if !action_path.starts_with(&destination.path) {
                if !destination.path.starts_with(action_path) {
                    continue;
                }
                // A discard of an enclosing value discards this place within
                // it. An enclosing hook observes the whole value and cannot
                // run piecewise.
                let CheckedNormalCleanupAction::Discard(_) = action else {
                    return None;
                };
                action = CheckedNormalCleanupAction::Discard(
                    checked_trees::CheckedUnitPartialAffineDiscardPlan {
                        source: source.clone(),
                        path: destination.path.clone(),
                        type_identity: destination.type_identity.clone(),
                        case_conditions: action.case_conditions(),
                    },
                );
            }
            if displaced {
                let conditions = shapes
                    .cleanup_control
                    .conditions
                    .span_or_empty(action.case_conditions())
                    .to_vec();
                let conditions = conditions
                    .into_iter()
                    .map(|mut condition| {
                        let path = shapes.cleanup_control.paths.span_or_empty(condition.path);
                        let path = path.strip_prefix(destination.path.as_slice())?.to_vec();
                        condition.path = shapes.cleanup_control.paths.insert_many(path);
                        Some(condition)
                    })
                    .collect::<Option<Vec<_>>>()?;
                let case_conditions = shapes.cleanup_control.conditions.insert_many(conditions);
                let (source, path) = match &mut action {
                    CheckedNormalCleanupAction::Discard(action) => {
                        action.case_conditions = case_conditions;
                        (&mut action.source, &mut action.path)
                    }
                    CheckedNormalCleanupAction::InvokeNominal(action) => {
                        action.case_conditions = case_conditions;
                        (&mut action.source, &mut action.path)
                    }
                    CheckedNormalCleanupAction::RetireHome { .. } => return None,
                };
                *path = path.strip_prefix(destination.path.as_slice())?.to_vec();
                *source = old_value.source.clone();
            }
            actions.push(action);
        }
    }
    // An occupied affine value cannot silently become a raw discard when its
    // source death is absent. The producer must supply that occurrence first.
    if actions.is_empty() {
        return None;
    }
    // The authenticated replacement join distinguishes the displaced value's
    // home from the retained containing record. A root rebind also ends its old
    // occurrence; a field store only ends that selected field value.
    if displaced || destination.path.is_empty() {
        super::actions::finish_home(
            &mut actions,
            CheckedNormalCleanupAction::RetireHome {
                source: old_value.source.clone(),
                type_identity: destination.type_identity.clone(),
            },
        );
    }
    Some(actions)
}
