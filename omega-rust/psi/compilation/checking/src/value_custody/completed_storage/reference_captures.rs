//! Select the actual descriptor captured inside a completed value. Inline
//! copies keep their own storage cut; crossing a reference switches to the
//! consumer's current cut. This is a value association, never a new loan or a
//! scalar store summary. Authored children keep their earlier operand epochs.
use super::{StoredCapture, StoredObservation, StoredOperand};
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowStorageSnapshot};
use facts::PlaceSegment;

#[derive(Clone)]
enum Selection {
    Unavailable,
    Sources {
        incoming: Vec<crate::CanonicalPlace>,
        values: Vec<StoredCapture>,
    },
}
#[derive(Clone, PartialEq, Eq)]
struct Query {
    value: Handle<FlowOperandFact>,
    path: Vec<PlaceSegment>,
}

/// None denotes an unchanged incoming slot; an explicit unavailable result
/// must not fall through to an older initializer or a declaration's frontier.
pub fn reference_captures_at_path(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    current: Handle<FlowStorageSnapshot>,
) -> Option<StoredOperand> {
    let sources = reference_sources_at_path(program, control, value, path, current);
    // This captures-only projection retains its existing unchanged-ingress
    // contract. Physical source consumers use the lossless frontier below.
    fn incoming_only(source: &StoredOperand) -> bool {
        match source {
            StoredOperand::Incoming(_) => true,
            StoredOperand::Alternatives(sources) => {
                !sources.is_empty() && sources.iter().all(incoming_only)
            }
            StoredOperand::Unavailable | StoredOperand::Value { .. } => false,
        }
    }
    (!incoming_only(&sources)).then_some(sources)
}

/// Resolve the complete descriptor source frontier at this same observation
/// cut. An incoming slot remains explicit even when there is no produced
/// capture; it is neither a pointee address nor new permission. Consumers must
/// authenticate that exact incoming namespace independently.
pub fn reference_sources_at_path(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    current: Handle<FlowStorageSnapshot>,
) -> StoredOperand {
    let mut resolver = Resolver {
        program,
        control,
        current,
        active: Vec::new(),
        memo: Vec::new(),
        active_profiles: Vec::new(),
        profile_memo: Vec::new(),
    };
    let Selection::Sources { incoming, values } = resolver.select(value, path) else {
        return StoredOperand::Unavailable;
    };
    let mut alternatives = incoming
        .into_iter()
        .map(StoredOperand::Incoming)
        .collect::<Vec<_>>();
    alternatives.extend(values.into_iter().map(|value| StoredOperand::Value {
        capture: value.capture,
        reference: value.reference,
        projection: value.projection,
    }));
    match alternatives.len() {
        0 => StoredOperand::Unavailable,
        1 => alternatives.pop().unwrap_or(StoredOperand::Unavailable),
        _ => StoredOperand::Alternatives(alternatives),
    }
}

type ProfileQuery = (
    Handle<checked_trees::FlowOperandRetention>,
    typed_trees::types::TypeReferenceHandle,
    Vec<PlaceSegment>,
);

struct Resolver<'a> {
    program: &'a dyn crate::CheckingInput,
    control: &'a FlowControlFacts,
    current: Handle<FlowStorageSnapshot>,
    active: Vec<Query>,
    memo: Vec<(Query, Selection)>,
    active_profiles: Vec<ProfileQuery>,
    profile_memo: Vec<(ProfileQuery, Option<Selection>)>,
}
impl Resolver<'_> {
    fn select(&mut self, value: Handle<FlowOperandFact>, path: &[PlaceSegment]) -> Selection {
        let query = Query {
            value,
            path: path.to_vec(),
        };
        if let Some((_, result)) = self.memo.iter().find(|(prior, _)| *prior == query) {
            return result.clone();
        }
        if !self.control.operands.is_valid(value) || self.active.contains(&query) {
            return Selection::Unavailable;
        }
        self.active.push(query.clone());
        let result = self.resolve(value, path);
        self.active.pop();
        self.memo.push((query, result.clone()));
        result
    }
    fn resolve(&mut self, value: Handle<FlowOperandFact>, path: &[PlaceSegment]) -> Selection {
        let capture = self.control.operands.get(value);
        if !capture.type_reference.is_valid() {
            return Selection::Unavailable;
        }
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let Some(rows) = self.control.operand_alternatives.span(alternatives) else {
                return Selection::Unavailable;
            };
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                return Selection::Unavailable;
            }
            let values = rows.iter().map(|row| row.value).collect::<Vec<_>>();
            return join(
                values
                    .into_iter()
                    .map(|value| self.select(value, path))
                    .collect(),
            );
        }
        let view = crate::reference_shell(self.program, capture.type_reference).is_some();
        if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } =
            capture.completion
        {
            if !self.control.copied_values.is_valid(copy) {
                return Selection::Unavailable;
            }
            let Some(prefix) = self.control.operand_child_paths.span(prefix) else {
                return Selection::Unavailable;
            };
            let copied = self.control.copied_values.get(copy);
            if !self.control.operands.is_valid(copied.source) {
                return Selection::Unavailable;
            }
            return self
                .reference_leaf(value, path, copied.references, prefix)
                .unwrap_or(Selection::Unavailable);
        }
        if !capture.reference_leaves.is_empty() {
            return self
                .reference_leaf(value, path, capture.reference_leaves, &[])
                .unwrap_or(Selection::Unavailable);
        }
        if view && self.control.operand_retentions.is_valid(capture.retention) {
            let profile = self.control.operand_retentions.get(capture.retention);
            if profile.descriptor.is_valid() || !profile.alternatives.is_empty() {
                return self
                    .profile_descriptor(capture.retention, capture.type_reference, path)
                    .unwrap_or(Selection::Unavailable);
            }
        }
        if view && path.is_empty() {
            return Selection::Sources {
                incoming: Vec::new(),
                values: vec![StoredCapture {
                    capture: value,
                    reference: capture.type_reference,
                    projection: Vec::new(),
                }],
            };
        }
        if !view {
            // Returned descriptors have their own associations; they are
            // not authored child evaluations or snapshots of their pointees.
            if let Some(selected) =
                self.profile_descriptor(capture.retention, capture.type_reference, path)
            {
                return selected;
            }
            if let Some((child, _, length)) = super::captured_operand_child_at_path(
                self.program,
                self.control,
                capture,
                capture.type_reference,
                path,
            ) {
                return self.select(child, &path[length..]);
            }
            if capture.value_source.is_valid() {
                return self.select(capture.value_source, path);
            }
            // A path can cross a stored reference before its final descriptor.
            // Select that pointer first, then observe slots behind it now.
            for length in 1..path.len() {
                if crate::project_type_reference_from_segments(
                    self.program,
                    capture.type_reference,
                    &path[..length],
                )
                .is_some_and(|reference| crate::reference_shell(self.program, reference).is_some())
                {
                    let Selection::Sources {
                        incoming,
                        values: pointers,
                    } = self.select(value, &path[..length])
                    else {
                        return Selection::Unavailable;
                    };
                    if !incoming.is_empty()
                        || pointers
                            .iter()
                            .any(|pointer| !pointer.projection.is_empty())
                    {
                        // A real descriptor subtree may have known lifetime
                        // dependencies without a captured pointee address.
                        return Selection::Unavailable;
                    }
                    let selected = pointers
                        .into_iter()
                        .map(|pointer| self.select(pointer.capture, &path[length..]))
                        .collect();
                    return join(selected);
                }
            }
        }
        let mut places = Vec::new();
        if !capture.referents.is_empty() {
            let Some(referents) = self.control.operand_referents.span(capture.referents) else {
                return Selection::Unavailable;
            };
            for referent in referents {
                let Some(segments) = self
                    .control
                    .operand_referent_segments
                    .span(referent.segments)
                else {
                    return Selection::Unavailable;
                };
                places.push(crate::CanonicalPlace {
                    root: referent.root,
                    segments: segments.to_vec(),
                });
            }
        } else if !view {
            let Some(place) = crate::canonical_place_from_expression_in_state(
                self.program,
                capture.state_symbol,
                capture.statement_index,
                capture.expression,
            ) else {
                return Selection::Unavailable;
            };
            places.push(place);
        }
        if places.is_empty() {
            return Selection::Unavailable;
        }
        let mut selected = Vec::new();
        for place in places {
            let Some(place) = super::project_captured_view_place(place, path) else {
                return Selection::Unavailable;
            };
            let snapshot = if view {
                self.current
            } else {
                capture.storage_snapshot
            };
            selected.push(
                match super::at_snapshot(
                    self.program,
                    self.control,
                    snapshot,
                    &place,
                    StoredObservation::Binding,
                ) {
                    None if view
                        || self
                            .program
                            .state_by_symbol(capture.state_symbol)
                            .is_some_and(|state| {
                                place.root.as_symbol().is_some_and(|symbol| {
                                    self.program
                                        .state_parameter_by_symbol(state, symbol)
                                        .is_some()
                                })
                            }) =>
                    {
                        Selection::Sources {
                            incoming: vec![place],
                            values: Vec::new(),
                        }
                    }
                    None => self.opaque_subtree(value, path),
                    Some(stored) => self.stored(stored, &[]),
                },
            );
        }
        join(selected)
    }
    /// Inline descriptor bytes belong to this completed occurrence. Crossing
    /// that descriptor reads its referent at the consumer's cut, while merely
    /// selecting the descriptor preserves its captured leaf and formation.
    fn reference_leaf(
        &mut self,
        value: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        references: arena::HandleSpan<checked_trees::FlowCapturedReferenceLeaf>,
        prefix: &[PlaceSegment],
    ) -> Option<Selection> {
        let mut whole_path = prefix.to_vec();
        whole_path.extend_from_slice(path);
        let Some(leaves) = self.control.captured_reference_leaves.span(references) else {
            return Some(Selection::Unavailable);
        };
        let mut matched = None;
        for leaf in leaves {
            let Some(leaf_path) = self.control.operand_child_paths.span(leaf.path) else {
                return Some(Selection::Unavailable);
            };
            if leaf_path.len() <= whole_path.len()
                && leaf_path.iter().zip(&whole_path).all(|(left, right)| {
                    crate::storage_path_relations::canonical_place_segments_equal(
                        self.program,
                        *left,
                        *right,
                    )
                })
            {
                if matched.replace((*leaf, leaf_path.to_vec())).is_some() {
                    return Some(Selection::Unavailable);
                }
            }
        }
        let (leaf, leaf_path) = matched?;
        if leaf_path.len() < prefix.len() {
            return Some(Selection::Unavailable);
        }
        let consumed = leaf_path.len() - prefix.len();
        let capture = self.control.operands.get(value);
        let Some(selected) = crate::project_type_reference_from_segments(
            self.program,
            capture.type_reference,
            &path[..consumed],
        ) else {
            return Some(Selection::Unavailable);
        };
        let (Some(actual), Some(declared)) = (
            crate::reference_shell(self.program, selected),
            crate::reference_shell(self.program, leaf.type_reference),
        ) else {
            return Some(Selection::Unavailable);
        };
        if !(actual.access == declared.access
            || declared.access == language_semantics::ReferenceAccess::Mutable)
            || !crate::storage_origin_types_match(self.program, actual.referee, declared.referee)
        {
            return Some(Selection::Unavailable);
        }
        if consumed == path.len() {
            return Some(self.opaque_subtree(value, path));
        }
        // Stop at the next descriptor. Further reference crossings recurse
        // through its actual stored value instead of flattening several hops.
        let end = (consumed + 1..=path.len()).find(|length| {
            crate::project_type_reference_from_segments(
                self.program,
                capture.type_reference,
                &path[..*length],
            )
            .is_some_and(|reference| crate::reference_shell(self.program, reference).is_some())
        });
        let Some(end) = end else {
            return Some(Selection::Unavailable);
        };
        let Some(referents) = self.control.operand_referents.span(leaf.referents) else {
            return Some(Selection::Unavailable);
        };
        if referents.is_empty() {
            return Some(Selection::Unavailable);
        }
        let mut results = Vec::new();
        for referent in referents {
            let Some(segments) = self
                .control
                .operand_referent_segments
                .span(referent.segments)
            else {
                return Some(Selection::Unavailable);
            };
            let Some(place) = super::project_captured_view_place(
                crate::CanonicalPlace {
                    root: referent.root,
                    segments: segments.to_vec(),
                },
                &path[consumed..end],
            ) else {
                return Some(Selection::Unavailable);
            };
            match super::at_snapshot(
                self.program,
                self.control,
                self.current,
                &place,
                StoredObservation::Binding,
            ) {
                None if end == path.len() => results.push(Selection::Sources {
                    incoming: vec![place],
                    values: Vec::new(),
                }),
                None => return Some(Selection::Unavailable),
                Some(stored) => results.push(self.stored(stored, &path[end..])),
            }
        }
        Some(join(results))
    }

    fn profile_descriptor(
        &mut self,
        retention: Handle<checked_trees::FlowOperandRetention>,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<Selection> {
        let query = (retention, reference, path.to_vec());
        if let Some((_, result)) = self.profile_memo.iter().find(|(prior, _)| *prior == query) {
            return result.clone();
        }
        if self.active_profiles.contains(&query) {
            return Some(Selection::Unavailable);
        }
        self.active_profiles.push(query.clone());
        let result = self.resolve_profile_descriptor(retention, reference, path);
        self.active_profiles.pop();
        self.profile_memo.push((query, result.clone()));
        result
    }

    fn resolve_profile_descriptor(
        &mut self,
        retention: Handle<checked_trees::FlowOperandRetention>,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<Selection> {
        if !self.control.operand_retentions.is_valid(retention) {
            return None;
        }
        let profile = *self.control.operand_retentions.get(retention);
        if !profile.alternatives.is_empty() {
            let Some(rows) = self.control.operand_retentions.span(profile.alternatives) else {
                return Some(Selection::Unavailable);
            };
            if rows.is_empty() {
                return Some(Selection::Unavailable);
            }
            let alternatives = profile.alternatives.handles().collect::<Vec<_>>();
            let selected = alternatives
                .into_iter()
                .map(|profile| {
                    self.profile_descriptor(profile, reference, path)
                        .unwrap_or(Selection::Unavailable)
                })
                .collect();
            return Some(join(selected));
        }
        if profile.descriptor.is_valid() {
            return Some(self.select(profile.descriptor, path));
        }
        // An owned profile may link to its completed value. A reference
        // profile instead lends its captured pointee: following that link
        // would substitute the pointee's bytes for the descriptor itself.
        if crate::reference_shell(self.program, reference).is_none()
            && self.control.operands.is_valid(profile.stored_value)
        {
            let prefix = self
                .control
                .operand_retained_paths
                .span_or_empty(profile.stored_projection);
            let Some(selected) = facts::project_captured_view_segments(prefix, path) else {
                return Some(Selection::Unavailable);
            };
            return Some(self.select(profile.stored_value, &selected));
        }
        let mut selected = self
            .control
            .operand_retained_leaves
            .span_or_empty(profile.leaves)
            .iter()
            .filter_map(|leaf| {
                let prefix = self.control.operand_retained_paths.span_or_empty(leaf.path);
                (!prefix.is_empty()
                    && prefix.len() <= path.len()
                    && prefix.iter().zip(path).all(|(left, right)| {
                        crate::storage_path_relations::canonical_place_segments_equal(
                            self.program,
                            *left,
                            *right,
                        )
                    }))
                .then_some((leaf.retention, prefix.len()))
            });
        let (profile, length) = selected.next()?;
        if selected.next().is_some() {
            return Some(Selection::Unavailable);
        }
        let Some(reference) =
            crate::project_type_reference_from_segments(self.program, reference, &path[..length])
        else {
            return Some(Selection::Unavailable);
        };
        self.profile_descriptor(profile, reference, &path[length..])
    }

    /// This completed value owns the typed inline subtree, independently of
    /// whether its producer disclosed that descriptor's address. Keep the
    /// parent occurrence and residual path together; lifetime evidence never
    /// manufactures a new evaluated child or a pointee coordinate. The caller
    /// still owes ordinary case selection and access proofs for the projection.
    fn opaque_subtree(&self, value: Handle<FlowOperandFact>, path: &[PlaceSegment]) -> Selection {
        let capture = self.control.operands.get(value);
        if !crate::project_type_reference_from_segments(self.program, capture.type_reference, path)
            .is_some_and(|reference| crate::reference_shell(self.program, reference).is_some())
        {
            return Selection::Unavailable;
        }
        let Some(profiles) =
            super::captured_retention_profiles_at_path(self.program, self.control, capture, path)
        else {
            return Selection::Unavailable;
        };
        if profiles.is_empty()
            || profiles
                .iter()
                .any(|profile| self.control.retained_roots(*profile).is_none())
        {
            return Selection::Unavailable;
        }
        Selection::Sources {
            incoming: Vec::new(),
            values: vec![StoredCapture {
                capture: value,
                reference: capture.type_reference,
                projection: path.to_vec(),
            }],
        }
    }

    fn stored(&mut self, stored: StoredOperand, suffix: &[PlaceSegment]) -> Selection {
        match stored {
            StoredOperand::Unavailable => Selection::Unavailable,
            StoredOperand::Incoming(place) if suffix.is_empty() => Selection::Sources {
                incoming: vec![place],
                values: Vec::new(),
            },
            // An entry slot is not an address for another pointer crossing.
            StoredOperand::Incoming(_) => Selection::Unavailable,
            StoredOperand::Value {
                capture,
                mut projection,
                ..
            } => {
                projection.extend_from_slice(suffix);
                self.select(capture, &projection)
            }
            StoredOperand::Alternatives(alternatives) => {
                let selections = alternatives
                    .into_iter()
                    .map(|alternative| self.stored(alternative, suffix))
                    .collect();
                join(selections)
            }
        }
    }
}
fn join(selections: Vec<Selection>) -> Selection {
    let mut incoming = Vec::new();
    let mut values: Vec<StoredCapture> = Vec::new();
    for selected in selections {
        let Selection::Sources {
            incoming: entries,
            values: selected,
        } = selected
        else {
            return Selection::Unavailable;
        };
        for entry in entries {
            if !incoming.contains(&entry) {
                incoming.push(entry);
            }
        }
        for value in selected {
            if !values
                .iter()
                .any(|prior| prior.capture == value.capture && prior.projection == value.projection)
            {
                values.push(value);
            }
        }
    }
    if incoming.is_empty() && values.is_empty() {
        Selection::Unavailable
    } else {
        Selection::Sources { incoming, values }
    }
}
