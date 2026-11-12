/*
Affine cleanup emission (`affine_cleanup`), borrowed storage move-out/restore windows
(`borrowed_window`), and continuation identity/specialization rewrites (`continuation_identities`,
`continuation_specialization`).
*/

pub(crate) use crate::emission::*;

pub(crate) mod affine_cleanup {
    use crate::shared::lowering_error::{LoweringError, unsupported};
    use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
    use terminal_psi::{
        Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, TerminalAffineCleanupAction, Terminator,
    };

    pub(crate) mod carrier_arrivals {

        use super::{LoweringError, unsupported};
        use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
        use terminal_psi::{
            Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, Terminator,
        };

        struct Arrival {
            place: PlaceId,
            sources: Vec<PlaceId>,
        }

        pub(crate) fn build_owned_affine_types(
            parameters: &[StructuralParameterDeclaration],
            local_places: &[StructuralPlaceDeclaration],
            blocks: &[Block],
        ) -> std::collections::HashMap<PlaceId, Option<StructuralTypeId>> {
            let mut map: std::collections::HashMap<PlaceId, Option<StructuralTypeId>> =
                std::collections::HashMap::new();
            let mut record =
                |place: PlaceId, valid_type: Option<StructuralTypeId>| match map.entry(place) {
                    std::collections::hash_map::Entry::Vacant(vacant) => {
                        vacant.insert(valid_type);
                    }
                    std::collections::hash_map::Entry::Occupied(mut occupied) => {
                        *occupied.get_mut() = None;
                    }
                };
            for parameter in parameters
                .iter()
                .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
            {
                record(
                    parameter.place,
                    (parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Affine)
                        .then_some(parameter.structural_type),
                );
            }
            for operation in blocks.iter().flat_map(|block| &block.operations) {
                if let Some(result) = operation.result.structural() {
                    record(
                        result.place,
                        (result.multiplicity == StructuralMultiplicity::Affine)
                            .then_some(result.structural_type),
                    );
                }
            }
            for local in local_places {
                if let StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                } = local.kind
                {
                    record(local.id, Some(structural_type));
                }
            }
            map
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn inherit(
            managed: &mut Vec<PlaceId>,
            parameters: &[StructuralParameterDeclaration],
            local_places: &[StructuralPlaceDeclaration],
            blocks: &[Block],
            types: &[StructuralTypeDeclaration],
            partial_transfers: &[PlaceId],
            scheduled: &[PlaceId],
        ) -> Result<(), LoweringError> {
            let has_candidates = blocks.iter().any(|block| {
                block.structural_parameters.iter().any(|parameter| {
                    parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Affine
                        && descriptor_type(types, parameter.structural_type)
                })
            });
            if !has_candidates {
                return Ok(());
            }
            let owned_affine_types = build_owned_affine_types(parameters, local_places, blocks);
            let mut parameter_place_counts: std::collections::HashMap<PlaceId, usize> =
                std::collections::HashMap::new();
            for block in blocks {
                for parameter in &block.structural_parameters {
                    *parameter_place_counts.entry(parameter.place).or_insert(0) += 1;
                }
            }
            let mut arrivals = Vec::new();
            for block in blocks {
                for (position, parameter) in block.structural_parameters.iter().enumerate() {
                    if managed.contains(&parameter.place)
                        || parameter.access != StructuralAccess::Owned
                        || parameter.multiplicity != StructuralMultiplicity::Affine
                        || partial_transfers.contains(&parameter.place)
                        || scheduled.contains(&parameter.place)
                        || !descriptor_type(types, parameter.structural_type)
                    {
                        continue;
                    }
                    if block
                        .structural_parameters
                        .iter()
                        .enumerate()
                        .any(|(index, parameter)| {
                            usize::try_from(parameter.position).ok() != Some(index)
                        })
                        || parameter_place_counts.get(&parameter.place).copied() != Some(1)
                    {
                        return unsupported("descriptor arrival has no unique positional binding");
                    }
                    let mut sources = Vec::new();
                    let mut complete = true;
                    for predecessor in blocks {
                        let edges = match &predecessor.terminator {
                            Terminator::Jump {
                                target,
                                structural_arguments,
                                cleanup_actions,
                                ..
                            } if *target == block.id => {
                                vec![(structural_arguments.as_slice(), cleanup_actions.as_slice())]
                            }
                            Terminator::Conditional {
                                when_true,
                                when_false,
                                ..
                            } => [when_true, when_false]
                                .into_iter()
                                .filter(|edge| edge.target == block.id)
                                .map(|edge| {
                                    (
                                        edge.structural_arguments.as_slice(),
                                        edge.cleanup_actions.as_slice(),
                                    )
                                })
                                .collect(),
                            Terminator::StructuralCase { cases, .. }
                                if cases.iter().any(|edge| edge.target == block.id) =>
                            {
                                complete = false;
                                Vec::new()
                            }
                            _ => Vec::new(),
                        };
                        for (arguments, cleanup) in edges {
                            if arguments.len() != block.structural_parameters.len() {
                                return unsupported(
                                    "descriptor arrival has an incomplete edge roster",
                                );
                            }
                            let source = &arguments[position];
                            if source.access != StructuralAccess::Owned
                                || !source.path.is_empty()
                                || partial_transfers.contains(&source.place)
                                || cleanup.iter().any(|action| action.place() == source.place)
                                || owned_affine_types.get(&source.place).copied().flatten()
                                    != Some(parameter.structural_type)
                            {
                                complete = false;
                            }
                            sources.push(source.place);
                        }
                    }
                    if complete && !sources.is_empty() {
                        arrivals.push(Arrival {
                            place: parameter.place,
                            sources,
                        });
                    }
                }
            }
            let mut inherited = Vec::new();
            loop {
                let previous = inherited.len();
                for arrival in &arrivals {
                    if !inherited.contains(&arrival.place)
                        && arrival
                            .sources
                            .iter()
                            .any(|source| managed.contains(source) || inherited.contains(source))
                    {
                        inherited.push(arrival.place);
                    }
                }
                if previous == inherited.len() {
                    break;
                }
            }
            loop {
                let previous = inherited.clone();
                inherited.retain(|place| {
                    arrivals
                        .iter()
                        .find(|arrival| arrival.place == *place)
                        .is_some_and(|arrival| {
                            arrival
                                .sources
                                .iter()
                                .all(|source| managed.contains(source) || previous.contains(source))
                        })
                });
                if inherited == previous {
                    break;
                }
            }
            managed.extend(inherited);
            Ok(())
        }

        fn descriptor_type(
            types: &[StructuralTypeDeclaration],
            identity: StructuralTypeId,
        ) -> bool {
            let mut matches = types
                .iter()
                .filter(|declaration| declaration.id == identity);
            let Some(declaration) = matches.next() else {
                return false;
            };
            matches.next().is_none()
                && (matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference { .. } | StructuralTypeShape::ElementView { .. }
                ) || declaration.shape.is_borrowed_byte_view())
        }
    }
    pub(crate) mod custody_flow {

        use super::{LoweringError, projected_transfers, unsupported};
        use semantic_vocabulary::{OperationId, PlaceId};
        use std::collections::{BTreeSet, HashMap, VecDeque};
        use terminal_psi::{Block, OperationKind, Terminator};

        pub(crate) struct Frontiers {
            pub(crate) entries: Vec<Vec<PlaceId>>,
            pub(crate) exits: Vec<Vec<PlaceId>>,
            pub(crate) outgoing: Vec<Vec<Vec<PlaceId>>>,
        }

        struct EdgeEffects {
            consumed: Vec<usize>,
            disposed: Vec<usize>,
        }

        fn edge_effects(
            block: &Block,
            managed_index: &HashMap<PlaceId, usize>,
            copied_roots: &BTreeSet<PlaceId>,
        ) -> Result<Vec<EdgeEffects>, LoweringError> {
            let edges: Vec<(
                &[terminal_psi::StructuralArgument],
                &[terminal_psi::TerminalAffineCleanupAction],
            )> = match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    cleanup_actions,
                    ..
                } => vec![(structural_arguments, cleanup_actions)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .map(|edge| {
                        (
                            edge.structural_arguments.as_slice(),
                            edge.cleanup_actions.as_slice(),
                        )
                    })
                    .collect(),
                Terminator::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|edge| {
                        (
                            edge.structural_arguments.as_slice(),
                            edge.cleanup_actions.as_slice(),
                        )
                    })
                    .collect(),
                _ => Vec::new(),
            };
            edges
                .into_iter()
                .map(|(arguments, discards)| {
                    let mut consumed = Vec::new();
                    for argument in arguments {
                        if !projected_transfers::transfers_custody(argument, copied_roots) {
                            continue;
                        }
                        let Some(&position) = managed_index.get(&argument.place) else {
                            continue;
                        };
                        if !argument.path.is_empty() {
                            continue;
                        }
                        consumed.push(position);
                    }
                    let disposed = discards
                        .iter()
                        .filter_map(|action| match action {
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                            | terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => {
                                managed_index.get(place).copied()
                            }
                            _ => None,
                        })
                        .collect();
                    Ok(EdgeEffects { consumed, disposed })
                })
                .collect()
        }

        enum Transfer {
            Consume {
                position: usize,
                missing: &'static str,
            },
            Birth {
                position: usize,
            },
        }

        fn transfers(
            block: &Block,
            entry_owners: &[PlaceId],
            managed_index: &HashMap<PlaceId, usize>,
            births_by_op: &HashMap<OperationId, Vec<PlaceId>>,
            copied_roots: &BTreeSet<PlaceId>,
        ) -> Result<Vec<Transfer>, LoweringError> {
            let mut transfers = Vec::new();
            let position = |place: PlaceId| managed_index.get(&place).copied();
            for parameter in &block.structural_parameters {
                if !entry_owners.contains(&parameter.place)
                    && let Some(position) = position(parameter.place)
                {
                    transfers.push(Transfer::Birth { position });
                }
            }
            for operation in &block.operations {
                for argument in projected_transfers::operands(&operation.kind) {
                    if !projected_transfers::transfers_custody(argument, copied_roots) {
                        continue;
                    }
                    let Some(position) = position(argument.place) else {
                        continue;
                    };
                    if !argument.path.is_empty() {
                        continue;
                    }
                    transfers.push(Transfer::Consume {
                        position,
                        missing: "scalar graph owned operand transfers an unavailable owned parameter",
                    });
                }
                if let OperationKind::ReleaseReference { source, path } = &operation.kind
                    && path.is_empty()
                    && let Some(position) = position(*source)
                {
                    transfers.push(Transfer::Consume {
                        position,
                        missing: "reference release consumes an unavailable carrier",
                    });
                }
                if let Some(places) = births_by_op.get(&operation.id) {
                    for place in places {
                        let Some(position) = position(*place) else {
                            return unsupported(
                                "scalar graph carrier birth is absent from its managed roster",
                            );
                        };
                        transfers.push(Transfer::Birth { position });
                    }
                }
            }
            if let Terminator::ReturnStructural { source, .. } = &block.terminator
                && let Some(position) = position(*source)
            {
                transfers.push(Transfer::Consume {
                    position,
                    missing: "structural completion consumes an unavailable owned carrier",
                });
            }
            Ok(transfers)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn solve(
            blocks: &[Block],
            entry: usize,
            entry_owners: &[PlaceId],
            managed: &[PlaceId],
            births: &[(PlaceId, OperationId)],
            copied_roots: &BTreeSet<PlaceId>,
            incoming: &[Vec<usize>],
            successors: &[Vec<usize>],
        ) -> Result<Frontiers, LoweringError> {
            let mut reachable = vec![false; blocks.len()];
            let mut pending = vec![entry];
            while let Some(position) = pending.pop() {
                if !reachable[position] {
                    reachable[position] = true;
                    pending.extend(successors[position].iter().copied());
                }
            }
            if reachable.iter().any(|reached| !reached) {
                return unsupported("owned scalar graph has unreachable custody");
            }
            let managed_index: HashMap<PlaceId, usize> = managed
                .iter()
                .enumerate()
                .map(|(index, &place)| (place, index))
                .collect();
            let mut births_by_op: HashMap<OperationId, Vec<PlaceId>> =
                HashMap::with_capacity(births.len());
            for &(place, op) in births {
                births_by_op.entry(op).or_default().push(place);
            }
            let transfers = blocks
                .iter()
                .map(|block| {
                    transfers(
                        block,
                        entry_owners,
                        &managed_index,
                        &births_by_op,
                        copied_roots,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let edges = blocks
                .iter()
                .map(|block| edge_effects(block, &managed_index, copied_roots))
                .collect::<Result<Vec<_>, _>>()?;
            if edges
                .iter()
                .zip(successors)
                .any(|(edges, successors)| edges.len() != successors.len())
            {
                return unsupported("owned edge custody disagrees with the control graph");
            }
            let mut entries = vec![vec![true; managed.len()]; blocks.len()];
            entries[entry] = managed
                .iter()
                .map(|place| entry_owners.contains(place))
                .collect();
            let mut exits = vec![vec![true; managed.len()]; blocks.len()];
            let mut ready = (0..blocks.len()).collect::<VecDeque<_>>();
            let mut queued = vec![true; blocks.len()];
            while let Some(position) = ready.pop_front() {
                queued[position] = false;
                if position != entry {
                    if incoming[position].is_empty() {
                        return unsupported(
                            "owned scalar graph has a reachable state without incoming custody",
                        );
                    }
                    let mut joined = vec![true; managed.len()];
                    for predecessor in &incoming[position] {
                        for (edge, target) in
                            edges[*predecessor].iter().zip(&successors[*predecessor])
                        {
                            if *target != position {
                                continue;
                            }
                            for (owner, held) in joined.iter_mut().enumerate() {
                                *held &= exits[*predecessor][owner]
                                    && !edge.consumed.contains(&owner)
                                    && !edge.disposed.contains(&owner);
                            }
                        }
                    }
                    entries[position] = joined;
                }
                let mut live = entries[position].clone();
                for transfer in &transfers[position] {
                    match transfer {
                        Transfer::Consume { position, .. } => live[*position] = false,
                        Transfer::Birth { position } => live[*position] = true,
                    }
                }
                if live != exits[position] {
                    exits[position] = live;
                    for successor in &successors[position] {
                        if *successor != entry && !queued[*successor] {
                            queued[*successor] = true;
                            ready.push_back(*successor);
                        }
                    }
                }
            }
            for (entry, transfers) in entries.iter().zip(&transfers) {
                let mut live = entry.clone();
                for transfer in transfers {
                    match transfer {
                        Transfer::Consume { position, missing } => {
                            if !live[*position] {
                                return unsupported(missing);
                            }
                            live[*position] = false;
                        }
                        Transfer::Birth { position } => {
                            if live[*position] {
                                return unsupported("scalar graph repeats a live descriptor birth");
                            }
                            live[*position] = true;
                        }
                    }
                }
            }
            let mut outgoing = Vec::with_capacity(blocks.len());
            for (exit, edges) in exits.iter().zip(&edges) {
                let mut outgoing_edges = Vec::with_capacity(edges.len());
                for edge in edges {
                    let mut live = exit.clone();
                    for position in &edge.consumed {
                        if !live[*position] {
                            return unsupported("owned edge transfers an unavailable owner");
                        }
                        live[*position] = false;
                    }
                    outgoing_edges.push(
                        managed
                            .iter()
                            .zip(live)
                            .filter_map(|(place, live)| live.then_some(*place))
                            .collect(),
                    );
                }
                outgoing.push(outgoing_edges);
            }
            let places = |frontiers: Vec<Vec<bool>>| {
                frontiers
                    .into_iter()
                    .map(|frontier| {
                        managed
                            .iter()
                            .zip(frontier)
                            .filter_map(|(place, live)| live.then_some(*place))
                            .collect()
                    })
                    .collect()
            };
            Ok(Frontiers {
                entries: places(entries),
                exits: places(exits),
                outgoing,
            })
        }
    }
    pub(crate) mod projected_transfers {
        use super::{LoweringError, block_position, unsupported};
        use crate::shared::call_operands::structural_arguments;
        use crate::shared::lowering_error::ProjectedCustodyObservation;
        use semantic_vocabulary::{
            BlockId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId,
        };
        use std::collections::{BTreeSet, VecDeque};
        use terminal_psi::{
            Block, OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
            StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
            StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
            Terminator,
        };

        use frontier::{Frontier, MovedRoot};

        /// What one program point knows about storage roots that had content
        /// moved out of them. The fields are private to this module so that every
        /// read of a root goes through [`Frontier::root`], which refuses a root
        /// whose arriving paths disagree.
        mod frontier {
            use super::{LoweringError, PlaceId, StructuralPathSegment, unsupported};
            use std::collections::BTreeSet;

            const DISPUTED: &str = "projected transfer histories disagree at a control join";

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub(super) struct MovedRoot {
                pub(super) place: PlaceId,
                pub(super) paths: BTreeSet<Vec<StructuralPathSegment>>,
            }

            #[derive(Clone, Debug, Default, PartialEq, Eq)]
            pub(super) struct Frontier {
                /// Roots with moved-out paths, ordered by place.
                moved: Vec<MovedRoot>,
                /// Roots that reached a join with different moved-out paths. A
                /// join only has to agree about a root that is used again, so a
                /// disagreement is kept here instead of refusing the join. The
                /// root stays unusable until an operation or block parameter
                /// establishes it afresh. Ordered by place.
                disputed: Vec<PlaceId>,
            }

            impl Frontier {
                /// Refuses a root whose arrivals disagree.
                fn settled(&self, place: PlaceId) -> Result<(), LoweringError> {
                    if self.disputed.binary_search(&place).is_ok() {
                        return unsupported(DISPUTED);
                    }
                    Ok(())
                }

                fn dispute(&mut self, place: PlaceId) {
                    if let Err(position) = self.disputed.binary_search(&place) {
                        self.disputed.insert(position, place);
                    }
                }

                /// The moved-out paths of `place`, `None` when the root is whole.
                /// Any use of a root must ask here first: a disputed root has no
                /// single answer, so the use is refused.
                pub(super) fn root(
                    &self,
                    place: PlaceId,
                ) -> Result<Option<&MovedRoot>, LoweringError> {
                    self.settled(place)?;
                    Ok(self.moved.iter().find(|root| root.place == place))
                }

                pub(super) fn root_mut(
                    &mut self,
                    place: PlaceId,
                ) -> Result<Option<&mut MovedRoot>, LoweringError> {
                    self.settled(place)?;
                    Ok(self.moved.iter_mut().find(|root| root.place == place))
                }

                /// The moved-out paths of `place`, starting an empty record when
                /// the root was whole.
                pub(super) fn root_or_open(
                    &mut self,
                    place: PlaceId,
                ) -> Result<&mut MovedRoot, LoweringError> {
                    self.settled(place)?;
                    let position = match self.moved.binary_search_by_key(&place, |root| root.place)
                    {
                        Ok(position) => position,
                        Err(position) => {
                            self.moved.insert(
                                position,
                                MovedRoot {
                                    place,
                                    paths: BTreeSet::new(),
                                },
                            );
                            position
                        }
                    };
                    Ok(&mut self.moved[position])
                }

                /// Records the first moved-out path of a root that was whole.
                pub(super) fn open(&mut self, place: PlaceId, path: Vec<StructuralPathSegment>) {
                    self.moved.push(MovedRoot {
                        place,
                        paths: BTreeSet::from([path]),
                    });
                    self.moved.sort_by_key(|root| root.place);
                }

                /// Drops the moved-out paths of a root whose region has ended or
                /// been restored. A disputed root stays disputed.
                pub(super) fn close(&mut self, place: PlaceId) {
                    self.moved.retain(|root| root.place != place);
                }

                /// A new value now lives at `place`, so nothing about the value
                /// that was there before applies, including a dispute.
                pub(super) fn establish(&mut self, place: PlaceId) {
                    self.close(place);
                    self.disputed.retain(|disputed| *disputed != place);
                }

                /// Merges another arrival at the same block and reports whether
                /// this frontier changed. Roots both arrivals describe alike are
                /// kept; every other root becomes disputed.
                pub(super) fn join(&mut self, arrival: &Self) -> bool {
                    let before = self.disputed.len();
                    for place in &arrival.disputed {
                        self.dispute(*place);
                    }
                    for root in &arrival.moved {
                        if !self.moved.contains(root) {
                            self.dispute(root.place);
                        }
                    }
                    let unmatched = self
                        .moved
                        .iter()
                        .filter(|root| !arrival.moved.contains(root))
                        .map(|root| root.place)
                        .collect::<Vec<_>>();
                    for place in unmatched {
                        self.dispute(place);
                    }
                    let disputed = &self.disputed;
                    self.moved
                        .retain(|root| disputed.binary_search(&root.place).is_err());
                    self.disputed.len() != before
                }
            }
        }

        pub(crate) fn owned_extractions(
            parameters: &[StructuralParameterDeclaration],
            local_places: &[StructuralPlaceDeclaration],
            blocks: &[Block],
            types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<(OperationId, StructuralArgument)>, LoweringError> {
            let has_extraction = blocks.iter().any(|block| {
                block
                    .operations
                    .iter()
                    .any(|op| matches!(op.kind, OperationKind::MoveStructuralField { .. }))
            });
            if !has_extraction {
                return Ok(Vec::new());
            }
            let roots = parameters
                .iter()
                .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
                .map(|parameter| (parameter.place, parameter.structural_type, parameter.access))
                .chain(
                    blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| {
                            operation.result.structural().map(|result| {
                                (
                                    result.place,
                                    result.structural_type,
                                    StructuralAccess::Owned,
                                )
                            })
                        }),
                )
                .chain(local_places.iter().filter_map(|local| match local.kind {
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    } => Some((local.id, structural_type, StructuralAccess::Owned)),
                    _ => None,
                }))
                .collect::<Vec<_>>();
            let mut extractions = Vec::new();
            for operation in blocks.iter().flat_map(|block| &block.operations) {
                let OperationKind::MoveStructuralField {
                    source,
                    path,
                    field,
                } = &operation.kind
                else {
                    continue;
                };
                let mut declarations = roots.iter().filter(|(place, _, _)| place == source);
                let (_, root_type, access) = declarations.next().ok_or(
                    LoweringError::Unsupported("extraction source has no storage declaration"),
                )?;
                if declarations.any(|(_, other_type, other_access)| {
                    other_type != root_type || other_access != access
                }) {
                    return unsupported("extraction source has inconsistent storage declarations");
                }
                if *access != StructuralAccess::Owned
                    || path.contains(&StructuralPathSegment::Referent)
                {
                    continue;
                }
                if types.iter().any(|declaration| {
                    declaration.id == *root_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        )
                }) {
                    continue;
                }
                let projection = terminal_semantics::structural_window_projection(
                    types.iter(),
                    *root_type,
                    path,
                    *field,
                )
                .ok_or(LoweringError::Unsupported(
                    "owned extraction has no exact storage projection",
                ))?;
                extractions.push((
                    operation.id,
                    StructuralArgument {
                        place: *source,
                        path: projection.path,
                        access: StructuralAccess::Owned,
                    },
                ));
            }
            Ok(extractions)
        }

        pub(crate) fn copied_operand_roots(
            parameters: &[StructuralParameterDeclaration],
            blocks: &[Block],
        ) -> BTreeSet<PlaceId> {
            parameters
                .iter()
                .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
                .filter(|parameter| {
                    parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                })
                .map(|parameter| parameter.place)
                .chain(
                    blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| {
                            operation
                                .result
                                .structural()
                                .filter(|result| {
                                    result.multiplicity == StructuralMultiplicity::Unrestricted
                                })
                                .map(|result| result.place)
                        }),
                )
                .collect()
        }

        pub(crate) fn transfers_custody(
            argument: &StructuralArgument,
            copied_roots: &BTreeSet<PlaceId>,
        ) -> bool {
            argument.access == StructuralAccess::Owned && !copied_roots.contains(&argument.place)
        }

        pub(crate) fn complete(
            completed_cleanup_frontiers: &[super::super::selected_continuations::CompletedCleanupFrontier],
            parameters: &[StructuralParameterDeclaration],
            local_places: &[StructuralPlaceDeclaration],
            entry: BlockId,
            blocks: &mut [Block],
            types: &[StructuralTypeDeclaration],
            extractions: &[(OperationId, StructuralArgument)],
        ) -> Result<(), LoweringError> {
            let completions = super::super::selected_continuations::CleanupCompletionIndex::new(
                completed_cleanup_frontiers,
            )?;
            let copied_roots = copied_operand_roots(parameters, blocks);
            if partitioned_roots(blocks, extractions, &copied_roots).is_empty() {
                return Ok(());
            }
            let roots = parameters
                .iter()
                .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
                .map(|parameter| (parameter.place, parameter.structural_type))
                .chain(
                    blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| {
                            operation
                                .result
                                .structural()
                                .map(|result| (result.place, result.structural_type))
                        }),
                )
                .chain(local_places.iter().filter_map(|local| match local.kind {
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    } => Some((local.id, structural_type)),
                    _ => None,
                }))
                .collect::<Vec<_>>();
            let mut incoming: Vec<Option<Frontier>> = vec![None; blocks.len()];
            let entry = block_position(blocks, entry)?;
            incoming[entry] = Some(Frontier::default());
            // Keep each saved frontier through every possible join comparison.
            // Once its block is processed and no predecessor can arrive again,
            // its paths are scratch storage with no remaining reader.
            let mut key_lifetimes =
                super::super::selected_continuations::IncomingKeyLifetimes::for_emitted_blocks(
                    blocks,
                );
            if let Some(lifetimes) = &mut key_lifetimes {
                lifetimes.discover(entry);
                lifetimes.close_unused_roots();
            }
            let mut ready = VecDeque::from([entry]);
            let mut queued = vec![false; blocks.len()];
            queued[entry] = true;
            let mut completed = blocks
                .iter()
                .map(|block| block.terminator.clone())
                .collect::<Vec<_>>();
            // A block is completed again whenever a later arrival disputes a root
            // it had taken as settled. Disputes only grow, so this ends, and each
            // pass starts over from the block's original cleanup roster.
            while let Some(position) = ready.pop_front() {
                queued[position] = false;
                let mut frontier = incoming[position].clone().expect("queued frontier");
                completed[position] = blocks[position].terminator.clone();
                for operation in &blocks[position].operations {
                    observe(&frontier, operation, &roots, types)?;
                    for argument in operands(&operation.kind) {
                        transfer(&mut frontier, argument, &copied_roots)?;
                    }
                    if let Some((_, extraction)) = extractions
                        .iter()
                        .find(|(identity, _)| *identity == operation.id)
                    {
                        if extraction.path.is_empty() {
                            if frontier.root(extraction.place)?.is_some() {
                                return unsupported(
                                    "whole extraction consumes a partially transferred root",
                                );
                            }
                            frontier.open(extraction.place, Vec::new());
                        } else {
                            remove_path(&mut frontier, extraction.place, &extraction.path)?;
                        }
                    }
                    if let OperationKind::ReleaseReference { source, path } = &operation.kind {
                        remove_path(&mut frontier, *source, path)?;
                    }
                    if let OperationKind::StoreStructuralField {
                        destination,
                        path,
                        field,
                        ..
                    } = &operation.kind
                        && frontier.root(*destination)?.is_some()
                    {
                        let root_type = roots
                            .iter()
                            .find_map(|(place, structural_type)| {
                                (*place == *destination).then_some(*structural_type)
                            })
                            .ok_or(LoweringError::Unsupported(
                                "restoration root has no storage declaration",
                            ))?;
                        let restored = terminal_semantics::structural_window_projection(
                            types.iter(),
                            root_type,
                            path,
                            *field,
                        )
                        .ok_or(LoweringError::Unsupported(
                            "restoration has no exact storage projection",
                        ))?;
                        restore_path(&mut frontier, *destination, &restored.path)?;
                    }
                    if let Some(result) = operation.result.structural() {
                        frontier.establish(result.place);
                    }
                }
                let mut successors = Vec::new();
                match &mut completed[position] {
                    Terminator::Jump {
                        edge,
                        target,
                        structural_arguments,
                        cleanup_actions,
                        ..
                    } => {
                        for argument in structural_arguments {
                            transfer(&mut frontier, argument, &copied_roots)?;
                        }
                        complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
                        successors.push((*edge, *target, frontier));
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for edge in [when_true, when_false] {
                            let mut branch = frontier.clone();
                            for argument in &edge.structural_arguments {
                                transfer(&mut branch, argument, &copied_roots)?;
                            }
                            complete_actions(
                                &mut branch,
                                &mut edge.cleanup_actions,
                                &roots,
                                types,
                            )?;
                            successors.push((edge.edge, edge.target, branch));
                        }
                    }
                    Terminator::StructuralCase {
                        source,
                        path,
                        cases,
                    } => {
                        if frontier.root(*source)?.is_some_and(|root| {
                            root.paths.iter().any(|moved| {
                                terminal_semantics::structural_paths_may_overlap(moved, path)
                            })
                        }) {
                            return unsupported("case inspection overlaps a transferred subtree");
                        }
                        for edge in cases {
                            let mut branch = frontier.clone();
                            complete_actions(
                                &mut branch,
                                &mut edge.cleanup_actions,
                                &roots,
                                types,
                            )?;
                            successors.push((edge.edge, edge.target, branch));
                        }
                    }
                    terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                        complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
                    }
                    Terminator::ReturnStructural {
                        source,
                        cleanup_actions,
                        ..
                    } => {
                        if frontier.root(*source)?.is_some() {
                            return unsupported(
                                "structural return consumes a partially transferred root",
                            );
                        }
                        complete_actions(&mut frontier, cleanup_actions, &roots, types)?;
                    }
                    Terminator::InvokeNoNormal {
                        structural_arguments,
                        ..
                    } => {
                        for argument in structural_arguments {
                            transfer(&mut frontier, argument, &copied_roots)?;
                        }
                    }
                    Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
                }
                for (edge, target, mut outgoing) in successors {
                    for completion in completions.at(edge) {
                        normalize_completed_frontier(&mut outgoing, completion)?;
                    }
                    let target = block_position(blocks, target)?;
                    for parameter in &blocks[target].structural_parameters {
                        outgoing.establish(parameter.place);
                    }
                    let changed = match &mut incoming[target] {
                        Some(previous) => previous.join(&outgoing),
                        first @ None => {
                            *first = Some(outgoing);
                            true
                        }
                    };
                    if changed && !queued[target] {
                        queued[target] = true;
                        // A block queued again is pending again, so the saved
                        // frontiers it can still reach stay alive until it is done.
                        if let Some(lifetimes) = &mut key_lifetimes {
                            lifetimes.discover(target);
                        }
                        ready.push_back(target);
                    }
                }
                if let Some(lifetimes) = &mut key_lifetimes {
                    // Finish all outgoing comparisons before closing this source.
                    for closed in lifetimes.processed(position) {
                        incoming[closed] = None;
                    }
                }
            }
            for (block, terminator) in blocks.iter_mut().zip(completed) {
                block.terminator = terminator;
            }
            Ok(())
        }

        pub(crate) fn partitioned_roots(
            blocks: &[Block],
            extractions: &[(OperationId, StructuralArgument)],
            copied_roots: &BTreeSet<PlaceId>,
        ) -> BTreeSet<PlaceId> {
            let mut roots = extractions
                .iter()
                .map(|(_, argument)| argument.place)
                .collect::<BTreeSet<_>>();
            for block in blocks {
                for argument in block
                    .operations
                    .iter()
                    .flat_map(|operation| operands(&operation.kind))
                    .chain(
                        block
                            .terminator
                            .successor_block_arguments()
                            .into_iter()
                            .flatten(),
                    )
                {
                    if transfers_custody(argument, copied_roots) && !argument.path.is_empty() {
                        roots.insert(argument.place);
                    }
                }
                for operation in &block.operations {
                    if let OperationKind::ReleaseReference { source, path } = &operation.kind
                        && !path.is_empty()
                    {
                        roots.insert(*source);
                    }
                }
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        let ends_region = match action {
                            TerminalAffineCleanupAction::DiscardResidual(_) => true,
                            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                                !cleanup.retains_descendants
                            }
                            _ => false,
                        };
                        if ends_region && !action.path().is_empty() {
                            roots.insert(action.place());
                        }
                    }
                }
            }
            roots
        }

        pub(crate) fn operands(kind: &OperationKind) -> impl Iterator<Item = &StructuralArgument> {
            let stored = match kind {
                OperationKind::StoreStructuralField { value, .. }
                | OperationKind::MoveStructuralValue { source: value } => Some(value),
                _ => None,
            };
            kind.constructor_arguments()
                .chain(stored)
                .chain(structural_arguments(kind))
        }

        fn transfer(
            frontier: &mut Frontier,
            argument: &StructuralArgument,
            copied_roots: &BTreeSet<PlaceId>,
        ) -> Result<(), LoweringError> {
            require_available(frontier, argument.place, &argument.path)?;
            if !transfers_custody(argument, copied_roots) {
                return Ok(());
            }
            remove_path(frontier, argument.place, &argument.path)
        }

        fn observe(
            frontier: &Frontier,
            operation: &terminal_psi::Operation,
            roots: &[(PlaceId, StructuralTypeId)],
            types: &[StructuralTypeDeclaration],
        ) -> Result<(), LoweringError> {
            let (result, kind) = match &operation.kind {
                OperationKind::StructuralCaseMembership { source, path, .. } => (
                    require_available(frontier, *source, path),
                    "case membership",
                ),
                OperationKind::StructuralLeafCopy { source, path } => (
                    require_available(frontier, *source, path),
                    "whole structural copy",
                ),
                OperationKind::StructuralCaseLeafCopy { source, path } => {
                    let root = declared_root_type(roots, *source)?;
                    let observed = terminal_semantics::canonical_structural_storage_path(
                        types.iter(),
                        root,
                        path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "projected copy has no exact storage projection",
                    ))?;
                    (
                        require_available(frontier, *source, &observed),
                        "projected structural copy",
                    )
                }
                _ => return Ok(()),
            };
            result.map_err(|mut error| {
                if let LoweringError::OverlappingProjectedCustody {
                    place, observation, ..
                } = &mut error
                {
                    *observation = Some(Box::new(ProjectedCustodyObservation {
                        operation: operation.id,
                        kind,
                        root_declarations: roots
                            .iter()
                            .filter(|(root, _)| root == place)
                            .map(|(_, structural_type)| *structural_type)
                            .collect(),
                    }));
                }
                error
            })
        }

        fn require_available(
            frontier: &Frontier,
            place: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<(), LoweringError> {
            if let Some(earlier) = frontier
                .root(place)?
                .into_iter()
                .flat_map(|root| &root.paths)
                .find(|earlier| terminal_semantics::structural_paths_may_overlap(earlier, path))
            {
                return Err(LoweringError::OverlappingProjectedCustody {
                    reason: "structural observation or cleanup overlaps an earlier transfer or release",
                    place,
                    earlier: earlier.clone(),
                    requested: path.to_vec(),
                    observation: None,
                });
            }
            Ok(())
        }

        fn declared_root_type(
            roots: &[(PlaceId, StructuralTypeId)],
            place: PlaceId,
        ) -> Result<StructuralTypeId, LoweringError> {
            let mut matching = roots.iter().filter(|(root, _)| *root == place);
            let (_, structural_type) = matching.next().ok_or(LoweringError::Unsupported(
                "projected custody root has no exact declaration",
            ))?;
            if matching.any(|(_, other)| other != structural_type) {
                return unsupported("projected custody root has inconsistent declarations");
            }
            Ok(*structural_type)
        }

        fn remove_path(
            frontier: &mut Frontier,
            place: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<(), LoweringError> {
            let root = frontier.root_mut(place)?;
            if path.is_empty() {
                if root.is_some() {
                    return unsupported("whole transfer consumes a partially transferred root");
                }
                return Ok(());
            }
            if let Some(root) = root {
                if let Some(earlier) = root.paths.iter().find(|earlier| {
                    earlier.starts_with(path) || path.starts_with(earlier.as_slice())
                }) {
                    return Err(LoweringError::OverlappingProjectedCustody {
                        reason: "projected custody overlaps an earlier transfer or release",
                        place,
                        earlier: earlier.clone(),
                        requested: path.to_vec(),
                        observation: None,
                    });
                }
                root.paths.insert(path.to_vec());
            } else {
                frontier.open(place, path.to_vec());
            }
            Ok(())
        }

        fn normalize_completed_frontier(
            frontier: &mut Frontier,
            completion: &super::super::selected_continuations::CompletedCleanupFrontier,
        ) -> Result<(), LoweringError> {
            use super::super::selected_continuations::CompletedCleanupKind;
            let selected = &completion.region;
            let paths = match &completion.kind {
                CompletedCleanupKind::Value => vec![selected.path.clone()],
                CompletedCleanupKind::CasePayloads(cases) => cases
                    .iter()
                    .map(|case| {
                        let mut path = selected.path.clone();
                        path.push(StructuralPathSegment::Case(*case));
                        path
                    })
                    .collect(),
            };
            let root = frontier.root_or_open(selected.place)?;
            for path in paths {
                if !root.paths.iter().any(|earlier| path.starts_with(earlier)) {
                    root.paths.retain(|earlier| !earlier.starts_with(&path));
                    root.paths.insert(path);
                }
            }
            Ok(())
        }

        fn restore_path(
            frontier: &mut Frontier,
            place: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<(), LoweringError> {
            let Some(root) = frontier.root_mut(place)? else {
                return Ok(());
            };
            if !path.is_empty() {
                root.paths.retain(|hole| {
                    hole.len() < path.len()
                        || !terminal_semantics::structural_paths_select_same_location(
                            path,
                            &hole[..path.len()],
                        )
                });
            }
            if path.is_empty() || root.paths.is_empty() {
                frontier.close(place);
            }
            Ok(())
        }

        fn residuals(
            root: &MovedRoot,
            selected: &StructuralAffineDiscard,
            types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<StructuralAffineDiscard>, LoweringError> {
            if root.paths.contains(&Vec::new()) {
                return Ok(Vec::new());
            }
            let removed = root
                .paths
                .iter()
                .filter_map(|path| {
                    path.strip_prefix(selected.path.as_slice())
                        .map(<[_]>::to_vec)
                })
                .collect();
            let paths = terminal_semantics::partial_affine_residuals(
                types,
                selected.structural_type,
                &removed,
                usize::MAX,
            )
            .ok_or(LoweringError::Unsupported(
                "projected cleanup has no bounded residual complement",
            ))?;
            Ok(paths
                .into_iter()
                .map(|(path, structural_type)| StructuralAffineDiscard {
                    place: root.place,
                    path: selected.path.iter().cloned().chain(path).collect(),
                    structural_type,
                })
                .collect())
        }

        fn complete_actions(
            frontier: &mut Frontier,
            cleanup_actions: &mut Vec<TerminalAffineCleanupAction>,
            roots: &[(PlaceId, StructuralTypeId)],
            types: &[StructuralTypeDeclaration],
        ) -> Result<(), LoweringError> {
            let mut actions = Vec::new();
            let mut placeholders = BTreeSet::new();
            for action in cleanup_actions.iter() {
                if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                    require_available(frontier, cleanup.place, &cleanup.path)?;
                }
                let first_new_action = actions.len();
                // Every cleanup action ends or reshapes the region of the root it
                // names, so each one first has to know that root's moved paths.
                let moved = frontier.root(action.place())?;
                match action {
                    TerminalAffineCleanupAction::DiscardRoot(place) if moved.is_some() => {
                        if !placeholders.insert(*place) {
                            return unsupported("duplicate scheduled whole cleanup");
                        }
                        let root = moved.ok_or(LoweringError::Unsupported(
                            "scheduled partial cleanup root is absent",
                        ))?;
                        let structural_type = declared_root_type(roots, *place)?;
                        let selected = StructuralAffineDiscard {
                            place: *place,
                            path: Vec::new(),
                            structural_type,
                        };
                        actions.extend(
                            residuals(root, &selected, types)?
                                .into_iter()
                                .map(TerminalAffineCleanupAction::DiscardResidual),
                        );
                        actions.push(TerminalAffineCleanupAction::RetireHome(*place));
                    }
                    TerminalAffineCleanupAction::DiscardResidual(selected)
                        if moved.is_some_and(|root| {
                            root.paths.iter().any(|path| {
                                path.len() > selected.path.len() && path.starts_with(&selected.path)
                            })
                        }) =>
                    {
                        let root = moved.ok_or(LoweringError::Unsupported(
                            "scheduled partial cleanup root is absent",
                        ))?;
                        actions.extend(
                            residuals(root, selected, types)?
                                .into_iter()
                                .map(TerminalAffineCleanupAction::DiscardResidual),
                        );
                    }
                    _ => actions.push(action.clone()),
                }
                for completed in &actions[first_new_action..] {
                    match completed {
                        TerminalAffineCleanupAction::RetireHome(place)
                        | TerminalAffineCleanupAction::DiscardRoot(place) => {
                            frontier.close(*place);
                        }
                        TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if !cleanup.retains_descendants =>
                        {
                            if cleanup.path.is_empty() {
                                frontier.close(cleanup.place);
                            } else {
                                remove_path(frontier, cleanup.place, &cleanup.path)?;
                            }
                        }
                        TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            remove_path(frontier, discard.place, &discard.path)?;
                        }
                        TerminalAffineCleanupAction::InvokeNominal(_) => {}
                    }
                }
                if let TerminalAffineCleanupAction::DiscardResidual(selected) = action
                    && let Some(root) = frontier.root_mut(selected.place)?
                {
                    root.paths.retain(|path| !path.starts_with(&selected.path));
                    root.paths.insert(selected.path.clone());
                }
            }
            *cleanup_actions = actions;
            Ok(())
        }
    }

    pub(crate) use projected_transfers::{
        copied_operand_roots, operands as structural_operands, owned_extractions,
        partitioned_roots, transfers_custody,
    };

    pub(crate) fn no_code_type_ids(
        checked: &checked_trees::CheckedTrees,
        type_ids: &[(
            typed_trees::type_identity::TypeIdentityId,
            semantic_vocabulary::StructuralTypeId,
        )],
    ) -> Vec<semantic_vocabulary::StructuralTypeId> {
        let no_code: std::collections::HashSet<typed_trees::type_identity::TypeIdentityId> =
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .filter(|declaration| declaration.no_code_affine_cleanup)
                .map(|declaration| declaration.identity)
                .collect();
        type_ids
            .iter()
            .filter_map(|(identity, id)| no_code.contains(identity).then_some(*id))
            .collect()
    }

    pub(crate) fn complete(
        completed_cleanup_frontiers: &[super::selected_continuations::CompletedCleanupFrontier],
        parameters: &[StructuralParameterDeclaration],
        entry_parameters: &[StructuralParameterDeclaration],
        no_code_types: &[semantic_vocabulary::StructuralTypeId],
        local_places: &[StructuralPlaceDeclaration],
        entry: BlockId,
        blocks: &mut [Block],
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LoweringError> {
        let extractions =
            projected_transfers::owned_extractions(entry_parameters, local_places, blocks, types)?;
        complete_carriers(
            parameters,
            entry_parameters,
            no_code_types,
            local_places,
            entry,
            blocks,
            types,
            &extractions,
        )?;
        projected_transfers::complete(
            completed_cleanup_frontiers,
            entry_parameters,
            local_places,
            entry,
            blocks,
            types,
            &extractions,
        )?;
        retire_borrowed_descriptor_arrivals(entry_parameters, entry, blocks, types)
    }

    fn retire_borrowed_descriptor_arrivals(
        entry_parameters: &[StructuralParameterDeclaration],
        entry: BlockId,
        blocks: &mut [Block],
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LoweringError> {
        let mut arrivals = Vec::new();
        for parameter in entry_parameters
            .iter()
            .filter(|parameter| borrowed_descriptor_arrival(parameter, types))
        {
            if !arrivals.iter().any(|(place, _)| *place == parameter.place) {
                arrivals.push((parameter.place, vec![entry]));
            }
        }
        for block in blocks.iter() {
            for parameter in block
                .structural_parameters
                .iter()
                .filter(|parameter| borrowed_descriptor_arrival(parameter, types))
            {
                if let Some((_, births)) = arrivals
                    .iter_mut()
                    .find(|(place, _)| *place == parameter.place)
                {
                    if !births.contains(&block.id) {
                        births.push(block.id);
                    }
                } else {
                    arrivals.push((parameter.place, vec![block.id]));
                }
            }
        }
        if !arrivals.is_empty() {
            let mut scheduled_deaths: std::collections::HashSet<PlaceId> =
                std::collections::HashSet::new();
            for block in blocks.iter() {
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        scheduled_deaths.insert(action.place());
                    }
                }
                for operation in &block.operations {
                    for argument in structural_operands(&operation.kind) {
                        if argument.path.is_empty() && argument.access == StructuralAccess::Owned {
                            scheduled_deaths.insert(argument.place);
                        }
                    }
                    if let terminal_psi::OperationKind::ReleaseReference { source, path } =
                        &operation.kind
                        && path.is_empty()
                    {
                        scheduled_deaths.insert(*source);
                    }
                }
                let record_owned =
                    |argument: &terminal_psi::StructuralArgument,
                     set: &mut std::collections::HashSet<PlaceId>| {
                        if argument.path.is_empty() && argument.access == StructuralAccess::Owned {
                            set.insert(argument.place);
                        }
                    };
                match &block.terminator {
                    Terminator::Jump {
                        structural_arguments,
                        ..
                    }
                    | Terminator::ExternalCompletion {
                        structural_arguments,
                        ..
                    } => {
                        for arg in structural_arguments {
                            record_owned(arg, &mut scheduled_deaths);
                        }
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for arg in when_true
                            .structural_arguments
                            .iter()
                            .chain(&when_false.structural_arguments)
                        {
                            record_owned(arg, &mut scheduled_deaths);
                        }
                    }
                    Terminator::StructuralCase { cases, .. } => {
                        for arg in cases.iter().flat_map(|case| &case.structural_arguments) {
                            record_owned(arg, &mut scheduled_deaths);
                        }
                    }
                    _ => {}
                }
            }
            let pending = arrivals
                .into_iter()
                .filter(|(place, _)| !scheduled_deaths.contains(place))
                .collect::<Vec<_>>();
            for (place, births) in pending {
                let mut reachable_set: std::collections::HashSet<BlockId> =
                    births.iter().copied().collect();
                let mut reachable = births;
                let mut cursor = 0;
                while cursor < reachable.len() {
                    let current = reachable[cursor];
                    cursor += 1;
                    let Some(block_index) =
                        blocks.binary_search_by_key(&current, |block| block.id).ok()
                    else {
                        continue;
                    };
                    for target in blocks[block_index].terminator.successor_targets() {
                        if reachable_set.insert(target) {
                            reachable.push(target);
                        }
                    }
                }
                for block in blocks.iter_mut() {
                    if !reachable_set.contains(&block.id) {
                        continue;
                    }
                    if matches!(
                        block.terminator,
                        Terminator::ReturnStructural { source, .. } if source == place
                    ) {
                        continue;
                    }
                    let Some(actions) = return_cleanup(&mut block.terminator) else {
                        continue;
                    };
                    if actions.iter().any(|action| action.place() == place) {
                        continue;
                    }
                    actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
                }
            }
        }
        Ok(())
    }

    fn borrowed_descriptor_arrival(
        parameter: &StructuralParameterDeclaration,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && parameter.access != StructuralAccess::Owned
            && terminal_semantics::view_descriptor_parameter(parameter, types.iter())
    }

    fn return_cleanup(
        terminator: &mut Terminator,
    ) -> Option<&mut Vec<TerminalAffineCleanupAction>> {
        match terminator {
            Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            }
            | Terminator::ReturnStructural {
                cleanup_actions, ..
            } => Some(cleanup_actions),
            _ => None,
        }
    }

    pub(crate) fn copyable_descriptor_type_ids(
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Vec<semantic_vocabulary::StructuralTypeId> {
        types
            .iter()
            .filter(|declaration| {
                declaration.descriptor_cleanup
                    && terminal_psi::structural_type_has_borrowed_storage_by(
                        |identity| types.iter().find(|candidate| candidate.id == identity),
                        declaration.id,
                    )
                    && terminal_semantics::discardable_store_value(
                        types.iter(),
                        declaration.id,
                        StructuralMultiplicity::Unrestricted,
                        false,
                    )
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. }
                            | terminal_psi::StructuralTypeShape::FixedArray { .. }
                            | terminal_psi::StructuralTypeShape::Sum { .. }
                            | terminal_psi::StructuralTypeShape::Mixed { .. }
                    )
            })
            .map(|declaration| declaration.id)
            .collect()
    }

    fn complete_carriers(
        parameters: &[StructuralParameterDeclaration],
        entry_parameters: &[StructuralParameterDeclaration],
        no_code_types: &[semantic_vocabulary::StructuralTypeId],
        local_places: &[StructuralPlaceDeclaration],
        entry: BlockId,
        blocks: &mut [Block],
        types: &[terminal_psi::StructuralTypeDeclaration],
        extractions: &[(
            semantic_vocabulary::OperationId,
            terminal_psi::StructuralArgument,
        )],
    ) -> Result<(), LoweringError> {
        let copyable_descriptor_types = copyable_descriptor_type_ids(types);
        let copyable_descriptor_set: std::collections::HashSet<
            semantic_vocabulary::StructuralTypeId,
        > = copyable_descriptor_types.iter().copied().collect();
        let no_code_type_set: std::collections::HashSet<semantic_vocabulary::StructuralTypeId> =
            no_code_types.iter().copied().collect();
        let entry_owners = parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.access == StructuralAccess::Owned
                    && (parameter.multiplicity == StructuralMultiplicity::Affine
                        || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && copyable_descriptor_set.contains(&parameter.structural_type))))
                .then_some(parameter.place)
            })
            .collect::<Vec<_>>();
        let scheduled = blocks
            .iter()
            .flat_map(|block| scheduled_discards(&block.terminator))
            .collect::<Vec<_>>();
        let scheduled_set: std::collections::HashSet<PlaceId> = scheduled.iter().copied().collect();
        let copied_roots = copied_operand_roots(entry_parameters, blocks);
        let partial_transfers_set = partitioned_roots(blocks, extractions, &copied_roots);
        let partial_transfers = partial_transfers_set.iter().copied().collect::<Vec<_>>();
        let mut unique_type_by_id: std::collections::HashMap<
            semantic_vocabulary::StructuralTypeId,
            Option<&terminal_psi::StructuralTypeDeclaration>,
        > = std::collections::HashMap::with_capacity(types.len());
        for declaration in types {
            match unique_type_by_id.entry(declaration.id) {
                std::collections::hash_map::Entry::Vacant(vacant) => {
                    vacant.insert(Some(declaration));
                }
                std::collections::hash_map::Entry::Occupied(mut occupied) => {
                    *occupied.get_mut() = None;
                }
            }
        }
        let trivial_affine_locals: std::collections::HashSet<PlaceId> = local_places
            .iter()
            .filter_map(|local| match local.kind {
                StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                } if no_code_type_set.contains(&structural_type) => Some(local.id),
                _ => None,
            })
            .collect();
        let mut births = blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                let place = established_place(operation)?;
                if partial_transfers_set.contains(&place) {
                    return None;
                }
                let Some(result) = operation.result.structural() else {
                    return trivial_affine_locals
                        .contains(&place)
                        .then_some((place, operation.id));
                };
                let copyable_home = result.multiplicity == StructuralMultiplicity::Unrestricted
                    && copyable_descriptor_set.contains(&result.structural_type);
                if (result.multiplicity != StructuralMultiplicity::Affine && !copyable_home)
                    || !result.claims.is_empty()
                {
                    return None;
                }
                let declaration = unique_type_by_id
                    .get(&result.structural_type)
                    .copied()
                    .flatten()?;
                let unscheduled_carrier = !scheduled_set.contains(&result.place)
                    && (matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                            | terminal_psi::StructuralTypeShape::Reference { .. }
                    ) || declaration.shape.is_borrowed_byte_view());
                if !(no_code_type_set.contains(&result.structural_type)
                    || unscheduled_carrier
                    || copyable_home)
                {
                    return None;
                }
                Some((result.place, operation.id))
            })
            .collect::<Vec<_>>();
        let mut managed = entry_owners.clone();
        managed.extend(births.iter().map(|(place, _)| *place));
        managed.extend(
            blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .filter(|parameter| {
                    parameter.access == StructuralAccess::Owned
                        && (parameter.multiplicity == StructuralMultiplicity::Affine
                            && no_code_type_set.contains(&parameter.structural_type)
                            || parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                && copyable_descriptor_set.contains(&parameter.structural_type))
                        && !entry_owners.contains(&parameter.place)
                        && !partial_transfers_set.contains(&parameter.place)
                })
                .map(|parameter| parameter.place),
        );
        carrier_arrivals::inherit(
            &mut managed,
            parameters,
            local_places,
            blocks,
            types,
            &partial_transfers,
            &scheduled,
        )?;
        let disposal_capability = managed.clone();
        let disposal_capability_set: std::collections::HashSet<PlaceId> =
            disposal_capability.iter().copied().collect();
        let mut tracked_disposals = if scheduled.is_empty() {
            Vec::new()
        } else {
            let owned_affine_types =
                carrier_arrivals::build_owned_affine_types(entry_parameters, local_places, blocks);
            let mut non_discard_root_places: std::collections::HashSet<PlaceId> =
                std::collections::HashSet::new();
            for block in blocks.iter() {
                for action in scheduled_actions(&block.terminator) {
                    if !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)) {
                        non_discard_root_places.insert(action.place());
                    }
                }
            }
            scheduled
                .iter()
                .copied()
                .filter(|place| {
                    !disposal_capability_set.contains(place)
                        && owned_affine_types.get(place).copied().flatten().is_some()
                        && !non_discard_root_places.contains(place)
                })
                .collect::<Vec<_>>()
        };
        tracked_disposals.sort_unstable();
        tracked_disposals.dedup();
        let tracked_disposals_set: std::collections::HashSet<PlaceId> =
            tracked_disposals.iter().copied().collect();
        let initial_owners = entry_owners
            .iter()
            .copied()
            .chain(
                entry_parameters
                    .iter()
                    .filter(|parameter| tracked_disposals_set.contains(&parameter.place))
                    .map(|parameter| parameter.place),
            )
            .collect::<Vec<_>>();
        births.extend(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| {
                    let place = established_place(operation)?;
                    tracked_disposals_set
                        .contains(&place)
                        .then_some((place, operation.id))
                }),
        );
        managed.extend(&tracked_disposals);
        if managed.is_empty() {
            return Ok(());
        }
        let mut managed_set: std::collections::HashSet<PlaceId> =
            std::collections::HashSet::with_capacity(managed.len());
        for &place in &managed {
            if !managed_set.insert(place) {
                return unsupported("scalar graph affine parameters have duplicate places");
            }
        }
        let entry_position = block_position(blocks, entry)?;
        let reentered =
            !entry_owners.is_empty() && !blocks[entry_position].structural_parameters.is_empty();
        let mut incoming = vec![Vec::new(); blocks.len()];
        let mut successors = vec![Vec::new(); blocks.len()];
        for (position, block) in blocks.iter().enumerate() {
            if (position != entry_position || !reentered)
                && block
                    .structural_parameters
                    .iter()
                    .any(|parameter| entry_owners.contains(&parameter.place))
            {
                return unsupported(
                    "scalar graph owned block parameters require exact forwarding custody",
                );
            }
            let targets = match &block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    cleanup_actions,
                    ..
                } => {
                    if (structural_arguments.iter().any(|argument| {
                        entry_owners.contains(&argument.place)
                            && projected_transfers::transfers_custody(argument, &copied_roots)
                    }) && !(reentered && *target == entry))
                        || cleanup_actions
                            .iter()
                            .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
                    {
                        return unsupported(
                            "scalar graph whole parameter cleanup cannot replace existing edge custody",
                        );
                    }
                    if reentered
                        && *target == entry
                        && structural_arguments.len() != parameters.len()
                    {
                        return unsupported(
                            "owned backedge lost its complete structural argument roster",
                        );
                    }
                    vec![*target]
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    if [when_true, when_false].iter().any(|edge| {
                        edge.structural_arguments.iter().any(|argument| {
                            entry_owners.contains(&argument.place)
                                && projected_transfers::transfers_custody(argument, &copied_roots)
                        }) || edge
                            .cleanup_actions
                            .iter()
                            .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
                            || (reentered && edge.target == entry)
                    }) {
                        return unsupported(
                            "scalar graph whole parameter cleanup cannot replace existing branch custody",
                        );
                    }
                    vec![when_true.target, when_false.target]
                }
                Terminator::StructuralCase { cases, .. } => {
                    if cases.iter().any(|edge| {
                        edge.cleanup_actions
                            .iter()
                            .any(|action| competing_cleanup(action, &entry_owners, &managed_set))
                            || (reentered && edge.target == entry)
                    }) {
                        return unsupported(
                            "scalar graph whole parameter cleanup cannot replace existing branch custody",
                        );
                    }
                    cases.iter().map(|edge| edge.target).collect()
                }
                terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
                | Terminator::ReturnUnit {
                    cleanup_actions, ..
                } if cleanup_actions
                    .iter()
                    .all(|action| !competing_cleanup(action, &entry_owners, &managed_set)) =>
                {
                    Vec::new()
                }
                Terminator::InvokeNoNormal { .. } => Vec::new(),
                Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => Vec::new(),
                _ => {
                    return unsupported(
                        "scalar graph whole parameter cleanup requires scalar or Unit exits and ordinary edges",
                    );
                }
            };
            for target in targets {
                let target_position = block_position(blocks, target)?;
                if !(reentered && target == entry) {
                    incoming[target_position].push(position);
                }
                successors[position].push(target_position);
            }
        }
        if !incoming[entry_position].is_empty() {
            return unsupported(
                "cyclic owned scalar graphs require exact structural forwarding custody",
            );
        }
        let custody_flow::Frontiers {
            entries,
            exits,
            outgoing,
        } = custody_flow::solve(
            blocks,
            entry_position,
            &initial_owners,
            &managed,
            &births,
            &copied_roots,
            &incoming,
            &successors,
        )?;
        let mut producers: std::collections::HashMap<
            semantic_vocabulary::PlaceId,
            semantic_vocabulary::OperationId,
        > = std::collections::HashMap::new();
        for operation in blocks.iter().flat_map(|block| &block.operations) {
            if let Some(place) = established_place(operation) {
                producers.entry(place).or_insert(operation.id);
            }
        }
        let dominators =
            crate::emission::control_flow::block_dominators(Some(entry_position), &successors);
        let mut parameter_blocks = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| !block.structural_parameters.is_empty())
            .collect::<Vec<_>>();
        parameter_blocks.sort_by_key(|(position, _)| dominators[*position].len());
        let mut parameter_order: std::collections::HashMap<semantic_vocabulary::PlaceId, usize> =
            std::collections::HashMap::new();
        for (index, place) in parameters
            .iter()
            .map(|parameter| parameter.place)
            .chain(
                parameter_blocks
                    .iter()
                    .flat_map(|(_, block)| &block.structural_parameters)
                    .map(|parameter| parameter.place),
            )
            .enumerate()
        {
            parameter_order.insert(place, index);
        }
        let complete_disposals =
            |actions: &mut Vec<TerminalAffineCleanupAction>,
             additions: Vec<semantic_vocabulary::PlaceId>,
             live: &[semantic_vocabulary::PlaceId]| {
                actions.retain(|action| {
                    !matches!(action,
                        TerminalAffineCleanupAction::DiscardRoot(place)
                            if tracked_disposals_set.contains(place) && !live.contains(place)
                    )
                });
                replace_managed_discards(
                    actions,
                    additions
                        .into_iter()
                        .filter(|place| disposal_capability_set.contains(place))
                        .collect(),
                    &disposal_capability_set,
                    &producers,
                    &parameter_order,
                    &copied_roots,
                );
            };
        for (position, block) in blocks.iter_mut().enumerate() {
            let discards = |edge_position: usize, target_position: usize| {
                outgoing[position][edge_position]
                    .iter()
                    .copied()
                    .filter(|place| !entries[target_position].contains(place))
                    .collect::<Vec<_>>()
            };
            match &mut block.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    cleanup_actions,
                    ..
                } => {
                    if reentered && *target == entry {
                        let mut remaining = exits[position].clone();
                        for (parameter, argument) in parameters.iter().zip(structural_arguments) {
                            if argument.access != parameter.access || !argument.path.is_empty() {
                                return unsupported(
                                    "owned backedge changes access or projects a whole owner",
                                );
                            }
                            if parameter.multiplicity == StructuralMultiplicity::Affine {
                                let Some(position) =
                                    remaining.iter().position(|place| *place == argument.place)
                                else {
                                    return unsupported(
                                        "owned backedge transfers a missing or already consumed owner",
                                    );
                                };
                                remaining.remove(position);
                            }
                        }
                        complete_disposals(cleanup_actions, remaining, &outgoing[position][0]);
                    } else {
                        complete_disposals(
                            cleanup_actions,
                            discards(0, successors[position][0]),
                            &outgoing[position][0],
                        );
                    }
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    complete_disposals(
                        &mut when_true.cleanup_actions,
                        discards(0, successors[position][0]),
                        &outgoing[position][0],
                    );
                    complete_disposals(
                        &mut when_false.cleanup_actions,
                        discards(1, successors[position][1]),
                        &outgoing[position][1],
                    );
                }
                Terminator::StructuralCase { cases, .. } => {
                    for (edge_position, (edge, successor)) in
                        cases.iter_mut().zip(&successors[position]).enumerate()
                    {
                        complete_disposals(
                            &mut edge.cleanup_actions,
                            discards(edge_position, *successor),
                            &outgoing[position][edge_position],
                        );
                    }
                }
                terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
                | Terminator::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    complete_disposals(cleanup_actions, exits[position].clone(), &exits[position]);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn replace_managed_discards(
        existing: &mut Vec<TerminalAffineCleanupAction>,
        additions: Vec<semantic_vocabulary::PlaceId>,
        managed: &std::collections::HashSet<semantic_vocabulary::PlaceId>,
        producers: &std::collections::HashMap<
            semantic_vocabulary::PlaceId,
            semantic_vocabulary::OperationId,
        >,
        parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
        copied_roots: &std::collections::BTreeSet<semantic_vocabulary::PlaceId>,
    ) {
        let mut additions = additions
            .into_iter()
            .filter(|place| {
                !existing.iter().any(|action| {
                    matches!(action, TerminalAffineCleanupAction::RetireHome(retired) if retired == place)
                })
            })
            .collect::<Vec<_>>();
        existing.retain(|action| {
            !matches!(action, TerminalAffineCleanupAction::DiscardRoot(place)
                if managed.contains(place) && !additions.contains(place))
        });
        additions.retain(|place| {
            !existing.iter().any(|action| {
                matches!(action, TerminalAffineCleanupAction::DiscardRoot(scheduled_place) if scheduled_place == place)
            })
        });
        merge_discards(
            existing,
            additions,
            producers,
            parameter_order,
            copied_roots,
        );
    }

    fn competing_cleanup(
        action: &TerminalAffineCleanupAction,
        affine: &[semantic_vocabulary::PlaceId],
        managed: &std::collections::HashSet<semantic_vocabulary::PlaceId>,
    ) -> bool {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => affine.contains(place),
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => managed.contains(&cleanup.place),
            TerminalAffineCleanupAction::DiscardResidual(_)
            | TerminalAffineCleanupAction::RetireHome(_) => false,
        }
    }

    fn established_place(
        operation: &terminal_psi::Operation,
    ) -> Option<semantic_vocabulary::PlaceId> {
        if let Some(result) = operation.result.structural() {
            return Some(result.place);
        }
        match &operation.kind {
            terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } => {
                Some(*destination)
            }
            _ => None,
        }
    }

    fn block_position(blocks: &[Block], target: BlockId) -> Result<usize, LoweringError> {
        if let Ok(index) = blocks.binary_search_by_key(&target, |block| block.id) {
            let duplicate_before = index > 0 && blocks[index - 1].id == target;
            let duplicate_after = index + 1 < blocks.len() && blocks[index + 1].id == target;
            if !duplicate_before && !duplicate_after {
                return Ok(index);
            }
        }
        let mut positions = blocks
            .iter()
            .enumerate()
            .filter_map(|(position, block)| (block.id == target).then_some(position));
        let position = positions.next().ok_or(LoweringError::Unsupported(
            "owned scalar graph has an absent block",
        ))?;
        if positions.next().is_some() {
            return unsupported("owned scalar graph has duplicate block identities");
        }
        Ok(position)
    }

    fn merge_discards(
        existing: &mut Vec<TerminalAffineCleanupAction>,
        additions: Vec<semantic_vocabulary::PlaceId>,
        producers: &std::collections::HashMap<
            semantic_vocabulary::PlaceId,
            semantic_vocabulary::OperationId,
        >,
        parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
        copied_roots: &std::collections::BTreeSet<semantic_vocabulary::PlaceId>,
    ) {
        for place in additions {
            let insertion = disposal_position(
                existing.iter().map(TerminalAffineCleanupAction::place),
                place,
                producers,
                parameter_order,
            );
            let action = if copied_roots.contains(&place) {
                TerminalAffineCleanupAction::RetireHome(place)
            } else {
                TerminalAffineCleanupAction::DiscardRoot(place)
            };
            existing.insert(insertion, action);
        }
    }

    fn disposal_position(
        existing: impl Iterator<Item = semantic_vocabulary::PlaceId>,
        place: semantic_vocabulary::PlaceId,
        producers: &std::collections::HashMap<
            semantic_vocabulary::PlaceId,
            semantic_vocabulary::OperationId,
        >,
        parameter_order: &std::collections::HashMap<semantic_vocabulary::PlaceId, usize>,
    ) -> usize {
        let birth = producers.get(&place).copied();
        let parameter_position = parameter_order.get(&place).copied();
        let mut length = 0;
        for (position, current) in existing.enumerate() {
            if birth.is_some_and(|birth| {
                producers
                    .get(&current)
                    .is_none_or(|&current_birth| current_birth < birth)
            }) || (birth.is_none()
                && parameter_position.is_some_and(|parameter_position| {
                    parameter_order
                        .get(&current)
                        .is_some_and(|&current_position| current_position < parameter_position)
                }))
            {
                return position;
            }
            length = position + 1;
        }
        length
    }

    fn scheduled_discards(terminator: &Terminator) -> Vec<semantic_vocabulary::PlaceId> {
        scheduled_actions(terminator)
            .into_iter()
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect()
    }

    fn scheduled_actions(terminator: &Terminator) -> Vec<&TerminalAffineCleanupAction> {
        match terminator {
            Terminator::Jump {
                cleanup_actions, ..
            } => cleanup_actions.iter().collect(),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .cleanup_actions
                .iter()
                .chain(&when_false.cleanup_actions)
                .collect(),
            terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => cleanup_actions.iter().collect(),
            Terminator::StructuralCase { cases, .. } => cases
                .iter()
                .flat_map(|edge| &edge.cleanup_actions)
                .collect(),
            _ => Vec::new(),
        }
    }
}

pub(crate) mod borrowed_window {

    use super::{
        CheckedUnitStructuralPathSegment, LoweringError, OperationKind, OperationResult, PlaceId,
        StructuralAccess, StructuralFieldId, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeId, allocate_dense, place_id,
    };
    use crate::emission::claim_bindings::emit_establishment_claims;
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::emission::runtime_elements::{self, ProjectionStep};
    use crate::emission::structural_scalar_store::{WindowLeaf, lower_window_path};
    use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
    use crate::shared::lowering_error::unsupported;
    use crate::shared::terminal_identities::lookup_type_id;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
    };
    use language_semantics::Multiplicity;
    use terminal_psi::{StructuralArgument, StructuralOperationResult};

    fn captured_window_storage(
        mut source: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<terminal_psi::TerminalIdentityIndex>,
        operations: &OperationBuffer,
    ) -> (
        PlaceId,
        Vec<StructuralPathSegment>,
        Option<terminal_psi::TerminalIdentityIndex>,
    ) {
        let mut path = path.to_vec();
        path.extend(field.map(StructuralPathSegment::Field));
        let mut visited = Vec::new();
        while path.first() == Some(&StructuralPathSegment::Referent) && !visited.contains(&source) {
            visited.push(source);
            let mut captures = operations
                .loan_captures
                .iter()
                .filter(|capture| capture.carrier == source);
            let Some(capture) = captures.next() else {
                break;
            };
            if captures.any(|other| other.source != capture.source) {
                break;
            }
            source = capture.source.place;
            let mut captured = capture.source.path.clone();
            captured.extend_from_slice(&path[1..]);
            path = captured;
        }
        let field = match path.last() {
            Some(StructuralPathSegment::Field(field)) => Some(*field),
            _ => None,
        };
        if field.is_some() {
            path.pop();
        }
        (source, path, field)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct OpenBorrowedWindow {
        opening: PlaceId,
        pub(crate) source: PlaceId,
        pub(crate) path: Vec<StructuralPathSegment>,
        field_path_identity: Option<terminal_psi::TerminalIdentityIndex>,
        pub(crate) hole_type: StructuralTypeId,
        spelling: String,
        restoration_required: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct BorrowedWindowRepairValue {
        pub(crate) place: PlaceId,
        pub(crate) structural_type: StructuralTypeId,
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub(crate) struct BorrowedWindowLedger {
        open: Vec<OpenBorrowedWindow>,
        assignment: Option<CapturedAssignmentDestination>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CapturedAssignmentDestination {
        statement_index: u32,
        destination_expression: typed_trees::expression::ExpressionHandle,
        destination: CheckedUnitStructuralArgumentPlan,
        normalized_path: Vec<CheckedUnitStructuralPathSegment>,
        source: PlaceId,
        root_type: StructuralTypeId,
        path: Vec<StructuralPathSegment>,
        original_source: PlaceId,
        original_path: Vec<StructuralPathSegment>,
    }

    struct EvaluatedStoragePlace {
        source: PlaceId,
        root_type: StructuralTypeId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        original_source: PlaceId,
        original_path: Vec<StructuralPathSegment>,
    }

    pub(crate) struct CapturedAssignmentPlace {
        pub(crate) source: PlaceId,
        pub(crate) root_type: StructuralTypeId,
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) original_source: PlaceId,
        pub(crate) original_path: Vec<StructuralPathSegment>,
    }

    pub(crate) struct CapturedIndexedDestination {
        pub(crate) source: PlaceId,
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) coordinate: semantic_vocabulary::ValueId,
    }

    struct ResolvedStorageAddress {
        source: PlaceId,
        root_type: StructuralTypeId,
        root_access: StructuralAccess,
        root_multiplicity: StructuralMultiplicity,
        steps: Vec<ProjectionStep>,
        fixed_prefix: usize,
        checked_path: Vec<CheckedUnitStructuralPathSegment>,
        field: Option<terminal_psi::StructuralFieldDeclaration>,
        subtree_type: Option<StructuralTypeId>,
        spelling: String,
        restoration_required: bool,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        additional_input_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    }

    impl ResolvedStorageAddress {
        fn selected_memberships(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            carrier_path: &[StructuralPathSegment],
        ) -> Result<
            (
                Vec<semantic_vocabulary::StructuralDomainId>,
                Vec<terminal_psi::StructuralPathQualification>,
            ),
            LoweringError,
        > {
            let mut path = carrier_path.to_vec();
            if self.field.is_some() {
                let Some(CheckedUnitStructuralPathSegment::Field(identity)) =
                    self.checked_path.last()
                else {
                    return Err(unpinned(
                        &self.spelling,
                        "the qualified hole lost its field identity",
                    ));
                };
                path.push(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                ));
            }
            terminal_psi::StructuralPathQualification::select_restorable(
                &self.qualifications,
                &self.projected_qualifications,
                &path,
            )
            .ok_or_else(|| {
                unpinned(
                    &self.spelling,
                    "field replacement cannot restore an ancestor's qualification",
                )
            })
        }
    }

    impl BorrowedWindowLedger {
        pub(crate) fn capture_assignment_destination(
            environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            destination: &CheckedUnitStructuralArgumentPlan,
            destination_expression: typed_trees::expression::ExpressionHandle,
            evaluation: &mut WindowEvaluation<'_, '_>,
            values: &mut Vec<terminal_psi::ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(), LoweringError> {
            let structural_types = environment.structural_types;
            let place = resolve_storage_address(
                identities,
                destination,
                StructuralAccess::WriteOnlyBorrow,
                &environment.parameters,
                evaluation.evaluation,
                operations,
                structural_types,
                environment.type_ids,
            )?;
            let indexes = runtime_elements::evaluate_selectors(
                identities,
                &place.steps,
                evaluation.checked,
                evaluation.machine,
                evaluation.state,
                evaluation.statement_index,
                evaluation.evaluation,
                evaluation.source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                evaluation.calls,
                environment,
            )?;
            let mut path = runtime_elements::complete_path(
                place.steps,
                indexes,
                evaluation.calls,
                operations,
            )?;
            if place.field.is_some() {
                let Some(CheckedUnitStructuralPathSegment::Field(identity)) =
                    destination.path.last()
                else {
                    return unsupported("captured field destination lost its declaration identity");
                };
                path.push(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                ));
            }
            let access = if terminal_semantics::reference_source_projection_accessible(
                structural_types.iter(),
                place.root_type,
                place.root_multiplicity,
                place.root_access,
                &path,
                StructuralAccess::MutableBorrow,
            ) {
                StructuralAccess::MutableBorrow
            } else {
                StructuralAccess::WriteOnlyBorrow
            };
            let capture_boundary = path
                .iter()
                .enumerate()
                .filter(|(position, _)| *position > place.fixed_prefix)
                .filter_map(|(position, segment)| {
                    if *segment == StructuralPathSegment::Referent {
                        return Some(position + 1);
                    }
                    (matches!(
                        segment,
                        StructuralPathSegment::FixedIndex(_)
                            | StructuralPathSegment::RuntimeIndex { .. }
                    ) && terminal_semantics::runtime_structural_path_tip(
                        structural_types.iter(),
                        place.root_type,
                        &path[..position],
                    )
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                                | terminal_psi::StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                )
                        )
                    }))
                    .then_some(position)
                })
                .next_back();
            let original_path = path.clone();
            let (source, root_type, path) = if let Some(capture_boundary) = capture_boundary {
                let retained_path = path.split_off(capture_boundary);
                let capture_type = terminal_semantics::runtime_structural_path_tip(
                    structural_types.iter(),
                    place.root_type,
                    &path,
                )
                .ok_or(LoweringError::Unsupported(
                    "captured destination lost its backing declaration",
                ))?
                .id;
                let (qualifications, projected_qualifications) =
                    terminal_psi::StructuralPathQualification::select(
                        &place.qualifications,
                        &place.projected_qualifications,
                        &path,
                    );
                let additional = terminal_psi::StructuralPathQualification::select_rows(
                    &place.additional_input_qualifications,
                    &path,
                );
                let capture = if let Some(shape) = super::completed_place::descriptor_shape(
                    structural_types,
                    place.root_type,
                    &path,
                ) {
                    super::completed_place::Capture::DescriptorValue {
                        shape,
                        preferred: None,
                    }
                } else {
                    super::completed_place::Capture::Address(capture_type)
                };
                let (capture_type, qualifications, projected_qualifications, additional) =
                    super::completed_place::present_capture(
                        &capture,
                        access,
                        structural_types,
                        environment.structural_domains(),
                        qualifications,
                        projected_qualifications,
                        additional,
                    )?;
                let source = super::completed_address::capture(
                    place.source,
                    path,
                    capture_type,
                    access,
                    qualifications,
                    projected_qualifications,
                    additional,
                    evaluation.evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    environment.next_place,
                    environment.private_places,
                    operations,
                )?;
                (source, capture_type, retained_path)
            } else {
                (place.source, place.root_type, path)
            };
            environment.windows.assignment = Some(CapturedAssignmentDestination {
                statement_index: evaluation.statement_index,
                destination_expression,
                destination: destination.clone(),
                normalized_path: place.checked_path,
                source,
                root_type,
                path,
                original_source: place.source,
                original_path,
            });
            Ok(())
        }

        fn assignment_destination(
            &self,
            statement_index: u32,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> Option<&CapturedAssignmentDestination> {
            self.assignment.as_ref().filter(|capture| {
                capture.statement_index == statement_index
                    && &capture.destination.source == source
                    && path.starts_with(&capture.destination.path)
            })
        }

        pub(crate) fn captured_displacement_expression(
            &self,
            statement_index: u32,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> typed_trees::expression::ExpressionHandle {
            self.assignment_destination(statement_index, source, path)
                .map_or_else(
                    typed_trees::expression::ExpressionHandle::invalid,
                    |capture| capture.destination_expression,
                )
        }

        pub(crate) fn captured_assignment_place(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            statement_index: u32,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> Result<Option<CapturedAssignmentPlace>, LoweringError> {
            let Some(capture) = self.assignment_destination(statement_index, source, path) else {
                return Ok(None);
            };
            let suffix =
                lower_structural_path(identities, &path[capture.destination.path.len()..])?;
            let mut retained = capture.path.clone();
            retained.extend_from_slice(&suffix);
            let mut original_path = capture.original_path.clone();
            original_path.extend(suffix);
            Ok(Some(CapturedAssignmentPlace {
                source: capture.source,
                root_type: capture.root_type,
                path: retained,
                original_source: capture.original_source,
                original_path,
            }))
        }

        pub(crate) fn captured_store_place(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            statement_index: u32,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> Result<Option<CapturedAssignmentPlace>, LoweringError> {
            let Some(capture) = self.assignment.as_ref().filter(|capture| {
                capture.statement_index == statement_index
                    && &capture.destination.source == source
                    && path.starts_with(&capture.normalized_path)
            }) else {
                return Ok(None);
            };
            let suffix = lower_structural_path(identities, &path[capture.normalized_path.len()..])?;
            let mut retained = capture.path.clone();
            retained.extend_from_slice(&suffix);
            let mut original_path = capture.original_path.clone();
            original_path.extend(suffix);
            Ok(Some(CapturedAssignmentPlace {
                source: capture.source,
                root_type: capture.root_type,
                path: retained,
                original_source: capture.original_source,
                original_path,
            }))
        }

        pub(crate) fn captured_indexed_destination(
            &self,
            statement_index: u32,
            source: &CheckedUnitStructuralArgumentSourcePlan,
            descriptor_path: &[CheckedUnitStructuralPathSegment],
        ) -> Result<Option<CapturedIndexedDestination>, LoweringError> {
            let Some(capture) = &self.assignment else {
                return Ok(None);
            };
            if capture.statement_index != statement_index || &capture.destination.source != source {
                return Ok(None);
            }
            let Some(suffix) = capture.normalized_path.strip_prefix(descriptor_path) else {
                return Ok(None);
            };
            if !matches!(
                suffix.first(),
                Some(
                    CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                )
            ) {
                return Ok(None);
            }
            let prefix_length =
                capture
                    .path
                    .len()
                    .checked_sub(suffix.len())
                    .ok_or(LoweringError::Unsupported(
                        "indexed store descriptor precedes its captured backing",
                    ))?;
            let Some(StructuralPathSegment::RuntimeIndex { index, .. }) =
                capture.path.get(prefix_length)
            else {
                return unsupported("indexed store lost its captured runtime coordinate");
            };
            Ok(Some(CapturedIndexedDestination {
                source: capture.source,
                path: capture.path[..prefix_length].to_vec(),
                coordinate: *index,
            }))
        }

        fn evaluated_place(
            capture: Option<&CapturedAssignmentDestination>,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            plan: &CheckedUnitStructuralArgumentPlan,
            place: &ResolvedStorageAddress,
            evaluation: &mut WindowEvaluation<'_, '_>,
            values: &mut Vec<terminal_psi::ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
            environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
        ) -> Result<EvaluatedStoragePlace, LoweringError> {
            if let Some(capture) = capture {
                let descendants = lower_structural_path(
                    identities,
                    &plan.path[capture.destination.path.len()..],
                )?;
                let mut path = capture.path.clone();
                path.extend_from_slice(&descendants);
                let mut original_path = capture.original_path.clone();
                original_path.extend(descendants);
                let field = place.field.as_ref().and_then(|field| {
                    matches!(path.last(), Some(StructuralPathSegment::Field(_))).then_some(field.id)
                });
                if field.is_some() {
                    path.pop();
                }
                if place.field.is_some() {
                    original_path.pop();
                }
                return Ok(EvaluatedStoragePlace {
                    source: capture.source,
                    root_type: capture.root_type,
                    path,
                    field,
                    original_source: capture.original_source,
                    original_path,
                });
            }
            let indexes = runtime_elements::evaluate_selectors(
                identities,
                &place.steps,
                evaluation.checked,
                evaluation.machine,
                evaluation.state,
                evaluation.statement_index,
                evaluation.evaluation,
                evaluation.source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                evaluation.calls,
                environment,
            )?;
            let path = runtime_elements::complete_path(
                place.steps.clone(),
                indexes,
                evaluation.calls,
                operations,
            )?;
            Ok(EvaluatedStoragePlace {
                source: place.source,
                root_type: place.root_type,
                original_path: path.clone(),
                path,
                field: place.field.as_ref().map(|field| field.id),
                original_source: place.source,
            })
        }

        pub(crate) fn emit_move(
            environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            moved: &CheckedUnitStructuralArgumentPlan,
            expression: typed_trees::expression::ExpressionHandle,
            result: &CheckedUnitStructuralResultBindingPlan,
            prepared_claims: &[crate::emission::claim_bindings::PreparedClaim],
            evaluation: &mut WindowEvaluation<'_, '_>,
            values: &mut Vec<terminal_psi::ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<PlaceId, LoweringError> {
            let structural_types = environment.structural_types;
            let type_ids = environment.type_ids;
            let (place, hole_type) = resolve_window_place(
                identities,
                moved,
                StructuralAccess::MutableBorrow,
                &environment.parameters,
                evaluation.evaluation,
                operations,
                structural_types,
                type_ids,
            )?;
            if lookup_type_id(type_ids, &result.type_identity)? != hole_type {
                return Err(unpinned(
                    &place.spelling,
                    "the moved binding's type differs from the declared field type",
                ));
            }
            if place.steps.is_empty() && place.field.is_none() && !place.restoration_required {
                return Err(unpinned(
                    &place.spelling,
                    "an owned whole local transfers without a restoration window",
                ));
            }
            let capture = environment
                .windows
                .assignment_destination(evaluation.statement_index, &moved.source, &moved.path)
                .filter(|capture| {
                    expression.is_valid() && capture.destination_expression == expression
                })
                .cloned();
            let evaluated = Self::evaluated_place(
                capture.as_ref(),
                identities,
                moved,
                &place,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                environment,
            )?;
            let source = evaluated.source;
            let path = evaluated.path;
            let field = evaluated.field;
            let (storage_source, storage_path, storage_field) = captured_window_storage(
                evaluated.original_source,
                &evaluated.original_path,
                place.field.as_ref().map(|field| field.identity),
                operations,
            );
            let (qualifications, projected_qualifications) =
                place.selected_memberships(identities, &evaluated.original_path)?;
            if let Some(absent) = environment.windows.open.iter().find(|absent| {
                absent.source == storage_source
                    && paths_overlap(
                        &absent.path,
                        absent.field_path_identity,
                        &storage_path,
                        storage_field,
                        &operations.scalar_index_coordinates,
                    )
            }) {
                return Err(unpinned(
                    &place.spelling,
                    if absent.path == storage_path && absent.field_path_identity == storage_field {
                        "the place is already absent: repeated extraction of one open window"
                    } else {
                        "the place overlaps an open window: extraction through absent storage"
                    },
                ));
            }
            runtime_elements::observe_element_extent(
                source,
                evaluated.root_type,
                &path,
                structural_types,
                next_value,
                operations,
            )?;
            let selected_path = terminal_semantics::structural_window_projection(
                structural_types.iter(),
                evaluated.root_type,
                &path,
                field,
            )
            .ok_or(LoweringError::Unsupported(
                "extraction has no exact current storage projection",
            ))?
            .path;
            let result_claims = if place.restoration_required {
                emit_establishment_claims(prepared_claims, &mut evaluation.calls.claims)?
            } else {
                let argument = StructuralArgument {
                    place: source,
                    path: selected_path.clone(),
                    access: StructuralAccess::Owned,
                };
                let completed = evaluation.evaluation.current_claims.source(&argument)?;
                crate::emission::claim_bindings::lower_owned_operand_claims(
                    identities,
                    evaluation.checked,
                    evaluation.machine,
                    evaluation.state,
                    evaluation.statement_index,
                    expression,
                    None,
                    None,
                    evaluation.calls.claims.bindings(),
                    completed.as_deref(),
                    hole_type,
                    structural_types,
                )?
            };
            if !place.restoration_required {
                evaluation
                    .evaluation
                    .current_claims
                    .replace(source, &selected_path, &[])?;
            }
            let moved_place = place_id(allocate_dense(environment.next_place)?);
            operations.invalidate_field_lengths_under(
                evaluated.original_source,
                &evaluated.original_path,
            );
            operations.invalidate_field_lengths_under(source, &path);
            operations.push_leaf(
                OperationResult::Structural(StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place: moved_place,
                    structural_type: hole_type,
                    multiplicity: terminal_multiplicity(result.multiplicity),
                    qualifications,
                    projected_qualifications,
                    additional_input_qualifications: Vec::new(),
                    claims: result_claims,
                }),
                OperationKind::MoveStructuralField {
                    source,
                    path: path.clone(),
                    field,
                },
            );
            let produced_claims = operations
                .operations
                .last()
                .and_then(|operation| operation.result.structural())
                .ok_or(LoweringError::Unsupported(
                    "extraction lost its completed producer",
                ))?
                .claims
                .clone();
            evaluation
                .evaluation
                .current_claims
                .publish(moved_place, produced_claims)?;
            if place.restoration_required {
                crate::emission::claim_bindings::record_prepared_introductions(
                    operations,
                    moved_place,
                    prepared_claims,
                    &evaluation.calls.claims,
                    crate::emission::claim_bindings::ClaimEstablishmentKind::DetachedValue,
                )?;
            }
            environment.windows.open.push(OpenBorrowedWindow {
                opening: moved_place,
                source: storage_source,
                path: storage_path,
                field_path_identity: storage_field,
                hole_type,
                spelling: place.spelling,
                restoration_required: place.restoration_required,
            });
            Ok(moved_place)
        }

        pub(crate) fn emit_store(
            environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            destination: &CheckedUnitStructuralArgumentPlan,
            opening: Option<PlaceId>,
            value: BorrowedWindowRepairValue,
            evaluation: &mut WindowEvaluation<'_, '_>,
            values: &mut Vec<terminal_psi::ValueDeclaration>,
            next_value: &mut u64,
            next_block: &mut u64,
            next_edge: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<(), LoweringError> {
            let structural_types = environment.structural_types;
            let type_ids = environment.type_ids;
            let (place, hole_type) = resolve_window_place(
                identities,
                destination,
                StructuralAccess::WriteOnlyBorrow,
                &environment.parameters,
                evaluation.evaluation,
                operations,
                structural_types,
                type_ids,
            )?;
            let capture = environment
                .windows
                .assignment_destination(
                    evaluation.statement_index,
                    &destination.source,
                    &destination.path,
                )
                .cloned();
            let evaluated = Self::evaluated_place(
                capture.as_ref(),
                identities,
                destination,
                &place,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                environment,
            )?;
            let source = evaluated.source;
            let path = evaluated.path;
            let field = evaluated.field;
            let (storage_source, storage_path, storage_field) = captured_window_storage(
                evaluated.original_source,
                &evaluated.original_path,
                place.field.as_ref().map(|field| field.identity),
                operations,
            );
            let index = opening.and_then(|opening| {
                environment
                    .windows
                    .open
                    .iter()
                    .position(|absent| absent.opening == opening)
            });
            if opening.is_some() && index.is_none() {
                return Err(unpinned(
                    &place.spelling,
                    "the restoration names no open extraction",
                ));
            }
            if let Some(index) = index {
                let absent = &environment.windows.open[index];
                if absent.source != storage_source
                    || absent.field_path_identity != storage_field
                    || absent.hole_type != hole_type
                {
                    return Err(unpinned(
                        &place.spelling,
                        "the restoration names a different storage or type than the extraction opened",
                    ));
                }
            }
            let multiplicity = environment
                .parameters
                .iter()
                .find(|parameter| parameter.place == value.place)
                .map(|parameter| parameter.multiplicity)
                .or_else(|| {
                    operations.operations.iter().find_map(|operation| {
                        operation
                            .result
                            .structural()
                            .filter(|result| result.place == value.place)
                            .map(|result| result.multiplicity)
                    })
                })
                .or_else(|| {
                    operations
                        .structural_values
                        .iter()
                        .find_map(|(_, binding)| {
                            binding
                                .operation_result()
                                .ok()
                                .filter(|result| result.place == value.place)
                                .map(|result| result.multiplicity)
                        })
                });
            let discardable = multiplicity == Some(StructuralMultiplicity::Unrestricted)
                || multiplicity == Some(StructuralMultiplicity::Affine)
                    && structural_types.iter().any(|declaration| {
                        declaration.id == value.structural_type
                            && (declaration.no_code_affine_cleanup
                                || declaration.descriptor_cleanup)
                    });
            if index.is_none()
                && ((place.restoration_required && !discardable)
                    || environment.windows.open.iter().any(|absent| {
                        absent.source == storage_source
                            && paths_overlap(
                                &absent.path,
                                absent.field_path_identity,
                                &storage_path,
                                storage_field,
                                &operations.scalar_index_coordinates,
                            )
                    }))
            {
                let reason = if environment.windows.open.iter().any(|absent| {
                    absent.source == storage_source
                        && paths_overlap(
                            &absent.path,
                            absent.field_path_identity,
                            &storage_path,
                            storage_field,
                            &operations.scalar_index_coordinates,
                        )
                }) {
                    "the store overlaps an open window without naming its exact place"
                } else {
                    "the store names a place with no open window: a replacement of established \
                     storage is not a restoration"
                };
                return Err(unpinned(&place.spelling, reason));
            }
            if value.structural_type != hole_type {
                return Err(unpinned(
                    &place.spelling,
                    "the repair value's type differs from the type moved out of the place",
                ));
            }
            let _ = place.selected_memberships(identities, &evaluated.original_path)?;
            let _ = environment
                .parameters
                .iter()
                .filter(|parameter| parameter.place == value.place)
                .map(|parameter| {
                    (
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                    )
                })
                .chain(operations.operations.iter().filter_map(|operation| {
                    operation
                        .result
                        .structural()
                        .filter(|result| result.place == value.place)
                        .map(|result| (&result.qualifications, &result.projected_qualifications))
                }))
                .chain(
                    operations
                        .structural_values
                        .iter()
                        .filter_map(|(_, binding)| {
                            binding
                                .operation_result()
                                .ok()
                                .filter(|result| result.place == value.place)
                                .map(|result| {
                                    (&result.qualifications, &result.projected_qualifications)
                                })
                        }),
                )
                .next()
                .ok_or_else(|| {
                    unpinned(
                        &place.spelling,
                        "the repair value has no declared membership source",
                    )
                })?;
            runtime_elements::observe_element_extent(
                source,
                evaluated.root_type,
                &path,
                structural_types,
                next_value,
                operations,
            )?;
            operations.invalidate_field_lengths_under(
                evaluated.original_source,
                &evaluated.original_path,
            );
            operations.invalidate_field_lengths_under(source, &path);
            if !place.restoration_required {
                let selected_path = terminal_semantics::structural_window_projection(
                    structural_types.iter(),
                    evaluated.root_type,
                    &path,
                    field,
                )
                .ok_or(LoweringError::Unsupported(
                    "store has no exact current storage projection",
                ))?
                .path;
                let argument = StructuralArgument {
                    place: value.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                };
                let claims = evaluation
                    .evaluation
                    .current_claims
                    .source(&argument)?
                    .ok_or_else(|| {
                        unpinned(
                            &format!(
                                "{} at statement {}: stored operand {:?} of type {:?}, destination {:?}/{:?}, last operation {:?}",
                                place.spelling,
                                evaluation.statement_index,
                                argument,
                                value.structural_type,
                                source,
                                selected_path,
                                operations
                                    .operations
                                    .last()
                                    .map(|operation| (operation.id, &operation.kind, &operation.result)),
                            ),
                            "stored operand has no completed custody namespace",
                        )
                    })?;
                evaluation
                    .evaluation
                    .current_claims
                    .replace(source, &selected_path, &claims)?;
            }
            let operation = operations.push_leaf(
                OperationResult::Unit,
                OperationKind::StoreStructuralField {
                    disposition: if index.is_none() && place.restoration_required {
                        terminal_psi::StructuralStoreDisposition::ReplaceDiscardable
                    } else {
                        terminal_psi::StructuralStoreDisposition::Vacant
                    },
                    destination: source,
                    path,
                    field,
                    value: StructuralArgument {
                        place: value.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                },
            );
            evaluation
                .evaluation
                .retire_structural_operation_operands(operation, operations)?;
            if let Some(index) = index {
                environment.windows.open.remove(index);
            }
            Ok(())
        }

        pub(crate) fn require_closed(&self) -> Result<(), LoweringError> {
            match self.open.iter().find(|absent| absent.restoration_required) {
                None => Ok(()),
                Some(absent) => Err(unpinned(
                    &absent.spelling,
                    "the place is still absent at an exit: missing restoration on this path",
                )),
            }
        }
    }

    fn unpinned(place: &str, reason: &'static str) -> LoweringError {
        LoweringError::UnpinnedBorrowedStorageWindow {
            place: place.to_owned(),
            reason,
        }
    }

    pub(crate) fn terminal_multiplicity(multiplicity: Multiplicity) -> StructuralMultiplicity {
        match multiplicity {
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Linear => StructuralMultiplicity::Linear,
        }
    }

    pub(crate) fn zeroed_image(multiplicity: StructuralMultiplicity) -> StructuralMultiplicity {
        match multiplicity {
            StructuralMultiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Linear => {
                StructuralMultiplicity::Affine
            }
        }
    }

    fn paths_overlap(
        first: &[StructuralPathSegment],
        first_field: Option<terminal_psi::TerminalIdentityIndex>,
        second: &[StructuralPathSegment],
        second_field: Option<terminal_psi::TerminalIdentityIndex>,
        coordinates: &[(
            semantic_vocabulary::ObligationId,
            semantic_vocabulary::ValueId,
            u64,
        )],
    ) -> bool {
        let normalize = |path: &[StructuralPathSegment]| {
            path.iter()
                .map(|segment| {
                    if let StructuralPathSegment::RuntimeIndex { index, obligation } = segment
                        && let Some((_, _, coordinate)) =
                            coordinates.iter().find(|(occurrence, value, _)| {
                                occurrence == obligation && value == index
                            })
                    {
                        StructuralPathSegment::FixedIndex(*coordinate)
                    } else {
                        segment.clone()
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut first = normalize(first);
        first.extend(first_field.map(StructuralPathSegment::Field));
        let mut second = normalize(second);
        second.extend(second_field.map(StructuralPathSegment::Field));
        terminal_semantics::structural_paths_may_overlap(&first, &second)
    }

    pub(crate) struct WindowEvaluation<'a, 'calls> {
        pub(crate) checked: &'a checked_trees::CheckedTrees,
        pub(crate) machine: symbols::SymbolHandle,
        pub(crate) state: symbols::SymbolHandle,
        pub(crate) statement_index: u32,
        pub(crate) evaluation: &'a mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        pub(crate) source_value_count: usize,
        pub(crate) calls: &'a mut CallEmissionContext<'calls>,
    }

    fn resolve_window_place(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        plan: &CheckedUnitStructuralArgumentPlan,
        required_access: StructuralAccess,
        parameters: &[StructuralParameterDeclaration],
        evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
        operations: &OperationBuffer,
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    ) -> Result<(ResolvedStorageAddress, StructuralTypeId), LoweringError> {
        let place = resolve_storage_address(
            identities,
            plan,
            required_access,
            parameters,
            evaluation,
            operations,
            structural_types,
            type_ids,
        )?;
        let checked_leaf_type = lookup_type_id(type_ids, &plan.type_identity)?;
        let hole_type = if let Some(field) = &place.field {
            field
                .field_type
                .as_structural()
                .or_else(|| {
                    let declaration =
                        terminal_psi::find_structural_type(structural_types, checked_leaf_type)?;
                    let presents = match field.field_type.byte_sequence_carrier() {
                        Some(carrier) => match declaration.shape {
                            terminal_psi::StructuralTypeShape::ByteSequence(declared) => {
                                terminal_semantics::byte_carrier_geometry_matches(carrier, declared)
                            }
                            _ => false,
                        },
                        None => {
                            field.field_type.declares_leaf(declaration)
                                || (declaration.scalar_restriction.is_none()
                                    && matches!(
                                        field.field_type,
                                        terminal_psi::StructuralFieldType::BoundedInteger(_)
                                            | terminal_psi::StructuralFieldType::RefinedScalar(_)
                                    )
                                    && field.field_type.scalar_type().is_some_and(|scalar| {
                                        declaration.shape
                                            == terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                                scalar,
                                            )
                                    }))
                        }
                    };
                    presents.then_some(checked_leaf_type)
                })
                .ok_or_else(|| {
                    unpinned(
                        &place.spelling,
                        "the field has no exact structural storage endpoint",
                    )
                })?
        } else {
            place.subtree_type.ok_or_else(|| {
                unpinned(
                    &place.spelling,
                    "the window has no exact structural endpoint",
                )
            })?
        };
        if checked_leaf_type != hole_type {
            return Err(unpinned(
                &place.spelling,
                "the checked place type differs from the declared leaf type",
            ));
        }
        Ok((place, hole_type))
    }

    fn resolve_storage_address(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        plan: &CheckedUnitStructuralArgumentPlan,
        required_access: StructuralAccess,
        parameters: &[StructuralParameterDeclaration],
        evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
        operations: &OperationBuffer,
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    ) -> Result<ResolvedStorageAddress, LoweringError> {
        let spelling = spell_place(plan);
        let (
            source,
            root_type,
            root_access,
            root_multiplicity,
            restoration_required,
            qualifications,
            projected_qualifications,
            additional_input_qualifications,
            alias_prefix,
            path_root,
        ) = match plan.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = parameters
                    .get(parameter_index as usize)
                    .ok_or_else(|| unpinned(&spelling, "the parameter root is not declared"))?;
                if !required_access.admits(parameter.access) {
                    return Err(unpinned(
                        &spelling,
                        "the root is not an exclusive `&mut` borrow: shared and write-only loans \
                         admit no extraction and owned roots use ordinary partial moves",
                    ));
                }
                if parameter.access == StructuralAccess::MutableBorrow
                    && !matches!(
                        parameter.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                {
                    return Err(unpinned(
                        &spelling,
                        "the root carries linear custody the window does not restore",
                    ));
                }
                (
                    parameter.place,
                    parameter.structural_type,
                    parameter.access,
                    parameter.multiplicity,
                    parameter.access != StructuralAccess::Owned,
                    parameter.qualifications.clone(),
                    parameter.projected_qualifications.clone(),
                    parameter.additional_input_qualifications.clone(),
                    Vec::new(),
                    parameter.structural_type,
                )
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let root = super::store_destination::local_store_place(
                    symbol,
                    &evaluation.structural_locals,
                    evaluation.structural_root_parameters(),
                    operations,
                )
                .map_err(|error| match error {
                    LoweringError::Unsupported(reason) => unpinned(&spelling, reason),
                    error => error,
                })?;
                if !required_access.admits(root.argument.access) {
                    return Err(unpinned(
                        &spelling,
                        "the local lacks the required storage access",
                    ));
                }
                (
                    root.argument.place,
                    root.structural_type,
                    root.argument.access,
                    root.multiplicity,
                    root.argument.access != StructuralAccess::Owned,
                    root.qualifications.to_vec(),
                    root.projected_qualifications.to_vec(),
                    root.additional_input_qualifications.to_vec(),
                    root.argument.path.clone(),
                    root.selected_type,
                )
            }
            _ => {
                return Err(unpinned(
                    &spelling,
                    "borrowed-storage windows require a mutable-borrowed parameter or an \
                     established owned or exclusive structural local",
                ));
            }
        };
        if plan.access != CheckedStructuralAccess::Owned {
            return Err(unpinned(
                &spelling,
                "the place is not taken as an owned value",
            ));
        }
        let whole_owned_home = root_access == StructuralAccess::Owned
            && alias_prefix.is_empty()
            && lookup_type_id(type_ids, &plan.type_identity)? == root_type;
        if plan.path.is_empty()
            && (whole_owned_home
                || matches!(
                    plan.source,
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
                ))
        {
            return Ok(ResolvedStorageAddress {
                source,
                root_type,
                root_access,
                root_multiplicity,
                steps: Vec::new(),
                fixed_prefix: 0,
                checked_path: Vec::new(),
                field: None,
                subtree_type: Some(root_type),
                spelling,
                restoration_required,
                qualifications,
                projected_qualifications,
                additional_input_qualifications,
            });
        }
        let (lowered_steps, leaf, borrowed_backing) =
            lower_window_path(identities, path_root, &plan.path, structural_types).map_err(
                |error| match error {
                    LoweringError::Unsupported(reason) => unpinned(&spelling, reason),
                    other => other,
                },
            )?;
        let restoration_required = restoration_required
            || borrowed_backing
            || alias_prefix.contains(&StructuralPathSegment::Referent);
        let leading_referent = matches!(
            lowered_steps.first(),
            Some(ProjectionStep::Static(StructuralPathSegment::Referent))
        );
        let fixed_prefix = alias_prefix.len();
        let steps = alias_prefix
            .into_iter()
            .map(ProjectionStep::Static)
            .chain(lowered_steps)
            .collect::<Vec<_>>();
        let (field, subtree_type) = match leaf {
            WindowLeaf::Field(field) => (Some(field.clone()), None),
            WindowLeaf::Subtree { structural_type } => (None, Some(structural_type)),
        };
        let mut checked_path = plan.path.clone();
        if leading_referent
            && !matches!(
                plan.path.first(),
                Some(CheckedUnitStructuralPathSegment::Referent)
            )
        {
            checked_path.insert(0, CheckedUnitStructuralPathSegment::Referent);
        }
        Ok(ResolvedStorageAddress {
            source,
            root_type,
            root_access,
            root_multiplicity,
            steps,
            fixed_prefix,
            checked_path,
            field,
            subtree_type,
            spelling,
            restoration_required,
            qualifications,
            projected_qualifications,
            additional_input_qualifications,
        })
    }

    fn spell_place(plan: &CheckedUnitStructuralArgumentPlan) -> String {
        let mut spelling = match plan.source {
            CheckedUnitStructuralArgumentSourcePlan::CapturedStorage { source_ordinal, .. } => {
                format!("captured selection source#{source_ordinal}")
            }
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index } => {
                format!("scalar parameter#{parameter_index}")
            }
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                format!("parameter#{parameter_index}")
            }
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. } => {
                "local".to_owned()
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } => {
                "computed expression".to_owned()
            }
            CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                declaration_ordinal,
            } => format!("local#{declaration_ordinal}"),
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                format!("result#{binding_ordinal}")
            }
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. } => {
                "literal".to_owned()
            }
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            } => format!("parameter#{parameter_index}[..]"),
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: checked_trees::CheckedStorageRoot::ViewLocal { .. },
                ..
            }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root: checked_trees::CheckedStorageRoot::ViewLocal { .. },
                ..
            } => "local[..]".to_owned(),
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: checked_trees::CheckedStorageRoot::OwnedLocal { .. },
                ..
            }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root: checked_trees::CheckedStorageRoot::OwnedLocal { .. },
                ..
            } => "local[..]".to_owned(),
        };
        for segment in &plan.path {
            match segment {
                CheckedUnitStructuralPathSegment::AllElements => {
                    spelling.push_str(".[all elements]")
                }
                CheckedUnitStructuralPathSegment::RepresentationView { type_identity, .. } => {
                    spelling.push_str(&format!(".[view {type_identity:?}]"));
                }
                CheckedUnitStructuralPathSegment::Case(identity) => {
                    spelling.push_str(".[");
                    spelling.push_str(&identity.spelling());
                    spelling.push(']');
                }
                CheckedUnitStructuralPathSegment::Field(identity) => {
                    spelling.push('.');
                    spelling.push_str(&identity.spelling());
                }
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    spelling.push_str(&format!("[{index}]"));
                }
                CheckedUnitStructuralPathSegment::Referent => spelling.push_str(".*"),
                CheckedUnitStructuralPathSegment::FixedRange { start, end } => {
                    spelling.push_str(&format!("[{start}..{end}]"));
                }
                CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                    spelling.push_str(&format!("[computed#{root:?}]"));
                }
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol, .. },
                ) => spelling.push_str(&format!("[storage#{symbol:?}]")),
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Parameter { position },
                ) => {
                    spelling.push_str(&format!("[selector#{position}]"));
                }
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                ) => spelling.push_str(&format!("[index#{depth}]")),
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Local { position },
                ) => spelling.push_str(&format!("[local#{position}]")),
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StructuralField(field),
                ) => {
                    spelling.push_str(&format!(
                        "[field#{}:{}]",
                        field.parameter_position,
                        field
                            .path
                            .iter()
                            .map(|segment| match segment {
                                checked_trees::CheckedStructuralPredicatePathSegment::Referent =>
                                    "*".to_string(),
                                checked_trees::CheckedStructuralPredicatePathSegment::Field(
                                    identity,
                                ) => identity.spelling(),
                                checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                                    format!("as {}", case.spelling())
                                }
                                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(
                                    index,
                                ) => index.to_string(),
                                checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                    _,
                                ) => "rt".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(".")
                    ));
                }
            }
        }
        spelling
    }
}

pub(crate) mod continuation_identities {

    use semantic_vocabulary::{
        BlockId, ClaimId, EdgeId, ObligationId, OperationId, PlaceId, ValueId,
    };
    use terminal_psi::{Block, Operation, OperationResult, StructuralPathSegment};

    use crate::shared::lowering_error::{LoweringError, unsupported};

    pub(crate) mod companions {

        use super::IdentityMap;
        use crate::shared::lowering_error::LoweringError;
        use lowered_psi::{
            LoweredSourceCallOccurrence, LoweredSourceCallSite, LoweredSourceLoanCapture,
        };
        use terminal_psi::{LoanCompletion, ScalarBlockInvariant};

        impl IdentityMap {
            pub(crate) fn loan_capture(&mut self, capture: &mut LoweredSourceLoanCapture) {
                capture.formation = self.operation(capture.formation);
                capture.carrier = self.place(capture.carrier);
                self.argument(&mut capture.source);
            }

            pub(crate) fn source_call(&mut self, call: &mut LoweredSourceCallOccurrence) {
                call.terminal_site = match call.terminal_site {
                    LoweredSourceCallSite::Operation(operation) => {
                        LoweredSourceCallSite::Operation(self.operation(operation))
                    }
                    LoweredSourceCallSite::InvokeNoNormal(edge) => {
                        LoweredSourceCallSite::InvokeNoNormal(self.edge(edge))
                    }
                    LoweredSourceCallSite::ExternalCompletion(edge) => {
                        LoweredSourceCallSite::ExternalCompletion(self.edge(edge))
                    }
                };
                let frontier = &mut call.frontier_before_call;
                for capture in &mut frontier.loan_captures {
                    self.loan_capture(capture);
                }
                for binding in &mut frontier.scalars {
                    binding.value.id = self.value(binding.value.id);
                }
                for binding in &mut frontier.structural {
                    self.argument(&mut binding.argument);
                }
                for (_, claim) in &mut frontier.claims {
                    *claim = self.claim(*claim);
                }
            }

            pub(crate) fn loan_completion(&mut self, completion: &mut LoanCompletion) {
                completion.operation = self.operation(completion.operation);
                completion.place = self.place(completion.place);
                self.path(&mut completion.path);
                for claim in &mut completion.claims {
                    claim.claim = self.claim(claim.claim);
                    self.path(&mut claim.path);
                }
                completion.claims.sort_by_key(|row| row.claim);
            }

            pub(crate) fn block_invariant(
                &mut self,
                invariant: &mut ScalarBlockInvariant,
            ) -> Result<(), LoweringError> {
                invariant.header = self.block_id(invariant.header);
                self.proposition(&mut invariant.predicate)?;
                for arrival in &mut invariant.arrivals {
                    arrival.edge = self.edge(arrival.edge);
                    arrival.obligation = self.obligation(arrival.obligation);
                }
                invariant.arrivals.sort_by_key(|arrival| arrival.edge);
                Ok(())
            }
        }
    }
    pub(crate) mod operations {

        use super::IdentityMap;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use semantic_vocabulary::ObligationId;
        use terminal_psi::{
            ElementViewRegion, OperationKind, RecordFieldValue, StructuralPathSegment,
        };

        impl IdentityMap {
            pub(crate) fn operation_kind(
                &mut self,
                operation: &mut OperationKind,
            ) -> Result<(), LoweringError> {
                let content_derivations = match operation {
                    OperationKind::TransformContent { derivations, .. } => {
                        std::mem::take(derivations)
                    }
                    _ => Vec::new(),
                };
                operation.map_scalar_uses(&mut |value| self.value(value));
                operation.map_structural_uses(&mut |place| self.place(place));
                map_obligations(operation, &mut |obligation| self.obligation(obligation));
                match operation {
                    OperationKind::EstablishNativeCodeLayout { .. }
                    | OperationKind::EstablishActivationClaim { .. }
                    | OperationKind::ReleaseActivationClaim { .. } => {}
                    OperationKind::TransformContent { derivations, .. } => {
                        *derivations = content_derivations;
                        for derived in derivations.iter_mut() {
                            derived.map_places(&mut |place| self.place(place));
                            for root in &mut derived.lineage_roots {
                                *root = self.claim(*root);
                            }
                            derived.lineage_roots.sort();
                            for step in &mut derived.ancestry {
                                step.producer_operation = self.operation(step.producer_operation);
                                for claim in step.inputs.iter_mut().chain(&mut step.outputs) {
                                    *claim = self.claim(*claim);
                                }
                                step.inputs.sort();
                                step.outputs.sort();
                            }
                            for binding in derived.inputs.iter_mut().chain(&mut derived.outputs) {
                                binding.claim = self.claim(binding.claim);
                            }
                            if let terminal_psi::ContentTransformationCustody::Boundary {
                                addresses,
                                ..
                            } = &mut derived.custody
                            {
                                for address in addresses.iter_mut() {
                                    address.input.claim = self.claim(address.input.claim);
                                    address.output.claim = self.claim(address.output.claim);
                                }
                                addresses.sort();
                            }
                            derived.inputs.sort();
                            derived.outputs.sort();
                        }
                        derivations.sort();
                    }
                    OperationKind::EstablishByteSequenceLiteral { destination, .. }
                    | OperationKind::EstablishElementView { destination, .. }
                    | OperationKind::EstablishMutableElementView { destination, .. }
                    | OperationKind::EstablishTrivialAffineLocal { destination } => {
                        *destination = self.place(*destination);
                    }
                    OperationKind::Call {
                        erased_arguments,
                        erased_proof_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallUnit {
                        erased_arguments,
                        erased_proof_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        erased_arguments,
                        erased_proof_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        erased_arguments,
                        erased_proof_arguments,
                        crash_continuations,
                        ..
                    }
                    | OperationKind::CallDynamic {
                        erased_arguments,
                        erased_proof_arguments,
                        crash_continuations,
                        ..
                    } => {
                        for argument in erased_arguments {
                            self.scalar_term(argument);
                        }
                        for argument in erased_proof_arguments {
                            self.proof_term(argument);
                        }
                        self.crash_routes(crash_continuations)?;
                    }
                    OperationKind::CallStructural {
                        crash_continuations,
                        selected_evidence,
                        ..
                    } => {
                        if !selected_evidence.is_empty() {
                            return unsupported(
                                "continuation specialization must precede selected evidence retention",
                            );
                        }
                        self.crash_routes(crash_continuations)?;
                    }
                    _ => {}
                }
                match operation {
                    OperationKind::ReleaseActivationClaim { claim, .. } => {
                        *claim = self.claim(*claim)
                    }
                    OperationKind::SplitActivationClaim { claim, .. } => {
                        *claim = self.claim(*claim)
                    }
                    OperationKind::MergeActivationClaim {
                        lower_claim,
                        upper_claim,
                        ..
                    } => {
                        *lower_claim = self.claim(*lower_claim);
                        *upper_claim = self.claim(*upper_claim);
                    }
                    OperationKind::CallUnit {
                        claim_transfers, ..
                    }
                    | OperationKind::CallStructuralScalar {
                        claim_transfers, ..
                    }
                    | OperationKind::CallStructural {
                        claim_transfers, ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        claim_transfers, ..
                    }
                    | OperationKind::CallDynamic {
                        claim_transfers, ..
                    } => {
                        for transfer in claim_transfers.iter_mut() {
                            transfer.claim = self.claim(transfer.claim);
                            self.path(&mut transfer.path);
                        }
                        claim_transfers.sort();
                    }
                    OperationKind::BoundaryCall {
                        completion_receipts,
                        ..
                    } => {
                        for receipt in completion_receipts.iter_mut() {
                            receipt.claim = self.claim(receipt.claim);
                            self.path(&mut receipt.path);
                        }
                        completion_receipts.sort();
                    }
                    _ => {}
                }
                Ok(())
            }
        }

        pub(crate) fn map_obligations(
            operation: &mut OperationKind,
            map: &mut impl FnMut(ObligationId) -> ObligationId,
        ) {
            for path in operation.structural_projection_paths_mut() {
                for segment in path {
                    if let StructuralPathSegment::RuntimeIndex { obligation, .. } = segment {
                        *obligation = map(*obligation);
                    }
                }
            }
            match operation {
                OperationKind::ReadScalarApplication { .. }
                | OperationKind::EstablishNativeCodeLayout { .. }
                | OperationKind::EstablishActivationClaim { .. }
                | OperationKind::SplitActivationClaim { .. }
                | OperationKind::MergeActivationClaim { .. }
                | OperationKind::ReleaseActivationClaim { .. } => {}
                OperationKind::EstablishPrimitiveLocal {
                    range_obligation, ..
                }
                | OperationKind::WriteOnlyPrimitiveStore {
                    range_obligation, ..
                }
                | OperationKind::StructuralScalarFieldStore {
                    range_obligation, ..
                } => {
                    if let Some(obligation) = range_obligation {
                        *obligation = map(*obligation);
                    }
                }
                OperationKind::StructuralByteViewFieldStore {
                    predicate_obligation,
                    ..
                }
                | OperationKind::ScalarRepresentationView {
                    landing_obligation: predicate_obligation,
                    ..
                } => {
                    if let Some(obligation) = predicate_obligation {
                        *obligation = map(*obligation);
                    }
                }
                OperationKind::StructuralByteSequenceFieldByteStore { obligation, .. }
                | OperationKind::ByteSequenceStore { obligation, .. }
                | OperationKind::StructuralByteSequenceFieldStore { obligation, .. }
                | OperationKind::StructuralByteSequenceFieldRead { obligation, .. }
                | OperationKind::EstablishByteSequenceConcatenation {
                    capacity_obligation: obligation,
                    ..
                }
                | OperationKind::ByteSequenceRead { obligation, .. }
                | OperationKind::ByteSequenceWrite { obligation, .. }
                | OperationKind::ByteSequenceSubslice { obligation, .. }
                | OperationKind::ElementViewRead { obligation, .. }
                | OperationKind::ElementViewWrite { obligation, .. }
                | OperationKind::ElementViewSubslice { obligation, .. }
                | OperationKind::IntegerExactCast { obligation, .. }
                | OperationKind::ExactIntegerShiftLeft { obligation, .. }
                | OperationKind::ExactIntegerShiftRight { obligation, .. }
                | OperationKind::ExactIntegerAdd { obligation, .. }
                | OperationKind::ExactIntegerSubtract { obligation, .. }
                | OperationKind::ExactIntegerMultiply { obligation, .. }
                | OperationKind::ExactIntegerDivide { obligation, .. }
                | OperationKind::ExactIntegerRemainder { obligation, .. }
                | OperationKind::WrappingIntegerDivide { obligation, .. }
                | OperationKind::WrappingIntegerRemainder { obligation, .. }
                | OperationKind::SaturatingIntegerDivide { obligation, .. }
                | OperationKind::SaturatingIntegerRemainder { obligation, .. } => {
                    *obligation = map(*obligation)
                }
                OperationKind::EstablishRecord { fields }
                | OperationKind::EstablishStructuralCase { fields, .. } => {
                    for field in fields {
                        if let RecordFieldValue::Scalar {
                            range_obligation: Some(obligation),
                            ..
                        } = &mut field.value
                        {
                            *obligation = map(*obligation);
                        }
                    }
                }
                OperationKind::EstablishScalarCase { fields, .. } => {
                    for field in fields {
                        if let Some(obligation) = &mut field.range_obligation {
                            *obligation = map(*obligation);
                        }
                    }
                }
                OperationKind::EstablishElementView { region, .. }
                | OperationKind::EstablishMutableElementView { region, .. } => {
                    if let ElementViewRegion::RepresentationBytes { obligation, .. } = region {
                        *obligation = map(*obligation);
                    } else if let ElementViewRegion::WholeRepresentation { obligation, .. } = region
                    {
                        *obligation = map(*obligation);
                    }
                }
                OperationKind::Call {
                    requirement_obligations,
                    ..
                }
                | OperationKind::CallUnit {
                    requirement_obligations,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    requirement_obligations,
                    ..
                }
                | OperationKind::CallStructural {
                    requirement_obligations,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    requirement_obligations,
                    ..
                }
                | OperationKind::CallDynamic {
                    requirement_obligations,
                    ..
                }
                | OperationKind::BoundaryCall {
                    requirement_obligations,
                    ..
                } => {
                    for obligation in requirement_obligations {
                        *obligation = map(*obligation);
                    }
                }
                OperationKind::EstablishAccessReference { .. }
                | OperationKind::EstablishReference { .. }
                | OperationKind::ReleaseReference { .. }
                | OperationKind::EstablishScalarArray { .. }
                | OperationKind::EstablishStructuralArray { .. }
                | OperationKind::EstablishZeroedStructural { .. }
                | OperationKind::EstablishStructuralLocal { .. }
                | OperationKind::StorageAddress { .. }
                | OperationKind::PrimitiveScalarRead { .. }
                | OperationKind::StructuralByteSequenceFieldLength { .. }
                | OperationKind::MoveStructuralField { .. }
                | OperationKind::MoveStructuralValue { .. }
                | OperationKind::TransformContent { .. }
                | OperationKind::StoreStructuralField { .. }
                | OperationKind::StructuralCaseMembership { .. }
                | OperationKind::StructuralLeafCopy { .. }
                | OperationKind::StructuralCaseLeafCopy { .. }
                | OperationKind::EstablishByteSequenceLiteral { .. }
                | OperationKind::ByteSequenceLength { .. }
                | OperationKind::ByteSequencePredicate { .. }
                | OperationKind::ByteSequenceEqual { .. }
                | OperationKind::ElementViewLength { .. }
                | OperationKind::EstablishTrivialAffineLocal { .. }
                | OperationKind::SelectImplementation { .. }
                | OperationKind::PortWrite { .. }
                | OperationKind::RegisterAccess { .. }
                | OperationKind::ActivateTaskRegister { .. }
                | OperationKind::ReadTaskRegister { .. }
                | OperationKind::ReloadSegmentDescriptors { .. }
                | OperationKind::ReadSegmentSelector { .. }
                | OperationKind::GlobalDescriptorTable { .. }
                | OperationKind::IntegerConstant { .. }
                | OperationKind::BooleanConstant { .. }
                | OperationKind::IeeeFloatConstant { .. }
                | OperationKind::FloatResultPolicy { .. }
                | OperationKind::IeeeFloatCompare { .. }
                | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                | OperationKind::BooleanStructuralField { .. }
                | OperationKind::ScalarStructuralField { .. }
                | OperationKind::BooleanNot { .. }
                | OperationKind::BooleanEqual { .. }
                | OperationKind::IntegerEqual { .. }
                | OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. }
                | OperationKind::IntegerBitwiseNot { .. }
                | OperationKind::IntegerWiden { .. }
                | OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. }
                | OperationKind::WrappingIntegerShiftLeft { .. }
                | OperationKind::WrappingIntegerShiftRight { .. }
                | OperationKind::WrappingIntegerAdd { .. }
                | OperationKind::SaturatingIntegerAdd { .. }
                | OperationKind::WrappingIntegerSubtract { .. }
                | OperationKind::SaturatingIntegerSubtract { .. }
                | OperationKind::WrappingIntegerMultiply { .. }
                | OperationKind::SaturatingIntegerMultiply { .. }
                | OperationKind::TrappingInteger { .. }
                | OperationKind::AtomicFence { .. }
                | OperationKind::AtomicAccess { .. } => {}
            }
        }
    }
    pub(crate) mod proofs {

        use semantic_vocabulary::{
            ContentConservation, ContentTerm, IntegerMathTerm, MathematicalArgument, ProofTerm,
            Proposition, ScalarTerm,
        };
        use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

        use crate::shared::lowering_error::LoweringError;

        impl super::IdentityMap {
            pub(crate) fn scalar_term(&mut self, term: &mut ScalarTerm) {
                let mut pending = vec![term];
                while let Some(term) = pending.pop() {
                    match term {
                        ScalarTerm::Value { id, .. } => *id = self.value(*id),
                        ScalarTerm::StructuralRead { root, path, .. } => {
                            *root = self.place(*root);
                            for projection in path.iter_mut().rev() {
                                if let semantic_vocabulary::ScalarReadProjection::Index(selector) =
                                    projection
                                {
                                    pending.push(selector);
                                }
                            }
                        }

                        ScalarTerm::BooleanField { root, .. }
                        | ScalarTerm::IntegerField { root, .. }
                        | ScalarTerm::IeeeFloatField { root, .. }
                        | ScalarTerm::ViewExtent { root, .. }
                        | ScalarTerm::ByteSequencePredicate { root, .. } => {
                            *root = self.place(*root)
                        }
                        semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                            pending.push(operand);
                        }
                        semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                            pending.push(right);
                            pending.push(left);
                        }
                        ScalarTerm::Application { .. }
                        | ScalarTerm::Boolean(_)
                        | ScalarTerm::Integer { .. }
                        | ScalarTerm::IeeeFloatLiteral(_) => {}
                    }
                }
            }

            pub(crate) fn proof_term(&mut self, term: &mut ProofTerm) {
                let mut pending = vec![term];
                while let Some(term) = pending.pop() {
                    match term {
                        ProofTerm::Construction { fields, .. } => {
                            pending.extend(fields.iter_mut().rev().map(|field| &mut field.term));
                        }
                        ProofTerm::Array { elements, .. } => {
                            pending.extend(elements.iter_mut().rev())
                        }
                        ProofTerm::Scalar(term) => self.scalar_term(term),
                        ProofTerm::MathematicalApplication { arguments, .. } => {
                            for argument in arguments {
                                if let MathematicalArgument::Scalar { term, .. } = argument {
                                    self.scalar_term(term);
                                }
                            }
                        }
                        ProofTerm::Formal { .. }
                        | ProofTerm::InvocationFormal { .. }
                        | ProofTerm::Mathematical { .. }
                        | ProofTerm::Projection { .. } => {}
                    }
                }
            }

            fn integer_math_term(&mut self, term: &mut IntegerMathTerm) {
                let mut pending = vec![term];
                while let Some(term) = pending.pop() {
                    match term {
                        IntegerMathTerm::Embed(term) => self.scalar_term(term),
                        IntegerMathTerm::MathValue { value, .. } => *value = self.value(*value),
                        IntegerMathTerm::IntegerLiteral(_) => {}
                        IntegerMathTerm::Add(left, right)
                        | IntegerMathTerm::Subtract(left, right)
                        | IntegerMathTerm::Multiply(left, right)
                        | IntegerMathTerm::ShiftLeft {
                            value: left,
                            count: right,
                        } => {
                            pending.push(right);
                            pending.push(left);
                        }
                    }
                }
            }

            fn content_term(&mut self, term: &mut ContentTerm) {
                match term {
                    ContentTerm::Projection { subject, .. } => {
                        subject.root = self.place(subject.root);
                    }
                    ContentTerm::Separate(children) => {
                        for child in children.iter_mut() {
                            self.content_term(child);
                        }
                        children.sort();
                    }
                }
            }

            pub(crate) fn proposition(
                &mut self,
                proposition: &mut Proposition,
            ) -> Result<(), LoweringError> {
                match proposition {
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                    Proposition::Equal(left, right) => {
                        self.scalar_term(left);
                        self.scalar_term(right);
                        let left_key = terminal_codec::canonical_scalar_term_order_key(left)
                            .map_err(LoweringError::DebugSemanticCodec)?;
                        let right_key = terminal_codec::canonical_scalar_term_order_key(right)
                            .map_err(LoweringError::DebugSemanticCodec)?;
                        if left_key > right_key {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                        self.scalar_term(left);
                        self.scalar_term(right);
                    }
                    Proposition::IntegerMathEqual(left, right) => {
                        self.integer_math_term(left);
                        self.integer_math_term(right);
                        if left > right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::IntegerMathLessThan(left, right)
                    | Proposition::IntegerMathLessOrEqual(left, right) => {
                        self.integer_math_term(left);
                        self.integer_math_term(right);
                    }
                    Proposition::IeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        *left = left.rebase(self.place(left.root()), &[]);
                        *right = right.rebase(self.place(right.root()), &[]);
                        if kind.is_symmetric() && left > right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::ScalarIeeeFloatComparison {
                        kind, left, right, ..
                    } => {
                        self.scalar_term(left);
                        self.scalar_term(right);
                        if kind.is_symmetric() && left > right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        *left = left.rebase(self.place(left.root()), &[]);
                        *right = right.rebase(self.place(right.root()), &[]);
                        if left > right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::StructuralCaseMembership { subject, .. } => {
                        *subject = subject.rebase(self.place(subject.root()), &[]);
                    }
                    Proposition::StructuralIdentity { left, right } => {
                        *left = left.rebase(self.place(left.root()), &[]);
                        *right = right.rebase(self.place(right.root()), &[]);
                        if left > right {
                            std::mem::swap(left, right);
                        }
                    }
                    Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                        for child in children.iter_mut() {
                            self.proposition(child)?;
                        }
                        let mut keyed = children
                            .iter()
                            .map(|child| {
                                terminal_codec::canonical_proposition_order_key(child)
                                    .map(|key| (key, child.clone()))
                                    .map_err(LoweringError::DebugSemanticCodec)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        keyed.sort_by(|left, right| left.0.cmp(&right.0));
                        *children = keyed.into_iter().map(|(_, child)| child).collect();
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        self.proposition(premise)?;
                        self.proposition(conclusion)?;
                    }
                    Proposition::ContentConservation(conservation) => {
                        let mut left = conservation.left().clone();
                        let mut right = conservation.right().clone();
                        self.content_term(&mut left);
                        self.content_term(&mut right);
                        *conservation =
                            ContentConservation::new(conservation.algebra().clone(), left, right);
                    }
                }
                Ok(())
            }

            pub(crate) fn crash_routes(
                &mut self,
                routes: &mut [CrashRouteBucket],
            ) -> Result<(), LoweringError> {
                for route in routes {
                    for alternative in &mut route.alternatives {
                        match alternative {
                            CrashRouteGuard::Truth => {}
                            CrashRouteGuard::Predicate(predicate) => {
                                let mut proposition = predicate.proposition().clone();
                                self.proposition(&mut proposition)?;
                                *predicate = CrashPredicateTerm::new(proposition);
                            }
                        }
                    }
                    route.alternatives.sort();
                }
                Ok(())
            }
        }
    }
    pub(crate) mod termination {

        use super::IdentityMap;
        use crate::shared::lowering_error::LoweringError;
        use semantic_vocabulary::{BlockId, EdgeId};
        use terminal_psi::{SuccessorEdge, TerminalAffineCleanupAction, Terminator};

        impl IdentityMap {
            fn target(&mut self, edge: EdgeId, target: BlockId) -> BlockId {
                self.edge_targets
                    .iter()
                    .find_map(|(original, replacement)| (*original == edge).then_some(*replacement))
                    .unwrap_or_else(|| self.block_id(target))
            }

            fn successor(&mut self, successor: &mut SuccessorEdge) {
                successor.target = self.target(successor.edge, successor.target);
                successor.edge = self.edge(successor.edge);
                for argument in &mut successor.arguments {
                    *argument = self.value(*argument);
                }
                for argument in &mut successor.erased_arguments {
                    self.scalar_term(argument);
                }
                for argument in &mut successor.erased_proof_arguments {
                    self.proof_term(argument);
                }
                for argument in &mut successor.structural_arguments {
                    self.argument(argument);
                }
                self.cleanup(&mut successor.cleanup_actions);
            }

            pub(crate) fn cleanup(&mut self, actions: &mut [TerminalAffineCleanupAction]) {
                for action in actions {
                    match action {
                        TerminalAffineCleanupAction::DiscardRoot(place)
                        | TerminalAffineCleanupAction::RetireHome(place) => {
                            *place = self.place(*place)
                        }
                        TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            discard.place = self.place(discard.place);
                            self.path(&mut discard.path);
                        }
                        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            cleanup.place = self.place(cleanup.place);
                            self.path(&mut cleanup.path);
                            for claim in &mut cleanup.consumed_claims {
                                *claim = self.claim(*claim);
                            }
                            cleanup.consumed_claims.sort();
                            for obligation in &mut cleanup.requirement_obligations {
                                *obligation = self.obligation(*obligation);
                            }
                        }
                    }
                }
            }

            pub(crate) fn terminator(
                &mut self,
                terminator: &mut Terminator,
            ) -> Result<(), LoweringError> {
                match terminator {
                    Terminator::Jump {
                        edge,
                        target,
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        structural_arguments,
                        cleanup_actions,
                    } => {
                        *target = self.target(*edge, *target);
                        *edge = self.edge(*edge);
                        for argument in arguments {
                            *argument = self.value(*argument);
                        }
                        for argument in erased_arguments {
                            self.scalar_term(argument);
                        }
                        for argument in erased_proof_arguments {
                            self.proof_term(argument);
                        }
                        for argument in structural_arguments {
                            self.argument(argument);
                        }
                        self.cleanup(cleanup_actions);
                    }
                    Terminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } => {
                        *condition = self.value(*condition);
                        self.successor(when_true);
                        self.successor(when_false);
                    }
                    Terminator::StructuralCase {
                        source,
                        path,
                        cases,
                    } => {
                        *source = self.place(*source);
                        self.path(path);
                        for case in cases {
                            case.target = self.target(case.edge, case.target);
                            case.edge = self.edge(case.edge);
                            for argument in &mut case.structural_arguments {
                                self.argument(argument);
                            }
                            self.cleanup(&mut case.cleanup_actions);
                        }
                    }
                    Terminator::Return {
                        edge,
                        value,
                        cleanup_actions,
                    } => {
                        *edge = self.edge(*edge);
                        *value = self.value(*value);
                        self.cleanup(cleanup_actions);
                    }
                    Terminator::ReturnUnit {
                        edge,
                        cleanup_actions,
                    } => {
                        *edge = self.edge(*edge);
                        self.cleanup(cleanup_actions);
                    }
                    Terminator::ReturnStructural {
                        edge,
                        source,
                        returned_claims,
                        cleanup_actions,
                    } => {
                        *edge = self.edge(*edge);
                        *source = self.place(*source);
                        for claim in returned_claims.iter_mut() {
                            claim.claim = self.claim(claim.claim);
                            self.path(&mut claim.path);
                        }
                        returned_claims.sort_by_key(|row| row.claim);
                        self.cleanup(cleanup_actions);
                    }
                    Terminator::Crash {
                        edge,
                        site_guard,
                        frontier_lower_bound,
                        ..
                    } => {
                        *edge = self.edge(*edge);
                        for guard in site_guard.iter_mut() {
                            let mut proposition = guard.proposition().clone();
                            self.proposition(&mut proposition)?;
                            *guard = terminal_psi::CrashPredicateTerm::new(proposition);
                        }
                        site_guard.sort();
                        for claim in frontier_lower_bound.iter_mut() {
                            *claim = self.claim(*claim);
                        }
                        frontier_lower_bound.sort();
                    }
                    Terminator::InvokeNoNormal {
                        edge,
                        arguments,
                        erased_arguments,
                        erased_proof_arguments,
                        structural_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        ..
                    } => {
                        *edge = self.edge(*edge);
                        for argument in arguments {
                            *argument = self.value(*argument);
                        }
                        for argument in erased_arguments {
                            self.scalar_term(argument);
                        }
                        for argument in erased_proof_arguments {
                            self.proof_term(argument);
                        }
                        for argument in structural_arguments {
                            self.argument(argument);
                        }
                        for transfer in claim_transfers.iter_mut() {
                            transfer.claim = self.claim(transfer.claim);
                            self.path(&mut transfer.path);
                        }
                        claim_transfers.sort();
                        for obligation in requirement_obligations {
                            *obligation = self.obligation(*obligation);
                        }
                        self.crash_routes(crash_continuations)?;
                    }
                    Terminator::ExternalCompletion {
                        edge,
                        arguments,
                        structural_arguments,
                        requirement_obligations,
                        frontier_lower_bound,
                        ..
                    } => {
                        *edge = self.edge(*edge);
                        for argument in arguments {
                            *argument = self.value(*argument);
                        }
                        for argument in structural_arguments {
                            self.argument(argument);
                        }
                        for obligation in requirement_obligations {
                            *obligation = self.obligation(*obligation);
                        }
                        for claim in frontier_lower_bound.iter_mut() {
                            *claim = self.claim(*claim);
                        }
                        frontier_lower_bound.sort();
                    }
                }
                Ok(())
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct IdentityMap {
        pub(crate) observed: Option<IdentityUses>,
        pub(crate) values: Vec<(ValueId, ValueId)>,
        pub(crate) places: Vec<(PlaceId, PlaceId)>,
        pub(crate) blocks: Vec<(BlockId, BlockId)>,
        pub(crate) edges: Vec<(EdgeId, EdgeId)>,
        pub(crate) operations: Vec<(OperationId, OperationId)>,
        pub(crate) inherited_operations: Vec<(OperationId, OperationId)>,
        pub(crate) obligations: Vec<(ObligationId, ObligationId)>,
        pub(crate) claims: Vec<(ClaimId, ClaimId)>,
        pub(crate) edge_targets: Vec<(EdgeId, BlockId)>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct IdentityUses {
        pub(crate) values: Vec<ValueId>,
        pub(crate) places: Vec<PlaceId>,
        pub(crate) operations: Vec<OperationId>,
        pub(crate) claims: Vec<ClaimId>,
        pub(crate) obligations: Vec<ObligationId>,
    }

    fn remember<T: PartialEq>(values: &mut Vec<T>, value: T) {
        if !values.contains(&value) {
            values.push(value);
        }
    }

    fn mapped<T: Copy + PartialEq>(rows: &[(T, T)], original: T) -> T {
        rows.iter()
            .rev()
            .find_map(|(source, target)| (*source == original).then_some(*target))
            .unwrap_or(original)
    }

    impl IdentityMap {
        pub(crate) fn value(&mut self, original: ValueId) -> ValueId {
            if let Some(observed) = &mut self.observed {
                remember(&mut observed.values, original);
            }
            mapped(&self.values, original)
        }
        pub(crate) fn place(&mut self, original: PlaceId) -> PlaceId {
            if let Some(observed) = &mut self.observed {
                remember(&mut observed.places, original);
            }
            mapped(&self.places, original)
        }
        pub(crate) fn block_id(&mut self, original: BlockId) -> BlockId {
            mapped(&self.blocks, original)
        }
        pub(crate) fn edge(&mut self, original: EdgeId) -> EdgeId {
            mapped(&self.edges, original)
        }
        pub(crate) fn operation(&mut self, original: OperationId) -> OperationId {
            if let Some(observed) = &mut self.observed {
                remember(&mut observed.operations, original);
            }
            self.operations
                .iter()
                .rev()
                .find_map(|(source, target)| (*source == original).then_some(*target))
                .unwrap_or_else(|| mapped(&self.inherited_operations, original))
        }
        pub(crate) fn obligation(&mut self, original: ObligationId) -> ObligationId {
            if let Some(observed) = &mut self.observed {
                remember(&mut observed.obligations, original);
            }
            mapped(&self.obligations, original)
        }
        pub(crate) fn claim(&mut self, original: ClaimId) -> ClaimId {
            if let Some(observed) = &mut self.observed {
                remember(&mut observed.claims, original);
            }
            mapped(&self.claims, original)
        }

        pub(crate) fn block(&mut self, block: &mut Block) -> Result<(), LoweringError> {
            block.id = self.block_id(block.id);
            for value in block
                .parameters
                .iter_mut()
                .chain(&mut block.erased_scalar_formals)
            {
                value.id = self.value(value.id);
            }
            for parameter in &mut block.structural_parameters {
                parameter.place = self.place(parameter.place);
                for qualification in parameter
                    .projected_qualifications
                    .iter_mut()
                    .chain(&mut parameter.additional_input_qualifications)
                {
                    self.path(&mut qualification.path);
                }
            }
            for operation in &mut block.operations {
                self.operation_definition(operation)?;
            }
            self.terminator(&mut block.terminator)
        }

        pub(crate) fn operation_definition(
            &mut self,
            operation: &mut Operation,
        ) -> Result<(), LoweringError> {
            if operation.static_reach_binding.is_some() || operation.suspension_crossing.is_some() {
                return unsupported(
                    "continuation specialization must precede source certificate retention",
                );
            }
            operation.id = self.operation(operation.id);
            match &mut operation.result {
                OperationResult::Unit => {}
                OperationResult::Scalar(value) => value.id = self.value(value.id),
                OperationResult::Structural(result) => {
                    result.place = self.place(result.place);
                    for row in result
                        .projected_qualifications
                        .iter_mut()
                        .chain(&mut result.additional_input_qualifications)
                    {
                        self.path(&mut row.path);
                    }
                    for row in &mut result.claims {
                        row.claim = self.claim(row.claim);
                        self.path(&mut row.path);
                    }
                    result.claims.sort_by_key(|row| row.claim);
                }
            }
            self.operation_kind(&mut operation.kind)
        }

        pub(crate) fn path(&mut self, path: &mut [StructuralPathSegment]) {
            for segment in path {
                if let StructuralPathSegment::RuntimeIndex { index, obligation } = segment {
                    *index = self.value(*index);
                    *obligation = self.obligation(*obligation);
                }
            }
        }

        pub(crate) fn argument(&mut self, argument: &mut terminal_psi::StructuralArgument) {
            argument.place = self.place(argument.place);
            self.path(&mut argument.path);
        }

        pub(crate) fn place_declaration(
            &mut self,
            declaration: &mut terminal_psi::StructuralPlaceDeclaration,
        ) {
            declaration.id = self.place(declaration.id);
            match &mut declaration.kind {
                semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                    *producer = self.operation(*producer)
                }
                semantic_vocabulary::StructuralPlaceKind::BlockParameter { block, .. } => {
                    *block = self.block_id(*block)
                }
                semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                | semantic_vocabulary::StructuralPlaceKind::Result
                | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
                | semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { .. }
                | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal { .. } => {}
            }
        }
    }
}

pub(crate) mod continuation_specialization {

    use super::continuation_identities::{IdentityMap, IdentityUses};
    use super::selected_continuations::{SelectedCleanupGroup, Specialization};
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::terminal_identities::{
        allocate_dense, block_id, claim_id, edge_id, obligation_id, operation_id, place_id,
        value_id,
    };
    use lowered_psi::{LoweredSelectedComparisonOccurrence, LoweredSourceCallOccurrence};
    use semantic_vocabulary::{
        BlockId, ClaimId, EdgeId, ObligationId, OperationId, PlaceId, StructuralPlaceKind, ValueId,
    };
    use terminal_psi::{
        ScalarBlockInvariant, ScalarQualificationCoercion, StructuralTypeDeclaration,
        TerminalMachine,
    };

    pub(crate) mod companions {

        use super::{CompanionRows, Counters, Instance};
        use crate::emission::selected_continuations::Specialization;
        use crate::shared::lowering_error::{LoweringError, unsupported};
        use crate::shared::terminal_identities::{allocate_dense, obligation_id};
        use lowered_psi::LoweredSourceCallSite;
        use terminal_psi::TerminalMachine;

        pub(crate) fn instantiate(
            machine: &mut TerminalMachine,
            topology: &Specialization,
            instances: &mut [Instance],
            rows: &mut CompanionRows<'_>,
            counters: &mut Counters<'_>,
        ) -> Result<(), LoweringError> {
            let calls = std::mem::take(rows.source_calls);
            let returns = std::mem::take(rows.source_returns);
            let comparisons = std::mem::take(rows.selected_comparisons);
            let completions = std::mem::take(&mut machine.loan_completions);
            let invariants = std::mem::take(rows.scalar_block_invariants);
            let coercions = std::mem::take(rows.qualification_coercions);
            let source_checked = std::mem::take(rows.source_checked_obligations);
            for instance in instances.iter_mut() {
                let namespace = &mut instance.namespace;
                for original in &returns {
                    if let Some((_, edge)) = namespace
                        .edges
                        .iter()
                        .find(|(source, _)| *source == original.edge)
                    {
                        let mut returned = original.clone();
                        returned.edge = *edge;
                        rows.source_returns.push(returned);
                    }
                }
                for original in &calls {
                    let owned = match original.terminal_site {
                        LoweredSourceCallSite::Operation(operation) => namespace
                            .operations
                            .iter()
                            .any(|(source, _)| *source == operation),
                        LoweredSourceCallSite::InvokeNoNormal(edge)
                        | LoweredSourceCallSite::ExternalCompletion(edge) => {
                            namespace.edges.iter().any(|(source, _)| *source == edge)
                        }
                    };
                    if owned {
                        let mut call = original.clone();
                        namespace.source_call(&mut call);
                        rows.source_calls.push(call);
                    }
                }
                for original in &comparisons {
                    if let Some((_, target)) = namespace
                        .operations
                        .iter()
                        .find(|(source, _)| *source == original.terminal_operation)
                    {
                        let mut comparison = *original;
                        comparison.terminal_operation = *target;
                        rows.selected_comparisons.push(comparison);
                    }
                }
                for original in &completions {
                    if namespace
                        .operations
                        .iter()
                        .any(|(source, _)| *source == original.operation)
                    {
                        let mut completion = original.clone();
                        namespace.loan_completion(&mut completion);
                        completion.claims.sort_by_key(|binding| binding.claim);
                        machine.loan_completions.push(completion);
                    }
                }
                for original in &source_checked {
                    if let Some((_, target)) = namespace
                        .obligations
                        .iter()
                        .rev()
                        .find(|(source, _)| source == original)
                    {
                        rows.source_checked_obligations.push(*target);
                    }
                }
            }
            machine.loan_completions.sort_by(|left, right| {
                (left.operation, left.place, &left.path).cmp(&(
                    right.operation,
                    right.place,
                    &right.path,
                ))
            });
            rows.source_checked_obligations
                .extend(source_checked.into_iter().filter(|original| {
                    !instances.iter().any(|instance| {
                        instance
                            .namespace
                            .obligations
                            .iter()
                            .any(|(source, _)| source == original)
                    })
                }));
            rows.source_checked_obligations.sort();
            rows.source_checked_obligations.dedup();

            for position in 0..instances.len() {
                let original_block =
                    topology.continuations[instances[position].continuation].original_block;
                for original in invariants.iter().filter(|invariant| {
                    invariant.machine == machine.id
                        && invariant.header == machine.blocks[original_block].id
                }) {
                    let mut invariant = original.clone();
                    invariant.arrivals.clear();
                    instances[position]
                        .namespace
                        .block_invariant(&mut invariant)?;
                    for predecessor in instances.iter_mut() {
                        for (edge, successor) in &predecessor.successors {
                            if *successor != position {
                                continue;
                            }
                            let obligation = if let Some(arrival) = original
                                .arrivals
                                .iter()
                                .find(|arrival| arrival.edge == *edge)
                            {
                                predecessor.namespace.obligation(arrival.obligation)
                            } else {
                                obligation_id(allocate_dense(counters.next_obligation)?)
                            };
                            invariant
                                .arrivals
                                .push(terminal_psi::ScalarBlockInvariantArrival {
                                    edge: predecessor.namespace.edge(*edge),
                                    obligation,
                                });
                        }
                    }
                    if position == 0 {
                        invariant.arrivals.extend(
                            original
                                .arrivals
                                .iter()
                                .filter(|arrival| {
                                    !machine.blocks.iter().any(|block| {
                                        block
                                            .terminator
                                            .successor_edges()
                                            .iter()
                                            .any(|(edge, _)| *edge == arrival.edge)
                                    })
                                })
                                .cloned(),
                        );
                    }
                    invariant.arrivals.sort_by_key(|arrival| arrival.edge);
                    rows.scalar_block_invariants.push(invariant);
                }
            }
            rows.scalar_block_invariants.extend(
                invariants
                    .into_iter()
                    .filter(|invariant| invariant.machine != machine.id),
            );
            rows.scalar_block_invariants
                .sort_by_key(|invariant| (invariant.machine, invariant.header));

            for position in 0..instances.len() {
                let original_block =
                    topology.continuations[instances[position].continuation].original_block;
                let original_edges = machine.blocks[original_block].terminator.successor_edges();
                for original in coercions
                    .iter()
                    .filter(|row| original_edges.iter().any(|(edge, _)| *edge == row.edge))
                {
                    let Some((_, successor)) = instances[position]
                        .successors
                        .iter()
                        .find(|(edge, _)| *edge == original.edge)
                    else {
                        return unsupported("qualification arrival lost its selected continuation");
                    };
                    let successor = *successor;
                    let source_target = original_edges
                        .iter()
                        .find(|(edge, _)| *edge == original.edge)
                        .unwrap()
                        .1;
                    let source_block =
                        machine
                            .block(&source_target)
                            .ok_or(LoweringError::Unsupported(
                                "qualification arrival has no original target",
                            ))?;
                    let ordinal = source_block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == original.destination)
                        .ok_or(LoweringError::Unsupported(
                            "qualification destination is not a target parameter",
                        ))?;
                    let destination_original =
                        topology.continuations[instances[successor].continuation].original_block;
                    let destination = machine.blocks[destination_original]
                        .parameters
                        .get(ordinal)
                        .ok_or(LoweringError::Unsupported(
                            "qualification bypass lost its parameter",
                        ))?
                        .id;
                    let mut row = *original;
                    row.edge = instances[position].namespace.edge(row.edge);
                    row.source = instances[position].namespace.value(row.source);
                    row.destination = instances[successor].namespace.value(destination);
                    rows.qualification_coercions.push(row);
                }
            }
            rows.qualification_coercions.extend(
                coercions
                    .into_iter()
                    .filter(|row| row.machine != machine.id),
            );
            rows.qualification_coercions
                .sort_by_key(|row| (row.machine, row.edge, row.argument_ordinal));
            Ok(())
        }
    }
    pub(crate) mod dependencies {

        use super::{CompanionRows, IdentityMap, IdentityUses, LoweringError, Specialization};
        use lowered_psi::LoweredSourceCallSite;
        use semantic_vocabulary::{ClaimId, EdgeId, ObligationId, OperationId, PlaceId, ValueId};
        use terminal_psi::{Block, OperationKind, OperationResult, TerminalMachine};

        // Ordinals refer to one immutable, sorted roster of complete typed IDs.
        // Store only nonempty 64-bit words; no identity value is approximated.
        #[derive(Clone, Default, PartialEq, Eq)]
        struct InputWords(Vec<(usize, u64)>);

        impl InputWords {
            fn insert(&mut self, ordinal: usize) {
                let word = ordinal / 64;
                let bit = 1_u64 << (ordinal % 64);
                match self
                    .0
                    .binary_search_by_key(&word, |(position, _)| *position)
                {
                    Ok(position) => self.0[position].1 |= bit,
                    Err(position) => self.0.insert(position, (word, bit)),
                }
            }

            fn contains(&self, ordinal: usize) -> bool {
                self.0
                    .binary_search_by_key(&(ordinal / 64), |(position, _)| *position)
                    .is_ok_and(|position| self.0[position].1 & (1_u64 << (ordinal % 64)) != 0)
            }

            fn remove(&mut self, ordinal: usize) {
                if let Ok(position) = self
                    .0
                    .binary_search_by_key(&(ordinal / 64), |(position, _)| *position)
                {
                    self.0[position].1 &= !(1_u64 << (ordinal % 64));
                    if self.0[position].1 == 0 {
                        self.0.remove(position);
                    }
                }
            }

            fn union(&mut self, other: &Self) {
                if other.0.is_empty() {
                    return;
                }
                if self.0.is_empty() {
                    self.0.clone_from(&other.0);
                    return;
                }
                let mut combined = Vec::with_capacity(self.0.len() + other.0.len());
                let mut left_position = 0;
                let mut right_position = 0;
                while left_position < self.0.len() && right_position < other.0.len() {
                    let left = self.0[left_position];
                    let right = other.0[right_position];
                    match left.0.cmp(&right.0) {
                        std::cmp::Ordering::Less => {
                            combined.push(left);
                            left_position += 1;
                        }
                        std::cmp::Ordering::Greater => {
                            combined.push(right);
                            right_position += 1;
                        }
                        std::cmp::Ordering::Equal => {
                            combined.push((left.0, left.1 | right.1));
                            left_position += 1;
                            right_position += 1;
                        }
                    }
                }
                combined.extend_from_slice(&self.0[left_position..]);
                combined.extend_from_slice(&other.0[right_position..]);
                self.0 = combined;
            }

            fn subtract(&mut self, other: &Self) {
                self.0.retain_mut(|(word, bits)| {
                    if let Ok(position) = other
                        .0
                        .binary_search_by_key(word, |(position, _)| *position)
                    {
                        *bits &= !other.0[position].1;
                    }
                    *bits != 0
                });
            }

            fn iter(&self) -> impl Iterator<Item = usize> + '_ {
                self.0.iter().flat_map(|(word, bits)| {
                    let word = *word;
                    let mut remaining = *bits;
                    std::iter::from_fn(move || {
                        if remaining == 0 {
                            return None;
                        }
                        let bit = remaining.trailing_zeros() as usize;
                        remaining &= remaining - 1;
                        Some(word * 64 + bit)
                    })
                })
            }
        }

        fn encode<T: Ord>(identities: &[T], roster: &[T]) -> Result<InputWords, LoweringError> {
            let mut words = InputWords::default();
            for identity in identities {
                words.insert(roster.binary_search(identity).map_err(|_| {
                    LoweringError::MissingDependencyRosterEntry {
                        identity_kind: std::any::type_name::<T>(),
                        role: "input or definition",
                    }
                })?);
            }
            Ok(words)
        }

        #[derive(Clone, Default, PartialEq, Eq)]
        struct PackedInputs {
            values: InputWords,
            places: InputWords,
            operations: InputWords,
            claims: InputWords,
            obligations: InputWords,
        }

        impl PackedInputs {
            fn uses(uses: &IdentityUses, roster: &IdentityUses) -> Result<Self, LoweringError> {
                Ok(Self {
                    values: encode(&uses.values, &roster.values)?,
                    places: encode(&uses.places, &roster.places)?,
                    operations: encode(&uses.operations, &roster.operations)?,
                    claims: encode(&uses.claims, &roster.claims)?,
                    obligations: encode(&uses.obligations, &roster.obligations)?,
                })
            }

            fn definitions(
                definitions: &Definitions,
                roster: &IdentityUses,
            ) -> Result<Self, LoweringError> {
                Ok(Self {
                    values: encode(&definitions.values, &roster.values)?,
                    places: encode(&definitions.places, &roster.places)?,
                    operations: encode(&definitions.operations, &roster.operations)?,
                    claims: encode(&definitions.claims, &roster.claims)?,
                    obligations: encode(&definitions.obligations, &roster.obligations)?,
                })
            }

            fn remove_definitions(&mut self, definitions: &Self) {
                self.values.subtract(&definitions.values);
                self.places.subtract(&definitions.places);
                self.operations.subtract(&definitions.operations);
                self.claims.subtract(&definitions.claims);
                self.obligations.subtract(&definitions.obligations);
            }
        }

        pub(crate) struct LiveInputs {
            roster: IdentityUses,
            continuations: Vec<PackedInputs>,
        }

        impl LiveInputs {
            pub(crate) fn values(&self, continuation: usize) -> impl Iterator<Item = ValueId> + '_ {
                self.continuations[continuation]
                    .values
                    .iter()
                    .map(|ordinal| self.roster.values[ordinal])
            }
            pub(crate) fn places(&self, continuation: usize) -> impl Iterator<Item = PlaceId> + '_ {
                self.continuations[continuation]
                    .places
                    .iter()
                    .map(|ordinal| self.roster.places[ordinal])
            }
            pub(crate) fn operations(
                &self,
                continuation: usize,
            ) -> impl Iterator<Item = OperationId> + '_ {
                self.continuations[continuation]
                    .operations
                    .iter()
                    .map(|ordinal| self.roster.operations[ordinal])
            }
            pub(crate) fn claims(&self, continuation: usize) -> impl Iterator<Item = ClaimId> + '_ {
                self.continuations[continuation]
                    .claims
                    .iter()
                    .map(|ordinal| self.roster.claims[ordinal])
            }
            pub(crate) fn obligations(
                &self,
                continuation: usize,
            ) -> impl Iterator<Item = ObligationId> + '_ {
                self.continuations[continuation]
                    .obligations
                    .iter()
                    .map(|ordinal| self.roster.obligations[ordinal])
            }
        }

        pub(crate) struct Definitions {
            pub(crate) values: Vec<ValueId>,
            pub(crate) places: Vec<PlaceId>,
            pub(crate) operations: Vec<OperationId>,
            pub(crate) edges: Vec<EdgeId>,
            pub(crate) obligations: Vec<ObligationId>,
            pub(crate) claims: Vec<ClaimId>,
        }

        impl Definitions {
            pub(crate) fn for_block(
                machine: &TerminalMachine,
                block: &Block,
                rows: &CompanionRows<'_>,
            ) -> Result<Self, LoweringError> {
                let mut definitions = Self {
                    values: block
                        .parameters
                        .iter()
                        .chain(&block.erased_scalar_formals)
                        .map(|parameter| parameter.id)
                        .collect(),
                    places: block
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.place)
                        .collect(),
                    operations: block
                        .operations
                        .iter()
                        .map(|operation| operation.id)
                        .collect(),
                    edges: block.terminator.edges().collect(),
                    obligations: Vec::new(),
                    claims: machine
                        .block_entry_claims
                        .iter()
                        .filter(|claim| {
                            block
                                .structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == claim.input)
                        })
                        .map(|claim| claim.claim)
                        .collect(),
                };
                for operation in &block.operations {
                    match &operation.result {
                        OperationResult::Unit => {}
                        OperationResult::Scalar(value) => definitions.values.push(value.id),
                        OperationResult::Structural(result) => {
                            definitions.places.push(result.place)
                        }
                    }
                    match &operation.kind {
                        OperationKind::EstablishByteSequenceLiteral { destination, .. }
                        | OperationKind::EstablishElementView { destination, .. }
                        | OperationKind::EstablishMutableElementView { destination, .. }
                        | OperationKind::EstablishTrivialAffineLocal { destination } => {
                            definitions.places.push(*destination);
                        }
                        _ => {}
                    }
                }
                definitions.claims.extend(
                    rows.claim_introductions
                        .iter()
                        .filter(|introduction| {
                            definitions.operations.contains(&introduction.operation)
                        })
                        .map(|introduction| introduction.claim),
                );
                let mut namespace = observer();
                namespace.block(&mut block.clone())?;
                definitions.obligations = namespace
                    .observed
                    .take()
                    .expect("dependency observer")
                    .obligations;
                definitions.obligations.extend(
                    rows.scalar_block_invariants
                        .iter()
                        .flat_map(|invariant| &invariant.arrivals)
                        .filter(|arrival| definitions.edges.contains(&arrival.edge))
                        .map(|arrival| arrival.obligation),
                );
                normalize(&mut definitions.values);
                normalize(&mut definitions.places);
                normalize(&mut definitions.operations);
                normalize(&mut definitions.edges);
                normalize(&mut definitions.obligations);
                normalize(&mut definitions.claims);
                Ok(definitions)
            }

            fn remove_from(&self, uses: &mut IdentityUses) {
                uses.values
                    .retain(|identity| !self.values.contains(identity));
                uses.places
                    .retain(|identity| !self.places.contains(identity));
                uses.operations
                    .retain(|identity| !self.operations.contains(identity));
                uses.claims
                    .retain(|identity| !self.claims.contains(identity));
                uses.obligations
                    .retain(|identity| !self.obligations.contains(identity));
            }
        }

        pub(crate) fn live_inputs(
            machine: &TerminalMachine,
            topology: &Specialization,
            definitions: &[Definitions],
            rows: &CompanionRows<'_>,
        ) -> Result<LiveInputs, LoweringError> {
            let observed = observe_local_inputs(machine, definitions, rows)?;
            let roster = input_roster(&observed, definitions, rows.claim_edge_bindings);
            let local = observed
                .iter()
                .map(|uses| PackedInputs::uses(uses, &roster))
                .collect::<Result<Vec<_>, _>>()?;
            drop(observed);
            let defined = definitions
                .iter()
                .map(|defined| PackedInputs::definitions(defined, &roster))
                .collect::<Result<Vec<_>, _>>()?;
            let continuations = solve_live_inputs(
                topology,
                &local,
                &defined,
                &roster,
                rows.claim_edge_bindings,
            )?;
            Ok(LiveInputs {
                roster,
                continuations,
            })
        }

        fn observe_local_inputs(
            machine: &TerminalMachine,
            definitions: &[Definitions],
            rows: &CompanionRows<'_>,
        ) -> Result<Vec<IdentityUses>, LoweringError> {
            let completions = super::super::selected_continuations::CleanupCompletionIndex::new(
                rows.completed_cleanup_frontiers,
            )?;
            let local =
                machine
                    .blocks
                    .iter()
                    .zip(definitions)
                    .map(|(block, definitions)| {
                        let mut namespace = observer();
                        namespace.block(&mut block.clone())?;
                        for edge in &definitions.edges {
                            for completion in completions.at(*edge) {
                                namespace.argument(&mut completion.region.clone());
                            }
                        }
                        for call in
                            rows.source_calls
                                .iter()
                                .filter(|call| match call.terminal_site {
                                    LoweredSourceCallSite::Operation(operation) => {
                                        definitions.operations.contains(&operation)
                                    }
                                    LoweredSourceCallSite::InvokeNoNormal(edge)
                                    | LoweredSourceCallSite::ExternalCompletion(edge) => {
                                        definitions.edges.contains(&edge)
                                    }
                                })
                        {
                            namespace.source_call(&mut call.clone());
                        }
                        for completion in machine.loan_completions.iter().filter(|completion| {
                            definitions.operations.contains(&completion.operation)
                        }) {
                            namespace.loan_completion(&mut completion.clone());
                        }
                        for invariant in rows
                            .scalar_block_invariants
                            .iter()
                            .filter(|invariant| invariant.header == block.id)
                        {
                            let mut invariant = invariant.clone();
                            invariant.arrivals.clear();
                            namespace.block_invariant(&mut invariant)?;
                        }
                        for coercion in rows
                            .qualification_coercions
                            .iter()
                            .filter(|coercion| definitions.edges.contains(&coercion.edge))
                        {
                            namespace.value(coercion.source);
                        }
                        for (_, value) in rows
                            .continuation_value_uses
                            .iter()
                            .filter(|(owner, _)| *owner == block.id)
                        {
                            namespace.value(*value);
                        }
                        for claim in machine.block_entry_claims.iter().filter(|claim| {
                            block
                                .structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == claim.input)
                        }) {
                            namespace.path(&mut claim.path.clone());
                        }
                        let mut uses = namespace.observed.take().expect("dependency observer");
                        definitions.remove_from(&mut uses);
                        normalize_uses(&mut uses);
                        Ok(uses)
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok(local)
        }

        fn input_roster(
            local: &[IdentityUses],
            definitions: &[Definitions],
            bindings: &[crate::emission::claim_bindings::ClaimEdgeBinding],
        ) -> IdentityUses {
            let mut roster = IdentityUses::default();
            for uses in local {
                roster.values.extend_from_slice(&uses.values);
                roster.places.extend_from_slice(&uses.places);
                roster.operations.extend_from_slice(&uses.operations);
                roster.claims.extend_from_slice(&uses.claims);
                roster.obligations.extend_from_slice(&uses.obligations);
            }
            for defined in definitions {
                roster.values.extend_from_slice(&defined.values);
                roster.places.extend_from_slice(&defined.places);
                roster.operations.extend_from_slice(&defined.operations);
                roster.claims.extend_from_slice(&defined.claims);
                roster.obligations.extend_from_slice(&defined.obligations);
            }
            // A substitution source can be absent from every local observation.
            // Collect it without validating or demanding the binding.
            for binding in bindings {
                roster.claims.extend([binding.destination, binding.source]);
            }
            normalize_uses(&mut roster);
            roster
        }

        fn solve_live_inputs(
            topology: &Specialization,
            local: &[PackedInputs],
            definitions: &[PackedInputs],
            roster: &IdentityUses,
            bindings: &[crate::emission::claim_bindings::ClaimEdgeBinding],
        ) -> Result<Vec<PackedInputs>, LoweringError> {
            let mut bindings = bindings.iter().collect::<Vec<_>>();
            bindings.sort_by_key(|binding| (binding.edge, binding.destination));
            let mut live = topology
                .continuations
                .iter()
                .map(|continuation| local[continuation.original_block].clone())
                .collect::<Vec<_>>();
            let mut predecessors = vec![Vec::new(); topology.continuations.len()];
            for (position, continuation) in topology.continuations.iter().enumerate() {
                for (_, successor) in &continuation.successors {
                    predecessors[*successor].push(position);
                }
            }
            for sources in &mut predecessors {
                sources.sort_unstable();
                sources.dedup();
            }
            let mut current = vec![true; live.len()];
            let mut next = vec![false; live.len()];
            loop {
                let mut deferred = false;
                for position in (0..topology.continuations.len()).rev() {
                    if !std::mem::replace(&mut current[position], false) {
                        continue;
                    }
                    let continuation = &topology.continuations[position];
                    let mut incoming = local[continuation.original_block].clone();
                    for (edge, successor) in &continuation.successors {
                        let source = &live[*successor];
                        incoming.values.union(&source.values);
                        incoming.places.union(&source.places);
                        incoming.operations.union(&source.operations);
                        incoming.obligations.union(&source.obligations);
                        let first = bindings.partition_point(|binding| binding.edge < *edge);
                        let last = bindings.partition_point(|binding| binding.edge <= *edge);
                        let mut remaining = &bindings[first..last];
                        if remaining.is_empty() {
                            incoming.claims.union(&source.claims);
                            continue;
                        }
                        let mut translated = source.claims.clone();
                        let mut receiving = InputWords::default();
                        while let Some(binding) = remaining.first() {
                            let count = remaining
                                .iter()
                                .take_while(|candidate| {
                                    candidate.destination == binding.destination
                                })
                                .count();
                            let destination = roster
                                .claims
                                .binary_search(&binding.destination)
                                .map_err(|_| LoweringError::MissingDependencyRosterEntry {
                                    identity_kind: "claim",
                                    role: "edge binding destination",
                                })?;
                            if source.claims.contains(destination) {
                                if count != 1 {
                                    return Err(LoweringError::Unsupported(
                                        "continuation edge repeats a claim binding",
                                    ));
                                }
                                translated.remove(destination);
                                receiving.insert(
                                    roster.claims.binary_search(&binding.source).map_err(|_| {
                                        LoweringError::MissingDependencyRosterEntry {
                                            identity_kind: "claim",
                                            role: "edge binding source",
                                        }
                                    })?,
                                );
                            }
                            remaining = &remaining[count..];
                        }
                        // Resolve every source from the unmodified successor set.
                        // Appending it after all destination removals is simultaneous,
                        // so one binding never chases another binding on this edge.
                        translated.union(&receiving);
                        incoming.claims.union(&translated);
                    }
                    incoming.remove_definitions(&definitions[continuation.original_block]);
                    if live[position] != incoming {
                        live[position] = incoming;
                        for predecessor in &predecessors[position] {
                            if *predecessor < position {
                                current[*predecessor] = true;
                            } else {
                                next[*predecessor] = true;
                                deferred = true;
                            }
                        }
                    }
                }
                if !deferred {
                    return Ok(live);
                }
                // Preserve the old reverse-sweep order. A self-loop or an already
                // visited predecessor remains pending for the following sweep.
                std::mem::swap(&mut current, &mut next);
            }
        }

        fn observer() -> IdentityMap {
            IdentityMap {
                observed: Some(IdentityUses::default()),
                ..IdentityMap::default()
            }
        }

        fn normalize<T: Ord>(identities: &mut Vec<T>) {
            identities.sort_unstable();
            identities.dedup();
        }

        fn normalize_uses(uses: &mut IdentityUses) {
            normalize(&mut uses.values);
            normalize(&mut uses.places);
            normalize(&mut uses.operations);
            normalize(&mut uses.claims);
            normalize(&mut uses.obligations);
        }

        #[cfg(test)]
        mod input_word_tests {
            use super::InputWords;
            use std::collections::BTreeSet;

            #[test]
            fn sparse_words_match_ordered_set_algebra_without_expanded_rows() {
                let left = [8191, 64, 0, 4095, 63, 64];
                let right = [8191, 1, 4096, 65];
                let mut words = InputWords::default();
                for ordinal in left {
                    words.insert(ordinal);
                }
                let mut other = InputWords::default();
                for ordinal in right {
                    other.insert(ordinal);
                }
                let mut expected = left.into_iter().chain(right).collect::<BTreeSet<_>>();
                words.union(&other);
                assert_eq!(
                    words.iter().collect::<Vec<_>>(),
                    expected.iter().copied().collect::<Vec<_>>()
                );
                words.remove(63);
                expected.remove(&63);
                words.subtract(&other);
                expected.retain(|ordinal| !right.contains(ordinal));
                assert_eq!(
                    words.iter().collect::<Vec<_>>(),
                    expected.iter().copied().collect::<Vec<_>>()
                );
                assert!(words.contains(64) && !words.contains(8191));
                assert!(words.0.iter().all(|(_, bits)| *bits != 0));
                assert!(words.0.windows(2).all(|pair| pair[0].0 < pair[1].0));

                let mut wide = InputWords::default();
                for ordinal in (0..8192).rev() {
                    wide.insert(ordinal);
                }
                assert!(wide.iter().eq(0..8192));
                assert_eq!(wide.0.len(), 128);
                let mut first_word = InputWords::default();
                for ordinal in 0..64 {
                    first_word.insert(ordinal);
                }
                wide.subtract(&first_word);
                assert!(wide.iter().eq(64..8192));
                assert_eq!(wide.0.len(), 127);
                eprintln!(
                    "8192 ordered identity ordinals use 128 nonzero words; union, removal, subtraction and ascending iteration match an independent ordered-set oracle"
                );
            }
        }
    }
    use dependencies::Definitions;

    pub(crate) struct CompanionRows<'a> {
        pub(crate) completed_cleanup_frontiers:
            &'a mut Vec<super::selected_continuations::CompletedCleanupFrontier>,
        pub(crate) source_returns:
            &'a mut Vec<crate::expression_preparation::qualifications::SourceScalarReturn>,
        pub(crate) continuation_value_uses: &'a [(BlockId, semantic_vocabulary::ValueId)],
        pub(crate) source_calls: &'a mut Vec<LoweredSourceCallOccurrence>,
        pub(crate) scalar_block_invariants: &'a mut Vec<ScalarBlockInvariant>,
        pub(crate) qualification_coercions: &'a mut Vec<ScalarQualificationCoercion>,
        pub(crate) selected_comparisons: &'a mut Vec<LoweredSelectedComparisonOccurrence>,
        pub(crate) source_checked_obligations: &'a mut Vec<ObligationId>,
        pub(crate) claim_introductions: &'a [super::claim_bindings::ClaimIntroduction],
        pub(crate) claim_edge_bindings: &'a [super::claim_bindings::ClaimEdgeBinding],
    }

    pub(crate) struct Counters<'a> {
        pub(crate) next_place: &'a mut u64,
        pub(crate) next_value: &'a mut u64,
        pub(crate) next_block: &'a mut u64,
        pub(crate) next_edge: &'a mut u64,
        pub(crate) next_operation: &'a mut u64,
        pub(crate) next_obligation: &'a mut u64,
    }

    pub(crate) struct Instance {
        continuation: usize,
        incoming: IdentityMap,
        namespace: IdentityMap,
        successors: Vec<(EdgeId, usize)>,
    }

    pub(crate) fn complete(
        machine: &mut TerminalMachine,
        groups: &[SelectedCleanupGroup],
        types: &[StructuralTypeDeclaration],
        mut rows: CompanionRows<'_>,
        mut counters: Counters<'_>,
    ) -> Result<Vec<IdentityMap>, LoweringError> {
        let Some(topology) = super::selected_continuations::prepare(
            machine.id,
            machine.entry,
            &machine.structural_parameters,
            &machine.structural_places,
            &machine.blocks,
            types,
            groups,
            rows.completed_cleanup_frontiers,
            !rows.claim_edge_bindings.is_empty(),
        )?
        else {
            return Ok(Vec::new());
        };
        if groups.is_empty() && rows.claim_edge_bindings.is_empty() {
            let mut retained = vec![false; machine.blocks.len()];
            if topology.continuations.iter().all(|continuation| {
                !std::mem::replace(&mut retained[continuation.original_block], true)
            }) {
                return Ok(Vec::new());
            }
        }
        let definitions = machine
            .blocks
            .iter()
            .map(|block| Definitions::for_block(machine, block, &rows))
            .collect::<Result<Vec<_>, _>>()?;
        let live = dependencies::live_inputs(machine, &topology, &definitions, &rows)?;
        let mut next_claim = machine
            .entry_claims
            .iter()
            .chain(&machine.block_entry_claims)
            .map(|row| row.claim.get())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.structural())
                    .flat_map(|result| result.claims.iter().map(|row| row.claim.get())),
            )
            .chain(
                machine
                    .loan_completions
                    .iter()
                    .flat_map(|completion| completion.claims.iter().map(|row| row.claim.get())),
            )
            .chain(rows.claim_introductions.iter().map(|row| row.claim.get()))
            .chain(
                rows.claim_edge_bindings
                    .iter()
                    .flat_map(|row| [row.destination.get(), row.source.get()]),
            )
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "continuation claim namespace exceeds u64",
            ))?;
        let mut instantiated = vec![false; machine.blocks.len()];
        let incoming = captured_namespace(
            &mut IdentityMap::default(),
            live.values(topology.entry),
            live.places(topology.entry),
            live.operations(topology.entry),
            live.claims(topology.entry),
            live.obligations(topology.entry),
        );
        let mut instances = vec![new_instance(
            topology.entry,
            incoming,
            machine,
            &topology,
            &definitions,
            &mut instantiated,
            &mut counters,
            &mut next_claim,
        )?];
        let mut position = 0;
        while position < instances.len() {
            let continuation = instances[position].continuation;
            let mut namespace = instances[position].namespace.clone();
            let mut successors = Vec::new();
            for (edge, target) in &topology.continuations[continuation].successors {
                let substitutions = rows
                    .claim_edge_bindings
                    .iter()
                    .filter(|binding| binding.edge == *edge)
                    .map(|binding| (binding.destination, namespace.claim(binding.source)))
                    .collect::<Vec<_>>();
                let mut incoming_namespace = namespace.clone();
                for (destination, source) in substitutions {
                    incoming_namespace
                        .claims
                        .retain(|(name, _)| *name != destination);
                    incoming_namespace.claims.push((destination, source));
                }
                let incoming = captured_namespace(
                    &mut incoming_namespace,
                    live.values(*target),
                    live.places(*target),
                    live.operations(*target),
                    live.claims(*target),
                    live.obligations(*target),
                );
                let target_instance = if let Some(existing) =
                    instances.iter().position(|instance| {
                        instance.continuation == *target && instance.incoming == incoming
                    }) {
                    existing
                } else {
                    let target_instance = instances.len();
                    instances.push(new_instance(
                        *target,
                        incoming,
                        machine,
                        &topology,
                        &definitions,
                        &mut instantiated,
                        &mut counters,
                        &mut next_claim,
                    )?);
                    target_instance
                };
                successors.push((*edge, target_instance));
            }
            instances[position].successors = successors;
            position += 1;
        }
        for position in 0..instances.len() {
            instances[position].namespace.edge_targets = instances[position]
                .successors
                .iter()
                .map(|(edge, target)| (*edge, instances[*target].namespace.blocks[0].1))
                .collect();
        }
        let mut blocks = Vec::with_capacity(instances.len());
        for instance in &mut instances {
            let original = topology.continuations[instance.continuation].original_block;
            let mut block = machine.blocks[original].clone();
            instance.namespace.block(&mut block)?;
            blocks.push(block);
        }
        companions::instantiate(machine, &topology, &mut instances, &mut rows, &mut counters)?;
        let mut places = machine
            .structural_places
            .iter()
            .filter(|place| {
                !definitions
                    .iter()
                    .any(|definition| definition.places.contains(&place.id))
            })
            .copied()
            .collect::<Vec<_>>();
        let mut block_entry_claims = Vec::new();
        for instance in &mut instances {
            let original = topology.continuations[instance.continuation].original_block;
            for source in &definitions[original].places {
                let mut declaration = machine
                    .structural_places
                    .iter()
                    .find(|place| place.id == *source)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "continuation definition has no structural declaration",
                    ))?;
                instance.namespace.place_declaration(&mut declaration);
                places.push(declaration);
            }
            for claim in machine.block_entry_claims.iter().filter(|claim| {
                machine.blocks[original]
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == claim.input)
            }) {
                let mut claim = claim.clone();
                claim.input = instance.namespace.place(claim.input);
                claim.claim = instance.namespace.claim(claim.claim);
                instance.namespace.path(&mut claim.path);
                block_entry_claims.push(claim);
            }
        }
        let originals = std::mem::take(rows.completed_cleanup_frontiers);
        let completions = super::selected_continuations::CleanupCompletionIndex::new(&originals)?;
        for instance in &mut instances {
            for (original, _) in instance.namespace.edges.clone() {
                for completion in completions.at(original) {
                    let mut completion = completion.clone();
                    completion.edge = instance.namespace.edge(original);
                    instance.namespace.argument(&mut completion.region);
                    rows.completed_cleanup_frontiers.push(completion);
                }
            }
        }
        places.sort_by_key(|place| place.id);
        let mut literal_ordinal = 0;
        for place in &mut places {
            if let StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } = &mut place.kind
            {
                *declaration_ordinal = literal_ordinal;
                literal_ordinal += 1;
            }
        }
        block_entry_claims.sort_by_key(|claim| claim.claim);
        blocks.sort_by_key(|block| block.id);
        machine.entry = instances[0].namespace.blocks[0].1;
        machine.blocks = blocks;
        machine.structural_places = places;
        machine.block_entry_claims = block_entry_claims;
        Ok(instances
            .into_iter()
            .map(|instance| instance.namespace)
            .collect())
    }

    fn captured_namespace(
        namespace: &mut IdentityMap,
        values: impl IntoIterator<Item = ValueId>,
        places: impl IntoIterator<Item = PlaceId>,
        operations: impl IntoIterator<Item = OperationId>,
        claims: impl IntoIterator<Item = ClaimId>,
        obligations: impl IntoIterator<Item = ObligationId>,
    ) -> IdentityMap {
        IdentityMap {
            values: values
                .into_iter()
                .map(|original| (original, namespace.value(original)))
                .collect(),
            places: places
                .into_iter()
                .map(|original| (original, namespace.place(original)))
                .collect(),
            inherited_operations: operations
                .into_iter()
                .map(|original| (original, namespace.operation(original)))
                .collect(),
            // Missing claim mappings mean unchanged. Observe every claim before
            // filtering so the exact dependency read is retained.
            claims: claims
                .into_iter()
                .map(|original| (original, namespace.claim(original)))
                .filter(|(original, mapped)| original != mapped)
                .collect(),
            obligations: obligations
                .into_iter()
                .map(|original| (original, namespace.obligation(original)))
                .collect(),
            ..IdentityMap::default()
        }
    }

    fn new_instance(
        continuation: usize,
        incoming: IdentityMap,
        machine: &TerminalMachine,
        topology: &Specialization,
        definitions: &[Definitions],
        instantiated: &mut [bool],
        counters: &mut Counters<'_>,
        next_claim: &mut u64,
    ) -> Result<Instance, LoweringError> {
        let original = topology.continuations[continuation].original_block;
        let block = &machine.blocks[original];
        let duplicate = std::mem::replace(&mut instantiated[original], true);
        let mut namespace = incoming.clone();
        namespace.blocks.push((
            block.id,
            if duplicate {
                block_id(allocate_dense(counters.next_block)?)
            } else {
                block.id
            },
        ));
        let definitions = &definitions[original];
        for original in &definitions.values {
            namespace.values.push((
                *original,
                if duplicate {
                    value_id(allocate_dense(counters.next_value)?)
                } else {
                    *original
                },
            ));
        }
        for original in &definitions.places {
            namespace.places.push((
                *original,
                if duplicate {
                    place_id(allocate_dense(counters.next_place)?)
                } else {
                    *original
                },
            ));
        }
        for original in &definitions.operations {
            namespace.operations.push((
                *original,
                if duplicate {
                    operation_id(allocate_dense(counters.next_operation)?)
                } else {
                    *original
                },
            ));
        }
        for original in &definitions.edges {
            namespace.edges.push((
                *original,
                if duplicate {
                    edge_id(allocate_dense(counters.next_edge)?)
                } else {
                    *original
                },
            ));
        }
        for original in &definitions.obligations {
            namespace.obligations.push((
                *original,
                if duplicate {
                    obligation_id(allocate_dense(counters.next_obligation)?)
                } else {
                    *original
                },
            ));
        }
        for original in &definitions.claims {
            namespace.claims.push((
                *original,
                if duplicate {
                    claim_id(allocate_dense(next_claim)?)
                } else {
                    *original
                },
            ));
        }
        Ok(Instance {
            continuation,
            incoming,
            namespace,
            successors: Vec::new(),
        })
    }

    #[cfg(test)]
    mod captured_claim_tests {
        use super::*;
        use semantic_vocabulary::ClaimId;

        fn capture_all_claim_pairs(
            namespace: &mut IdentityMap,
            live: &IdentityUses,
        ) -> IdentityMap {
            IdentityMap {
                claims: live
                    .claims
                    .iter()
                    .map(|original| (*original, namespace.claim(*original)))
                    .collect(),
                ..IdentityMap::default()
            }
        }

        fn captured_namespace(namespace: &mut IdentityMap, live: &IdentityUses) -> IdentityMap {
            super::captured_namespace(
                namespace,
                live.values.iter().copied(),
                live.places.iter().copied(),
                live.operations.iter().copied(),
                live.claims.iter().copied(),
                live.obligations.iter().copied(),
            )
        }

        #[test]
        fn captured_claims_keep_renames_and_observations_without_identity_rows() {
            let id = |value| ClaimId::new(value).unwrap();
            let live = IdentityUses {
                claims: vec![id(3), id(1), id(2)],
                ..IdentityUses::default()
            };
            let source = IdentityMap {
                // Last-match identity overrides the earlier rename for claim 1.
                claims: vec![
                    (id(1), id(8)),
                    (id(1), id(1)),
                    (id(2), id(6)),
                    (id(3), id(7)),
                ],
                ..IdentityMap::default()
            };
            let mut different = source.clone();
            different.claims.push((id(2), id(9)));
            let mut outside = source.clone();
            outside.claims.push((id(99), id(9)));
            let sources = [IdentityMap::default(), source.clone(), different, outside];
            let mut full = Vec::new();
            let mut sparse = Vec::new();
            for mut namespace in sources {
                full.push(capture_all_claim_pairs(&mut namespace.clone(), &live));
                sparse.push(captured_namespace(&mut namespace, &live));
            }
            assert_eq!(sparse[1].claims, vec![(id(3), id(7)), (id(2), id(6))]);
            for left_position in 0..full.len() {
                for right_position in 0..full.len() {
                    // Comparison is within one fixed continuation/live roster.
                    assert_eq!(
                        full[left_position] == full[right_position],
                        sparse[left_position] == sparse[right_position],
                    );
                }
                for original in [id(1), id(2), id(3), id(99)] {
                    assert_eq!(
                        full[left_position].claim(original),
                        sparse[left_position].claim(original),
                    );
                }
            }
            // Edge substitutions may explicitly restore identity after a rename.
            // Capture must resolve that final substitution before omitting it.
            let mut full_reset = full[1].clone();
            let mut sparse_reset = sparse[1].clone();
            for namespace in [&mut full_reset, &mut sparse_reset] {
                namespace.claims.retain(|(original, _)| *original != id(2));
                namespace.claims.push((id(2), id(2)));
            }
            let mut reset = captured_namespace(&mut sparse_reset, &live);
            let mut full_reset = capture_all_claim_pairs(&mut full_reset, &live);
            assert_eq!(reset.claims, vec![(id(3), id(7))]);
            for original in &live.claims {
                assert_eq!(reset.claim(*original), full_reset.claim(*original));
            }
            let wide = IdentityUses {
                claims: (1..=8192).map(id).collect(),
                ..IdentityUses::default()
            };
            let mut observed = IdentityMap {
                observed: Some(IdentityUses::default()),
                ..IdentityMap::default()
            };
            let captured = captured_namespace(&mut observed, &wide);
            assert!(captured.claims.is_empty());
            assert_eq!(observed.observed.unwrap().claims, wide.claims);
            assert_eq!(
                capture_all_claim_pairs(&mut IdentityMap::default(), &wide)
                    .claims
                    .len(),
                8192
            );
            eprintln!(
                "8192 unchanged live claims retain 0 input pairs instead of 8192; ordered renames, last-match resets, lookup observations and fixed-roster comparison classes match the full-row oracle"
            );
        }
    }
}
