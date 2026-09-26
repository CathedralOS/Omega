//! Result operands retain exact source access, ownership, and projected storage.
use crate::execution::terminal_unit::CheckFacts;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralPathSegment;
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

use crate::execution::terminal_unit::calls::argument_paths::projected_argument_path_with_identity;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;

mod anonymous_shared;

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
    let source_state = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == state)?;
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
        && super::super::reference_results::parts(program, local.type_reference).is_some()
        // Anonymous operand results carry their consuming statement's index,
        // which may be a `&mut` local binding that does not own this operand
        // place. Only an operand rooted at that local belongs to this lane;
        // other roots continue to the general argument checks below.
        && place.root == facts::PlaceRoot::Symbol(local.symbol)
    {
        let (_, access) =
            super::super::reference_results::parts(program, parameter.type_reference)?;
        if !place.segments.is_empty()
            || result.type_identity
                != program
                    .normalized_type_identity(local.type_reference)
                    .as_str()
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(local.type_reference)
        {
            return None;
        }
        let flow = state_flow(facts, machine, state)?;
        let producer = facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .find(|producer| producer.authored_expression == local.initial_value)?;
        let loan = super::super::reference_results::result_loan(
            program,
            facts,
            machine,
            source_state,
            producer,
            result,
        )?;
        let end = super::super::reference_results::release_statement(facts, machine, state, loan)?;
        if call.statement_index <= result.statement_index as usize
            || call.statement_index >= end as usize
        {
            return None;
        }
        return Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: vec![CheckedUnitStructuralPathSegment::Referent],
            type_identity: target_identity.to_owned(),
            access,
        });
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
    // A projected exclusive borrow lends a subtree of a result binding that
    // stays live after the call. The loan evidence comes from the borrow
    // record below rather than from an owned-projection permission, so this
    // lane is separate from `allow_projection`: no custody moves.
    let projected_borrow = projected
        && matches!(
            access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        );
    // A projected owned-record operand carries its subtree's captured leaves
    // into the call; the same resolved path both proves that custody and
    // names the argument's projected edge below. A projected exclusive
    // borrow resolves the same path to name the loaned subtree.
    let projected_path =
        if projected && (access == CheckedStructuralAccess::Owned || projected_borrow) {
            projected_argument_path_with_identity(
                program,
                state,
                call.statement_index,
                place,
                target_identity,
            )
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
    // A whole borrowed `&[T]` view result forwards the descriptor the caller
    // already holds: the argument is the view's own name, not a new `&place`
    // borrow, and the viewed storage keeps its owner and extent. The lend is
    // admitted only while the view's checked loan is still live at this call,
    // so a forwarded view cannot outlast the storage it borrows. Every other
    // shared argument keeps the `&place` lane below, which checks its own
    // borrow expression and referent.
    // A `&[T]`/`&[u8]`/`&'a V` view forwarded whole names a live `Read`
    // loan on the view symbol — `let`-bound `&[u8]` results register
    // exactly that loan, so the byte carrier joins the same lane (its
    // element is `u8`, which the older `&[T]` predicate excluded). A
    // `let`-bound `&'a V` mints its `ref(...)`-shelled identity — the
    // shared-view result's own spelling — and only that shelled mint
    // enters: an owned local read through `&token` keeps the peeled
    // carrier identity of its binding and stays on the `&place` lanes.
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
        let borrow_state = facts
            .borrow
            .states
            .iter()
            .map(|(_, borrow_state)| borrow_state)
            .find(|borrow_state| {
                borrow_state.machine_symbol == machine && borrow_state.state_symbol == state
            })?;
        let mut loans = facts
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| loan.owner_symbol == view_symbol);
        let loan = loans.next()?;
        if loans.next().is_some()
            || loan.kind != checked_trees::BorrowAccessKind::Read
            || call.statement_index > loan.last_use_statement_index
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
    if unrestricted
        && !projected_borrow
        && (projected
            || access != CheckedStructuralAccess::Owned
            || !(validation::is_closed_primitive_array_type(program, parameter.type_reference)
                || validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    parameter.type_reference,
                )))
    {
        return None;
    }
    let path = if projected {
        if access != CheckedStructuralAccess::Owned && !projected_borrow {
            return None;
        }
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
            let referee = shared_plain_affine_referent(program, parameter.type_reference)?;
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
            // Only a projected subtree of a live result binding can be lent
            // here: whole-place borrows still travel the parameter and alias
            // routes that own their own custody checks.
            if !projected_borrow {
                return None;
            }
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
            (borrow.target, *referee)
        }
    };
    if (!projected && result.type_identity != target_identity)
        || program.type_multiplicity(referent) != result.multiplicity
        || (!unrestricted
            && !linear
            && !owned_reference_record
            && !validation::has_plain_owned_contents(program, referent)
            // A nominal-cleanup result is discharged by the consuming call's
            // own transfer edge, replayed below through its exact permission
            // event, so cleanup-owned contents admit the same whole move.
            && !validation::has_cleanup_owned_contents(program, referent))
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
            if access == CheckedStructuralAccess::SharedBorrow
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
                // An exclusive subloan needs mutable owned storage behind
                // the binding; every other lane reads or moves immutable
                // owned storage.
                if local.is_mutable != projected_borrow
                    || local.symbol != symbol
                    || (!unrestricted
                        && !linear
                        && !owned_reference_record
                        && !validation::has_plain_owned_contents(program, local.type_reference))
                    || program.type_multiplicity(local.type_reference) != result.multiplicity
                    || (unrestricted
                        && !(validation::is_closed_primitive_array_type(
                            program,
                            local.type_reference,
                        ) || validation::has_plain_owned_contents_with_numeric_constraints(
                            program,
                            local.type_reference,
                        )))
                    || base_type_identity(program, local.type_reference, &[])?
                        != result.type_identity
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
        // An inline case or record construction is established as a
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
                || !matches!(
                    facts.values.structural_values.nodes.get(root.root).kind,
                    checked_trees::CheckedStructuralValueKind::Case(_)
                        | checked_trees::CheckedStructuralValueKind::StructuralCase { .. }
                        | checked_trees::CheckedStructuralValueKind::Record { .. }
                )
                || program
                    .normalized_type_identity(root.type_reference)
                    .as_str()
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
    if access == CheckedStructuralAccess::SharedBorrow || unrestricted {
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
