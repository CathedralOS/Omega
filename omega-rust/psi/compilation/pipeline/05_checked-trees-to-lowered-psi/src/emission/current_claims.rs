//! Current transport names belong to the emitting execution frame. Producing
//! declarations remain immutable: a store replaces one authenticated subtree
//! in this association, and a move vacates it before the next observation.
//! These rows name custody already authorized by checked events; they establish
//! no resource, access, qualification, liveness or cleanup permission.

use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralArgument, StructuralPathSegment, StructuralResultClaimBinding};

#[derive(Clone, Default)]
pub(crate) struct CurrentClaimNames {
    homes: Vec<(PlaceId, Vec<StructuralResultClaimBinding>)>,
    unavailable: Vec<PlaceId>,
}

impl CurrentClaimNames {
    pub(crate) fn source(
        &self,
        argument: &StructuralArgument,
    ) -> Result<Option<Vec<StructuralResultClaimBinding>>, LoweringError> {
        if self.unavailable.contains(&argument.place) {
            return unsupported("current storage names require an unbound branch arrival");
        }
        if let Some((_, claims)) = self
            .homes
            .iter()
            .find(|(place, _)| *place == argument.place)
        {
            return Ok(Some(
                claims
                    .iter()
                    .filter_map(|binding| {
                        binding
                            .path
                            .strip_prefix(argument.path.as_slice())
                            .map(|path| StructuralResultClaimBinding {
                                claim: binding.claim,
                                path: path.to_vec(),
                            })
                    })
                    .collect(),
            ));
        }
        Ok(None)
    }

    /// A straight-line producing owner publishes the results it just emitted.
    /// Alternative bodies must publish their joined result instead of passing
    /// a concatenated arm roster to this operation.
    pub(crate) fn publish_operations(
        &mut self,
        operations: &[terminal_psi::Operation],
    ) -> Result<(), LoweringError> {
        for operation in operations {
            if let Some(result) = operation.result.structural() {
                self.publish(result.place, result.claims.clone())?;
            } else if let terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination }
            | terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                destination,
                ..
            } = &operation.kind
            {
                self.publish(*destination, Vec::new())?;
            }
        }
        Ok(())
    }

    pub(crate) fn contains(&self, place: PlaceId) -> bool {
        self.homes.iter().any(|(home, _)| *home == place) || self.unavailable.contains(&place)
    }

    /// Publish the exact receiving namespace, including a genuinely empty one.
    /// Its producer or predecessor edges, rather than this table, own admission.
    pub(crate) fn publish(
        &mut self,
        place: PlaceId,
        mut claims: Vec<StructuralResultClaimBinding>,
    ) -> Result<(), LoweringError> {
        self.unavailable.retain(|home| *home != place);
        claims.sort();
        if claims.iter().enumerate().any(|(index, claim)| {
            claims[..index]
                .iter()
                .any(|previous| previous.claim == claim.claim || previous.path == claim.path)
        }) {
            return unsupported("current storage namespace repeats a claim or path");
        }
        if let Some((_, current)) = self.homes.iter_mut().find(|(home, _)| *home == place) {
            *current = claims;
        } else {
            self.homes.push((place, claims));
        }
        Ok(())
    }

    /// Snapshot the old whole-home namespace before replacing its subtree.
    /// Missing source evidence is unavailable, never an empty declaration.
    pub(crate) fn replace(
        &mut self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        replacement: &[StructuralResultClaimBinding],
    ) -> Result<(), LoweringError> {
        let mut claims = self
            .source(&StructuralArgument {
                place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::Owned,
            })?
            .ok_or(LoweringError::Unsupported(
                "storage update has no current whole-home namespace",
            ))?;
        claims.retain(|claim| !claim.path.starts_with(path));
        claims.extend(replacement.iter().map(|claim| {
            let mut destination = path.to_vec();
            destination.extend_from_slice(&claim.path);
            StructuralResultClaimBinding {
                claim: claim.claim,
                path: destination,
            }
        }));
        self.publish(place, claims)
    }

    /// Project checked storage aliases over this actual predecessor's names.
    /// Every source is read before any destination is installed, so swaps do
    /// not observe a partially rebound frame. This changes no storage, claim
    /// authority, or unavailable-home judgment.
    pub(crate) fn with_edge_aliases(
        &self,
        aliases: &[super::claim_bindings::ClaimEdgeBinding],
    ) -> Result<Self, LoweringError> {
        let Some(first) = aliases.first() else {
            return Ok(self.clone());
        };
        if aliases.iter().any(|alias| alias.edge != first.edge) {
            return unsupported("selected storage aliases belong to different control edges");
        }
        for (position, alias) in aliases.iter().enumerate() {
            if aliases[..position].iter().any(|previous| {
                (previous.destination == alias.destination && previous.source != alias.source)
                    || (previous.source == alias.source
                        && previous.destination != alias.destination)
            }) {
                return unsupported(
                    "selected storage aliases have ambiguous source or destination names",
                );
            }
        }
        let mut projected = self.clone();
        let mut observed = vec![false; aliases.len()];
        for (place, claims) in &self.homes {
            if self.unavailable.contains(place) {
                continue;
            }
            let mut names = claims.clone();
            for (original, receiving) in claims.iter().zip(&mut names) {
                for (position, alias) in aliases.iter().enumerate() {
                    if alias.source == original.claim {
                        receiving.claim = alias.destination;
                        observed[position] = true;
                    }
                }
            }
            projected.publish(*place, names)?;
        }
        if observed.iter().any(|observed| !observed) {
            return unsupported("selected storage alias has no available current source name");
        }
        Ok(projected)
    }

    /// Equality of names on every real predecessor is the only invariant
    /// projection. Receiving parameters publish their own arrival names below;
    /// a stationary home without such transport must not pick an arm's roster.
    pub(crate) fn meet(&mut self, incoming: &Self) -> Result<(), LoweringError> {
        let mut homes = self
            .homes
            .iter()
            .chain(&incoming.homes)
            .map(|(place, _)| *place)
            .collect::<Vec<_>>();
        homes.extend(
            self.unavailable
                .iter()
                .chain(&incoming.unavailable)
                .copied(),
        );
        homes.sort();
        homes.dedup();
        for place in homes {
            if self.unavailable.contains(&place) || incoming.unavailable.contains(&place) {
                if !self.unavailable.contains(&place) {
                    self.unavailable.push(place);
                }
                continue;
            }
            let argument = StructuralArgument {
                place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::Owned,
            };
            let left = self.source(&argument)?;
            let right = incoming.source(&argument)?;
            match (left, right) {
                (Some(left), Some(right)) if left == right => self.publish(place, left)?,
                _ => self.unavailable.push(place),
            }
        }
        Ok(())
    }

    pub(crate) fn ingress(
        &mut self,
        parameters: impl IntoIterator<Item = PlaceId>,
        claims: &[terminal_psi::EntryClaim],
    ) -> Result<(), LoweringError> {
        for place in parameters {
            self.publish(
                place,
                claims
                    .iter()
                    .filter(|claim| claim.input == place)
                    .map(|claim| StructuralResultClaimBinding {
                        claim: claim.claim,
                        path: claim.path.clone(),
                    })
                    .collect(),
            )?;
        }
        Ok(())
    }
}
