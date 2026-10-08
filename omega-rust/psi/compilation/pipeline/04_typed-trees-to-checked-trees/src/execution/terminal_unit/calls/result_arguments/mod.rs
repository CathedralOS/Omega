//! Result operands retain exact source access, ownership, and projected storage.
use crate::execution::terminal_unit::CheckFacts;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralResultBindingPlan;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::Multiplicity;
use crate::execution::terminal_unit::PermissionAccess;
use crate::execution::terminal_unit::PermissionClaimIdentity;
use crate::execution::terminal_unit::PermissionEventKind;
use crate::execution::terminal_unit::PermissionEventSource;
use crate::execution::terminal_unit::types::{
    ShapeCollector, base_type_identity, state_flow, structural_access_for_type_reference,
};

use crate::execution::terminal_unit::StateParameter;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::SymbolHandle;

use crate::execution::terminal_unit::calls::argument_paths::{
    projected_argument_path_with_identity, projected_borrowed_path,
};
use crate::execution::terminal_unit::calls::argument_presentation::ArgumentPresentation;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;

mod anonymous_shared;
mod bounded_owner;
mod reference;

pub(crate) use anonymous_shared::validate as validate_shared_temporary;

#[allow(clippy::too_many_arguments)]
pub(super) fn argument(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    place: &validation::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
    allow_projection: bool,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let mut argument = argument_contract(
        program,
        facts,
        machine,
        state,
        call,
        expression,
        place,
        result,
        parameter,
        target_identity,
        allow_projection,
    )?;
    // Bind producer operands to the current home without replacing an explicit
    // loan descriptor: its source already retains captured geometry and access.
    if matches!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
    ) && let Some(symbol) = place.root.as_symbol()
    {
        let source_state = program.state_by_symbol(state)?;
        argument.source =
            super::super::local_storage::source(program, source_state, operations, symbol)?;
    }
    Some(argument)
}

#[allow(clippy::too_many_arguments)]
fn argument_contract(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    place: &validation::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
    allow_projection: bool,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let (source_machine, source_state) = validation::find_state_with_machine(program, state)?;
    if let Some(argument) = super::super::reference_results::record_argument(
        program,
        facts,
        machine,
        source_state,
        u32::try_from(call.statement_index).ok()?,
        expression,
        result,
        parameter.type_reference,
    ) {
        return Some(argument);
    }
    if let Some(StatementNode::LocalData(local)) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
        && reference::is_carrier(program, local.type_reference)
        // Anonymous operand results carry their consuming statement's index,
        // which may be a `&mut` local binding that does not own this operand
        // place. Only an operand rooted at that local belongs to this lane;
        // other roots continue to the general argument checks below.
        && place.root == facts::PlaceRoot::Symbol(local.symbol)
    {
        return reference::argument(
            program,
            machine,
            source_state,
            call.statement_index,
            place,
            local,
            result,
            parameter,
            target_identity,
        );
    }
    // A bounded byte owner local lends its whole carrier to a formal of the
    // same carrier; the loan leaves the owner in place.
    if let Some(argument) = bounded_owner::loan(
        program,
        facts,
        machine,
        source_state,
        call,
        expression,
        place,
        result,
        parameter,
        target_identity,
    ) {
        return Some(argument);
    }
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    let formal_type = parameter.type_reference;
    let projected = !place.segments.is_empty();
    let unrestricted = result.multiplicity == Multiplicity::Unrestricted;
    let linear = result.multiplicity == Multiplicity::Linear;
    // A projected borrow — shared, mutable or write-only — lends a subtree of
    // a result binding that stays live after the call. The loan evidence
    // comes from the borrow record below, rejoined per access kind, rather
    // than from an owned-projection permission, so this lane is separate
    // from `allow_projection`: no custody moves.
    let projected_borrow = projected && access != CheckedStructuralAccess::Owned;
    // A range lends the original array's window, not the element type that
    // generic indexing projects. Reuse the parameter operand's range walk,
    // including omitted endpoints resolved from the declared backing extent.
    let byte_window = projected_borrow
        .then(|| {
            let Some(borrow) = program.expression_table.borrow_expression(expression) else {
                return None;
            };
            if !validation::has_builtin_subslice_meaning(
                program,
                source_machine,
                Some(source_state),
                borrow.target,
            ) {
                return None;
            }
            super::argument_paths::fixed_byte_array_range_path(
                program,
                state,
                call.statement_index,
                place,
                parameter.type_reference,
            )
            .or_else(|| {
                super::argument_paths::open_byte_range_path(
                    program,
                    state,
                    call.statement_index,
                    place,
                    parameter.type_reference,
                )
            })
        })
        .flatten();
    // Root custody and selected storage are different coordinates. A result
    // owns the declared local even when an argument lends an array field or
    // one of its windows; presentation checks the selected storage instead.
    // Resolve that storage and path once for every projected loan.
    let borrowed_storage = if projected_borrow {
        byte_window.or_else(|| {
            projected_borrowed_path(program, machine, state, call.statement_index, place)
        })
    } else {
        None
    };
    let storage_type = if projected {
        borrowed_storage.as_ref().map(|(storage, _)| *storage)
    } else {
        whole_local_storage_type(program, source_state, place, result)
    };
    // A local array lends the same element view as an array parameter or
    // field. The local keeps its backing identity; the call presents only
    // the checked view, with its exact loan replayed below.
    let presentation = storage_type.and_then(|storage| {
        super::argument_presentation::classify(
            program,
            storage,
            parameter.type_reference,
            target_identity,
            &[],
        )
    });
    let array_view = access != CheckedStructuralAccess::Owned
        && presentation == Some(ArgumentPresentation::FixedArraySlice);
    // A projected owned-record operand carries its subtree's captured leaves
    // into the call; the same resolved path both proves that custody and
    // names the argument's projected edge below. A projected borrow resolves
    // the same path to name the loaned subtree.
    let projected_path =
        if projected {
            if projected_borrow {
                // An element or field borrow resolves runtime selectors the way
                // the parameter lanes do: the segment carries the checked
                // `RuntimeIndex` coordinate Terminal re-proves against the
                // container's extent.
                let (_, path) = borrowed_storage?;
                let presentation = presentation?;
                // Raw array views retain initialized field-only backing. Indexing
                // a collection of arrays does not acquire this presentation.
                if presentation == ArgumentPresentation::FixedArraySlice
                && !path.iter().all(|segment| {
                    matches!(segment,
                        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. })
                })
            {
                return None;
            }
                Some(path)
            } else {
                projected_argument_path_with_identity(
                    program,
                    state,
                    call.statement_index,
                    place,
                    target_identity,
                )
            }
        } else {
            None
        };
    let owned_reference_record = access == CheckedStructuralAccess::Owned
        && validation::reference_result_custody::is_reference_record(
            program,
            parameter.type_reference,
        )
        && if projected {
            projected_path.as_deref().is_some_and(|path| {
                u32::try_from(call.statement_index)
                    .ok()
                    .is_some_and(|index| {
                        validation::reference_result_custody::projected_record_argument(
                            program,
                            facts,
                            machine,
                            source_state,
                            index,
                            result.statement_index,
                            path,
                        ) || match place.root {
                            // The projected carrier may be a nested call's
                            // anonymous result bound at this same statement;
                            // its returned leaf roster replays the caller's
                            // captured loans through the nested actuals.
                            facts::PlaceRoot::Expression(source) => {
                                validation::reference_result_custody::nested_call_record_argument(
                                    program,
                                    facts,
                                    machine,
                                    source_state,
                                    index,
                                    source,
                                    path,
                                )
                            }
                            _ => false,
                        }
                    })
            })
        } else {
            validation::reference_result_custody::owned_record_argument(
                program,
                facts,
                machine,
                source_state,
                u32::try_from(call.statement_index).ok()?,
                result.statement_index,
            )
        };
    // A whole view argument reads its established carrier. Check the current
    // call's exact access, not the presence of a loan owned by this local:
    // image-backed results have no caller-owned backing to loan. The producer
    // and Terminal descriptor-origin replay retain the original backing through
    // calls; recognizing literal syntax here would lose that compositionality.
    // Named reference views keep their shelled identity; an owned local lent
    // through `&token` still follows the ordinary storage-loan path below.
    // A mutable slice view forwards its descriptor the same way: the exact
    // access replay below requires the call to hold that exclusive loan.
    if !projected
        && ((matches!(
            access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        ) && crate::execution::terminal_unit::types::slice_view_carrier(
            program,
            parameter.type_reference,
        ) && result.type_identity == target_identity)
            || (access == CheckedStructuralAccess::SharedBorrow
                && crate::execution::terminal_unit::types::borrowed_named_view(
                    program,
                    parameter.type_reference,
                )
                && result.type_identity
                    == program
                        .normalized_type_identity(parameter.type_reference)
                        .as_str()))
        && let Some(view_symbol) = place.root.as_symbol()
    {
        let Some(StatementNode::LocalData(local)) = program
            .statement_table
            .statements(source_state.statement_nodes)
            .get(result.statement_index as usize)
        else {
            return None;
        };
        if local.symbol != view_symbol
            || result.statement_index as usize >= call.statement_index
            || !names_whole_local(program, call, expression, view_symbol)
        {
            return None;
        }
        // A slice result is already its descriptor. Its produced type can
        // differ from an element-only local annotation; result admission and
        // receiving view replay authenticate that descriptor and its backing.
        // Lending it does not cross a stored Reference shell.
        if crate::execution::terminal_unit::types::slice_view_carrier(
            program,
            parameter.type_reference,
        ) {
            if exact_structural_argument_access(
                program, facts, machine, state, call, place, access,
            )? != access
            {
                return None;
            }
            return Some(CheckedUnitStructuralArgumentPlan::structural_result(
                result.binding_ordinal,
                target_identity.to_owned(),
                access,
            ));
        }
        // Keep the common loan's logical root coordinate. Replacing its source
        // with a physical result ordinal would lose the carrier join needed
        // when publication retains a reference descriptor.
        let argument = super::computation_arguments::borrowed_storage_argument(
            program,
            &facts.borrow,
            machine,
            source_state,
            call,
            place,
            parameter,
            None,
        )?;
        if !matches!(argument.source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { symbol, .. }
                if symbol == view_symbol)
            || argument.type_identity != target_identity
        {
            return None;
        }
        return Some(argument);
    }
    // A borrowed-view value forwards its established descriptor, not an owned
    // referent. The caller's operation sequence supplies this exact earlier
    // binding whether a call or a checked descriptor formation produced it;
    // requiring a flow-call row would erase non-call producers and their loans.
    // This forwards the descriptor's already checked access, including an
    // exclusive range operand. It does not borrow the anonymous descriptor's
    // home or recover authority from the consumer's requested referent type.
    if !projected
        && ((matches!(
            access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        ) && crate::execution::terminal_unit::types::slice_view_carrier(
            program,
            parameter.type_reference,
        )) || (access == CheckedStructuralAccess::SharedBorrow
            && crate::execution::terminal_unit::types::borrowed_named_view(
                program,
                parameter.type_reference,
            )))
        && result.type_identity == target_identity
        && let Some(source) = place.root.as_expression()
        && source == expression
    {
        if usize::try_from(result.statement_index).ok()? != call.statement_index {
            return None;
        }
        return Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: target_identity.to_owned(),
            access,
        });
    }
    // Owned linear projections still need their exact claim handoff. Lending
    // storage uses captured loan authority below and transfers no owner claim,
    // regardless of that owner's multiplicity or how it was established.
    if linear && projected && access == CheckedStructuralAccess::Owned {
        return None;
    }
    // A completed value's represented contents do not depend on whether a
    // call, constructor or copy produced it. Exact source contracts rejoin
    // below; this classification grants no predicates at the consuming call.
    if unrestricted
        && access == CheckedStructuralAccess::Owned
        && (projected
            || !(validation::is_closed_primitive_array_type(program, formal_type)
                || validation::has_contents(
                    program,
                    formal_type,
                    validation::ContentsRequirement::RepresentedOwned,
                )))
    {
        return None;
    }
    let path = if projected {
        if !projected_borrow && !(allow_projection || owned_reference_record) {
            return None;
        }
        projected_path?
    } else {
        Vec::new()
    };
    let (value_expression, referent) = match access {
        CheckedStructuralAccess::Owned => (expression, formal_type),
        CheckedStructuralAccess::SharedBorrow => {
            let (referent_expression, referee) = crate::semantic::calls::shared_argument_referent(
                program,
                expression,
                parameter.type_reference,
            )?;
            let referee = if array_view { storage_type? } else { referee };
            match place.root {
                facts::PlaceRoot::Symbol(_) => {
                    if exact_structural_argument_access(
                        program, facts, machine, state, call, place, access,
                    )? != access
                    {
                        return None;
                    }
                }
                facts::PlaceRoot::Expression(source) if source == referent_expression => {
                    validate_shared_temporary(program, &facts.flow, machine, state, call, source)?;
                }
                _ => return None,
            }
            (referent_expression, referee)
        }
        CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow => {
            // Whole locals and projected subtrees retain their owner. Both
            // must rejoin the authored borrow and its captured access.
            let expected_access = match access {
                CheckedStructuralAccess::MutableBorrow => {
                    language_semantics::ReferenceAccess::Mutable
                }
                CheckedStructuralAccess::WriteOnlyBorrow => {
                    language_semantics::ReferenceAccess::WriteOnly
                }
                _ => return None,
            };
            let typed_trees::types::TypeReferenceNode::Reference {
                access: reference_access,
                referee,
                ..
            } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return None;
            };
            let Some(borrow) = program.expression_table.borrow_expression(expression) else {
                return None;
            };
            if *reference_access != expected_access
                || borrow.access != expected_access
                || !program.expression_table.expression_is_valid(borrow.target)
            {
                return None;
            }
            let _ = place.root.as_symbol()?;
            if exact_structural_argument_access(
                program, facts, machine, state, call, place, access,
            )? != access
            {
                return None;
            }
            (
                borrow.target,
                if array_view { storage_type? } else { *referee },
            )
        }
    };
    // The result owns the referent; a bounded byte view may retain its
    // reference shell in the call presentation identity. Compare storage
    // against storage, while preserving that presentation on the argument.
    let storage_identity_matches = projected
        || array_view
        || if access == CheckedStructuralAccess::Owned {
            result.type_identity == target_identity
        } else {
            base_type_identity(program, referent, &[])? == result.type_identity
        };
    // An anonymous literal constructs its operand in place under the
    // structural-construction contents family: its interval-domain scalar members
    // are legal because the construction proves each member's range
    // obligation, so moving that result whole keeps the mint's requirement.
    let constructed_literal = matches!(
        place.root,
        facts::PlaceRoot::Expression(source)
            if source == value_expression
                && !matches!(
                    program.expression_table.expression(source),
                    ExpressionNode::Call(_)
                )
    );
    // A projection selects the referent's type, not the containing owner's
    // multiplicity: an affine owner can hold linear members. Its resolved
    // storage/formal correspondence was checked by the presentation above;
    // the local declaration separately rejoins the complete owner below.
    // Whole operands retain their result contract.
    let selected_multiplicity = if projected_borrow {
        program.type_multiplicity(storage_type?)
    } else if projected {
        program.type_multiplicity(validation::canonical_place_type_reference(
            program,
            state,
            call.statement_index,
            place,
        )?)
    } else {
        result.multiplicity
    };
    if !storage_identity_matches
        || (program.type_multiplicity(referent) != selected_multiplicity
            // A shared loan presents unrestricted regardless of the referent's
            // own cleanup profile: the viewed element is already rejoined by
            // the projected path's identity check, and the binding stores
            // only the unrestricted descriptor.
            && !(access == CheckedStructuralAccess::SharedBorrow
                && result.multiplicity == Multiplicity::Unrestricted)
        )
        // Only an owned transfer needs an owned-content classification here.
        // A loan keeps the established source home and its declared member
        // predicates; exact access was rejoined above and the source/result
        // declaration correspondence is checked below for whole and projected
        // places alike.
        || (access == CheckedStructuralAccess::Owned
            && !unrestricted
            && !linear
            && !owned_reference_record
            && !validation::has_contents(program, referent, validation::ContentsRequirement::LinearOwned)
            // A nominal-cleanup result is discharged by the consuming call's
            // own transfer edge, replayed below through its exact permission
            // event, so cleanup-owned contents admit the same whole move.
            && !validation::has_contents(program, referent, validation::ContentsRequirement::CleanupOwned)
            // Moving a record that carries shared view descriptors preserves
            // those views' independently retained referent loans.
            && !validation::has_contents(program, referent, validation::ContentsRequirement::OwnedOrSharedView)
            && !(constructed_literal
                && validation::has_contents(program, referent, validation::ContentsRequirement::StructuralConstruction)))
        || usize::try_from(result.statement_index).ok()? > call.statement_index
    {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if usize::try_from(result.statement_index).ok()? == call.statement_index
                || !symbol.is_valid()
                || (!projected && !names_whole_local(program, call, value_expression, symbol))
            {
                return None;
            }
            if access != CheckedStructuralAccess::Owned
                || projected
                || unrestricted
                || linear
                || owned_reference_record
            {
                let source_state = program.state_by_symbol(state)?;
                let local = program
                    .statement_table
                    .statements(source_state.statement_nodes)
                    .get(usize::try_from(result.statement_index).ok()?)?
                    .as_local_data()?;
                // Moves consume the checked ownership of the current value;
                // binding mutability only permits its later reestablishment.
                // Loans, whole or projected, use the exact captured access
                // checked above. Borrow formation owns writability and access
                // rules (including write-only authority); a binding's `mut`
                // marker is not a substitute for that evidence.
                //
                // A loan leaves the value in the local's own storage and
                // presents the declared type itself (both identities were
                // matched above), so a field's represented domain or range is
                // carried by that declaration rather than copied: the callee
                // is checked against the same field constraints. An owned
                // move needs finite owned storage, not claim-free storage:
                // descendant claims cross through the call's custody plan.
                // Exact source/formal correspondence retains qualifications.
                let owned_contents = validation::has_contents(
                    program,
                    local.type_reference,
                    validation::ContentsRequirement::LinearOwned,
                );
                // A loan retains the complete owner's claims and disposal debt.
                // Captured access authorizes this observation, and the exact
                // declaration below authenticates its home. Only an owned
                // argument reaches claim-transfer planning after this query.
                // A borrowed-view binding lends elements through its
                // descriptor, not through owned contents on its reference
                // shell: the projected borrow's evidence is the exact
                // access rejoined above, so only the binding's identity
                // must match — the owned-local fences do not apply.
                let view_binding = projected_borrow
                    && local.symbol == symbol
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(local.type_reference),
                        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                            if matches!(
                                program.type_reference_table.type_reference(*referee),
                                typed_trees::types::TypeReferenceNode::Slice { .. }
                            )
                    );
                if !view_binding
                    && (local.symbol != symbol
                        || (access == CheckedStructuralAccess::Owned
                            && !unrestricted
                            && !linear
                            && !owned_reference_record
                            && !owned_contents)
                        || program.type_multiplicity(local.type_reference) != result.multiplicity
                        || (access == CheckedStructuralAccess::Owned
                            && unrestricted
                            && !(validation::is_closed_primitive_array_type(
                                program,
                                local.type_reference,
                            ) || validation::has_contents(
                                program,
                                local.type_reference,
                                validation::ContentsRequirement::RepresentedOwned,
                            )))
                        || base_type_identity(program, local.type_reference, &[])?
                            != result.type_identity)
                {
                    return None;
                }
                if access == CheckedStructuralAccess::Owned
                    && (linear || unrestricted)
                    && validation::structural_result_qualifications(program, local.type_reference)
                        .ok()?
                        != validation::structural_result_qualifications(program, referent).ok()?
                {
                    return None;
                }
            }
        }
        // A construction or copied place is established as a
        // state-local value at the consuming statement itself; the binding
        // replays like an anonymous result but carries no producer call.
        facts::PlaceRoot::Expression(source)
            if source == value_expression
                && !projected
                && !matches!(
                    program.expression_table.expression(source),
                    ExpressionNode::Call(_)
                ) =>
        {
            if usize::try_from(result.statement_index).ok()? != call.statement_index {
                return None;
            }
            let root = facts.values.structural_values.root_for_expression(
                state,
                u32::try_from(call.statement_index).ok()?,
                source,
            )?;
            if root.machine != machine
                || !facts.values.structural_values.nodes.is_valid(root.root)
                || facts
                    .values
                    .structural_values
                    .nodes
                    .get(root.root)
                    .expression
                    != source
                || program.type_multiplicity(root.type_reference) != result.multiplicity
                || program.normalized_type_identity(root.type_reference)
                    != program.normalized_type_identity(referent)
            {
                return None;
            }
            // The already matched qualified source/formal type retains its
            // declared predicates. The result separately names the storage
            // carrier every structural producer shares, under the structural
            // identity rule: unqualified, except a bounded byte owner's.
            validation::structural_result_contract(program, root.type_reference).ok()?;
            if validation::structural_type_identity(program, root.type_reference, &[], &[])
                != result.type_identity
            {
                return None;
            }
        }
        // Ordinary and boundary affine producers own anonymous results.
        // Rejoin their exact captured
        // preorder coordinate; the shared sequencer executes it in postorder.
        facts::PlaceRoot::Expression(source) if source == value_expression || projected => {
            if projected
                && validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    call.statement_index,
                    expression,
                )
                .as_ref()
                    != Some(place)
            {
                return None;
            }
            if usize::try_from(result.statement_index).ok()? != call.statement_index {
                return None;
            }
            let flow = state_flow(facts, machine, state)?;
            let mut producers =
                facts
                    .flow
                    .control
                    .calls
                    .span(flow.calls)?
                    .iter()
                    .filter(|producer| {
                        producer.statement_index == call.statement_index
                            && producer.authored_expression == source
                    });
            let producer = producers.next()?;
            if producers.next().is_some() || producer.call_ordinal <= call.call_ordinal {
                return None;
            }
            let authored = program.expression_table.call_node(source)?;
            let completed = super::super::control::structural_operands::result(
                program,
                machine,
                source,
                &mut ShapeCollector::new(program),
            )?;
            if validation::call_target_symbol(program, authored) != producer.target_symbol
                || completed.type_identity != result.type_identity
                || (access == CheckedStructuralAccess::Owned
                    && completed.qualifications
                        != validation::structural_result_contract(program, referent)
                            .ok()?
                            .qualifications)
            {
                return None;
            }
        }
        _ => return None,
    }
    // Borrowed arguments loan the binding's storage into the callee: no
    // owned custody moves at the edge, so the owned-move permission replay
    // below does not apply to them.
    if access != CheckedStructuralAccess::Owned || unrestricted {
        return Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: path.clone(),
            type_identity: target_identity.to_owned(),
            access,
        });
    }
    // A projected owned reference-record operand moves a subtree, not its
    // still-live root, so permission production records no place-move event
    // for it: `projected_affine` publishes projected transfer events only for
    // plain affine contents, while a reference-bearing subtree's custody lives
    // in its leaf-loan roster. `projected_record_argument` above already
    // proved every captured leaf under this edge stays live until this
    // consuming statement, and the bare reference result's `result_loan`
    // replay pins the call as that leaf's last use; requiring the absent
    // whole-place event here would double-count custody the loans carry.
    if !(projected && owned_reference_record) {
        let mut events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.source
                        == PermissionEventSource::Call {
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            target_symbol: call.target_symbol,
                        }
                    && event.root == place.root
                    && event.access == PermissionAccess::Owned
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .starts_with(place.segments.as_slice())
            });
        // Permission production tracks a claim-free owner as one affine row,
        // and a claimed owner through its exact descendant claim frontier.
        // Root multiplicity cannot classify those claims: an affine record
        // can contain linear children. The enclosing custody replay checks
        // the complete roster against the callee's entry/outcome set; the
        // owned storage contract carries its residual affine complement.
        let mut affine_transfer = false;
        let mut claims = Vec::new();
        for event in events.by_ref() {
            let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
            if event.kind != PermissionEventKind::Transfer || segments.len() != event.segments.len()
            {
                return None;
            }
            if event.claim_identity == PermissionClaimIdentity::Unknown {
                if affine_transfer
                    || event.obligation_live
                    || event.multiplicity != selected_multiplicity
                    || event.multiplicity == Multiplicity::Linear
                    || segments != place.segments.as_slice()
                {
                    return None;
                }
                affine_transfer = true;
            } else {
                let relative = segments.strip_prefix(place.segments.as_slice())?;
                if event.multiplicity != Multiplicity::Linear
                    || !event.obligation_live
                    || claims.contains(&event.claim_identity)
                    || validation::structural_claim_path(program, formal_type, relative).is_err()
                {
                    return None;
                }
                claims.push(event.claim_identity);
            }
        }
        if !affine_transfer && claims.is_empty() {
            return None;
        }
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target_identity.to_owned(),
        access: CheckedStructuralAccess::Owned,
    })
}

/// Whether the argument spelling names exactly the whole local `symbol`: an
/// authored `Name` expression, or a method-spelled receiver that the call
/// fact names by symbol because no argument expression exists for it.
fn names_whole_local(
    program: &dyn ::validation::CheckingInput,
    call: &checked_trees::FlowCallFact,
    value_expression: typed_trees::expression::ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    if !value_expression.is_valid() {
        return call.has_receiver && call.receiver_symbol == symbol;
    }
    matches!(program.expression_table.expression(value_expression),
        ExpressionNode::Name(name) if name.symbol == symbol
            && name.head_symbol == symbol
            && program.expression_table.name_path_members(name.members).len() == 1)
}

/// The declared type of the local that binds `result` when the receiver
/// place is exactly that whole local.
fn whole_local_storage_type(
    program: &dyn ::validation::CheckingInput,
    source_state: &typed_trees::state::State,
    place: &validation::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let symbol = place.root.as_symbol()?;
    let local = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(usize::try_from(result.statement_index).ok()?)?
        .as_local_data()?;
    (place.segments.is_empty() && local.symbol == symbol).then_some(local.type_reference)
}
