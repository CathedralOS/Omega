//! Candidate normal deaths retained across selected value transfers.
//!
//! Source availability remains conservative: a possible transfer forbids reuse.
//! Cleanup instead keeps the established occurrence until the selected runtime
//! topology either transfers it or reaches its actual normal death. Candidate
//! rows never enter the unconditional permission replay ledger.
use super::cleanup_dispositions::RemainingCustody;
use super::linear_obligations::LinearPlace;
use checked_trees::{
    ContextualCleanupFacts, FlowClaimJoinExit, FlowOwnershipFacts, FlowPermissionEventFact,
};
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{PermissionAccess, PermissionEventKind, PermissionProvenance};

#[derive(Clone, Default)]
pub(super) struct SelectedCleanup {
    owners: Vec<Owner>,
}

#[derive(Clone)]
struct Owner {
    root: PlaceRoot,
    places: Vec<LinearPlace>,
    remaining: RemainingCustody,
    transfers: Vec<FlowPermissionEventFact>,
}

impl SelectedCleanup {
    pub(super) fn observe(
        &mut self,
        program: &dyn validation::CheckingInput,
        selected: &[typed_trees::expression::ExpressionHandle],
        places: &[LinearPlace],
        remaining: &RemainingCustody,
        events: &[FlowPermissionEventFact],
        segments: &arena::Arena<PlaceSegment>,
    ) -> Result<(), diagnostics::Diagnostic> {
        let mut prefix = remaining.clone();
        for event in events
            .iter()
            .filter(|event| event.access == PermissionAccess::Owned)
        {
            let path = segments.span_or_empty(event.segments);
            let before_transfer = (selected.contains(&event.expression)
                && matches!(
                    event.kind,
                    PermissionEventKind::Transfer | PermissionEventKind::Consume
                ))
            .then(|| prefix.clone());
            prefix.advance(program, std::slice::from_ref(event), segments);
            if matches!(
                event.kind,
                PermissionEventKind::Establish | PermissionEventKind::Restore
            ) && self.owners.iter().any(|owner| owner.root == event.root)
            {
                // A replacement is a different death, not permission to silently
                // reset the old occurrence's skipped-path obligation.
                return Err(unsupported(
                    program,
                    event,
                    "selected owner replacement requires an ordered conditional old-value cleanup partition",
                ));
            }
            if !matches!(
                event.kind,
                PermissionEventKind::Transfer | PermissionEventKind::Consume
            ) {
                continue;
            }
            if !selected.contains(&event.expression) {
                if path.is_empty() {
                    self.owners.retain(|owner| owner.root != event.root);
                } else {
                    for owner in self
                        .owners
                        .iter_mut()
                        .filter(|owner| owner.root == event.root)
                    {
                        owner.remaining.transfer_storage(event.root, path);
                    }
                }
                continue;
            }
            if event.root.as_symbol().is_none() {
                // A branch-local expression is born on that branch and retains
                // its ordinary producer custody, not a preexisting skipped owner.
                continue;
            }
            if !path.is_empty() {
                let statement = match event.source {
                    language_semantics::PermissionEventSource::Statement { statement_index }
                    | language_semantics::PermissionEventSource::Call {
                        statement_index, ..
                    } => statement_index,
                    _ => 0,
                };
                let reference = validation::canonical_place_type_reference(
                    program,
                    event.state_symbol,
                    statement,
                    &validation::CanonicalPlace {
                        root: event.root,
                        segments: path.to_vec(),
                    },
                );
                if reference.is_none_or(|reference| {
                    validation::type_graph_requires_nominal_drop(
                        program,
                        reference,
                        &[],
                        &mut std::collections::BTreeSet::new(),
                    )
                }) {
                    return Err(unsupported(
                        program,
                        event,
                        "selected partial owner cleanup requires an ordered conditional region partition",
                    ));
                }
                // No-code region reconciliation already owns these partial
                // transfers. This candidate roster introduces no second death.
                continue;
            }
            if let Some(owner) = self
                .owners
                .iter_mut()
                .find(|owner| owner.root == event.root)
            {
                owner.transfers.push(event.clone());
                continue;
            }
            let established = places
                .iter()
                .filter(|place| place.root == event.root)
                .cloned()
                .collect::<Vec<_>>();
            if established.is_empty()
                || !established
                    .iter()
                    .any(|place| place.ever_established && place.live)
                || event.provenance == PermissionProvenance::Unknown
                || !established
                    .iter()
                    .any(|place| place.provenance == Some(event.provenance))
                || before_transfer
                    .as_ref()
                    .is_none_or(|remaining| !remaining.is_present(event.root, path))
            {
                continue;
            }
            self.owners.push(Owner {
                root: event.root,
                places: established,
                remaining: before_transfer.expect("selected transfer retained its exact prefix"),
                transfers: vec![event.clone()],
            });
        }
        Ok(())
    }

    pub(super) fn finish(
        &self,
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        machine: symbols::SymbolHandle,
        exit: FlowClaimJoinExit,
        ordinary: &[FlowPermissionEventFact],
        remaining: &mut RemainingCustody,
        ownership: &mut FlowOwnershipFacts,
        output: &mut ContextualCleanupFacts,
    ) {
        let mut deaths = ordinary
            .iter()
            .cloned()
            .map(|event| (event, None))
            .collect::<Vec<_>>();
        for (index, owner) in self.owners.iter().enumerate() {
            if ordinary.iter().any(|event| event.root == owner.root) {
                continue;
            }
            let mut candidates = Vec::new();
            super::linear_validation::append_owned_cleanup_permission_events(
                program,
                state,
                machine,
                exit,
                &owner.places,
                &owner.remaining,
                &mut ownership.segments,
                &mut candidates,
            );
            for candidate in candidates
                .into_iter()
                .filter(|event| event.root == owner.root)
            {
                ownership
                    .selected_cleanup_permissions
                    .append(candidate.clone());
                deaths.push((candidate, Some(index)));
            }
        }
        // Preserve the source cleanup producer's reverse declaration order
        // when ordinary and selected owners interleave (including later births).
        let mut roots = program
            .state_parameters(state)
            .iter()
            .map(|parameter| PlaceRoot::Symbol(parameter.symbol))
            .collect::<Vec<_>>();
        roots.extend(
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| {
                    statement
                        .as_local_data()
                        .map(|local| PlaceRoot::Symbol(local.symbol))
                }),
        );
        deaths.sort_by_key(|(event, _)| {
            std::cmp::Reverse(roots.iter().position(|root| *root == event.root))
        });
        for (event, candidate) in deaths {
            if let Some(index) = candidate {
                self.owners[index].remaining.clone().observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    true,
                );
            } else {
                remaining.observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    false,
                );
            }
        }
    }
}

fn unsupported(
    program: &dyn validation::CheckingInput,
    event: &FlowPermissionEventFact,
    message: &'static str,
) -> diagnostics::Diagnostic {
    use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
    let site = match event.source {
        language_semantics::PermissionEventSource::Statement { statement_index }
        | language_semantics::PermissionEventSource::Call {
            statement_index, ..
        } => u32::try_from(statement_index)
            .ok()
            .map(|index| GapSite::Statement {
                state: event.state_symbol,
                index,
            }),
        _ => None,
    }
    .unwrap_or(GapSite::State(event.state_symbol));
    let gap = plan_gap(
        program,
        event.machine_symbol,
        site,
        checked_trees::CheckedUnitPlanGapReason::Unsupported(message),
    );
    diagnostics::Diagnostic::incomplete(
        message,
        gap.coverage.exceeded(gap.source_span.span.start as u64),
    )
    .with_source_span(gap.source_span)
}
