//! A block parameter renames custody; it does not establish content authority.
//! Resolve every incoming owned actual at the exact relative path, then retain
//! a root roster only when every incoming alternative justifies the same set.

use super::{OriginTraversal, local_roots};
use semantic_vocabulary::{BlockId, ClaimId, ContentProjectionIdentity, PlaceId};
use terminal_psi::{StructuralAccess, StructuralPathSegment, TerminalMachine, TerminalModule};

fn source_claim(
    machine: &TerminalMachine,
    dominators: &crate::control_graph::DominatorTree,
    predecessor: BlockId,
    place: PlaceId,
    path: &[StructuralPathSegment],
    traversal: &mut OriginTraversal,
) -> Result<ClaimId, &'static str> {
    let mut selected = None;
    let mut retain = |claim| {
        if selected.is_some_and(|previous| previous != claim) {
            return Err("content arrival has ambiguous source claim identities");
        }
        selected = Some(claim);
        Ok(())
    };
    for entry in &machine.entry_claims {
        traversal.visit()?;
        if entry.input == place && entry.path == path {
            retain(entry.claim)?;
        }
    }
    for block in &machine.blocks {
        traversal.visit()?;
        if !dominators.dominates(block.id, predecessor) {
            continue;
        }
        if block
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == place)
        {
            for entry in &machine.block_entry_claims {
                traversal.visit()?;
                if entry.input == place && entry.path == path {
                    retain(entry.claim)?;
                }
            }
        }
        for operation in &block.operations {
            traversal.visit()?;
            if let Some(result) = operation
                .result
                .as_structural()
                .filter(|result| result.place == place)
            {
                for binding in &result.claims {
                    traversal.visit()?;
                    if binding.path == path {
                        retain(binding.claim)?;
                    }
                }
            }
        }
    }
    selected.ok_or("content arrival has no exact dominating source claim")
}

pub(super) fn roots(
    module: &TerminalModule,
    machine: &TerminalMachine,
    claim: ClaimId,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<Option<Vec<ClaimId>>, &'static str> {
    let mut entries = machine
        .block_entry_claims
        .iter()
        .filter(|entry| entry.claim == claim);
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    if entries.next().is_some() {
        return Err("content arrival has ambiguous destination claim declarations");
    }
    let mut parameters =
        machine.blocks.iter().flat_map(|block| {
            block.structural_parameters.iter().enumerate().filter_map(
                move |(position, parameter)| {
                    (parameter.place == entry.input).then_some((block, position, parameter))
                },
            )
        });
    let (destination, position, parameter) = parameters
        .next()
        .ok_or("content arrival lacks its destination formal")?;
    if parameters.next().is_some() || parameter.access != StructuralAccess::Owned {
        return Err("content arrival lacks one exact owned destination formal");
    }
    let dominators = crate::control_graph::dominators(machine);
    let mut roots = None;
    let mut incoming = false;
    for predecessor in &machine.blocks {
        traversal.visit()?;
        let arguments = predecessor.terminator.successor_block_arguments();
        for (index, (_, target)) in predecessor.terminator.successor_edges().iter().enumerate() {
            traversal.visit()?;
            if *target != destination.id {
                continue;
            }
            incoming = true;
            let actual = arguments
                .get(index)
                .and_then(|arguments| arguments.get(position))
                .ok_or("content arrival lacks an exact positional structural actual")?;
            if actual.access != StructuralAccess::Owned {
                return Err("content arrival changes the source's owned access");
            }
            let mut path = actual.path.clone();
            path.extend_from_slice(&entry.path);
            let source = source_claim(
                machine,
                &dominators,
                predecessor.id,
                actual.place,
                &path,
                traversal,
            )?;
            // An identity-preserving edge supplies no new root. A real producer
            // or a renamed incoming edge must still ground this claim below.
            if source == claim {
                continue;
            }
            let candidate = local_roots(module, machine, source, projection, traversal)?;
            if roots
                .as_ref()
                .is_some_and(|previous| previous != &candidate)
            {
                return Err("content arrival input rosters differ across incoming paths");
            }
            roots = Some(candidate);
        }
    }
    if !incoming {
        return Err("content arrival has no grounded predecessor");
    }
    Ok(roots)
}
