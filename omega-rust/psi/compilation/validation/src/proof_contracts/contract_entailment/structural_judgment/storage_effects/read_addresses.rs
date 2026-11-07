//! A selector may write payload after capturing its collection's address.
//! Retain that completed read on a live place only when every descriptor in
//! its captured address spine still holds the same pointer at the read cut.
//! Payload preservation, access, bounds and lifetime remain separate checks.

use super::{CanonicalPlace, ExpressionHandle, Machine, State, StructuralJudge};
use crate::completed_storage::{self, StoredObservation, StoredOperand};
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandAddressProjection, FlowOperandFact};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_read_address_is_current(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(graph) = self.storage_graph() else {
            return false;
        };
        let Some(at) = self.read_storage_version(state, expression) else {
            return false;
        };
        if self.storage_version != Some(at)
            || !self.ordinary_storage_path(Some((machine, state)), expression)
        {
            return false;
        }
        let Some(original) = crate::canonical_place_from_expression(self.program, expression)
        else {
            return false;
        };
        let Some(place) = self
            .capture_storage_place(original.clone())
            .and_then(|coordinate| Self::coordinate_place(&coordinate))
        else {
            return false;
        };
        // Select all retained occurrences, never one convenient generated use.
        // A shared expression with different execution lanes stays unavailable.
        graph.with_expression_operands(expression, |rows| {
            if rows.iter().any(|handle| !graph.operands.is_valid(*handle)) {
                return false;
            }
            let mut selected = rows
                .iter()
                .copied()
                .filter(|handle| graph.operands.get(*handle).state_symbol == state.symbol);
            let Some(first) = selected.next() else {
                return false;
            };
            let site = graph.operands.get(first);
            if site.evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                return false;
            }
            std::iter::once(first).chain(selected).all(|handle| {
                let row = graph.operands.get(handle);
                row.expression == expression
                    && row.statement_index == site.statement_index
                    && row.evaluation == site.evaluation
                    && row.storage_snapshot == at.snapshot
                    && self.captured_address_is_current(graph, handle, &original, place.clone())
            })
        })
    }

    fn captured_address_is_current(
        &self,
        graph: &FlowControlFacts,
        read: Handle<FlowOperandFact>,
        original: &CanonicalPlace,
        mut place: CanonicalPlace,
    ) -> bool {
        let site = graph.operands.get(read);
        let mut current = read;
        // One captured receiver edge per step; malformed cycles cannot search
        // beyond the finite operand roster or fall back to a source spelling.
        for _ in 0..graph.operands.len() {
            let row = graph.operands.get(current);
            if row.state_symbol != site.state_symbol
                || row.statement_index != site.statement_index
                || row.evaluation != site.evaluation
            {
                return false;
            }
            let Some(projection) = row.address_projection else {
                if row.backing.is_valid() {
                    return false;
                }
                let Some(base) =
                    crate::canonical_place_from_expression(self.program, row.expression)
                else {
                    return false;
                };
                // A missing projection record cannot stand for an unobserved
                // field or descriptor crossing. The spine must reach its
                // declared root after consuming every retained segment.
                if !base.segments.is_empty() || !place.segments.is_empty() {
                    return false;
                }
                let Some(reference) = crate::canonical_place_type_reference(
                    self.program,
                    row.state_symbol,
                    row.statement_index,
                    &base,
                ) else {
                    return false;
                };
                let declared = self.program.normalized_type_identity(reference);
                let captured = self.program.normalized_type_identity(row.type_reference);
                return base.root == original.root
                    && declared.is_valid()
                    && captured.is_valid()
                    && declared == captured;
            };
            let FlowOperandAddressProjection::Place(path) = projection else {
                return false;
            };
            let Some(path) = graph.operand_child_paths.span(path) else {
                return false;
            };
            let Some(start) = place.segments.len().checked_sub(path.len()) else {
                return false;
            };
            if !path.iter().enumerate().all(|(offset, segment)| {
                let index = start + offset;
                crate::storage_path_relations::canonical_place_segments_equal(
                    self.program,
                    *segment,
                    original.segments[index],
                ) || *segment == place.segments[index]
            }) || !graph.operands.is_valid(row.backing)
            {
                return false;
            }
            let receiver = graph.operands.get(row.backing);
            let shell = crate::reference_shell(self.program, receiver.type_reference);
            let carrier = shell.map_or(receiver.type_reference, |shell| shell.referee);
            let Some(projected) =
                crate::owned_projection_type_reference(self.program, carrier, path)
            else {
                return false;
            };
            let projected = self.program.normalized_type_identity(projected);
            let result = self.program.normalized_type_identity(row.type_reference);
            if !projected.is_valid() || !result.is_valid() || projected != result {
                return false;
            }
            place.segments.truncate(start);
            if shell.is_some() && !binding_preserved(graph, self.program, &place, receiver, site) {
                return false;
            }
            current = row.backing;
        }
        false
    }
}

fn binding_preserved(
    graph: &FlowControlFacts,
    program: &dyn crate::CheckingInput,
    place: &CanonicalPlace,
    receiver: &FlowOperandFact,
    read: &FlowOperandFact,
) -> bool {
    let anchor = receiver.storage_snapshot;
    let at = read.storage_snapshot;
    if (anchor.is_valid() && !graph.storage_snapshots.is_valid(anchor))
        || (at.is_valid() && !graph.storage_snapshots.is_valid(at))
    {
        return false;
    }
    if anchor == at {
        return true;
    }
    if !anchor.is_valid() {
        return matches!(
            completed_storage::sources_at_snapshot(
                program,
                graph,
                at,
                place,
                StoredObservation::Binding
            ),
            Some(StoredOperand::Incoming(_))
        );
    }
    at.is_valid()
        && completed_storage::contents_preserved_from(
            program,
            graph,
            at,
            place,
            &[anchor],
            StoredObservation::Binding,
        )
}
