//! Whole affine result operands share one source move judgment across callees.

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
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if parameter.is_self
        || result.multiplicity != Multiplicity::Affine
        || result.type_identity != target_identity
        || !place.segments.is_empty()
        || structural_access_for_type_reference(program, parameter.type_reference)?
            != CheckedStructuralAccess::Owned
        || program.type_multiplicity(parameter.type_reference) != Multiplicity::Affine
        || !validation::has_plain_owned_contents(program, parameter.type_reference)
        || usize::try_from(result.statement_index).ok()? > call.statement_index
    {
        return None;
    }
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if usize::try_from(result.statement_index).ok()? == call.statement_index
                || !symbol.is_valid()
                || !matches!(program.expression_table.expression(expression),
                    ExpressionNode::Name(name) if name.symbol == symbol
                        && name.head_symbol == symbol
                        && program.expression_table.name_path_members(name.members).len() == 1)
            {
                return None;
            }
        }
        // Only the existing ordinary affine producer owns an anonymous result,
        // even when its consumer is a boundary. Rejoin its exact captured
        // preorder coordinate; the shared sequencer executes it in postorder.
        facts::PlaceRoot::Expression(source) if source == expression => {
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
                || !facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines
                    .iter()
                    .any(|target| {
                        target.state == producer.target_symbol
                            && target.result.type_identity == result.type_identity
                            && target.result.multiplicity == Multiplicity::Affine
                            && target.result.qualifications.is_empty()
                    })
            {
                return None;
            }
        }
        _ => return None,
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
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .is_empty()
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path: Vec::new(),
        type_identity: target_identity.to_owned(),
        access: CheckedStructuralAccess::Owned,
    })
}
