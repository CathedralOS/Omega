//! A nested case decision may constrain only its actual unchanged owned home.

use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowOperandAddressProjection, FlowOperandCompletion, FlowOperandFact};
use std::collections::BTreeSet;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};
use validation::completed_storage::{StoredObservation, contents_preserved_from};

pub(super) struct OwnedCaseSubject {
    place: validation::CanonicalPlace,
    captured_storage: Handle<checked_trees::FlowStorageSnapshot>,
    checked_storage: Handle<checked_trees::FlowStorageSnapshot>,
    source_current: bool,
    path: Option<HandleSpan<facts::PlaceSegment>>,
}

impl Execution<'_, '_, '_> {
    pub(super) fn owned_case_subject(&self, subject: &FlowOperandFact) -> Option<OwnedCaseSubject> {
        if subject.state_symbol != self.state.symbol
            || subject.statement_index != self.statement_index
            || subject.evaluation != self.evaluation
            || subject.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
        {
            return None;
        }
        let mut current = *subject;
        let mut projections = Vec::new();
        let mut visited = BTreeSet::new();
        loop {
            if current.state_symbol != self.state.symbol
                || !current.type_reference.is_valid()
                || validation::reference_shell(self.program, current.type_reference).is_some()
                || self.program.type_multiplicity(current.type_reference)
                    == language_semantics::Multiplicity::Unrestricted
                || matches!(current.completion, FlowOperandCompletion::Copied { .. })
            {
                return None;
            }
            let Some(projection) = current.address_projection else {
                if current.backing.is_valid() {
                    return None;
                }
                break;
            };
            let FlowOperandAddressProjection::Place(path) = projection else {
                return None;
            };
            let backing = current.backing;
            if !self.context.control.operands.is_valid(backing)
                || !visited.insert((backing.arena_index(), backing.generation()))
            {
                return None;
            }
            let segments = self.context.control.operand_child_paths.span(path)?;
            let receiver = self.context.control.operands.get(backing);
            let projected = validation::owned_projection_type_reference(
                self.program,
                receiver.type_reference,
                segments,
            )?;
            if self.program.normalized_type_identity(projected)
                != self
                    .program
                    .normalized_type_identity(current.type_reference)
            {
                return None;
            }
            projections.push(path);
            current = *self.context.control.operands.get(backing);
        }
        let mut place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            current.statement_index,
            current.expression,
        )?;
        // An expression namespace needs its own completed value formation.
        // In particular, a missing saved supplier is not a new owner.
        if place.root.as_expression().is_some()
            && !matches!(
                current.completion,
                FlowOperandCompletion::Constructed(_)
                    | FlowOperandCompletion::InvocationResult { .. }
            )
        {
            return None;
        }
        for path in projections.into_iter().rev() {
            place
                .segments
                .extend_from_slice(self.context.control.operand_child_paths.span(path)?);
        }
        if !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        }) {
            // A live selector is not the saved index value. Without a fixed
            // captured coordinate it cannot select an owner's cleanup path.
            return None;
        }
        let reference = validation::canonical_place_type_reference(
            self.program,
            self.state.symbol,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(reference)
            != self
                .program
                .normalized_type_identity(subject.type_reference)
            || !self
                .context
                .control
                .storage_snapshots
                .is_valid(current.storage_snapshot)
            || !self
                .context
                .control
                .storage_snapshots
                .is_valid(self.storage_snapshot)
            || !contents_preserved_from(
                self.program,
                &self.context.control,
                self.storage_snapshot,
                &place,
                &[current.storage_snapshot],
                StoredObservation::InlineContents,
            )
        {
            return None;
        }
        Some(OwnedCaseSubject {
            place,
            captured_storage: current.storage_snapshot,
            checked_storage: self.storage_snapshot,
            source_current: true,
            path: None,
        })
    }

    pub(super) fn retain_owned_case_selection(
        &mut self,
        expression: ExpressionHandle,
        source_arm: Handle<TableMatchArm>,
        subject: &FlowOperandFact,
        owner: &mut OwnedCaseSubject,
        variant: symbols::SymbolHandle,
        miss_excludes_case: bool,
    ) {
        if !owner.source_current {
            return;
        }
        if owner.checked_storage != self.storage_snapshot {
            owner.source_current = self
                .context
                .control
                .storage_snapshots
                .is_valid(self.storage_snapshot)
                && contents_preserved_from(
                    self.program,
                    &self.context.control,
                    self.storage_snapshot,
                    &owner.place,
                    &[owner.checked_storage],
                    StoredObservation::InlineContents,
                );
            if !owner.source_current {
                return;
            }
            owner.checked_storage = self.storage_snapshot;
        }
        let subject = self.context.control.operands.insert(*subject);
        let path = *owner.path.get_or_insert_with(|| {
            self.context
                .control
                .operand_referent_segments
                .insert_many(owner.place.segments.iter().copied())
        });
        let rows = &mut self
            .context
            .control
            .statements
            .get_mut(self.statement_fact)
            .owned_case_selections;
        self.context.control.owned_case_selections.append_to_span(
            rows,
            checked_trees::FlowOwnedCaseSelection {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                statement: self.statement_fact,
                expression,
                source_arm,
                evaluation: self.evaluation,
                subject,
                root: owner.place.root,
                path,
                variant,
                miss_excludes_case,
                captured_storage: owner.captured_storage,
                selected_storage: self.storage_snapshot,
            },
        );
    }
}
