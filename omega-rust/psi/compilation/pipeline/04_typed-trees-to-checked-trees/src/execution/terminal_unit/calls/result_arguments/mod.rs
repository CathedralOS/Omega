//! Result operands retain exact source access, ownership, and projected storage.
//! Projection admission belongs to that source/formal correspondence and its
//! custody, not whether the consuming call is a statement or a value operand.
//! A completed producer has already established its declared contents. This
//! planner preserves type, qualification, access and storage correspondence;
//! it does not admit the value again through a contents whitelist. Source flow
//! checks copyability and loans, and call_claim_transfers carries owned claims.
use crate::execution::terminal_unit::CheckFacts;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralResultBindingPlan;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::Multiplicity;
use crate::execution::terminal_unit::types::{
    base_type_identity, structural_access_for_type_reference,
};

use crate::execution::terminal_unit::StateParameter;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::SymbolHandle;

use crate::execution::terminal_unit::calls::argument_paths::{
    projected_argument_path_for_identity, projected_borrowed_path,
};
use crate::execution::terminal_unit::calls::argument_presentation::ArgumentPresentation;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;

mod anonymous_shared;
mod bounded_owner;
mod reference;

pub(crate) use anonymous_shared::validate as validate_shared_temporary;
pub(crate) use reference::{
    argument as retained_reference_argument, is_carrier as is_stored_reference,
};

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
    target_identity: typed_trees::type_identity::TypeIdentityId,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    trace: &super::super::control::LocalConstructionTrace,
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
        operations,
        trace,
    )?;
    // Bind producer operands to the current home without replacing an explicit
    // loan descriptor: its source already retains captured geometry and access.
    if matches!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
    ) && let Some(symbol) = place.root.as_symbol()
    {
        trace.phase("call operation: result argument: current storage home");
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
    target_identity: typed_trees::type_identity::TypeIdentityId,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    trace: &super::super::control::LocalConstructionTrace,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let (source_machine, source_state) = validation::find_state(program, state)?;
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
            facts::PlaceRoot::Symbol(local.symbol),
            local.type_reference,
            result,
            parameter,
            target_identity,
        );
    }
    // Anonymous producers publish the same stored descriptor as named locals.
    // Rejoin the unique completed result and its actual source type, whether
    // a call or a value establishment produced it. The descriptor crossing
    // and access judgment must not depend on that producer's syntax. The
    // result's own identity decides whether it holds that descriptor: a
    // referent moved out of a `&mut` place (`transform(choice)` for
    // `choice: &mut Choice`) publishes the owned referent, which continues to
    // the owned-result lane below like any other completed value.
    if let Some(owner) = place.root.as_expression() {
        let reference = facts
            .values
            .structural_values
            .root_for_expression(state, result.statement_index, owner)
            .map(|root| root.type_reference)
            .or_else(|| {
                validation::declared_place_type_raw(
                    program,
                    source_machine,
                    Some(source_state),
                    owner,
                )
            })?;
        if reference::is_carrier(program, reference)
            && result.type_identity == program.normalized_type_identity(reference)
        {
            if result.statement_index as usize != call.statement_index
                || operations
                    .iter()
                    .filter_map(|operation| operation.structural_result_binding())
                    .filter(|binding| *binding == result)
                    .count()
                    != 1
            {
                return None;
            }
            return reference::argument(
                program,
                machine,
                source_state,
                call.statement_index,
                place,
                facts::PlaceRoot::Expression(owner),
                reference,
                result,
                parameter,
                target_identity,
            );
        }
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
    // than from an owned-projection permission: no custody moves.
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
        byte_window
            .or_else(|| {
                projected_borrowed_path(program, machine, state, call.statement_index, place)
            })
            .and_then(|(selected, path)| {
                super::argument_paths::borrowed_argument_endpoint(
                    program,
                    selected,
                    path,
                    parameter.type_reference,
                )
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
                projected_argument_path_for_identity(
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
            checked_trees::checked_borrowed_access_pair_pattern!()
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
                    == program.normalized_type_identity(parameter.type_reference)))
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
            checked_trees::checked_borrowed_access_pair_pattern!()
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
    let path = if projected {
        projected_path?
    } else {
        Vec::new()
    };
    let (value_expression, referent) = match access {
        CheckedStructuralAccess::Owned => (expression, formal_type),
        CheckedStructuralAccess::SharedBorrow => {
            trace.phase("call operation: result argument: shared referent");
            let (referent_expression, referee) = crate::semantic::calls::shared_argument_referent(
                program,
                expression,
                parameter.type_reference,
            )?;
            let referee = if array_view { storage_type? } else { referee };
            match place.root {
                facts::PlaceRoot::Symbol(_) => {
                    trace.phase("call operation: result argument: retained local loan");
                    if exact_structural_argument_access(
                        program, facts, machine, state, call, place, access,
                    )? != access
                    {
                        return None;
                    }
                }
                facts::PlaceRoot::Expression(source) => {
                    // A loan selects a place within the complete producer.
                    // Rejoin that root/path, not expression equality: the
                    // selected field is not a separate temporary owner.
                    if validation::canonical_place_from_expression_in_state(
                        program,
                        state,
                        call.statement_index,
                        referent_expression,
                    )
                    .as_ref()
                        != Some(place)
                    {
                        return None;
                    }
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
    trace.phase("call operation: result argument: referent storage identity");
    let storage_identity_matches = projected
        || array_view
        || if access == CheckedStructuralAccess::Owned {
            result.type_identity == target_identity
        } else {
            base_type_identity(program, referent, &[], &[])? == result.type_identity
        };
    // Only owned transfer carries the selected storage's disposal obligation.
    // A loan retains its owner's custody; its source/formal presentation and
    // exact access were checked above. In particular a stored slice descriptor
    // is unrestricted while its unsized referent is not an owned result.
    // Comparing those cleanup profiles would make borrowing depend on whether
    // an unrelated sibling makes the containing record affine.
    let transferred_multiplicity = if projected && access == CheckedStructuralAccess::Owned {
        validation::reference_result_custody::result_multiplicity(
            program,
            validation::canonical_place_type_reference(
                program,
                state,
                call.statement_index,
                place,
            )?,
        )
    } else {
        result.multiplicity
    };
    if !storage_identity_matches && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!(
            "RESULT_ARGUMENT_IDENTITY machine={} state={} statement={} root={:?} access={:?} referent={} storage={:?} established={:?} formal={:?}",
            program.symbols.display_path(machine, "::"),
            program.symbols.display_path(state, "::"),
            call.statement_index,
            place,
            access,
            program.normalized_type_identity_text(referent),
            base_type_identity(program, referent, &[], &[])
                .and_then(|id| program.type_identity_text(id)),
            program.type_identity_text(result.type_identity),
            program.type_identity_text(target_identity)
        );
    }
    if !storage_identity_matches
        || (access == CheckedStructuralAccess::Owned
            && validation::reference_result_custody::result_multiplicity(program, referent)
                != transferred_multiplicity)
        || usize::try_from(result.statement_index).ok()? > call.statement_index
    {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            trace.phase("call operation: result argument: earlier whole local correspondence");
            if usize::try_from(result.statement_index).ok()? == call.statement_index
                || !symbol.is_valid()
                || (!projected && !names_whole_local(program, call, value_expression, symbol))
            {
                return None;
            }
            trace.phase("call operation: result argument: local binding custody");
            let source_state = program.state_by_symbol(state)?;
            let local = program
                .statement_table
                .statements(source_state.statement_nodes)
                .get(usize::try_from(result.statement_index).ok()?)?
                .as_local_data()?;
            // Authenticate the containing local independently of the selected
            // subtree. Loans use the captured access checked above and leave
            // its claims intact; owned arguments use call claim transfers.
            // A slice-view local lends its descriptor's element storage, not
            // the reference shell, so the projected path owns that geometry.
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
                    || validation::reference_result_custody::result_multiplicity(
                        program,
                        local.type_reference,
                    ) != result.multiplicity
                    || base_type_identity(program, local.type_reference, &[], &[])?
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
        // Anonymous values are admitted by their already sequenced producer,
        // not by whether their source spells a constructor, copy or call.
        // Extraction keeps the authored storage coordinate for the transfer
        // evidence below while the call reads its newly owned result slot.
        facts::PlaceRoot::Expression(source) if source == value_expression || projected => {
            if usize::try_from(result.statement_index).ok()? != call.statement_index
                || operations
                    .iter()
                    .filter_map(|operation| operation.structural_result_binding())
                    .filter(|binding| *binding == result)
                    .count()
                    != 1
                || (projected
                    && validation::canonical_place_from_expression_in_state(
                        program,
                        state,
                        call.statement_index,
                        expression,
                    )
                    .as_ref()
                        != Some(place))
            {
                return None;
            }
            let source_type = facts
                .values
                .structural_values
                .root_for_expression(state, result.statement_index, source)
                .map(|root| root.type_reference)
                .or_else(|| {
                    validation::declared_place_type_raw(
                        program,
                        source_machine,
                        Some(source_state),
                        source,
                    )
                })?;
            let source_type = if access == CheckedStructuralAccess::Owned {
                validation::assignment_value_type(program, source_type)
            } else {
                source_type
            };
            let contract = validation::structural_result_contract(program, source_type).ok()?;
            if validation::reference_result_custody::result_multiplicity(program, source_type)
                != result.multiplicity
                || validation::structural_type_identity(program, source_type, &[], &[])
                    != result.type_identity
                || (!projected
                    && access == CheckedStructuralAccess::Owned
                    && (program.normalized_type_identity(source_type)
                        != program.normalized_type_identity(referent)
                        || contract.qualifications
                            != validation::structural_result_contract(program, referent)
                                .ok()?
                                .qualifications))
            {
                return None;
            }
        }
        _ => return None,
    }
    // Checking already established this actual's access and consuming edge.
    // The call's shared claim-transfer planner carries its captured frontier;
    // result presentation must not demand a second permission replay here.
    // A borrowed referent has no owned-ingress permission rows to replay:
    // its extraction operation relocates custody into this same result slot.
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target_identity.to_owned(),
        access,
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
