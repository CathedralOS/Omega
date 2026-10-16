//! Structural call arguments: their custody, restored reborrow aliases,
//! exact borrow access and the claim transfers a call performs.
//!
//! The borrow roster retains the full operand evaluation history for conflict
//! checking. Invocation contracts consume only accesses still live at the call:
//! a completed nested operand can mutate a place before a later shared argument
//! without supplying mutable access to the enclosing invocation.

use crate::execution::terminal_unit::calls::argument_paths::byte_sequence_literal_argument;
use crate::execution::terminal_unit::calls::computation_arguments;
use crate::execution::terminal_unit::calls::parameter_argument_paths::{
    ParameterArgument, SourceParameter, boundary_parameter_path, ordinary_parameter_path,
};
use crate::execution::terminal_unit::calls::reference_forwarding;
use crate::execution::terminal_unit::calls::result_arguments;
use crate::execution::terminal_unit::calls::view_subslice;
use crate::execution::terminal_unit::types::{
    base_type_identity, is_unit, structural_access_for_type_reference,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitClaimTransferPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, MachineSupplyMode, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, SymbolHandle,
    strips_erased_parameter,
};

pub(crate) fn structural_call_arguments(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    call: &checked_trees::FlowCallFact,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    callee: super::CallTarget<'_>,
    call_site: &validation::CallSite<'_>,
    statement_index: usize,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    caller_operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    trace: &crate::execution::terminal_unit::control::LocalConstructionTrace,
) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
    let source_parameters = program.state_parameters(caller_state);
    let target_parameters = callee.parameters;
    // Erased borrow carriers name their captured referent's storage: the
    // checked alias roster substitutes that referent for the authored root
    // while the authored spelling still joins the borrow-access evidence.
    let borrow_aliases = crate::execution::terminal_unit::receiver_aliases::aliases(
        program,
        facts,
        caller_machine,
        caller_state,
    )
    .unwrap_or_default();
    let explicit_arguments = validation::call_site_argument_expressions(program, call_site);
    let mut explicit_index = 0usize;
    let mut output = Vec::new();
    let mut structural_argument_ordinal = 0usize;
    // Each argument-source arm marks the trace before it can decline, so
    // an omission here names the source vocabulary that refused the actual.
    let arm = |name: &'static str| {
        trace.phase(name);
        trace.statement(u32::try_from(statement_index).ok());
    };

    for target in target_parameters {
        // The erased position's authored argument is proof material with no
        // structural or scalar transfer; consume it and plan nothing.
        arm("call operation: structural arguments: erased position");
        if strips_erased_parameter(target)? {
            explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            continue;
        }
        arm("call operation: structural arguments: scalar position");
        if program
            .primitive_type_reference(target.type_reference)
            .is_some()
        {
            explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            continue;
        }
        let argument_ordinal = structural_argument_ordinal;
        let selector = |expression| {
            let root = facts.values.scalar_computations.root_at(
                caller_state.symbol,
                u32::try_from(statement_index).ok()?,
                checked_trees::CheckedScalarExpressionRole::CallSelector {
                    call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                    expression,
                },
            )?;
            Some(checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root.root))
        };
        structural_argument_ordinal = structural_argument_ordinal.checked_add(1)?;
        arm("call operation: structural arguments: argument place");
        let authored_place = {
            let expression = *explicit_arguments.get(explicit_index)?;
            explicit_index += 1;
            // A retained producer owns the actual, even when the authored
            // expression also has an inline literal or range presentation.
            // Shared temporaries retain their referent expression; exclusive
            // borrows cannot be peeled into an owned result.
            let owner_expression = if caller_structural_results
                .iter()
                .any(|(_, root)| *root == facts::PlaceRoot::Expression(expression))
            {
                expression
            } else {
                crate::semantic::calls::shared_argument_referent(
                    program,
                    expression,
                    target.type_reference,
                )
                .map_or(expression, |(referent, _)| referent)
            };
            let produced = caller_structural_results
                .iter()
                .any(|(_, root)| *root == facts::PlaceRoot::Expression(owner_expression));
            if produced {
                validation::CanonicalPlace {
                    root: facts::PlaceRoot::Expression(owner_expression),
                    segments: Vec::new(),
                }
            } else {
                if callee.supply_mode == MachineSupplyMode::CheckedBody
                    && let Some(literal) =
                        byte_sequence_literal_argument(program, target.type_reference, expression)
                {
                    output.push(literal);
                    continue;
                }
                // Selection can retain a bodyless requirement for a checked
                // target body. Its range operand is still the same evaluated
                // value, not a static storage path. Admission precedes the
                // shared call-scoped fixed-window presentation decision.
                if validation::subslice_expression(program, expression).is_some() {
                    let subslice = view_subslice::admit(
                        program,
                        facts,
                        caller_machine,
                        caller_state,
                        caller_parameters,
                        target.type_reference,
                        expression,
                        statement_index,
                        checked_trees::CheckedSubsliceSite::CallArgument {
                            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                        },
                    )?;
                    if let Some(argument) = subslice.call_argument(
                        program,
                        caller_state,
                        statement_index,
                        callee.has_bodyless_signature(),
                    ) {
                        output.push(argument);
                        continue;
                    }
                }
                // A fixed range projection must still name the builtin slice
                // operation. Canonical storage identity alone does not authorize
                // replacing an authored range operator with a byte-window loan.
                if matches!(program.expression_table.expression(expression),
                crate::execution::terminal_unit::ExpressionNode::Indexed(indexed)
                    if matches!(program.expression_table.expression(indexed.index),
                        crate::execution::terminal_unit::ExpressionNode::Range(_)))
                    && !validation::has_builtin_subslice_meaning(
                        program,
                        caller_machine,
                        Some(caller_state),
                        expression,
                    )
                {
                    return None;
                }
                validation::canonical_place_from_expression_in_state(
                    program,
                    caller_state.symbol,
                    statement_index,
                    expression,
                )?
            }
        };
        arm("call operation: structural arguments: alias and identity");
        let materialized = |root| {
            caller_structural_results
                .iter()
                .any(|(_, candidate)| *candidate == root)
        };
        let resolved_alias = crate::execution::terminal_unit::receiver_aliases::resolve(
            &borrow_aliases,
            &authored_place,
            materialized,
        );
        let materialized_root = resolved_alias.as_ref().unwrap_or(&authored_place).root;
        let restored_alias = (!materialized(materialized_root))
            .then(|| {
                reborrow_restored_call_alias_target(
                    facts,
                    caller_machine.symbol,
                    caller_state.symbol,
                    call,
                    &authored_place,
                )
                .or_else(|| {
                    reborrow_restored_shared_cohort_observation_alias_target(
                        program,
                        facts,
                        caller_machine.symbol,
                        caller_state,
                        callee,
                        call,
                        call_site,
                        &authored_place,
                    )
                })
            })
            .flatten();
        let place = restored_alias
            .clone()
            .or_else(|| resolved_alias.clone())
            .unwrap_or_else(|| authored_place.clone());
        // Byte carriers and `&[T]` views name their own carriers under the
        // same structural identity rule as every other formal.
        let target_identity = base_type_identity(program, target.type_reference, &[], &[])?;
        arm("call operation: structural arguments: caller structural result");
        if let Some((result, _)) = caller_structural_results
            .iter()
            .find(|(_, root)| *root == place.root)
        {
            // Call construction has already admitted the result and supply.
            // Operand custody depends on this source and formal, not whether
            // the callee returns a scalar, descriptor or owned aggregate.
            let expression = *explicit_arguments.get(explicit_index.checked_sub(1)?)?;
            output.push(result_arguments::argument(
                program,
                facts,
                caller_machine.symbol,
                caller_state.symbol,
                call,
                expression,
                &place,
                result,
                target,
                target_identity,
                caller_operations,
                trace,
            )?);
            continue;
        }
        arm("call operation: structural arguments: source symbol");
        let source_symbol = place.root.as_symbol()?;
        if crate::execution::terminal_unit::structural_scalar_store::primitive_local_before(
            program,
            caller_state,
            statement_index,
            source_symbol,
        )
        .is_some()
        {
            if restored_alias.is_some() {
                return None;
            }
            output.push(computation_arguments::primitive_local_argument(
                program,
                &facts.borrow,
                caller_machine.symbol,
                caller_state,
                call,
                &place,
                &authored_place,
                target.type_reference,
            )?);
            continue;
        }
        arm("call operation: structural arguments: trivial affine local");
        if let Some((local, _)) = caller_trivial_affine_locals
            .iter()
            .find(|(_, symbol)| *symbol == source_symbol)
        {
            let exact_transfer = facts.flow.ownership.permissions.iter().any(|(_, event)| {
                event.machine_symbol == caller_machine.symbol
                    && event.state_symbol == caller_state.symbol
                    && event.source
                        == PermissionEventSource::Call {
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            target_symbol: call.target_symbol,
                        }
                    && event.kind == PermissionEventKind::Transfer
                    && event.multiplicity == Multiplicity::Affine
                    && event.access == PermissionAccess::Owned
                    && event.claim_identity == PermissionClaimIdentity::Unknown
                    && !event.obligation_live
                    && event.root == facts::PlaceRoot::Symbol(source_symbol)
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .is_empty()
            });
            if !place.segments.is_empty()
                || local.type_identity != target_identity
                || structural_access_for_type_reference(program, target.type_reference)?
                    != CheckedStructuralAccess::Owned
                || !exact_transfer
            {
                return None;
            }
            output.push(CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                    declaration_ordinal: local.declaration_ordinal,
                },
                path: Vec::new(),
                type_identity: target_identity,
                access: CheckedStructuralAccess::Owned,
            });
            continue;
        }
        arm("call operation: structural arguments: view local");
        // Forward an existing descriptor through the same exact root/endpoint
        // operation as a computed call. Array-to-view formation remains with
        // the parameter presentation below: it borrows backing storage instead.
        if let Some(plan) = computation_arguments::borrowed_view_argument(
            program,
            &facts.borrow,
            caller_machine.symbol,
            caller_state,
            call,
            &place,
            target,
            Some(&selector),
        ) {
            output.push(plan);
            continue;
        }
        arm("call operation: structural arguments: borrowed local");
        // Formal and local roots share the same exact borrowed endpoint join.
        // A projected descriptor must cross its stored reference boundary in
        // both coordinates; type-only parameter presentation loses that hop.
        if let Some(plan) = computation_arguments::borrowed_storage_argument(
            program,
            &facts.borrow,
            caller_machine.symbol,
            caller_state,
            call,
            &place,
            target,
            Some(&selector),
        ) {
            output.push(plan);
            continue;
        }
        arm("call operation: structural arguments: parameter source");
        let source = SourceParameter::resolve(source_parameters, caller_parameters, source_symbol)?;
        let target_access = structural_access_for_type_reference(program, target.type_reference);
        let custody = source.custody(target_access);
        // A closed primitive array parameter passes only whole, owned and
        // unrestricted into an owned closed-array formal. Provider selection
        // changes the implementation, never the argument's source custody.
        if validation::is_closed_primitive_array_type(program, source.declaration.type_reference)
            && !(place.segments.is_empty()
                && custody.access == CheckedStructuralAccess::Owned
                && custody.multiplicity == Multiplicity::Unrestricted
                && custody.unqualified
                && validation::is_closed_primitive_array_type(program, target.type_reference)
                && target_access == Some(CheckedStructuralAccess::Owned))
        {
            return None;
        }
        arm("call operation: structural arguments: parameter path");
        let argument = ParameterArgument {
            program,
            machine: caller_machine.symbol,
            state: caller_state.symbol,
            statement_index,
            place: &place,
            target: target.type_reference,
            target_identity,
            selectors: Some(&selector),
        };
        // Bodyless nominal requirements use the same boundary presentation
        // as trait requirements. The declared supply owns this distinction;
        // placing a call in a selected arm does not change its argument ABI.
        let path = if callee.has_bodyless_signature() {
            boundary_parameter_path(argument, custody, &[])
        } else {
            ordinary_parameter_path(argument, custody, callee.supply_mode)
        }?;
        arm("call operation: structural arguments: parameter identity");
        if path.is_empty()
            && !argument.whole_parameter_is_admitted(
                source,
                callee.supply_mode == MachineSupplyMode::CheckedBody
                    && custody.lends_unrestricted_borrow(),
                &[],
            )
        {
            return None;
        }
        arm("call operation: structural arguments: parameter access");
        let target_access = target_access?;
        output.push(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(source.index).ok()?,
            },
            path,
            type_identity: target_identity,
            access: if restored_alias.is_some() {
                target_access
            } else {
                let authored_access = exact_structural_argument_access(
                    program,
                    facts,
                    caller_machine.symbol,
                    caller_state.symbol,
                    call,
                    &authored_place,
                    target_access,
                )?;
                if authored_access == target_access {
                    authored_access
                } else if authored_access == CheckedStructuralAccess::MutableBorrow
                    && target_access == CheckedStructuralAccess::SharedBorrow
                {
                    // A `&mut` reborrow weakens to the callee's shared loan:
                    // the caller's mutable custody already covers the read
                    // the callee performs.
                    target_access
                } else {
                    // A borrow carrier passed bare moves its reference: the
                    // call records only a Read of that value, while the erased
                    // loan's own kind is the authority the argument forwards.
                    // The alias's exact last use pins the move to this call.
                    alias_forwarded_access(
                        &borrow_aliases,
                        &authored_place,
                        authored_access,
                        target_access,
                        call.statement_index,
                    )?
                }
            },
        });
    }
    arm("call operation: structural arguments: argument count");
    if explicit_index != explicit_arguments.len() {
        return None;
    }
    Some(output)
}

/// The access a moved borrow-carrier argument actually forwards. The authored
/// call records a Read of the carrier's reference value, so the ordinary
/// access evidence reports SharedBorrow while the erased loan itself supplies
/// the target's exclusive access. Admission requires exactly that shape: a
/// bare, segment-free alias root, only a carrier read at the call, the alias's
/// terminal use at this statement, and the loan kind matching the target
/// access exactly.
fn alias_forwarded_access(
    aliases: &[crate::execution::terminal_unit::receiver_aliases::ReceiverAlias],
    authored_place: &validation::CanonicalPlace,
    authored_access: CheckedStructuralAccess,
    target_access: CheckedStructuralAccess,
    statement_index: usize,
) -> Option<CheckedStructuralAccess> {
    if authored_access != CheckedStructuralAccess::SharedBorrow
        || !authored_place.segments.is_empty()
    {
        return None;
    }
    let owner = authored_place.root.as_symbol()?;
    let alias = aliases.iter().find(|alias| alias.owner == owner)?;
    if alias.last_use != statement_index {
        return None;
    }
    let forwarded = match alias.kind {
        checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
        checked_trees::BorrowAccessKind::Mutable => CheckedStructuralAccess::MutableBorrow,
        checked_trees::BorrowAccessKind::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    };
    (forwarded == target_access).then_some(target_access)
}

/// Translate the one bare reference carrier admitted by the checked
/// post-reactivation certificate back to its exact restored structural place.
/// This is intentionally not a general local-alias resolver.
fn reborrow_restored_call_alias_target(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    authored_place: &validation::CanonicalPlace,
) -> Option<validation::CanonicalPlace> {
    let authored_root = authored_place.root.as_symbol()?;
    if !authored_place.segments.is_empty() {
        return None;
    }
    let candidates = facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .filter_map(|(_, event)| {
            if event.machine_symbol != machine
                || event.state_symbol != state
                || !event.restored_call.is_valid()
            {
                return None;
            }
            let checked_trees::CheckedParentBorrowResource::DirectRoot {
                resource: parent_resource,
            } = event.parent_resource
            else {
                return None;
            };
            if !facts.borrow.direct_loan_resources.is_valid(parent_resource) {
                return None;
            }
            let parent = facts.borrow.direct_loan_resources.get(parent_resource);
            if parent.owner_symbol != authored_root
                || parent.access != checked_trees::BorrowAccessKind::Mutable
            {
                return None;
            }
            let certified_call = facts.flow.control.calls.get(event.restored_call);
            (certified_call.statement_index == call.statement_index
                && certified_call.call_ordinal == call.call_ordinal
                && certified_call.target_symbol == call.target_symbol
                && certified_call.receiver_symbol == call.receiver_symbol
                && certified_call.has_receiver == call.has_receiver
                && certified_call.accesses == call.accesses)
                .then_some(validation::CanonicalPlace {
                    root: parent.captured_place.root,
                    segments: parent.captured_place.segments.clone(),
                })
        })
        .collect::<Vec<_>>();
    let [place] = candidates.as_slice() else {
        return None;
    };
    Some(place.clone())
}

/// Translate any member of the exact two- or three-child shared-freeze cohort for
/// its final observation call. The checked restoration certificate remains
/// the authority: every bare child alias must occur exactly once in the call,
/// every target parameter must be a shared reference, and the certified
/// whole-parent mutation must be the immediately following statement.
fn reborrow_restored_shared_cohort_observation_alias_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    caller_state: &typed_trees::state::State,
    callee: super::CallTarget<'_>,
    call: &checked_trees::FlowCallFact,
    call_site: &validation::CallSite<'_>,
    authored_place: &validation::CanonicalPlace,
) -> Option<validation::CanonicalPlace> {
    let authored_root = authored_place.root.as_symbol()?;
    if !authored_place.segments.is_empty() || call.call_ordinal != 0 {
        return None;
    }

    let target_parameters = callee.parameters;
    // The cohort's observation is an empty checked body; a callee without an
    // inspectable declaration cannot be shown to be one.
    if callee.supply_mode != MachineSupplyMode::CheckedBody
        || callee.declaration.is_none_or(|(_, state)| {
            !program
                .statement_table
                .statements(state.statement_nodes)
                .is_empty()
        })
        || !is_unit(program, callee.return_type)
        || !matches!(target_parameters.len(), 2 | 3)
        || target_parameters.iter().any(|parameter| {
            structural_access_for_type_reference(program, parameter.type_reference)
                != Some(CheckedStructuralAccess::SharedBorrow)
        })
    {
        return None;
    }
    let arguments = validation::call_site_argument_expressions(program, call_site);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    let argument_roots = arguments
        .iter()
        .map(|expression| {
            let place = validation::canonical_place_from_expression_in_state(
                program,
                caller_state.symbol,
                call.statement_index,
                *expression,
            )?;
            let root = place.root.as_symbol()?;
            place.segments.is_empty().then_some(root)
        })
        .collect::<Option<Vec<_>>>()?;
    if !argument_roots
        .iter()
        .enumerate()
        .all(|(index, root)| !argument_roots[..index].contains(root))
    {
        return None;
    }

    let candidates = facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .filter_map(|(_, event)| {
            if event.machine_symbol != machine
                || event.state_symbol != caller_state.symbol
                || !event.restored_call.is_valid()
            {
                return None;
            }
            let checked_trees::CheckedParentBorrowResource::DirectRoot {
                resource: parent_resource,
            } = event.parent_resource
            else {
                return None;
            };
            if !facts.borrow.direct_loan_resources.is_valid(parent_resource) {
                return None;
            }
            let parent = facts.borrow.direct_loan_resources.get(parent_resource);
            let certified_call = facts.flow.control.calls.get(event.restored_call);
            if certified_call.target_symbol == call.target_symbol
                || certified_call.statement_index != call.statement_index.checked_add(1)?
                || certified_call.call_ordinal != 0
                || event.shared_cohort.len() != target_parameters.len()
            {
                return None;
            }
            let member_roots = event
                .shared_cohort
                .iter()
                .map(|member| {
                    if !facts.borrow.reborrow_loan_resources.is_valid(*member) {
                        return None;
                    }
                    let member = facts.borrow.reborrow_loan_resources.get(*member);
                    (member.machine_symbol == machine
                        && member.state_symbol == caller_state.symbol
                        && member.access == checked_trees::BorrowAccessKind::Read)
                        .then_some(member.owner_symbol)
                })
                .collect::<Option<Vec<_>>>()?;
            if !member_roots
                .iter()
                .enumerate()
                .all(|(index, root)| !member_roots[..index].contains(root))
                || argument_roots != member_roots
                || !member_roots.contains(&authored_root)
            {
                return None;
            }
            Some(validation::CanonicalPlace {
                root: parent.captured_place.root,
                segments: parent.captured_place.segments.clone(),
            })
        })
        .collect::<Vec<_>>();
    let [place] = candidates.as_slice() else {
        return None;
    };
    Some(place.clone())
}

/// The `(root, segments)` spelling the borrow facts record for one authored
/// place. An attached-data field is its own borrow root, so a place rooted at
/// the attached machine with a leading field segment is recorded as that
/// field with the remaining segments; every other place is already spelled
/// the way it was authored.
fn borrow_access_spelling(
    machine: SymbolHandle,
    place: &validation::CanonicalPlace,
) -> Option<(SymbolHandle, &[facts::PlaceSegment])> {
    let root = place.root.as_symbol()?;
    if root == machine
        && let Some((facts::PlaceSegment::Field { symbol }, remaining)) =
            place.segments.split_first()
    {
        return Some((*symbol, remaining));
    }
    Some((root, place.segments.as_slice()))
}

pub(crate) fn exact_structural_argument_access(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    place: &validation::CanonicalPlace,
    target_access: CheckedStructuralAccess,
) -> Option<CheckedStructuralAccess> {
    let returned_loans = program
        .state_by_symbol(state)
        .and_then(|source_state| {
            let local = program
                .statement_table
                .statement_at(source_state, call.statement_index)?
                .as_local_data()?;
            if crate::execution::terminal_unit::reference_results::is_reference_record(
                program,
                local.type_reference,
            ) {
                if call.authored_expression != local.initial_value {
                    return None;
                }
                return crate::execution::terminal_unit::reference_results::local_record_loans(
                    program,
                    facts,
                    machine,
                    source_state,
                    u32::try_from(call.statement_index).ok()?,
                )
                .and_then(|leaves| {
                    let mut captured = Vec::new();
                    for (_, group) in leaves {
                        captured.extend(facts.borrow.loan_group(group)?.map(|(loan, _)| loan));
                    }
                    Some(captured)
                });
            }
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index: u32::try_from(call.statement_index).ok()?,
                binding_ordinal: 0,
                type_identity: program.normalized_type_identity(local.type_reference),
                multiplicity: Multiplicity::Affine,
                local: Default::default(),
            };
            crate::execution::terminal_unit::reference_results::result_loans(
                program,
                facts,
                machine,
                source_state,
                call,
                &result,
            )
            .and_then(|loans| {
                facts
                    .borrow
                    .loan_group(loans)
                    .map(|group| group.map(|(loan, _)| loan).collect::<Vec<_>>())
            })
        })
        .unwrap_or_default();
    exact_structural_borrow_access(
        program,
        &facts.borrow,
        machine,
        state,
        call,
        place,
        target_access,
        &returned_loans,
    )
}

pub(crate) fn exact_structural_borrow_access(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    place: &validation::CanonicalPlace,
    target_access: CheckedStructuralAccess,
    returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
) -> Option<CheckedStructuralAccess> {
    if target_access == CheckedStructuralAccess::Owned {
        return Some(CheckedStructuralAccess::Owned);
    }
    // The borrow model roots an attached-data field at the field's own symbol,
    // while a place reaching that field through the attached machine roots at
    // the machine. Re-express the machine-rooted place in the borrow spelling
    // before matching.
    let (root_symbol, segments) = borrow_access_spelling(machine, place)?;
    let borrow_state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state)?;
    let matching_calls = borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|candidate| {
            candidate.statement_index == call.statement_index
                && candidate.call_ordinal == call.call_ordinal
                && candidate.target_symbol == call.target_symbol
        })
        .collect::<Vec<_>>();
    let [borrow_call] = matching_calls.as_slice() else {
        return None;
    };
    let kinds = borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .filter(|access| {
            // None retains an access through invocation. A recorded end belongs
            // to operand evaluation, whose conflicts are checked with the full
            // roster; it must not change this invocation's access classification.
            access.end_before_access.is_none()
                && borrow.access_segments(access) == segments
                && access.root == facts::PlaceRoot::Symbol(root_symbol)
        })
        .map(|access| &access.kind)
        .collect::<Vec<_>>();
    let Some(first) = kinds.first() else {
        return None;
    };
    if kinds.iter().any(|candidate| *candidate != *first) {
        return None;
    }
    if target_access == CheckedStructuralAccess::MutableBorrow
        && **first == checked_trees::BorrowAccessKind::Read
        && segments.is_empty()
        && reference_forwarding::preserves_mutable_referent(
            program,
            borrow,
            borrow_state,
            borrow_call,
            call,
            root_symbol,
            returned_loans,
        )
    {
        return Some(CheckedStructuralAccess::MutableBorrow);
    }
    Some(match *first {
        checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
        checked_trees::BorrowAccessKind::Mutable => CheckedStructuralAccess::MutableBorrow,
        checked_trees::BorrowAccessKind::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    })
}

/// Join owned arguments to the call's complete linear permission-event set.
/// Argument preparation already checks each source, path and loan. A loan
/// never transfers its backing's claim, independently of whether it comes
/// from a parameter, local, projection or computed result. Event coverage
/// below still refuses a purported owned transfer with no owned argument.
pub(crate) fn call_claim_transfers(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    arguments: &[CheckedUnitStructuralArgumentPlan],
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = call_claim_events(&facts.flow, machine, state, call);
    let mut output = Vec::new();
    let caller_state = program.state_by_symbol(state)?;
    let source_parameters = program.state_parameters(caller_state);
    for (argument_index, argument) in arguments.iter().enumerate() {
        if argument.access != CheckedStructuralAccess::Owned {
            continue;
        }
        if matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        ) || view_subslice::is_view_subslice(argument)
        {
            return None;
        }
        if argument.byte_sequence_literal().is_some() {
            // An owned literal is a fresh unrestricted bounded owner.
            if !argument.path.is_empty() {
                return None;
            }
            continue;
        }
        // The entry roster describes where claims arrived, not where this
        // argument holds them now. Reseating a parameter's child is the same
        // custody operation as reseating a local's child: join the current
        // call events at its exact owner and selected path.
        let root = match argument.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = caller_parameters.get(parameter_index as usize)?;
                let source = source_parameters.get(parameter.position as usize)?;
                facts::PlaceRoot::Symbol(source.symbol)
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                caller_structural_results
                    .iter()
                    .find(|(result, _)| result.binding_ordinal == binding_ordinal)?
                    .1
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                facts::PlaceRoot::Symbol(symbol)
            }
            _ if argument.source_local_declaration_ordinal().is_some()
                && argument.path.is_empty() =>
            {
                continue;
            }
            _ => return None,
        };
        for (claim_identity, path) in owner_claim_crossings(
            program,
            &facts.flow,
            state,
            call,
            &events,
            root,
            &argument.path,
        )? {
            if output
                .iter()
                .any(|transfer: &CheckedUnitClaimTransferPlan| {
                    transfer.claim_identity == claim_identity
                })
            {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
                path,
            });
        }
    }
    if output.len() != events.len() {
        return None;
    }
    Some(output)
}

/// The linear claims leaving the caller at one call: every live owned
/// transfer or consumption the permission pass recorded at its coordinate.
/// Whether the destination is a checked body or a boundary does not change
/// the explicit argument's ownership judgment; the call route decides whether
/// these rows become ingress transfers or normal-completion receipts.
fn call_claim_events<'facts>(
    flow: &'facts checked_trees::FlowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
) -> Vec<&'facts checked_trees::FlowPermissionEventFact> {
    flow.ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && matches!(
                    event.kind,
                    PermissionEventKind::Transfer | PermissionEventKind::Consume
                )
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect()
}

/// The claims one owned operand carries across its call: the events rooted at
/// the operand's owner beneath its selected path. A projected operand carries
/// only the claims beneath that path; sibling claims stay with their owner or
/// cross at another operand of the same call. Named, parameter and anonymous
/// owners share this join: the owner's root, not the operand's source
/// spelling, identifies the claims it holds.
fn owner_claim_crossings(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    events: &[&checked_trees::FlowPermissionEventFact],
    root: facts::PlaceRoot,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<
    Vec<(
        PermissionClaimIdentity,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    )>,
> {
    let mut crossing_events = events.iter().filter(|event| event.root == root).peekable();
    // An owned value with no crossing linear claims needs no claim-path type
    // lookup. Anonymous constructions still participate in ordinary custody.
    if crossing_events.peek().is_none() {
        return Some(Vec::new());
    }
    let root_type = validation::canonical_place_type_reference(
        program,
        state,
        call.statement_index,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )?;
    let mut claims = Vec::new();
    for event in crossing_events {
        let (event_path, _) = validation::structural_owned_path(
            program,
            root_type,
            flow.ownership.segments.span_or_empty(event.segments),
        )
        .ok()?;
        let Some(relative) = event_path.strip_prefix(path) else {
            continue;
        };
        // Disposal settles a claim without transferring it. It cannot supply
        // ingress or a normal boundary receipt, regardless of source shape.
        if event.kind != PermissionEventKind::Transfer
            || event.claim_identity == PermissionClaimIdentity::Unknown
            || claims
                .iter()
                .any(|(identity, _)| *identity == event.claim_identity)
        {
            return None;
        }
        claims.push((event.claim_identity, relative.to_vec()));
    }
    Some(claims)
}

/// Join a computed call's owned operands to the call's complete linear
/// permission-event set, exactly as an ordinary call operation's arguments
/// join it. Each operand names its owner through the canonical place its
/// authored actual denotes, so a parameter, a named local and an anonymous
/// construction or call result transfer the claims their owner holds. A
/// claim no owned operand carries, or one the callee consumes in place,
/// leaves the computation without a transfer plan.
pub(crate) fn computed_call_claim_transfers(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    operands: &[(
        typed_trees::expression::ExpressionHandle,
        &CheckedUnitStructuralArgumentPlan,
    )],
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = call_claim_events(flow, machine, state, call);
    let mut output = Vec::new();
    for (argument_index, (expression, argument)) in operands.iter().enumerate() {
        if argument.access != CheckedStructuralAccess::Owned {
            continue;
        }
        let root = match argument.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                facts::PlaceRoot::Expression(expression)
            }
            _ => {
                validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    call.statement_index,
                    *expression,
                )?
                .root
            }
        };
        for (claim_identity, path) in
            owner_claim_crossings(program, flow, state, call, &events, root, &argument.path)?
        {
            if output
                .iter()
                .any(|transfer: &CheckedUnitClaimTransferPlan| {
                    transfer.claim_identity == claim_identity
                })
            {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
                path,
            });
        }
    }
    (output.len() == events.len()).then_some(output)
}
