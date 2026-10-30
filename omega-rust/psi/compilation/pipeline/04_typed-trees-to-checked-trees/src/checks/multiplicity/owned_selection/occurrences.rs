//! Selection consumes an occurrence, not its variable forever. This scratch
//! frontier travels with the permission schedule's edge-local custody. Only
//! an authenticated covering write, after RHS evaluation, ends its barrier;
//! inherited claim identity or provenance does not date that new occurrence.

use super::place_paths_overlap;
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use checked_trees::{
    CheckFacts, FlowOwnedSelectionReceipt, FlowOwnershipFacts, FlowPermissionEventFact,
};
use diagnostics::Diagnostic;
use language_semantics::{Multiplicity, PermissionAccess, PermissionEventKind};
use validation::CanonicalPlace;

#[derive(Clone)]
struct ConsumedRegion {
    place: CanonicalPlace,
    linear: bool,
}

#[derive(Clone, Default)]
pub(in crate::checks::multiplicity) struct SelectionOccurrences {
    consumed: Vec<ConsumedRegion>,
}

impl SelectionOccurrences {
    pub(in crate::checks::multiplicity) fn consume(
        &mut self,
        program: &dyn validation::CheckingInput,
        ownership: &FlowOwnershipFacts,
        receipt: &FlowOwnedSelectionReceipt,
    ) {
        let linear = program.type_multiplicity(receipt.type_reference) == Multiplicity::Linear;
        let mut regions = Vec::new();
        if linear {
            for transfer in ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
            {
                if !transfer.source.is_valid() {
                    continue;
                }
                let root = facts::PlaceRoot::Symbol(
                    ownership.selection_sources.get(transfer.source).symbol,
                );
                let claims = ownership
                    .selection_transfer_claims
                    .span_or_empty(transfer.claims);
                if claims.is_empty() {
                    regions.push(CanonicalPlace {
                        root,
                        segments: ownership.segments.span_or_empty(transfer.path).to_vec(),
                    });
                } else {
                    regions.extend(claims.iter().map(|claim| CanonicalPlace {
                        root,
                        segments: ownership.segments.span_or_empty(claim.path).to_vec(),
                    }));
                }
            }
        } else {
            regions.extend(
                ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .map(|source| CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(source.symbol),
                        segments: Vec::new(),
                    }),
            );
        }
        for place in regions {
            if !self.consumed.iter().any(|region| region.place == place) {
                self.consumed.push(ConsumedRegion { place, linear });
            }
        }
    }

    /// Observe actual reads, not the names nested inside projections or a
    /// whole assignment destination. A strict child write cannot fill its
    /// unavailable containing owner, even if no later read occurs.
    pub(in crate::checks::multiplicity) fn check_statement(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
    ) -> Result<(), Diagnostic> {
        if self.consumed.is_empty() {
            return Ok(());
        }
        for occurrence in crate::checks::ownership_occurrences::statement_steps(
            program,
            machine,
            state,
            statement_index,
            statement,
            &[],
            &facts.operators,
        ) {
            if let crate::checks::ownership_occurrences::StatementStep::Observe { place, .. } =
                occurrence.step
                && let Some(region) = self.conflict(place.root, &place.segments)
            {
                return Err(consumed_use(region.linear));
            }
        }
        if let typed_trees::statement::StatementNode::Assignment(assignment) = statement
            && let Some(destination) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                assignment.target,
            )
            && let Some(region) = self.consumed.iter().find(|region| {
                region.place.root == destination.root
                    && place_paths_overlap(&destination.segments, &region.place.segments)
                    && !region.place.segments.starts_with(&destination.segments)
            })
        {
            return Err(consumed_use(region.linear));
        }
        Ok(())
    }

    fn conflict(
        &self,
        root: facts::PlaceRoot,
        path: &[facts::PlaceSegment],
    ) -> Option<&ConsumedRegion> {
        self.consumed.iter().find(|region| {
            region.place.root == root && place_paths_overlap(path, &region.place.segments)
        })
    }

    /// No unconditional event may consume or restore a still-selected source.
    /// The producer checks its RHS prefix before calling `establish`; replay
    /// checks the same ordered events using the checked write destination.
    pub(in crate::checks::multiplicity) fn check_events<'a>(
        &self,
        program: &dyn validation::CheckingInput,
        events: impl IntoIterator<Item = &'a FlowPermissionEventFact>,
        segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    ) -> Result<(), Diagnostic> {
        for event in events {
            if event.access == PermissionAccess::Owned
                // An affine death names a storage home, not entitlement to
                // every child. Contextual cleanup applies its exact remaining
                // partition; a moved selection leaf is already excluded. It
                // must not be mistaken for a second whole-value transfer.
                && event.kind != language_semantics::PermissionEventKind::AffineDrop
                && self
                    .conflict(event.root, segments.span_or_empty(event.segments))
                    .is_some()
            {
                return Err(Diagnostic::error(format!(
                    "owned selection acquired unconditional transfer, establishment, or residual cleanup events in machine `{}` state `{}` at {:?}",
                    program.symbols.name(event.machine_symbol),
                    program.symbols.name(event.state_symbol),
                    event.source,
                )));
            }
        }
        Ok(())
    }

    pub(in crate::checks::multiplicity) fn establish(
        &mut self,
        root: facts::PlaceRoot,
        destination: &[facts::PlaceSegment],
    ) {
        self.consumed.retain(|region| {
            region.place.root != root || !region.place.segments.starts_with(destination)
        });
    }

    pub(in crate::checks::multiplicity) fn replay_statement(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        evaluation: checked_trees::FlowPermissionEvaluation,
        events: &[&FlowPermissionEventFact],
        ownership: &FlowOwnershipFacts,
        places: &[LinearPlace],
    ) -> Result<(), Diagnostic> {
        let statement = &program.statement_table.statements(state.statement_nodes)[statement_index];
        let targets = super::super::linear_validation::written_linear_targets(
            program,
            state.symbol,
            statement_index,
            statement,
            evaluation,
            places,
        );
        for event in events.iter().copied().filter(|event| {
            super::super::linear_validation::permission_event_statement_index(event.source)
                == Some(statement_index)
                && event.evaluation == evaluation
        }) {
            if event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
            {
                let path = ownership.segments.span_or_empty(event.segments);
                for target in targets.iter().filter(|target| {
                    target.root == event.root && places[target.place_index].path == path
                }) {
                    self.establish(target.root, &target.destination_path);
                }
            }
            self.check_events(program, std::iter::once(event), &ownership.segments)?;
        }
        Ok(())
    }
}

fn consumed_use(linear: bool) -> Diagnostic {
    Diagnostic::error(if linear {
        "linear match transfer consumed this exact claim on every edge; it cannot be used here"
    } else {
        "owned value may have been transferred by an earlier match; it cannot be used here"
    })
}
