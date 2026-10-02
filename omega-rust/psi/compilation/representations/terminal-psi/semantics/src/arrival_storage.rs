//! The arrival storage a structural place occupies.

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use std::collections::BTreeMap;
use terminal_psi::{
    Block, StructuralArgument, StructuralParameterDeclaration, TerminalMachine, Terminator,
};

/// A block's own structural parameter at the position its place records.
/// Mirrors the machine-roster half below for the per-block roster.
fn block_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)
        .filter(|parameter| {
            parameter.place == place && parameter.position == position && !parameter.is_self
        })
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
/// the place is.
pub fn arrival_parameter<'machine>(
    machine: &'machine TerminalMachine,
    place: PlaceId,
) -> Option<&'machine StructuralParameterDeclaration> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| {
            parameter.place == place
                && machine.structural_places.iter().any(|row| {
                    row.id == place
                        && row.kind
                            == StructuralPlaceKind::Parameter {
                                position: parameter.position,
                                is_self: parameter.is_self,
                            }
                })
        })
        .or_else(|| block_parameter(machine, place))
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
        return Some(place);
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
        if !argument.path.is_empty() || forwarded.is_some_and(|earlier| earlier != argument.place) {
            return None;
        }
        forwarded = Some(argument.place);
    }
    forwarded
}

/// Whether `predecessor` reaches `block` by a structural-case edge, which
/// declares payload fields instead of forwarding the block's parameters.
fn reaches_by_case(predecessor: &Block, block: BlockId) -> bool {
    matches!(&predecessor.terminator, Terminator::StructuralCase { cases, .. }
        if cases.iter().any(|successor| successor.target == block))
}

/// The names one storage answers to, per block, among the blocks behind a use
/// of it.
///
/// A place denotes one storage wherever it is mentioned, so the difficulty is
/// never that a place changes meaning between blocks: it is that two different
/// places denote one storage, because every edge that forwards it renames it.
/// A receiver gives every state the one machine parameter, while a named data
/// parameter gives each state its own block parameter, so a search comparing
/// place identities finds an operation in another state only for the receiver
/// spelling. The use site's own place is therefore always an answer, and each
/// block behind the use may answer to one further place as well.
///
/// Membership says only that two places denote one storage, never that the
/// operation holding one of them runs before another. Callers that need that
/// ordering still prove it -- `unchanged_since` requires the earlier operation
/// to dominate the later one on every path -- so a name only some arrivals
/// reach cannot carry a fact the others never established.
pub struct StorageNames {
    root: PlaceId,
    behind: BTreeMap<BlockId, Option<PlaceId>>,
}

impl StorageNames {
    /// The places `block` may use for this storage: the use site's own place,
    /// and the place this block forwards into it, when the two differ and this
    /// block reaches the use under exactly one such place.
    pub fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
        std::iter::once(self.root).chain(
            self.behind
                .get(&block)
                .copied()
                .flatten()
                .filter(|place| *place != self.root),
        )
    }
}

/// The names the storage that `root` names inside `block` answers to in the
/// blocks that reach `block`.
///
/// Only predecessors are followed. An establishment is looked for behind the
/// use, never ahead of it, and naming a predecessor's argument is exact for
/// arrivals through that predecessor: the edge binds the two to one storage.
/// Following successors instead would name a target's parameter from one
/// incoming edge while another edge binds different storage into it, which no
/// later check would catch. Only whole-storage bindings are followed, for the
/// reason [`root_before_edge`] states.
pub fn storage_names(machine: &TerminalMachine, block: BlockId, root: PlaceId) -> StorageNames {
    let mut behind = BTreeMap::new();
    behind.insert(block, Some(root));
    let mut changed = true;
    while changed {
        changed = false;
        for predecessor in &machine.blocks {
            for (target, arguments) in forwarded_bindings(predecessor) {
                let Some(&Some(known)) = behind.get(&target) else {
                    continue;
                };
                let Some(parameters) = machine
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == target)
                    .map(|candidate| &candidate.structural_parameters)
                else {
                    continue;
                };
                for (position, argument) in arguments.iter().enumerate() {
                    let Some(parameter) = parameters.get(position) else {
                        continue;
                    };
                    if parameter.place != known || !argument.path.is_empty() {
                        continue;
                    }
                    changed |= learn(&mut behind, predecessor.id, argument.place);
                }
            }
        }
    }
    StorageNames { root, behind }
}

/// Record one block-local name, erasing the entry to ambiguous when the block
/// already reaches the use under a different place: two names at once identify
/// no one storage, and the use site's own place remains the answer there.
fn learn(behind: &mut BTreeMap<BlockId, Option<PlaceId>>, block: BlockId, place: PlaceId) -> bool {
    match behind.get(&block) {
        Some(Some(known)) if *known == place => false,
        Some(None) => false,
        Some(Some(_)) => {
            behind.insert(block, None);
            true
        }
        None => {
            behind.insert(block, Some(place));
            true
        }
    }
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
