//! Call continuations retire the complement of an already transferred subtree.
//! Establishment and transfer belong to the shared occurrence ledger, not this
//! cleanup. The original owner, rather than the projected argument, dies here.
use super::owners::{TemporaryConsumption, TemporaryOwner};
use super::{
    CheckFacts, FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionProvenance, SymbolHandle,
};
use language_semantics::PermissionEventSource;

#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn append_cleanup(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [super::super::linear_obligations::LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) -> Result<Vec<facts::PlaceRoot>, diagnostics::Diagnostic> {
    let mut retired = Vec::new();
    for owner in owners
        .iter()
        .rev()
        .filter(|owner| owner.consumer == consumer && owner.evaluation == evaluation)
    {
        let TemporaryConsumption::OwnedProjection { path } = &owner.consumption else {
            continue;
        };
        if !matches!(consumer, PermissionEventSource::Call { .. }) {
            continue;
        }
        retired.push(retire_owner(
            program,
            &mut facts.flow.ownership,
            machine,
            state,
            ProjectedConsumer {
                source: consumer,
                evaluation,
                producer: PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: owner.producer,
                },
            },
            owner.expression,
            path,
            places,
            remaining,
            permissions,
        )?);
    }
    Ok(retired)
}

/// The completed consumer of one projected move out of an anonymous owner.
pub(in crate::checks::multiplicity) struct ProjectedConsumer {
    pub(in crate::checks::multiplicity) source: PermissionEventSource,
    pub(in crate::checks::multiplicity) evaluation: checked_trees::FlowPermissionEvaluation,
    /// The establishment of the anonymous owner whose complement dies here.
    pub(in crate::checks::multiplicity) producer: PermissionProvenance,
}

/// Retire an anonymous owner whose selected subtree has already transferred:
/// its untouched complement dies at the consumer that completed the move,
/// whether that consumer is a call continuation or the statement whose value
/// received the subtree. Nothing can name the owner afterwards.
#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn retire_owner(
    program: &dyn validation::CheckingInput,
    ownership: &mut checked_trees::FlowOwnershipFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: ProjectedConsumer,
    expression: typed_trees::expression::ExpressionHandle,
    path: &[facts::PlaceSegment],
    places: &mut [super::super::linear_obligations::LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) -> Result<facts::PlaceRoot, diagnostics::Diagnostic> {
    let root = facts::PlaceRoot::Expression(expression);
    let statement = match consumer.source {
        PermissionEventSource::Call {
            statement_index, ..
        }
        | PermissionEventSource::Statement { statement_index } => statement_index,
        _ => {
            return Err(diagnostics::Diagnostic::error(
                "projected temporary cleanup has no completing call or statement",
            ));
        }
    };
    let reference = super::owners::projectable_owner(program, state, statement, expression)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error("projected temporary cleanup has no exact affine owner")
        })?;
    if path.is_empty()
        || remaining.is_present(root, path)
        || !remaining.is_present(root, &[])
        || !places
            .iter()
            .any(|place| place.root == root && place.ever_established)
    {
        return Err(diagnostics::Diagnostic::error(
            "projected temporary cleanup lacks its exact completed transfer and residual partition",
        ));
    }
    let start = permissions.len();
    // This is the same exact owned-region judgment used by exits and
    // replacements. The actual transfer already excluded its subtree from
    // `remaining`; live siblings keep their original claim identities and
    // provenance. The containing disposition publishes those exclusions before
    // its child Consume receipts, so cleanup never disposes the moved subtree.
    super::super::linear_validation::append_owned_region_cleanup_permission_events(
        program,
        reference,
        root,
        &[],
        places,
        remaining,
        FlowPermissionEventFact {
            expression,
            evaluation: consumer.evaluation,
            machine_symbol: machine,
            state_symbol: state,
            source: consumer.source,
            kind: PermissionEventKind::AffineDrop,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: consumer.producer,
            root,
            segments: Default::default(),
            obligation_live: false,
        },
        &mut ownership.segments,
        permissions,
    );
    if permissions.len() == start {
        return Err(diagnostics::Diagnostic::error(
            "cannot automatically dispose a projected temporary: its exact residual claims need owner-authorized terminal dispositions",
        ));
    }
    // Every live claim below this original owner now has its own authorized
    // Consume receipt. Clearing custody is a consequence of that complete
    // partition, never an affine containing home's permission to erase debt.
    for place in places.iter_mut().filter(|place| place.root == root) {
        place.live = false;
    }
    Ok(root)
}
