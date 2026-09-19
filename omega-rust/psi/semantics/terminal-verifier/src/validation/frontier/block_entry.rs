//! The frontier a block starts from: the machine's entry frontier and the
//! join of a block's incoming frontiers.

use super::super::{
    BTreeMap, BlockId, ClaimId, ModuleError, StructuralAccess, StructuralMultiplicity,
    TerminalMachine, TerminalModule,
};
use super::{LiveClaim, StructuralOwnershipFrontier};

/// The frontier the machine's entry block starts from: the live entry and
/// content entry claims, the entry references, and the owned parameters
/// that enter by-value custody.
pub(super) fn entry_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<StructuralOwnershipFrontier, ModuleError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &machine.entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("entry claims were validated against structural parameters");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &machine.content_entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    let entry = StructuralOwnershipFrontier {
        references: super::super::references::entry_references(module, machine)?,
        claims,
        owned_places: machine
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                // Only owned parameters enter by-value custody. Borrowed self
                // and explicit reference parameters obey the same rule;
                // referent multiplicity does not turn a loan into a transfer.
                (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    && parameter.access == StructuralAccess::Owned)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
        restoration_debt: BTreeMap::new(),
    };
    Ok(entry)
}

/// The frontier a block starts from: every incoming frontier must agree on
/// claims, owned places, references and partial custody.
pub(super) fn joined_frontier(
    block_id: BlockId,
    frontiers: &[StructuralOwnershipFrontier],
) -> Result<StructuralOwnershipFrontier, ModuleError> {
    let frontier = frontiers
        .first()
        .expect("a reachable block has an incoming frontier")
        .clone();
    if frontiers
        .iter()
        .any(|candidate| candidate.claims != frontier.claims)
    {
        return Err(ModuleError::ClaimFrontierJoinMismatch(block_id));
    }
    if frontiers
        .iter()
        .any(|candidate| candidate.owned_places != frontier.owned_places)
        || frontiers
            .iter()
            .any(|candidate| candidate.references != frontier.references)
        || frontiers
            .iter()
            .any(|candidate| candidate.partial_custody_paths != frontier.partial_custody_paths)
        || frontiers
            .iter()
            .any(|candidate| candidate.restoration_debt != frontier.restoration_debt)
    {
        return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block_id));
    }
    Ok(frontier)
}
