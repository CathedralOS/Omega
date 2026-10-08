//! Claim transport across composed-control state edges.
//!
//! A Terminal claim is a machine-local identity that moves with its place: a
//! jump binds each successor structural parameter from its outgoing value,
//! and every claim that value holds moves into the parameter unchanged.
//! Checking instead names a fresh claim at each state's entry. `resolve`
//! joins every such entry claim to the caller claim each incoming edge's
//! checked Transfer event moves into that parameter path — an invocation
//! claim of the entry state, a claim a state established for a local, a
//! construction or a call result, or another state's entry claim — and
//! follows those joins to the one root identity the graph publishes.
//! Emission then binds each successor identity to its root's `ClaimId`, so
//! call, cleanup and return replays in any state resolve the claim they hold.
//!
//! Every arrival of one parameter path must carry the same root claim. A
//! Terminal operation names one static claim, so alternatives arriving with
//! different identities reject rather than invent a join; a cycle through
//! several states settles through its arrivals from outside the cycle.
//!
//! Ordinary borrowed referents use the address-arrival path, not this claim
//! identity resolution.
use super::super::super::{Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, successors};
use checked_trees::{
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use language_semantics::{
    CarryPolicy, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};

/// Resolved claim custody for one composed machine: each successor-state
/// entry claim joined to the root claim every arrival carries, so the
/// lowered `ClaimId` table resolves either identity.
pub(super) struct ClaimTransport {
    pub(super) aliases: Vec<(PermissionClaimIdentity, PermissionClaimIdentity)>,
}

pub(super) fn resolve(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
) -> Result<ClaimTransport, LoweringError> {
    // Each (entry claim, claim an arriving edge moves into its path).
    let mut arrivals = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        if !live[position] || state.entry_claims.is_empty() {
            continue;
        }
        // Dead-source edges never execute, so a live target does not count
        // them among its transporting edges.
        let incoming = plan
            .states
            .iter()
            .enumerate()
            .filter(|(source, _)| live[*source])
            .flat_map(|(source, candidate)| {
                successors(candidate)
                    .into_iter()
                    .filter(|edge| edge.target_state == state.state)
                    .map(move |edge| (source, edge))
            })
            .collect::<Vec<_>>();
        if position != 0 && incoming.is_empty() {
            return unsupported("Unit graph claim has no transporting edge");
        }
        for claim in &state.entry_claims {
            // A projected linear claim can belong to an affine carrier; the
            // exact entry path, not the carrier's root multiplicity,
            // identifies custody.
            let parameter = state
                .structural_parameters
                .get(claim.parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph claim parameter is missing",
                ))?;
            if claim.carry != CarryPolicy::STRICT
                || parameter.access != checked_trees::CheckedStructuralAccess::Owned
                || parameter.multiplicity == Multiplicity::Unrestricted
            {
                return unsupported("Unit graph claim is not exact owned claim custody");
            }
            for (source, edge) in &incoming {
                arrivals.push((
                    claim.claim_identity,
                    arriving_claim(checked, plan, *source, edge, claim)?,
                ));
            }
        }
    }
    // The entry state's claims are the invocation's own; every other entry
    // claim continues whatever its arrivals carry. A source identity that is
    // no successor entry claim was established in this machine and is
    // published under its own identity.
    let successor_claims = plan
        .states
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(position, _)| live[*position])
        .flat_map(|(_, state)| &state.entry_claims)
        .map(|claim| claim.claim_identity)
        .collect::<Vec<_>>();
    let mut roots: Vec<(PermissionClaimIdentity, PermissionClaimIdentity)> = Vec::new();
    let root_of = |roots: &[(PermissionClaimIdentity, PermissionClaimIdentity)],
                   identity: PermissionClaimIdentity| {
        if successor_claims.contains(&identity) {
            roots
                .iter()
                .find(|(successor, _)| *successor == identity)
                .map(|(_, root)| *root)
        } else {
            Some(identity)
        }
    };
    loop {
        let mut progress = false;
        for (claim, arriving) in &arrivals {
            if !successor_claims.contains(claim) || root_of(&roots, *claim).is_some() {
                continue;
            }
            if let Some(root) = root_of(&roots, *arriving) {
                roots.push((*claim, root));
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    // Every arrival, including a reentry of the authored entry, must carry
    // the one root its target claim settled on.
    for (claim, arriving) in &arrivals {
        let (Some(expected), Some(actual)) = (root_of(&roots, *claim), root_of(&roots, *arriving))
        else {
            return unsupported("Unit graph claim transport lost its root claim");
        };
        if expected != actual {
            return unsupported("Unit graph claim parameter carried from divergent claims");
        }
    }
    if successor_claims
        .iter()
        .any(|claim| root_of(&roots, *claim).is_none())
    {
        return unsupported("Unit graph claim transport lost its root claim");
    }
    Ok(ClaimTransport { aliases: roots })
}

/// The caller claim one edge moves into `claim`'s parameter path: the
/// edge's checked Transfer event rooted at the transferred value's owner,
/// beneath the transfer's own projection.
fn arriving_claim(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source_position: usize,
    edge: &CheckedStructuralControlSuccessorPlan,
    claim: &CheckedUnitEntryClaimPlan,
) -> Result<PermissionClaimIdentity, LoweringError> {
    let source = &plan.states[source_position];
    let mut transfers = edge
        .transfers
        .iter()
        .filter(|transfer| transfer.target_parameter_index == claim.parameter_index);
    let (Some(transfer), None) = (transfers.next(), transfers.next()) else {
        return unsupported("Unit graph claim transfer row is missing");
    };
    let (root, prefix) = transferred_owner(checked, source, &transfer.source)?;
    let statement_index = edge.statement_ordinal as usize;
    let root_type = validation::canonical_place_type_reference(
        &checked.typed,
        source.state,
        statement_index,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )
    .ok_or(LoweringError::Unsupported(
        "Unit graph claim source has no owner type",
    ))?;
    let machine = super::owner(checked, source.state)?;
    // A transition names its target state, or the owning machine when it
    // reenters that machine's entry.
    let target_machine = super::owner(checked, edge.target_state)?;
    let mut arriving = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == source.state
                && matches!(event.source,
                    PermissionEventSource::Call { statement_index: index, target_symbol, .. }
                        if index == statement_index
                            && (target_symbol == edge.target_state
                                || target_symbol == target_machine))
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.root == root
        })
        .filter_map(|event| {
            let (path, _) = validation::structural_owned_path(
                &checked.typed,
                root_type,
                checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments),
            )
            .ok()?;
            (path.strip_prefix(prefix.as_slice()) == Some(claim.path.as_slice()))
                .then_some(event.claim_identity)
        });
    let (Some(identity), None) = (arriving.next(), arriving.next()) else {
        return unsupported("Unit graph claim has no unique arriving transfer");
    };
    if identity == PermissionClaimIdentity::Unknown {
        return unsupported("Unit graph claim arrives without an identity");
    }
    Ok(identity)
}

/// The owner a successor transfer moves out of its source state, and the
/// projection beneath that owner it selects.
fn transferred_owner(
    checked: &CheckedTrees,
    source: &checked_trees::CheckedComposedUnitControlStatePlan,
    transfer: &CheckedStructuralControlTransferSourcePlan,
) -> Result<(facts::PlaceRoot, Vec<CheckedUnitStructuralPathSegment>), LoweringError> {
    match transfer {
        CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
            Ok((parameter_root(checked, source, *index)?, Vec::new()))
        }
        CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
            Ok((result_root(source, *binding_ordinal)?, Vec::new()))
        }
        CheckedStructuralControlTransferSourcePlan::ProjectedValue { subject } => {
            let root = match subject.source {
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    parameter_root(checked, source, parameter_index)?
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    result_root(source, binding_ordinal)?
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    facts::PlaceRoot::Symbol(symbol)
                }
                _ => return unsupported("Unit graph claim projects an unsupported owner"),
            };
            Ok((root, subject.path.clone()))
        }
        CheckedStructuralControlTransferSourcePlan::EstablishedValue { establishment, .. } => {
            let expression = match establishment.as_ref() {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value,
                    ..
                } if checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .is_valid(*value) =>
                {
                    checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value)
                        .expression
                }
                checked_trees::CheckedUnitEffectOperationPlan::Call(call) => {
                    match call.source_site().flatten() {
                        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
                            expression
                        }
                        _ => return unsupported("Unit graph claim call operand has no occurrence"),
                    }
                }
                _ => return unsupported("Unit graph claim operand has no owner occurrence"),
            };
            Ok((facts::PlaceRoot::Expression(expression), Vec::new()))
        }
        _ => unsupported("Unit graph claim arrives through an unsupported transfer"),
    }
}

fn parameter_root(
    checked: &CheckedTrees,
    source: &checked_trees::CheckedComposedUnitControlStatePlan,
    index: u32,
) -> Result<facts::PlaceRoot, LoweringError> {
    let parameter =
        source
            .structural_parameters
            .get(index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph claim transfer source parameter is missing",
            ))?;
    let (_, authored) = crate::declarations::state_declaration(checked, source.state)?;
    let authored = checked
        .state_parameters(authored)
        .get(parameter.position as usize)
        .ok_or(LoweringError::Unsupported(
            "Unit graph claim transfer source parameter lost its declaration",
        ))?;
    Ok(facts::PlaceRoot::Symbol(authored.symbol))
}

fn result_root(
    source: &checked_trees::CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Result<facts::PlaceRoot, LoweringError> {
    let mut results = source
        .operation_dependencies()
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::structural_result_binding)
        .filter(|result| result.binding_ordinal == binding_ordinal);
    let (Some(result), None) = (results.next(), results.next()) else {
        return unsupported("Unit graph claim transfer source result is missing");
    };
    if !result.local.symbol.is_valid() {
        return unsupported("Unit graph claim transfers an unnamed state result");
    }
    Ok(facts::PlaceRoot::Symbol(result.local.symbol))
}
