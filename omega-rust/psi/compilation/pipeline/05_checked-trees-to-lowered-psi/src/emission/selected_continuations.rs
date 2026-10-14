//! Retain exact owner frontiers through private, pre-publication continuations.
//!
//! Source checking supplies ordered candidate deaths, never liveness. Actual
//! structural births, owned operands and edge disposal select whether a death's
//! complete cleanup graph executes. The product graph preserves the original
//! scalar decisions, including their captured values; it does not observe source
//! conditions again or reconcile different nominal lifetimes at an early join.
//! Physical namespace allocation belongs to continuation identity instantiation.

use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Block, OperationKind, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, Terminator,
};

/// An empty jump into one source-authorized candidate death's complete graph.
/// `bypass` accepts the same captured scalar/proof frame as the jump's target.
/// Skipping the group also skips its case observations and nominal prerequisites.
#[derive(Clone, Debug)]
pub(crate) struct SelectedCleanupGroup {
    pub(crate) edge: EdgeId,
    pub(crate) region: StructuralArgument,
    pub(crate) bypass: BlockId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RegionState {
    Absent,
    Live,
    /// Exact outstanding holes. A store restores only the subtree it supplies;
    /// an unrelated write cannot turn a partial occurrence into a whole owner.
    Partial(Vec<Vec<StructuralPathSegment>>),
}

/// A source block and its exact incoming semantic occurrence frontier. Physical
/// cloned definitions are deliberately not used as the frontier's identities.
#[derive(Clone, Debug)]
pub(crate) struct Continuation {
    pub(crate) original_block: usize,
    pub(crate) incoming: Vec<RegionState>,
    /// Original edge identity and product-node destination, including bypasses.
    pub(crate) successors: Vec<(EdgeId, usize)>,
}

pub(crate) struct Specialization {
    pub(crate) entry: usize,
    pub(crate) continuations: Vec<Continuation>,
}

/// Analyze the unpublished graph without granting any cleanup authority. The
/// caller must instantiate every retained block and its proof/source companions
/// in one namespace before the ordinary Terminal verifiers see the result.
pub(crate) fn prepare(
    entry: BlockId,
    parameters: &[StructuralParameterDeclaration],
    places: &[StructuralPlaceDeclaration],
    blocks: &[Block],
    types: &[StructuralTypeDeclaration],
    groups: &[SelectedCleanupGroup],
) -> Result<Option<Specialization>, LoweringError> {
    let mut regions = Vec::new();
    for group in groups {
        if group.region.access != StructuralAccess::Owned {
            return unsupported("selected cleanup does not name an owned occurrence");
        }
        if !regions.contains(&group.region) {
            regions.push(group.region.clone());
        }
        validate_group(blocks, group)?;
    }
    let extractions = super::affine_cleanup::owned_extractions(parameters, places, blocks, types)?;
    let projected_roots = super::affine_cleanup::partitioned_roots(blocks, &extractions);
    if !projected_roots.is_empty() {
        // Preserve partitioned roots through their discriminator branches.
        // Differing case payload holes meet only after restoration or the
        // already scheduled home retirement; no inactive case is fabricated.
        for &place in &projected_roots {
            let root = StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            };
            if !regions.contains(&root) {
                regions.push(root);
            }
        }
    }
    if regions.is_empty() {
        return Ok(None);
    }
    let entry = block_position(blocks, entry)?;
    let initial = regions
        .iter()
        .map(|region| {
            if parameters.iter().any(|parameter| {
                parameter.place == region.place
                    && (parameter.access == StructuralAccess::Owned
                        || projected_roots.contains(&region.place))
            }) {
                RegionState::Live
            } else {
                RegionState::Absent
            }
        })
        .collect();
    let mut continuations = vec![Continuation {
        original_block: entry,
        incoming: initial,
        successors: Vec::new(),
    }];
    let mut position = 0;
    while position < continuations.len() {
        let block = &blocks[continuations[position].original_block];
        let mut frontier = continuations[position].incoming.clone();
        for parameter in &block.structural_parameters {
            // Projected tracking records storage availability, not permission
            // to own a borrowed referent. Only the supplied candidate groups
            // can select cleanup; every actual operand retains its access.
            if parameter.access == StructuralAccess::Owned
                || projected_roots.contains(&parameter.place)
            {
                establish(&regions, &mut frontier, parameter.place);
            }
        }
        for operation in &block.operations {
            for argument in super::affine_cleanup::structural_operands(&operation.kind) {
                consume_argument(&regions, &mut frontier, argument);
            }
            for (_, extraction) in extractions.iter().filter(|(id, _)| *id == operation.id) {
                consume_argument(&regions, &mut frontier, extraction);
            }
            if let OperationKind::ReleaseReference { source, path } = &operation.kind {
                consume(&regions, &mut frontier, *source, path);
            }
            if let OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                ..
            } = &operation.kind
                && regions.iter().any(|region| region.place == *destination)
            {
                let root = root_type(*destination, parameters, places, blocks)?;
                let restored = terminal_semantics::structural_window_projection(
                    types.iter(),
                    root,
                    path,
                    *field,
                )
                .ok_or(LoweringError::Unsupported(
                    "selected owner restoration has no exact structural projection",
                ))?;
                restore(&regions, &mut frontier, *destination, &restored.path);
            }
            if let Some(result) = operation.result.structural() {
                establish(&regions, &mut frontier, result.place);
            } else if let OperationKind::EstablishTrivialAffineLocal { destination }
            | OperationKind::EstablishByteSequenceLiteral { destination, .. }
            | OperationKind::EstablishElementView { destination, .. }
            | OperationKind::EstablishMutableElementView { destination, .. } =
                &operation.kind
            {
                establish(&regions, &mut frontier, *destination);
            }
        }
        if let Terminator::ReturnStructural { source, .. } = &block.terminator {
            consume(&regions, &mut frontier, *source, &[]);
        }
        let edges = block.terminator.successor_edges();
        let arguments = if matches!(block.terminator, Terminator::StructuralCase { .. }) {
            vec![&[][..]; edges.len()]
        } else {
            block.terminator.successor_block_arguments()
        };
        if edges.len() != arguments.len() {
            return unsupported("selected continuation lost its successor argument roster");
        }
        let mut successors = Vec::with_capacity(edges.len());
        for ((edge, original_target), arguments) in edges.into_iter().zip(arguments) {
            let mut outgoing = frontier.clone();
            for argument in arguments {
                consume_argument(&regions, &mut outgoing, argument);
            }
            let mut target = original_target;
            if let Some(group) = groups.iter().find(|group| group.edge == edge) {
                let selected = regions
                    .iter()
                    .position(|region| region == &group.region)
                    .ok_or(LoweringError::Unsupported(
                        "selected death lost its occurrence",
                    ))?;
                match outgoing[selected] {
                    RegionState::Live => {}
                    RegionState::Absent => target = group.bypass,
                    RegionState::Partial(_) => {
                        return unsupported(
                            "selected whole-owner cleanup has a partially transferred occurrence",
                        );
                    }
                }
            }
            if let Some((_, actions)) = block
                .terminator
                .cleanup_rosters()
                .find(|(id, _)| *id == edge)
            {
                for action in actions {
                    dispose(&regions, &mut outgoing, action);
                }
            }
            let original_block = block_position(blocks, target)?;
            let successor = continuations
                .iter()
                .position(|candidate| {
                    candidate.original_block == original_block && candidate.incoming == outgoing
                })
                .unwrap_or_else(|| {
                    let successor = continuations.len();
                    continuations.push(Continuation {
                        original_block,
                        incoming: outgoing,
                        successors: Vec::new(),
                    });
                    successor
                });
            successors.push((edge, successor));
        }
        continuations[position].successors = successors;
        position += 1;
    }
    Ok(Some(Specialization {
        entry: 0,
        continuations,
    }))
}

fn establish(regions: &[StructuralArgument], frontier: &mut [RegionState], place: PlaceId) {
    for (region, state) in regions.iter().zip(frontier) {
        if region.place == place {
            *state = RegionState::Live;
        }
    }
}

fn consume_argument(
    regions: &[StructuralArgument],
    frontier: &mut [RegionState],
    argument: &StructuralArgument,
) {
    if argument.access == StructuralAccess::Owned {
        consume(regions, frontier, argument.place, &argument.path);
    }
}

fn consume(
    regions: &[StructuralArgument],
    frontier: &mut [RegionState],
    place: PlaceId,
    path: &[StructuralPathSegment],
) {
    for (region, state) in regions.iter().zip(frontier) {
        if region.place != place || *state == RegionState::Absent {
            continue;
        }
        if contains_path(path, &region.path) {
            *state = RegionState::Absent;
        } else if terminal_semantics::structural_paths_may_overlap(&region.path, path) {
            match state {
                RegionState::Absent => {}
                RegionState::Live => *state = RegionState::Partial(vec![path.to_vec()]),
                RegionState::Partial(missing) => {
                    if !missing.iter().any(|prior| contains_path(prior, path)) {
                        missing.retain(|prior| !contains_path(path, prior));
                        missing.push(path.to_vec());
                        missing.sort();
                    }
                }
            }
        }
    }
}

/// Compare captured storage coordinates, not operation-owned bounds witnesses.
/// Each individual projection still owes its own ordinary bound obligation.
fn contains_path(prefix: &[StructuralPathSegment], path: &[StructuralPathSegment]) -> bool {
    prefix.len() <= path.len()
        && terminal_semantics::structural_paths_select_same_location(prefix, &path[..prefix.len()])
}

fn restore(
    regions: &[StructuralArgument],
    frontier: &mut [RegionState],
    place: PlaceId,
    path: &[StructuralPathSegment],
) {
    for (region, state) in regions.iter().zip(frontier) {
        if region.place != place {
            continue;
        }
        if contains_path(path, &region.path) {
            *state = RegionState::Live;
        } else if let RegionState::Partial(missing) = state {
            missing.retain(|hole| !contains_path(path, hole));
            if missing.is_empty() {
                *state = RegionState::Live;
            }
        }
    }
}

fn root_type(
    place: PlaceId,
    parameters: &[StructuralParameterDeclaration],
    places: &[StructuralPlaceDeclaration],
    blocks: &[Block],
) -> Result<StructuralTypeId, LoweringError> {
    let mut declarations = parameters
        .iter()
        .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
        .filter(|parameter| parameter.place == place)
        .map(|parameter| parameter.structural_type)
        .chain(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.structural())
                .filter(|result| result.place == place)
                .map(|result| result.structural_type),
        )
        .chain(places.iter().filter_map(|declaration| {
            if declaration.id != place {
                return None;
            }
            match declaration.kind {
                StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                } => Some(structural_type),
                _ => None,
            }
        }));
    let root = declarations.next().ok_or(LoweringError::Unsupported(
        "selected owner restoration has no structural declaration",
    ))?;
    if declarations.any(|other| other != root) {
        return unsupported("selected owner restoration has inconsistent structural declarations");
    }
    Ok(root)
}

fn dispose(
    regions: &[StructuralArgument],
    frontier: &mut [RegionState],
    action: &TerminalAffineCleanupAction,
) {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place)
        | TerminalAffineCleanupAction::RetireHome(place) => {
            consume(regions, frontier, *place, &[]);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            consume(regions, frontier, discard.place, &discard.path)
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) if !cleanup.retains_descendants => {
            consume(regions, frontier, cleanup.place, &cleanup.path);
        }
        TerminalAffineCleanupAction::InvokeNominal(_) => {}
    }
}

fn block_position(blocks: &[Block], target: BlockId) -> Result<usize, LoweringError> {
    let mut positions = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.id == target);
    let Some((position, _)) = positions.next() else {
        return unsupported("selected continuation names an absent block");
    };
    if positions.next().is_some() {
        return unsupported("selected continuation repeats a block identity");
    }
    Ok(position)
}

fn validate_group(blocks: &[Block], group: &SelectedCleanupGroup) -> Result<(), LoweringError> {
    let mut sites = blocks.iter().filter_map(|block| match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            structural_arguments,
            cleanup_actions,
            ..
        } if *edge == group.edge => Some((*target, structural_arguments, cleanup_actions)),
        _ => None,
    });
    let Some((target, arguments, actions)) = sites.next() else {
        return unsupported("selected cleanup group has no private entry jump");
    };
    if sites.next().is_some() || !arguments.is_empty() || !actions.is_empty() {
        return unsupported("selected cleanup group entry changes structural custody");
    }
    let entry = &blocks[block_position(blocks, target)?];
    let bypass = &blocks[block_position(blocks, group.bypass)?];
    if !entry.structural_parameters.is_empty()
        || !bypass.structural_parameters.is_empty()
        || entry
            .parameters
            .iter()
            .map(|value| value.value_type())
            .ne(bypass.parameters.iter().map(|value| value.value_type()))
        || entry
            .erased_scalar_formals
            .iter()
            .map(|value| value.value_type())
            .ne(bypass
                .erased_scalar_formals
                .iter()
                .map(|value| value.value_type()))
        || entry.erased_proof_formals != bypass.erased_proof_formals
    {
        return unsupported("selected cleanup bypass changes the captured continuation frame");
    }
    Ok(())
}
