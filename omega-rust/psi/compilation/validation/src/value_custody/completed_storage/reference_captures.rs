//! Select the actual descriptor captured inside a completed value. Inline
//! copies keep their own storage cut; crossing a reference switches to the
//! consumer's current cut. This is a value association, never a new loan or a
//! scalar store summary. Authored children keep their earlier operand epochs.
use super::{StoredCapture, StoredObservation, StoredOperand};
use arena::Handle;
use checked_trees::FlowOperandCompletion;
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
    let mut resolver = Resolver::new(program, control, current);
    stored_selection(resolver.select(value, path))
}

/// Save the descriptor inside an input, rather than the pointer used to reach
/// it. An empty path still crosses a reference input once: `&mut &Cell` lends
/// the stored `&Cell`, while a Referent edge keeps the outer pointer itself.
/// The pointer was captured earlier; its contents are observed at this cut.
pub fn stored_reference_sources_at_path(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    current: Handle<FlowStorageSnapshot>,
) -> StoredOperand {
    if !control.operands.is_valid(value) {
        return StoredOperand::Unavailable;
    }
    let operand = control.operands.get(value);
    let Some(shell) = crate::reference_shell(program, operand.type_reference) else {
        return reference_sources_at_path(program, control, value, path, current);
    };
    if !path.is_empty() {
        return reference_sources_at_path(program, control, value, path, current);
    }
    if crate::reference_shell(program, shell.referee).is_none() {
        return StoredOperand::Unavailable;
    }
    let Some(places) = super::projectable_reference_places(program, control, value, &[]) else {
        return StoredOperand::Unavailable;
    };
    if places.is_empty() {
        return StoredOperand::Unavailable;
    }
    let mut resolver = Resolver::new(program, control, current);
    stored_selection(join(
        places
            .into_iter()
            .map(|place| {
                let place = crate::CanonicalPlace {
                    root: place.root,
                    segments: place.segments,
                };
                match super::at_snapshot(
                    program,
                    control,
                    current,
                    &place,
                    StoredObservation::Binding,
                ) {
                    Some(stored) => resolver.stored(stored, &[]),
                    None => Selection::Sources {
                        incoming: vec![place],
                        values: Vec::new(),
                    },
                }
            })
            .collect(),
    ))
}

/// Follow the saved completed-value association of an owned observation.
/// A reference's stored_value describes its pointee and must never substitute
/// for its descriptor. None means no owned association; an explicitly unknown
/// association remains Unavailable rather than falling through to old storage.
/// Pointer crossings use this observation's immutable capture cut.
pub fn owned_profile_reference_sources(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    path: &[PlaceSegment],
) -> Option<StoredOperand> {
    if crate::reference_shell(program, operand.type_reference).is_some() {
        return None;
    }
    Resolver::new(program, control, operand.storage_snapshot)
        .profile_descriptor(operand.retention, operand.type_reference, path)
        .map(stored_selection)
}

fn stored_selection(selection: Selection) -> StoredOperand {
    let Selection::Sources { incoming, values } = selection else {
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
        slot: value.slot,
    }));
    match alternatives.len() {
        0 => StoredOperand::Unavailable,
        1 => alternatives.pop().unwrap_or(StoredOperand::Unavailable),
        _ => StoredOperand::Alternatives(alternatives),
    }
}

impl<'a> Resolver<'a> {
    fn new(
        program: &'a dyn crate::CheckingInput,
        control: &'a FlowControlFacts,
        current: Handle<FlowStorageSnapshot>,
    ) -> Self {
        Self {
            program,
            control,
            current,
            active: Vec::new(),
            memo: Vec::new(),
            active_profiles: Vec::new(),
            profile_memo: Vec::new(),
        }
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
        if !self.control.operands.is_valid(value) {
            return Selection::Unavailable;
        }
        let Some(path) = crate::corresponding_place_projection(
            self.program,
            self.control.operands.get(value).type_reference,
            path,
        ) else {
            self.trace_selection(
                value,
                path,
                "projection has no applied member correspondence",
            );
            return Selection::Unavailable;
        };
        let path = path.as_slice();
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
        if matches!(result, Selection::Unavailable) {
            self.trace_selection(value, path, "captured descriptor selection unavailable");
        }
        self.active.pop();
        self.memo.push((query, result.clone()));
        result
    }
    fn trace_selection(&self, value: Handle<FlowOperandFact>, path: &[PlaceSegment], reason: &str) {
        if std::env::var_os("OMEGA_STORAGE_QUERY_TRACE").is_none()
            || !self.control.operands.is_valid(value)
        {
            return;
        }
        let capture = self.control.operands.get(value);
        if std::env::var("OMEGA_STORAGE_QUERY_TRACE_STATE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .is_some_and(|state| capture.state_symbol.arena_index() != state)
        {
            return;
        }
        eprintln!(
            "STORAGE_QUERY capture-refused value={value:?} state={:?} statement={} current={:?} type={:?} expression={:?} path={path:?} reason={reason} capture={capture:?}",
            capture.state_symbol,
            capture.statement_index,
            self.current,
            capture.type_reference,
            capture.expression
        );
    }
    fn resolve(&mut self, value: Handle<FlowOperandFact>, path: &[PlaceSegment]) -> Selection {
        let capture = self.control.operands.get(value);
        if !capture.type_reference.is_valid() {
            return Selection::Unavailable;
        }
        if let FlowOperandCompletion::IncomingContents { root, path: prefix } = capture.completion {
            let Some(prefix) = self.control.operand_retained_paths.span(prefix) else {
                return Selection::Unavailable;
            };
            if !crate::project_type_reference_from_segments(
                self.program,
                capture.type_reference,
                path,
            )
            .is_some_and(|selected| crate::reference_shell(self.program, selected).is_some())
            {
                return Selection::Unavailable;
            }
            let Some(place) = super::project_captured_storage_place(
                self.program,
                capture.state_symbol,
                capture.statement_index,
                crate::CanonicalPlace {
                    root,
                    segments: prefix.to_vec(),
                },
                path,
            ) else {
                return Selection::Unavailable;
            };
            // A changed or unavailable slot must remain changed/unavailable;
            // the namespace never revives an older entry value. An unchanged
            // slot remains explicitly incoming for the authority consumer.
            return match super::at_snapshot(
                self.program,
                self.control,
                self.current,
                &place,
                StoredObservation::Binding,
            ) {
                None => Selection::Sources {
                    incoming: vec![place],
                    values: Vec::new(),
                },
                Some(stored) => self.stored(stored, &[]),
            };
        }
        if matches!(
            capture.completion,
            FlowOperandCompletion::InvocationResult { .. }
        ) && path.is_empty()
            && crate::reference_shell(self.program, capture.type_reference).is_some()
        {
            return Selection::Sources {
                incoming: Vec::new(),
                values: vec![StoredCapture {
                    capture: value,
                    reference: capture.type_reference,
                    projection: Vec::new(),
                    slot: None,
                }],
            };
        }
        if let FlowOperandCompletion::Selected(alternatives) = capture.completion {
            // Arm operands can omit the addresses formation already recorded
            // on this view. Those recorded referents are the authenticated
            // set; an empty record still walks the arms and stays unavailable
            // when an arm has no address.
            let arm_values = self
                .control
                .operand_alternatives
                .span(alternatives)
                .and_then(|rows| {
                    if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                        return None;
                    }
                    Some(rows.iter().map(|row| row.value).collect::<Vec<_>>())
                });
            let walked = arm_values.and_then(|values| {
                let joined = join(
                    values
                        .into_iter()
                        .map(|value| self.select(value, path))
                        .collect(),
                );
                match joined {
                    Selection::Unavailable => None,
                    selected => Some(selected),
                }
            });
            if let Some(selected) = walked {
                return selected;
            }
            if path.is_empty()
                && crate::reference_shell(self.program, capture.type_reference).is_some()
                && !capture.referents.is_empty()
            {
                return Selection::Sources {
                    incoming: Vec::new(),
                    values: vec![StoredCapture {
                        capture: value,
                        reference: capture.type_reference,
                        projection: Vec::new(),
                        slot: None,
                    }],
                };
            }
            return Selection::Unavailable;
        }
        let view = crate::reference_shell(self.program, capture.type_reference).is_some();
        if view
            && path.is_empty()
            && let Some(selected) = self.read_descriptor(value)
        {
            return selected;
        }
        if let FlowOperandCompletion::Copied { copy, path: prefix } = capture.completion {
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
                .reference_leaf(
                    value,
                    path,
                    copied.references,
                    prefix,
                    self.control.operands.get(copied.source).type_reference,
                )
                .unwrap_or(Selection::Unavailable);
        }
        if !capture.reference_leaves.is_empty() {
            return self
                .reference_leaf(
                    value,
                    path,
                    capture.reference_leaves,
                    &[],
                    capture.type_reference,
                )
                .unwrap_or(Selection::Unavailable);
        }
        if view && self.control.operand_retentions.is_valid(capture.retention) {
            let profile = self.control.operand_retentions.get(capture.retention);
            if profile.descriptor.is_valid() || !profile.alternatives.is_empty() {
                match self.profile_descriptor(capture.retention, capture.type_reference, path) {
                    Some(selected) => return selected,
                    None if path.is_empty() => return Selection::Unavailable,
                    // No descriptor association is not an unavailable current
                    // slot. A nonempty path crosses this pointer: its exact
                    // referents below select the slot at the consumer's cut,
                    // rather than replaying captured pointee contents from a
                    // lifetime profile. Explicit unknown alternatives above
                    // still return Unavailable and cannot take this route.
                    None => {}
                }
            }
        }
        if view && path.is_empty() {
            return Selection::Sources {
                incoming: Vec::new(),
                values: vec![StoredCapture {
                    capture: value,
                    reference: capture.type_reference,
                    projection: Vec::new(),
                    slot: None,
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
            let Some(place) = super::project_captured_storage_place(
                self.program,
                capture.state_symbol,
                capture.statement_index,
                place,
                path,
            ) else {
                self.trace_selection(value, path, "physical receiver projection unavailable");
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
    /// Read the descriptor that was actually loaded, not the pointer's later
    /// home or its pointee. The access row records the evaluated slot; its
    /// capture snapshot fixes the binding even if a later argument replaces it.
    /// Pointee crossings keep this resolver's separate current-content cut.
    fn read_descriptor(&mut self, value: Handle<FlowOperandFact>) -> Option<Selection> {
        let capture = *self.control.operands.get(value);
        if !self.control.case_accesses.is_valid(capture.case_access) {
            return None;
        }
        let access = self.control.case_accesses.get(capture.case_access);
        if access.state_symbol != capture.state_symbol
            || access.statement_index != capture.statement_index
            || access.expression != capture.expression
        {
            return Some(Selection::Unavailable);
        }
        let Some(state) = self.program.state_by_symbol(capture.state_symbol) else {
            return Some(Selection::Unavailable);
        };
        if self
            .program
            .machine_by_own_or_state_symbol(capture.state_symbol)
            .map(|machine| machine.symbol)
            != Some(access.machine_symbol)
        {
            return Some(Selection::Unavailable);
        }
        let Some(places) = self.control.operand_referents.span(access.places) else {
            return Some(Selection::Unavailable);
        };
        if places.is_empty() {
            return None;
        }
        let expected = crate::reference_shell(self.program, capture.type_reference)?;
        let mut slots = Vec::new();
        for place in places {
            let root = place.root.as_symbol()?;
            let segments = self
                .control
                .operand_referent_segments
                .span(place.segments)?;
            let declared = crate::reference_result_custody::storage_owner_type_reference(
                self.program,
                state,
                root,
                u32::try_from(capture.statement_index).ok()?,
            )?;
            let slot =
                crate::project_type_reference_from_segments(self.program, declared, segments)?;
            let supplied = crate::reference_shell(self.program, slot)?;
            // A borrow of a pointer and a read of that pointer have different
            // types. Only a matching descriptor slot supplies this value.
            if !(supplied.access == expected.access
                || supplied.access == language_semantics::ReferenceAccess::Mutable)
                || !crate::storage_origin_types_match(
                    self.program,
                    supplied.referee,
                    expected.referee,
                )
            {
                return None;
            }
            slots.push(crate::CanonicalPlace {
                root: place.root,
                segments: segments.to_vec(),
            });
        }
        let captured_pointer = |place| Selection::Sources {
            incoming: Vec::new(),
            values: vec![StoredCapture {
                capture: value,
                reference: capture.type_reference,
                projection: Vec::new(),
                slot: Some(place),
            }],
        };
        let mut selected = Vec::new();
        for place in slots {
            // An exclusive reference formal lends its referent, not a pointer
            // home. Its mutability flag permits writes through that pointer.
            // Immutable shared formals also keep their entry value; mutable
            // shared bindings and stored fields still query capture history.
            if place.segments.is_empty()
                && place.root.as_symbol().is_some_and(|root| {
                    self.program
                        .state_parameter_by_symbol(state, root)
                        .is_some_and(|parameter| {
                            !parameter.is_mutable
                                || crate::reference_shell(self.program, parameter.type_reference)
                                    .is_some_and(|shell| shell.access.is_exclusive())
                        })
                })
            {
                selected.push(captured_pointer(place));
                continue;
            }
            match super::at_snapshot(
                self.program,
                self.control,
                capture.storage_snapshot,
                &place,
                StoredObservation::Binding,
            ) {
                Some(stored) => selected.push(self.stored(stored, &[])),
                None if place.root.as_symbol().is_some_and(|root| {
                    self.program
                        .state_parameter_by_symbol(state, root)
                        .is_some()
                }) =>
                {
                    // This operand is an actual completed read of the entry
                    // pointer. Keep its value capture and slot; an unread
                    // incoming carrier remains Incoming in the ordinary
                    // source-frontier query above.
                    selected.push(captured_pointer(place));
                }
                None => return Some(Selection::Unavailable),
            }
        }
        Some(join(selected))
    }

    fn reference_leaf(
        &mut self,
        value: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        references: arena::HandleSpan<checked_trees::FlowCapturedReferenceLeaf>,
        prefix: &[PlaceSegment],
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<Selection> {
        let mut whole_path = prefix.to_vec();
        whole_path.extend_from_slice(path);
        let Some(whole_path) =
            crate::corresponding_place_projection(self.program, reference, &whole_path)
        else {
            return Some(Selection::Unavailable);
        };
        let Some(leaves) = self.control.captured_reference_leaves.span(references) else {
            return Some(Selection::Unavailable);
        };
        let mut matched = None;
        for leaf in leaves {
            let Some(leaf_path) =
                self.control
                    .operand_child_paths
                    .span(leaf.path)
                    .and_then(|path| {
                        crate::corresponding_place_projection(self.program, reference, path)
                    })
            else {
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
                && matched.replace((*leaf, leaf_path)).is_some()
            {
                return Some(Selection::Unavailable);
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
            let Some(place) = super::project_captured_storage_place(
                self.program,
                capture.state_symbol,
                capture.statement_index,
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
        let Some(path) = crate::corresponding_place_projection(self.program, reference, path)
        else {
            return Some(Selection::Unavailable);
        };
        let path = path.as_slice();
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
                .map(|profile| self.profile_descriptor(profile, reference, path))
                .collect::<Vec<_>>();
            // A roots-only alternative keeps lifetime debt without recording
            // a path association. When no alternative resolves one, this
            // profile does not decide the slot; other operand evidence may
            // still resolve the incoming place uniformly.
            if selected.iter().all(Option::is_none) {
                return None;
            }
            return Some(join(
                selected
                    .into_iter()
                    .map(|selected| selected.unwrap_or(Selection::Unavailable))
                    .collect(),
            ));
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
                let prefix = crate::corresponding_place_projection(
                    self.program,
                    reference,
                    self.control.operand_retained_paths.span_or_empty(leaf.path),
                )?;
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
                slot: None,
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
                slot,
                ..
            } => {
                projection.extend_from_slice(suffix);
                let selected = self.select(capture, &projection);
                if matches!(selected, Selection::Unavailable) {
                    return selected;
                }
                // Keep the completed installation owner for an inline
                // descriptor. Flattening it to constructor children or match
                // arms loses the current descriptor's formation identity.
                // A pointer crossing is different: its stored leaves belong
                // to the consumer's current cut and keep the selected sources.
                let reference = self.control.operands.get(capture).type_reference;
                if crate::owned_projection_type_reference(self.program, reference, &projection)
                    .is_some_and(|selected| {
                        crate::reference_shell(self.program, selected).is_some()
                    })
                {
                    Selection::Sources {
                        incoming: Vec::new(),
                        values: vec![StoredCapture {
                            capture,
                            reference,
                            projection,
                            slot,
                        }],
                    }
                } else {
                    selected
                }
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
            if !values.iter().any(|prior| {
                prior.capture == value.capture
                    && prior.projection == value.projection
                    && prior.slot == value.slot
            }) {
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
