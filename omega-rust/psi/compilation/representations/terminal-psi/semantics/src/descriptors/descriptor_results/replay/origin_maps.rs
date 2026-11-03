//! Replay origins partitioned by their actual storage root. A write detaches
//! only that root's rows; earlier cuts retain every missing/unknown distinction
//! and iterate the same complete key order as the former flat BTreeMap.
use super::replay::Value;
use super::{DescriptorContentsQuery, DescriptorLocation, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
mod row_pages;
use row_pages::RowPages;

/// Storage root is the leading coordinate of both concrete key orderings.
/// Partitioning therefore preserves their complete flat ordered iteration.
pub(super) trait OriginKey: Ord + Clone {
    fn root(&self) -> PlaceId;
}
impl OriginKey for DescriptorLocation {
    fn root(&self) -> PlaceId {
        self.root
    }
}
impl OriginKey for DescriptorContentsQuery {
    fn root(&self) -> PlaceId {
        self.root
    }
}

// Page only the directory of immutable root rows. A snapshot write copies
// at most one 64-root directory page instead of every root in a large body.
// Numeric pages preserve PlaceId order; they carry no provenance meaning.
const ROOT_PAGE_SHIFT: u32 = 6;
type RootRows<Key, Element> = BTreeMap<PlaceId, Arc<RowPages<Key, Element>>>;
fn page(root: PlaceId) -> u64 {
    root.get() >> ROOT_PAGE_SHIFT
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OriginMap<Key: OriginKey, Element: Clone + Eq = Value> {
    pages: BTreeMap<u64, Arc<RootRows<Key, Element>>>,
    entries: usize,
}
impl<Key: OriginKey, Element: Clone + Eq> Default for OriginMap<Key, Element> {
    fn default() -> Self {
        Self {
            pages: BTreeMap::new(),
            entries: 0,
        }
    }
}
impl<Key: OriginKey, Element: Clone + Eq> OriginMap<Key, Element> {
    /// Every differing raw entry is included. Identical immutable pages can
    /// be skipped; unchanged neighbors in a detached page may also be returned.
    pub(super) fn keys_in_changed_pages(&self, other: &Self) -> BTreeSet<Key> {
        let mut keys = BTreeSet::new();
        if std::ptr::eq(self, other) {
            return keys;
        }
        for (page, roots) in &self.pages {
            let peers = other.pages.get(page);
            if peers.is_some_and(|peer| Arc::ptr_eq(roots, peer)) {
                continue;
            }
            for (root, rows) in roots.iter() {
                match peers.and_then(|peer| peer.get(root)) {
                    Some(peer) if Arc::ptr_eq(rows, peer) => {}
                    Some(peer) => keys.extend(rows.keys_in_changed_pages(peer)),
                    None => keys.extend(rows.keys().cloned()),
                }
            }
        }
        for (page, roots) in &other.pages {
            let peers = self.pages.get(page);
            if peers.is_some_and(|peer| Arc::ptr_eq(roots, peer)) {
                continue;
            }
            for (root, rows) in roots.iter() {
                if peers.and_then(|peer| peer.get(root)).is_none() {
                    keys.extend(rows.keys().cloned());
                }
            }
        }
        keys
    }
    pub(super) fn len(&self) -> usize {
        self.entries
    }
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Element> {
        self.pages
            .get(&page(key.root()))?
            .get(&key.root())?
            .get(key)
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Element> {
        self.get(key)?;
        let roots = Arc::make_mut(self.pages.get_mut(&page(key.root()))?);
        Arc::make_mut(roots.get_mut(&key.root())?).get_mut(key)
    }
    pub(super) fn insert(&mut self, key: Key, value: Element) -> Option<Element> {
        let roots = Arc::make_mut(self.pages.entry(page(key.root())).or_default());
        let rows = roots.entry(key.root()).or_default();
        let previous = Arc::make_mut(rows).insert(key, value);
        self.entries += usize::from(previous.is_none());
        previous
    }
    pub(super) fn remove(&mut self, key: &Key) -> Option<Element> {
        self.get(key)?;
        let group = page(key.root());
        let roots = Arc::make_mut(self.pages.get_mut(&group)?);
        let rows = Arc::make_mut(roots.get_mut(&key.root())?);
        let previous = rows.remove(key);
        if rows.is_empty() {
            roots.remove(&key.root());
        }
        if roots.is_empty() {
            self.pages.remove(&group);
        }
        self.entries -= usize::from(previous.is_some());
        previous
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (&Key, &Element)> {
        self.pages
            .values()
            .flat_map(|roots| roots.values())
            .flat_map(|rows| rows.iter())
    }
    /// Contents overlap is confined to one exact storage root. Read that
    /// immutable row directly instead of scanning unrelated roots at every cut.
    pub(super) fn for_root(
        &self,
        root: PlaceId,
    ) -> impl DoubleEndedIterator<Item = (&Key, &Element)> {
        self.pages
            .get(&page(root))
            .and_then(|roots| roots.get(&root))
            .into_iter()
            .flat_map(|rows| rows.iter())
    }
    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &Key> {
        self.pages
            .values()
            .flat_map(|roots| roots.values())
            .flat_map(|rows| rows.keys())
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Key, &Element) -> bool) {
        let mut removed_entries = 0;
        self.pages.retain(|_, roots| {
            let changes: Vec<_> = roots
                .iter()
                .filter_map(|(root, rows)| {
                    let removed: Vec<_> = rows
                        .iter()
                        .filter_map(|(key, value)| (!keep(key, value)).then(|| key.clone()))
                        .collect();
                    (!removed.is_empty()).then_some((*root, removed))
                })
                .collect();
            if !changes.is_empty() {
                let roots = Arc::make_mut(roots);
                for (root, removed) in changes {
                    removed_entries += removed.len();
                    let rows = Arc::make_mut(roots.get_mut(&root).expect("retained root exists"));
                    for key in removed {
                        rows.remove(&key);
                    }
                    if rows.is_empty() {
                        roots.remove(&root);
                    }
                }
            }
            !roots.is_empty()
        });
        self.entries -= removed_entries;
    }
}
impl<Key: OriginKey, Element: Clone + Eq, const N: usize> From<[(Key, Element); N]>
    for OriginMap<Key, Element>
{
    fn from(rows: [(Key, Element); N]) -> Self {
        rows.into_iter().collect()
    }
}
impl<Key: OriginKey, Element: Clone + Eq> FromIterator<(Key, Element)> for OriginMap<Key, Element> {
    fn from_iter<T: IntoIterator<Item = (Key, Element)>>(rows: T) -> Self {
        let mut map = Self::new();
        for (key, value) in rows {
            map.insert(key, value);
        }
        map
    }
}

pub(super) struct IntoIter<Key: OriginKey, Element: Clone + Eq> {
    pages: std::collections::btree_map::IntoValues<u64, Arc<RootRows<Key, Element>>>,
    roots: Option<std::collections::btree_map::IntoValues<PlaceId, Arc<RowPages<Key, Element>>>>,
    current: Option<row_pages::IntoIter<Key, Element>>,
}
impl<Key: OriginKey, Element: Clone + Eq> Iterator for IntoIter<Key, Element> {
    type Item = (Key, Element);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current.as_mut().and_then(Iterator::next) {
                return Some(next);
            }
            if let Some(rows) = self.roots.as_mut().and_then(Iterator::next) {
                let rows = Arc::try_unwrap(rows).unwrap_or_else(|shared| shared.as_ref().clone());
                self.current = Some(rows.into_iter());
                continue;
            }
            let roots = self.pages.next()?;
            let roots = Arc::try_unwrap(roots).unwrap_or_else(|shared| shared.as_ref().clone());
            self.roots = Some(roots.into_values());
        }
    }
}
impl<Key: OriginKey, Element: Clone + Eq> IntoIterator for OriginMap<Key, Element> {
    type Item = (Key, Element);
    type IntoIter = IntoIter<Key, Element>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            pages: self.pages.into_values(),
            roots: None,
            current: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::replay::Origin;
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn snapshot_writes_share_untouched_payloads_inside_a_detached_page() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Counted {
            payload: Vec<u64>,
            copies: Arc<AtomicUsize>,
        }
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.copies.fetch_add(1, Ordering::Relaxed);
                Self {
                    payload: self.payload.clone(),
                    copies: self.copies.clone(),
                }
            }
        }
        impl PartialEq for Counted {
            fn eq(&self, other: &Self) -> bool {
                self.payload == other.payload
            }
        }
        impl Eq for Counted {}
        let key = |ordinal| {
            let mut key = location(1);
            key.path
                .push(crate::descriptor_lifetimes::DescriptorPathSegment::FixedIndex(ordinal));
            key
        };
        let copies = Arc::new(AtomicUsize::new(0));
        let value = |ordinal| Counted {
            payload: vec![ordinal; 128],
            copies: copies.clone(),
        };
        let mut map = OriginMap::new();
        for ordinal in 0..512 {
            map.insert(key(ordinal), value(ordinal));
        }
        let mut snapshots = Vec::new();
        for ordinal in 0..64 {
            snapshots.push(map.clone());
            let old = map.insert(key(ordinal), value(ordinal + 1000)).unwrap();
            assert_eq!(old.payload[0], ordinal);
        }
        // Returning the overwritten owned value clones that one shared cell;
        // the other entries in its detached page retain their original payloads.
        assert_eq!(copies.load(Ordering::Relaxed), 64);
        assert_eq!(snapshots[0].get(&key(0)).unwrap().payload[0], 0);
        let before_edit = map.clone();
        map.get_mut(&key(511)).unwrap().payload[0] = 9000;
        assert_eq!(copies.load(Ordering::Relaxed), 65);
        assert_eq!(before_edit.get(&key(511)).unwrap().payload[0], 511);
        assert_eq!(map.get(&key(511)).unwrap().payload[0], 9000);
        map.remove(&key(511));
        map.retain(|subject, _| subject != &key(510));
        assert_eq!(map.len(), 510);
        assert!(before_edit.get(&key(510)).is_some());
        assert!(map.get(&key(510)).is_none());
        assert_eq!(
            map.into_iter()
                .map(|(subject, _)| subject)
                .collect::<Vec<_>>(),
            (0..510).map(key).collect::<Vec<_>>()
        );
    }

    fn location(root: u64) -> DescriptorLocation {
        DescriptorLocation::whole(PlaceId::new(root).unwrap())
    }
    #[test]
    fn wide_root_pages_preserve_logical_order_and_earlier_snapshots() {
        let mut map = OriginMap::new();
        let mut reference = BTreeMap::new();
        for ordinal in 0..512 {
            let index = ordinal * 197 % 512;
            let mut key = location(1);
            key.path
                .push(crate::descriptor_lifetimes::DescriptorPathSegment::FixedIndex(index));
            let value =
                (index % 2 == 0).then(|| BTreeSet::from([Origin::Storage(location(5000 + index))]));
            map.insert(key.clone(), value.clone());
            reference.insert(key, value);
        }
        let snapshot = map.clone();
        let original = reference.clone();
        let mut reversed = OriginMap::new();
        for (key, value) in reference.iter().rev() {
            reversed.insert(key.clone(), value.clone());
        }
        assert_eq!(map, reversed);
        assert!(map.keys_in_changed_pages(&snapshot).is_empty());
        for (key, value) in reference.iter_mut().step_by(17) {
            *map.get_mut(key).unwrap() = None;
            *value = None;
        }
        let keep = |key: &DescriptorLocation| match key.path.last() {
            Some(crate::descriptor_lifetimes::DescriptorPathSegment::FixedIndex(index)) => {
                index % 7 != 0
            }
            _ => true,
        };
        map.retain(|key, _| keep(key));
        reference.retain(|key, _| keep(key));
        let possible = map.keys_in_changed_pages(&snapshot);
        for key in reference.keys().chain(original.keys()) {
            if reference.get(key) != original.get(key) {
                assert!(possible.contains(key));
            }
        }
        assert!(map.iter().eq(reference.iter()));
        assert!(map.iter().rev().eq(reference.iter().rev()));
        assert!(snapshot.iter().eq(original.iter()));
        for key in original.keys() {
            assert_eq!(map.get(key), reference.get(key));
        }
        assert!(map.into_iter().eq(reference.into_iter()));
        assert!(snapshot.into_iter().eq(original.into_iter()));
    }
    #[test]
    fn replay_writes_share_untouched_roots_and_preserve_unknown_and_absent_slots() {
        let mut map = OriginMap::new();
        let mut reference = BTreeMap::new();
        for root in 1..=2048 {
            map.insert(location(root), None);
            reference.insert(location(root), None);
        }
        let mut child = location(1);
        child
            .path
            .push(crate::descriptor_lifetimes::DescriptorPathSegment::Referent);
        map.insert(child.clone(), None);
        reference.insert(child.clone(), None);
        let before = map.clone();
        let value = Some(BTreeSet::from([Origin::Storage(location(3))]));
        map.insert(location(1), value.clone());
        reference.insert(location(1), value);
        *map.get_mut(&child).unwrap() = Some(BTreeSet::from([Origin::Storage(location(4))]));
        *reference.get_mut(&child).unwrap() = Some(BTreeSet::from([Origin::Storage(location(4))]));
        assert_eq!(before.get(&child), Some(&None));
        assert_eq!(map.get(&location(2)), Some(&None));
        assert_eq!(map.get(&location(2049)), None);
        assert_eq!(before.get(&location(1)), Some(&None));
        let detached = map
            .pages
            .values()
            .flat_map(|roots| roots.iter())
            .filter(|(root, rows)| !Arc::ptr_eq(rows, &before.pages[&page(**root)][root]))
            .count();
        assert_eq!(detached, 1);
        let detached_pages = map
            .pages
            .iter()
            .filter(|(key, roots)| !Arc::ptr_eq(roots, &before.pages[key]))
            .count();
        assert_eq!(detached_pages, 1);
        assert!(map.pages.values().all(|roots| roots.len() <= 64));
        assert_eq!(
            map.iter().collect::<Vec<_>>(),
            reference.iter().collect::<Vec<_>>()
        );
        map.retain(|key, _| key.root.get() % 2 == 0);
        reference.retain(|key, _| key.root.get() % 2 == 0);
        assert_eq!(
            map.into_iter().collect::<Vec<_>>(),
            reference.into_iter().collect::<Vec<_>>()
        );
        assert_eq!(before.keys().count(), 2049);
    }
}
