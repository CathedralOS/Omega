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
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::StateParameter;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::SymbolHandle;
use crate::execution::terminal_unit::TypedTrees;
use crate::execution::terminal_unit::base_type_identity;
use crate::execution::terminal_unit::calls::argument_paths::projected_argument_path_with_identity;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;
use crate::execution::terminal_unit::shared_plain_affine_referent;
use crate::execution::terminal_unit::state_flow;
use crate::execution::terminal_unit::structural_access_for_type_reference;

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
    let projected = !place.segments.is_empty();
    let unrestricted = result.multiplicity == Multiplicity::Unrestricted;
    let linear = result.multiplicity == Multiplicity::Linear;
    // A projected owned-record operand carries its subtree's captured leaves
    // into the call; the same resolved path both proves that custody and
    // names the argument's projected edge below.
    let projected_path = if projected && access == CheckedStructuralAccess::Owned {
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
    // A whole linear result carries the producer's live claim, not affine
    // cleanup debt. Its exact qualification and transfer events must agree
    // with the consumer; projected and borrowed claim joins remain separate.
    if linear && (projected || access != CheckedStructuralAccess::Owned) {
        return None;
    }
    if unrestricted
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
        if access != CheckedStructuralAccess::Owned || !(allow_projection || owned_reference_record)
        {
            return None;
        }
        projected_path?
    } else {
        Vec::new()
    };
    let (value_expression, referent) = match access {
        CheckedStructuralAccess::Owned => (expression, parameter.type_reference),
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
            return None;
        }
    };
    if parameter.is_self
        || (!projected && result.type_identity != target_identity)
        || program.type_multiplicity(referent) != result.multiplicity
        || (!unrestricted
            && !linear
            && !owned_reference_record
            && !validation::has_plain_owned_contents(program, referent))
        || usize::try_from(result.statement_index).ok()? > call.statement_index
    {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if usize::try_from(result.statement_index).ok()? == call.statement_index
                || !symbol.is_valid()
                || (!projected
                    && !matches!(program.expression_table.expression(value_expression),
                    ExpressionNode::Name(name) if name.symbol == symbol
                        && name.head_symbol == symbol
                        && program.expression_table.name_path_members(name.members).len() == 1))
            {
                return None;
            }
            if access == CheckedStructuralAccess::SharedBorrow
                || projected
                || unrestricted
                || linear
                || owned_reference_record
            {
                let source_state = crate::semantic_calls::find_state(program, state)?;
                let StatementNode::LocalData(local) = program
                    .statement_table
                    .statements(source_state.statement_nodes)
                    .get(usize::try_from(result.statement_index).ok()?)?
                else {
                    return None;
                };
                if local.is_mutable
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
            let source_machine = program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == machine)?;
            let source_state = crate::semantic_calls::find_state(program, state)?;
            let parameter_position =
                crate::semantic_calls::call_target_parameters(program, call.target_symbol)?
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
            path: Vec::new(),
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
        // Non-self owned parameters transfer custody even at direct or
        // nominal boundaries. Consume events describe terminal self/claim
        // settlement, not an ordinary value handoff. Linear handoffs retain a
        // known live claim; affine handoffs have neither a claim identity nor
        // a live obligation.
        if linear {
            // Moving one whole aggregate transfers every live claim below it.
            // This operand check establishes typed source events; the enclosing
            // call's custody replay joins their complete paths and identities to
            // the callee entry/outcome set before retaining the operation.
            let mut claims = Vec::new();
            for event in std::iter::once(event).chain(events) {
                let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
                if event.kind != PermissionEventKind::Transfer
                    || event.multiplicity != Multiplicity::Linear
                    || event.claim_identity == PermissionClaimIdentity::Unknown
                    || !event.obligation_live
                    || segments.len() != event.segments.len()
                    || claims.contains(&event.claim_identity)
                    || validation::structural_claim_path(
                        program,
                        parameter.type_reference,
                        segments,
                    )
                    .is_err()
                {
                    return None;
                }
                claims.push(event.claim_identity);
            }
        } else if events.next().is_some()
            || event.kind != PermissionEventKind::Transfer
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
