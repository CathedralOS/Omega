//! Current storage facts use physical borrowed origins, not a retired spelling.
//!
//! The independently validated edge graph supplies every possible origin of a
//! borrowed block parameter. Writes invalidate an observation if ANY possible
//! regions overlap. Return publication can rename a fact only when ALL origins
//! agree on one invocation parameter and exact projection. These are separate
//! judgments: possible interference grants no equality or access permission.
//! Prepare the map once per machine reconstruction and reuse it for stores,
//! calls, and normal exits. Immutable SSA captures remain owned by field_snapshots.

use semantic_vocabulary::{CanonicalStructuralPathSegment, PlaceId, Proposition};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, TerminalMachine, TerminalModule};

pub(super) struct StorageObservations {
    origins: BTreeMap<PlaceId, BTreeSet<(PlaceId, Vec<CanonicalStructuralPathSegment>)>>,
    invocation_origins: BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
}

impl StorageObservations {
    pub(super) fn new(module: &TerminalModule, machine: &TerminalMachine) -> Self {
        let origins = crate::validation::window_aliases(module, machine);
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
                        parameter.place == origin.0
                            && matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow
                            )
                    })
                    .then(|| (*place, origin.clone()))
            })
            .collect();
        Self {
            origins,
            invocation_origins,
        }
    }

    fn regions(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
        match self.origins.get(&root) {
            Some(origins) => origins
                .iter()
                .map(|(origin, prefix)| {
                    let mut full = prefix.clone();
                    full.extend_from_slice(path);
                    (*origin, full)
                })
                .collect(),
            None => vec![(root, path.to_vec())],
        }
    }

    pub(super) fn observes_write(
        &self,
        proposition: &Proposition,
        root: PlaceId,
        written: &[CanonicalStructuralPathSegment],
    ) -> bool {
        let written_regions = self.regions(root, written);
        crate::validation::proposition_observation_sites(proposition)
            .into_iter()
            .any(|(observed, path)| {
                self.regions(observed, path.as_deref().unwrap_or_default())
                    .iter()
                    .any(|(observed_root, observed_path)| {
                        written_regions.iter().any(|(written_root, written_path)| {
                            observed_root == written_root
                                && (observed_path.starts_with(written_path)
                                    || written_path.starts_with(observed_path))
                        })
                    })
            })
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
        crate::validation::proposition_boolean_field_roots(proposition)
            .into_iter()
            .any(|observed| {
                self.regions(observed, &[])
                    .iter()
                    .any(|(observed_root, observed_path)| {
                        places.iter().any(|written| {
                            self.regions(*written, &[]).iter().any(
                                |(written_root, written_path)| {
                                    observed_root == written_root
                                        && (observed_path.starts_with(written_path)
                                            || written_path.starts_with(observed_path))
                                },
                            )
                        })
                    })
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
