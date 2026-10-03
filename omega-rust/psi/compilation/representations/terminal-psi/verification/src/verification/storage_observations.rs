//! Current storage facts use physical borrowed origins, not a retired spelling.
//!
//! The independently validated edge graph supplies every possible origin of a
//! borrowed block parameter. Writes invalidate an observation if ANY possible
//! regions overlap. Return publication can rename a fact only when ALL origins
//! agree on one invocation parameter and exact projection. These are separate
//! judgments: possible interference grants no equality or access permission.
//! Prepare the map once per machine reconstruction and reuse it for stores,
//! calls, and normal exits. Immutable SSA captures remain owned by field_snapshots.

use crate::validation::ObservationKind;
use semantic_vocabulary::{CanonicalStructuralPathSegment, PlaceId, Proposition};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, TerminalMachine, TerminalModule};
use terminal_semantics::borrowed_storage::{self, AliasSources, Region};

pub(super) struct StorageObservations<'a> {
    module: &'a TerminalModule,
    machine: &'a TerminalMachine,
    aliases: AliasSources,
    invocation_origins: BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
}

impl<'a> StorageObservations<'a> {
    pub(super) fn new(module: &'a TerminalModule, machine: &'a TerminalMachine) -> Self {
        let origins = crate::validation::borrowed_storage_origins(module, machine);
        let invocation_origins = origins
            .iter()
            .filter_map(|(place, alternatives)| {
                if alternatives.len() != 1 {
                    return None;
                }
                let origin = alternatives.first()?;
                machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| {
                        origin.exact
                            && parameter.place == origin.root
                            && matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow
                            )
                    })
                    .then(|| (*place, (origin.root, origin.path.clone())))
            })
            .collect();
        Self {
            module,
            machine,
            aliases: crate::validation::borrowed_observation_aliases(module, machine),
            invocation_origins,
        }
    }

    fn regions(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        kind: ObservationKind,
    ) -> Vec<Region> {
        let projection =
            crate::validation::observation_projection(self.module, self.machine, root, path, kind);
        borrowed_storage::resolve(&self.aliases, root, projection)
    }

    fn sites_overlap(
        &self,
        sites: impl IntoIterator<
            Item = (
                (PlaceId, Option<Vec<CanonicalStructuralPathSegment>>),
                ObservationKind,
            ),
        >,
        root: PlaceId,
        written: &[CanonicalStructuralPathSegment],
        written_kind: ObservationKind,
    ) -> bool {
        let written_regions = self.regions(root, written, written_kind);
        sites.into_iter().any(|((observed, path), kind)| {
            self.regions(observed, path.as_deref().unwrap_or_default(), kind)
                .iter()
                .any(|observation| {
                    written_regions
                        .iter()
                        .any(|written| observation.overlaps(written))
                })
        })
    }

    pub(super) fn observes_write(
        &self,
        proposition: &Proposition,
        root: PlaceId,
        written: &[CanonicalStructuralPathSegment],
    ) -> bool {
        self.sites_overlap(
            crate::validation::storage_observations(proposition),
            root,
            written,
            ObservationKind::Storage,
        )
    }

    /// Replacing an existing byte/element changes payload, not descriptor extent.
    pub(super) fn observes_payload_write(
        &self,
        proposition: &Proposition,
        root: PlaceId,
        written: &[CanonicalStructuralPathSegment],
    ) -> bool {
        self.sites_overlap(
            crate::validation::storage_observations(proposition)
                .into_iter()
                .filter(|(_, kind)| *kind != ObservationKind::Extent),
            root,
            written,
            ObservationKind::ByteContents,
        )
    }

    pub(super) fn observes_places(&self, proposition: &Proposition, places: &[PlaceId]) -> bool {
        places
            .iter()
            .any(|root| self.observes_write(proposition, *root, &[]))
    }

    pub(super) fn observes_unversioned_places(
        &self,
        proposition: &Proposition,
        places: &[PlaceId],
    ) -> bool {
        places.iter().any(|root| {
            self.sites_overlap(
                crate::validation::storage_observations(proposition)
                    .into_iter()
                    .filter(|((_, path), _)| path.is_some()),
                *root,
                &[],
                ObservationKind::Storage,
            )
        })
    }

    /// Return facts describe the current invocation storage, not entry values.
    /// Their lifetime was already bounded by alias-aware writes and calls.
    pub(super) fn append_invocation_facts(&self, axioms: &mut Vec<Proposition>) {
        if self.invocation_origins.is_empty() {
            return;
        }
        let count = axioms.len();
        for index in 0..count {
            // Content revisions and projection paths have an independent
            // conservation owner. A current-storage alias is no evidence for
            // moving a versioned content term to an invocation root.
            if crate::validation::proposition_contains_content(&axioms[index]) {
                continue;
            }
            let rewritten = super::substitution::substitute_proposition_structural_places(
                &axioms[index],
                &self.invocation_origins,
            );
            if rewritten != axioms[index] && !axioms.contains(&rewritten) {
                axioms.push(rewritten);
            }
        }
    }
}

#[cfg(test)]
mod tests;
