//! Ordered copy-on-write pages inside one storage root.
//!
//! A wide aggregate can have many descriptor slots on one root. Keeping a
//! snapshot must not make its next write copy every immutable neighboring slot.
//! A write detaches one bounded row page; the directory keeps complete key order.
//! Page boundaries are storage choices, so equality compares logical entries.
//! Writers use Arc::make_mut: a shared page identity therefore proves that all
//! its raw entries are unchanged. This shortcut never normalizes symbolic facts.
use super::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
const PAGE_ROWS: usize = 64;
#[derive(Clone, Debug)]
pub(super) struct RowPages<Key: Ord + Clone> {
    pages: BTreeMap<Key, Arc<BTreeMap<Key, Value>>>,
}
impl<Key: Ord + Clone> Default for RowPages<Key> {
    fn default() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }
}
impl<Key: Ord + Clone> PartialEq for RowPages<Key> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl<Key: Ord + Clone> Eq for RowPages<Key> {}
impl<Key: Ord + Clone> RowPages<Key> {
    fn floor(&self, key: &Key) -> Option<Key> {
        self.pages
            .range(..=key)
            .next_back()
            .map(|(key, _)| key.clone())
    }
    pub(super) fn keys_in_changed_pages(&self, other: &Self) -> BTreeSet<Key> {
        let mut keys = BTreeSet::new();
        for (floor, rows) in &self.pages {
            if !other
                .pages
                .get(floor)
                .is_some_and(|peer| Arc::ptr_eq(rows, peer))
            {
                keys.extend(rows.keys().cloned());
            }
        }
        for (floor, rows) in &other.pages {
            if !self
                .pages
                .get(floor)
                .is_some_and(|peer| Arc::ptr_eq(rows, peer))
            {
                keys.extend(rows.keys().cloned());
            }
        }
        keys
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Value> {
        self.pages.range(..=key).next_back()?.1.get(key)
    }
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        let floor = self.floor(key)?;
        Arc::make_mut(self.pages.get_mut(&floor)?).get_mut(key)
    }
    pub(super) fn insert(&mut self, key: Key, value: Value) -> Option<Value> {
        let mut floor = self
            .floor(&key)
            .or_else(|| self.pages.first_key_value().map(|(key, _)| key.clone()));
        if floor.as_ref().is_some_and(|floor| &key < floor) {
            let rows = self.pages.remove(floor.as_ref().unwrap()).unwrap();
            self.pages.insert(key.clone(), rows);
            floor = Some(key.clone());
        }
        let floor = floor.unwrap_or_else(|| key.clone());
        let rows = Arc::make_mut(self.pages.entry(floor).or_default());
        let previous = rows.insert(key, value);
        if rows.len() > PAGE_ROWS {
            let middle = rows.keys().nth(rows.len() / 2).unwrap().clone();
            let upper = rows.split_off(&middle);
            self.pages.insert(middle, Arc::new(upper));
        }
        previous
    }
    pub(super) fn remove(&mut self, key: &Key) -> Option<Value> {
        let floor = self.floor(key)?;
        let rows = Arc::make_mut(self.pages.get_mut(&floor)?);
        let previous = rows.remove(key);
        if rows.is_empty() {
            self.pages.remove(&floor);
        }
        previous
    }
    pub(super) fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (&Key, &Value)> {
        self.pages.values().flat_map(|rows| rows.iter())
    }
    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &Key> {
        self.pages.values().flat_map(|rows| rows.keys())
    }
}
pub(super) struct IntoIter<Key: Ord + Clone> {
    pages: std::collections::btree_map::IntoValues<Key, Arc<BTreeMap<Key, Value>>>,
    current: Option<std::collections::btree_map::IntoIter<Key, Value>>,
}
impl<Key: Ord + Clone> Iterator for IntoIter<Key> {
    type Item = (Key, Value);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(row) = self.current.as_mut().and_then(Iterator::next) {
                return Some(row);
            }
            let rows = self.pages.next()?;
            let rows = Arc::try_unwrap(rows).unwrap_or_else(|shared| shared.as_ref().clone());
            self.current = Some(rows.into_iter());
        }
    }
}
impl<Key: Ord + Clone> IntoIterator for RowPages<Key> {
    type Item = (Key, Value);
    type IntoIter = IntoIter<Key>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            pages: self.pages.into_values(),
            current: None,
        }
    }
}
