//! A consuming call or construction receives a completed anonymous owner.
//! Its producer established the actual claim frontier before this operation.
//! Calls transfer it whole; transparent constructors retain each child's
//! identity and provenance at its typed receiving path. Neither operation
//! settles a child by inventing a replacement claim at its destination.
use super::owners::{TemporaryConsumption, TemporaryDestination, TemporaryOwner};
use super::{
    CheckFacts, FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionProvenance,
};
use language_semantics::PermissionEventSource;

#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn append_owned_transfer(
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    consumer: PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    completed: typed_trees::expression::ExpressionHandle,
    owners: &[TemporaryOwner],
    written_targets: &mut [(
        super::super::linear_obligations::WrittenLinearTarget,
        PermissionEventSource,
    )],
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    conditional_carrier_transfers: &mut Vec<usize>,
) -> Result<(), diagnostics::Diagnostic> {
    for owner in owners.iter().filter(|owner| {
        matches!(owner.consumption, TemporaryConsumption::OwnedTransfer)
            && owner.consumer == consumer
            && owner.evaluation == evaluation
            && match &owner.destination {
                TemporaryDestination::CallArgument => {
                    matches!(consumer, PermissionEventSource::Call { .. })
                }
                TemporaryDestination::ConstructedChild { expression, .. } => {
                    *expression == completed
                }
            }
    }) {
        let root = facts::PlaceRoot::Expression(owner.expression);
        let incoming = places
            .iter()
            .filter(|place| place.root == root)
            .collect::<Vec<_>>();
        let completed_value = incoming
            .iter()
            .any(|place| place.ever_established && (place.live || place.case_excluded));
        if matches!(
            owner.destination,
            TemporaryDestination::ConstructedChild { .. }
        ) && (incoming.is_empty()
            || incoming.iter().any(|place| !place.ever_established)
            || !completed_value
            || incoming.iter().any(|place| {
                place.live
                    && ((place.multiplicity == Multiplicity::Linear
                        && place.claim_identity.is_none())
                        || place.provenance.is_none())
            }))
        {
            return Err(diagnostics::Diagnostic::error(
                "constructed child has no unique completed incoming custody",
            ));
        }
        for index in 0..places.len() {
            if places[index].root != root || !places[index].live {
                continue;
            }
            if let TemporaryDestination::ConstructedChild {
                expression, path, ..
            } = &owner.destination
            {
                let mut destination = path.clone();
                destination.extend_from_slice(&places[index].path);
                // Transparent receiving frontiers retain the child's exact
                // current name. A nominal constructor's own root obligation
                // remains distinct; it has no receiving claim at this path.
                let nominal = written_targets.iter().any(|(target, _)| {
                    target.value == *expression
                        && places[target.place_index].multiplicity == Multiplicity::Linear
                        && places[target.place_index].path == target.destination_path
                });
                let mut receivers = 0;
                for (target, _) in written_targets
                    .iter_mut()
                    .filter(|(target, _)| target.value == *expression)
                {
                    if places[target.place_index]
                        .path
                        .strip_prefix(target.destination_path.as_slice())
                        == Some(destination.as_slice())
                    {
                        receivers += 1;
                        target.claim_identity = places[index].claim_identity;
                        target.provenance = places[index].provenance;
                        target.obligation_live = places[index].multiplicity == Multiplicity::Linear;
                    }
                }
                if places[index].multiplicity == Multiplicity::Linear && !nominal && receivers != 1
                {
                    return Err(diagnostics::Diagnostic::error(
                        "constructed child claim has no unique typed receiving path",
                    ));
                }
            }
        }
        append_root_transfer(
            facts,
            machine,
            state,
            owner.consumer,
            evaluation,
            owner.expression,
            places,
            permissions,
            conditional_carrier_transfers,
        );
        // The whole carrier moves even when its selected case has no active
        // claim. Claim expansion alone cannot retire its storage frontier.
        if completed_value {
            remaining.transfer_storage(root, &[]);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_root_transfer(
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    consumer: PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    expression: typed_trees::expression::ExpressionHandle,
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    permissions: &mut Vec<FlowPermissionEventFact>,
    conditional_carrier_transfers: &mut Vec<usize>,
) {
    let root = facts::PlaceRoot::Expression(expression);
    for place in places
        .iter_mut()
        .filter(|place| place.root == root && place.live)
    {
        super::super::linear_validation::record_conditional_carrier_transfer(
            place,
            &[],
            permissions.len(),
            conditional_carrier_transfers,
        );
        permissions.push(FlowPermissionEventFact {
            expression,
            evaluation,
            machine_symbol: machine,
            state_symbol: state,
            source: consumer,
            kind: PermissionEventKind::Transfer,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity: place
                .claim_identity
                .unwrap_or(PermissionClaimIdentity::Unknown),
            provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
            root,
            segments: facts
                .flow
                .ownership
                .segments
                .insert_many(place.path.iter().copied()),
            obligation_live: place.multiplicity == Multiplicity::Linear,
        });
        place.live = false;
    }
}

/// A retained store receives the actual completed value, including when its
/// destination is borrowed storage with no caller-owned claim slot. Named
/// receiving production already owns its writes; this settles only a separate
/// live expression home, without deriving any claim from receiver annotations.
#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn append_stored_transfer(
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    conditional_carrier_transfers: &mut Vec<usize>,
) -> Result<(), diagnostics::Diagnostic> {
    let mut sources = Vec::new();
    for (_, scope) in facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, scope)| scope.machine_symbol == machine && scope.state_symbol == state)
    {
        for row in facts
            .flow
            .control
            .statements
            .span_or_empty(scope.statements)
            .iter()
            .filter(|row| {
                row.statement_index == statement
                    && facts.flow.control.operands.is_valid(row.stored_value)
                    && facts.semantic.places.is_valid(row.stored_destination)
            })
        {
            let value = facts.flow.control.operands.get(row.stored_value);
            if value.state_symbol != state || value.statement_index != statement {
                return Err(diagnostics::Diagnostic::error(
                    "stored owner has a different completion coordinate",
                ));
            }
            let source = (value.expression, row.stored_destination);
            crate::collections::push_unique(&mut sources, source);
        }
    }
    if sources.len() > 1 {
        return Err(diagnostics::Diagnostic::error(
            "stored owner has ambiguous receiving storage",
        ));
    }
    for (expression, _) in sources {
        let root = facts::PlaceRoot::Expression(expression);
        // No expression establishment means the existing named-receiver route
        // owns this write. Never turn absence into transfer authority.
        let live = places.iter().any(|place| place.root == root && place.live);
        let inactive = places
            .iter()
            .any(|place| place.root == root && place.ever_established && place.case_excluded)
            && permissions.iter().any(|event| {
                event.root == root
                    && event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.evaluation == evaluation
                    && event.kind == PermissionEventKind::Establish
                    && matches!(event.source,
                    PermissionEventSource::Statement { statement_index }
                    | PermissionEventSource::Call { statement_index, .. }
                    if statement_index == statement)
            });
        if !live && !inactive {
            continue;
        }
        if !remaining.is_present(root, &[]) {
            // An earlier checked transport already owns an inactive carrier.
            // Its absent claims cannot make another source home available.
            if !live {
                continue;
            }
            return Err(diagnostics::Diagnostic::error(
                "stored owner has no complete source custody",
            ));
        }
        append_root_transfer(
            facts,
            machine,
            state,
            PermissionEventSource::Statement {
                statement_index: statement,
            },
            evaluation,
            expression,
            places,
            permissions,
            conditional_carrier_transfers,
        );
        remaining.transfer_storage(root, &[]);
    }
    Ok(())
}
