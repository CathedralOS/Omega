//! Planning checked bodies on several cores without changing what they plan.
//!
//! Checked bodies are planned against one collector in roster order, and a
//! body can see what earlier bodies registered: a calling parameter gains a
//! callback backing only when its identity is already catalogued, and two
//! presentations of one identity must agree. So a body plans against a fork:
//! a copy of the collector as it stood after some prefix of the roster (the
//! fork's base), which remembers every row it consulted and wrote. The merge
//! walks the roster in order and keeps a log of the position at which each row
//! last changed. A fork that consulted no row changed at or after its base saw
//! exactly the catalog the sequential pass would have shown it, so its plan
//! and its writes are the sequential ones and are replayed as they are. A
//! stale fork plans again from the merged collector; see `admits`.
//!
//! Cleanup-control arenas hand out spans that the plan stores, so a fork that
//! appended to them is replayed only when no other body appended since its
//! base. The completed-request memo is a query cache: a fork that misses it
//! recomputes the same admitted shape, and its entries are replayed.

use super::*;
use std::sync::{Mutex, PoisonError};

/// A keyed catalog that remembers which keys an attempt consulted, through
/// shared and exclusive access alike, and the rows it wrote, with each
/// row's value before its first write.
pub(crate) struct ConsultedMap<K, V> {
    rows: BTreeMap<K, V>,
    consulted: Mutex<Consulted<K>>,
    written: BTreeMap<K, Option<V>>,
}

/// The keys an attempt looked up, and whether it read the whole catalog.
struct Consulted<K> {
    keys: BTreeSet<K>,
    every_key: bool,
}

impl<K> Default for Consulted<K> {
    fn default() -> Self {
        Self {
            keys: BTreeSet::new(),
            every_key: false,
        }
    }
}

impl<K: Ord + Clone, V: Clone + PartialEq> ConsultedMap<K, V> {
    fn consult(&self, key: &K) {
        self.consulted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys
            .insert(key.clone());
    }

    fn write(&mut self, key: &K) {
        self.consulted
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .keys
            .insert(key.clone());
        if !self.written.contains_key(key) {
            self.written
                .insert(key.clone(), self.rows.get(key).cloned());
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.consult(key);
        self.rows.get(key)
    }

    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.consult(key);
        self.rows.contains_key(key)
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.write(key);
        self.rows.get_mut(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.write(&key);
        self.rows.insert(key, value)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.write(key);
        self.rows.remove(key)
    }

    /// Reading every row consults the whole catalog, including rows another
    /// attempt may have added.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.consulted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .every_key = true;
        self.rows.iter()
    }

    pub(crate) fn retain(&mut self, keep: impl FnMut(&K, &mut V) -> bool) {
        self.rows.retain(keep);
    }

    pub(crate) fn into_values(self) -> impl Iterator<Item = V> {
        self.rows.into_values()
    }

    /// What this attempt consulted, and the rows whose value it changed, as
    /// they stand now.
    fn forget(&mut self) -> (Consulted<K>, Vec<(K, Option<V>)>) {
        let written = std::mem::take(&mut self.written)
            .into_iter()
            .filter(|(key, before)| self.rows.get(key) != before.as_ref())
            .map(|(key, _)| {
                let now = self.rows.get(&key).cloned();
                (key, now)
            })
            .collect();
        (
            std::mem::take(
                self.consulted
                    .get_mut()
                    .unwrap_or_else(PoisonError::into_inner),
            ),
            written,
        )
    }
}

impl<K: Ord, V> Default for ConsultedMap<K, V> {
    fn default() -> Self {
        Self {
            rows: BTreeMap::new(),
            consulted: Mutex::default(),
            written: BTreeMap::new(),
        }
    }
}

/// A copy has consulted and written nothing yet.
impl<K: Ord + Clone, V: Clone> Clone for ConsultedMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            consulted: Mutex::default(),
            written: BTreeMap::new(),
        }
    }
}

/// A recursion-marker set that remembers which members an attempt consulted.
pub(crate) struct ConsultedSet<K>(ConsultedMap<K, ()>);

impl<K: Ord + Clone> Clone for ConsultedSet<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K: Ord + Clone> ConsultedSet<K> {
    pub(crate) fn new() -> Self {
        Self(ConsultedMap::default())
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }

    pub(crate) fn insert(&mut self, key: K) -> bool {
        self.0.insert(key, ()).is_none()
    }

    pub(crate) fn remove(&mut self, key: &K) -> bool {
        self.0.remove(key).is_some()
    }
}

/// Where each append-only catalog ended before an attempt.
#[derive(Clone, Copy)]
pub(crate) struct RowMarks {
    domains: usize,
    opaque_representations: usize,
    cleanup_rows: usize,
}

/// One attempt's consultations, and the rows it changed as they stood when it
/// finished. A replayable fork also carries what replaying it needs.
pub(crate) struct ShapeEdits {
    consulted_types: BTreeSet<TypeIdentityId>,
    consulted_every_type: bool,
    consulted_domains: BTreeSet<(u32, TypeIdentityId)>,
    types: Vec<(TypeIdentityId, Option<CheckedUnitStructuralTypePlan>)>,
    in_progress: Vec<(TypeIdentityId, bool)>,
    domains_in_progress: Vec<((u32, TypeIdentityId), bool)>,
    domains: Vec<CheckedUnitStructuralDomainPlan>,
    opaque_representations: Vec<checked_trees::CheckedOpaqueRepresentation>,
    completed_requests: Vec<(usize, TypeShapeRequest, TypeIdentityId)>,
    byte_view_elements: Vec<TypeIdentityId>,
    unselected_opaque: Option<SymbolHandle>,
    cleanup_control: Option<checked_trees::CheckedCleanupControlPlans>,
}

/// The completed-request memo, shared between a collector and its forks.
///
/// The memo has one slot per type-reference arena row, so it grows with the
/// program. Each body plans on a fork, and copying the whole memo into every
/// fork made planning quadratic in the number of machines. Forks now share
/// the memo and keep only what they complete themselves in `local`, which is
/// also exactly what replaying the fork needs. A collector that holds the
/// only reference extends the shared rows in place.
#[derive(Clone, Default)]
pub(crate) struct CompletedRequests {
    shared: std::sync::Arc<Vec<Vec<(TypeShapeRequest, TypeIdentityId)>>>,
    /// Requests completed while `shared` was shared, in completion order.
    /// Every one of them follows every shared request of its slot.
    local: Vec<(usize, TypeShapeRequest, TypeIdentityId)>,
}

impl CompletedRequests {
    /// The first completed request in `slot` that `matches`, in completion order.
    pub(super) fn find(
        &self,
        slot: usize,
        mut matches: impl FnMut(&TypeShapeRequest) -> bool,
    ) -> Option<TypeIdentityId> {
        let shared = self.shared.get(slot).into_iter().flatten();
        let local = self
            .local
            .iter()
            .filter(|(owner, _, _)| *owner == slot)
            .map(|(_, request, identity)| (request, identity));
        shared
            .map(|(request, identity)| (request, identity))
            .chain(local)
            .find_map(|(request, identity)| matches(request).then_some(*identity))
    }

    pub(super) fn push(
        &mut self,
        slot: usize,
        request: TypeShapeRequest,
        identity: TypeIdentityId,
    ) {
        // Shared rows precede local ones, so only an empty local log may
        // extend them without reordering a slot.
        if self.local.is_empty()
            && let Some(rows) = std::sync::Arc::get_mut(&mut self.shared)
        {
            if rows.len() <= slot {
                rows.resize_with(slot + 1, Vec::new);
            }
            rows[slot].push((request, identity));
        } else {
            self.local.push((slot, request, identity));
        }
    }

    fn slot_len(&self, slot: usize) -> usize {
        self.shared.get(slot).map_or(0, Vec::len)
            + self
                .local
                .iter()
                .filter(|(owner, _, _)| *owner == slot)
                .count()
    }

    /// Every request completed here beyond what `base` holds in each slot.
    fn since(&self, base: &Self) -> Vec<(usize, TypeShapeRequest, TypeIdentityId)> {
        // A fork of `base` shares its rows and extends its local log.
        if std::sync::Arc::ptr_eq(&self.shared, &base.shared)
            && self.local.len() >= base.local.len()
        {
            return self.local[base.local.len()..].to_vec();
        }
        let slots = self.shared.len().max(
            self.local
                .iter()
                .map(|(slot, _, _)| slot + 1)
                .max()
                .unwrap_or(0),
        );
        let mut since = Vec::new();
        for slot in 0..slots {
            let known = base.slot_len(slot);
            let shared = self.shared.get(slot).into_iter().flatten();
            let local = self
                .local
                .iter()
                .filter(|(owner, _, _)| *owner == slot)
                .map(|(_, request, identity)| (request, identity));
            since.extend(
                shared
                    .map(|(request, identity)| (request, identity))
                    .chain(local)
                    .skip(known)
                    .map(|(request, identity)| (slot, request.clone(), *identity)),
            );
        }
        since
    }
}

/// The roster position at which each collector row last changed.
#[derive(Default)]
pub(crate) struct ChangeLog {
    types: BTreeMap<TypeIdentityId, usize>,
    domains: BTreeMap<(u32, TypeIdentityId), usize>,
    last_type_change: Option<usize>,
    last_cleanup_change: Option<usize>,
}

impl ChangeLog {
    /// A fork based after the first `base` bodies is the sequential run of
    /// its body when nothing it consulted changed at a later position.
    pub(crate) fn admits(&self, edits: &ShapeEdits, base: usize) -> bool {
        let before_base = |position: Option<&usize>| position.is_none_or(|changed| *changed < base);
        (edits.cleanup_control.is_none() || before_base(self.last_cleanup_change.as_ref()))
            && (!edits.consulted_every_type || before_base(self.last_type_change.as_ref()))
            && edits
                .consulted_types
                .iter()
                .all(|identity| before_base(self.types.get(identity)))
            && edits
                .consulted_domains
                .iter()
                .all(|key| before_base(self.domains.get(key)))
    }

    pub(crate) fn record(&mut self, edits: &ShapeEdits, position: usize) {
        let types = edits
            .types
            .iter()
            .map(|(identity, _)| *identity)
            .chain(edits.in_progress.iter().map(|(identity, _)| *identity))
            .chain(edits.opaque_representations.iter().map(|row| row.opaque))
            .collect::<Vec<_>>();
        if !types.is_empty() {
            self.last_type_change = Some(position);
        }
        self.types
            .extend(types.into_iter().map(|identity| (identity, position)));
        self.domains.extend(
            edits
                .domains_in_progress
                .iter()
                .map(|(key, _)| *key)
                .chain(
                    edits
                        .domains
                        .iter()
                        .map(|row| (row.domain.0, row.carrier_type_identity)),
                )
                .map(|key| (key, position)),
        );
        if edits.cleanup_control.is_some() {
            self.last_cleanup_change = Some(position);
        }
    }
}

impl ShapeCollector<'_> {
    /// Build the declaration index every fork would otherwise rebuild.
    pub(crate) fn prepare_forks(&mut self) {
        self.canonical_carriers();
    }

    /// A copy for planning one body: the same catalogs, nothing consulted.
    pub(crate) fn fork(&self) -> Self {
        let mut fork = self.clone();
        fork.forget_consulted();
        fork
    }

    pub(crate) fn forget_consulted(&mut self) {
        self.types.forget();
        self.in_progress.0.forget();
        self.domains_in_progress.0.forget();
        self.consulted_domains.clear();
    }

    pub(crate) fn row_marks(&self) -> RowMarks {
        RowMarks {
            domains: self.domains.len(),
            opaque_representations: self.opaque_representations.len(),
            cleanup_rows: self.cleanup_rows(),
        }
    }

    /// Cleanup arenas only grow while bodies are planned.
    fn cleanup_rows(&self) -> usize {
        let control = &self.cleanup_control;
        control.conditions.len()
            + control.paths.len()
            + control.normal_exit_rows.len()
            + control.value_completions.len()
            + control.actions.len()
            + control.normal_exit_regions.len()
            + control.reference_owner_cleanups.len()
    }

    /// What this attempt consulted and wrote since `marks`. A fork passes its
    /// `base` so the edits carry what replaying it needs.
    pub(crate) fn edits_since(&mut self, base: Option<&Self>, marks: RowMarks) -> ShapeEdits {
        let (consulted_types, written_types) = self.types.forget();
        let (consulted_progress, written_progress) = self.in_progress.0.forget();
        let (consulted_domain_progress, written_domain_progress) =
            self.domains_in_progress.0.forget();
        let mut consulted_domains = std::mem::take(&mut self.consulted_domains);
        consulted_domains.extend(consulted_domain_progress.keys);
        let cleanup_changed = self.cleanup_rows() != marks.cleanup_rows;
        let mut edits = ShapeEdits {
            consulted_every_type: consulted_types.every_key,
            consulted_types: consulted_types
                .keys
                .into_iter()
                .chain(consulted_progress.keys)
                .collect(),
            consulted_domains,
            types: written_types,
            in_progress: written_progress
                .into_iter()
                .map(|(identity, now)| (identity, now.is_some()))
                .collect(),
            domains_in_progress: written_domain_progress
                .into_iter()
                .map(|(key, now)| (key, now.is_some()))
                .collect(),
            domains: self.domains[marks.domains..].to_vec(),
            opaque_representations: self.opaque_representations[marks.opaque_representations..]
                .to_vec(),
            completed_requests: Vec::new(),
            byte_view_elements: Vec::new(),
            unselected_opaque: self.unselected_opaque,
            cleanup_control: cleanup_changed.then(Default::default),
        };
        let Some(base) = base else {
            return edits;
        };
        edits.completed_requests = self.completed_requests.since(&base.completed_requests);
        edits.byte_view_elements = self
            .byte_view_elements
            .difference(&base.byte_view_elements)
            .copied()
            .collect();
        if cleanup_changed {
            edits.cleanup_control = Some(std::mem::take(&mut self.cleanup_control));
        }
        edits
    }

    /// Replay an admitted fork's writes, as its sequential run made them.
    pub(crate) fn apply(&mut self, edits: ShapeEdits) {
        for (identity, row) in edits.types {
            match row {
                Some(row) => self.types.rows.insert(identity, row),
                None => self.types.rows.remove(&identity),
            };
        }
        for (identity, present) in edits.in_progress {
            if present {
                self.in_progress.0.rows.insert(identity, ());
            } else {
                self.in_progress.0.rows.remove(&identity);
            }
        }
        for (key, present) in edits.domains_in_progress {
            if present {
                self.domains_in_progress.0.rows.insert(key, ());
            } else {
                self.domains_in_progress.0.rows.remove(&key);
            }
        }
        self.domains.extend(edits.domains);
        self.opaque_representations
            .extend(edits.opaque_representations);
        for (slot, request, identity) in edits.completed_requests {
            self.completed_requests.push(slot, request, identity);
        }
        self.byte_view_elements.extend(edits.byte_view_elements);
        self.unselected_opaque = edits.unselected_opaque;
        // Admitted only when no body appended since this fork's base, so its
        // arenas extend the merged ones exactly.
        if let Some(cleanup_control) = edits.cleanup_control {
            self.cleanup_control = cleanup_control;
        }
    }
}
