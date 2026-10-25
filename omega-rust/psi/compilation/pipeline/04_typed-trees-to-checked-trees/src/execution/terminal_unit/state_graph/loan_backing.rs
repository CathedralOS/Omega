//! Captured loan resources name the actual storage retained across an arrival,
//! including stored-field crossings and call-returned references. Missing
//! descriptor provenance refuses retention rather than guessing a local home.
//! A selected arrival may retire a descriptor and transport its owned backing
//! only when no other captured value needs that backing's address. Retirement
//! alone says nothing about copied descriptors. Use the checked call's complete
//! operand resources and their stored dependencies, not producer spellings or
//! lexical last-use indices. Unresolved coverage retains the stable-home route.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, CheckFacts, CheckedCallBorrowLoanSource, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, FlowConstraintRef,
};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

fn selected_call(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
) -> Option<Handle<checked_trees::FlowCallFact>> {
    let flow = facts.flow.control.state(machine, state)?;
    let mut calls = flow.calls.handles().filter(|handle| {
        let call = facts.flow.control.calls.get(*handle);
        call.statement_index == statement
            && call.authored_site == checked_trees::FlowCallSite::NamedTransition
            && call.authored_target == target
            && facts
                .flow
                .control
                .call_arguments
                .span(call.authored_arguments)
                == Some(arguments)
    });
    let call = calls.next()?;
    calls.next().is_none().then_some(call)
}

/// Rejoin captured permission to its state-local storage occurrence. The
/// checker has already composed stored-field crossings and call-result
/// provenance. Replaying descriptor constructors here would lose those edges.
pub(super) struct RetainedStorage {
    pub(super) backing: Option<u32>,
    pub(super) parents: Vec<(
        CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralArgumentSourcePlan,
    )>,
    pub(super) operand_parents: Vec<CheckedUnitStructuralArgumentSourcePlan>,
}

pub(super) fn retained_storage(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    argument_position: usize,
    operations: &[CheckedUnitEffectOperationPlan],
    address_source: Option<CheckedUnitStructuralArgumentSourcePlan>,
) -> Result<RetainedStorage, &'static str> {
    let call_handle = selected_call(facts, machine, state, statement, target, arguments)
        .ok_or("state loan: exact captured call")?;
    let operand = facts
        .flow
        .control
        .calls
        .get(call_handle)
        .argument_operands()
        .handles()
        .nth(argument_position)
        .ok_or("state loan: captured operand position")?;
    let (parents, mut operand_parents) = retained_parents(
        program,
        facts,
        machine,
        state,
        statement,
        call_handle,
        operand,
        operations,
        address_source,
    )?;
    let mut roots = facts
        .borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.call == call_handle
                && resource.machine_symbol == machine
                && resource.state_symbol == state
                && resource.owner_operand == operand
                && resource.owner_path.is_empty()
        })
        .map(|(_, resource)| resource.parent_lifetime.root);
    let Some(root) = roots.next() else {
        let capture = facts.flow.control.operands.get(operand);
        if validation::reference_result_custody::stored_loan_parts(program, capture.type_reference)
            .is_some()
        {
            // A completed descriptor requires its whole invocation resource.
            // Missing resolution is not evidence of caller-owned storage.
            return Err("state loan: descriptor resource is absent");
        }
        return Ok(RetainedStorage {
            backing: None,
            parents,
            operand_parents,
        });
    };
    // A unique backing may use the ordinary whole-home transport. A selected
    // descriptor can instead name several independently owned homes: retain
    // every captured possibility through its target parameter, without choosing
    // one home or granting relocation of any of them. Their checked lifetime
    // and disposal dependencies remain separate from the selected address.
    let roots = std::iter::once(root).chain(roots).collect::<Vec<_>>();
    let backing = local_backing_binding(program, facts, state, statement, operations, root)?;
    let mut common_backing = true;
    for candidate in &roots {
        common_backing &=
            local_backing_binding(program, facts, state, statement, operations, *candidate)?
                == backing;
    }
    if !common_backing {
        for root in roots {
            if let Some(parent) =
                owned_parent_source(program, facts, state, statement, operations, root)?
                && !operand_parents.contains(&parent)
            {
                operand_parents.push(parent);
            }
        }
    }
    let backing = common_backing.then_some(backing).flatten();
    Ok(RetainedStorage {
        backing,
        parents,
        operand_parents,
    })
}

fn local_backing_binding(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: usize,
    operations: &[CheckedUnitEffectOperationPlan],
    root: PlaceRoot,
) -> Result<Option<u32>, &'static str> {
    let root_type = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )
    .ok_or("state loan: backing root type")?;
    if validation::reference_result_custody::stored_loan_parts(program, root_type).is_some() {
        // Owning a descriptor is not owning provider/caller storage it denotes.
        return Ok(None);
    }
    local_storage_binding(program, facts, state, operations, root)
}

/// Locate owned descriptor storage as well as ordinary owned values. Unlike
/// the primary referent query, this never interprets the descriptor as authority
/// over the caller/provider storage it denotes.
fn local_storage_binding(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
    root: PlaceRoot,
) -> Result<Option<u32>, &'static str> {
    let typed_state = program
        .state_by_symbol(state)
        .ok_or("state loan: source state")?;
    if let Some(symbol) = root.as_symbol() {
        if program
            .state_parameter_by_symbol(typed_state, symbol)
            .is_some()
        {
            return Ok(None);
        }
        let source = super::super::local_storage::source(program, typed_state, operations, symbol);
        // No state-local occurrence means no locally owned cleanup to retain.
        return match source {
            Some(source) => source
                .current_result_binding(operations)
                .map(Some)
                .ok_or("state loan: backing occurrence is not a production result"),
            None => Ok(None),
        };
    }
    let mut producers = operations.iter().filter(|operation| {
        super::super::cleanup::normal_edges::producer_root(facts, state, operation) == Some(root)
    });
    let Some(producer) = producers.next() else {
        return Ok(None);
    };
    if producers.next().is_some() {
        return Err("state loan: duplicate backing producers");
    }
    Ok(Some(
        producer
            .structural_result_binding()
            .ok_or("state loan: backing producer has no structural result")?
            .binding_ordinal,
    ))
}

/// The selected call's complete stored-loan frontier already names these
/// dependencies, including replaced/call-produced references. Retain the
/// current checked roots, never replay construction syntax or stale initializers.
/// A dependency is storage lifetime only: cleanup still needs its own action.
fn retained_parents(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    call_handle: Handle<checked_trees::FlowCallFact>,
    operand: Handle<checked_trees::FlowOperandFact>,
    operations: &[CheckedUnitEffectOperationPlan],
    address_source: Option<CheckedUnitStructuralArgumentSourcePlan>,
) -> Result<
    (
        Vec<(
            CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralArgumentSourcePlan,
        )>,
        Vec<CheckedUnitStructuralArgumentSourcePlan>,
    ),
    &'static str,
> {
    let call = facts.flow.control.calls.get(call_handle);
    let capture = facts.flow.control.operands.get(operand);
    // State ingress already owns opaque carried permissions, not fresh local
    // loan rows. Replacements still supply their current stored-loan resources.
    // Their lifetime is continued by the actual parameter handoff, never by
    // inventing a local parent or granting relocation of its referent.
    let ingress = facts
        .borrow
        .incoming_references
        .iter()
        .filter_map(|(_, incoming)| {
            if incoming.machine_symbol != machine || incoming.state_symbol != state {
                return None;
            }
            let checked_trees::IncomingReferenceOrigin::Parameter { symbol } = incoming.origin
            else {
                return None;
            };
            Some((
                PlaceRoot::Symbol(symbol),
                facts
                    .borrow
                    .owner_segments
                    .span_or_empty(incoming.owner_path),
            ))
        })
        .collect::<Vec<_>>();
    let typed_state = program
        .state_by_symbol(state)
        .ok_or("state loan parents: source state")?;
    let ingress_storage = program
        .state_parameters(typed_state)
        .iter()
        .filter_map(|parameter| {
            validation::reference_result_custody::input_carrier_type(
                program,
                parameter.type_reference,
            )
            .map(|carrier| (PlaceRoot::Symbol(parameter.symbol), carrier))
        })
        .collect::<Vec<_>>();
    let paths =
        validation::borrow_contracts::borrow_carrying_owner_paths(program, capture.type_reference)
            .ok_or("state loan parents: unresolved operand frontier")?;
    let roots = facts
        .borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.call == call_handle
                && resource.machine_symbol == machine
                && resource.state_symbol == state
                && resource.owner_operand == operand
        })
        .map(|(_, resource)| resource)
        .collect::<Vec<_>>();
    let mut shared_backings = Vec::new();
    for path in &paths {
        if roots.iter().any(|resource| {
            facts
                .borrow
                .owner_segments
                .span_or_empty(resource.owner_path)
                == path.as_slice()
        }) || ingress_covers_capture_path(
            program,
            facts,
            operand,
            &crate::borrow::owner_place_segments(path),
            &ingress,
        ) {
            continue;
        }
        let retained = shared_leaf_lifetime_roots(program, facts, capture, path)
            .ok_or("state loan parents: captured reference leaves have no resources")?;
        for root in retained {
            if !shared_backings.contains(&root) {
                shared_backings.push(root);
            }
        }
    }
    if roots.is_empty() && shared_backings.is_empty() && address_source.is_none() {
        return Ok((Vec::new(), Vec::new()));
    }
    let borrow_state = facts
        .borrow
        .unique_state_borrow(machine, state)
        .ok_or("state loan parents: source borrow state")?;
    let dependencies = Dependencies {
        program,
        facts,
        state,
        statement,
        borrow_state,
        forbidden: &[],
        ingress: &ingress,
        ingress_storage: &ingress_storage,
    };
    let mut captured = Vec::new();
    let mut operand_parents = Vec::new();
    // A projected address can keep its physical source descriptor alive even
    // when the selected leaf's logical loan names a different ultimate home.
    // Retain that descriptor's checked stored dependencies as well. This is
    // lifetime evidence only; it grants neither access nor relocation.
    if let Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }) =
        address_source
    {
        let operation = operations
            .iter()
            .find(|operation| {
                operation
                    .structural_result_binding()
                    .is_some_and(|binding| binding.binding_ordinal == binding_ordinal)
            })
            .ok_or("state loan parents: address source production")?;
        let root = super::super::cleanup::normal_edges::producer_root(facts, state, operation)
            .ok_or("state loan parents: address source storage")?;
        if !dependencies.contents(root, call.entry_constraints, &mut Vec::new(), &mut captured) {
            return Err("state loan parents: incomplete address storage dependencies");
        }
    }
    for resource in roots {
        let root = resource.parent_lifetime.root;
        // A moved enclosing value carries its stored leaves even though the
        // enclosing value itself has no direct-reference resource. The emitted
        // target parameter owns this dependency, including inline constructions.
        let incoming_referent = match resource.source {
            CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                facts.borrow.loans.get(loan).incoming_reference.is_valid()
            }
            CheckedCallBorrowLoanSource::CallAccess { .. } => false,
        };
        // An input aggregate's opaque reference leaf denotes caller storage,
        // not the owned aggregate slot that shares its invocation root name.
        if !resource.owner_path.is_empty()
            && !incoming_referent
            && let Some(parent) =
                owned_parent_source(program, facts, state, statement, operations, root)?
            && !operand_parents.contains(&parent)
        {
            operand_parents.push(parent);
        }
        if !dependencies.contents(root, call.entry_constraints, &mut Vec::new(), &mut captured) {
            return Err("state loan parents: incomplete captured storage dependencies");
        }
    }
    // Shared returned descriptors may outlive the temporary access that formed
    // them. Their completed lifetime profiles still retain exact local homes;
    // absence of a current invocation access never makes that backing ambient.
    // This relation keeps storage alive and grants no access or relocation.
    for root in shared_backings {
        if let Some(parent) =
            owned_parent_source(program, facts, state, statement, operations, root)?
            && !operand_parents.contains(&parent)
        {
            operand_parents.push(parent);
        }
        if !dependencies.contents(root, call.entry_constraints, &mut Vec::new(), &mut captured) {
            return Err("state loan parents: incomplete shared storage dependencies");
        }
    }
    let mut parents = Vec::new();
    for (dependent, parent) in captured {
        let Some(parent) =
            owned_parent_source(program, facts, state, statement, operations, parent)?
        else {
            // Caller/provider storage is not owned by this activation.
            continue;
        };
        let dependent =
            owned_parent_source(program, facts, state, statement, operations, dependent)?;
        let Some(dependent) = dependent else {
            // Incoming storage is retained by its caller, not this local roster.
            continue;
        };
        let relation = (dependent, parent);
        if !parents.contains(&relation) {
            parents.push(relation);
        }
    }
    Ok((parents, operand_parents))
}

/// A complete shared-leaf lifetime profile can retain storage after its forming
/// access ends. Read every exact completed profile; neither shared type spelling
/// nor a missing resource establishes external or image backing. Exclusive
/// leaves still require their captured permission resource or ingress relation.
fn shared_leaf_lifetime_roots(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    capture: &checked_trees::FlowOperandFact,
    path: &[checked_trees::BorrowLoanOwnerSegment],
) -> Option<Vec<PlaceRoot>> {
    let path = crate::borrow::owner_place_segments(path);
    let reference =
        validation::project_type_reference_from_segments(program, capture.type_reference, &path)?;
    if validation::reference_shell(program, reference)?.access
        != language_semantics::ReferenceAccess::Shared
    {
        return None;
    }
    let profiles = validation::completed_storage::captured_retention_profiles_at_path(
        program,
        &facts.flow.control,
        capture,
        &path,
    )?;
    if profiles.is_empty() {
        return None;
    }
    let mut roots = Vec::new();
    for profile in profiles {
        for root in facts.flow.control.retained_roots(profile)? {
            if !roots.contains(root) {
                roots.push(*root);
            }
        }
    }
    Some(roots)
}

/// Reuse the completed assembly's child projection. An opaque child keeps its
/// remaining path in its own parameter namespace; alternatives must each own
/// that same permission. No initializer or source constructor is replayed.
fn ingress_covers_capture_path(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: Handle<checked_trees::FlowOperandFact>,
    path: &[facts::PlaceSegment],
    ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
) -> bool {
    ingress_covers_capture_path_inner(
        program,
        facts,
        operand,
        path,
        ingress,
        facts.flow.control.operands.get(operand).storage_snapshot,
        &mut Vec::new(),
    )
}

fn ingress_covers_capture_path_inner(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: Handle<checked_trees::FlowOperandFact>,
    path: &[facts::PlaceSegment],
    ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
    current: Handle<checked_trees::FlowStorageSnapshot>,
    active: &mut Vec<(
        Handle<checked_trees::FlowOperandFact>,
        Vec<facts::PlaceSegment>,
    )>,
) -> bool {
    let key = (operand, path.to_vec());
    if !facts.flow.control.operands.is_valid(operand) || active.contains(&key) {
        return false;
    }
    active.push(key);
    let covered =
        ingress_covers_capture_path_body(program, facts, operand, path, ingress, current, active);
    active.pop();
    covered
}

fn ingress_covers_capture_path_body(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: Handle<checked_trees::FlowOperandFact>,
    path: &[facts::PlaceSegment],
    ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
    current: Handle<checked_trees::FlowStorageSnapshot>,
    active: &mut Vec<(
        Handle<checked_trees::FlowOperandFact>,
        Vec<facts::PlaceSegment>,
    )>,
) -> bool {
    let control = &facts.flow.control;
    let capture = control.operands.get(operand);
    if crate::flow::literal_value_path_is_inactive(
        program,
        capture.expression,
        capture.type_reference,
        path,
    ) {
        return true;
    }
    if !capture.alternatives.is_empty() {
        return capture.alternatives.handles().all(|alternative| {
            ingress_covers_capture_path_inner(
                program,
                facts,
                alternative,
                path,
                ingress,
                current,
                active,
            )
        });
    }
    // Completed helper results retain exact supplier occurrences. A missing
    // association in any alternative cannot be covered by another supplier.
    if let Some(profiles) = validation::completed_storage::captured_retention_profiles_at_path(
        program, control, capture, path,
    ) {
        let suppliers = profiles
            .iter()
            .map(|profile| {
                validation::completed_storage::descriptor_suppliers(
                    control,
                    *profile,
                    &mut Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        if suppliers.iter().any(Option::is_some) {
            return !suppliers.is_empty()
                && suppliers.into_iter().all(|suppliers| {
                    suppliers.is_some_and(|suppliers| {
                        !suppliers.is_empty()
                            && suppliers.iter().all(|supplier| {
                                ingress_covers_capture_path_inner(
                                    program,
                                    facts,
                                    supplier.operand,
                                    control
                                        .operand_retained_paths
                                        .span_or_empty(supplier.projection),
                                    ingress,
                                    current,
                                    active,
                                )
                            })
                    })
                });
        }
    }
    if let Some(stored) = validation::completed_storage::reference_captures_at_path(
        program, control, operand, path, current,
    ) {
        let Some(captures) = stored.captures() else {
            return false;
        };
        return !captures.is_empty()
            && captures.iter().all(|selected| {
                // A view descriptor can select itself. Only exact unchanged
                // ingress below can cover that terminal query; never assume it.
                if selected.capture == operand && selected.projection == path {
                    return unchanged_ingress_capture(program, control, capture, path, ingress);
                }
                ingress_covers_capture_path_inner(
                    program,
                    facts,
                    selected.capture,
                    &selected.projection,
                    ingress,
                    current,
                    active,
                )
            });
    }
    unchanged_ingress_capture(program, control, capture, path, ingress)
}

fn unchanged_ingress_capture(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    capture: &checked_trees::FlowOperandFact,
    path: &[facts::PlaceSegment],
    ingress: &[(PlaceRoot, &[checked_trees::BorrowLoanOwnerSegment])],
) -> bool {
    let Some(mut source) = validation::canonical_place_from_expression_in_state(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return false;
    };
    source.segments.extend_from_slice(path);
    // A matching input spelling cannot cover a replaced binding. Query this
    // completed value's cut; later rebindings cannot retarget an earlier copy.
    if validation::completed_storage::at_snapshot(
        program,
        control,
        capture.storage_snapshot,
        &source,
        validation::completed_storage::StoredObservation::Binding,
    )
    .is_some()
    {
        return false;
    }
    let selected = crate::borrow::owner_path_from_place_segments(program, &source.segments);
    ingress
        .iter()
        .any(|(root, path)| *root == source.root && *path == selected)
}

fn owned_parent_source(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: usize,
    operations: &[CheckedUnitEffectOperationPlan],
    root: PlaceRoot,
) -> Result<Option<CheckedUnitStructuralArgumentSourcePlan>, &'static str> {
    let reference = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )
    .ok_or("state loan parents: parent type")?;
    if validation::reference_result_custody::result_multiplicity(program, reference)
        == language_semantics::Multiplicity::Unrestricted
    {
        return Ok(None);
    }
    let typed_state = program
        .state_by_symbol(state)
        .ok_or("state loan parents: source state")?;
    if let Some(symbol) = root.as_symbol()
        && let Some(parameter) = program.state_parameter_by_symbol(typed_state, symbol)
    {
        // Borrowed ingress denotes storage retained by the caller. An owned
        // input instead belongs to this activation even after a local move.
        if validation::reference_shell(program, parameter.type_reference).is_some() {
            return Ok(None);
        }
        let parameters = program.state_parameters(typed_state);
        let authored = parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .ok_or("state loan parents: owned input position")?;
        let parameter_index =
            super::super::types::dense_structural_parameter_index(program, parameters, authored)
                .ok_or("state loan parents: owned input namespace")?;
        return Ok(Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index,
        }));
    }
    Ok(
        local_storage_binding(program, facts, state, operations, root)?.map(|binding_ordinal| {
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
        }),
    )
}

pub(super) fn retired_carrier_backing(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    argument_position: usize,
    operations: &[CheckedUnitEffectOperationPlan],
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Option<u32> {
    if subject.path.as_slice() != [CheckedUnitStructuralPathSegment::Referent] {
        return None;
    }
    let carrier = subject.source.current_result_binding(operations)?;
    let (formation, producer) = operations.iter().enumerate().find(|(_, operation)| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == carrier)
    })?;
    let lent = producer.established_reference_source(&facts.values.structural_values)?;
    if !lent.path.is_empty() {
        return None;
    }
    let backing = lent
        .source
        .current_result_binding(&operations[..formation])?;
    let backing_producer = operations[..formation].iter().find(|operation| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == backing)
    })?;
    let backing_root =
        super::super::cleanup::normal_edges::producer_root(facts, state, backing_producer)?;
    let carrier_root = super::super::cleanup::normal_edges::producer_root(facts, state, producer)?;
    let backing_type = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root: backing_root,
            segments: Vec::new(),
        },
    )?;
    if validation::reference_shell(program, backing_type).is_some() {
        return None;
    }
    let released =
        operations
            .iter()
            .enumerate()
            .find_map(|(position, operation)| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    case_conditions,
                    loans,
                    select_evaluated_storage: false,
                    defer_to_exit: true,
                    ..
                } if path.is_empty()
                    && case_conditions.is_empty()
                    && source.current_result_binding(&operations[..position]) == Some(carrier) =>
                {
                    Some(*loans)
                }
                _ => None,
            })?;

    let call_handle = selected_call(facts, machine, state, statement, target, arguments)?;
    let call = facts.flow.control.calls.get(call_handle);
    let selected_operand = call.argument_operands().handles().nth(argument_position)?;
    let resources = facts
        .borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.call == call_handle
                && resource.machine_symbol == machine
                && resource.state_symbol == state
        })
        .map(|(_, resource)| resource)
        .collect::<Vec<_>>();
    let borrow_state = facts.borrow.unique_state_borrow(machine, state)?;
    let dependencies = Dependencies {
        program,
        facts,
        state,
        statement,
        borrow_state,
        forbidden: &[backing_root, carrier_root],
        ingress: &[],
        ingress_storage: &[],
    };
    let mut selected = false;
    for operand in call.argument_operands().handles() {
        let capture = facts.flow.control.operands.get(operand);
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(
            program,
            capture.type_reference,
        )?;
        let captured = resources
            .iter()
            .copied()
            .filter(|resource| resource.owner_operand == operand)
            .collect::<Vec<_>>();
        if !paths.iter().all(|path| {
            captured.iter().any(|resource| {
                facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
            })
        }) || captured.iter().any(|resource| {
            !paths.iter().any(|path| {
                facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
            })
        }) {
            return None;
        }
        for resource in captured {
            if operand == selected_operand {
                // Rebinding one whole reference is not permission to retarget
                // another leaf or an alternative captured source.
                if !resource.owner_path.is_empty() || resource.parent_lifetime.root != backing_root
                {
                    return None;
                }
                let CheckedCallBorrowLoanSource::CallAccess { access, parents } = resource.source
                else {
                    return None;
                };
                if facts.borrow.argument_accesses.get(access).expression != capture.expression {
                    return None;
                }
                let parents = facts.borrow.call_loan_parents.span(parents)?;
                let released = facts.borrow.loan_group(released)?.collect::<Vec<_>>();
                if released.is_empty()
                    || !released.iter().all(|(loan, _)| parents.contains(loan))
                    || parents.iter().any(|parent| {
                        !released.iter().any(|(loan, _)| loan == parent)
                            && !dependencies.loan(
                                *parent,
                                capture.constraints,
                                &mut Vec::new(),
                                &mut Vec::new(),
                                &mut Vec::new(),
                            )
                    })
                {
                    return None;
                }
                selected = true;
                continue;
            }
            if !dependencies.root(
                resource.parent_lifetime.root,
                call.entry_constraints,
                &mut Vec::new(),
                &mut Vec::new(),
            ) {
                return None;
            }
            let loans = match resource.source {
                CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                    facts.borrow.call_loan_parents.span(parents)?.to_vec()
                }
            };
            for loan in loans {
                if !dependencies.loan(
                    loan,
                    capture.constraints,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut Vec::new(),
                ) {
                    return None;
                }
            }
        }
    }
    // The moved home carries its own reference leaves too. Exclude self- or
    // mutually backed storage even when the outgoing formal is just &Home.
    if !selected
        || !dependencies.contents(
            backing_root,
            call.entry_constraints,
            &mut Vec::new(),
            &mut Vec::new(),
        )
    {
        return None;
    }
    Some(backing)
}

struct Dependencies<'a> {
    program: &'a dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    state: SymbolHandle,
    statement: usize,
    borrow_state: &'a checked_trees::StateBorrowFact,
    forbidden: &'a [PlaceRoot],
    /// Exact caller-supplied leaf contracts are sufficient for continued
    /// lifetime, not for proving relocation independence. That judgment passes
    /// no ingress assumptions and retains its existing fail-closed behavior.
    ingress: &'a [(PlaceRoot, &'a [checked_trees::BorrowLoanOwnerSegment])],
    ingress_storage: &'a [(PlaceRoot, typed_trees::types::TypeReferenceHandle)],
}

impl Dependencies<'_> {
    fn root(
        &self,
        root: PlaceRoot,
        constraints: HandleSpan<FlowConstraintRef>,
        active: &mut Vec<PlaceRoot>,
        captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
    ) -> bool {
        if self.forbidden.contains(&root) {
            return false;
        }
        if let Some(dependent) = active.last().copied()
            && !captured.contains(&(dependent, root))
        {
            captured.push((dependent, root));
        }
        self.contents(root, constraints, active, captured)
    }

    fn contents(
        &self,
        root: PlaceRoot,
        constraints: HandleSpan<FlowConstraintRef>,
        active: &mut Vec<PlaceRoot>,
        captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
    ) -> bool {
        if active.contains(&root) {
            return false;
        }
        let Some(mut reference) = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        ) else {
            return false;
        };
        let mut opaque_referent = false;
        if root.as_symbol().is_some_and(|symbol| {
            self.program
                .state_by_symbol(self.state)
                .is_some_and(|state| {
                    self.program
                        .state_parameter_by_symbol(state, symbol)
                        .is_some()
                })
        }) {
            // A borrowed parameter names its caller-supplied referent. Its
            // root permission is not a stored descriptor requiring a loan row;
            // only references inside that referent join the stored frontier.
            // A readable borrowed parameter peels to its referent carrier.
            // A write-only parameter carries no interior leaves into this
            // state at all: checking publishes no trackers for them, so the
            // referent is opaque storage with an empty stored frontier.
            match validation::reference_result_custody::input_carrier_type(self.program, reference)
            {
                Some(carrier) => reference = carrier,
                None if validation::reference_shell(self.program, reference).is_some() => {
                    opaque_referent = true;
                }
                None => return false,
            }
        }
        let paths = if opaque_referent {
            Some(Vec::new())
        } else if let Some((_, carrier)) = self
            .ingress_storage
            .iter()
            .find(|(owner, _)| *owner == root)
        {
            validation::reference_result_custody::declared_carrier_references(
                self.program,
                *carrier,
            )
            .and_then(|sources| {
                sources
                    .into_iter()
                    .map(|source| {
                        let segments = validation::reference_result_custody::origin_place_segments(
                            self.program,
                            *carrier,
                            source.path,
                        )?;
                        Some(crate::borrow::owner_path_from_place_segments(
                            self.program,
                            &segments,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()
            })
        } else {
            validation::borrow_contracts::borrow_carrying_owner_paths(self.program, reference)
        };
        let Some(paths) = paths else {
            return false;
        };
        let loans = self
            .facts
            .flow
            .borrow_loan_constraints(constraints)
            .filter(|handle| self.facts.borrow.loans.get(*handle).storage_owner == root)
            .collect::<Vec<_>>();
        if !paths.iter().all(|path| {
            loans.iter().any(|handle| {
                self.facts
                    .borrow
                    .loan_owner_path(self.facts.borrow.loans.get(*handle))
                    == path.as_slice()
            }) || self
                .ingress
                .iter()
                .any(|(owner, incoming)| *owner == root && *incoming == path.as_slice())
        }) || loans.iter().any(|handle| {
            !paths.iter().any(|path| {
                self.facts
                    .borrow
                    .loan_owner_path(self.facts.borrow.loans.get(*handle))
                    == path.as_slice()
            })
        }) {
            return false;
        }
        active.push(root);
        let independent = loans
            .into_iter()
            .all(|loan| self.loan(loan, constraints, active, &mut Vec::new(), captured));
        active.pop();
        independent
    }

    fn loan(
        &self,
        handle: Handle<BorrowLoanFact>,
        constraints: HandleSpan<FlowConstraintRef>,
        active_roots: &mut Vec<PlaceRoot>,
        active_loans: &mut Vec<Handle<BorrowLoanFact>>,
        captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
    ) -> bool {
        if !self.facts.borrow.state_owns_loan(self.borrow_state, handle)
            || active_loans.contains(&handle)
        {
            return false;
        }
        let loan = self.facts.borrow.loans.get(handle);
        if self.forbidden.contains(&loan.root) {
            return false;
        }
        // Incoming referents predate this state's independent local storage.
        // Their identity is not the descriptor slot named by their root.
        let incoming = self
            .facts
            .borrow
            .incoming_references
            .is_valid(loan.incoming_reference)
            && {
                let identity = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                identity.machine_symbol == self.borrow_state.machine_symbol
                    && identity.state_symbol == self.state
                    && matches!(identity.origin,
                        checked_trees::IncomingReferenceOrigin::Parameter { symbol }
                            if loan.root == PlaceRoot::Symbol(symbol))
            };
        if loan.incoming_reference.is_valid() && !incoming {
            return false;
        }
        if !incoming && !self.root(loan.root, constraints, active_roots, captured) {
            return false;
        }
        active_loans.push(handle);
        let parent_independent = match loan.lineage {
            checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => self.loan(
                parent_loan,
                constraints,
                active_roots,
                active_loans,
                captured,
            ),
            checked_trees::BorrowLoanLineage::DirectRoot => true,
            // A keepalive obligation does not locate provider-owned storage.
            checked_trees::BorrowLoanLineage::LifetimeDependency => false,
            // Transport has no new reborrow parent. Its exact incoming
            // identity or complete supplier relation still names its backing;
            // this independence query grants no restoration authority.
            checked_trees::BorrowLoanLineage::UnretainedDerived => {
                incoming || !loan.supplying_loans.is_empty()
            }
        };
        let independent = parent_independent
            && self
                .facts
                .borrow
                .loan_suppliers
                .span(loan.supplying_loans)
                .is_some_and(|suppliers| {
                    suppliers.iter().all(|supplier| {
                        self.loan(*supplier, constraints, active_roots, active_loans, captured)
                    })
                });
        active_loans.pop();
        independent
    }
}
