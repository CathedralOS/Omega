//! Storage validity read from the program's one storage graph.
//!
//! A recorded storage term names the storage versions it was established at.
//! It still describes the current contents at a later version exactly when
//! every path from those versions preserves the place's contents
//! (`fact_validity.md`: a read may equal an exact reaching stored value, and
//! every predecessor must supply it). A term established at incoming storage
//! holds while the place still reads that incoming storage. Versions are per
//! state: a state's first statement starts at its incoming storage, which the
//! invalid snapshot denotes.

use super::super::StructuralJudge;
use super::{CapturedStorageCoordinate, CapturedStorageSegment, CapturedStorageValue};
use crate::CanonicalPlace;
use crate::completed_storage::versions;
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowStateFact, FlowStorageSnapshot};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;

#[cfg(test)]
mod tests;

/// One position in the storage graph: a state's history and the version after
/// the last effect observed there. The invalid snapshot is incoming storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::proof_contracts::contract_entailment) struct StorageVersion {
    pub(in crate::proof_contracts::contract_entailment) state: SymbolHandle,
    pub(in crate::proof_contracts::contract_entailment) snapshot: Handle<FlowStorageSnapshot>,
}

impl StorageVersion {
    pub(in crate::proof_contracts::contract_entailment) fn incoming(state: SymbolHandle) -> Self {
        Self {
            state,
            snapshot: Handle::invalid(),
        }
    }
}

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn storage_graph(
        &self,
    ) -> Option<&'program FlowControlFacts> {
        self.program.storage_graph()
    }

    fn flow_state(&self, state: SymbolHandle) -> Option<&'program FlowStateFact> {
        versions::state_row(self.program, self.storage_graph()?, state)
    }

    /// Incoming storage of `state`, for a runtime body whose state the graph
    /// records. Proof-only bodies and graph-less programs have no version.
    pub(in crate::proof_contracts::contract_entailment) fn incoming_storage_version(
        &self,
        state: &State,
    ) -> Option<StorageVersion> {
        (self.requires_value_preservation() && self.flow_state(state.symbol).is_some())
            .then(|| StorageVersion::incoming(state.symbol))
    }

    /// The version after statement `statement_index` of `state` completes.
    pub(in crate::proof_contracts::contract_entailment) fn statement_storage_version(
        &self,
        state: &State,
        statement_index: usize,
    ) -> Option<StorageVersion> {
        let snapshot = versions::statement_snapshot(
            self.program,
            self.storage_graph()?,
            state.symbol,
            statement_index,
        )?;
        Some(StorageVersion {
            state: state.symbol,
            snapshot,
        })
    }

    /// The version after `statement` of `state` completes, found by identity.
    pub(in crate::proof_contracts::contract_entailment) fn statement_node_storage_version(
        &self,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
    ) -> Option<StorageVersion> {
        let index = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .position(|candidate| std::ptr::eq(candidate, statement))?;
        self.statement_storage_version(state, index)
    }

    /// The version after the call at `expression` returns normally.
    pub(in crate::proof_contracts::contract_entailment) fn call_storage_version(
        &self,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<StorageVersion> {
        let call = versions::unique_call(
            self.program,
            self.storage_graph()?,
            state.symbol,
            expression,
        )?;
        Some(StorageVersion {
            state: state.symbol,
            snapshot: call.completed_storage,
        })
    }

    /// A named terminal call has no expression handle. Its actual source
    /// occurrence selects the completed call cut; another arm's call or the
    /// pre-call statement snapshot cannot describe this normal return.
    pub(in crate::proof_contracts::contract_entailment) fn transition_call_storage_version(
        &self,
        machine: &typed_trees::machine::Machine,
        state: &State,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Option<StorageVersion> {
        let graph = self.storage_graph()?;
        let row = self.flow_state(state.symbol)?;
        let mut calls = graph.calls.span(row.calls)?.iter().filter(|call| {
            crate::find_call_site(
                self.program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            )
            .is_some_and(|site| site.completes_transition(self.program, target))
        });
        let call = calls.next()?;
        calls.next().is_none().then_some(StorageVersion {
            state: state.symbol,
            snapshot: call.completed_storage,
        })
    }

    /// Preserve a declared inline referent across one exact completed call.
    /// A reinterpreted view names another address space; preservation of its
    /// owner descriptor cannot stand in for preservation of that view's cells.
    pub(in crate::proof_contracts::contract_entailment) fn call_preserves_inline_subject(
        &self,
        state: &State,
        expression: ExpressionHandle,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
    ) -> bool {
        let [source] = subject.sources.as_slice() else {
            return false;
        };
        if !source.recasts.is_empty() || source.representation.is_some() {
            return false;
        }
        let Some(coordinate) = self
            .captured_reference_subject(subject, state)
            .and_then(|coordinate| Self::coordinate_place(&coordinate))
        else {
            return false;
        };
        let Some(graph) = self.storage_graph() else {
            return false;
        };
        versions::unique_call(self.program, graph, state.symbol, expression).is_some_and(|call| {
            crate::completed_storage::contents_preserved_from(
                self.program,
                graph,
                call.completed_storage,
                &coordinate,
                &[call.invocation_storage],
                crate::completed_storage::StoredObservation::InlineContents,
            )
        })
    }

    /// The version a read observed: its own completed operand's snapshot.
    pub(in crate::proof_contracts::contract_entailment) fn read_storage_version(
        &self,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<StorageVersion> {
        let snapshot = versions::operand_snapshot(self.storage_graph()?, state.symbol, expression)?;
        Some(StorageVersion {
            state: state.symbol,
            snapshot,
        })
    }

    /// The coordinate's graph place up to its first selector captured as a value.
    fn coordinate_prefix_place(coordinate: &CapturedStorageCoordinate) -> CanonicalPlace {
        let mut segments = Vec::new();
        for segment in &coordinate.segments {
            match segment {
                CapturedStorageSegment::Place(segment) => segments.push(*segment),
                CapturedStorageSegment::Index(super::super::StructuralTerm::Integer(index)) => {
                    match index.to_i64().and_then(|index| usize::try_from(index).ok()) {
                        Some(index) => segments.push(PlaceSegment::FixedIndex { index }),
                        None => break,
                    }
                }
                CapturedStorageSegment::Index(_) => break,
            }
        }
        CanonicalPlace {
            root: coordinate.root,
            segments,
        }
    }

    /// The graph place a coordinate names. A selector captured as a value
    /// other than a known index has no graph place; such a term is only valid
    /// at the version it was established at.
    pub(super) fn coordinate_place(
        coordinate: &CapturedStorageCoordinate,
    ) -> Option<CanonicalPlace> {
        let segments = coordinate
            .segments
            .iter()
            .map(|segment| match segment {
                CapturedStorageSegment::Place(segment) => Some(*segment),
                CapturedStorageSegment::Index(super::super::StructuralTerm::Integer(index)) => {
                    usize::try_from(index.to_i64()?)
                        .ok()
                        .map(|index| PlaceSegment::FixedIndex { index })
                }
                CapturedStorageSegment::Index(_) => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(CanonicalPlace {
            root: coordinate.root,
            segments,
        })
    }

    /// Move to `version` when the effect site supplies one. Terms returned by a
    /// call being completed are anchored at its post-call version.
    pub(in crate::proof_contracts::contract_entailment) fn advance_storage_version(
        &mut self,
        version: Option<StorageVersion>,
    ) {
        let Some(version) = version else {
            return;
        };
        let previous = self.storage_version.replace(version);
        if let Some(previous) = previous
            && previous.state == version.state
            && previous.snapshot != version.snapshot
        {
            self.rejoin_followed_path(previous, version);
        }
        if self
            .stored_values
            .iter()
            .any(|stored| stored.current_anchors.contains(&StorageAnchor::CallReturn))
        {
            for stored in std::sync::Arc::make_mut(&mut self.stored_values) {
                if stored.current_anchors.contains(&StorageAnchor::CallReturn) {
                    for anchor in &mut stored.current_anchors {
                        if *anchor == StorageAnchor::CallReturn {
                            *anchor = StorageAnchor::After(version.snapshot);
                        }
                    }
                    stored.anchor_state = version.state;
                }
            }
        }
    }

    /// A judge follows one path. When it advances across a join, the other
    /// predecessors are paths it did not take: a value current at `from` stays
    /// current at `to` when no event on the followed path writes its place.
    fn rejoin_followed_path(&mut self, from: StorageVersion, to: StorageVersion) {
        let Some(graph) = self.storage_graph() else {
            return;
        };
        let Some(path) = versions::followed_path(graph, from.snapshot, to.snapshot) else {
            return;
        };
        let retained = self
            .stored_values
            .iter()
            .map(|stored| {
                stored.current.is_some()
                    && self.current_value_valid(stored, &stored.coordinate, from)
                    && Self::coordinate_place(&stored.coordinate)
                        .or_else(|| Some(Self::coordinate_prefix_place(&stored.coordinate)))
                        .is_some_and(|place| {
                            path.iter().all(|(event, before)| {
                                crate::completed_storage::contents_preserved_from(
                                    self.program,
                                    graph,
                                    *event,
                                    &place,
                                    &[*before],
                                    crate::completed_storage::StoredObservation::Contents,
                                )
                            })
                        })
            })
            .collect::<Vec<_>>();
        for (stored, retained) in std::sync::Arc::make_mut(&mut self.stored_values)
            .iter_mut()
            .zip(retained)
        {
            if retained {
                stored.current_anchors = vec![StorageAnchor::After(to.snapshot)];
                stored.anchor_state = to.state;
            }
        }
    }

    /// Whether this judge reads storage validity from the graph.
    pub(in crate::proof_contracts::contract_entailment) fn graph_storage_mode(&self) -> bool {
        self.storage_version.is_some()
    }

    /// Whether incoming storage of `place`'s root no longer reaches `place`
    /// at this judge's version. Immutable scalar bindings have no writer.
    pub(super) fn graph_place_invalidated(&self, place: &CanonicalPlace) -> bool {
        let (Some(graph), Some(at)) = (self.storage_graph(), self.storage_version) else {
            return false;
        };
        if self.immutable_scalar_place(place) {
            return false;
        }
        if !at.snapshot.is_valid() {
            return false;
        }
        // A local's own initializer establishes it; only a later store or an
        // unknown write invalidates what was observed of it.
        let initializer = place
            .root
            .as_symbol()
            .and_then(|symbol| self.local_initializer(at.state, symbol));
        let invalidates = |reaching: &crate::completed_storage::StoredOperand| match reaching {
            crate::completed_storage::StoredOperand::Incoming(_) => false,
            crate::completed_storage::StoredOperand::Value { capture, .. } => {
                initializer != Some(graph.operands.get(*capture).expression)
            }
            _ => true,
        };
        match crate::completed_storage::at_snapshot(
            self.program,
            graph,
            at.snapshot,
            place,
            crate::completed_storage::StoredObservation::Contents,
        ) {
            None => false,
            Some(crate::completed_storage::StoredOperand::Alternatives(alternatives)) => {
                alternatives.iter().any(invalidates)
            }
            Some(reaching) => invalidates(&reaching),
        }
    }

    /// The initializer of a local declared in `state`, if `symbol` is one.
    fn local_initializer(
        &self,
        state: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Option<ExpressionHandle> {
        let (_, state) = crate::find_state(self.program, state)?;
        self.program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| statement.as_local_data())
            .find(|local| local.symbol == symbol)
            .map(|local| local.initial_value)
    }

    pub(super) fn graph_observation_invalidated(&self, expression: ExpressionHandle) -> bool {
        let Some(place) = crate::canonical_place_from_expression(self.program, expression) else {
            return false;
        };
        self.graph_place_invalidated(&place)
    }

    pub(super) fn graph_binding_invalidated(&self, symbol: SymbolHandle) -> bool {
        self.graph_place_invalidated(&CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        })
    }

    /// The anchor a value captured by a completed read receives: the store
    /// or incoming storage that read reached, or the read's own version.
    pub(super) fn read_anchor(
        &self,
        state: &State,
        expression: ExpressionHandle,
        coordinate: &CapturedStorageCoordinate,
    ) -> Option<StorageAnchor> {
        let graph = self.storage_graph()?;
        let version = self.read_storage_version(state, expression)?;
        let Some(place) = Self::coordinate_place(coordinate) else {
            return Some(StorageAnchor::After(version.snapshot));
        };
        if !version.snapshot.is_valid() {
            return Some(StorageAnchor::Incoming);
        }
        Some(
            match crate::completed_storage::at_snapshot(
                self.program,
                graph,
                version.snapshot,
                &place,
                crate::completed_storage::StoredObservation::Contents,
            ) {
                None | Some(crate::completed_storage::StoredOperand::Incoming(_)) => {
                    StorageAnchor::Incoming
                }
                Some(crate::completed_storage::StoredOperand::Value { capture, .. }) => {
                    StorageAnchor::Store(graph.operands.get(capture).expression)
                }
                _ => StorageAnchor::After(version.snapshot),
            },
        )
    }

    /// Whether `stored`'s current term still describes `coordinate`, a place at
    /// or below its own, at `at`: the place's reaching value there is a store
    /// or incoming storage the term was established from, or its contents are
    /// preserved from an anchored version. A write to one field of an
    /// aggregate retires only the values read through that field.
    pub(super) fn current_value_valid(
        &self,
        stored: &CapturedStorageValue,
        coordinate: &CapturedStorageCoordinate,
        at: StorageVersion,
    ) -> bool {
        let Some(graph) = self.storage_graph() else {
            return true;
        };
        // A declared owned storage path contains inline cells up to its first
        // reference boundary. Use the path's type, not an ObservedValue wrapper:
        // isolated values need no wrapper. A separately backed content write
        // invalidates referent observations, not the owner's inline cells.
        let inline = stored
            .coordinate
            .root
            .as_symbol()
            .and_then(|root| crate::declaration_type_reference(self.program, root))
            .and_then(|reference| {
                stored
                    .coordinate
                    .segments
                    .iter()
                    .try_fold(reference, |reference, segment| {
                        crate::reference_shell(self.program, reference)
                            .is_none()
                            .then(|| segment.projected_type(self.program, reference))
                            .flatten()
                    })
            })
            .is_some_and(|reference| crate::reference_shell(self.program, reference).is_none());
        // A saved shared scalar keeps its contents while that same pointer
        // binding survives: its live loan forbids changing the scalar. This
        // applies only at the saved copy's own home, never at its former source
        // path, which may now lead through a replacement pointer.
        let shared_scalar = stored.coordinate == *coordinate
            && stored
                .current
                .as_ref()
                .and_then(|value| {
                    super::super::super::structural_terms::shared_inline_observation_type(
                        self.program,
                        value,
                    )
                })
                .and_then(|reference| crate::reference_shell(self.program, reference))
                .is_some_and(|shell| {
                    self.program
                        .primitive_type_reference(shell.referee)
                        .is_some()
                });
        let observation = if shared_scalar {
            crate::completed_storage::StoredObservation::Binding
        } else if inline {
            crate::completed_storage::StoredObservation::InlineContents
        } else {
            crate::completed_storage::StoredObservation::Contents
        };
        if stored.current_anchors.is_empty() || stored.anchor_state != at.state {
            return false;
        }
        // An immutable binding's owned scalar fields have no write authority.
        // This never crosses a stored reference or treats an atomic as stable.
        if Self::coordinate_place(coordinate)
            .is_some_and(|place| self.immutable_scalar_place(&place))
        {
            return true;
        }
        let Some(place) = Self::coordinate_place(coordinate) else {
            // A selector captured as a value: the element is unchanged when no
            // write touched the place up to that selector since the anchor.
            let prefix = Self::coordinate_prefix_place(coordinate);
            return stored.current_anchors.iter().all(|anchor| match anchor {
                StorageAnchor::Incoming => {
                    !at.snapshot.is_valid()
                        || matches!(
                            crate::completed_storage::at_snapshot(
                                self.program,
                                graph,
                                at.snapshot,
                                &prefix,
                                observation,
                            ),
                            None | Some(crate::completed_storage::StoredOperand::Incoming(_))
                        )
                }
                StorageAnchor::After(cut) => {
                    *cut == at.snapshot
                        || (cut.is_valid()
                            && at.snapshot.is_valid()
                            && crate::completed_storage::contents_preserved_from(
                                self.program,
                                graph,
                                at.snapshot,
                                &prefix,
                                &[*cut],
                                observation,
                            ))
                }
                StorageAnchor::Store(_) | StorageAnchor::CallReturn => false,
            });
        };
        let preserved = |cut: Handle<FlowStorageSnapshot>| {
            cut == at.snapshot
                || (cut.is_valid()
                    && at.snapshot.is_valid()
                    && crate::completed_storage::contents_preserved_from(
                        self.program,
                        graph,
                        at.snapshot,
                        &place,
                        &[cut],
                        observation,
                    ))
        };
        let reached = |reaching: &crate::completed_storage::StoredOperand| match reaching {
            crate::completed_storage::StoredOperand::Incoming(_) => {
                stored.current_anchors.iter().any(|anchor| match anchor {
                    StorageAnchor::Incoming => true,
                    // A followed branch can carry unchanged entry contents
                    // across a join under a newer anchor. They still describe
                    // this place when no later write has changed it.
                    StorageAnchor::After(cut) => preserved(*cut),
                    StorageAnchor::Store(_) | StorageAnchor::CallReturn => false,
                })
            }
            crate::completed_storage::StoredOperand::Value { capture, .. } => {
                let expression = graph.operands.get(*capture).expression;
                stored.current_anchors.iter().any(|anchor| match anchor {
                    StorageAnchor::Store(stored_expression) => *stored_expression == expression,
                    StorageAnchor::After(cut) => preserved(*cut),
                    StorageAnchor::Incoming | StorageAnchor::CallReturn => false,
                })
            }
            _ => false,
        };
        if !at.snapshot.is_valid() {
            return stored
                .current_anchors
                .iter()
                .any(|anchor| matches!(anchor, StorageAnchor::Incoming));
        }
        let reaching = crate::completed_storage::at_snapshot(
            self.program,
            graph,
            at.snapshot,
            &place,
            observation,
        );
        if super::super::super::struct_trace()
            && stored
                .current_anchors
                .iter()
                .any(|anchor| matches!(anchor, StorageAnchor::After(_)))
        {
            let reaching_kind = match &reaching {
                None | Some(crate::completed_storage::StoredOperand::Incoming(_)) => "incoming",
                Some(crate::completed_storage::StoredOperand::Value { .. }) => "stored value",
                Some(crate::completed_storage::StoredOperand::Alternatives(_)) => "alternatives",
                Some(crate::completed_storage::StoredOperand::Unavailable) => "unavailable",
            };
            eprintln!(
                "STRUCT anchored-storage-value owner={} stored={:?} requested={coordinate:?} anchors={:?} at={at:?} reaching={reaching_kind}",
                self.program.symbols.display_path(self.machine_symbol, "::"),
                stored.coordinate,
                stored.current_anchors,
            );
        }
        let valid = match reaching {
            None => reached(&crate::completed_storage::StoredOperand::Incoming(
                place.clone(),
            )),
            Some(crate::completed_storage::StoredOperand::Alternatives(alternatives)) => {
                alternatives.iter().all(reached)
            }
            Some(crate::completed_storage::StoredOperand::Unavailable) => stored
                .current_anchors
                .iter()
                .any(|anchor| matches!(anchor, StorageAnchor::After(cut) if preserved(*cut))),
            Some(reaching) => reached(&reaching),
        };
        valid
    }
}

/// Where a recorded storage term was established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) enum StorageAnchor {
    /// The state's incoming storage.
    Incoming,
    /// The completed store whose stored value is this expression's operand.
    Store(ExpressionHandle),
    /// Storage as it stands at this version, until a later write retires it.
    After(Handle<FlowStorageSnapshot>),
    /// The version after the call this judge is completing, resolved when the
    /// call's own normal-return writes have been observed.
    CallReturn,
}
