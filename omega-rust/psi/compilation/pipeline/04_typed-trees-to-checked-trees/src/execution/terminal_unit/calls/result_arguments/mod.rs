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
    ShapeCollector, base_type_identity, shared_plain_affine_referent, state_flow,
    structural_access_for_type_reference,
};

use crate::execution::terminal_unit::StateParameter;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::SymbolHandle;
use crate::execution::terminal_unit::TypedTrees;

use crate::execution::terminal_unit::calls::argument_paths::{
    projected_argument_path_with_identity, projected_borrowed_path,
};
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;

mod anonymous_shared;
mod bounded_owner;
mod reference;

pub(in crate::execution::terminal_unit) use reference::receiver_argument as reference_result_receiver_argument;

#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    place: &crate::flow::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
    allow_projection: bool,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let (source_machine, source_state) =
        crate::semantic::calls::find_state_with_machine(program, state)?;
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
            facts,
            machine,
            source_state,
            call,
            expression,
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
    // An attached method's owned `self` formal carries `Named { machine,
    // "Self" }`, the machine's alias for its retained owner application. A
    // receiver-specialized callee's application is already concrete
    // (`Task<Token>`), so it supplies the formal's real multiplicity and
    // claim-path carrier below; a non-self formal keeps its authored type.
    // A by-value `self` formal is spelled `Self`; its referent is the
    // receiver value's own declared type, whose identity already matched the
    // attached data above. A local binding names that type directly.
    let formal_type = if parameter.is_self {
        crate::execution::terminal_unit::types::attached_self_application(
            program,
            parameter.type_reference,
        )
        .or_else(|| receiver_local_type(program, source_state, place, result))
        .unwrap_or(parameter.type_reference)
    } else {
        parameter.type_reference
    };
    let projected = !place.segments.is_empty();
    let unrestricted = result.multiplicity == Multiplicity::Unrestricted;
    let linear = result.multiplicity == Multiplicity::Linear;
    // A projected borrow — shared, mutable or write-only — lends a subtree of
    // a result binding that stays live after the call. The loan evidence
    // comes from the borrow record below, rejoined per access kind, rather
    // than from an owned-projection permission, so this lane is separate
    // from `allow_projection`: no custody moves.
    let projected_borrow = projected && access != CheckedStructuralAccess::Owned;
    let local_type = receiver_local_type(program, source_state, place, result);
    // A range lends the original array's window, not the element type that
    // generic indexing projects. Reuse the parameter operand's range walk,
    // including omitted endpoints resolved from the declared backing extent.
    let byte_window = projected_borrow
        .then(|| {
            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
            else {
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
    // The window is projected, but its result binding still owns the entire
    // local. Match that root's declaration for storage custody; the range
    // walk above separately proves the selected backing and presentation.
    let local_type = local_type.or_else(|| {
        byte_window.as_ref()?;
        receiver_local_type(
            program,
            source_state,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
            result,
        )
    });
    // A local array lends the same element view as an array parameter or
    // field. The local keeps its backing identity; the call presents only
    // the checked view, with its exact loan replayed below.
    let array_view = byte_window.is_some()
        || (access != CheckedStructuralAccess::Owned
            && local_type.is_some_and(|source| {
                super::boundary_admission::fixed_array_slice_view_is_admitted(
                    program,
                    source,
                    parameter.type_reference,
                )
            }));
    // A projected owned-record operand carries its subtree's captured leaves
    // into the call; the same resolved path both proves that custody and
    // names the argument's projected edge below. A projected borrow resolves
    // the same path to name the loaned subtree.
    let projected_path = if projected {
        if projected_borrow {
            // An element or field borrow resolves runtime selectors the way
            // the parameter lanes do: the segment carries the checked
            // `RuntimeIndex` coordinate Terminal re-proves against the
            // container's extent.
            byte_window.or_else(|| {
                projected_borrowed_path(program, machine, state, call.statement_index, place)
                    .filter(|(projected, _)| {
                        super::argument_presentation::is_admitted(
                            program,
                            *projected,
                            parameter.type_reference,
                            target_identity,
                            &[],
                        )
                    })
                    .map(|(_, path)| path)
            })
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
    if !projected
        && access == CheckedStructuralAccess::SharedBorrow
        && ((crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            parameter.type_reference,
        ) && result.type_identity == target_identity)
            || (crate::execution::terminal_unit::types::borrowed_named_view(
                program,
                parameter.type_reference,
            ) && result.type_identity
                == program
                    .normalized_type_identity(parameter.type_reference)
                    .as_str()))
        && let facts::PlaceRoot::Symbol(view_symbol) = place.root
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
            || exact_structural_argument_access(
                program, facts, machine, state, call, place, access,
            )? != access
        {
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
    // A `&[u8]`/`&'a V` borrowed-view result forwarded whole as the actual:
    // the producer already loaned its own storage, so the argument names the
    // anonymous binding at this statement — no `&expr` spelling and no owned
    // custody event. Same producer uniqueness and ordering the general
    // anonymous-result lane below enforces.
    if !projected
        && access == CheckedStructuralAccess::SharedBorrow
        && (crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            parameter.type_reference,
        ) || crate::execution::terminal_unit::types::borrowed_named_view(
            program,
            parameter.type_reference,
        ))
        && result.type_identity == target_identity
        && let facts::PlaceRoot::Expression(source) = place.root
        && source == expression
    {
        if usize::try_from(result.statement_index).ok()? != call.statement_index {
            return None;
        }
        let flow = state_flow(facts, machine, state)?;
        let mut producers = facts
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
        return Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: target_identity.to_owned(),
            access,
        });
    }
    // A whole linear result carries the producer's live claim, not affine
    // cleanup debt. Its exact qualification and transfer events must agree
    // with the consumer; projected and borrowed claim joins remain separate.
    if linear && (projected || access != CheckedStructuralAccess::Owned) {
        return None;
    }
    if unrestricted && !projected_borrow {
        // A completed nested byte call owns its qualified result. Scheduling
        // it does not establish predicates: retain the exact declared roster
        // required by this formal, then rejoin its source/ordinal below.
        let completed_byte_operand = access == CheckedStructuralAccess::Owned
            && !projected
            && place.root == facts::PlaceRoot::Expression(expression)
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
            && validation::bounded_byte_buffer_capacity(program, formal_type).is_some()
            && validation::has_plain_owned_contents_with_represented_constraints(
                program,
                formal_type,
            )
            && super::super::control::structural_operands::result(
                program,
                facts,
                machine,
                expression,
                &mut ShapeCollector::new(program),
            )
            .is_some_and(|completed| {
                completed.type_identity == result.type_identity
                    && validation::structural_result_qualifications(program, formal_type)
                        .is_ok_and(|required| completed.qualifications == required)
            });
        // A shared borrow views the binding's own storage: the formal's
        // referent — not its `&` shell — carries the closed/plain-content
        // obligation, and the borrow arm below proves the loan itself.
        let storage = if array_view {
            local_type
        } else {
            match access {
                CheckedStructuralAccess::Owned => Some(parameter.type_reference),
                CheckedStructuralAccess::SharedBorrow => {
                    shared_closed_referent(program, parameter.type_reference)
                }
                CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow => {
                    match program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    {
                        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                            Some(*referee)
                        }
                        _ => None,
                    }
                }
            }
        };
        if projected
            || !storage.is_some_and(|storage| {
                validation::is_closed_primitive_array_type(program, storage)
                    || completed_byte_operand
                    || validation::has_plain_owned_contents_with_numeric_constraints(
                        program, storage,
                    )
                    // A loan presents the existing local's declared carrier;
                    // byte predicates remain attached to that same storage.
                    || (access != CheckedStructuralAccess::Owned
                        && local_type.is_some()
                        && validation::has_plain_owned_contents_with_represented_constraints(
                            program, storage,
                        ))
                    // Owned snapshots retain the exact field declarations,
                    // including bounded live-byte predicates. The producer and
                    // source occurrence are rejoined below; this is not a loan
                    // or permission to establish arbitrary constrained bytes.
                    || (access == CheckedStructuralAccess::Owned
                        && place.root == facts::PlaceRoot::Expression(expression)
                        && facts.values.structural_values.root_for_expression(
                            state, result.statement_index, expression,
                        ).is_some_and(|root| {
                            root.machine == machine
                                && facts.values.structural_values.nodes.is_valid(root.root)
                                && matches!(facts.values.structural_values.nodes.get(root.root).kind,
                                    checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. })
                                && program.normalized_type_identity(root.type_reference)
                                    == program.normalized_type_identity(formal_type)
                                && base_type_identity(program, root.type_reference, &[])
                                    .is_some_and(|identity| identity == result.type_identity)
                        })
                        && validation::has_plain_owned_contents_with_represented_constraints(
                            program, storage,
                        ))
            })
        {
            return None;
        }
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
            let referee = shared_plain_affine_referent(program, parameter.type_reference)
                .or_else(|| shared_closed_referent(program, parameter.type_reference))
                .or(if array_view { local_type } else { None })
                .or_else(|| {
                    // A named local lends its own storage, exactly as the
                    // exclusive arm below does: the declared record's field
                    // domains ride the loan, and the local's contents and
                    // identity are rejoined against the binding below.
                    matches!(place.root, facts::PlaceRoot::Symbol(_))
                        .then(|| {
                            shared_numeric_affine_referent(program, parameter.type_reference)
                                .or_else(|| {
                                    shared_view_affine_referent(program, parameter.type_reference)
                                })
                        })
                        .flatten()
                })?;
            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if borrow.access != language_semantics::ReferenceAccess::Shared
                || !program.expression_table.expression_is_valid(borrow.target)
            {
                return None;
            }
            match place.root {
                facts::PlaceRoot::Symbol(_) => {
                    if exact_structural_argument_access(
                        program, facts, machine, state, call, place, access,
                    )? != access
                    {
                        return None;
                    }
                }
                facts::PlaceRoot::Expression(source) if source == borrow.target => {
                    anonymous_shared::validate(program, facts, machine, state, call, source)?;
                }
                _ => return None,
            }
            (borrow.target, referee)
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
            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if *reference_access != expected_access
                || borrow.access != expected_access
                || !program.expression_table.expression_is_valid(borrow.target)
            {
                return None;
            }
            let facts::PlaceRoot::Symbol(_) = place.root else {
                return None;
            };
            if exact_structural_argument_access(
                program, facts, machine, state, call, place, access,
            )? != access
            {
                return None;
            }
            (
                borrow.target,
                if array_view { local_type? } else { *referee },
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
    if !storage_identity_matches
        || (program.type_multiplicity(referent) != result.multiplicity
            // A shared loan presents unrestricted regardless of the referent's
            // own cleanup profile: the viewed element is already rejoined by
            // the projected path's identity check, and the binding stores
            // only the unrestricted descriptor.
            && !(access == CheckedStructuralAccess::SharedBorrow
                && result.multiplicity == Multiplicity::Unrestricted)
            // A projected loan of an unrestricted leaf (`&pair.high`) observes
            // or replaces that leaf in place; the affine root keeps its own
            // custody and cleanup, so the leaf's multiplicity is not the
            // root's. Whole loans and moves keep the exact equality above.
            && !(projected_borrow
                && program.type_multiplicity(referent) == Multiplicity::Unrestricted
                && result.multiplicity == Multiplicity::Affine))
        // Only an owned transfer needs an owned-content classification here.
        // A loan keeps the established source home and its declared member
        // predicates; exact access was rejoined above and the source/result
        // declaration correspondence is checked below for whole and projected
        // places alike.
        || (access == CheckedStructuralAccess::Owned
            && !unrestricted
            && !linear
            && !owned_reference_record
            && !validation::has_plain_owned_contents(program, referent)
            // A nominal-cleanup result is discharged by the consuming call's
            // own transfer edge, replayed below through its exact permission
            // event, so cleanup-owned contents admit the same whole move.
            && !validation::has_cleanup_owned_contents(program, referent)
            // Moving a record that carries shared view descriptors preserves
            // those views' independently retained referent loans.
            && !validation::has_owned_or_shared_view_contents(program, referent)
            && !(constructed_literal
                && validation::has_structural_construction_contents(program, referent)))
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
                let source_state = crate::semantic::calls::find_state(program, state)?;
                let StatementNode::LocalData(local) = program
                    .statement_table
                    .statements(source_state.statement_nodes)
                    .get(usize::try_from(result.statement_index).ok()?)?
                else {
                    return None;
                };
                // A whole owned move retains the immutable-binding fence.
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
                // move still needs the plain-contents fence, because moving
                // a constrained value owes its own value correspondence.
                let owned_contents = if access == CheckedStructuralAccess::Owned {
                    validation::has_plain_owned_contents(program, local.type_reference)
                } else {
                    // A loan presents the declared record: a member that is
                    // itself a shared view rides the same carrier the
                    // disposal, result-custody and scalar-read gates already
                    // admit — the view's own loan stays with the caller.
                    validation::has_plain_owned_contents_with_represented_constraints(
                        program,
                        local.type_reference,
                    ) || validation::has_owned_or_shared_view_contents(
                        program,
                        local.type_reference,
                    )
                };
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
                    && ((access == CheckedStructuralAccess::Owned && local.is_mutable)
                        || local.symbol != symbol
                        || (!unrestricted && !linear && !owned_reference_record && !owned_contents)
                        || program.type_multiplicity(local.type_reference) != result.multiplicity
                        || (unrestricted
                            && !(validation::is_closed_primitive_array_type(
                                program,
                                local.type_reference,
                            )
                                || validation::has_plain_owned_contents_with_numeric_constraints(
                                    program,
                                    local.type_reference,
                                )
                                || (access != CheckedStructuralAccess::Owned && owned_contents)))
                        || base_type_identity(program, local.type_reference, &[])?
                            != result.type_identity)
                {
                    return None;
                }
                if linear
                    && validation::structural_result_qualifications(program, local.type_reference)
                        .ok()?
                        != validation::structural_result_qualifications(program, referent).ok()?
                {
                    return None;
                }
            }
        }
        facts::PlaceRoot::Expression(source)
            if unrestricted
                && validation::is_closed_primitive_array_type(
                    program,
                    parameter.type_reference,
                )
                && source == value_expression
                // A copied array is an ordinary retained structural value,
                // not an element-by-element literal construction. Its exact
                // copied-place root is rejoined by the following arm.
                && facts.values.structural_values.root_for_expression(
                    state,
                    u32::try_from(call.statement_index).ok()?,
                    source,
                ).is_none()
                && !matches!(
                    program.expression_table.expression(source),
                    ExpressionNode::Call(_)
                ) =>
        {
            if usize::try_from(result.statement_index).ok()? != call.statement_index {
                return None;
            }
            let source_machine = crate::lookup::machine_by_symbol(program, machine)?;
            let source_state = crate::semantic::calls::find_state(program, state)?;
            let parameter_position =
                crate::semantic::calls::call_target_parameters(program, call.target_symbol)?
                    .iter()
                    .position(|candidate| candidate.symbol == parameter.symbol)?;
            let expected = checked_trees::CheckedArrayConstructionSource::CallArgument {
                call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                parameter_position: u32::try_from(parameter_position).ok()?,
            };
            if !crate::values::call_array_constructions(
                program,
                &facts.flow,
                source_machine,
                source_state,
                call.statement_index,
            )
            .iter()
            .any(|array| {
                array.source == expected
                    && array.expression == source
                    && array.type_reference == parameter.type_reference
            }) {
                return None;
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
            // Copy custody preserves the exact qualified source/formal type.
            // Its result names storage, whose byte-carrier identity omits the
            // qualification shell; comparing that shell to storage rejects
            // an otherwise ordinary qualified byte-field snapshot.
            let identity_matches = if matches!(
                facts.values.structural_values.nodes.get(root.root).kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            ) {
                program.normalized_type_identity(root.type_reference)
                    == program.normalized_type_identity(formal_type)
                    && base_type_identity(program, root.type_reference, &[])?
                        == result.type_identity
            } else {
                program
                    .normalized_type_identity(root.type_reference)
                    .as_str()
                    == result.type_identity
            };
            if root.machine != machine
                || !matches!(
                    facts.values.structural_values.nodes.get(root.root).kind,
                    checked_trees::CheckedStructuralValueKind::Case(_)
                        | checked_trees::CheckedStructuralValueKind::StructuralCase { .. }
                        | checked_trees::CheckedStructuralValueKind::Record { .. }
                        | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
                )
                || !identity_matches
            {
                return None;
            }
        }
        // Ordinary and boundary affine producers own anonymous results.
        // Rejoin their exact captured
        // preorder coordinate; the shared sequencer executes it in postorder.
        facts::PlaceRoot::Expression(source) if source == value_expression || projected => {
            if projected
                && crate::flow::canonical_place_from_expression_in_state(
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
            let ExpressionNode::Call(authored) = program.expression_table.expression(source) else {
                return None;
            };
            if authored.target_symbol != producer.target_symbol
                || super::super::control::structural_operands::result(
                    program,
                    facts,
                    machine,
                    source,
                    &mut ShapeCollector::new(program),
                )?
                .type_identity
                    != result.type_identity
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
                    && (linear
                        || facts.flow.ownership.segments.span_or_empty(event.segments)
                            == place.segments.as_slice())
            });
        let event = events.next()?;
        // `permission_kind_for_move` settles an owned `self` move as Consume
        // when the callee's return carries no linear obligation — the
        // receiver's terminal settlement, not an ordinary value handoff.
        // Non-self owned parameters always transfer custody. Linear handoffs
        // retain a known live claim; affine handoffs have neither a claim
        // identity nor a live obligation.
        let expected_move_kind = if parameter.is_self
            && crate::semantic::calls::find_state(program, call.target_symbol).is_some_and(
                |target| {
                    !crate::checks::type_carries_linear_obligation(program, target.return_type)
                },
            ) {
            PermissionEventKind::Consume
        } else {
            PermissionEventKind::Transfer
        };
        if linear {
            // Moving one whole aggregate transfers every live claim below it.
            // This operand check establishes typed source events; the enclosing
            // call's custody replay joins their complete paths and identities to
            // the callee entry/outcome set before retaining the operation.
            let mut claims = Vec::new();
            for event in std::iter::once(event).chain(events) {
                let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
                if event.kind != expected_move_kind
                    || event.multiplicity != Multiplicity::Linear
                    || event.claim_identity == PermissionClaimIdentity::Unknown
                    || !event.obligation_live
                    || segments.len() != event.segments.len()
                    || claims.contains(&event.claim_identity)
                    || validation::structural_claim_path(program, formal_type, segments).is_err()
                {
                    return None;
                }
                claims.push(event.claim_identity);
            }
        } else if events.next().is_some()
            || event.kind != expected_move_kind
            || event.multiplicity != result.multiplicity
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.obligation_live
        {
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

/// Shared references to a whole unrestricted closed carrier — a fixed
/// primitive array or a record of represented plain-owned fields — view storage
/// the binding itself retains. Nested references and unsupported constraints
/// stay out; the loan does not copy or establish the retained field predicates.
fn shared_closed_referent(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let typed_trees::types::TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) == Multiplicity::Unrestricted
        && (validation::is_closed_primitive_array_type(program, *referee)
            || validation::has_plain_owned_contents_with_represented_constraints(
                program, *referee,
            )))
    .then_some(*referee)
}

/// Shared references to an affine record whose leaves are plain owned values,
/// numeric domains and ranges included. Only a named local's own storage uses
/// this: the loan presents the declared type unchanged, so its field
/// constraints need no separate transport. Nested references, cleanup-owned
/// contents and linear referents stay out.
fn shared_numeric_affine_referent(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let typed_trees::types::TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) == Multiplicity::Affine
        && validation::has_plain_owned_contents_with_numeric_constraints(program, *referee))
    .then_some(*referee)
}

/// Shared references to an affine record whose members mix plain owned
/// scalars and shared views (`&[T]` / `&'a V`) ride the same lane as the
/// plain-affine companion: the loan presents the declared record unchanged,
/// and a shared borrow moves no view custody — each view field's captured
/// loan stays with the caller for the call's duration. Mutable access,
/// linear referents and mutating loans stay out.
fn shared_view_affine_referent(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let typed_trees::types::TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (program.type_multiplicity(*referee) == Multiplicity::Affine
        && validation::has_owned_or_shared_view_contents(program, *referee))
    .then_some(*referee)
}

/// Whether the argument spelling names exactly the whole local `symbol`: an
/// authored `Name` expression, or a method-spelled receiver that the call
/// fact names by symbol because no argument expression exists for it.
fn names_whole_local(
    program: &TypedTrees,
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
fn receiver_local_type(
    program: &TypedTrees,
    source_state: &typed_trees::state::State,
    place: &crate::flow::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(usize::try_from(result.statement_index).ok()?)?
    else {
        return None;
    };
    (place.segments.is_empty() && local.symbol == symbol).then_some(local.type_reference)
}
