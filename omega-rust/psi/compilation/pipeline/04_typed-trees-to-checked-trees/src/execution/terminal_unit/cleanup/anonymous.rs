//! Anonymous argument owners die at the consuming call's normal continuation.
//! The nested schedule establishes each owner once; a projected argument names
//! its subtree without establishing another owner. Borrowed owners retire whole,
//! while partial transfers retire the maximal untouched complement, in reverse
//! establishment order. Captured reference descriptors use the shared loan
//! retirement schedule, which keeps them live through returned descendants.
//! Loan capture records retained ancestry, not a completed physical release.
//! Normal completion retires each eligible descriptor before its backing;
//! a later loan schedule observes that actual event rather than duplicating it.
//! Checking selects the dying owner's complement once. Value planning retains
//! that home's completion request; action finalization consumes the contextual
//! disposition after executable hook targets exist. It does not reconstruct a
//! second permission partition or dispose a selected live linear obligation.
use super::super::CheckedUnitStructuralResultBindingPlan;
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PermissionEventSource,
};
use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity};
use checked_trees::CheckedNormalCleanupAction;

/// Retain the original anonymous owner's completion request. Finalization
/// resolves its checked contextual disposition in the actual call namespace;
/// a selected subtree never becomes a second whole temporary.
pub(crate) fn projected_temporary_cleanup(
    program: &dyn ::validation::CheckingInput,
    owner_type: typed_trees::types::TypeReferenceHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } = argument.source
    else {
        return None;
    };
    if argument.access != CheckedStructuralAccess::Owned
        || argument.path.is_empty()
        || program.type_multiplicity(owner_type) != Multiplicity::Affine
        || !validation::has_contents(
            program,
            owner_type,
            &[],
            validation::ContentsRequirement::BorrowFree,
        )
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let identity = shapes.add_type(owner_type, &[], &[])?;
    Some(vec![CheckedNormalCleanupAction::RetireHome {
        source: argument.source.clone(),
        type_identity: identity,
    }])
}

/// Dispose anonymous argument owners at the call's normal continuation.
/// The argument's access determines whether the whole owner or its maximal
/// complement dies. Captured reference descriptors instead keep their one
/// release in the loan schedule, including any returned descendant's lifetime.
/// Results follow establishment order, so reverse traversal also orders mixed
/// borrowed owners and partially transferred owners without regrouping them.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn insert_continuation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    insertion: usize,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    trace: &super::super::control::LocalConstructionTrace,
) -> Option<()> {
    let invocation = operations
        .get(..insertion)?
        .iter()
        .rev()
        .find(|operation| operation.invocation_coordinate() == Some(coordinate))?
        .clone();
    let completions = normal_call_completion(
        program,
        facts,
        shapes,
        machine,
        state,
        results,
        operations,
        &invocation,
        trace,
    )?;
    // Selection observes the complete planned namespace, including exact
    // result consumption. Execution belongs to the captured call continuation,
    // before an enclosing assignment can displace storage lent to this call.
    // The callee may have installed a new value there: end its temporary loan
    // before the enclosing replacement disposes those current contents.
    operations.splice(insertion..insertion, completions);
    Some(())
}

/// Flat statement calls and nested value calls publish the same normal-return
/// transaction: release eligible operand descriptors, then dispose the exact
/// backing owners selected by checking. The result's retained inputs stay live.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn normal_call_completion(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
    invocation: &CheckedUnitEffectOperationPlan,
    trace: &super::super::control::LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let coordinate = invocation.invocation_coordinate()?;
    let completion_phase = |phase| {
        trace.phase(phase);
        trace.statement(Some(coordinate.statement_index));
    };
    let Some(call) = invocation.call() else {
        // Register invocations admit scalar operands only; they have no
        // descriptor or owned operand continuation to complete.
        return invocation.structural_operands().is_empty().then(Vec::new);
    };
    if !call.has_normal_continuation() {
        // Preserve operand custody in the suspended caller. Absence of return
        // grants neither descriptor death nor borrowed-storage restoration.
        return Some(Vec::new());
    }
    completion_phase("normal completion: operand descriptors");
    let mut completions = super::temporary_descriptors::completions(
        program,
        facts,
        machine.symbol,
        state.symbol,
        results,
        call,
        operations,
        shapes,
    )?;

    // Capture recognition preserves ancestry; it is not evidence that this
    // carrier has already ended. The descriptor owner above has checked input
    // retention and bound the exact loan group. Publish its selected physical
    // release here, before backing hooks, unless that same home already ended.
    let mut selected_descriptors = Vec::new();
    for completion in completions {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            source:
                source @ CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
            ..
        } = &completion
        else {
            return None;
        };
        if operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference {
                source: released, path, defer_to_exit: false, ..
            } if released == source && path.is_empty())
            })
        {
            continue;
        }
        // Every carrier has a producer, but direct reference/view formation
        // has no automatic result-discard flag. Its explicit release is the
        // entire disposition; only producers with an automatic death need it
        // withdrawn here.
        {
            let mut producers = operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                .filter(|binding| binding.binding_ordinal == *binding_ordinal);
            producers.next()?;
            if producers.next().is_some() {
                return None;
            }
        }
        for operation in operations.iter_mut() {
            if let Some((_, discard)) =
                operation.discardable_structural_result_by_ordinal_mut(*binding_ordinal)
            {
                *discard = false;
            }
        }
        selected_descriptors.push(completion);
    }
    completions = selected_descriptors;
    let statement = coordinate.statement_index;
    let values = &facts.values.structural_values;
    let mut dying = Vec::new();
    facts.contextual_cleanup.for_each_state_disposition(
        machine.symbol,
        state.symbol,
        |_, disposition| {
            if !matches!(
                disposition.source,
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    ..
                } if statement_index == statement as usize
                    && call_ordinal == coordinate.call_ordinal as usize
            ) {
                return;
            }
            let Some(expression) = disposition.root.as_expression() else {
                return;
            };
            if !dying.contains(&expression) {
                dying.push(expression);
            }
        },
    );
    if dying.is_empty() {
        return Some(completions);
    }

    // Completed value dependencies and captured lifetime roots authenticate
    // storage independently. Neither relation selects an additional death.
    let mut retained = Vec::new();
    for argument in invocation.structural_operands() {
        let root = if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
            let mut bindings = results
                .iter()
                .filter(|(result, _)| result.binding_ordinal == ordinal);
            let (_, root) = bindings.next()?;
            if bindings.next().is_some() {
                return None;
            }
            *root
        } else if let Some(root) = argument.structural_storage_root() {
            root
        } else {
            continue;
        };
        let Some(expression) = root.as_expression() else {
            continue;
        };
        let Some(root) = values.root_for_expression(state.symbol, statement, expression) else {
            continue;
        };
        retained.extend(values.value_dependencies(root.root)?);
    }
    let flow = facts.flow.control.state(machine.symbol, state.symbol)?;
    let mut source_calls = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|source| {
            source.statement_index == statement as usize
                && source.call_ordinal == coordinate.call_ordinal as usize
        });
    let source_call = source_calls.next()?;
    if source_calls.next().is_some() {
        return None;
    }
    let mut retained_owners = Vec::new();
    for operand in facts.flow.control.operands.span(source_call.operands)? {
        for root in crate::flow::captured_lifetime_roots(program, &facts.flow.control, operand)? {
            if !retained_owners.contains(&root) {
                retained_owners.push(root);
            }
        }
    }
    let (_, statement_node) = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .find(|(position, _)| *position == statement as usize)?;
    let completed = crate::checks::ownership_occurrences::completed_expressions(
        program,
        machine,
        state,
        statement as usize,
        statement_node,
        &facts.operators,
    );
    if dying.iter().any(|expression| {
        completed
            .iter()
            .filter(|value| *value == expression)
            .count()
            != 1
    }) {
        return None;
    }
    dying.sort_by_key(|expression| {
        std::cmp::Reverse(completed.iter().position(|value| value == expression))
    });
    let mut cleanup_actions = Vec::new();
    for expression in dying {
        completion_phase("normal completion: owner value graph");
        let root = values.root_for_expression(state.symbol, statement, expression)?;
        completion_phase("normal completion: owner retained association");
        if root.machine != machine.symbol
            || (!retained.contains(&root.root)
                && !retained_owners.contains(&facts::PlaceRoot::Expression(expression)))
        {
            return None;
        }
        completion_phase("normal completion: owner result binding");
        let mut bindings = results.iter().filter(|(result, source)| {
            result.statement_index == statement
                && *source == facts::PlaceRoot::Expression(expression)
        });
        let source = if let Some((result, _)) = bindings.next() {
            if bindings.next().is_some() {
                return None;
            }
            let source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            };
            // A physical carrier release already owns this whole disposition.
            // Neither a matching loan group on another carrier nor a planned
            // exit release can stand in for this exact completed home.
            if operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .chain(completions.iter())
                .any(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference {
                        source: released, path, defer_to_exit: false, ..
                    } if *released == source && path.is_empty())
                })
            {
                continue;
            }
            completion_phase("normal completion: owner producer lookup");
            {
                // Binding identities span the whole value evaluation. A nested
                // argument owns the same disposition as a flat producer; keep
                // the exact-once check over that complete inventory before
                // asking its existing ordinal owner to change the flag.
                let mut producers = operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .filter_map(CheckedUnitEffectOperationPlan::discardable_structural_result)
                    .filter(|(produced, _)| produced.binding_ordinal == result.binding_ordinal);
                let (produced, _) = producers.next()?;
                completion_phase("normal completion: owner producer agreement");
                if producers.next().is_some() || *produced != *result {
                    return None;
                }
            }
            let (_, discard_on_return) = operations.iter_mut().find_map(|operation| {
                operation.discardable_structural_result_by_ordinal_mut(result.binding_ordinal)
            })?;
            // Scheduling metadata is not custody. A selected contextual death
            // remains authoritative even without an automatic return discard.
            *discard_on_return = false;
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            }
        } else {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression }
        };
        completion_phase("normal completion: owner prior continuation");
        if operations.iter().flat_map(CheckedUnitEffectOperationPlan::with_value_calls).any(|operation| matches!(operation,
            CheckedUnitEffectOperationPlan::ContinuationCleanup { cleanup_actions, .. }
                if cleanup_actions.iter().any(|action| action.source() == &source && action.path().is_empty())))
        { return None; }
        completion_phase("normal completion: owner cleanup type");
        cleanup_actions.push(CheckedNormalCleanupAction::RetireHome {
            source,
            type_identity: base_type_identity(
                program,
                root.type_reference,
                &crate::execution::terminal_unit::types::machine_binders(program, machine),
                &[],
            )?,
        });
    }
    if !cleanup_actions.is_empty() {
        completions.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
            selected_regions: Vec::new(),
            boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
            cleanup_actions,
        });
    }
    Some(completions)
}
