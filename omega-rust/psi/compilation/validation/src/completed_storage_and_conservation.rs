/*
Completed storage associations (`completed_storage`) and content conservation
and projection plans (`content_conservation` and `content_projections`).
*/
pub mod completed_storage {
    use arena::Handle;
    use checked_trees::FlowOperandFact;
    pub(crate) mod query {
        use super::{StoredObservation, StoredOperand};
        use crate::CanonicalPlace;
        use arena::Handle;
        use checked_trees::{FlowControlFacts, FlowStorageOperation, FlowStorageSnapshot};
        use std::collections::BTreeMap;

        pub(crate) type Snapshot = Handle<FlowStorageSnapshot>;

        #[derive(Clone)]
        pub(crate) enum Resolution {
            Incoming,
            Unavailable,
            Sources {
                incoming: bool,
                stores: Vec<Snapshot>,
            },
        }

        pub(crate) enum Annotation {
            Active,
            Complete(Resolution),
        }

        fn identity(snapshot: Snapshot) -> (u32, u32) {
            (snapshot.generation(), snapshot.arena_index())
        }

        fn resolved(
            annotations: &BTreeMap<(u32, u32), Annotation>,
            snapshot: Snapshot,
        ) -> Resolution {
            if !snapshot.is_valid() {
                return Resolution::Incoming;
            }
            match annotations.get(&identity(snapshot)) {
                Some(Annotation::Complete(value)) => value.clone(),
                _ => Resolution::Unavailable,
            }
        }

        fn matches_store(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            source: &CanonicalPlace,
            destination: checked_trees::FlowOperandReferent,
        ) -> Option<bool> {
            let path = control
                .operand_referent_segments
                .span(destination.segments)?;
            Some(
                destination.root == source.root
                    && path.len() <= source.segments.len()
                    && path.iter().zip(&source.segments).all(|(left, right)| {
                        crate::storage_path_relations::canonical_place_segments_equal(
                            program, *left, *right,
                        )
                    }),
            )
        }

        fn preserved_binding_matches(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            source: &CanonicalPlace,
            destination: checked_trees::FlowOperandReferent,
            observation: StoredObservation,
        ) -> Option<bool> {
            if !matches!(observation, StoredObservation::Binding) {
                return Some(false);
            }
            let path = control
                .operand_referent_segments
                .span(destination.segments)?;
            Some(
                path.len() == source.segments.len()
                    && matches_store(program, control, source, destination)?,
            )
        }

        pub(crate) enum SourceBoundary<'query> {
            Stores,
            ReadCuts(&'query std::collections::BTreeSet<(u32, u32)>),
        }

        pub fn contents_preserved_from(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            cuts: &[Snapshot],
            observation: StoredObservation,
        ) -> bool {
            if cuts.is_empty()
                || cuts
                    .iter()
                    .any(|cut| !control.storage_snapshots.is_valid(*cut))
            {
                return false;
            }
            let cuts = cuts.iter().map(|cut| identity(*cut)).collect();
            matches!(resolve_snapshot(program, control, snapshot, source, observation, SourceBoundary::ReadCuts(&cuts)),
        Some(Resolution::Sources { incoming: false, stores }) if !stores.is_empty())
        }

        fn resolve_snapshot(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            observation: StoredObservation,
            boundary: SourceBoundary<'_>,
        ) -> Option<Resolution> {
            if !snapshot.is_valid() {
                return None;
            }
            let Some(mut remaining) = control
                .storage_snapshots
                .len()
                .checked_mul(3)
                .and_then(|nodes| {
                    control
                        .storage_predecessors
                        .len()
                        .checked_mul(2)
                        .and_then(|edges| nodes.checked_add(edges))
                })
                .and_then(|work| work.checked_add(1))
            else {
                return Some(Resolution::Unavailable);
            };
            if let Some(result) =
                resolve_chain(program, control, snapshot, source, observation, &boundary)
            {
                return result;
            }
            let mut annotations = BTreeMap::new();
            let mut pending = vec![(snapshot, false)];
            while let Some((current, complete)) = pending.pop() {
                let Some(next) = remaining.checked_sub(1) else {
                    return Some(Resolution::Unavailable);
                };
                remaining = next;
                if !current.is_valid() {
                    continue;
                }
                if !control.storage_snapshots.is_valid(current) {
                    return Some(Resolution::Unavailable);
                }
                let event = control.storage_snapshots.get(current);
                let key = identity(current);
                if let SourceBoundary::ReadCuts(cuts) = boundary
                    && cuts.contains(&key)
                {
                    annotations.insert(
                        key,
                        Annotation::Complete(Resolution::Sources {
                            incoming: false,
                            stores: vec![current],
                        }),
                    );
                    continue;
                }
                if complete {
                    let result = match event.operation {
                        FlowStorageOperation::Join { predecessors } => {
                            let Some(predecessors) =
                                control.storage_predecessors.span(predecessors)
                            else {
                                return Some(Resolution::Unavailable);
                            };
                            let mut stores = Vec::new();
                            let mut incoming = false;
                            let mut unavailable = predecessors.is_empty();
                            for predecessor in predecessors {
                                match resolved(&annotations, *predecessor) {
                                    Resolution::Incoming => incoming = true,
                                    Resolution::Unavailable => unavailable = true,
                                    Resolution::Sources {
                                        incoming: entry,
                                        stores: values,
                                    } => {
                                        incoming |= entry;
                                        stores.extend(values);
                                    }
                                }
                            }
                            stores.sort_unstable_by_key(|handle| identity(*handle));
                            stores.dedup();
                            if unavailable
                                || (incoming
                                    && !stores.is_empty()
                                    && !matches!(observation, StoredObservation::Binding))
                            {
                                Resolution::Unavailable
                            } else if stores.is_empty() {
                                Resolution::Incoming
                            } else {
                                Resolution::Sources { incoming, stores }
                            }
                        }
                        FlowStorageOperation::PreserveBinding {
                            destination,
                            source: entry,
                        } if preserved_binding_matches(
                            program,
                            control,
                            source,
                            destination,
                            observation,
                        ) == Some(true) =>
                        {
                            resolved(&annotations, entry)
                        }
                        _ => resolved(&annotations, event.previous),
                    };
                    annotations.insert(key, Annotation::Complete(result));
                    continue;
                }
                match annotations.get(&key) {
                    Some(Annotation::Complete(_)) => continue,
                    Some(Annotation::Active) => return Some(Resolution::Unavailable),
                    None => {}
                }
                let terminal = terminal_resolution(
                    program,
                    control,
                    current,
                    &event.operation,
                    source,
                    observation,
                    &boundary,
                )?;
                if let Some(result) = terminal {
                    if matches!(result, Resolution::Unavailable) {
                        trace_query_refusal(control, current, source, observation);
                    }
                    annotations.insert(key, Annotation::Complete(result));
                } else {
                    annotations.insert(key, Annotation::Active);
                    pending.push((current, true));
                    if let FlowStorageOperation::Join { predecessors } = event.operation {
                        let Some(predecessors) = control.storage_predecessors.span(predecessors)
                        else {
                            return Some(Resolution::Unavailable);
                        };
                        pending.extend(predecessors.iter().rev().map(|handle| (*handle, false)));
                    } else if let FlowStorageOperation::PreserveBinding {
                        destination,
                        source: entry,
                    } = event.operation
                        && preserved_binding_matches(
                            program,
                            control,
                            source,
                            destination,
                            observation,
                        ) == Some(true)
                    {
                        pending.push((entry, false));
                    } else {
                        pending.push((event.previous, false));
                    }
                }
            }
            Some(resolved(&annotations, snapshot))
        }

        fn terminal_resolution(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            current: Snapshot,
            operation: &FlowStorageOperation,
            source: &CanonicalPlace,
            observation: StoredObservation,
            boundary: &SourceBoundary<'_>,
        ) -> Option<Option<Resolution>> {
            Some(match *operation {
                FlowStorageOperation::Unknown => Some(Resolution::Unavailable),
                FlowStorageOperation::Store { destination, .. }
                    if matches!(boundary, SourceBoundary::ReadCuts(_)) =>
                {
                    let path = control
                        .operand_referent_segments
                        .span(destination.segments)?;
                    let overlaps = match observation {
                        StoredObservation::Binding => {
                            crate::storage_path_relations::binding_overwrite(program, source, path)
                        }
                        StoredObservation::InlineContents | StoredObservation::Contents => {
                            crate::storage_path_relations::contents_overlap(program, source, path)
                        }
                    };
                    (destination.root == source.root && overlaps).then_some(Resolution::Unavailable)
                }
                FlowStorageOperation::Store {
                    destination, value, ..
                } => match matches_store(program, control, source, destination) {
                    None => Some(Resolution::Unavailable),
                    Some(false) => None,
                    Some(true) if control.operands.is_valid(value) => Some(Resolution::Sources {
                        incoming: false,
                        stores: vec![current],
                    }),
                    Some(true) => Some(Resolution::Unavailable),
                },
                FlowStorageOperation::Writes {
                    bindings,
                    contents,
                    bindings_complete,
                    contents_complete,
                } => {
                    let (writes, complete) = match observation {
                        StoredObservation::Binding | StoredObservation::InlineContents => {
                            (bindings, bindings_complete)
                        }
                        StoredObservation::Contents => (contents, contents_complete),
                    };
                    let stable = complete
                        && control
                            .operand_origin_writes
                            .span(writes)
                            .is_some_and(|writes| {
                                writes.iter().all(|write| {
                                    write.root != source.root
                                || control
                                    .operand_referent_segments
                                    .span(write.segments)
                                    .is_some_and(|segments| !match observation {
                                        StoredObservation::Binding => {
                                            crate::storage_path_relations::binding_overwrite(
                                                program, source, segments,
                                            )
                                        }
                                        StoredObservation::InlineContents
                                        | StoredObservation::Contents => {
                                            crate::storage_path_relations::contents_overlap(
                                                program, source, segments,
                                            )
                                        }
                                    })
                                })
                            });
                    (!stable).then_some(Resolution::Unavailable)
                }
                FlowStorageOperation::PreserveBinding { destination, .. } => {
                    preserved_binding_matches(program, control, source, destination, observation)
                        .is_none()
                        .then_some(Resolution::Unavailable)
                }
                FlowStorageOperation::Join { .. } => None,
            })
        }

        fn resolve_chain(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            observation: StoredObservation,
            boundary: &SourceBoundary<'_>,
        ) -> Option<Option<Resolution>> {
            let mut current = snapshot;
            for _ in 0..=control.storage_snapshots.len() {
                if !current.is_valid() {
                    return Some(Some(Resolution::Incoming));
                }
                if !control.storage_snapshots.is_valid(current) {
                    return Some(Some(Resolution::Unavailable));
                }
                if let SourceBoundary::ReadCuts(cuts) = boundary
                    && cuts.contains(&identity(current))
                {
                    return Some(Some(Resolution::Sources {
                        incoming: false,
                        stores: vec![current],
                    }));
                }
                let event = control.storage_snapshots.get(current);
                let Some(terminal) = terminal_resolution(
                    program,
                    control,
                    current,
                    &event.operation,
                    source,
                    observation,
                    boundary,
                ) else {
                    return Some(None);
                };
                if let Some(result) = terminal {
                    if matches!(result, Resolution::Unavailable) {
                        trace_query_refusal(control, current, source, observation);
                    }
                    return Some(Some(result));
                }
                current = match event.operation {
                    FlowStorageOperation::Join { .. } => return None,
                    FlowStorageOperation::PreserveBinding {
                        destination,
                        source: entry,
                    } if preserved_binding_matches(
                        program,
                        control,
                        source,
                        destination,
                        observation,
                    ) == Some(true) =>
                    {
                        entry
                    }
                    _ => event.previous,
                };
            }
            None
        }

        pub fn at_snapshot(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            observation: StoredObservation,
        ) -> Option<StoredOperand> {
            let resolved = resolve_snapshot(
                program,
                control,
                snapshot,
                source,
                observation,
                SourceBoundary::Stores,
            )?;
            if matches!(resolved, Resolution::Incoming) {
                None
            } else {
                sources_from_resolution(control, snapshot, source, resolved)
            }
        }

        pub fn sources_at_snapshot(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            observation: StoredObservation,
        ) -> Option<StoredOperand> {
            let resolved = resolve_snapshot(
                program,
                control,
                snapshot,
                source,
                observation,
                SourceBoundary::Stores,
            )?;
            sources_from_resolution(control, snapshot, source, resolved)
        }

        fn sources_from_resolution(
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            resolved: Resolution,
        ) -> Option<StoredOperand> {
            match resolved {
                Resolution::Incoming => Some(StoredOperand::Incoming(source.clone())),
                Resolution::Unavailable => Some(StoredOperand::Unavailable),
                Resolution::Sources { incoming, stores } => {
                    let mut values = Vec::new();
                    if incoming {
                        values.push(StoredOperand::Incoming(source.clone()));
                    }
                    for handle in stores {
                        let FlowStorageOperation::Store {
                            destination,
                            value,
                            projection,
                        } = control.storage_snapshots.get(handle).operation
                        else {
                            return Some(StoredOperand::Unavailable);
                        };
                        let Some(path) =
                            control.operand_referent_segments.span(destination.segments)
                        else {
                            return Some(StoredOperand::Unavailable);
                        };
                        let Some(selected) = control.operand_retained_paths.span(projection) else {
                            return Some(StoredOperand::Unavailable);
                        };
                        let mut projection = selected.to_vec();
                        projection.extend_from_slice(&source.segments[path.len()..]);
                        if trace_query_enabled(source) {
                            eprintln!(
                                "STORAGE_QUERY selected-store query_snapshot={snapshot:?} store={handle:?} source={source:?} destination_root={:?} destination_path={path:?} value={value:?} type={:?} projection={projection:?}",
                                destination.root,
                                control.operands.get(value).type_reference
                            );
                        }
                        values.push(StoredOperand::Value {
                            capture: value,
                            reference: control.operands.get(value).type_reference,
                            projection,
                            slot: Some(source.clone()),
                        });
                    }
                    if values.len() == 1 {
                        values.pop()
                    } else {
                        Some(StoredOperand::Alternatives(values))
                    }
                }
            }
        }

        fn trace_query_enabled(source: &CanonicalPlace) -> bool {
            if !crate::trace_flags::storage_query_trace() {
                return false;
            }
            std::env::var("OMEGA_STORAGE_QUERY_TRACE_ROOT")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .is_none_or(|root| {
                    source
                        .root
                        .as_symbol()
                        .is_some_and(|symbol| symbol.arena_index() == root)
                })
        }

        fn trace_query_refusal(
            control: &FlowControlFacts,
            snapshot: Snapshot,
            source: &CanonicalPlace,
            observation: StoredObservation,
        ) {
            if !trace_query_enabled(source) {
                return;
            }
            let event = control.storage_snapshots.get(snapshot);
            let mode = match observation {
                StoredObservation::Binding => "binding",
                StoredObservation::InlineContents => "inline-contents",
                StoredObservation::Contents => "contents",
            };
            eprintln!(
                "STORAGE_QUERY refused source={source:?} snapshot={snapshot:?} observation={mode} event={event:?}"
            );
            match event.operation {
                FlowStorageOperation::Store {
                    destination,
                    value,
                    projection,
                } => {
                    eprintln!(
                        "STORAGE_QUERY refused-store destination_root={:?} destination_path={:?} value={value:?} type={:?} projection={:?}",
                        destination.root,
                        control.operand_referent_segments.span(destination.segments),
                        control
                            .operands
                            .is_valid(value)
                            .then(|| control.operands.get(value).type_reference),
                        control.operand_retained_paths.span(projection)
                    );
                }
                FlowStorageOperation::Writes {
                    bindings,
                    contents,
                    bindings_complete,
                    contents_complete,
                } => {
                    for (mode, writes, complete) in [
                        ("binding", bindings, bindings_complete),
                        ("contents", contents, contents_complete),
                    ] {
                        let writes = control.operand_origin_writes.span(writes).map(|rows| {
                            rows.iter()
                                .map(|write| {
                                    (
                                        write.root,
                                        control.operand_referent_segments.span(write.segments),
                                    )
                                })
                                .collect::<Vec<_>>()
                        });
                        eprintln!(
                            "STORAGE_QUERY refused-writes mode={mode} complete={complete} writes={writes:?}"
                        );
                    }
                }
                _ => {}
            }
        }
    }
    pub use query::{at_snapshot, contents_preserved_from, sources_at_snapshot};
    pub mod versions {
        use super::{StoredObservation, StoredOperand, at_snapshot, contents_preserved_from};
        use crate::CanonicalPlace;
        use arena::Handle;
        use checked_trees::{
            FlowCallFact, FlowControlFacts, FlowStateFact, FlowStorageOperation,
            FlowStorageSnapshot,
        };
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;

        pub type Snapshot = Handle<FlowStorageSnapshot>;

        pub fn state_row<'graph>(
            program: &dyn crate::CheckingInput,
            graph: &'graph FlowControlFacts,
            state: SymbolHandle,
        ) -> Option<&'graph FlowStateFact> {
            let (machine, _) = crate::find_state(program, state)?;
            graph.unique_state(machine.symbol, state)
        }

        pub fn statement_snapshot(
            program: &dyn crate::CheckingInput,
            graph: &FlowControlFacts,
            state: SymbolHandle,
            statement_index: usize,
        ) -> Option<Snapshot> {
            let row = state_row(program, graph, state)?;
            graph
                .statements
                .span(row.statements)?
                .iter()
                .find(|statement| statement.statement_index == statement_index)
                .map(|statement| statement.storage_snapshot)
        }

        pub fn operand_snapshot(
            graph: &FlowControlFacts,
            state: SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<Snapshot> {
            graph.with_expression_operands(expression, |rows| {
                let mut matching = rows
                    .iter()
                    .map(|handle| graph.operands.get(*handle))
                    .filter(|operand| operand.state_symbol == state);
                let operand = matching.next()?;
                matching
                    .all(|other| other.storage_snapshot == operand.storage_snapshot)
                    .then_some(operand.storage_snapshot)
            })
        }

        pub fn invocation_snapshot(
            program: &dyn crate::CheckingInput,
            graph: &FlowControlFacts,
            state: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        ) -> Option<Snapshot> {
            let row = state_row(program, graph, state)?;
            graph
                .calls
                .span(row.calls)?
                .iter()
                .find(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
                .map(|call| call.invocation_storage)
        }

        pub fn unique_call<'graph>(
            program: &dyn crate::CheckingInput,
            graph: &'graph FlowControlFacts,
            state: SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<&'graph FlowCallFact> {
            let row = state_row(program, graph, state)?;
            let mut calls = graph
                .calls
                .span(row.calls)?
                .iter()
                .filter(|call| call.authored_expression == expression);
            let call = calls.next()?;
            calls.next().is_none().then_some(call)
        }

        pub fn followed_path(
            graph: &FlowControlFacts,
            from: Snapshot,
            to: Snapshot,
        ) -> Option<Vec<(Snapshot, Snapshot)>> {
            let mut path = Vec::new();
            let mut crossed_join = false;
            let mut current = to;
            while current != from {
                if !current.is_valid() || !graph.storage_snapshots.is_valid(current) {
                    return None;
                }
                let event = graph.storage_snapshots.get(current);
                let previous = match event.operation {
                    FlowStorageOperation::Join { predecessors } => {
                        let mut reaching = graph
                            .storage_predecessors
                            .span_or_empty(predecessors)
                            .iter()
                            .filter(|predecessor| reaches(graph, **predecessor, from));
                        let predecessor = *reaching.next()?;
                        if reaching.next().is_some() {
                            return None;
                        }
                        crossed_join = true;
                        predecessor
                    }
                    _ => {
                        if !event.previous.is_valid() && from.is_valid() {
                            return None;
                        }
                        path.push((current, event.previous));
                        event.previous
                    }
                };
                current = previous;
            }
            (crossed_join && path.iter().all(|(_, before)| before.is_valid())).then_some(path)
        }

        fn reaches(graph: &FlowControlFacts, start: Snapshot, target: Snapshot) -> bool {
            let mut visited = std::collections::BTreeSet::new();
            let mut pending = vec![start];
            while let Some(current) = pending.pop() {
                if current == target {
                    return true;
                }
                if !current.is_valid()
                    || !graph.storage_snapshots.is_valid(current)
                    || !visited.insert(current.arena_index())
                {
                    continue;
                }
                let event = graph.storage_snapshots.get(current);
                match event.operation {
                    FlowStorageOperation::Join { predecessors } => {
                        pending.extend(graph.storage_predecessors.span_or_empty(predecessors));
                    }
                    _ => pending.push(event.previous),
                }
            }
            false
        }

        pub fn place_preserved_since(
            program: &dyn crate::CheckingInput,
            graph: &FlowControlFacts,
            at: Snapshot,
            place: &CanonicalPlace,
            anchor: Snapshot,
        ) -> bool {
            if anchor == at {
                return true;
            }
            if !anchor.is_valid() {
                return !at.is_valid()
                    || matches!(
                        at_snapshot(program, graph, at, place, StoredObservation::Contents),
                        None | Some(StoredOperand::Incoming(_))
                    );
            }
            at.is_valid()
                && contents_preserved_from(
                    program,
                    graph,
                    at,
                    place,
                    &[anchor],
                    StoredObservation::Contents,
                )
        }
    }

    #[derive(Clone)]
    pub enum StoredOperand {
        Incoming(crate::CanonicalPlace),
        Unavailable,
        Alternatives(Vec<StoredOperand>),
        Value {
            capture: Handle<FlowOperandFact>,
            reference: typed_trees::types::TypeReferenceHandle,
            projection: Vec<facts::PlaceSegment>,
            slot: Option<crate::CanonicalPlace>,
        },
    }

    #[derive(Clone, Copy)]
    pub enum StoredObservation {
        Binding,
        InlineContents,
        Contents,
    }

    #[derive(Clone)]
    pub struct StoredCapture {
        pub capture: Handle<FlowOperandFact>,
        pub reference: typed_trees::types::TypeReferenceHandle,
        pub projection: Vec<facts::PlaceSegment>,
        pub slot: Option<crate::CanonicalPlace>,
    }

    impl StoredOperand {
        pub fn captures(self) -> Option<Vec<StoredCapture>> {
            match self {
                Self::Unavailable | Self::Incoming(_) => None,
                Self::Value {
                    capture,
                    reference,
                    projection,
                    slot,
                } => Some(vec![StoredCapture {
                    capture,
                    reference,
                    projection,
                    slot,
                }]),
                Self::Alternatives(alternatives) => {
                    let mut captures = Vec::new();
                    for alternative in alternatives {
                        captures.extend(alternative.captures()?);
                    }
                    (!captures.is_empty()).then_some(captures)
                }
            }
        }
    }

    pub(crate) mod backing {
        use arena::{Handle, HandleSpan};
        use checked_trees::FlowOperandRetention;
        use checked_trees::{CapturedPlace, FlowControlFacts, FlowOperandFact};
        use facts::PlaceSegment;

        use super::{FrozenReferenceSelection, frozen_reference_leaf};

        pub(crate) type Query = (Handle<FlowOperandFact>, Vec<PlaceSegment>);
        pub(crate) type UnchangedLoan<'a> =
            dyn FnMut(Handle<FlowOperandFact>, &[PlaceSegment]) -> Option<Vec<CapturedPlace>> + 'a;

        pub fn reference_backing_places(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            unchanged_loan: &mut UnchangedLoan<'_>,
        ) -> Option<Vec<CapturedPlace>> {
            Resolver {
                program,
                control,
                unchanged_loan,
                active: Vec::new(),
                active_profiles: Vec::new(),
                projectable: false,
                write_footprint: false,
            }
            .places(operand, path)
        }

        /*
            The storage a write through this reference may reach. A view's enclosure
            is widened to its complete footprint here, not in loan or pointer
            resolution: those consumers keep the anchor they authenticate.
        */
        pub fn reference_write_backing_places(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            unchanged_loan: &mut UnchangedLoan<'_>,
        ) -> Option<Vec<CapturedPlace>> {
            Resolver {
                program,
                control,
                unchanged_loan,
                active: Vec::new(),
                active_profiles: Vec::new(),
                projectable: false,
                write_footprint: true,
            }
            .places(operand, path)
        }

        pub fn projectable_reference_places(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
        ) -> Option<Vec<CapturedPlace>> {
            Resolver {
                program,
                control,
                unchanged_loan: &mut |_, _| None,
                active: Vec::new(),
                active_profiles: Vec::new(),
                projectable: true,
                write_footprint: false,
            }
            .places(operand, path)
        }

        pub(crate) struct Resolver<'a, 'resolver> {
            program: &'a dyn crate::CheckingInput,
            control: &'a FlowControlFacts,
            unchanged_loan: &'resolver mut UnchangedLoan<'a>,
            active: Vec<Query>,
            active_profiles: Vec<Handle<FlowOperandRetention>>,
            projectable: bool,
            write_footprint: bool,
        }

        pub(crate) enum ProfileBacking {
            Known(Vec<CapturedPlace>),
            Unchanged,
        }

        impl Resolver<'_, '_> {
            fn places(
                &mut self,
                operand: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
            ) -> Option<Vec<CapturedPlace>> {
                let query = (operand, path.to_vec());
                if self.active.contains(&query) || !self.control.operands.is_valid(operand) {
                    return None;
                }
                self.active.push(query);
                let result = self.resolve(operand, path);
                self.active.pop();
                result
            }

            fn resolve(
                &mut self,
                operand: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
            ) -> Option<Vec<CapturedPlace>> {
                let capture = *self.control.operands.get(operand);
                if let checked_trees::FlowOperandCompletion::Selected(alternatives) =
                    capture.completion
                {
                    let rows = self.control.operand_alternatives.span(alternatives)?;
                    if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                        return None;
                    }
                    let values = rows.iter().map(|row| row.value).collect::<Vec<_>>();
                    let mut output = Vec::new();
                    for value in values {
                        Self::append(&mut output, self.places(value, path)?);
                    }
                    return Some(output);
                }
                if path.is_empty()
                    && let Some(profile) = self.profile(&capture)
                    && (!profile.possible_storage.is_empty() || !profile.alternatives.is_empty())
                {
                    return match self.profile_backing(capture.retention)? {
                        ProfileBacking::Known(places) => Some(places),
                        ProfileBacking::Unchanged => None,
                    };
                }
                match frozen_reference_leaf(self.program, self.control, operand, path) {
                    FrozenReferenceSelection::Unavailable => return None,
                    FrozenReferenceSelection::Endpoint { leaf, .. } => {
                        if leaf.referents.is_empty() {
                            let profiles = super::captured_retention_profiles_at_path(
                                self.program,
                                self.control,
                                &capture,
                                path,
                            )?;
                            if profiles.is_empty() {
                                return None;
                            }
                            let mut output = Vec::new();
                            for profile in profiles {
                                match self.profile_backing(profile)? {
                                    ProfileBacking::Known(places) => {
                                        Self::append(&mut output, places)
                                    }
                                    ProfileBacking::Unchanged => return None,
                                }
                            }
                            return Some(output);
                        }
                        return self.referents_or_loan(operand, path, leaf.referents);
                    }
                    FrozenReferenceSelection::NotFrozen
                    | FrozenReferenceSelection::ThroughReference => {}
                }
                if path.is_empty()
                    && (crate::reference_shell(self.program, capture.type_reference).is_some()
                        || matches!(
                            capture.completion,
                            checked_trees::FlowOperandCompletion::Observation
                        ))
                    && let Some(profile) = self.profile(&capture)
                    && profile.alternatives.is_empty()
                {
                    if profile.enclosing_storage.is_valid() {
                        return self.enclosing(profile.enclosing_storage);
                    }
                    if !profile.enclosing_regions.is_empty() {
                        return self.regions(profile.enclosing_regions);
                    }
                }
                let Some(selected) = super::reference_captures_at_path(
                    self.program,
                    self.control,
                    operand,
                    path,
                    capture.storage_snapshot,
                ) else {
                    return (self.unchanged_loan)(operand, path);
                };
                let selected = selected.captures()?;
                if selected.is_empty() {
                    return None;
                }
                let mut output = Vec::new();
                let mut needs_unchanged_loan = false;
                for selected in selected {
                    let places = if !selected.projection.is_empty() {
                        self.places(selected.capture, &selected.projection)?
                    } else {
                        if !self.control.operands.is_valid(selected.capture) {
                            return None;
                        }
                        let descriptor = *self.control.operands.get(selected.capture);
                        let profile = self.profile(&descriptor);
                        if profile.is_some_and(|profile| !profile.alternatives.is_empty()) {
                            match self.profile_backing(descriptor.retention)? {
                                ProfileBacking::Known(places) => places,
                                ProfileBacking::Unchanged => {
                                    needs_unchanged_loan = true;
                                    continue;
                                }
                            }
                        } else if profile.is_some_and(|profile| profile.immutable_image) {
                            continue;
                        } else if let Some(profile) = profile
                            && profile.enclosing_storage.is_valid()
                        {
                            self.enclosing(profile.enclosing_storage)?
                        } else if let Some(profile) = profile
                            && !profile.enclosing_regions.is_empty()
                        {
                            self.regions(profile.enclosing_regions)?
                        } else if !descriptor.referents.is_empty() {
                            self.referents_or_loan(selected.capture, &[], descriptor.referents)?
                        } else {
                            match self.profile_backing(descriptor.retention)? {
                                ProfileBacking::Known(places) => places,
                                ProfileBacking::Unchanged => {
                                    needs_unchanged_loan = true;
                                    continue;
                                }
                            }
                        }
                    };
                    Self::append(&mut output, places);
                }
                if needs_unchanged_loan {
                    Self::append(&mut output, (self.unchanged_loan)(operand, path)?);
                }
                Some(output)
            }

            fn profile(&self, capture: &FlowOperandFact) -> Option<FlowOperandRetention> {
                self.control
                    .operand_retentions
                    .is_valid(capture.retention)
                    .then(|| *self.control.operand_retentions.get(capture.retention))
            }

            fn profile_backing(
                &mut self,
                profile: Handle<FlowOperandRetention>,
            ) -> Option<ProfileBacking> {
                if !profile.is_valid() {
                    return Some(ProfileBacking::Unchanged);
                }
                if !self.control.operand_retentions.is_valid(profile) {
                    return None;
                }
                if self.active_profiles.contains(&profile) {
                    return None;
                }
                self.active_profiles.push(profile);
                let result = self.resolve_profile(profile);
                self.active_profiles.pop();
                result
            }

            fn resolve_profile(
                &mut self,
                profile: Handle<FlowOperandRetention>,
            ) -> Option<ProfileBacking> {
                let row = *self.control.operand_retentions.get(profile);
                if !row.alternatives.is_empty() {
                    self.control.operand_retentions.span(row.alternatives)?;
                    let handles = row.alternatives.handles().collect::<Vec<_>>();
                    let mut output = Vec::new();
                    let mut known = false;
                    let mut unchanged = false;
                    for profile in handles {
                        match self.profile_backing(profile)? {
                            ProfileBacking::Known(places) => {
                                known = true;
                                Self::append(&mut output, places);
                            }
                            ProfileBacking::Unchanged => unchanged = true,
                        }
                    }
                    return if known && unchanged {
                        None
                    } else if unchanged {
                        Some(ProfileBacking::Unchanged)
                    } else {
                        Some(ProfileBacking::Known(output))
                    };
                }
                if row.immutable_image {
                    return Some(ProfileBacking::Known(Vec::new()));
                }
                if self.projectable
                    && (row.enclosing_storage.is_valid() || !row.enclosing_regions.is_empty())
                {
                    return None;
                }
                if !row.possible_storage.is_empty() {
                    return Some(ProfileBacking::Known(super::direct_referent_places(
                        self.control,
                        row.possible_storage,
                    )?));
                }
                if row.enclosing_storage.is_valid() {
                    return Some(ProfileBacking::Known(
                        self.enclosing(row.enclosing_storage)?,
                    ));
                }
                if !row.enclosing_regions.is_empty() {
                    return Some(ProfileBacking::Known(self.regions(row.enclosing_regions)?));
                }
                if row.descriptor.is_valid() {
                    return Some(ProfileBacking::Known(self.places(row.descriptor, &[])?));
                }
                if self.projectable || row.suppliers.is_empty() {
                    return Some(ProfileBacking::Unchanged);
                }
                let suppliers = self
                    .control
                    .operand_retained_suppliers
                    .span(row.suppliers)?
                    .to_vec();
                if suppliers.is_empty() {
                    return None;
                }
                let mut output = Vec::new();
                for supplier in suppliers {
                    let path = self
                        .control
                        .operand_retained_paths
                        .span(supplier.projection)?
                        .to_vec();
                    Self::append(&mut output, self.places(supplier.operand, &path)?);
                }
                Some(ProfileBacking::Known(output))
            }

            fn regions(
                &mut self,
                regions: HandleSpan<checked_trees::FlowOperandRetainedRegion>,
            ) -> Option<Vec<CapturedPlace>> {
                if self.projectable {
                    return None;
                }
                let regions = self
                    .control
                    .operand_retained_regions
                    .span(regions)?
                    .to_vec();
                if regions.is_empty() {
                    return None;
                }
                let mut output = Vec::new();
                for region in regions {
                    let projection = self
                        .control
                        .operand_retained_paths
                        .span(region.projection)?
                        .to_vec();
                    Self::append(&mut output, self.places(region.operand, &projection)?);
                }
                Some(output)
            }

            fn enclosing(
                &mut self,
                enclosing: Handle<FlowOperandFact>,
            ) -> Option<Vec<CapturedPlace>> {
                if self.projectable {
                    return None;
                }
                if !self.control.operands.is_valid(enclosing) {
                    return None;
                }
                let storage = *self.control.operands.get(enclosing);
                if crate::reference_shell(self.program, storage.type_reference).is_some()
                    || crate::slice_view_shell(self.program, storage.type_reference, &[]).is_some()
                {
                    return self.places(enclosing, &[]);
                }
                let mut places = self.referents_or_loan(enclosing, &[], storage.referents)?;
                if self.write_footprint {
                    for place in &mut places {
                        super::widen_element_anchor(&mut place.segments);
                    }
                }
                Some(places)
            }

            fn referents_or_loan(
                &mut self,
                operand: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
                span: HandleSpan<checked_trees::FlowOperandReferent>,
            ) -> Option<Vec<CapturedPlace>> {
                let referents = self.control.operand_referents.span(span)?;
                if referents.is_empty() {
                    return None;
                }
                for referent in referents {
                    if referent.formation.is_valid() {
                        if !self
                            .control
                            .referent_formations
                            .is_valid(referent.formation)
                        {
                            return None;
                        }
                        let formation = self.control.referent_formations.get(referent.formation);
                        if formation.owner.is_valid()
                            && !self.control.operands.is_valid(formation.owner)
                        {
                            return None;
                        }
                    }
                }
                let deferred = |referent: &checked_trees::FlowOperandReferent| {
                    self.control
                        .referent_formations
                        .is_valid(referent.formation)
                        && self
                            .control
                            .referent_formations
                            .get(referent.formation)
                            .owner
                            .is_valid()
                };
                if referents.iter().any(deferred) {
                    if !referents.iter().all(deferred) {
                        return None;
                    }
                    return (self.unchanged_loan)(operand, path);
                }
                direct_referent_places(self.control, span)
            }

            fn append(output: &mut Vec<CapturedPlace>, places: Vec<CapturedPlace>) {
                for place in places {
                    if !output.contains(&place) {
                        output.push(place);
                    }
                }
            }
        }

        pub fn direct_referent_places(
            control: &FlowControlFacts,
            span: HandleSpan<checked_trees::FlowOperandReferent>,
        ) -> Option<Vec<CapturedPlace>> {
            let referents = control.operand_referents.span(span)?;
            if referents.is_empty() {
                return None;
            }
            let mut output = Vec::new();
            for referent in referents {
                if !referent.root.is_valid() {
                    return None;
                }
                if referent.formation.is_valid()
                    && (!control.referent_formations.is_valid(referent.formation)
                        || control
                            .referent_formations
                            .get(referent.formation)
                            .owner
                            .is_valid())
                {
                    return None;
                }
                let place = CapturedPlace {
                    incoming_reference: Handle::invalid(),
                    incoming_owner_path: Default::default(),
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span(referent.segments)?
                        .to_vec(),
                };
                if !output.contains(&place) {
                    output.push(place);
                }
            }
            Some(output)
        }
    }
    /*
        A view formed at one element (`&bytes[k] as &T`) addresses the collection
        from that element onward, so it may cover later elements: the anchor element
        alone does not enclose its footprint, and a write recorded there would leave
        facts about the later elements standing. The collection does enclose it. A
        range anchor already names its exact tiled extent and is left unchanged.
    */
    pub fn widen_element_anchor(segments: &mut Vec<facts::PlaceSegment>) {
        while matches!(
            segments.last(),
            Some(facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. })
        ) {
            segments.pop();
        }
    }

    pub use backing::{
        direct_referent_places, projectable_reference_places, reference_backing_places,
        reference_write_backing_places,
    };
    pub(crate) mod captures {
        use checked_trees::FlowOperandCompletion;
        use checked_trees::{FlowControlFacts, FlowOperandFact};
        use facts::PlaceSegment;
        use typed_trees::types::TypeReferenceHandle;

        pub fn operand_child_type(
            program: &dyn crate::CheckingInput,
            parent: TypeReferenceHandle,
            destination: &[PlaceSegment],
            declared: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            parent
                .is_valid()
                .then(|| crate::project_type_reference_from_segments(program, parent, destination))
                .flatten()
                .or_else(|| declared.is_valid().then_some(declared))
        }

        pub fn captured_retention_profiles_at_path(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
            fn operand_profiles(
                program: &dyn crate::CheckingInput,
                control: &FlowControlFacts,
                operand: &FlowOperandFact,
                path: &[PlaceSegment],
                active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
                active_copies: &mut Vec<arena::Handle<checked_trees::FlowCopiedValue>>,
                active_selections: &mut Vec<
                    arena::HandleSpan<checked_trees::FlowOperandAlternative>,
                >,
            ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
                let path =
                    crate::corresponding_place_projection(program, operand.type_reference, path)?;
                let path = path.as_slice();
                if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
                    if active_selections.contains(&alternatives) {
                        return None;
                    }
                    let rows = control.operand_alternatives.span(alternatives)?;
                    if rows.is_empty()
                        || rows.iter().any(|row| {
                            !row.source_arm.is_valid() || !control.operands.is_valid(row.value)
                        })
                    {
                        return None;
                    }
                    active_selections.push(alternatives);
                    let result = (|| {
                        let mut profiles = Vec::new();
                        for row in rows {
                            for profile in operand_profiles(
                                program,
                                control,
                                control.operands.get(row.value),
                                path,
                                active,
                                active_copies,
                                active_selections,
                            )? {
                                if !profiles.contains(&profile) {
                                    profiles.push(profile);
                                }
                            }
                        }
                        (!profiles.is_empty()).then_some(profiles)
                    })();
                    active_selections.pop();
                    return result;
                }
                if !operand.reference_leaves.is_empty() {
                    let mut matching = control
                        .captured_reference_leaves
                        .span_or_empty(operand.reference_leaves)
                        .iter()
                        .filter(|leaf| {
                            let selected = control.operand_child_paths.span_or_empty(leaf.path);
                            selected.len() == path.len()
                                && selected.iter().zip(path).all(|(left, right)| {
                                    crate::storage_path_relations::canonical_place_segments_equal(
                                        program, *left, *right,
                                    )
                                })
                        });
                    let leaf = matching.next()?;
                    if matching.next().is_some() {
                        return None;
                    }
                    return profile_at_path(
                        program,
                        control,
                        leaf.retention,
                        leaf.type_reference,
                        &[],
                        active,
                        active_copies,
                        active_selections,
                    );
                }
                if let FlowOperandCompletion::Copied { copy, path: prefix } = operand.completion {
                    if !control.copied_values.is_valid(copy) || active_copies.contains(&copy) {
                        return None;
                    }
                    active_copies.push(copy);
                    let selected = (|| {
                        let copy = control.copied_values.get(copy);
                        let mut selected_path =
                            control.operand_child_paths.span_or_empty(prefix).to_vec();
                        selected_path.extend_from_slice(path);
                        let mut leaves = control
                            .captured_reference_leaves
                            .span_or_empty(copy.references)
                            .iter()
                            .filter(|leaf| {
                                control.operand_child_paths.span_or_empty(leaf.path)
                                    == selected_path
                            });
                        if let Some(leaf) = leaves.next() {
                            if leaves.next().is_some() {
                                return None;
                            }
                            return profile_at_path(
                                program,
                                control,
                                leaf.retention,
                                leaf.type_reference,
                                &[],
                                active,
                                active_copies,
                                active_selections,
                            );
                        }
                        if !control.operands.is_valid(copy.source) {
                            return None;
                        }
                        operand_profiles(
                            program,
                            control,
                            control.operands.get(copy.source),
                            &selected_path,
                            active,
                            active_copies,
                            active_selections,
                        )
                    })();
                    active_copies.pop();
                    return selected;
                }
                let mut selected_child = None;
                for child in control
                    .operand_children
                    .span_or_empty(operand.constructed_children())
                {
                    let destination = control.operand_child_paths.span_or_empty(child.path);
                    if destination.is_empty() {
                        return None;
                    }
                    if destination.len() <= path.len()
                        && destination.iter().zip(path).all(|(left, right)| {
                            crate::storage_path_relations::canonical_place_segments_equal(
                                program, *left, *right,
                            )
                        })
                    {
                        if selected_child.is_some() || !control.operands.is_valid(child.value) {
                            return None;
                        }
                        selected_child = Some((child.value, destination.len()));
                    }
                }
                if let Some((child, length)) = selected_child {
                    return operand_profiles(
                        program,
                        control,
                        control.operands.get(child),
                        &path[length..],
                        active,
                        active_copies,
                        active_selections,
                    );
                }
                profile_at_path(
                    program,
                    control,
                    operand.retention,
                    operand.type_reference,
                    path,
                    active,
                    active_copies,
                    active_selections,
                )
            }

            fn profile_at_path(
                program: &dyn crate::CheckingInput,
                control: &FlowControlFacts,
                retention: arena::Handle<checked_trees::FlowOperandRetention>,
                reference: TypeReferenceHandle,
                path: &[PlaceSegment],
                active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
                active_copies: &mut Vec<arena::Handle<checked_trees::FlowCopiedValue>>,
                active_selections: &mut Vec<
                    arena::HandleSpan<checked_trees::FlowOperandAlternative>,
                >,
            ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
                if !control.operand_retentions.is_valid(retention) || active.contains(&retention) {
                    return None;
                }
                let path = crate::corresponding_place_projection(program, reference, path)?;
                let path = path.as_slice();
                let profile = control.operand_retentions.get(retention);
                if path.is_empty() && profile.stored_projection.is_empty() {
                    return Some(vec![retention]);
                }
                active.push(retention);
                let selected = (|| {
                    let mut selected = control
                        .operand_retained_leaves
                        .span_or_empty(profile.leaves)
                        .iter()
                        .filter(|leaf| {
                            let Some(candidate) = crate::corresponding_place_projection(
                                program,
                                reference,
                                control.operand_retained_paths.span_or_empty(leaf.path),
                            ) else {
                                return false;
                            };
                            !candidate.is_empty()
                                && candidate.len() <= path.len()
                                && candidate.iter().zip(path).all(|(left, right)| {
                                    crate::storage_path_relations::canonical_place_segments_equal(
                                        program, *left, *right,
                                    )
                                })
                        });
                    if let Some(leaf) = selected.next() {
                        if selected.next().is_some() {
                            return None;
                        }
                        let length = leaf.path.len();
                        return profile_at_path(
                            program,
                            control,
                            leaf.retention,
                            crate::project_type_reference_from_segments(
                                program,
                                reference,
                                &path[..length],
                            )?,
                            &path[length..],
                            active,
                            active_copies,
                            active_selections,
                        );
                    }
                    if !profile.alternatives.is_empty() {
                        let mut profiles = Vec::new();
                        for alternative in profile.alternatives.handles() {
                            for selected in profile_at_path(
                                program,
                                control,
                                alternative,
                                reference,
                                path,
                                active,
                                active_copies,
                                active_selections,
                            )? {
                                if !profiles.contains(&selected) {
                                    profiles.push(selected);
                                }
                            }
                        }
                        return (!profiles.is_empty()).then_some(profiles);
                    }
                    let stored = profile.stored_value;
                    if !control.operands.is_valid(stored) {
                        return None;
                    }
                    let prefix = control
                        .operand_retained_paths
                        .span_or_empty(profile.stored_projection);
                    let mut selected = prefix.to_vec();
                    selected.extend_from_slice(path);
                    operand_profiles(
                        program,
                        control,
                        control.operands.get(stored),
                        &selected,
                        active,
                        active_copies,
                        active_selections,
                    )
                })();
                active.pop();
                selected
            }
            operand_profiles(
                program,
                control,
                operand,
                path,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
            )
        }

        pub fn project_captured_view_place(
            mut place: crate::CanonicalPlace,
            path: &[PlaceSegment],
        ) -> Option<crate::CanonicalPlace> {
            place.segments = facts::project_captured_view_segments(&place.segments, path)?;
            Some(place)
        }

        pub fn project_captured_storage_place(
            program: &dyn crate::CheckingInput,
            state: symbols::SymbolHandle,
            statement_index: usize,
            mut place: crate::CanonicalPlace,
            path: &[PlaceSegment],
        ) -> Option<crate::CanonicalPlace> {
            for segment in path {
                let selected = if matches!(
                    segment,
                    PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
                ) {
                    crate::canonical_place_type_reference(program, state, statement_index, &place)
                        .and_then(|reference| {
                            crate::corresponding_place_projection(
                                program,
                                reference,
                                std::slice::from_ref(segment),
                            )
                        })
                        .and_then(|mapped| match mapped.as_slice() {
                            [selected] => Some(*selected),
                            _ => None,
                        })
                        .unwrap_or(*segment)
                } else {
                    *segment
                };
                place = project_captured_view_place(place, std::slice::from_ref(&selected))?;
            }
            Some(place)
        }

        pub fn captured_operand_child_at_path(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: &FlowOperandFact,
            reference: TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> Option<(arena::Handle<FlowOperandFact>, TypeReferenceHandle, usize)> {
            let (child, consumed) =
                selected_operand_child_at_path(program, control, operand, path)?;
            let destination = control.operand_child_paths.span(child.path)?;
            Some((
                child.value,
                operand_child_type(program, reference, destination, child.type_reference)?,
                consumed,
            ))
        }

        fn selected_operand_child_at_path(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            operand: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<(checked_trees::FlowOperandChild, usize)> {
            let mut selected = None;
            for child in control
                .operand_children
                .span(operand.constructed_children())?
            {
                let destination = control.operand_child_paths.span(child.path)?;
                if destination.is_empty() {
                    return None;
                }
                if destination.len() <= path.len()
                    && destination.iter().zip(path).all(|(left, right)| {
                        crate::storage_path_relations::canonical_place_segments_equal(
                            program, *left, *right,
                        )
                    })
                {
                    if selected.is_some() || !control.operands.is_valid(child.value) {
                        return None;
                    }
                    selected = Some((*child, destination.len()));
                }
            }
            selected
        }

        pub fn finite_projection_paths(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            projection: &[PlaceSegment],
        ) -> Option<Vec<Vec<PlaceSegment>>> {
            const MAX_PROJECTIONS: usize = 4096;
            let mut pending = vec![projection.to_vec()];
            let mut paths = Vec::new();
            while let Some(path) = pending.pop() {
                let Some(position) = path
                    .iter()
                    .position(|segment| matches!(segment, PlaceSegment::Index { .. }))
                else {
                    paths.push(path);
                    continue;
                };
                let collection = crate::project_type_reference_from_segments(
                    program,
                    reference,
                    &path[..position],
                )?;
                let length = crate::fixed_array_type_length(program, collection)?;
                if length == 0
                    || paths
                        .len()
                        .checked_add(pending.len())?
                        .checked_add(length)?
                        > MAX_PROJECTIONS
                {
                    return None;
                }
                for index in (0..length).rev() {
                    let mut selected = path.clone();
                    selected[position] = PlaceSegment::FixedIndex { index };
                    pending.push(selected);
                }
            }
            Some(paths)
        }

        pub fn captured_operand_children_at_path(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            capture: arena::Handle<FlowOperandFact>,
            path: &[PlaceSegment],
        ) -> Option<Vec<super::StoredCapture>> {
            if !control.operands.is_valid(capture) || path.is_empty() {
                return None;
            }
            let operand = control.operands.get(capture);
            if !operand.expression.is_valid()
                || !operand.state_symbol.is_valid()
                || !operand.type_reference.is_valid()
            {
                return None;
            }
            let FlowOperandCompletion::Constructed(children) = operand.completion else {
                return None;
            };
            let rows = control.operand_children.span(children)?;
            if rows.is_empty() || rows.iter().any(|child| child.parent != operand.expression) {
                return None;
            }
            let paths = finite_projection_paths(program, operand.type_reference, path)?;
            let mut output = Vec::with_capacity(paths.len());
            for selected in paths {
                let (selected_child, consumed) =
                    selected_operand_child_at_path(program, control, operand, &selected)?;
                let child = selected_child.value;
                let declared = selected_child.type_reference;
                if child == capture || !control.operands.is_valid(child) {
                    return None;
                }
                let value = control.operands.get(child);
                let reference = crate::project_type_reference_from_segments(
                    program,
                    operand.type_reference,
                    &selected[..consumed],
                )?;
                if !reference.is_valid()
                    || !declared.is_valid()
                    || !value.type_reference.is_valid()
                    || value.state_symbol != operand.state_symbol
                    || value.statement_index != operand.statement_index
                    || program.normalized_type_identity(reference)
                        != program.normalized_type_identity(declared)
                    || program.normalized_type_identity(reference)
                        != program.normalized_type_identity(value.type_reference)
                {
                    return None;
                }
                let projection = selected[consumed..].to_vec();
                output.push(super::StoredCapture {
                    capture: child,
                    reference,
                    projection,
                    slot: None,
                });
            }
            (!output.is_empty()).then_some(output)
        }
    }
    pub(crate) mod descriptor_suppliers {
        pub fn descriptor_suppliers(
            control: &checked_trees::FlowControlFacts,
            profile: arena::Handle<checked_trees::FlowOperandRetention>,
            active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
        ) -> Option<Vec<checked_trees::FlowOperandRetainedSupplier>> {
            if !control.operand_retentions.is_valid(profile) {
                return None;
            }
            if active.contains(&profile) {
                return Some(Vec::new());
            }
            let row = control.operand_retentions.get(profile);
            if !row.suppliers.is_empty() {
                return Some(
                    control
                        .operand_retained_suppliers
                        .span_or_empty(row.suppliers)
                        .to_vec(),
                );
            }
            if row.alternatives.is_empty() {
                return None;
            }
            active.push(profile);
            let mut values = Vec::new();
            let mut missing = false;
            let mut unavailable = false;
            for alternative in row.alternatives.handles() {
                match descriptor_suppliers(control, alternative, active) {
                    None => missing = true,
                    Some(sources) if sources.is_empty() => unavailable = true,
                    Some(sources) => values.extend(sources),
                }
            }
            active.pop();
            if unavailable || (missing && !values.is_empty()) {
                Some(Vec::new())
            } else if missing {
                None
            } else {
                Some(values)
            }
        }
    }
    pub(crate) mod reference_captures {
        use super::{StoredCapture, StoredObservation, StoredOperand};
        use arena::Handle;
        use checked_trees::FlowOperandCompletion;
        use checked_trees::{FlowControlFacts, FlowOperandFact, FlowStorageSnapshot};
        use facts::PlaceSegment;

        #[derive(Clone)]
        pub(crate) enum Selection {
            Unavailable,
            Sources {
                incoming: Vec<crate::CanonicalPlace>,
                values: Vec<StoredCapture>,
            },
        }
        #[derive(Clone, PartialEq, Eq)]
        pub(crate) struct Query {
            value: Handle<FlowOperandFact>,
            path: Vec<PlaceSegment>,
        }

        pub fn reference_captures_at_path(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            current: Handle<FlowStorageSnapshot>,
        ) -> Option<StoredOperand> {
            let sources = reference_sources_at_path(program, control, value, path, current);
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
            let Some(places) = super::projectable_reference_places(program, control, value, &[])
            else {
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

        pub(crate) type ProfileQuery = (
            Handle<checked_trees::FlowOperandRetention>,
            typed_trees::types::TypeReferenceHandle,
            Vec<PlaceSegment>,
        );

        pub(crate) struct Resolver<'a> {
            program: &'a dyn crate::CheckingInput,
            control: &'a FlowControlFacts,
            current: Handle<FlowStorageSnapshot>,
            active: Vec<Query>,
            memo: Vec<(Query, Selection)>,
            active_profiles: Vec<ProfileQuery>,
            profile_memo: Vec<(ProfileQuery, Option<Selection>)>,
        }
        impl Resolver<'_> {
            fn select(
                &mut self,
                value: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
            ) -> Selection {
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
            fn trace_selection(
                &self,
                value: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
                reason: &str,
            ) {
                if !crate::trace_flags::storage_query_trace()
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
            fn resolve(
                &mut self,
                value: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
            ) -> Selection {
                let capture = self.control.operands.get(value);
                if !capture.type_reference.is_valid() {
                    return Selection::Unavailable;
                }
                if let FlowOperandCompletion::IncomingContents { root, path: prefix } =
                    capture.completion
                {
                    let Some(prefix) = self.control.operand_retained_paths.span(prefix) else {
                        return Selection::Unavailable;
                    };
                    if !crate::project_type_reference_from_segments(
                        self.program,
                        capture.type_reference,
                        path,
                    )
                    .is_some_and(|selected| {
                        crate::reference_shell(self.program, selected).is_some()
                    }) {
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
                    let arm_values = self
                        .control
                        .operand_alternatives
                        .span(alternatives)
                        .and_then(|rows| {
                            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid())
                            {
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
                        match self.profile_descriptor(
                            capture.retention,
                            capture.type_reference,
                            path,
                        ) {
                            Some(selected) => return selected,
                            None if path.is_empty() => return Selection::Unavailable,
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
                    for length in 1..path.len() {
                        if crate::project_type_reference_from_segments(
                            self.program,
                            capture.type_reference,
                            &path[..length],
                        )
                        .is_some_and(|reference| {
                            crate::reference_shell(self.program, reference).is_some()
                        }) {
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
                    let Some(referents) = self.control.operand_referents.span(capture.referents)
                    else {
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
                        self.trace_selection(
                            value,
                            path,
                            "physical receiver projection unavailable",
                        );
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
                    let slot = crate::project_type_reference_from_segments(
                        self.program,
                        declared,
                        segments,
                    )?;
                    let supplied = crate::reference_shell(self.program, slot)?;
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
                    if place.segments.is_empty()
                        && place.root.as_symbol().is_some_and(|root| {
                            self.program
                                .state_parameter_by_symbol(state, root)
                                .is_some_and(|parameter| {
                                    !parameter.is_mutable
                                        || crate::reference_shell(
                                            self.program,
                                            parameter.type_reference,
                                        )
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
                    let Some(leaf_path) = self
                        .control
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
                    || !crate::storage_origin_types_match(
                        self.program,
                        actual.referee,
                        declared.referee,
                    )
                {
                    return Some(Selection::Unavailable);
                }
                if consumed == path.len() {
                    return Some(self.opaque_subtree(value, path));
                }
                let end = (consumed + 1..=path.len()).find(|length| {
                    crate::project_type_reference_from_segments(
                        self.program,
                        capture.type_reference,
                        &path[..*length],
                    )
                    .is_some_and(|reference| {
                        crate::reference_shell(self.program, reference).is_some()
                    })
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
                let Some(path) =
                    crate::corresponding_place_projection(self.program, reference, path)
                else {
                    return Some(Selection::Unavailable);
                };
                let path = path.as_slice();
                let query = (retention, reference, path.to_vec());
                if let Some((_, result)) =
                    self.profile_memo.iter().find(|(prior, _)| *prior == query)
                {
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
                    let Some(rows) = self.control.operand_retentions.span(profile.alternatives)
                    else {
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
                let Some(reference) = crate::project_type_reference_from_segments(
                    self.program,
                    reference,
                    &path[..length],
                ) else {
                    return Some(Selection::Unavailable);
                };
                self.profile_descriptor(profile, reference, &path[length..])
            }

            fn opaque_subtree(
                &self,
                value: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
            ) -> Selection {
                let capture = self.control.operands.get(value);
                if !crate::project_type_reference_from_segments(
                    self.program,
                    capture.type_reference,
                    path,
                )
                .is_some_and(|reference| crate::reference_shell(self.program, reference).is_some())
                {
                    return Selection::Unavailable;
                }
                let Some(profiles) = super::captured_retention_profiles_at_path(
                    self.program,
                    self.control,
                    capture,
                    path,
                ) else {
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
                        let reference = self.control.operands.get(capture).type_reference;
                        if crate::owned_projection_type_reference(
                            self.program,
                            reference,
                            &projection,
                        )
                        .is_some_and(|selected| {
                            crate::reference_shell(self.program, selected).is_some()
                        }) {
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
    }
    pub use captures::{
        captured_operand_child_at_path, captured_operand_children_at_path,
        captured_retention_profiles_at_path, finite_projection_paths, operand_child_type,
        project_captured_storage_place, project_captured_view_place,
    };
    pub use descriptor_suppliers::descriptor_suppliers;
    pub use reference_captures::{
        owned_profile_reference_sources, reference_captures_at_path, reference_sources_at_path,
        stored_reference_sources_at_path,
    };

    pub(crate) mod frozen_references {
        use arena::Handle;
        use checked_trees::FlowOperandFact;

        pub enum FrozenReferenceSelection {
            NotFrozen,
            Endpoint {
                owner: FlowOperandFact,
                leaf: checked_trees::FlowCapturedReferenceLeaf,
                path: Vec<facts::PlaceSegment>,
            },
            ThroughReference,
            Unavailable,
        }

        pub fn frozen_reference_leaf(
            program: &dyn crate::CheckingInput,
            control: &checked_trees::FlowControlFacts,
            operand: Handle<FlowOperandFact>,
            projection: &[facts::PlaceSegment],
        ) -> FrozenReferenceSelection {
            use FrozenReferenceSelection::{Endpoint, NotFrozen, ThroughReference, Unavailable};
            if !control.operands.is_valid(operand) {
                return Unavailable;
            }
            let capture = *control.operands.get(operand);
            let (owner, leaves, prefix, path) = match capture.completion {
                checked_trees::FlowOperandCompletion::Copied { copy, path } => {
                    if !control.copied_values.is_valid(copy) {
                        return Unavailable;
                    }
                    let copied = control.copied_values.get(copy);
                    if !control.operands.is_valid(copied.source) {
                        return Unavailable;
                    }
                    let Some(prefix) = control.operand_child_paths.span(path) else {
                        return Unavailable;
                    };
                    let mut path = prefix.to_vec();
                    path.extend_from_slice(projection);
                    (
                        *control.operands.get(copied.source),
                        copied.references,
                        prefix.len(),
                        path,
                    )
                }
                _ if !capture.reference_leaves.is_empty() => {
                    (capture, capture.reference_leaves, 0, projection.to_vec())
                }
                _ => return NotFrozen,
            };
            let Some(leaves) = control.captured_reference_leaves.span(leaves) else {
                return Unavailable;
            };
            let mut selected = None;
            for leaf in leaves {
                let Some(leaf_path) = control.operand_child_paths.span(leaf.path) else {
                    return Unavailable;
                };
                if leaf_path.len() <= path.len()
                    && leaf_path.iter().zip(&path).all(|(left, right)| {
                        crate::storage_path_relations::canonical_place_segments_equal(
                            program, *left, *right,
                        )
                    })
                    && selected.replace((*leaf, leaf_path)).is_some()
                {
                    return Unavailable;
                }
            }
            let Some((leaf, leaf_path)) = selected else {
                return Unavailable;
            };
            if leaf_path.len() < prefix {
                return Unavailable;
            }
            let Some(selected) = crate::project_type_reference_from_segments(
                program,
                owner.type_reference,
                leaf_path,
            ) else {
                return Unavailable;
            };
            let (Some(actual), Some(declared)) = (
                crate::reference_shell(program, selected),
                crate::reference_shell(program, leaf.type_reference),
            ) else {
                return Unavailable;
            };
            if actual.access != declared.access
                || !crate::storage_origin_types_match(program, actual.referee, declared.referee)
            {
                return Unavailable;
            }
            if leaf_path.len() < path.len() {
                if !crate::project_type_reference_from_segments(
                    program,
                    owner.type_reference,
                    &path,
                )
                .is_some_and(|reference| crate::reference_shell(program, reference).is_some())
                {
                    return Unavailable;
                }
                return ThroughReference;
            }
            Endpoint { owner, leaf, path }
        }
    }
    pub use frozen_references::{FrozenReferenceSelection, frozen_reference_leaf};
    pub(crate) mod descriptor_extents {
        use arena::Handle;
        use checked_trees::{FlowControlFacts, FlowOperandFact, FlowStorageSnapshot};
        use facts::PlaceSegment;

        pub fn descriptor_extent(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            current: Handle<FlowStorageSnapshot>,
        ) -> Option<u64> {
            let mut pending = vec![(value, path.to_vec())];
            let mut visited = Vec::new();
            let mut extent = None;
            while let Some((value, path)) = pending.pop() {
                if visited.contains(&(value, path.clone())) {
                    return None;
                }
                visited.push((value, path.clone()));
                let selected =
                    super::reference_captures_at_path(program, control, value, &path, current)?
                        .captures()?;
                if selected.is_empty() {
                    return None;
                }
                for selected in selected {
                    if !selected.projection.is_empty() {
                        pending.push((selected.capture, selected.projection));
                        continue;
                    }
                    let capture = control.operands.get(selected.capture);
                    let element =
                        crate::collection_element_type_reference(program, capture.type_reference)?;
                    let referents = control.operand_referents.span(capture.referents)?;
                    if referents.is_empty() {
                        return None;
                    }
                    for referent in referents {
                        let segments = control.operand_referent_segments.span(referent.segments)?;
                        let (prefix, range) = match segments.split_last() {
                            Some((PlaceSegment::FixedRange { start, end }, prefix)) => {
                                (prefix, Some((*start, *end)))
                            }
                            _ => (segments, None),
                        };
                        let source = crate::canonical_place_type_reference(
                            program,
                            capture.state_symbol,
                            capture.statement_index,
                            &crate::CanonicalPlace {
                                root: referent.root,
                                segments: prefix.to_vec(),
                            },
                        )?;
                        let source_element =
                            crate::collection_element_type_reference(program, source)?;
                        if program.normalized_type_identity(element)
                            != program.normalized_type_identity(source_element)
                        {
                            return None;
                        }
                        let count = match range {
                            Some((start, end)) => {
                                let end = if end == usize::MAX {
                                    crate::fixed_array_type_length(program, source)?
                                } else {
                                    end
                                };
                                u64::try_from(end.checked_sub(start)?).ok()?
                            }
                            None => u64::try_from(crate::fixed_array_type_length(program, source)?)
                                .ok()?,
                        };
                        if extent.is_some_and(|prior| prior != count) {
                            return None;
                        }
                        extent = Some(count);
                    }
                }
            }
            extent
        }
    }
    pub use descriptor_extents::descriptor_extent;

    pub(crate) mod predecessors {
        use arena::Handle;
        use checked_trees::FlowOperandFact;
        use checked_trees::FlowStatementFact;
        use facts::PlaceSegment;

        pub fn terminal_alternatives(
            control: &checked_trees::FlowControlFacts,
            value: Handle<FlowOperandFact>,
        ) -> Option<Vec<Handle<FlowOperandFact>>> {
            let mut pending = vec![value];
            let mut visited = Vec::new();
            let mut completed = Vec::new();
            while let Some(value) = pending.pop() {
                if !control.operands.is_valid(value) || visited.contains(&value) {
                    return None;
                }
                visited.push(value);
                let capture = control.operands.get(value);
                if let checked_trees::FlowOperandCompletion::Selected(alternatives) =
                    capture.completion
                {
                    let rows = control.operand_alternatives.span(alternatives)?;
                    if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                        return None;
                    }
                    pending.extend(rows.iter().map(|row| row.value));
                } else {
                    completed.push(value);
                }
            }
            Some(completed)
        }

        pub fn predecessors(
            program: &dyn crate::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            value: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            predecessors_in_flow(
                program,
                facts.flow,
                facts.semantic,
                value,
                projection,
                super::StoredObservation::Contents,
            )
        }

        pub fn predecessors_in_flow(
            program: &dyn crate::CheckingInput,
            flow: checked_trees::FlowFactsRef<'_>,
            _semantic: &facts::FactPlan,
            value: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
            observation: super::StoredObservation,
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            predecessors_in_control(program, flow.control, value, projection, observation, None)
        }

        pub fn predecessors_in_control(
            program: &dyn crate::CheckingInput,
            control: &checked_trees::FlowControlFacts,
            value: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
            observation: super::StoredObservation,
            active_statements: Option<(
                symbols::SymbolHandle,
                arena::HandleSpan<FlowStatementFact>,
            )>,
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            if !control.operands.is_valid(value) {
                return None;
            }
            let capture = control.operands.get(value);
            let crosses_reference = (0..=projection.len()).any(|length| {
                crate::project_type_reference_from_segments(
                    program,
                    capture.type_reference,
                    &projection[..length],
                )
                .is_some_and(|reference| crate::reference_shell(program, reference).is_some())
            });
            if capture.value_source.is_valid()
                && (matches!(observation, super::StoredObservation::Binding) || !crosses_reference)
            {
                return control
                    .operands
                    .is_valid(capture.value_source)
                    .then(|| vec![(capture.value_source, projection.to_vec())]);
            }
            if let checked_trees::FlowOperandCompletion::Copied { copy, path } = capture.completion
                && (matches!(observation, super::StoredObservation::Binding) || !crosses_reference)
            {
                if !control.copied_values.is_valid(copy) {
                    return None;
                }
                let copied = control.copied_values.get(copy);
                let mut selected = control.operand_child_paths.span(path)?.to_vec();
                selected.extend_from_slice(projection);
                return Some(vec![(copied.source, selected)]);
            }
            if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion
            {
                let rows = control.operand_alternatives.span(alternatives)?;
                if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                    return None;
                }
                return Some(
                    rows.iter()
                        .map(|row| (row.value, projection.to_vec()))
                        .collect(),
                );
            }
            if !projection.is_empty()
                && let Some((selected, _, consumed)) = super::captured_operand_child_at_path(
                    program,
                    control,
                    capture,
                    capture.type_reference,
                    projection,
                )
                && selected != value
            {
                return Some(vec![(selected, projection[consumed..].to_vec())]);
            }
            let state = program.state_by_symbol(capture.state_symbol)?;
            if let Some((receiver, mut path)) =
                owned_observation_projection(program, control, value, active_statements)?
            {
                path.extend_from_slice(projection);
                return Some(vec![(receiver, path)]);
            }
            let mut source = crate::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                capture.statement_index,
                capture.expression,
            )?;
            source.segments.extend_from_slice(projection);
            let captures = super::at_snapshot(
                program,
                control,
                capture.storage_snapshot,
                &source,
                observation,
            )?
            .captures()?;
            let mut predecessors = Vec::new();
            for selected in captures {
                if !selected.reference.is_valid() {
                    return None;
                }
                predecessors.push((selected.capture, selected.projection));
            }
            Some(predecessors)
        }

        pub fn owned_observation_projection(
            program: &dyn crate::CheckingInput,
            control: &checked_trees::FlowControlFacts,
            value: Handle<FlowOperandFact>,
            active_statements: Option<(
                symbols::SymbolHandle,
                arena::HandleSpan<FlowStatementFact>,
            )>,
        ) -> Option<Option<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            if !control.operands.is_valid(value) {
                return None;
            }
            let capture = control.operands.get(value);
            let Some(member) = program.expression_table.member_node(capture.expression) else {
                return Some(None);
            };
            let statements = active_statements
                .filter(|(state, _)| *state == capture.state_symbol)
                .map(|(_, statements)| statements)
                .or_else(|| {
                    control
                        .state_by_symbol(capture.state_symbol)
                        .map(|state| state.statements)
                })?;
            let statement = control
                .statements
                .span(statements)?
                .iter()
                .find(|statement| statement.statement_index == capture.statement_index)?;
            // Publishing the member's own facts can extend its constraints after
            // the receiver was observed. The retained final result identifies that
            // exact completion without requiring the earlier prefix to equal it.
            let mut observations = control
                .value_observations
                .span_or_empty(statement.value_observations)
                .iter()
                .filter(|observation| {
                    observation.expression == capture.expression
                        && observation.evaluation == capture.evaluation
                        && if observation.result.is_valid() {
                            observation.result == value
                                || (capture.value_source == observation.result
                                    && control.operands.is_valid(observation.result)
                                    && {
                                        let source = control.operands.get(observation.result);
                                        source.state_symbol == capture.state_symbol
                                            && source.statement_index == capture.statement_index
                                            && source.evaluation == capture.evaluation
                                            && source.expression == capture.expression
                                            && source.storage_snapshot == capture.storage_snapshot
                                            && source.type_reference == capture.type_reference
                                    })
                        } else {
                            observation.constraints == capture.constraints
                        }
                });
            let Some(observation) = observations.next() else {
                return Some(None);
            };
            if observations.next().is_some() || !control.operands.is_valid(observation.receiver) {
                return None;
            }
            let receiver = control.operands.get(observation.receiver);
            if crate::reference_shell(program, receiver.type_reference).is_some() {
                return Some(None);
            }
            let symbol = crate::effective_member_symbol(program, member.receiver, member);
            if !symbol.is_valid() || program.any_field_owner(symbol).is_none() {
                return None;
            }
            let mut path = Vec::new();
            if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                path.push(PlaceSegment::Case { variant });
            }
            path.push(PlaceSegment::Field { symbol });
            Some(Some((observation.receiver, path)))
        }
    }
    pub use predecessors::{
        owned_observation_projection, predecessors, predecessors_in_control, predecessors_in_flow,
        terminal_alternatives,
    };
    pub(crate) mod memberships {
        use arena::Handle;
        use checked_trees::{CheckFacts, FlowOperandFact};
        use facts::PlaceSegment;
        use language_semantics::SemanticDomainId;
        use symbols::SymbolHandle;

        #[derive(Clone, Copy)]
        pub struct CompletedMembershipRefusal {
            pub reason: &'static str,
        }

        pub(crate) struct MembershipWalk<'observer> {
            visiting: Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
            remaining: usize,
            on_refusal: &'observer mut dyn FnMut(&'static str),
        }

        /// A completed checked call may return a qualification proved on its raw
        /// result, even when that qualification is not an authored callee ensures.
        /// Read every actual normal return; never reintroduce a moved caller input
        /// or use an earlier callee entry value as its completed result.
        fn checked_invocation_returns(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            state: SymbolHandle,
            call: &checked_trees::FlowCallFact,
            reference: typed_trees::types::TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> Option<Vec<Handle<FlowOperandFact>>> {
            let control = &facts.flow.control;
            let target = crate::call_target_state(program, call.target_symbol)?;
            let machine = program.machine_holding_state(target.symbol)?;
            let caller = program.machine_holding_state(state)?;
            let site = crate::find_call_site(
                program,
                caller.symbol,
                state,
                call.statement_index,
                call.call_ordinal,
            )?;
            if crate::call_target_entry_symbol(program, site.target_symbol(program))
                != target.symbol
                || site.static_machine_parameter().is_valid()
                || site.static_requirement_dispatch().is_some()
                || control.call_arguments.span(call.authored_arguments)
                    != Some(crate::call_site_argument_expressions(program, &site))
            {
                return None;
            }
            match &site {
                crate::CallSite::Expression {
                    expression,
                    call: source,
                } => {
                    if *expression != call.authored_expression
                        || source.requirement_application.is_valid()
                    {
                        return None;
                    }
                    // A selected requirement or callable binder cannot expose its
                    // implementation's stronger private result qualifications.
                    let (declaration, _) =
                        crate::authored_call_parameters(program, caller.symbol, &site).ok()?;
                    program.machine_by_symbol(declaration)?;
                }
                crate::CallSite::TransitionNamed { .. } => {
                    let transition = program
                        .statement_table
                        .statement_at(program.state_by_symbol(state)?, call.statement_index)?
                        .as_transition()?;
                    if transition.exit != typed_trees::statement::TransitionExit::Ordinary
                        || !call.authored_closing_invocation
                        || ![transition.target, transition.continuation]
                            .into_iter()
                            .filter(|target| target.is_valid())
                            .any(|target| site.completes_transition(program, target))
                    {
                        return None;
                    }
                }
                crate::CallSite::Statement(_) => return None,
            }
            if !call.authored_source_custody_valid
                || call.static_requirement_dispatch.is_some()
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !reference.is_valid()
                || !target.return_type.is_valid()
                || program.normalized_type_identity(reference)
                    != program.normalized_type_identity(target.return_type)
            {
                return None;
            }
            for length in 0..=path.len() {
                let selected = crate::project_type_reference_from_segments(
                    program,
                    target.return_type,
                    &path[..length],
                )?;
                if crate::reference_shell(program, selected).is_some()
                    || !crate::borrow_contracts::borrow_carrying_owner_paths(program, selected)?
                        .is_empty()
                {
                    return None;
                }
            }
            let flow = control.unique_state(machine.symbol, target.symbol)?;
            let mut returned = Vec::new();
            for outgoing in control.outgoing.span(flow.outgoing)? {
                if outgoing.cannot_return_normally {
                    continue;
                }
                if outgoing.transition_target.is_valid() {
                    let transition = program
                        .statement_table
                        .statement_at(target, outgoing.statement_index)?
                        .as_transition()?;
                    if transition.exit != typed_trees::statement::TransitionExit::Ordinary {
                        continue;
                    }
                }
                if outgoing.machine_symbol != machine.symbol
                    || outgoing.state_symbol != target.symbol
                    || !control.operands.is_valid(outgoing.returned_value)
                {
                    return None;
                }
                let captured = control.operands.get(outgoing.returned_value);
                if captured.state_symbol != target.symbol
                    || captured.statement_index != outgoing.statement_index
                    || !captured.type_reference.is_valid()
                    || program.normalized_type_identity(captured.type_reference)
                        != program.normalized_type_identity(target.return_type)
                {
                    return None;
                }
                returned.push(outgoing.returned_value);
            }
            (!returned.is_empty()).then_some(returned)
        }

        fn checked_call_returns(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            value: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<Vec<Handle<FlowOperandFact>>> {
            let control = &facts.flow.control;
            let call = if let checked_trees::FlowOperandCompletion::InvocationResult {
                invocation,
                result_type,
                path: prefix,
            } = value.completion
            {
                let flow = control.state_by_symbol(value.state_symbol)?;
                if !control.calls.is_valid(invocation)
                    || !flow.calls.handles().any(|handle| handle == invocation)
                    || !control.operand_retained_paths.span(prefix)?.is_empty()
                    || !result_type.is_valid()
                    || program.normalized_type_identity(result_type)
                        != program.normalized_type_identity(value.type_reference)
                {
                    return None;
                }
                control.calls.get(invocation)
            } else {
                super::versions::unique_call(
                    program,
                    control,
                    value.state_symbol,
                    value.expression,
                )?
            };
            if call.statement_index != value.statement_index
                || call.completed_storage != value.storage_snapshot
            {
                return None;
            }
            checked_invocation_returns(
                program,
                facts,
                value.state_symbol,
                call,
                value.type_reference,
                path,
            )
        }

        fn checked_named_exit_returns(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            exit: &checked_trees::FlowExitFact,
            path: &[PlaceSegment],
        ) -> Option<Vec<Handle<FlowOperandFact>>> {
            if !exit.transition_target.is_valid() {
                return None;
            }
            let control = &facts.flow.control;
            let flow = control.unique_state(exit.machine_symbol, exit.state_symbol)?;
            let mut calls = control.calls.span(flow.calls)?.iter().filter(|call| {
                call.statement_index == exit.statement_index
                    && call.authored_site == checked_trees::FlowCallSite::NamedTransition
                    && crate::find_call_site(
                        program,
                        exit.machine_symbol,
                        exit.state_symbol,
                        exit.statement_index,
                        call.call_ordinal,
                    )
                    .is_some_and(|site| site.completes_transition(program, exit.transition_target))
            });
            let call = calls.next()?;
            if calls.next().is_some() {
                return None;
            }
            checked_invocation_returns(
                program,
                facts,
                exit.state_symbol,
                call,
                program.state_by_symbol(exit.state_symbol)?.return_type,
                path,
            )
        }

        pub fn completed_membership(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
        ) -> bool {
            judge_completed_membership(
                program,
                facts,
                std::slice::from_ref(&value),
                path,
                domain,
                semantic_domain,
                &mut |_| {},
            )
        }

        pub fn completed_membership_with_refusal(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            exit: &checked_trees::FlowExitFact,
            path: &[PlaceSegment],
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
            first_refusal: &mut Option<CompletedMembershipRefusal>,
        ) -> bool {
            let values = if facts.flow.control.operands.is_valid(exit.returned_value) {
                vec![exit.returned_value]
            } else if let Some(returned) = checked_named_exit_returns(program, facts, exit, path) {
                returned
            } else {
                return false;
            };
            judge_completed_membership(
                program,
                facts,
                &values,
                path,
                domain,
                semantic_domain,
                &mut |reason| {
                    first_refusal.get_or_insert(CompletedMembershipRefusal { reason });
                },
            )
        }

        fn judge_completed_membership(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            values: &[Handle<FlowOperandFact>],
            path: &[PlaceSegment],
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
            on_refusal: &mut dyn FnMut(&'static str),
        ) -> bool {
            fn proves(
                program: &dyn crate::CheckingInput,
                facts: &CheckFacts,
                value: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
                domain: SymbolHandle,
                semantic_domain: SemanticDomainId,
                walk: &mut MembershipWalk<'_>,
            ) -> bool {
                if walk.remaining == 0 || walk.visiting.len() >= 128 {
                    (walk.on_refusal)("completed membership traversal exhausted");
                    return false;
                }
                walk.remaining -= 1;
                if !facts.flow.control.operands.is_valid(value)
                    || walk
                        .visiting
                        .iter()
                        .any(|(seen, projection)| *seen == value && projection == path)
                {
                    return false;
                }
                if facts
                    .qualifications
                    .content
                    .establishes_completed_membership(
                        &facts.flow,
                        value,
                        path,
                        domain,
                        semantic_domain,
                    )
                {
                    return true;
                }
                match super::owned_memberships::transferred_membership(
                    program,
                    facts,
                    value,
                    path,
                    domain,
                    semantic_domain,
                ) {
                    Some(Ok(())) => return true,
                    Some(Err(reason)) => {
                        (walk.on_refusal)(reason);
                        if crate::trace_flags::struct_trace() {
                            let captured = facts.flow.control.operands.get(value);
                            eprintln!(
                                "STRUCT owned-membership value={value:?} state={:?} statement={} lane={:?} path_len={} storage_cut={:?} domain={domain:?} reason={reason}",
                                captured.state_symbol,
                                captured.statement_index,
                                captured.evaluation,
                                path.len(),
                                captured.storage_snapshot,
                            );
                        }
                    }
                    None => {}
                }
                let predecessors = checked_call_returns(
                    program,
                    facts,
                    facts.flow.control.operands.get(value),
                    path,
                )
                .map(|returned| {
                    returned
                        .into_iter()
                        .map(|returned| (returned, path.to_vec()))
                        .collect()
                })
                .or_else(|| super::predecessors(program, facts.view(), value, path));
                let Some(predecessors) = predecessors else {
                    return false;
                };
                walk.visiting.push((value, path.to_vec()));
                let proven = !predecessors.is_empty()
                    && predecessors.iter().all(|(previous, projection)| {
                        proves(
                            program,
                            facts,
                            *previous,
                            projection,
                            domain,
                            semantic_domain,
                            walk,
                        )
                    });
                walk.visiting.pop();
                proven
            }
            let mut walk = MembershipWalk {
                visiting: Vec::new(),
                remaining: 65_536,
                on_refusal,
            };
            !values.is_empty()
                && values.iter().all(|value| {
                    proves(
                        program,
                        facts,
                        *value,
                        path,
                        domain,
                        semantic_domain,
                        &mut walk,
                    )
                })
        }
    }
    pub(crate) mod owned_memberships {
        use arena::Handle;
        use checked_trees::{CheckFacts, FlowOperandFact};
        use facts::PlaceSegment;
        use language_semantics::content::ContentProjectionPlan;
        use language_semantics::{
            PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
            PermissionProvenance, SemanticDomainId,
        };
        use symbols::SymbolHandle;

        pub(crate) fn transferred_membership(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            domain: SymbolHandle,
            semantic_domain: SemanticDomainId,
        ) -> Option<Result<(), &'static str>> {
            let control = &facts.flow.control;
            if !control.operands.is_valid(value)
                || program
                    .expression_table
                    .cast_node(control.operands.get(value).expression)
                    .is_none()
            {
                return None;
            }
            let mut projections = facts
                .qualifications
                .content
                .plans
                .iter()
                .filter(|projection| {
                    semantic_domain.is_valid()
                        && projection.domain == domain
                        && projection.semantic_domain == semantic_domain
                });
            let projection = projections.next()?;
            if projections.next().is_some() {
                return Some(Err("owned membership has ambiguous content projections"));
            }
            Some(preserved_transfer(program, facts, value, path, projection))
        }

        fn preserved_transfer(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            projection: &ContentProjectionPlan,
        ) -> Result<(), &'static str> {
            let domain = projection.domain;
            let semantic_domain = projection.semantic_domain;
            let control = &facts.flow.control;
            if !control.operands.is_valid(value) {
                return Err("owned transfer has no completed value");
            }
            let completed = control.operands.get(value);
            let mut source = completed;
            let mut source_handle = value;
            let mut transferred = false;
            while let Some(expression) = crate::value_preserving_cast_operand(
                program,
                source.state_symbol,
                source.statement_index,
                source.expression,
            ) {
                let [input] = control
                    .operands
                    .span(source.evaluated_inputs)
                    .ok_or("owned cast has no evaluated operand")?
                else {
                    return Err("owned cast does not have one evaluated operand");
                };
                if input.expression != expression
                    || input.state_symbol != source.state_symbol
                    || input.statement_index != source.statement_index
                    || input.evaluation != source.evaluation
                    || source
                        .conditional_inputs_from
                        .is_some_and(|prefix| prefix != 1)
                    || !crate::declared_carriers_match(
                        program,
                        input.type_reference,
                        source.type_reference,
                    )
                {
                    return Err("owned cast lost its exact evaluated operand");
                }
                let input_handle = source
                    .evaluated_inputs
                    .handles()
                    .next()
                    .ok_or("owned cast has no evaluated operand handle")?;
                if input_handle.arena_index() >= source_handle.arena_index() {
                    return Err("owned cast input does not precede its completion");
                }
                source_handle = input_handle;
                source = input;
                transferred = true;
            }
            if !transferred {
                return Err("completed value is not an owned qualification transfer");
            }
            let mut place = crate::canonical_place_from_expression_in_state(
                program,
                source.state_symbol,
                source.statement_index,
                source.expression,
            )
            .ok_or("owned input has no exact semantic place")?;
            let owning_source = crate::ownership_source_expression(
                program,
                completed.state_symbol,
                completed.statement_index,
                completed.expression,
            );
            if owning_source != source.expression {
                return Err("owned transfer lost its captured ownership source");
            }
            let mut owned = crate::ownership_place_from_expression_in_state(
                program,
                completed.state_symbol,
                completed.statement_index,
                owning_source,
            )
            .ok_or("owned transfer has no exact source place")?;
            place.segments.extend_from_slice(path);
            owned.segments.extend_from_slice(path);
            for length in 0..=path.len() {
                let reference = crate::project_type_reference_from_segments(
                    program,
                    source.type_reference,
                    &path[..length],
                )
                .ok_or("owned membership has an unresolved typed projection")?;
                if crate::reference_shell(program, reference).is_some() {
                    return Err("owned membership cannot cross a borrowed referent");
                }
                if length == path.len() {
                    let carrier = program
                        .unwrapped_type_reference(reference)
                        .ok_or("owned membership has no complete carrier")?;
                    if program.normalized_type_identity_text(carrier) != projection.carrier_identity
                    {
                        return Err("owned membership has a different projection carrier");
                    }
                }
            }
            if !control.storage_snapshots.is_valid(source.storage_snapshot)
                || !control
                    .storage_snapshots
                    .is_valid(completed.storage_snapshot)
                || (source.storage_snapshot != completed.storage_snapshot
                    && !super::contents_preserved_from(
                        program,
                        control,
                        completed.storage_snapshot,
                        &owned,
                        &[source.storage_snapshot],
                        super::StoredObservation::InlineContents,
                    ))
            {
                return Err("owned input changed after its captured membership");
            }
            let known = facts
                .flow
                .semantic_constraint_contexts(source.constraints)
                .any(|context| {
                    facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(context))
                        .facts()
                        .any(|fact| {
                            let Some((_, _, candidate, instance)) =
                                fact.payload.domain_membership()
                            else {
                                return false;
                            };
                            candidate == domain
                                && instance == semantic_domain
                                && fact.place.as_place().is_some_and(|handle| {
                                    crate::membership_evidence::canonical_place_from_semantic_place(
                                        program,
                                        &facts.semantic,
                                        facts.semantic.places.get(handle),
                                    )
                                    .is_some_and(|source| source == place)
                                })
                        })
                });
            if !known {
                return Err("captured input has no live exact content membership");
            }
            let machine = program
                .machine_holding_state(completed.state_symbol)
                .ok_or("owned transfer has no machine")?;
            let state = control
                .unique_state(machine.symbol, completed.state_symbol)
                .ok_or("owned transfer has no unique state occurrence")?;
            if !super::consumer_captures(
                control,
                completed.state_symbol,
                completed.statement_index,
                completed.expression,
            )
            .is_some_and(|captures| captures.contains(&value))
            {
                return Err("owned transfer is not an actual completed consumer operand");
            }
            if completed.evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                return Err("owned transfer has no actual evaluation lane");
            }
            let mut identity = None;
            let permissions = facts
                .flow
                .ownership
                .with_state_permissions(completed.state_symbol, |handles| handles.to_vec());
            for handle in permissions {
                let event = facts.flow.ownership.permissions.get(handle);
                if event.machine_symbol != machine.symbol
                    || event.state_symbol != completed.state_symbol
                    || event.expression != source.expression
                    || event.evaluation != completed.evaluation
                    || event.root != owned.root
                    || facts.flow.ownership.segments.span_or_empty(event.segments) != owned.segments
                {
                    continue;
                }
                let statement = match event.source {
                    PermissionEventSource::Statement { statement_index } => statement_index,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        target_symbol,
                    } => {
                        if !control.calls.span_or_empty(state.calls).iter().any(|call| {
                            call.statement_index == statement_index
                                && call.call_ordinal == call_ordinal
                                && call.target_symbol == target_symbol
                                && call.operands.handles().any(|operand| operand == value)
                        }) {
                            return Err("owned transfer has no exact consuming invocation");
                        }
                        statement_index
                    }
                    _ => continue,
                };
                if statement != completed.statement_index
                    || event.kind != PermissionEventKind::Transfer
                {
                    continue;
                }
                if event.access != PermissionAccess::Owned
                    || !event.obligation_live
                    || event.claim_identity == PermissionClaimIdentity::Unknown
                    || event.provenance == PermissionProvenance::Unknown
                    || identity.is_some_and(|previous| previous != event.claim_identity)
                {
                    return Err("owned transfer has unavailable or ambiguous claim custody");
                }
                identity = Some(event.claim_identity);
            }
            identity
                .map(|_| ())
                .ok_or("completed cast has no live owned claim transfer")
        }
    }
    pub use memberships::{
        CompletedMembershipRefusal, completed_membership, completed_membership_with_refusal,
    };

    pub(crate) mod occurrences {
        use arena::Handle;
        use checked_trees::{FlowControlFacts, FlowOperandFact};

        pub fn consumer_captures(
            control: &FlowControlFacts,
            state: symbols::SymbolHandle,
            statement: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<Vec<Handle<FlowOperandFact>>> {
            let state = control.state_by_symbol(state)?;
            let mut pending = Vec::new();
            for row in control
                .statements
                .span_or_empty(state.statements)
                .iter()
                .filter(|row| row.statement_index == statement)
            {
                pending.extend([row.stored_value, row.store_target]);
                for invocation in control
                    .operator_invocations
                    .span(row.operator_invocations)?
                {
                    pending.extend(invocation.operands.handles());
                }
                for observation in control
                    .value_observations
                    .span_or_empty(row.value_observations)
                {
                    pending.extend([observation.result, observation.receiver]);
                }
            }
            for call in control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .filter(|row| row.statement_index == statement)
            {
                pending.extend(call.operands.handles());
            }
            for outgoing in control
                .outgoing
                .span_or_empty(state.outgoing)
                .iter()
                .filter(|row| row.statement_index == statement)
            {
                pending.push(outgoing.returned_value);
            }
            for exit in control
                .exits
                .span_or_empty(state.exits)
                .iter()
                .filter(|row| row.statement_index == statement)
            {
                pending.push(exit.returned_value);
            }
            let mut visited = Vec::new();
            let mut selected = Vec::new();
            while let Some(handle) = pending.pop() {
                if !handle.is_valid() {
                    continue;
                }
                if !control.operands.is_valid(handle) {
                    return None;
                }
                if visited.contains(&handle) {
                    continue;
                }
                visited.push(handle);
                let operand = control.operands.get(handle);
                if operand.state_symbol == state.state_symbol
                    && operand.statement_index == statement
                    && operand.expression == expression
                {
                    selected.push(handle);
                }
                pending.extend(operand.evaluated_inputs.handles());
                if operand.backing.is_valid()
                    && control.operands.get(operand.backing).statement_index == statement
                {
                    pending.push(operand.backing);
                }
                if operand.value_source.is_valid()
                    && control.operands.get(operand.value_source).statement_index == statement
                {
                    pending.push(operand.value_source);
                }
                pending.extend(
                    control
                        .operand_children
                        .span(operand.constructed_children())?
                        .iter()
                        .map(|child| child.value),
                );
                if let checked_trees::FlowOperandCompletion::Selected(alternatives) =
                    operand.completion
                {
                    let rows = control.operand_alternatives.span(alternatives)?;
                    if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                        return None;
                    }
                    pending.extend(rows.iter().map(|row| row.value));
                }
            }
            (!selected.is_empty()).then_some(selected)
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::consumer_captures;
            use arena::Handle;
            use checked_trees::{FlowCallFact, FlowControlFacts, FlowOperandFact, FlowStateFact};

            #[test]
            fn captures_follow_evaluated_sources_but_not_unrelated_history() {
                let state = Handle::from_arena_index(1);
                let expression = Handle::from_arena_index(2);
                let mut control = FlowControlFacts::default();
                let source = FlowOperandFact {
                    state_symbol: state,
                    statement_index: 3,
                    expression,
                    ..Default::default()
                };
                let sources = control.operands.insert_many([source]);
                let source_handle = sources.handles().next().unwrap();
                control.operands.insert(source);
                let previous = control.operands.insert(FlowOperandFact {
                    statement_index: 2,
                    ..source
                });
                let actuals = control.operands.insert_many([FlowOperandFact {
                    state_symbol: state,
                    statement_index: 3,
                    expression: Handle::from_arena_index(4),
                    evaluated_inputs: sources,
                    value_source: previous,
                    ..Default::default()
                }]);
                let calls = control.calls.insert_many([FlowCallFact {
                    statement_index: 3,
                    operands: actuals,
                    ..Default::default()
                }]);
                control.states.insert(FlowStateFact {
                    state_symbol: state,
                    calls,
                    ..Default::default()
                });
                assert_eq!(
                    consumer_captures(&control, state, 3, expression),
                    Some(vec![source_handle])
                );
                assert_eq!(consumer_captures(&control, state, 2, expression), None);
            }
        }
    }
    pub use occurrences::consumer_captures;

    pub(crate) mod loan_homes {
        use arena::{Handle, HandleSpan};
        use checked_trees::{
            BorrowFacts, BorrowLoanFact, BorrowLoanOwnerSegment, FlowControlFacts, FlowOperandFact,
            FlowStatementFact,
        };
        use facts::PlaceSegment;
        use std::borrow::Cow;

        pub fn descriptor_formation_sources(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            statements: HandleSpan<FlowStatementFact>,
        ) -> Option<Vec<super::StoredCapture>> {
            if !control.operands.is_valid(value) {
                return None;
            }
            let root = control.operands.get(value);
            let mut pending = vec![(value, path.to_vec())];
            let mut visited = Vec::new();
            let mut sources = Vec::new();
            let mut remaining = control.operands.len().checked_mul(3)?.checked_add(1)?;
            while let Some((capture, projection)) = pending.pop() {
                if remaining == 0 {
                    return None;
                }
                remaining -= 1;
                if visited.contains(&(capture, projection.clone())) {
                    continue;
                }
                visited.push((capture, projection.clone()));
                if !control.operands.is_valid(capture) {
                    return None;
                }
                let source = control.operands.get(capture);
                if source.state_symbol != root.state_symbol
                    || source.statement_index != root.statement_index
                {
                    continue;
                }
                if !crate::owned_projection_type_reference(
                    program,
                    source.type_reference,
                    &projection,
                )
                .is_some_and(|selected| crate::reference_shell(program, selected).is_some())
                {
                    continue;
                }
                sources.push(super::StoredCapture {
                    capture,
                    reference: source.type_reference,
                    projection: projection.clone(),
                    slot: None,
                });
                if let Some(previous) = super::predecessors_in_control(
                    program,
                    control,
                    capture,
                    &projection,
                    super::StoredObservation::Binding,
                    Some((root.state_symbol, statements)),
                ) {
                    pending.extend(previous);
                }
            }
            (!sources.is_empty()).then_some(sources)
        }

        pub fn descriptor_formation_matches_loan(
            program: &dyn crate::CheckingInput,
            control: &FlowControlFacts,
            borrow: &BorrowFacts,
            state: symbols::SymbolHandle,
            statements: HandleSpan<FlowStatementFact>,
            loan: &BorrowLoanFact,
            value: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
        ) -> Option<bool> {
            if !control.operands.is_valid(value) {
                return None;
            }
            let installed = control.operands.get(value);
            if installed.state_symbol != state || installed.statement_index != loan.statement_index
            {
                return Some(false);
            }
            let formed = place_segments(borrow.owner_segments.span(loan.formation_owner_path?)?);
            let sources = descriptor_formation_sources(program, control, value, path, statements)?;
            Some(sources.iter().any(|source| {
                let capture = control.operands.get(source.capture);
                capture.expression == loan.formation_expression && source.projection == formed
            }))
        }

        #[derive(Clone, Copy)]
        pub struct InstalledLoanHomes<'a> {
            pub control: &'a FlowControlFacts,
            pub semantic: &'a facts::FactPlan,
            pub borrow: &'a BorrowFacts,
            pub statements: HandleSpan<FlowStatementFact>,
            pub order: &'a typed_trees::statement::StatementExecutionOrder,
            pub state: symbols::SymbolHandle,
        }

        impl<'a> InstalledLoanHomes<'a> {
            pub fn owner_path(
                &self,
                program: &dyn crate::CheckingInput,
                loan: &BorrowLoanFact,
            ) -> Cow<'a, [BorrowLoanOwnerSegment]> {
                let owner = self.borrow.loan_owner_path(loan);
                if !owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
                    return Cow::Borrowed(owner);
                }
                self.installed_owner(program, loan)
                    .map(Cow::Owned)
                    .unwrap_or_else(|| Cow::Borrowed(self.borrow.loan_owner_path(loan)))
            }

            fn installed_owner(
                &self,
                program: &dyn crate::CheckingInput,
                loan: &BorrowLoanFact,
            ) -> Option<Vec<BorrowLoanOwnerSegment>> {
                let position = self.order.position(loan.statement_index)?;
                if position >= self.statements.count() as usize {
                    return None;
                }
                let completed = self
                    .control
                    .statements
                    .get(self.statements.handle_at(position as u32));
                if completed.statement_index != loan.statement_index
                    || !self.control.operands.is_valid(completed.stored_value)
                    || !self.semantic.places.is_valid(completed.stored_destination)
                {
                    return None;
                }
                let value = self.control.operands.get(completed.stored_value);
                if value.state_symbol != self.state || value.statement_index != loan.statement_index
                {
                    return None;
                }
                let installed = crate::membership_evidence::canonical_place_from_semantic_place(
                    program,
                    self.semantic,
                    self.semantic.places.get(completed.stored_destination),
                )?;
                let owner = self.borrow.loan_owner_path(loan);
                if installed.root != loan.storage_owner || installed.segments.len() > owner.len() {
                    return None;
                }
                let mut prefix = Vec::with_capacity(owner.len());
                for (declared, selected) in owner.iter().zip(&installed.segments) {
                    let captured = static_owner(*selected)?;
                    if !owner_segment_agrees(program, *declared, captured) {
                        return None;
                    }
                    prefix.push(captured);
                }
                let suffix = &owner[installed.segments.len()..];
                let path = place_segments(suffix);
                if !descriptor_formation_matches_loan(
                    program,
                    self.control,
                    self.borrow,
                    self.state,
                    self.statements,
                    loan,
                    completed.stored_value,
                    &path,
                )? {
                    return None;
                }
                prefix.extend_from_slice(suffix);
                Some(prefix)
            }
        }

        fn static_owner(segment: PlaceSegment) -> Option<BorrowLoanOwnerSegment> {
            Some(match segment {
                PlaceSegment::Field { symbol } if symbol.is_valid() => {
                    BorrowLoanOwnerSegment::Field(symbol)
                }
                PlaceSegment::Case { variant } if variant.is_valid() => {
                    BorrowLoanOwnerSegment::Case(variant)
                }
                PlaceSegment::FixedIndex { index } => BorrowLoanOwnerSegment::FixedIndex(index),
                _ => return None,
            })
        }

        fn owner_segment_agrees(
            program: &dyn crate::CheckingInput,
            declared: BorrowLoanOwnerSegment,
            captured: BorrowLoanOwnerSegment,
        ) -> bool {
            match (declared, captured) {
                (BorrowLoanOwnerSegment::DynamicIndex, BorrowLoanOwnerSegment::FixedIndex(_)) => {
                    true
                }
                (BorrowLoanOwnerSegment::Field(left), BorrowLoanOwnerSegment::Field(right)) => {
                    left == right
                        || program
                            .any_field_owner(left)
                            .zip(program.any_field_owner(right))
                            .is_some_and(|(left, right)| {
                                left.1.path_identity() == right.1.path_identity()
                            })
                }
                _ => declared == captured,
            }
        }

        pub(crate) fn place_segments(path: &[BorrowLoanOwnerSegment]) -> Vec<PlaceSegment> {
            path.iter()
                .map(|segment| match segment {
                    BorrowLoanOwnerSegment::Field(symbol) => {
                        PlaceSegment::Field { symbol: *symbol }
                    }
                    BorrowLoanOwnerSegment::Case(variant) => {
                        PlaceSegment::Case { variant: *variant }
                    }
                    BorrowLoanOwnerSegment::FixedIndex(index) => {
                        PlaceSegment::FixedIndex { index: *index }
                    }
                    BorrowLoanOwnerSegment::DynamicIndex => PlaceSegment::Index {
                        expression: typed_trees::expression::ExpressionHandle::invalid(),
                    },
                })
                .collect()
        }
    }
    pub use loan_homes::{
        InstalledLoanHomes, descriptor_formation_matches_loan, descriptor_formation_sources,
    };
    pub(crate) mod call_stores {
        use arena::Handle;
        use checked_trees::{BorrowLoanFact, BorrowLoanFormation, CheckFacts, FlowCallFact};
        use symbols::SymbolHandle;

        pub struct CompletedCallDescriptor {
            pub call: Handle<FlowCallFact>,
            pub loan: Handle<BorrowLoanFact>,
            pub source: CompletedCallDescriptorSource,
            pub origin: CompletedCallDescriptorOrigin,
        }

        #[derive(Clone)]
        pub enum CompletedCallDescriptorSource {
            Captured(super::StoredCapture),
            Incoming(crate::CanonicalPlace),
        }

        pub enum CompletedCallDescriptorOrigin {
            Produced,
            Retained,
        }

        pub fn loan_call_descriptors(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            loan: Handle<BorrowLoanFact>,
        ) -> Result<Option<Vec<CompletedCallDescriptor>>, &'static str> {
            let borrow = &facts.borrow;
            let owner = borrow
                .unique_state_borrow(machine, state)
                .ok_or("the installed reference has no unique borrowing state")?;
            if !borrow.state_owns_loan(owner, loan) {
                return Err("the installed reference belongs to another borrowing state");
            }
            if borrow.loans.get(loan).formation != BorrowLoanFormation::ReturnedDescriptor {
                return Ok(None);
            }
            let flow = &facts.flow;
            let control = &flow.control;
            let flow_state = control
                .unique_state(machine, state)
                .ok_or("the installed reference has no unique flow state")?;
            let mut pending = vec![(loan, false)];
            let mut active = Vec::new();
            let mut completed = Vec::new();
            let mut output = Vec::new();
            while let Some((handle, closing)) = pending.pop() {
                if closing {
                    active.pop();
                    completed.push(handle);
                    continue;
                }
                if completed.contains(&handle) {
                    continue;
                }
                if active.contains(&handle) || !borrow.state_owns_loan(owner, handle) {
                    return Err("the installed reference has a cyclic or foreign supplier");
                }
                let row = borrow.loans.get(handle);
                if row.formation != BorrowLoanFormation::ReturnedDescriptor {
                    return Err("the installed reference supplier is not a call generation");
                }
                active.push(handle);
                pending.push((handle, true));
                if !row.established_by_call.is_valid() {
                    let suppliers = borrow
                        .loan_suppliers
                        .span(row.supplying_loans)
                        .filter(|suppliers| !suppliers.is_empty())
                        .ok_or("the installed reference lost its original call generation")?;
                    for supplier in suppliers.iter().rev() {
                        pending.push((*supplier, false));
                    }
                    continue;
                }
                if !owner
                    .calls
                    .handles()
                    .any(|call| call == row.established_by_call)
                    || !borrow.calls.is_valid(row.established_by_call)
                {
                    return Err("the installed reference names another state's call");
                }
                let source = borrow.calls.get(row.established_by_call);
                if source.statement_index != row.statement_index
                    || !source
                        .completed_reference_loans
                        .handles()
                        .any(|loan| loan == handle)
                {
                    return Err("the installed reference is not in its call's completed roster");
                }
                let mut calls = flow_state.calls.handles().filter(|call| {
                    let call = control.calls.get(*call);
                    call.statement_index == source.statement_index
                        && call.call_ordinal == source.call_ordinal
                        && call.target_symbol == source.target_symbol
                        && call.authored_expression == source.authored_expression
                        && flow
                            .borrow_loan_constraints(call.exit_constraints)
                            .any(|loan| loan == handle)
                });
                let call = calls
                    .next()
                    .ok_or("the installed reference has no performed normal completion")?;
                if calls.next().is_some() {
                    return Err("the installed reference has ambiguous normal completions");
                }
                let invocation = control.calls.get(call);
                let root = crate::CanonicalPlace {
                    root: row.storage_owner,
                    segments: Vec::new(),
                };
                let owner_type = crate::canonical_place_type_reference(
                    program,
                    state,
                    row.statement_index,
                    &root,
                )
                .ok_or("the installed descriptor lost its physical owner type")?;
                let destination = crate::CanonicalPlace {
                    root: root.root,
                    segments: crate::corresponding_place_projection(
                        program,
                        owner_type,
                        &super::loan_homes::place_segments(borrow.loan_owner_path(row)),
                    )
                    .ok_or("the installed descriptor lost its physical member correspondence")?,
                };
                let stored = super::sources_at_snapshot(
                    program,
                    control,
                    invocation.completed_storage,
                    &destination,
                    super::StoredObservation::Binding,
                )
                .ok_or("the completed call has no complete installed descriptor frontier")?;
                let sources = descriptor_sources(program, stored)?;
                let mut entry = None;
                for source in sources {
                    let produced = match &source {
                        CompletedCallDescriptorSource::Captured(captured) => {
                            let value = control.operands.get(captured.capture);
                            if value.state_symbol != state {
                                return Err("the installed descriptor belongs to another state");
                            }
                            let reference = crate::project_type_reference_from_segments(
                                program,
                                captured.reference,
                                &captured.projection,
                            )
                            .and_then(|reference| crate::reference_shell(program, reference))
                            .ok_or("the completed store does not contain the declared reference")?;
                            if row.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                                && checked_trees::BorrowAccessKind::from(reference.access)
                                    .direct_reborrow_effect(&row.kind)
                                    .is_none()
                            {
                                return Err(
                                    "the installed descriptor cannot supply its recorded access",
                                );
                            }
                            value.statement_index == row.statement_index
                                && value.storage_snapshot == invocation.invocation_storage
                        }
                        CompletedCallDescriptorSource::Incoming(_) => false,
                    };
                    let origin = if produced {
                        CompletedCallDescriptorOrigin::Produced
                    } else {
                        if entry.is_none() {
                            let stored = super::sources_at_snapshot(
                                program,
                                control,
                                invocation.invocation_storage,
                                &destination,
                                super::StoredObservation::Binding,
                            )
                            .ok_or("the retained descriptor has no complete call-entry frontier")?;
                            entry = Some(descriptor_sources(program, stored)?);
                        }
                        if !entry
                            .as_ref()
                            .ok_or("the retained descriptor lost its call-entry frontier")?
                            .iter()
                            .any(|old| same_source(old, &source))
                        {
                            return Err(
                                "the retained descriptor is not an original call-entry alternative",
                            );
                        }
                        CompletedCallDescriptorOrigin::Retained
                    };
                    output.push(CompletedCallDescriptor {
                        call,
                        loan: handle,
                        source,
                        origin,
                    });
                }
            }
            Ok(Some(output))
        }

        fn descriptor_sources(
            program: &dyn crate::CheckingInput,
            stored: super::StoredOperand,
        ) -> Result<Vec<CompletedCallDescriptorSource>, &'static str> {
            let mut pending = vec![stored];
            let mut output = Vec::new();
            while let Some(source) = pending.pop() {
                match source {
                    super::StoredOperand::Unavailable => {
                        return Err("the descriptor frontier is unavailable");
                    }
                    super::StoredOperand::Alternatives(alternatives) => {
                        if alternatives.is_empty() {
                            return Err("the descriptor frontier has no alternatives");
                        }
                        pending.extend(alternatives.into_iter().rev());
                    }
                    super::StoredOperand::Incoming(place) => {
                        output.push(CompletedCallDescriptorSource::Incoming(place));
                    }
                    super::StoredOperand::Value {
                        capture,
                        reference,
                        projection,
                        slot,
                    } => {
                        let projection =
                            crate::corresponding_place_projection(program, reference, &projection)
                                .ok_or(
                                    "the stored descriptor lost its applied member correspondence",
                                )?;
                        output.push(CompletedCallDescriptorSource::Captured(
                            super::StoredCapture {
                                capture,
                                reference,
                                projection,
                                slot,
                            },
                        ));
                    }
                }
            }
            Ok(output)
        }

        fn same_source(
            left: &CompletedCallDescriptorSource,
            right: &CompletedCallDescriptorSource,
        ) -> bool {
            match (left, right) {
                (
                    CompletedCallDescriptorSource::Captured(left),
                    CompletedCallDescriptorSource::Captured(right),
                ) => left.capture == right.capture && left.projection == right.projection,
                (
                    CompletedCallDescriptorSource::Incoming(left),
                    CompletedCallDescriptorSource::Incoming(right),
                ) => left == right,
                _ => false,
            }
        }

        pub fn matches_call_descriptor_frontier(
            program: &dyn crate::CheckingInput,
            stored: super::StoredOperand,
            descriptors: &[CompletedCallDescriptor],
        ) -> Result<bool, &'static str> {
            let sources = descriptor_sources(program, stored)?;
            Ok(!sources.is_empty()
                && sources.iter().all(|source| {
                    descriptors
                        .iter()
                        .any(|descriptor| same_source(source, &descriptor.source))
                }))
        }
    }
    pub use call_stores::{
        CompletedCallDescriptor, CompletedCallDescriptorOrigin, CompletedCallDescriptorSource,
        loan_call_descriptors, matches_call_descriptor_frontier,
    };
}

pub mod content_conservation {
    pub(crate) mod contexts {
        use super::{
            ContentConservationSourcePlan, ContentProjectionFormation, collect_callable_plans,
        };
        use arena::{Arena, Handle};
        use diagnostics::Diagnostic;
        use language_semantics::content::{ContentConservationOwnerKind, ContentProjectionPlan};
        use std::collections::HashSet;
        use std::sync::Arc;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::references::{self, TypedReferences};
        use typed_trees::signature::StateSignature;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn non_contract_expression_nodes(
            program: &dyn crate::CheckingInput,
            projections: &[ContentProjectionPlan],
            proof_nodes: &mut HashSet<(u32, u32)>,
            diagnostics: &mut Vec<Diagnostic>,
            checking_plans: &mut Vec<ContentConservationSourcePlan>,
            formations: &mut Arena<ContentProjectionFormation>,
        ) -> HashSet<ExpressionHandle> {
            let mut contexts = ContractContexts {
                program,
                projections,
                proof_nodes,
                diagnostics,
                checking_plans,
                formations,
            };
            references::visit_program(program.typed_program(), &mut contexts);
            for signature in program.prepared_callable_signatures() {
                references::visit_signature(program.typed_program(), &mut contexts, signature);
            }
            let mut uses = NonContractUses {
                program,
                proof_nodes,
                expressions: HashSet::new(),
                facts: HashSet::new(),
                types: HashSet::new(),
            };
            references::visit_program(program.typed_program(), &mut uses);
            uses.expressions
        }

        pub(crate) struct ContractContexts<'program, 'output> {
            program: &'program dyn crate::CheckingInput,
            projections: &'program [ContentProjectionPlan],
            proof_nodes: &'output mut HashSet<(u32, u32)>,
            diagnostics: &'output mut Vec<Diagnostic>,
            checking_plans: &'output mut Vec<ContentConservationSourcePlan>,
            formations: &'output mut Arena<ContentProjectionFormation>,
        }

        impl Extend<SymbolHandle> for ContractContexts<'_, '_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }

        impl TypedReferences for ContractContexts<'_, '_> {
            fn expression(&mut self, _: ExpressionHandle) -> bool {
                false
            }
            fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
                false
            }
            fn proof_fact(&mut self, _: Handle<ProofFact>) -> bool {
                false
            }

            fn callable_signature(&mut self, signature: &StateSignature) {
                let contracts = self
                    .program
                    .state_signature_contracts(signature)
                    .iter()
                    .collect::<Vec<_>>();
                let has_unvisited_fact = contracts.iter().any(|contract| {
                    self.program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .any(|fact| {
                            let root = fact.as_expression().copied().or_else(|| {
                                fact.as_membership().map(|membership| membership.value)
                            });
                            root.is_some_and(|root| {
                                !self
                                    .proof_nodes
                                    .contains(&(root.arena_index(), root.generation()))
                            })
                        })
                });
                if !has_unvisited_fact {
                    return;
                }
                collect_callable_plans(
                    self.program,
                    self.projections,
                    ContentConservationOwnerKind::Machine,
                    signature.symbol,
                    signature.symbol,
                    signature.name.as_str(),
                    self.program.state_signature_parameters(signature),
                    signature.return_type,
                    SymbolHandle::invalid(),
                    &contracts,
                    &Arc::from([]),
                    self.proof_nodes,
                    self.checking_plans,
                    self.formations,
                    self.diagnostics,
                );
            }
        }

        pub(crate) struct NonContractUses<'program> {
            program: &'program dyn crate::CheckingInput,
            proof_nodes: &'program HashSet<(u32, u32)>,
            expressions: HashSet<ExpressionHandle>,
            facts: HashSet<Handle<ProofFact>>,
            types: HashSet<TypeReferenceHandle>,
        }

        impl Extend<SymbolHandle> for NonContractUses<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }

        impl TypedReferences for NonContractUses<'_> {
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                self.expressions.insert(expression)
            }
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                self.types.insert(reference)
            }
            fn proof_fact(&mut self, handle: Handle<ProofFact>) -> bool {
                if !self.facts.insert(handle) {
                    return false;
                }
                let fact = self.program.proof_facts.get(handle);
                let root = fact
                    .as_expression()
                    .copied()
                    .or_else(|| fact.as_membership().map(|membership| membership.value));
                !root.is_some_and(|root| {
                    self.proof_nodes
                        .contains(&(root.arena_index(), root.generation()))
                })
            }
        }
    }
    pub(crate) mod normalization {
        use super::{
            ContentProjectionFormation, algebra_key, domain_label, is_old_call, is_separate_call,
            projection_label, projection_plan_for_call, structural_place_label,
        };
        use crate::frozen_program::expression_table;
        use language_semantics::ReferenceAccess;
        use language_semantics::content::{
            ContentAlgebraIdentity, ContentCaseSegment, ContentConservationEquation,
            ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment,
            ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
        };
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) struct NormalizationContext<'program, 'contracts> {
            pub(crate) program: &'program dyn crate::CheckingInput,
            pub(crate) callable: SymbolHandle,
            pub(crate) source_expression: ExpressionHandle,
            pub(crate) projections: &'program [ContentProjectionPlan],
            pub(crate) parameters: &'program [StateParameter],
            pub(crate) return_type: TypeReferenceHandle,
            pub(crate) self_data: SymbolHandle,
            pub(crate) contracts: &'contracts [&'contracts SignatureContract],
            pub(crate) boundary_subjects: &'contracts [crate::BoundaryQualificationObligation],
        }

        pub(crate) fn normalize_equation(
            context: &NormalizationContext<'_, '_>,
            expression: ExpressionHandle,
            formations: &mut Vec<ContentProjectionFormation>,
        ) -> Result<(ContentAlgebraIdentity, ContentConservationEquation), String> {
            let identity = crate::mathematical_applications::identity_application_at(
                context.program,
                expression,
            )
            .map_err(|reason| {
                format!("content conservation requires the sealed typed core::Identity: {reason}")
            })?;
            let carrier = super::super::content_projections::content_algebra_for_type(
                context.program,
                identity.carrier,
            )
            .ok_or_else(|| {
                "content conservation identity requires an exact closed content algebra carrier"
                    .to_owned()
            })?;
            let [left, right] = identity.subjects;
            let (left, left_algebra) = normalize_term(context, left, formations)?;
            let (right, right_algebra) = normalize_term(context, right, formations)?;
            if left_algebra != right_algebra {
                return Err(format!(
                    "the two sides select different content algebras (`{}` versus `{}`)",
                    algebra_key(&left_algebra),
                    algebra_key(&right_algebra),
                ));
            }
            if carrier != left_algebra {
                return Err(format!(
                    "the identity carrier selects content algebra `{}` but its subjects select `{}`",
                    algebra_key(&carrier),
                    algebra_key(&left_algebra),
                ));
            }
            Ok((left_algebra, ContentConservationEquation::new(left, right)))
        }

        fn normalize_term(
            context: &NormalizationContext<'_, '_>,
            expression: ExpressionHandle,
            formations: &mut Vec<ContentProjectionFormation>,
        ) -> Result<(ContentConservationTerm, ContentAlgebraIdentity), String> {
            let Some(call) = expression_table(context.program).call_node(expression) else {
                return Err(format!(
                    "term `{}` is not an exact content projection or `separate(...)` composition",
                    expression_table(context.program).display_name(expression),
                ));
            };

            if is_separate_call(context.program, call) {
                if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                    return Err("`separate(...)` is a receiverless compiler-owned proof intrinsic and accepts no static machine arguments".to_owned());
                }
                let arguments =
                    expression_table(context.program).expression_handles(call.arguments);
                if arguments.len() < 2 {
                    return Err("`separate(...)` requires at least two content terms".to_owned());
                }
                let mut terms = Vec::with_capacity(arguments.len());
                let mut selected_algebra = None;
                for argument in arguments {
                    let (term, algebra) = normalize_term(context, *argument, formations)?;
                    if let Some(selected) = &selected_algebra {
                        if selected != &algebra {
                            return Err(format!(
                                "`separate(...)` mixes incompatible algebras (`{}` and `{}`)",
                                algebra_key(selected),
                                algebra_key(&algebra),
                            ));
                        }
                    } else {
                        selected_algebra = Some(algebra.clone());
                    }
                    terms.push(term);
                }
                return Ok((
                    ContentConservationTerm::separate(terms),
                    selected_algebra.expect("two terms select an algebra"),
                ));
            }

            let projection = projection_plan_for_call(context.program, context.projections, call)
        .ok_or_else(|| {
            format!(
                "call `{}` is not the exact owner-unique `Content<Owner, A>::project` conformance machine; there is no generic `content(...)` intrinsic",
                expression_table(context.program).display_name(expression),
            )
        })?;
            if !call.machine_arguments.is_empty() {
                return Err(
                    "a content projection call accepts no static machine arguments".to_owned(),
                );
            }
            let [argument] = expression_table(context.program).expression_handles(call.arguments)
            else {
                return Err(
                    "an exact content projection call requires exactly one borrowed subject place"
                        .to_owned(),
                );
            };
            let subject = normalize_projection_subject(context, *argument, projection)?;
            formations.push(ContentProjectionFormation {
                callable: context.callable,
                source_expression: context.source_expression,
                projection: projection.machine,
                call: expression,
                argument: *argument,
            });
            Ok((
                ContentConservationTerm::Projection {
                    domain: projection.domain,
                    semantic_domain: projection.semantic_domain,
                    projection_machine: projection.machine,
                    projection_report_fingerprint: projection.report_fingerprint,
                    subject,
                },
                projection.algebra.clone(),
            ))
        }

        fn normalize_projection_subject(
            context: &NormalizationContext<'_, '_>,
            expression: ExpressionHandle,
            projection: &ContentProjectionPlan,
        ) -> Result<ContentStructuralPlace, String> {
            let (version, borrowed) = match expression_table(context.program).expression(expression)
            {
                ExpressionNode::Call(call) if is_old_call(context.program, call) => {
                    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                        return Err(
                            "`old(place)` is a receiverless compiler-owned proof intrinsic"
                                .to_owned(),
                        );
                    }
                    let [argument] =
                        expression_table(context.program).expression_handles(call.arguments)
                    else {
                        return Err(
                    "`old(place)` requires exactly one parameter, `self`, or structural subplace"
                        .to_owned(),
                );
                    };
                    (ContentPlaceVersion::Entry, *argument)
                }
                _ => (ContentPlaceVersion::Current, expression),
            };

            let borrowed = match expression_table(context.program).expression(borrowed) {
                ExpressionNode::Borrow(inner) if inner.access == ReferenceAccess::Shared => {
                    inner.target
                }
                ExpressionNode::Borrow(_) => {
                    return Err("content projection subjects must use a shared borrow (`&place`), not a mutable borrow, so the proof observation never mutates authority".to_owned());
                }
                _ => borrowed,
            };
            let (root_name, root_symbol, mut segments) =
                collect_structural_place(context.program, borrowed)?;
            let (root, root_type) = if language_core::is_reserved_result(&root_name) {
                if version == ContentPlaceVersion::Entry {
                    return Err(
                        "`old(result)` is invalid: `result` does not exist at callable entry"
                            .to_owned(),
                    );
                }
                if !context.return_type.is_valid() {
                    return Err(
                        "the projected `result` place belongs to a callable with no result type"
                            .to_owned(),
                    );
                }
                (ContentPlaceRoot::Result, context.return_type)
            } else {
                let Some((position, parameter)) =
                    context
                        .parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            root_symbol.is_valid() && parameter.symbol == root_symbol
                        })
                else {
                    return Err(format!(
                        "projection root `{root_name}` is not a callable parameter, `self`, or `result`",
                    ));
                };
                (
                    ContentPlaceRoot::Parameter {
                        position: u32::try_from(position).expect("parameter position fits u32"),
                        symbol: parameter.symbol,
                        name: parameter.name.as_str().to_owned(),
                    },
                    parameter.type_reference,
                )
            };

            let final_type = structural_place_type(
                context.program,
                root_type,
                &mut segments,
                context.self_data,
            )?;
            retain_payload_case_segments(context.program, &mut segments);
            let carrier = observation_carrier(context.program, final_type)?;
            let carrier_identity = context.program.normalized_type_identity_text(carrier);
            if carrier_identity != projection.carrier_identity {
                return Err(format!(
                    "projection `{}` expects carrier `{}`, but the selected structural place has `{carrier_identity}`",
                    projection_label(context.program, projection),
                    projection.carrier_identity,
                ));
            }

            let subject = ContentStructuralPlace {
                version,
                root,
                segments,
            };
            if !type_has_domain(context.program, final_type, projection.semantic_domain)
                && !contracts_establish_domain(
                    context,
                    &subject,
                    projection.domain,
                    projection.semantic_domain,
                )
                && !boundary_postcondition_forms_domain(context, &subject, projection)
            {
                return Err(format!(
                    "projection `{}` requires exact qualification `{}`, but `{}` is not qualified by its type or an ordinary callable contract",
                    projection_label(context.program, projection),
                    domain_label(context.program, projection.domain),
                    structural_place_label(&subject),
                ));
            }
            Ok(subject)
        }

        fn boundary_postcondition_forms_domain(
            context: &NormalizationContext<'_, '_>,
            subject: &ContentStructuralPlace,
            projection: &ContentProjectionPlan,
        ) -> bool {
            if subject.version != ContentPlaceVersion::Current || !subject.segments.is_empty() {
                return false;
            }
            context.boundary_subjects.iter().any(|obligation| {
                obligation.requirement.is_valid()
                    && obligation.realization_state == context.callable
                    && obligation.realization_state.is_valid()
                    && obligation.domain == projection.domain
                    && obligation.semantic_domain.is_valid()
                    && obligation.semantic_domain == projection.semantic_domain
                    && match &subject.root {
                        ContentPlaceRoot::Result => !obligation.parameter.is_valid(),
                        ContentPlaceRoot::Parameter { symbol, .. } => {
                            obligation.parameter.is_valid() && obligation.parameter == *symbol
                        }
                    }
            })
        }

        fn collect_structural_place(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Result<(String, SymbolHandle, Vec<ContentPlaceSegment>), String> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    let names = expression_table(program).name_path_members(path.members);
                    if names.is_empty() {
                        return Err("an empty name path is not a structural place".to_owned());
                    }
                    let symbols =
                        expression_table(program).name_path_member_symbols(path.member_symbols);
                    let root_name = names[0].as_str().to_owned();
                    let root_symbol = symbols.first().copied().unwrap_or(path.head_symbol);
                    let mut segments = Vec::new();
                    for (index, name) in names.iter().enumerate().skip(1) {
                        push_structural_field(
                            program,
                            &mut segments,
                            symbols
                                .get(index)
                                .copied()
                                .unwrap_or(SymbolHandle::invalid()),
                            name.as_str(),
                        );
                    }
                    Ok((root_name, root_symbol, segments))
                }
                ExpressionNode::Member(member) => {
                    let (root, symbol, mut segments) =
                        collect_structural_place(program, member.receiver)?;
                    push_structural_field(
                        program,
                        &mut segments,
                        member.member_symbol,
                        member.member.as_str(),
                    );
                    Ok((root, symbol, segments))
                }
                ExpressionNode::Indexed(indexed) => {
                    let (root, symbol, mut segments) =
                        collect_structural_place(program, indexed.collection)?;
                    let Some(index) = expression_table(program).integer_node(indexed.index) else {
                        return Err(
                            "old/current structural places admit only literal fixed-array indices"
                                .to_owned(),
                        );
                    };
                    let Some(index) = index.value_u64() else {
                        return Err(
                            "a structural fixed-array index must be a nonnegative `u64` literal"
                                .to_owned(),
                        );
                    };
                    segments.push(ContentPlaceSegment::FixedIndex(index));
                    Ok((root, symbol, segments))
                }
                _ => Err(format!(
                    "`{}` is not a parameter, `self`, `result`, or structural subplace",
                    expression_table(program).display_name(expression),
                )),
            }
        }

        pub fn push_structural_field(
            program: &dyn crate::CheckingInput,
            segments: &mut Vec<ContentPlaceSegment>,
            field_symbol: SymbolHandle,
            field_name: &str,
        ) {
            if let Some(variant_symbol) = facts::payload_variant_for_field(program, field_symbol)
                && let Some(variant_name) = data_variant_name(program, variant_symbol)
            {
                segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: variant_symbol,
                    name: variant_name.to_owned(),
                }));
            }
            segments.push(ContentPlaceSegment::Field(ContentFieldSegment {
                symbol: field_symbol,
                name: field_name.to_owned(),
            }));
        }

        fn retain_payload_case_segments(
            program: &dyn crate::CheckingInput,
            segments: &mut Vec<ContentPlaceSegment>,
        ) {
            let source = std::mem::take(segments);
            for segment in source {
                if let ContentPlaceSegment::Field(field) = &segment
                    && let Some(variant_symbol) =
                        facts::payload_variant_for_field(program, field.symbol)
                    && !matches!(
                        segments.last(),
                        Some(ContentPlaceSegment::Case(case)) if case.symbol == variant_symbol
                    )
                    && let Some(variant_name) = data_variant_name(program, variant_symbol)
                {
                    segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
                        symbol: variant_symbol,
                        name: variant_name.to_owned(),
                    }));
                }
                segments.push(segment);
            }
        }

        pub fn data_variant_name(
            program: &dyn crate::CheckingInput,
            variant_symbol: SymbolHandle,
        ) -> Option<&str> {
            program.data_definitions().iter().find_map(|definition| {
                program
                    .data_variant(definition, variant_symbol)
                    .map(|variant| variant.name.as_str())
            })
        }

        fn observation_carrier(
            program: &dyn crate::CheckingInput,
            mut current: TypeReferenceHandle,
        ) -> Result<TypeReferenceHandle, String> {
            for _ in 0..program.type_reference_table.type_reference_count() {
                if !current.is_valid() {
                    break;
                }
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } if access.is_readable() => {
                        current = *referee;
                    }
                    TypeReferenceNode::Reference { .. } => {
                        return Err(
                            "content projection subjects cannot observe write-only reference storage"
                                .to_owned(),
                        );
                    }
                    _ => return Ok(current),
                }
            }
            Err("the projected structural place has no resolved readable carrier type".to_owned())
        }

        fn structural_place_type(
            program: &dyn crate::CheckingInput,
            mut current: TypeReferenceHandle,
            segments: &mut [ContentPlaceSegment],
            self_data: SymbolHandle,
        ) -> Result<TypeReferenceHandle, String> {
            let mut active_variant = None;
            let mut at_root = true;
            for segment in segments {
                let unwrapped = observation_carrier(program, current)?;
                match segment {
                    ContentPlaceSegment::Case(case) => {
                        if active_variant.is_some() {
                            return Err("a sum-case path must select a payload field".to_owned());
                        }
                        let data = crate::value_custody::places::data_definition_for_type(
                            program, unwrapped,
                        )
                        .or_else(|| {
                            (at_root && self_data.is_valid())
                                .then(|| {
                                    crate::declarations::data_queries::data_definition_by_symbol(
                                        program, self_data,
                                    )
                                })
                                .flatten()
                        })
                        .ok_or_else(|| {
                            format!("`{}` is selected from a non-sum carrier", case.name)
                        })?;
                        let variant = program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant)
                                    if variant.name.as_str() == case.name =>
                                {
                                    Some(variant)
                                }
                                _ => None,
                            })
                            .ok_or_else(|| {
                                format!("data `{}` has no sum case `{}`", data.name, case.name)
                            })?;
                        if case.symbol.is_valid() && case.symbol != variant.symbol {
                            return Err(format!(
                                "sum case `{}` does not match its resolved identity",
                                case.name
                            ));
                        }
                        case.symbol = variant.symbol;
                        active_variant = Some(variant.symbol);
                    }
                    ContentPlaceSegment::Field(field) => {
                        let direct_data = crate::value_custody::places::data_definition_for_type(
                            program, unwrapped,
                        );
                        let through_attached_self =
                            direct_data.is_none() && at_root && self_data.is_valid();
                        let data = direct_data
                            .or_else(|| {
                                through_attached_self.then_some(()).and_then(|()| {
                                    crate::declarations::data_queries::data_definition_by_symbol(
                                        program, self_data,
                                    )
                                })
                            })
                            .ok_or_else(|| {
                                format!("`{}` is selected from a non-record carrier", field.name)
                            })?;
                        let selected = if let Some(variant_symbol) = active_variant.take() {
                            program.data_members(data).iter().find_map(|member| {
                                let variant = member.as_variant()?;
                                (variant.symbol == variant_symbol).then(|| {
                                    program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|candidate| candidate.name.as_str() == field.name)
                                        .map(|candidate| {
                                            (candidate.symbol, candidate.type_reference)
                                        })
                                })?
                            })
                        } else {
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Field(candidate)
                                        if candidate.name.as_str() == field.name =>
                                    {
                                        Some((candidate.symbol, candidate.type_reference))
                                    }
                                    DataMember::Variant(variant) => program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|candidate| candidate.name.as_str() == field.name)
                                        .map(|candidate| {
                                            (candidate.symbol, candidate.type_reference)
                                        }),
                                    _ => None,
                                })
                        };
                        let (field_symbol, field_type) = selected.ok_or_else(|| {
                            format!(
                                "data `{}` has no structural field `{}`",
                                data.name, field.name
                            )
                        })?;
                        if field.symbol.is_valid()
                            && field.symbol != field_symbol
                            && !through_attached_self
                        {
                            return Err(format!(
                                "structural field `{}` does not match its resolved identity",
                                field.name
                            ));
                        }
                        field.symbol = field_symbol;
                        current = field_type;
                    }
                    ContentPlaceSegment::FixedIndex(_) => {
                        if active_variant.is_some() {
                            return Err("a sum-case path must select a payload field".to_owned());
                        }
                        current = match program.type_reference_table.type_reference(unwrapped) {
                            TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
                            _ => {
                                return Err(
                                    "a fixed index is applied to a non-fixed-array structural place"
                                        .to_owned(),
                                );
                            }
                        };
                    }
                }
                at_root = false;
            }
            if active_variant.is_some() {
                return Err("a sum-case path must select a payload field".to_owned());
            }
            Ok(current)
        }

        fn contracts_establish_domain(
            context: &NormalizationContext<'_, '_>,
            subject: &ContentStructuralPlace,
            domain: SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
        ) -> bool {
            if context
                .program
                .domain_definition(domain)
                .is_none_or(|definition| definition.semantic_id != semantic_domain)
            {
                return false;
            }
            context.contracts.iter().any(|contract| {
                let allowed_kind = match (&subject.root, subject.version) {
                    (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                        contract.kind == SignatureContractKind::Ensures
                    }
                    (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Entry) => {
                        contract.kind == SignatureContractKind::Requires
                    }
                    (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Current) => matches!(
                        contract.kind,
                        SignatureContractKind::Requires | SignatureContractKind::Ensures
                    ),
                    (ContentPlaceRoot::Result, ContentPlaceVersion::Entry) => false,
                };
                allowed_kind
                    && context
                        .program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .any(|fact| {
                            let Some(membership) = fact.as_membership() else {
                                return false;
                            };
                            membership.domain_symbol == domain
                                && collect_structural_place(context.program, membership.value)
                                    .ok()
                                    .is_some_and(|(root, symbol, segments)| {
                                        structural_place_matches(subject, &root, symbol, &segments)
                                    })
                        })
            })
        }

        fn structural_place_matches(
            expected: &ContentStructuralPlace,
            root_name: &str,
            root_symbol: SymbolHandle,
            segments: &[ContentPlaceSegment],
        ) -> bool {
            let root_matches = match &expected.root {
                ContentPlaceRoot::Result => language_core::is_reserved_result(root_name),
                ContentPlaceRoot::Parameter { symbol, name, .. } => {
                    (*symbol == root_symbol && symbol.is_valid()) || name == root_name
                }
            };
            root_matches && expected.segments == segments
        }

        fn type_has_domain(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            domain: language_semantics::SemanticDomainId,
        ) -> bool {
            if !type_reference.is_valid() {
                return false;
            }
            match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => type_has_domain(program, *referee, domain),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, typed_trees::types::TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain))
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
            }
        }
    }

    use crate::CheckingInput;
    pub use normalization::{data_variant_name, push_structural_field};

    use crate::frozen_program::expression_table;
    use normalization::{NormalizationContext, normalize_equation};

    use arena::Arena;
    use diagnostics::Diagnostic;
    use language_semantics::content::{
        ContentAlgebraIdentity, ContentConservationOwnerKind, ContentConservationPlan,
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
        ContentProjectionPlan, ContentStructuralPlace, conservation_report_fingerprint,
    };
    use std::collections::HashSet;
    use std::sync::Arc;
    use symbols::{BuiltinFunction, SymbolHandle};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ContentConservationSourcePlan {
        pub source_expression: ExpressionHandle,
        pub plan: ContentConservationPlan,
        pub boundary_subjects: Arc<[crate::BoundaryQualificationObligation]>,
    }

    #[derive(Debug)]
    pub(crate) struct ContentConservationInputs {
        plans: Arc<Vec<ContentConservationSourcePlan>>,
        projections: Arc<Vec<ContentProjectionPlan>>,
        checking_plans: Vec<ContentConservationSourcePlan>,
        diagnostics: Vec<Diagnostic>,
        formations: Arena<ContentProjectionFormation>,
    }

    #[derive(Debug, Clone, Default)]
    pub(crate) struct ContentProjectionFormation {
        callable: SymbolHandle,
        source_expression: ExpressionHandle,
        projection: SymbolHandle,
        call: ExpressionHandle,
        argument: ExpressionHandle,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ContentDeclarationFormation<'inputs> {
        inputs: &'inputs ContentConservationInputs,
        callable: SymbolHandle,
        source_expression: ExpressionHandle,
    }

    impl ContentConservationInputs {
        pub(crate) fn forms_projection_call(
            &self,
            owner: SymbolHandle,
            call: ExpressionHandle,
        ) -> bool {
            self.formations.iter().any(|(_, formation)| {
                formation.call == call
                    && self.plans.iter().chain(&self.checking_plans).any(|plan| {
                        plan.plan.callable == formation.callable
                            && plan.source_expression == formation.source_expression
                            && (plan.plan.callable == owner || plan.plan.owner == owner)
                    })
            })
        }

        pub(crate) fn declaration_formation(
            &self,
            callable: SymbolHandle,
            source_expression: ExpressionHandle,
        ) -> Option<ContentDeclarationFormation<'_>> {
            self.plans
                .iter()
                .chain(&self.checking_plans)
                .any(|plan| {
                    plan.plan.callable == callable && plan.source_expression == source_expression
                })
                .then_some(ContentDeclarationFormation {
                    inputs: self,
                    callable,
                    source_expression,
                })
        }
    }

    impl ContentDeclarationFormation<'_> {
        pub(crate) fn forms_argument(
            self,
            projection: SymbolHandle,
            call: ExpressionHandle,
            argument: ExpressionHandle,
        ) -> bool {
            self.inputs.formations.iter().any(|(_, formation)| {
                formation.callable == self.callable
                    && formation.source_expression == self.source_expression
                    && formation.projection == projection
                    && formation.call == call
                    && formation.argument == argument
            })
        }
    }

    pub(crate) fn validate_content_conservation_contracts(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let inputs = content_conservation_inputs(program);
        diagnostics.extend(inputs.diagnostics.iter().cloned());
    }

    pub(crate) fn content_conservation_inputs(
        program: &dyn CheckingInput,
    ) -> Arc<ContentConservationInputs> {
        crate::frozen_program::frozen_memo(
            program,
            |memos| &memos.content_conservation,
            || {
                let projections =
                    crate::value_custody::content_projections::build_content_projection_plans(
                        program,
                    );
                let mut diagnostics = Vec::new();
                let mut formations = Arena::default();
                let mut checking_plans = Vec::new();
                let plans = collect_content_conservation_plans(
                    program,
                    &projections,
                    &mut diagnostics,
                    &mut formations,
                    &mut checking_plans,
                );
                ContentConservationInputs {
                    plans: Arc::new(plans),
                    projections: Arc::new(projections),
                    checking_plans,
                    diagnostics,
                    formations,
                }
            },
        )
    }

    pub fn content_conservation_plans(
        program: &dyn CheckingInput,
    ) -> Arc<Vec<ContentConservationSourcePlan>> {
        Arc::clone(&content_conservation_inputs(program).plans)
    }

    pub(crate) fn content_projection_plans(
        program: &dyn CheckingInput,
    ) -> Arc<Vec<ContentProjectionPlan>> {
        Arc::clone(&content_conservation_inputs(program).projections)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ContentBoundaryBodyRequirement {
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub realization_state: SymbolHandle,
        pub conservation: ContentConservationPlan,
    }

    pub fn boundary_body_content_requirements(
        program: &dyn CheckingInput,
        body: &ContentConservationSourcePlan,
    ) -> Result<Vec<ContentBoundaryBodyRequirement>, &'static str> {
        let inputs = content_conservation_inputs(program);
        let mut selected = Vec::new();
        for obligation in body.boundary_subjects.iter().filter(|obligation| {
            !obligation.parameter.is_valid()
                && obligation.realization_state == body.plan.callable
                && obligation.semantic_domain.is_valid()
        }) {
            if !result_has_qualification(
                body.plan.equation.left(),
                obligation.domain,
                obligation.semantic_domain,
            ) && !result_has_qualification(
                body.plan.equation.right(),
                obligation.domain,
                obligation.semantic_domain,
            ) {
                continue;
            }
            for requirement in inputs.plans.iter().chain(&inputs.checking_plans) {
                if requirement.plan.callable != obligation.requirement
                    || requirement.plan.algebra != body.plan.algebra
                    || !same_conservation_terms(
                        requirement.plan.equation.left(),
                        body.plan.equation.left(),
                    )
                    || !same_conservation_terms(
                        requirement.plan.equation.right(),
                        body.plan.equation.right(),
                    )
                {
                    continue;
                }
                let candidate = ContentBoundaryBodyRequirement {
                    declaring_trait: obligation.declaring_trait,
                    requirement: obligation.requirement,
                    realization_state: obligation.realization_state,
                    conservation: requirement.plan.clone(),
                };
                if !selected.contains(&candidate) {
                    selected.push(candidate);
                }
            }
        }
        if selected.is_empty() {
            return Err("body custody law has no exact authorized public boundary requirement");
        }
        Ok(selected)
    }

    fn result_has_qualification(
        term: &ContentConservationTerm,
        domain: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        match term {
            ContentConservationTerm::Projection {
                domain: actual_domain,
                semantic_domain: actual_semantic,
                subject,
                ..
            } => {
                *actual_domain == domain
                    && *actual_semantic == semantic_domain
                    && subject.root == ContentPlaceRoot::Result
                    && subject.version == ContentPlaceVersion::Current
                    && subject.segments.is_empty()
            }
            ContentConservationTerm::Separate(children) => children
                .iter()
                .any(|child| result_has_qualification(child, domain, semantic_domain)),
        }
    }

    fn same_conservation_terms(
        left: &ContentConservationTerm,
        right: &ContentConservationTerm,
    ) -> bool {
        match (left, right) {
            (
                ContentConservationTerm::Projection {
                    domain: left_domain,
                    semantic_domain: left_semantic,
                    projection_machine: left_projection,
                    subject: left,
                    ..
                },
                ContentConservationTerm::Projection {
                    domain: right_domain,
                    semantic_domain: right_semantic,
                    projection_machine: right_projection,
                    subject: right,
                    ..
                },
            ) => {
                left_domain == right_domain
                    && left_semantic == right_semantic
                    && left_projection == right_projection
                    && left.version == right.version
                    && left.segments == right.segments
                    && match (&left.root, &right.root) {
                        (
                            ContentPlaceRoot::Parameter { position: left, .. },
                            ContentPlaceRoot::Parameter {
                                position: right, ..
                            },
                        ) => left == right,
                        (ContentPlaceRoot::Result, ContentPlaceRoot::Result) => true,
                        _ => false,
                    }
            }
            (ContentConservationTerm::Separate(left), ContentConservationTerm::Separate(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| same_conservation_terms(left, right))
            }
            _ => false,
        }
    }

    fn collect_content_conservation_plans(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        diagnostics: &mut Vec<Diagnostic>,
        formations: &mut Arena<ContentProjectionFormation>,
        checking_plans: &mut Vec<ContentConservationSourcePlan>,
    ) -> Vec<ContentConservationSourcePlan> {
        let mut plans = Vec::new();
        let mut proof_nodes = HashSet::new();
        let no_boundary_subjects: Arc<[crate::BoundaryQualificationObligation]> = Arc::from([]);

        for trait_definition in program.traits() {
            for signature in program.trait_machine_signatures(trait_definition) {
                let contracts = program
                    .state_signature_contracts(signature)
                    .iter()
                    .collect::<Vec<_>>();
                collect_callable_plans(
                    program,
                    projections,
                    ContentConservationOwnerKind::TraitRequirement,
                    trait_definition.symbol,
                    signature.symbol,
                    &format!("{}::{}", trait_definition.name, signature.name),
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    SymbolHandle::invalid(),
                    &contracts,
                    &no_boundary_subjects,
                    &mut proof_nodes,
                    &mut plans,
                    formations,
                    diagnostics,
                );
            }
        }

        let mut boundary_owner = SymbolHandle::invalid();
        let mut boundary_subjects = Arc::clone(&no_boundary_subjects);
        program.for_each_machine_state_contract_frame(|machine, state, contracts, label| {
            if boundary_owner != machine.symbol {
                boundary_subjects =
                    crate::boundary_provider_qualification_obligations(program, machine).into();
                boundary_owner = machine.symbol;
            }
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::Machine,
                machine.symbol,
                state.symbol,
                &label,
                program.state_parameters(state),
                state.return_type,
                machine.attached_data_symbol,
                &contracts,
                &boundary_subjects,
                &mut proof_nodes,
                &mut plans,
                formations,
                diagnostics,
            );
        });

        let non_contract_nodes = contexts::non_contract_expression_nodes(
            program,
            projections,
            &mut proof_nodes,
            diagnostics,
            checking_plans,
            formations,
        );

        for (handle, node) in expression_table(program).iter_expressions() {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            if !non_contract_nodes.contains(&handle) {
                continue;
            }
            if is_old_call(program, call)
                || is_separate_call(program, call)
                || projection_plan_for_call(program, projections, call).is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                "proof-only content operation `{}` is used in executable expression position; `old`, `separate`, and exact `Content<Owner, A>::project` machines are contract-only",
                call.target.as_str(),
            )));
            }
        }

        plans
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_callable_plans(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        owner_kind: ContentConservationOwnerKind,
        owner: SymbolHandle,
        callable: SymbolHandle,
        label: &str,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
        self_data: SymbolHandle,
        contracts: &[&SignatureContract],
        boundary_subjects: &Arc<[crate::BoundaryQualificationObligation]>,
        proof_nodes: &mut HashSet<(u32, u32)>,
        plans: &mut Vec<ContentConservationSourcePlan>,
        formations: &mut Arena<ContentProjectionFormation>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for contract in contracts {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    collect_expression_nodes(program, *expression, proof_nodes);
                } else if let Some(membership) = fact.as_membership() {
                    collect_expression_nodes(program, membership.value, proof_nodes);
                }
            }
        }

        let first_callable_plan = plans.len();
        for contract in contracts {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                if !expression_uses_content_surface(program, projections, *expression) {
                    continue;
                }
                if contract.kind != SignatureContractKind::Ensures {
                    diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` uses proof-only content operations outside an `ensures` contract; callable-entry/current conservation relates callable outcomes",
                )));
                    continue;
                }

                let context = NormalizationContext {
                    program,
                    callable,
                    source_expression: *expression,
                    projections,
                    parameters,
                    return_type,
                    self_data,
                    contracts,
                    boundary_subjects,
                };
                let mut projection_formations = Vec::new();
                let (algebra, equation) = match normalize_equation(
                    &context,
                    *expression,
                    &mut projection_formations,
                ) {
                    Ok(normalized) => normalized,
                    Err(message) => {
                        diagnostics.push(Diagnostic::error(format!(
                        "callable `{label}` has an invalid content-conservation contract: {message}",
                    )));
                        continue;
                    }
                };
                if let Some(first) = plans[first_callable_plan..].iter().find(|previous| {
                    previous.plan.algebra == algebra
                        && [
                            previous.plan.equation.left(),
                            previous.plan.equation.right(),
                        ]
                        .into_iter()
                        .any(|previous_term| {
                            [equation.left(), equation.right()]
                                .into_iter()
                                .any(|term| share_projection_owner(previous_term, term))
                        })
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` publishes more than one content-conservation equation for the same qualification owner and algebra (facts `{}` and `{}`); author one normalized equation per outcome row and qualification owner",
                    expression_table(program).display_name(first.source_expression),
                    expression_table(program).display_name(*expression),
                )));
                    continue;
                }
                formations.insert_many(projection_formations);
                let report_fingerprint = conservation_report_fingerprint(&algebra, &equation);
                plans.push(ContentConservationSourcePlan {
                    source_expression: *expression,
                    boundary_subjects: Arc::clone(boundary_subjects),
                    plan: ContentConservationPlan {
                        owner_kind,
                        owner,
                        callable,
                        algebra,
                        equation,
                        report_fingerprint,
                    },
                });
            }
        }
    }

    fn share_projection_owner(
        left: &ContentConservationTerm,
        right: &ContentConservationTerm,
    ) -> bool {
        match (left, right) {
            (ContentConservationTerm::Separate(children), other)
            | (other, ContentConservationTerm::Separate(children)) => children
                .iter()
                .any(|child| share_projection_owner(child, other)),
            (
                ContentConservationTerm::Projection {
                    domain: left_domain,
                    semantic_domain: left_instance,
                    projection_machine: left_projection,
                    ..
                },
                ContentConservationTerm::Projection {
                    domain: right_domain,
                    semantic_domain: right_instance,
                    projection_machine: right_projection,
                    ..
                },
            ) => {
                left_domain == right_domain
                    && left_instance == right_instance
                    && left_projection == right_projection
            }
        }
    }

    fn projection_plan_for_call<'a>(
        program: &dyn CheckingInput,
        projections: &'a [ContentProjectionPlan],
        call: &TableCallExpression,
    ) -> Option<&'a ContentProjectionPlan> {
        projections.iter().find(|projection| {
            program.machine(projection.machine).is_some_and(|machine| {
                call.target_symbol == machine.symbol
                    || typed_trees::state::state_by_symbol(
                        program.machine_states(machine),
                        call.target_symbol,
                    )
                    .is_some()
            })
        })
    }

    pub(crate) fn intrinsic_result_type_reference(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
        mut operand_type: impl FnMut(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<TypeReferenceHandle> {
        let function = program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)?;
        if !matches!(
            function,
            BuiltinFunction::ContentOld | BuiltinFunction::ContentSeparate
        ) || call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return None;
        }
        let arguments = expression_table(program).expression_handles(call.arguments);
        match function {
            BuiltinFunction::ContentOld if arguments.len() == 1 => operand_type(arguments[0]),
            BuiltinFunction::ContentSeparate if arguments.len() >= 2 => {
                let carrier = operand_type(arguments[0])?;
                arguments[1..]
                    .iter()
                    .all(|argument| {
                        operand_type(*argument).is_some_and(|other| {
                            program.normalized_type_identity(carrier)
                                == program.normalized_type_identity(other)
                        })
                    })
                    .then_some(carrier)
            }
            _ => None,
        }
    }

    fn is_old_call(program: &dyn CheckingInput, call: &TableCallExpression) -> bool {
        is_builtin_call(program, call, BuiltinFunction::ContentOld)
    }

    fn is_separate_call(program: &dyn CheckingInput, call: &TableCallExpression) -> bool {
        is_builtin_call(program, call, BuiltinFunction::ContentSeparate)
    }

    fn is_builtin_call(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
        function: BuiltinFunction,
    ) -> bool {
        !call.receiver.is_valid()
            && program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(function)
    }

    fn expression_uses_content_surface(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        expression: ExpressionHandle,
    ) -> bool {
        if !expression.is_valid() {
            return false;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Match(dispatch) => {
                crate::queries::expression_traversal::match_children(program, *dispatch)
                    .any(|child| expression_uses_content_surface(program, projections, child))
            }
            ExpressionNode::Call(call) => {
                is_old_call(program, call)
                    || is_separate_call(program, call)
                    || projection_plan_for_call(program, projections, call).is_some()
                    || expression_uses_content_surface(program, projections, call.receiver)
                    || expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| {
                            expression_uses_content_surface(program, projections, *argument)
                        })
            }
            ExpressionNode::Atomic(atomic) => {
                expression_uses_content_surface(program, projections, atomic.value)
            }
            ExpressionNode::Binary(binary) => {
                expression_uses_content_surface(program, projections, binary.left)
                    || expression_uses_content_surface(program, projections, binary.right)
            }
            ExpressionNode::Unary(unary) => {
                expression_uses_content_surface(program, projections, unary.operand)
            }
            ExpressionNode::Cast(cast) => {
                expression_uses_content_surface(program, projections, cast.value)
            }
            ExpressionNode::Indexed(indexed) => {
                expression_uses_content_surface(program, projections, indexed.collection)
                    || expression_uses_content_surface(program, projections, indexed.index)
            }
            ExpressionNode::Member(member) => {
                expression_uses_content_surface(program, projections, member.receiver)
            }
            ExpressionNode::Borrow(inner) => {
                expression_uses_content_surface(program, projections, inner.target)
            }
            ExpressionNode::Range(range) => {
                expression_uses_content_surface(program, projections, range.start)
                    || expression_uses_content_surface(program, projections, range.end)
            }
            ExpressionNode::ArrayLiteral(items) => expression_table(program)
                .expression_handles(*items)
                .iter()
                .any(|item| expression_uses_content_surface(program, projections, *item)),
            ExpressionNode::StructLiteral(literal) => expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .any(|field| expression_uses_content_surface(program, projections, field.value)),
            ExpressionNode::TypeExpression(_)
            | ExpressionNode::SavedSubject(_)
            | typed_trees::literal_expression_pattern!() => false,
        }
    }

    fn collect_expression_nodes(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        nodes: &mut HashSet<(u32, u32)>,
    ) {
        if !expression.is_valid()
            || !nodes.insert((expression.arena_index(), expression.generation()))
        {
            return;
        }
        crate::queries::expression_traversal::children(
            program,
            expression_table(program).expression(expression),
            |child| collect_expression_nodes(program, child, nodes),
        );
    }

    fn algebra_key(algebra: &ContentAlgebraIdentity) -> String {
        match algebra {
            ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
                format!("IntervalSet<{coordinate_space}>")
            }
            ContentAlgebraIdentity::CountedQuantity { unit } => {
                format!("CountedQuantity<{unit}>")
            }
        }
    }

    fn projection_label(program: &dyn CheckingInput, projection: &ContentProjectionPlan) -> String {
        program
            .machine(projection.machine)
            .map(|machine| machine.name.to_string())
            .unwrap_or_else(|| format!("projection#{}", projection.machine.arena_index()))
    }

    fn domain_label(program: &dyn CheckingInput, domain: SymbolHandle) -> String {
        program
            .domain_definition(domain)
            .map(|definition| definition.name.to_string())
            .unwrap_or_else(|| format!("domain#{}", domain.arena_index()))
    }

    fn structural_place_label(place: &ContentStructuralPlace) -> String {
        let mut label = match &place.root {
            ContentPlaceRoot::Parameter { name, .. } => name.clone(),
            ContentPlaceRoot::Result => "result".to_owned(),
        };
        for segment in &place.segments {
            match segment {
                ContentPlaceSegment::Case(case) => {
                    label.push_str("::");
                    label.push_str(&case.name);
                }
                ContentPlaceSegment::Field(field) => {
                    label.push('.');
                    label.push_str(&field.name);
                }
                ContentPlaceSegment::FixedIndex(index) => {
                    label.push('[');
                    label.push_str(&index.to_string());
                    label.push(']');
                }
            }
        }
        if place.version == ContentPlaceVersion::Entry {
            format!("old(&{label})")
        } else {
            format!("&{label}")
        }
    }
}

pub mod content_projections {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::value_custody::type_references::type_references_match;
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use language_semantics::content::{
        ContentAlgebraIdentity, ContentArithmeticOperator, ContentFieldSegment,
        ContentIntervalExpression, ContentProjectionExpression, ContentProjectionPlan,
        ContentScalarExpression, projection_report_fingerprint,
    };
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::domain::DomainDefinition;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use typed_trees::trait_definition::TraitDefinition;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ProjectionSubject {
        machine: SymbolHandle,
        state: SymbolHandle,
        symbol: SymbolHandle,
        qualified: TypeReferenceHandle,
        carrier: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ProjectionDomainCandidate<'program> {
        definition: &'program DomainDefinition,
        semantic_domain: language_semantics::SemanticDomainId,
    }

    pub(crate) fn validate_content_projection_conformances(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let mut published: Vec<(language_semantics::SemanticDomainId, SymbolHandle, String)> =
            Vec::new();

        for machine in program.machines() {
            for conformance in program.machine_trait_conformances(machine) {
                let Some(content) = program
                    .trait_definition_by_symbol(conformance.symbol)
                    .filter(|candidate| is_content_projection_trait(program, candidate))
                else {
                    continue;
                };

                if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project") {
                    diagnostics.push(Diagnostic::error(format!(
                        "content projection machine `{}` must explicitly satisfy `{}::project`",
                        machine.name, content.name
                    )));
                    continue;
                }

                let Some(subject) = projection_subject(program, machine) else {
                    diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` must take exactly one shared borrowed subject",
                    machine.name
                )));
                    continue;
                };
                let candidates = projection_domain_candidates(program, machine, subject);
                let candidate = match candidates.as_slice() {
                    [candidate] => *candidate,
                    [] => {
                        diagnostics.push(Diagnostic::error(format!(
                        "content projection machine `{}` is not attached to the exact qualification it projects; use `<Qualification>::content` with a subject matching that domain's carrier",
                        machine.name
                    )));
                        continue;
                    }
                    _ => {
                        diagnostics.push(Diagnostic::error(format!(
                        "content projection machine `{}` ambiguously names more than one exact qualification for subject `{}`",
                        machine.name,
                        program.display_type_reference(subject.carrier)
                    )));
                        continue;
                    }
                };
                let domain = candidate.definition;

                if program.type_multiplicity(subject.carrier) != Multiplicity::Linear {
                    diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` targets `{}` whose carrier `{}` is not linear; fine-grained content accounting belongs to owned linear claims",
                    machine.name,
                    domain.name,
                    program.display_type_reference(subject.carrier),
                )));
                    continue;
                }

                if domain.alias.is_some() {
                    diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` targets transparent alias `{}`; publish content on one exact atomic qualification instead",
                    machine.name, domain.name
                )));
                    continue;
                }
                if !matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::CheckedBody
                ) || program.machine_states(machine).first().is_none_or(|state| {
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .is_empty()
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "content projection `{}` for `{}` must be a bodyful checked machine",
                        machine.name, domain.name
                    )));
                    continue;
                }

                let Some(algebra) = selected_content_algebra(program, conformance) else {
                    diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` must select compiler-owned `IntervalSet<CoordinateSpace>` or `CountedQuantity<Unit>`, not `{}`",
                    machine.name,
                    program
                        .type_reference_table
                        .type_reference_handles(conformance.arguments)
                        .get(1)
                        .map(|argument| program.display_type_reference(*argument))
                        .unwrap_or_else(|| "<missing>".to_owned())
                )));
                    continue;
                };
                if let Some(expression) = projection_result_expression(program, machine) {
                    let before = diagnostics.len();
                    crate::proof_contracts::contract_entailment::specification_calls::validate_proof_body_formation(
                    program, machine, expression, diagnostics,

    formation_obligations,
    );
                    if diagnostics.len() != before {
                        continue;
                    }
                }
                if projection_result_expression(program, machine).is_none_or(|expression| {
                    normalize_projection_expression(program, subject, &algebra, expression)
                        .is_none()
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` is outside the closed projection fragment; use one selected algebra constructor over subject field reads, proof-natural constructors, and closed arithmetic only",
                    machine.name
                )));
                    continue;
                }

                if let Some((_, _, first)) = published.iter().find(|(semantic_id, symbol, _)| {
                    (candidate.semantic_domain.is_valid()
                        && *semantic_id == candidate.semantic_domain)
                        || (!candidate.semantic_domain.is_valid() && *symbol == domain.symbol)
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                    "exact qualification `{}` publishes more than one `Content<Owner, A>` projection (`{first}` and `{}`); one qualification has one owner-unique projection identity",
                    domain.name, machine.name
                )));
                    continue;
                }
                published.push((
                    candidate.semantic_domain,
                    domain.symbol,
                    machine.name.to_string(),
                ));
            }
        }
    }

    pub fn build_content_projection_plans(
        program: &dyn CheckingInput,
    ) -> Vec<ContentProjectionPlan> {
        program
            .machines()
            .iter()
            .flat_map(|machine| {
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter_map(move |conformance| {
                        let content = program
                            .trait_definition_by_symbol(conformance.symbol)
                            .filter(|candidate| is_content_projection_trait(program, candidate))?;
                        if conformance.requirement.as_ref().map(|name| name.as_str())
                            != Some("project")
                        {
                            return None;
                        }
                        let subject = projection_subject(program, machine)?;
                        let candidates = projection_domain_candidates(program, machine, subject);
                        let [candidate] = candidates.as_slice() else {
                            return None;
                        };
                        let domain = candidate.definition;
                        if domain.alias.is_some() || content.is_boundary {
                            return None;
                        }
                        let algebra = selected_content_algebra(program, conformance)?;
                        let expression = normalize_projection_expression(
                            program,
                            subject,
                            &algebra,
                            projection_result_expression(program, machine)?,
                        )?;
                        let report_fingerprint =
                            projection_report_fingerprint(&algebra, &expression);
                        Some(ContentProjectionPlan {
                            domain: domain.symbol,
                            semantic_domain: candidate.semantic_domain,
                            carrier_identity: program
                                .normalized_type_identity_text(subject.carrier),
                            machine: machine.symbol,
                            algebra,
                            expression,
                            report_fingerprint,
                        })
                    })
            })
            .collect()
    }

    pub(crate) fn is_content_projection_machine(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> bool {
        program
            .machine_trait_conformances(machine)
            .iter()
            .any(|conformance| {
                if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project")
                    || !program
                        .trait_definition_by_symbol(conformance.symbol)
                        .is_some_and(|candidate| is_content_projection_trait(program, candidate))
                {
                    return false;
                }
                let Some(subject) = projection_subject(program, machine) else {
                    return false;
                };
                let Some(algebra) = selected_content_algebra(program, conformance) else {
                    return false;
                };
                projection_result_expression(program, machine).is_some_and(|expression| {
                    normalize_projection_expression(program, subject, &algebra, expression)
                        .is_some()
                })
            })
    }

    fn selected_content_algebra(
        program: &dyn CheckingInput,
        conformance: &typed_trees::machine::TraitConformance,
    ) -> Option<ContentAlgebraIdentity> {
        let [_owner, argument] = program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        else {
            return None;
        };
        content_algebra_for_type(program, *argument)
    }

    /// The closed algebra carrier selected by content projections and explicit
    /// conservation identities. A same-spelled user declaration has no authority.
    pub fn content_algebra_for_type(
        program: &dyn CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<ContentAlgebraIdentity> {
        Some(content_algebra_application(program, reference)?.identity)
    }

    pub struct ContentAlgebraApplication {
        pub identity: ContentAlgebraIdentity,
        pub parameter: typed_trees::types::TypeReferenceHandle,
    }

    pub fn content_algebra_application(
        program: &dyn CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<ContentAlgebraApplication> {
        let TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if !compiler_owned_symbol(program, *base_symbol) {
            return None;
        }
        let [parameter] = program
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            return None;
        };
        let identity = program.normalized_type_identity_text(*parameter);
        let algebra = match base_name.as_str().rsplit("::").next() {
            Some("IntervalSet") => ContentAlgebraIdentity::IntervalSet {
                coordinate_space: identity,
            },
            Some("CountedQuantity") => ContentAlgebraIdentity::CountedQuantity { unit: identity },
            _ => return None,
        };
        Some(ContentAlgebraApplication {
            identity: algebra,
            parameter: *parameter,
        })
    }

    fn projection_result_expression(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ExpressionHandle> {
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let [statement] = program.statement_table.statements(state.statement_nodes) else {
            return None;
        };
        match statement {
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::Transition(transition)
                if transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() =>
            {
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(expression) => Some(*expression),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(crate) fn content_projection_body_expression(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ExpressionHandle> {
        program
            .machine_trait_conformances(machine)
            .iter()
            .any(|conformance| {
                conformance.requirement.as_ref().map(|name| name.as_str()) == Some("project")
                    && program
                        .trait_definition_by_symbol(conformance.symbol)
                        .is_some_and(|candidate| is_content_projection_trait(program, candidate))
            })
            .then(|| projection_result_expression(program, machine))
            .flatten()
    }

    fn normalize_projection_expression(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        algebra: &ContentAlgebraIdentity,
        expression: ExpressionHandle,
    ) -> Option<ContentProjectionExpression> {
        let literal = expression_table(program).struct_literal_node(expression)?;
        let expected_name = match algebra {
            ContentAlgebraIdentity::IntervalSet { .. } => "IntervalSet",
            ContentAlgebraIdentity::CountedQuantity { .. } => "CountedQuantity",
        };
        let literal_leaf = literal
            .type_name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(literal.type_name.as_str());
        if literal_leaf != expected_name || literal.case_name.is_some() {
            return None;
        }
        let fields = expression_table(program).struct_fields(literal.fields);
        match algebra {
            ContentAlgebraIdentity::IntervalSet { .. } => {
                let [start, end] = ["start", "end"].map(|name| {
                    fields
                        .iter()
                        .find(|field| field.name.as_str() == name)
                        .and_then(|field| {
                            normalize_projection_scalar(program, subject, field.value)
                        })
                });
                (fields.len() == 2).then_some(ContentProjectionExpression::IntervalSet {
                    members: vec![ContentIntervalExpression::new(start?, end?)],
                })
            }
            ContentAlgebraIdentity::CountedQuantity { .. } => {
                let magnitude = fields
                    .iter()
                    .find(|field| field.name.as_str() == "magnitude")
                    .and_then(|field| normalize_projection_scalar(program, subject, field.value))?;
                (fields.len() == 1)
                    .then_some(ContentProjectionExpression::CountedQuantity { magnitude })
            }
        }
    }

    fn normalize_projection_scalar(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> Option<ContentScalarExpression> {
        normalize_projection_scalar_context(program, subject, expression, false)
    }

    fn normalize_projection_scalar_context(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
        within_integer_coercion: bool,
    ) -> Option<ContentScalarExpression> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Cast(cast)
                if program
                    .named_type_reference(cast.target_type)
                    .is_some_and(|name| name.as_str() == "Nat")
                    && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
                    && cast.semantic_domain.count() == 0 =>
            {
                normalize_projection_scalar_context(program, subject, cast.value, true)
            }
            ExpressionNode::Integer(value) => {
                let value = value.value_bignum()?;
                (!value.is_negative()).then(|| ContentScalarExpression::Natural(value.to_string()))
            }
            ExpressionNode::Name(path) => {
                let members = expression_table(program).name_path_members(path.members);
                matches!(members, [first, second] if first.as_str() == "Nat" && second.as_str() == "Zero")
                .then(|| ContentScalarExpression::Natural("0".to_owned()))
            }
            ExpressionNode::Member(_) => normalize_subject_field(program, subject, expression)
                .map(|(path, _)| ContentScalarExpression::SubjectField(path)),
            ExpressionNode::Call(call)
                if within_integer_coercion
                    && crate::proof_contracts::proof_embeddings::is_exact_embed_call(
                        program, call,
                    ) =>
            {
                let [argument] = expression_table(program).expression_handles(call.arguments)
                else {
                    return None;
                };
                let (path, field_type) = normalize_subject_field(program, subject, *argument)?;
                runtime_scalar_can_embed(program, field_type)
                    .then_some(ContentScalarExpression::RuntimeScalarEmbedding(path))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                ) && projection_arithmetic_has_integer_meaning(program, subject, expression) =>
            {
                Some(ContentScalarExpression::Arithmetic {
                    operator: match binary.operator {
                        BinaryOperator::Add => ContentArithmeticOperator::Add,
                        BinaryOperator::Subtract => ContentArithmeticOperator::Subtract,
                        BinaryOperator::Multiply => ContentArithmeticOperator::Multiply,
                        _ => unreachable!("guarded closed arithmetic operator"),
                    },
                    left: Box::new(normalize_projection_scalar_context(
                        program,
                        subject,
                        binary.left,
                        within_integer_coercion,
                    )?),
                    right: Box::new(normalize_projection_scalar_context(
                        program,
                        subject,
                        binary.right,
                        within_integer_coercion,
                    )?),
                })
            }
            ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str().rsplit("::").next() == Some("Nat") =>
            {
                match literal.case_name.as_ref().map(|name| name.as_str()) {
                    Some("Zero")
                        if expression_table(program)
                            .struct_fields(literal.fields)
                            .is_empty() =>
                    {
                        Some(ContentScalarExpression::Natural("0".to_owned()))
                    }
                    Some("Succ") => {
                        let fields = expression_table(program).struct_fields(literal.fields);
                        let [field] = fields else {
                            return None;
                        };
                        if field.name.as_str() != "prev" {
                            return None;
                        }
                        Some(ContentScalarExpression::Successor(Box::new(
                            normalize_projection_scalar(program, subject, field.value)?,
                        )))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn projection_arithmetic_has_integer_meaning(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> bool {
        use crate::value_custody::expression_types::{BuiltinBinaryResult, builtin_binary_result};
        use numerics::arithmetic::ArithmeticDomain;

        let Some(machine) = program.machine_by_symbol(subject.machine) else {
            return false;
        };
        let Some(state) = program.state_by_symbol(subject.state) else {
            return false;
        };
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        let mut expression_type =
            |operand| crate::expression_result_type_reference(program, machine, state, operand);
        let operands = [binary.left, binary.right].map(&mut expression_type);
        if !crate::proof_contracts::bound_expression_meaning::builtin_arithmetic_for_operand_types(
            program,
            machine.symbol,
            expression,
            &operands,
        ) {
            return false;
        }
        if [binary.left, binary.right].into_iter().all(|operand| {
            crate::value_custody::literals::has_anonymous_numeric_results(program, operand)
        }) {
            return true;
        }
        match builtin_binary_result(program, binary, operands, &mut expression_type) {
            Some(BuiltinBinaryResult::ProofInteger) => true,
            Some(BuiltinBinaryResult::Arithmetic {
                symbol,
                policy: ArithmeticDomain::Exact,
            }) => program
                .symbols
                .builtin_type_atom(symbol)
                .is_some_and(symbols::BuiltinTypeAtom::is_fixed_integer),
            _ => false,
        }
    }

    fn normalize_subject_field(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> Option<(Vec<ContentFieldSegment>, TypeReferenceHandle)> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) if path.symbol == subject.symbol => {
                Some((Vec::new(), subject.carrier))
            }
            ExpressionNode::Member(member) => {
                let (mut path, receiver_type) =
                    normalize_subject_field(program, subject, member.receiver)?;
                let field = data_field(program, receiver_type, member.member.as_str())?;
                path.push(ContentFieldSegment {
                    symbol: field.symbol,
                    name: member.member.as_str().to_owned(),
                });
                Some((path, field.type_reference))
            }
            _ => None,
        }
    }

    fn data_field<'program>(
        program: &'program dyn CheckingInput,
        receiver_type: TypeReferenceHandle,
        name: &str,
    ) -> Option<&'program typed_trees::data::DataField> {
        let receiver_type = program
            .type_reference_table
            .peeled_type_reference(receiver_type);
        let (symbol, fallback) = match program.type_reference_table.type_reference(receiver_type) {
            TypeReferenceNode::Named { symbol, name } => (*symbol, name),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => (*base_symbol, base_name),
            _ => return None,
        };
        let definition =
            crate::field_domains::data_definition_by_symbol_or_name(program, symbol, fallback)?;
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == name => Some(field),
                _ => None,
            })
    }

    fn is_content_projection_trait(
        program: &dyn CheckingInput,
        candidate: &TraitDefinition,
    ) -> bool {
        compiler_owned_symbol(program, candidate.symbol)
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|name| name == "Content")
            && !candidate.is_boundary
            && program.trait_type_parameters(candidate).len() == 2
            && matches!(program.trait_machine_signatures(candidate), [requirement] if requirement.name.as_str() == "project")
    }

    fn runtime_scalar_can_embed(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let type_reference = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Named { name, .. }
                if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "addr")
        )
    }

    fn compiler_owned_symbol(program: &dyn CheckingInput, symbol: SymbolHandle) -> bool {
        let owning_machine = program.machine_by_own_or_state_symbol(symbol);
        let declaration = owning_machine.map_or(symbol, |machine| machine.symbol);
        let authored = program
            .machine_specialization(declaration)
            .map_or(declaration, |specialization| specialization.template);
        match program.symbols.symbol_source_origin(authored) {
            Some(source::SourceOrigin::Toolchain) => true,
            Some(source::SourceOrigin::User) => false,
            None => !program.symbols.has_source_metadata(),
        }
    }

    fn projection_subject(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ProjectionSubject> {
        let state = program.machine_states(machine).first()?;
        let [parameter] = program.state_parameters(state) else {
            return None;
        };
        if parameter.is_mutable {
            return None;
        }
        match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() && !access.is_exclusive() => Some(ProjectionSubject {
                machine: machine.symbol,
                state: state.symbol,
                symbol: parameter.symbol,
                qualified: *referee,
                carrier: program
                    .type_reference_table
                    .unconstrained_type_reference(*referee),
            }),
            _ => None,
        }
    }

    fn projection_domain_candidates<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        subject: ProjectionSubject,
    ) -> Vec<ProjectionDomainCandidate<'program>> {
        let Some(owner) = machine.attached_data.as_ref().map(|name| name.as_str()) else {
            return Vec::new();
        };
        let owner_leaf = owner.rsplit("::").next().unwrap_or(owner);
        program
            .domain_definitions()
            .iter()
            .filter_map(|domain| {
                if program.symbols.has_source_metadata()
                    && !program
                        .symbols
                        .same_symbol_source_package(machine.symbol, domain.symbol)
                {
                    return None;
                }
                let name = domain.name.as_str();
                let leaf = name.rsplit("::").next().unwrap_or(name);
                if (name != owner && leaf != owner_leaf)
                    || !domain_accepts_projection_carrier(program, domain, subject.carrier)
                {
                    return None;
                }
                let exact = declared_domain_constraints(program, subject.qualified)
                    .into_iter()
                    .filter(|constraint| {
                        (constraint.symbol.is_valid() && constraint.symbol == domain.symbol)
                            || (!constraint.symbol.is_valid()
                                && constraint.name.as_str().rsplit("::").next() == Some(owner_leaf))
                    })
                    .collect::<Vec<_>>();
                match exact.as_slice() {
                    [constraint] => Some(ProjectionDomainCandidate {
                        definition: domain,
                        semantic_domain: constraint.semantic_id,
                    }),
                    [] if domain.index_arguments.is_empty() => Some(ProjectionDomainCandidate {
                        definition: domain,
                        semantic_domain: domain.semantic_id,
                    }),
                    _ => None,
                }
            })
            .collect()
    }

    fn domain_accepts_projection_carrier(
        program: &dyn CheckingInput,
        domain: &DomainDefinition,
        carrier: TypeReferenceHandle,
    ) -> bool {
        if type_references_match(program, carrier, domain.target_type) {
            return true;
        }
        let parameters = program.domain_type_parameters(domain);
        let TypeReferenceNode::Named { symbol, name } = program
            .type_reference_table
            .type_reference(domain.target_type)
        else {
            return false;
        };
        typed_trees::domain::has_generic_carrier(program, domain)
            && carrier.is_valid()
            && parameters.iter().any(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    && (*symbol == parameter.symbol || name == &parameter.name)
            })
    }

    fn declared_domain_constraints(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<&typed_trees::types::DomainConstraint> {
        typed_trees::operator::declared_domain_constraints(program.typed_program(), type_reference)
    }
}
