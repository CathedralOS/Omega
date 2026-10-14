//! Borrowed access to an anonymous owner ends before its call cleanup.
//! Every contained linear obligation needs an authorized disposition on normal
//! completion, including obligations outside a borrowed projection.
use super::owners::{TemporaryConsumption, TemporaryOwner};
use super::{
    CheckFacts, FlowPermissionEventFact, HandleSpan, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionProvenance,
};
use diagnostics::Diagnostic;
use language_semantics::PermissionEventSource;

pub(in crate::checks::multiplicity) fn append_borrow(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    consumer: PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    permissions: &mut Vec<FlowPermissionEventFact>,
    diagnostics: &mut Vec<Diagnostic>,
    dispositions: &mut checked_trees::ContextualCleanupFacts,
) -> Vec<facts::PlaceRoot> {
    let mut retired = Vec::new();
    for (owner, access, borrowed_path) in owners
        .iter()
        .filter_map(|owner| match &owner.consumption {
            TemporaryConsumption::Borrow {
                access,
                borrowed_path,
            } => Some((owner, *access, borrowed_path)),
            TemporaryConsumption::OwnedTransfer | TemporaryConsumption::OwnedProjection { .. } => {
                None
            }
        })
        .filter(|(owner, _, _)| owner.consumer == consumer && owner.evaluation == evaluation)
    {
        let root = facts::PlaceRoot::Expression(owner.expression);
        if super::super::owned_cleanup::claim_receipts(
            program,
            owner.type_reference,
            root,
            &[],
            places,
            &[],
        )
        .is_none()
        {
            let PermissionEventSource::Call {
                target_symbol,
                statement_index,
                ..
            } = owner.consumer
            else {
                continue;
            };
            diagnostics.push(Diagnostic::error(format!(
                "cannot automatically dispose borrowed temporary `{}` after call `{}` at statement {}: every contained linear obligation needs its owner-authorized terminal disposition",
                program.normalized_type_identity_text(owner.type_reference),
                program.symbols.name(target_symbol), statement_index,
            )));
            continue;
        }
        if validation::reference_result_custody::binding_multiplicity(program, owner.type_reference)
            == Multiplicity::Unrestricted
        {
            continue;
        }
        // Every owned temporary needs its exact normal disposition, including
        // claim-free affine owners whose selected payload invokes a hook.
        // The borrow lends access; it cannot retire the backing owner. The
        // admitted claim receipts above retain every Linear authorization.
        dispositions
            .dispositions
            .append(checked_trees::ContextualCleanupDisposition {
                establishment_ordered: false,
                zero_image: false,
                machine,
                state,
                source: owner.consumer,
                exit: checked_trees::FlowClaimJoinExit::default(),
                root,
                disposed_type: owner.type_reference,
                path: HandleSpan::empty(),
                exclusions: HandleSpan::empty(),
            });
        permissions.push(FlowPermissionEventFact {
            expression: Default::default(),
            evaluation,
            machine_symbol: machine,
            state_symbol: state,
            source: owner.consumer,
            kind: PermissionEventKind::Establish,
            multiplicity: Multiplicity::Unrestricted,
            access,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: owner.producer,
            },
            root,
            segments: facts
                .flow
                .ownership
                .segments
                .insert_many(borrowed_path.iter().copied()),
            obligation_live: false,
        });
        for place in places
            .iter_mut()
            .filter(|place| place.root == root && place.live)
        {
            permissions.push(FlowPermissionEventFact {
                expression: Default::default(),
                evaluation,
                machine_symbol: machine,
                state_symbol: state,
                source: owner.consumer,
                kind: if place.multiplicity == Multiplicity::Linear {
                    PermissionEventKind::Consume
                } else {
                    PermissionEventKind::AffineDrop
                },
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
                obligation_live: false,
            });
            place.live = false;
        }
        // The disposition owns the containing home as well as its claim
        // leaves. Leaf receipts alone would leave an empty transparent home
        // available for a second cleanup at state exit.
        retired.push(root);
    }
    retired
}
