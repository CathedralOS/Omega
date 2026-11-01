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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OriginMap<Key: OriginKey> {
    roots: BTreeMap<PlaceId, Arc<BTreeMap<Key, Value>>>,
}
impl<Key: OriginKey> Default for OriginMap<Key> {
    fn default() -> Self {
        Self {
            roots: BTreeMap::new(),
        }
    }
}
impl<Key: OriginKey> OriginMap<Key> {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Value> {
        self.roots.get(&key.root())?.get(key)
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        let rows = self.roots.get_mut(&key.root())?;
        if !rows.contains_key(key) {
            return None;
        }
        Arc::make_mut(rows).get_mut(key)
    }
    pub(super) fn insert(&mut self, key: Key, value: Value) -> Option<Value> {
        let rows = self.roots.entry(key.root()).or_default();
        Arc::make_mut(rows).insert(key, value)
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (&Key, &Value)> {
        self.roots.values().flat_map(|rows| rows.iter())
    }
    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &Key> {
        self.roots.values().flat_map(|rows| rows.keys())
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Key, &Value) -> bool) {
        self.roots.retain(|_, rows| {
            let removed: Vec<_> = rows
                .iter()
                .filter_map(|(key, value)| (!keep(key, value)).then(|| key.clone()))
                .collect();
            if !removed.is_empty() {
                let rows = Arc::make_mut(rows);
                for key in removed {
                    rows.remove(&key);
                }
            }
            !rows.is_empty()
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
    roots: std::collections::btree_map::IntoValues<PlaceId, Arc<BTreeMap<Key, Value>>>,
    current: Option<std::collections::btree_map::IntoIter<Key, Value>>,
}
impl<Key: OriginKey> Iterator for IntoIter<Key> {
    type Item = (Key, Value);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current.as_mut().and_then(Iterator::next) {
                return Some(next);
            }
            let rows = self.roots.next()?;
            let rows = Arc::try_unwrap(rows).unwrap_or_else(|shared| shared.as_ref().clone());
            self.current = Some(rows.into_iter());
        }
    }
}
impl<Key: OriginKey> IntoIterator for OriginMap<Key> {
    type Item = (Key, Value);
    type IntoIter = IntoIter<Key>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            roots: self.roots.into_values(),
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
            .roots
            .iter()
            .filter(|(root, rows)| !Arc::ptr_eq(rows, &before.roots[root]))
            .count();
        assert_eq!(detached, 1);
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
