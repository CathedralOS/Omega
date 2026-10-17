//! The arrival storage a structural place occupies.

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    Block, StructuralArgument, StructuralParameterDeclaration, TerminalMachine, Terminator,
};

/// Qualification metadata does not add descriptor words. Only element views
/// carry universal child rows; byte descriptors retain their whole predicates.
/// This classifies geometry, never establishes the retained memberships.
pub fn view_descriptor_projected_qualifications<'types>(
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
    root: semantic_vocabulary::StructuralTypeId,
    projected: &[terminal_psi::StructuralPathQualification],
) -> bool {
    if projected.is_empty() {
        return true;
    }
    let mut matching = declarations.filter(|row| row.id == root);
    let Some(declaration) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::ElementView { .. }
        )
        && projected.iter().all(|row| row.is_element_membership())
}

/// A readable descriptor arrival retains its pointer and extent, not its
/// backing storage. Affine carrier lifetime does not turn borrowed access into
/// ownership or make the payload a thin address join. Qualification admission,
/// exact source access and live loan custody remain the consumer's judgments.
pub fn view_descriptor_parameter<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    if !matches!(
        parameter.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) || !matches!(
        parameter.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
            | terminal_psi::StructuralMultiplicity::Affine
    ) {
        return false;
    }
    let mut matching =
        declarations.filter(|declaration| declaration.id == parameter.structural_type);
    let Some(declaration) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && (parameter.projected_qualifications.is_empty()
            || matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
            ) && parameter
                .projected_qualifications
                .iter()
                .all(|row| row.is_element_membership()))
        && (declaration.shape.is_borrowed_byte_carrier()
            || matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
            ))
}

/// Backing access retained by a live descriptor arrival. Owning the descriptor
/// storage does not own its referent; its type and multiplicity retain that
/// permission. This query grants neither storage ownership nor availability.
pub fn descriptor_parameter_access<'types>(
    parameter: &StructuralParameterDeclaration,
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    if parameter.access == terminal_psi::StructuralAccess::Owned {
        if !view_descriptor_projected_qualifications(
            declarations.clone(),
            parameter.structural_type,
            &parameter.projected_qualifications,
        ) {
            return None;
        }
        crate::descriptor_results::descriptor_value_access(
            declarations,
            parameter.structural_type,
            parameter.multiplicity,
        )
    } else {
        view_descriptor_parameter(parameter, declarations).then_some(parameter.access)
    }
}

/// The parameter declaration a structural place arrives as: the machine's own
/// parameter at its declared position, or a block parameter in the block that
/// declares it. Both are storage an operation addresses in place, unlike an
/// operation result, a byte-sequence literal or a provider attachment, none of
/// which has a caller-owned home to write through.
///
/// A receiver collapses every state onto the one machine parameter, so a
/// validator consulting only `machine.structural_parameters` admits the
/// receiver spelling of a program and rejects the same program spelled with a
/// named data parameter, where each state owns its own block parameter. The two
/// rosters carry the same access, multiplicity and qualification vocabulary and
/// each is validated where it is declared, so the roster a root happens to sit
/// in was spelling, not permission. Callers still read the returned access,
/// multiplicity and qualifications themselves; this answers only which storage
/// the place is. The place, owner and dense roster position must name exactly
/// one declaration. Malformed duplicates and entry-block formals fail closed;
/// operation results are not arrival storage.
pub fn arrival_parameter<'machine>(
    machine: &'machine TerminalMachine,
    place: PlaceId,
) -> Option<&'machine StructuralParameterDeclaration> {
    let mut place_declarations = machine
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let place_declaration = place_declarations.next()?;
    if place_declarations.next().is_some() {
        return None;
    }

    match place_declaration.kind {
        StructuralPlaceKind::Parameter { position } => {
            let parameter = machine.structural_parameters.get(position as usize)?;
            (parameter.place == place
                && parameter.position == position
                && machine
                    .structural_parameters
                    .iter()
                    .filter(|candidate| candidate.place == place)
                    .count()
                    == 1
                && machine.blocks.iter().all(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .all(|candidate| candidate.place != place)
                }))
            .then_some(parameter)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            if block == machine.entry
                || machine
                    .structural_parameters
                    .iter()
                    .any(|candidate| candidate.place == place)
            {
                return None;
            }
            let mut owner_blocks = machine
                .blocks
                .iter()
                .filter(|candidate| candidate.id == block);
            let owner = owner_blocks.next()?;
            if owner_blocks.next().is_some() {
                return None;
            }
            let parameter = owner.structural_parameters.get(position as usize)?;
            (parameter.place == place
                && parameter.position == position
                && machine
                    .blocks
                    .iter()
                    .flat_map(|candidate| &candidate.structural_parameters)
                    .filter(|candidate| candidate.place == place)
                    .count()
                    == 1)
                .then_some(parameter)
        }
        _ => None,
    }
}

/// The place in `predecessor` that denotes, on the edge into `block`, the same
/// storage as `place` denotes inside `block`.
///
/// Each incoming edge binds the target block's structural parameters
/// positionally, so the edge renames the storage without changing it: a fact
/// established about that storage before the edge is a fact about it after.
/// A fact that holds across states therefore has to follow the rename, because
/// a receiver gives every state the one machine parameter while a named data
/// parameter gives each state its own block parameter.
///
/// The edge must forward the whole storage. A projected argument names only a
/// part of it, and a structural-case edge binds payload fields rather than
/// forwarding a parameter, so both answer `None` and the caller must abandon
/// the fact on that path rather than carry it. Two edges from one predecessor
/// into `block` that bind different places answer `None` for the same reason.
/// A place that is not this block's own parameter -- an operation result, a
/// byte-sequence literal, the machine's parameter -- already denotes one
/// storage machine-wide and answers itself.
pub fn root_before_edge(
    machine: &TerminalMachine,
    block: BlockId,
    predecessor: &Block,
    place: PlaceId,
) -> Option<PlaceId> {
    let projection = projection_before_edge(machine, block, predecessor, place)?;
    projection.path.is_empty().then_some(projection.root)
}

/// The exact source projection a checked edge binds to this arrival. The path
/// is retained rather than treating a pointee or field as its carrier root.
/// This names storage only; access, custody and type correspondence remain
/// independently checked. Conflicting edges and case payloads supply no match.
pub fn projection_before_edge<'edge>(
    machine: &TerminalMachine,
    block: BlockId,
    predecessor: &'edge Block,
    place: PlaceId,
) -> Option<terminal_psi::OperationProjection<'edge>> {
    let Some(position) = machine
        .structural_places
        .iter()
        .find_map(|row| match row.kind {
            StructuralPlaceKind::BlockParameter {
                block: owner,
                position,
            } if row.id == place && owner == block => Some(position),
            _ => None,
        })
    else {
        return Some(terminal_psi::OperationProjection {
            root: place,
            path: &[],
        });
    };
    if reaches_by_case(predecessor, block) {
        return None;
    }
    let mut forwarded = None;
    for (target, arguments) in forwarded_bindings(predecessor) {
        if target != block {
            continue;
        }
        let argument = arguments.get(position as usize)?;
        let projection = terminal_psi::OperationProjection {
            root: argument.place,
            path: &argument.path,
        };
        if forwarded.is_some_and(|earlier| earlier != projection) {
            return None;
        }
        forwarded = Some(projection);
    }
    forwarded
}

/// Whether `predecessor` reaches `block` by a structural-case edge, which
/// declares payload fields instead of forwarding the block's parameters.
fn reaches_by_case(predecessor: &Block, block: BlockId) -> bool {
    matches!(&predecessor.terminator, Terminator::StructuralCase { cases, .. }
        if cases.iter().any(|successor| successor.target == block))
}

/// Candidate names of the storage at a use, grouped by predecessor block.
///
/// A state parameter renames its incoming storage; an ordinary control edge
/// keeps the dominating name. Both edges participate in discovery. A join can
/// reach the use through different storage, so membership is only a candidate,
/// never an equality or a freshness proof. Consumers must independently trace
/// every arrival and match the producer's root as well as its operation.
pub struct StorageNames {
    behind: BTreeMap<BlockId, BTreeSet<PlaceId>>,
}

impl StorageNames {
    /// Names encountered in `block` while tracing this use backward.
    pub fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
        self.behind.get(&block).into_iter().flatten().copied()
    }
}

/// Follow whole-storage bindings backward, including edges without a rename.
///
/// Visit each (block, place) pair rather than retaining one provisional name
/// per block. Different paths can disagree in one block and converge to the
/// same upstream storage; discarding an ambiguous block would lose that route,
/// while retaining its previously propagated name would depend on block order.
/// The finite pair worklist handles cycles without either shortcut. Projected
/// and case-payload bindings remain excluded by [`root_before_edge`].
pub fn storage_names(machine: &TerminalMachine, block: BlockId, root: PlaceId) -> StorageNames {
    let mut behind: BTreeMap<BlockId, BTreeSet<PlaceId>> = BTreeMap::new();
    let mut pending = vec![(block, root)];
    while let Some((block, root)) = pending.pop() {
        if !behind.entry(block).or_default().insert(root) {
            continue;
        }
        for predecessor in &machine.blocks {
            let reaches = reaches_by_case(predecessor, block)
                || forwarded_bindings(predecessor)
                    .iter()
                    .any(|(target, _)| *target == block);
            if reaches && let Some(before) = root_before_edge(machine, block, predecessor, root) {
                pending.push((predecessor.id, before));
            }
        }
    }
    StorageNames { behind }
}

/// Each edge out of `predecessor` that forwards the target block's structural
/// parameters, as the target and its positional bindings. A structural-case
/// edge is absent: it declares payload fields instead of forwarding them.
fn forwarded_bindings(predecessor: &Block) -> Vec<(BlockId, &[StructuralArgument])> {
    match &predecessor.terminator {
        Terminator::Jump {
            target,
            structural_arguments,
            ..
        } => vec![(*target, structural_arguments.as_slice())],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| (successor.target, successor.structural_arguments.as_slice()))
            .collect(),
        _ => Vec::new(),
    }
}
