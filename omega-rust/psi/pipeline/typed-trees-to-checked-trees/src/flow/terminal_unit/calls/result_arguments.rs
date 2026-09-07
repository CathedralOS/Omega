//! Affine result operands retain exact source access and projected storage.

use super::*;

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
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    let projected = !place.segments.is_empty();
    let path = if projected {
        if !allow_projection || access != CheckedStructuralAccess::Owned {
            return None;
        }
        projected_argument_path_with_identity(
            program,
            state,
            call.statement_index,
            place,
            target_identity,
        )?
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
                || !matches!(place.root, facts::PlaceRoot::Symbol(_))
                || exact_structural_argument_access(
                    program, facts, machine, state, call, place, access,
                )? != access
            {
                return None;
            }
            (borrow.target, referee)
        }
        CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow => {
            return None;
        }
    };
    if parameter.is_self
        || result.multiplicity != Multiplicity::Affine
        || (!projected && result.type_identity != target_identity)
        || program.type_multiplicity(referent) != Multiplicity::Affine
        || !validation::has_plain_owned_contents(program, referent)
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
            if access == CheckedStructuralAccess::SharedBorrow || projected {
                let source_state = crate::find_state(program, state)?;
                let StatementNode::LocalData(local) = program
                    .statement_table
                    .statements(source_state.statement_nodes)
                    .get(usize::try_from(result.statement_index).ok()?)?
                else {
                    return None;
                };
                if local.is_mutable
                    || local.symbol != symbol
                    || !validation::has_plain_owned_contents(program, local.type_reference)
                    || program.type_multiplicity(local.type_reference) != Multiplicity::Affine
                    || base_type_identity(program, local.type_reference, &[])?
                        != result.type_identity
                {
                    return None;
                }
            }
        }
        // Ordinary and boundary affine producers own anonymous results.
        // Rejoin their exact captured
        // preorder coordinate; the shared sequencer executes it in postorder.
        facts::PlaceRoot::Expression(source) if source == expression || projected => {
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
    if access == CheckedStructuralAccess::SharedBorrow {
        return Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: target_identity.to_owned(),
            access,
        });
    }
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
                && facts.flow.ownership.segments.span_or_empty(event.segments)
                    == place.segments.as_slice()
        });
    let event = events.next()?;
    // Non-self owned parameters transfer custody even at direct or nominal
    // boundaries. Consume events describe terminal self/claim settlement, not
    // this claim-free affine argument convention.
    if events.next().is_some()
        || event.kind != PermissionEventKind::Transfer
        || event.multiplicity != Multiplicity::Affine
        || event.claim_identity != PermissionClaimIdentity::Unknown
        || event.obligation_live
    {
        return None;
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
