//! Replay origins partitioned by their actual storage root. A write detaches
//! only that root's rows; earlier cuts retain every missing/unknown distinction
//! and iterate the same complete key order as the former flat BTreeMap.
use super::replay::Value;
use super::{DescriptorContentsQuery, DescriptorLocation, PlaceId};
use std::collections::BTreeMap;
use std::sync::Arc;

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
type RootRows<Key> = BTreeMap<PlaceId, Arc<BTreeMap<Key, Value>>>;
fn page(root: PlaceId) -> u64 {
    root.get() >> ROOT_PAGE_SHIFT
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OriginMap<Key: OriginKey> {
    pages: BTreeMap<u64, Arc<RootRows<Key>>>,
}
impl<Key: OriginKey> Default for OriginMap<Key> {
    fn default() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }
}
impl<Key: OriginKey> OriginMap<Key> {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Value> {
        self.pages
            .get(&page(key.root()))?
            .get(&key.root())?
            .get(key)
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        self.get(key)?;
        let roots = Arc::make_mut(self.pages.get_mut(&page(key.root()))?);
        Arc::make_mut(roots.get_mut(&key.root())?).get_mut(key)
    }
    pub(super) fn insert(&mut self, key: Key, value: Value) -> Option<Value> {
        let roots = Arc::make_mut(self.pages.entry(page(key.root())).or_default());
        let rows = roots.entry(key.root()).or_default();
        Arc::make_mut(rows).insert(key, value)
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (&Key, &Value)> {
        self.pages
            .values()
            .flat_map(|roots| roots.values())
            .flat_map(|rows| rows.iter())
    }
    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &Key> {
        self.pages
            .values()
            .flat_map(|roots| roots.values())
            .flat_map(|rows| rows.keys())
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Key, &Value) -> bool) {
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
    }
}
impl<Key: OriginKey, const N: usize> From<[(Key, Value); N]> for OriginMap<Key> {
    fn from(rows: [(Key, Value); N]) -> Self {
        rows.into_iter().collect()
    }
}
impl<Key: OriginKey> FromIterator<(Key, Value)> for OriginMap<Key> {
    fn from_iter<T: IntoIterator<Item = (Key, Value)>>(rows: T) -> Self {
        let mut map = Self::new();
        for (key, value) in rows {
            map.insert(key, value);
        }
        map
    }
}

pub(super) struct IntoIter<Key: OriginKey> {
    pages: std::collections::btree_map::IntoValues<u64, Arc<RootRows<Key>>>,
    roots: Option<std::collections::btree_map::IntoValues<PlaceId, Arc<BTreeMap<Key, Value>>>>,
    current: Option<std::collections::btree_map::IntoIter<Key, Value>>,
}
impl<Key: OriginKey> Iterator for IntoIter<Key> {
    type Item = (Key, Value);
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
impl<Key: OriginKey> IntoIterator for OriginMap<Key> {
    type Item = (Key, Value);
    type IntoIter = IntoIter<Key>;
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

    fn location(root: u64) -> DescriptorLocation {
        DescriptorLocation::whole(PlaceId::new(root).unwrap())
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
