//! Join retained storage by owned occurrence, not by the number of references
//! lending it. A parent's lifetime is the union of its dependent state chains;
//! the checked dependency graph orders child disposal before parent disposal.
//! Entry-held storage is already retained to return and must not die twice.
//! Geometry and lifetime do not grant cleanup: selected whole-home actions (or
//! the existing implicit-carrier no-code capability) still authorize each death.
use super::{
    ChainBacking, ChainExit, DisposalPosition, Forwarding, LoanChain, end_on_lender_exits,
    private_region,
};
use crate::LoweringError;
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralMultiplicity, StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalMachine,
};

#[derive(Clone, Default)]
struct Lifetime {
    members: BTreeSet<BlockId>,
    carriers: BTreeSet<PlaceId>,
}

impl Lifetime {
    fn include(&mut self, other: &Self) -> bool {
        let previous = (self.members.len(), self.carriers.len());
        self.members.extend(&other.members);
        self.carriers.extend(&other.carriers);
        previous != (self.members.len(), self.carriers.len())
    }
}

struct Home {
    place: PlaceId,
    lender: BlockId,
    lifetime: Lifetime,
    implicit_carrier: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish(
    machine: &mut TerminalMachine,
    chains: &[LoanChain],
    states: &[BlockId],
    headers: &BTreeSet<BlockId>,
    captures: &super::super::captured_loans::CapturedLoans,
    types: &[StructuralTypeDeclaration],
    entry_homes: &[PlaceId],
    parents: &[(PlaceId, PlaceId)],
    producers: &BTreeMap<PlaceId, OperationId>,
    parameters: &[PlaceId],
    next_obligation: &mut u64,
) -> Result<(), LoweringError> {
    // A direct borrow of descriptor storage retains that physical owner,
    // independently of the ultimate referent's checked logical loan. Loading
    // through a reference does not borrow the descriptor bytes: stop at those
    // crossings rather than inventing a dependency on the pointer container.
    let owned_homes = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| {
            parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
        })
        .map(|parameter| parameter.place)
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.result {
                    terminal_psi::OperationResult::Structural(result)
                        if result.multiplicity != StructuralMultiplicity::Unrestricted =>
                    {
                        Some(result.place)
                    }
                    _ => None,
                }),
        )
        .collect::<BTreeSet<_>>();
    let mut parents = parents.to_vec();
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        if let terminal_psi::OperationKind::EstablishReference { source, .. } = &operation.kind
            && owned_homes.contains(&source.place)
            && !source
                .path
                .iter()
                .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
            && let terminal_psi::OperationResult::Structural(result) = &operation.result
            && result.place != source.place
            && !parents.contains(&(result.place, source.place))
        {
            parents.push((result.place, source.place));
        }
    }
    let parents = parents.as_slice();
    let availability = super::super::loaned_local_homes::HomeAvailability::new(machine)?;
    let mut homes: Vec<Home> = Vec::new();
    let mut supplied = carrier_lifetimes(machine, headers, parents)?;
    for chain in chains {
        let mut lifetime = Lifetime::default();
        for &(position, dense, parameter) in &chain.members {
            lifetime.members.insert(states[position]);
            lifetime.carriers.insert(parameter);
            if let Some(carrier) = captures.parent_carrier(position, dense) {
                lifetime.carriers.insert(carrier.place);
            }
        }
        supplied.extend(
            chain
                .source_homes
                .iter()
                .map(|source| (*source, lifetime.clone())),
        );
        if chain.multiplicity != StructuralMultiplicity::Affine {
            continue;
        }
        if let ChainBacking::LenderHome { home, lender } = chain.backing {
            let home = home.ok_or(LoweringError::Unsupported(
                "state loan chain lost its retained home",
            ))?;
            if entry_homes.contains(&home) {
                continue;
            }
            if let Some(existing) = homes.iter_mut().find(|existing| existing.place == home) {
                if existing.lender != states[lender] {
                    return Err(LoweringError::Unsupported(
                        "retained home has distinct birth states",
                    ));
                }
                existing.lifetime.include(&lifetime);
            } else {
                homes.push(Home {
                    place: home,
                    lender: states[lender],
                    lifetime,
                    implicit_carrier: true,
                });
            }
        }
    }
    // Close finite lifetime sets over the checked parent graph. Two chains
    // borrowing one parent enlarge one region, never create two deaths.
    loop {
        let mut changed = false;
        for &(dependent, parent) in parents {
            if entry_homes.contains(&parent) {
                continue;
            }
            let mut lifetime = Lifetime::default();
            for (_, source) in supplied.iter().filter(|(source, _)| *source == dependent) {
                lifetime.include(source);
            }
            if let Some(source) = homes.iter().find(|home| home.place == dependent) {
                lifetime.include(&source.lifetime);
            }
            if lifetime.members.is_empty() {
                continue;
            }
            lifetime.carriers.insert(dependent);
            if let Some(home) = homes.iter_mut().find(|home| home.place == parent) {
                changed |= home.lifetime.include(&lifetime);
            } else {
                homes.push(Home {
                    place: parent,
                    lender: birth_header(machine, parent, headers, &availability)?,
                    lifetime,
                    implicit_carrier: false,
                });
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut remaining = (0..homes.len()).collect::<Vec<_>>();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .position(|candidate| {
                !parents.iter().any(|(child, parent)| {
                    *parent == homes[*candidate].place
                        && remaining.iter().any(|other| homes[*other].place == *child)
                })
            })
            .ok_or(LoweringError::Unsupported(
                "retained backing dependencies are cyclic",
            ))?;
        let position = remaining.remove(ready);
        let home = &homes[position];
        availability.require_in_regions(machine, home.place, &home.lifetime.members)?;
        if home.lifetime.members.contains(&home.lender) {
            return Err(LoweringError::Unsupported(
                "retained parent is reestablished inside its own loan lifetime",
            ));
        }
        let actions = super::super::loaned_local_homes::whole_home_actions(machine, home.place)?;
        let actions = match actions {
            Some(actions) => actions,
            None if home.implicit_carrier
                && terminal_semantics::structural_store_value_admitted(
                    machine,
                    types.iter(),
                    terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                    home.place,
                ) =>
            {
                vec![TerminalAffineCleanupAction::DiscardRoot(home.place)]
            }
            None => {
                return Err(LoweringError::Unsupported(
                    "retained backing lacks its checked whole-home disposition",
                ));
            }
        };
        let carriers = home.lifetime.carriers.iter().copied().collect::<Vec<_>>();
        let birth = machine.blocks[availability.birth(machine, home.place)?].id;
        let source_exits = private_region(machine, birth, headers)
            .into_iter()
            .filter(|block| {
                machine.block(block).is_some_and(|block| {
                    matches!(
                        block.terminator,
                        terminal_psi::terminator_return_pair_pattern!(..)
                            | terminal_psi::Terminator::ReturnStructural { .. }
                    ) || block
                        .terminator
                        .successor_targets()
                        .iter()
                        .any(|target| headers.contains(target))
                })
            })
            .collect();
        availability.require_in_regions(machine, home.place, &source_exits)?;
        // Completion scopes can separate the checked source-state cleanup
        // from its final state-arrival edge. Retain the authenticated whole
        // occurrence throughout that private region, not just on the edge
        // adjacent to the state header. This applies equally to descriptor
        // retirement and nominal cleanup: the original ordered actions above,
        // including nominal requirements, move to the actual lifetime exits.
        // Contextual cleanup checking proves a successor-loaned owner's hook
        // against that exact invocation's exit constraints, including aliases;
        // equal rosters alone would not grant relocation of a contextual hook.
        // Storage retention never substitutes a no-code death for a hook.
        retain_storage(
            machine,
            home.place,
            home.lender,
            &home.lifetime.members,
            headers,
        );
        end_on_lender_exits(
            machine,
            home.lender,
            headers,
            home.place,
            producers,
            &home.lifetime.members,
            &carriers,
            &actions,
            next_obligation,
        )?;
        let earlier = earlier_homes(position, &homes, producers, machine)?;
        for header in &home.lifetime.members {
            let exit = ChainExit {
                headers,
                storage: home.place,
                cleanup: &actions,
                forwarded: Forwarding::IntoMember(&home.lifetime.members),
                dependent_carriers: &carriers,
                position: DisposalPosition::RetainedHome {
                    earlier_homes: earlier.clone(),
                    machine_parameters: parameters,
                },
            };
            let region = private_region(machine, *header, headers);
            for block in machine
                .blocks
                .iter_mut()
                .filter(|block| region.contains(&block.id))
            {
                exit.dispose(&mut block.terminator, next_obligation)?;
            }
        }
    }
    Ok(())
}

fn retain_storage(
    machine: &mut TerminalMachine,
    home: PlaceId,
    lender: BlockId,
    members: &BTreeSet<BlockId>,
    headers: &BTreeSet<BlockId>,
) {
    let mut region = private_region(machine, lender, headers);
    for member in members {
        region.extend(private_region(machine, *member, headers));
    }
    for block in machine
        .blocks
        .iter_mut()
        .filter(|block| region.contains(&block.id))
    {
        match &mut block.terminator {
            terminal_psi::Terminator::Jump {
                cleanup_actions, ..
            }
            | terminal_psi::Terminator::ReturnStructural {
                cleanup_actions, ..
            }
            | terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                cleanup_actions.retain(|action| action.place() != home);
            }
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true
                    .cleanup_actions
                    .retain(|action| action.place() != home);
                when_false
                    .cleanup_actions
                    .retain(|action| action.place() != home);
            }
            terminal_psi::Terminator::StructuralCase { cases, .. } => {
                for case in cases {
                    case.cleanup_actions.retain(|action| action.place() != home);
                }
            }
            terminal_psi::Terminator::Crash { .. }
            | terminal_psi::Terminator::ExternalCompletion { .. } => {}
        }
    }
}

/// Captured carriers retain their checked dependencies through later state
/// handoffs, whether the carrier is owned or borrowed. Close the emitted
/// handoffs backwards from exact state parameters. A borrowed projection can
/// extend its source's lifetime but cannot move or dispose of that source;
/// owned projections still need their complete dependency frontier.
fn carrier_lifetimes(
    machine: &TerminalMachine,
    headers: &BTreeSet<BlockId>,
    parents: &[(PlaceId, PlaceId)],
) -> Result<Vec<(PlaceId, Lifetime)>, LoweringError> {
    let mut dependent_carriers = parents
        .iter()
        .map(|(child, _)| *child)
        .collect::<BTreeSet<_>>();
    loop {
        let mut changed = false;
        for block in &machine.blocks {
            for (target, arguments) in block
                .terminator
                .successor_targets()
                .into_iter()
                .zip(block.terminator.successor_block_arguments())
            {
                let target = machine.block(&target).ok_or(LoweringError::Unsupported(
                    "owned lifetime transport lost its target",
                ))?;
                if target.structural_parameters.len() != arguments.len() {
                    return Err(LoweringError::Unsupported(
                        "owned lifetime transport has mismatched arity",
                    ));
                }
                for (parameter, argument) in target.structural_parameters.iter().zip(arguments) {
                    if !dependent_carriers.contains(&argument.place) {
                        continue;
                    }
                    if argument.access == terminal_psi::StructuralAccess::Owned
                        && !argument.path.is_empty()
                    {
                        return Err(LoweringError::Unsupported(
                            "retained owned transport needs an exact projected dependency frontier",
                        ));
                    }
                    changed |= dependent_carriers.insert(parameter.place);
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut supplied: Vec<(PlaceId, Lifetime)> = Vec::new();
    for block in machine
        .blocks
        .iter()
        .filter(|block| headers.contains(&block.id))
    {
        for parameter in &block.structural_parameters {
            if dependent_carriers.contains(&parameter.place) {
                supplied.push((
                    parameter.place,
                    Lifetime {
                        members: BTreeSet::from([block.id]),
                        carriers: BTreeSet::from([parameter.place]),
                    },
                ));
            }
        }
    }
    loop {
        let mut changed = false;
        for block in &machine.blocks {
            for (target, arguments) in block
                .terminator
                .successor_targets()
                .into_iter()
                .zip(block.terminator.successor_block_arguments())
            {
                let target = machine.block(&target).ok_or(LoweringError::Unsupported(
                    "owned lifetime transport lost its target",
                ))?;
                for (parameter, argument) in target.structural_parameters.iter().zip(arguments) {
                    if argument.access == terminal_psi::StructuralAccess::Owned
                        && !argument.path.is_empty()
                    {
                        continue;
                    }
                    let Some((_, lifetime)) =
                        supplied.iter().find(|(place, _)| *place == parameter.place)
                    else {
                        continue;
                    };
                    let lifetime = lifetime.clone();
                    if let Some((_, current)) = supplied
                        .iter_mut()
                        .find(|(place, _)| *place == argument.place)
                    {
                        changed |= current.include(&lifetime);
                    } else {
                        supplied.push((argument.place, lifetime));
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return Ok(supplied);
        }
    }
}

fn birth_header(
    machine: &TerminalMachine,
    home: PlaceId,
    headers: &BTreeSet<BlockId>,
    availability: &super::super::loaned_local_homes::HomeAvailability,
) -> Result<BlockId, LoweringError> {
    // Moving an owned input or block parameter into a local keeps the same
    // storage occurrence. Its birth is the invocation/arrival, not necessarily
    // an operation result; use the shared occurrence-availability judgment.
    let birth = machine.blocks[availability.birth(machine, home)?].id;
    let mut owners = headers
        .iter()
        .copied()
        .chain((!headers.contains(&machine.entry)).then_some(machine.entry))
        .filter(|header| private_region(machine, *header, headers).contains(&birth));
    let owner = owners.next().ok_or(LoweringError::Unsupported(
        "retained parent has no birth region",
    ))?;
    if owners.next().is_some() {
        return Err(LoweringError::Unsupported(
            "retained parent has ambiguous birth regions",
        ));
    }
    Ok(owner)
}

fn earlier_homes(
    current: usize,
    homes: &[Home],
    producers: &BTreeMap<PlaceId, OperationId>,
    machine: &TerminalMachine,
) -> Result<Vec<PlaceId>, LoweringError> {
    let mut pending = vec![current];
    let mut visited = vec![current];
    let mut earlier = Vec::new();
    let birth = |place| {
        producers
            .get(&place)
            .map(|producer| (1, producer.get()))
            .or_else(|| {
                // Invocation and block parameters are births too. Their
                // declared order precedes operations in the owning region.
                machine
                    .structural_parameters
                    .iter()
                    .chain(
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .position(|parameter| parameter.place == place)
                    .map(|position| (0, position as u64))
            })
            .ok_or(LoweringError::Unsupported(
                "retained home has no ordered birth",
            ))
    };
    while let Some(younger) = pending.pop() {
        for (candidate, home) in homes.iter().enumerate() {
            if candidate == younger {
                continue;
            }
            let precedes = if home.lender == homes[younger].lender {
                birth(home.place)? < birth(homes[younger].place)?
            } else {
                home.lifetime.members.contains(&homes[younger].lender)
            };
            if !precedes {
                continue;
            }
            if candidate == current {
                return Err(LoweringError::Unsupported(
                    "retained homes have cyclic birth dependencies",
                ));
            }
            if !visited.contains(&candidate) {
                visited.push(candidate);
                pending.push(candidate);
                earlier.push(home.place);
            }
        }
    }
    Ok(earlier)
}
