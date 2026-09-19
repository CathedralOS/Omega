//! Structural call arguments: their custody, restored reborrow aliases,
//! exact borrow access and the claim transfers a call performs.

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::byte_subslice;
use crate::execution::terminal_unit::calls::argument_paths::{
    byte_sequence_literal_argument, projected_argument_path, projected_argument_path_with_identity,
};
use crate::execution::terminal_unit::calls::boundary_admission::{
    boundary_argument_presentation_is_admitted, fixed_byte_array_mutable_view_is_admitted,
    is_registered_boundary_scalar_target,
};
use crate::execution::terminal_unit::calls::computation_arguments;
use crate::execution::terminal_unit::calls::reference_forwarding;
use crate::execution::terminal_unit::calls::result_arguments;
use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitClaimTransferPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan, MachineSupplyMode,
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PrimitiveType, ShapeCollector, StatementNode, SymbolHandle, TypedTrees,
    attached_data_identity, base_type_identity, byte_sequence_type_identity, is_reference, is_unit,
    parameter_root_symbol, scalar_targets, strips_erased_parameter,
    structural_access_for_type_reference,
};

pub(crate) fn structural_call_arguments(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: Option<ScalarCalleePlans<'_>>,
    call: &checked_trees::FlowCallFact,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    call_site: &crate::semantic_calls::CallSite<'_>,
    receiver_symbol: SymbolHandle,
    statement_index: usize,
    allow_fixed_index_projection: bool,
    allow_field_path_projection: bool,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
    let source_parameters = program.state_parameters(caller_state);
    let target_parameters = program.state_parameters(target_state);
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
    let explicit_arguments =
        crate::semantic_calls::call_site_argument_expressions(program, call_site);
    let explicit_self = explicit_arguments.len()
        > target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let mut explicit_index = 0usize;
    let mut output = Vec::new();
    let mut structural_argument_ordinal = 0usize;

    for target in target_parameters {
        // The erased position's authored argument is proof material with no
        // structural or scalar transfer; consume it and plan nothing.
        if strips_erased_parameter(target)? {
            explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            continue;
        }
        if program
            .primitive_type_reference(target.type_reference)
            .is_some()
        {
            if target.is_self {
                return None;
            }
            explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            continue;
        }
        let argument_ordinal = structural_argument_ordinal;
        if !target.is_self || explicit_self {
            structural_argument_ordinal = structural_argument_ordinal.checked_add(1)?;
        }
        let authored_place = if target.is_self {
            if is_reference(program, target.type_reference) {
                // The completed Unit callee decides whether its borrowed self
                // survives attachment specialization. receiver_calls rejoins
                // retained receiver arguments after those plans are available.
                continue;
            }
            if explicit_self {
                let expression = *explicit_arguments.get(explicit_index)?;
                explicit_index += 1;
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    caller_state.symbol,
                    statement_index,
                    expression,
                )?
            } else {
                crate::flow::owned_method_receiver_place(
                    program,
                    caller_state.symbol,
                    statement_index,
                    call_site,
                    target_parameters,
                    receiver_symbol,
                )
                .or_else(|| crate::flow::canonical_place_from_symbol(receiver_symbol))?
            }
        } else {
            let expression = *explicit_arguments.get(explicit_index)?;
            explicit_index += 1;
            if target_machine.supply_mode == MachineSupplyMode::CheckedBody
                && let Some(literal) =
                    byte_sequence_literal_argument(program, target.type_reference, expression)
            {
                output.push(literal);
                continue;
            }
            if target_machine.supply_mode == MachineSupplyMode::CheckedBody
                && is_unit(program, target_state.return_type)
                && let Some(subslice) = byte_subslice::argument(
                    program,
                    facts,
                    caller_machine,
                    caller_state,
                    caller_parameters,
                    target.type_reference,
                    expression,
                    statement_index,
                    call.call_ordinal,
                    argument_ordinal,
                )
            {
                output.push(subslice);
                continue;
            }
            crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state.symbol,
                statement_index,
                expression,
            )
            .or_else(|| {
                caller_structural_results
                    .iter()
                    .find(|(_, root)| *root == facts::PlaceRoot::Expression(expression))
                    .map(|_| crate::flow::CanonicalPlace {
                        root: facts::PlaceRoot::Expression(expression),
                        segments: Vec::new(),
                    })
            })?
        };
        let restored_alias = reborrow_restored_call_alias_target(
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
                target_machine,
                target_state,
                call,
                call_site,
                &authored_place,
            )
        });
        let resolved_alias = if restored_alias.is_none() {
            crate::execution::terminal_unit::receiver_aliases::resolve(
                &borrow_aliases,
                &authored_place,
            )
        } else {
            None
        };
        let place = restored_alias
            .clone()
            .or_else(|| resolved_alias.clone())
            .unwrap_or_else(|| authored_place.clone());
        let target_identity = if target.is_self {
            attached_data_identity(program, target_machine)?
        } else if byte_sequence_carrier(program, target.type_reference, &[]).is_some() {
            byte_sequence_type_identity(program, target.type_reference, &[], &[])?
        } else {
            base_type_identity(program, target.type_reference, &[])?
        };
        if let Some((result, _)) = caller_structural_results
            .iter()
            .find(|(_, root)| *root == place.root)
        {
            if result.multiplicity == Multiplicity::Unrestricted
                && target_machine.supply_mode != MachineSupplyMode::CheckedBody
            {
                return None;
            }
            if !target_machine.supply_mode.is_boundary_declaration()
                && (target_machine.supply_mode != MachineSupplyMode::CheckedBody
                    || (!is_unit(program, target_state.return_type)
                        && !validation::is_closed_primitive_array_type(
                            program,
                            target_state.return_type,
                        )
                        && crate::execution::terminal_unit::control::checked_structural_result_type(
                            program,
                            &mut ShapeCollector::new(program),
                            target_state.return_type,
                            &[],
                        )
                        .is_none()
                        && !program
                            .primitive_type_reference(target_state.return_type)
                            .is_some_and(|result| {
                                is_registered_boundary_scalar_target(
                                    scalar_callees,
                                    target_machine.symbol,
                                    target_state.symbol,
                                    result,
                                ) || scalar_targets::registered_primitive_store_target(
                                    program,
                                    facts,
                                    scalar_callees,
                                    target_machine.symbol,
                                    target_state.symbol,
                                    result,
                                )
                                .is_some()
                                    || scalar_targets::registered_structural_graph_target(
                                        program,
                                        facts,
                                        scalar_callees,
                                        target_machine.symbol,
                                        target_state.symbol,
                                        result,
                                    )
                                    .is_some()
                            })))
            {
                return None;
            }
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
                &target_identity,
                allow_field_path_projection
                    && target_machine.supply_mode == MachineSupplyMode::CheckedBody
                    && is_unit(program, target_state.return_type),
            )?);
            continue;
        }
        let facts::PlaceRoot::Symbol(source_symbol) = place.root else {
            return None;
        };
        if crate::execution::terminal_unit::primitive_store::primitive_local_before(
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
                &authored_place,
                target.type_reference,
            )?);
            continue;
        }
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
        // An authored `self.field` argument roots at the `self` parameter's own
        // symbol, while an implicit receiver place roots at the machine the
        // parameter is attached to. Both spellings name one parameter.
        let source_parameter = source_parameters.iter().find(|parameter| {
            parameter_root_symbol(caller_machine.symbol, parameter) == source_symbol
                || parameter.symbol == source_symbol
        })?;
        let source_index = caller_parameters.iter().position(|candidate| {
            candidate.position
                == u32::try_from(
                    source_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == source_parameter.symbol)
                        .unwrap_or(usize::MAX),
                )
                .unwrap_or(u32::MAX)
        })?;
        if validation::is_closed_primitive_array_type(program, source_parameter.type_reference)
            && (target_machine.supply_mode != MachineSupplyMode::CheckedBody
                || !place.segments.is_empty()
                || caller_parameters[source_index].access != CheckedStructuralAccess::Owned
                || caller_parameters[source_index].multiplicity != Multiplicity::Unrestricted
                || !caller_parameters[source_index].qualifications.is_empty()
                || !validation::is_closed_primitive_array_type(program, target.type_reference)
                || structural_access_for_type_reference(program, target.type_reference)?
                    != CheckedStructuralAccess::Owned)
        {
            return None;
        }
        let source_identity = caller_parameters.get(source_index)?.type_identity.clone();
        let path = match place.segments.as_slice() {
            [] => Vec::new(),
            segments
                if !segments.is_empty()
                    && target_machine.supply_mode == MachineSupplyMode::CheckedBody
                    && caller_parameters[source_index].access
                        == CheckedStructuralAccess::MutableBorrow
                    && caller_parameters[source_index].qualifications.is_empty()
                    && structural_access_for_type_reference(program, target.type_reference)?
                        == CheckedStructuralAccess::MutableBorrow
                    && byte_sequence_carrier(program, target.type_reference, &[])
                        == Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                    && segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) =>
            {
                let (projected_type, path) =
                    projected_argument_path(program, caller_state.symbol, statement_index, &place)?;
                if !boundary_argument_presentation_is_admitted(
                    program,
                    projected_type,
                    target.type_reference,
                    &target_identity,
                    &[],
                ) && !(caller_parameters[source_index].multiplicity
                    == Multiplicity::Unrestricted
                    && segments
                        .iter()
                        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
                    && fixed_byte_array_mutable_view_is_admitted(
                        program,
                        projected_type,
                        target.type_reference,
                    ))
                {
                    return None;
                }
                path
            }
            segments
                if target_machine.supply_mode == MachineSupplyMode::CheckedBody
                    && caller_parameters[source_index].multiplicity
                        == Multiplicity::Unrestricted
                    && caller_parameters[source_index].qualifications.is_empty()
                    && matches!(
                        (
                            caller_parameters[source_index].access,
                            structural_access_for_type_reference(program, target.type_reference)?
                        ),
                        (
                            CheckedStructuralAccess::MutableBorrow,
                            CheckedStructuralAccess::SharedBorrow
                                | CheckedStructuralAccess::MutableBorrow
                                | CheckedStructuralAccess::WriteOnlyBorrow
                        ) | (
                            CheckedStructuralAccess::SharedBorrow,
                            CheckedStructuralAccess::SharedBorrow
                        ) | (
                            CheckedStructuralAccess::WriteOnlyBorrow,
                            CheckedStructuralAccess::WriteOnlyBorrow
                        )
                    )
                    && segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) =>
            {
                // Static projection changes the selected referent, not loan
                // authority. One exact path/type/bounds reconstruction serves
                // fields and arrays at every depth; authored access and alias
                // custody are still rejoined below. Byte-view presentation and
                // owned partial transfers retain their separate contracts.
                projected_argument_path_with_identity(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                    &target_identity,
                )?
            }
            segments
                if allow_field_path_projection
                    && allow_fixed_index_projection
                    && target_machine.supply_mode == MachineSupplyMode::CheckedBody
                    && caller_parameters[source_index].multiplicity == Multiplicity::Affine
                    && caller_parameters[source_index].access == CheckedStructuralAccess::Owned
                    && caller_parameters[source_index].qualifications.is_empty()
                    && structural_access_for_type_reference(program, target.type_reference)?
                        == CheckedStructuralAccess::Owned
                    && segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) =>
            {
                projected_argument_path_with_identity(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                    &target_identity,
                )?
            }
            [facts::PlaceSegment::FixedIndex { index }]
                if allow_fixed_index_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                projected_argument_path_with_identity(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                    &target_identity,
                )?
            }
            segments @ [facts::PlaceSegment::FixedIndex { .. }, ..]
                if (matches!(segments.len(), 2 | 3)
                    || (target_machine.supply_mode == MachineSupplyMode::CheckedBody
                        && caller_parameters.get(source_index)?.access
                            == CheckedStructuralAccess::WriteOnlyBorrow
                        && structural_access_for_type_reference(
                            program,
                            target.type_reference,
                        )? == CheckedStructuralAccess::WriteOnlyBorrow))
                    && segments.iter().all(|segment| {
                        matches!(segment, facts::PlaceSegment::FixedIndex { .. })
                    })
                    && allow_fixed_index_projection
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                projected_argument_path_with_identity(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                    &target_identity,
                )?
            }
            segments @ [facts::PlaceSegment::Field { .. }, ..]
                if (allow_field_path_projection
                    && segments
                        .iter()
                        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. })))
                    && caller_parameters
                        .get(source_index)?
                        .qualifications
                        .is_empty() =>
            {
                projected_argument_path_with_identity(
                    program,
                    caller_state.symbol,
                    statement_index,
                    &place,
                    &target_identity,
                )?
            }
            _ => return None,
        };
        if path.is_empty()
            && source_identity != target_identity
            && !(target_machine.supply_mode == MachineSupplyMode::CheckedBody
                && is_unit(program, target_state.return_type)
                && caller_parameters[source_index].access == CheckedStructuralAccess::MutableBorrow
                && caller_parameters[source_index].multiplicity == Multiplicity::Unrestricted
                && caller_parameters[source_index].qualifications.is_empty()
                && fixed_byte_array_mutable_view_is_admitted(
                    program,
                    source_parameter.type_reference,
                    target.type_reference,
                ))
        {
            return None;
        }
        output.push(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(source_index).ok()?,
            },
            path,
            type_identity: target_identity,
            access: if restored_alias.is_some() {
                structural_access_for_type_reference(program, target.type_reference)?
            } else {
                let target_access =
                    structural_access_for_type_reference(program, target.type_reference)?;
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
    authored_place: &crate::flow::CanonicalPlace,
    authored_access: CheckedStructuralAccess,
    target_access: CheckedStructuralAccess,
    statement_index: usize,
) -> Option<CheckedStructuralAccess> {
    if authored_access != CheckedStructuralAccess::SharedBorrow
        || !authored_place.segments.is_empty()
    {
        return None;
    }
    let facts::PlaceRoot::Symbol(owner) = authored_place.root else {
        return None;
    };
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
    authored_place: &crate::flow::CanonicalPlace,
) -> Option<crate::flow::CanonicalPlace> {
    let facts::PlaceRoot::Symbol(authored_root) = authored_place.root else {
        return None;
    };
    if !authored_place.segments.is_empty() {
        return None;
    }
    let candidates = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            if certificate.machine_symbol != machine
                || certificate.state_symbol != state
                || certificate.target_symbol != call.target_symbol
                || certificate.carrier_place.root_symbol != authored_root
                || !certificate.carrier_place.segments.is_empty()
                || certificate.access != checked_trees::BorrowAccessKind::Mutable
                || !facts.flow.control.calls.is_valid(certificate.call)
            {
                return None;
            }
            let certified_call = facts.flow.control.calls.get(certificate.call);
            (certified_call.statement_index == call.statement_index
                && certified_call.call_ordinal == call.call_ordinal
                && certified_call.target_symbol == call.target_symbol
                && certified_call.receiver_symbol == call.receiver_symbol
                && certified_call.has_receiver == call.has_receiver
                && certified_call.accesses == call.accesses)
                .then_some(crate::flow::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(certificate.restored_place.root_symbol),
                    segments: certificate.restored_place.segments.clone(),
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
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    caller_state: &typed_trees::state::State,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    call_site: &crate::semantic_calls::CallSite<'_>,
    authored_place: &crate::flow::CanonicalPlace,
) -> Option<crate::flow::CanonicalPlace> {
    let facts::PlaceRoot::Symbol(authored_root) = authored_place.root else {
        return None;
    };
    if !authored_place.segments.is_empty() || call.call_ordinal != 0 {
        return None;
    }

    let target_parameters = program.state_parameters(target_state);
    if target_machine.supply_mode != MachineSupplyMode::CheckedBody
        || !program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
        || !is_unit(program, target_state.return_type)
        || !matches!(target_parameters.len(), 2 | 3)
        || target_parameters.iter().any(|parameter| {
            parameter.is_self
                || structural_access_for_type_reference(program, parameter.type_reference)
                    != Some(CheckedStructuralAccess::SharedBorrow)
        })
    {
        return None;
    }
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, call_site);
    if arguments.len() != target_parameters.len() {
        return None;
    }
    let argument_roots = arguments
        .iter()
        .map(|expression| {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                caller_state.symbol,
                call.statement_index,
                *expression,
            )?;
            let facts::PlaceRoot::Symbol(root) = place.root else {
                return None;
            };
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
        .reborrow_restored_call_use_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            if certificate.machine_symbol != machine
                || certificate.state_symbol != caller_state.symbol
                || certificate.target_symbol == call.target_symbol
                || !facts.flow.control.calls.is_valid(certificate.call)
                || !facts
                    .borrow
                    .reborrow_disposition_events
                    .is_valid(certificate.disposition)
            {
                return None;
            }
            let certified_call = facts.flow.control.calls.get(certificate.call);
            let disposition = facts
                .borrow
                .reborrow_disposition_events
                .get(certificate.disposition);
            if certified_call.statement_index != call.statement_index.checked_add(1)?
                || certified_call.call_ordinal != 0
                || disposition.shared_cohort.len() != target_parameters.len()
            {
                return None;
            }
            let member_roots = disposition
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
            Some(crate::flow::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(certificate.restored_place.root_symbol),
                segments: certificate.restored_place.segments.clone(),
            })
        })
        .collect::<Vec<_>>();
    let [place] = candidates.as_slice() else {
        return None;
    };
    Some(place.clone())
}

/// The `(root, segments)` spelling the borrow facts record for one authored
/// place. An attached-data field is its own borrow root, so `self.line` is
/// recorded as `line` with no segments; every other place is already spelled
/// the way it was authored.
fn borrow_access_spelling<'place>(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    place: &'place crate::flow::CanonicalPlace,
) -> Option<(SymbolHandle, &'place [facts::PlaceSegment])> {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    if (root == machine
        || crate::semantic_calls::find_state(program, state).is_some_and(|state| {
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self && parameter.symbol == root)
        }))
        && let Some((facts::PlaceSegment::Field { symbol }, remaining)) =
            place.segments.split_first()
    {
        return Some((*symbol, remaining));
    }
    Some((root, place.segments.as_slice()))
}

pub(crate) fn exact_structural_argument_access(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_access: CheckedStructuralAccess,
) -> Option<CheckedStructuralAccess> {
    let returned_loans = crate::semantic_calls::find_state(program, state)
        .and_then(|source_state| {
            let StatementNode::LocalData(local) = program
                .statement_table
                .statements(source_state.statement_nodes)
                .get(call.statement_index)?
            else {
                return None;
            };
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
                .map(|loans| loans.into_iter().map(|(_, loan)| loan).collect::<Vec<_>>());
            }
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index: u32::try_from(call.statement_index).ok()?,
                binding_ordinal: 0,
                type_identity: program
                    .normalized_type_identity(local.type_reference)
                    .into_string(),
                multiplicity: Multiplicity::Affine,
            };
            crate::execution::terminal_unit::reference_results::result_loan(
                program,
                facts,
                machine,
                source_state,
                call,
                &result,
            )
            .map(|loan| vec![loan])
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
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_access: CheckedStructuralAccess,
    returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
) -> Option<CheckedStructuralAccess> {
    if target_access == CheckedStructuralAccess::Owned {
        return Some(CheckedStructuralAccess::Owned);
    }
    // The borrow model roots an attached-data field at the field's own symbol,
    // while an authored `self.field` argument roots at `self`. Re-express the
    // place in the borrow spelling before matching, under the same self test
    // `canonical_place_type_reference` applies to types.
    let (root_symbol, segments) = borrow_access_spelling(program, machine, state, place)?;
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
            access.root_symbol == root_symbol && borrow.access_segments(access) == segments
        })
        .map(|access| &access.kind)
        .collect::<Vec<_>>();
    let first = kinds.first()?;
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
    Some(match first {
        checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
        checked_trees::BorrowAccessKind::Mutable => CheckedStructuralAccess::MutableBorrow,
        checked_trees::BorrowAccessKind::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    })
}

pub(crate) fn call_claim_transfers(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    arguments: &[CheckedUnitStructuralArgumentPlan],
    kind: PermissionEventKind,
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = facts
        .flow
        .ownership
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
                && event.kind == kind
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (argument_index, argument) in arguments.iter().enumerate() {
        if matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        ) {
            if !argument.path.is_empty() || argument.access == CheckedStructuralAccess::Owned {
                return None;
            }
            continue;
        }
        if argument.byte_sequence_literal().is_some()
            || matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            )
        {
            if !argument.path.is_empty() || argument.access != CheckedStructuralAccess::SharedBorrow
            {
                return None;
            }
            continue;
        }
        let source_parameter_index = argument.source_parameter_index();
        if argument
            .source_structural_result_binding_ordinal()
            .is_some()
            && argument.path.last() == Some(&CheckedUnitStructuralPathSegment::Referent)
            && argument.path[..argument.path.len() - 1]
                .iter()
                .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
            && argument.access == CheckedStructuralAccess::MutableBorrow
        {
            // Whole and record-leaf references already rejoined their exact
            // active source loan in result argument preparation. Neither
            // transfers an owned referent claim to the callee.
            continue;
        }
        if source_parameter_index.is_none() {
            if (argument.source_local_declaration_ordinal().is_none()
                && argument
                    .source_structural_result_binding_ordinal()
                    .is_none())
                || (!argument.path.is_empty()
                    && !(argument
                        .source_structural_result_binding_ordinal()
                        .is_some()
                        && argument.access == CheckedStructuralAccess::Owned))
                || (argument.access != CheckedStructuralAccess::Owned
                    && !(argument
                        .source_structural_result_binding_ordinal()
                        .is_some()
                        && argument.access == CheckedStructuralAccess::SharedBorrow))
            {
                return None;
            }
            // A whole linear result moved into this call carries the
            // producer's live claim, not an entry claim: its transfer events
            // root at the local the producing statement bound. Publish one
            // receipt per event so the call's outbound claim set stays exact.
            if let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal()
                && argument.access == CheckedStructuralAccess::Owned
            {
                let Some((_, root)) = caller_structural_results
                    .iter()
                    .find(|(result, _)| result.binding_ordinal == binding_ordinal)
                else {
                    return None;
                };
                let mut claims = Vec::new();
                for event in events.iter().filter(|event| event.root == *root) {
                    if event.claim_identity == PermissionClaimIdentity::Unknown
                        || claims.contains(&event.claim_identity)
                    {
                        return None;
                    }
                    claims.push(event.claim_identity);
                }
                for claim_identity in claims {
                    output.push(CheckedUnitClaimTransferPlan {
                        claim_identity,
                        argument_index: u32::try_from(argument_index).ok()?,
                    });
                }
            }
            continue;
        }
        let source_parameter_index = source_parameter_index?;
        let entries = entry_claims
            .iter()
            .filter(|entry| {
                entry.parameter_index == source_parameter_index
                    && (argument.path.is_empty() || entry.path == argument.path)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            if caller_parameters
                .get(source_parameter_index as usize)?
                .multiplicity
                == Multiplicity::Linear
            {
                return None;
            }
            continue;
        }
        for entry in entries {
            let matching = events
                .iter()
                .filter(|event| event.claim_identity == entry.claim_identity)
                .collect::<Vec<_>>();
            if matching.len() != 1 || entry.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity: entry.claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
            });
        }
    }
    if output.len() != events.len() {
        return None;
    }
    Some(output)
}

pub(crate) fn exact_integer_at(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<u64> {
    let matches = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: machine,
                    state_symbol: state,
                    statement_index,
                    role: checked_trees::CheckedValueStatementRole::CallArgument,
                }
        })
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return None;
    };
    if value.primitive_type != Some(expected_type) {
        return None;
    }
    let range = value.integer_range.as_ref()?;
    (range.minimum == range.maximum)
        .then(|| range.minimum.to_u64())
        .flatten()
}
