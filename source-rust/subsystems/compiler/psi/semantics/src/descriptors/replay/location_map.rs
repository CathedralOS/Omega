//! Checking a later write must preserve the storage origins remembered for
//! earlier operations. Share unchanged groups of roots between saved states
//! so a write copies its group instead of the entire machine's map.

use std::collections::{BTreeMap, btree_map};
use std::sync::Arc;

use crate::descriptors::descriptor_lifetimes::DescriptorLocation;

const ROOTS_PER_PAGE: u64 = 64;
type Page<Value> = (u64, Arc<BTreeMap<DescriptorLocation, Value>>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocationMap<Value> {
    // Sorted page coordinates preserve ordinary location ordering. Keeping
    // only actual pages also bounds storage for sparse, untrusted root IDs.
    pages: Vec<Page<Value>>,
}

impl<Value> Default for LocationMap<Value> {
    fn default() -> Self {
        Self { pages: Vec::new() }
    }
}

impl<Value> LocationMap<Value> {
    pub(super) fn new() -> Self {
        Self::default()
    }

    fn page(&self, location: &DescriptorLocation) -> Result<usize, usize> {
        let page = (location.root.get() - 1) / ROOTS_PER_PAGE;
        self.pages.binary_search_by_key(&page, |(page, _)| *page)
    }

    pub(super) fn get(&self, location: &DescriptorLocation) -> Option<&Value> {
        self.pages.get(self.page(location).ok()?)?.1.get(location)
    }

    pub(super) fn contains_key(&self, location: &DescriptorLocation) -> bool {
        self.get(location).is_some()
    }

    pub(super) fn iter(&self) -> Iter<'_, Value> {
        Iter {
            pages: self.pages.iter(),
            current: None,
        }
    }

    /// At a join, rows on a page both branches share already have the same value.
    /// Visit only pages that changed or are absent on the other branch.
    pub(super) fn unshared_entries<'maps>(
        &'maps self,
        other: &'maps Self,
    ) -> impl Iterator<Item = (&'maps DescriptorLocation, &'maps Value)> {
        self.pages
            .iter()
            .filter(move |(number, page)| {
                let Ok(index) = other.pages.binary_search_by_key(number, |(key, _)| *key) else {
                    return true;
                };
                !Arc::ptr_eq(page, &other.pages[index].1)
            })
            .flat_map(|(_, page)| page.iter())
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &DescriptorLocation> {
        self.iter().map(|(location, _)| location)
    }
}

impl<Value: Clone> LocationMap<Value> {
    pub(super) fn get_mut(&mut self, location: &DescriptorLocation) -> Option<&mut Value> {
        let index = self.page(location).ok()?;
        let page = &mut self.pages.get_mut(index)?.1;
        if !page.contains_key(location) {
            return None;
        }
        Arc::make_mut(page).get_mut(location)
    }

    pub(super) fn insert(&mut self, location: DescriptorLocation, value: Value) -> Option<Value> {
        let index = match self.page(&location) {
            Ok(index) => index,
            Err(index) => {
                self.pages.insert(
                    index,
                    (
                        (location.root.get() - 1) / ROOTS_PER_PAGE,
                        Arc::new(BTreeMap::new()),
                    ),
                );
                index
            }
        };
        Arc::make_mut(&mut self.pages[index].1).insert(location, value)
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&DescriptorLocation, &Value) -> bool) {
        self.pages.retain_mut(|(_, page)| {
            let mut removed = Vec::new();
            for (location, value) in page.iter() {
                if !keep(location, value) {
                    removed.push(location.clone());
                }
            }
            if removed.is_empty() {
                return true;
            }
            if removed.len() == page.len() {
                return false;
            }
            let page = Arc::make_mut(page);
            for location in removed {
                page.remove(&location);
            }
            !page.is_empty()
        });
    }

    /// Intersects two maps by key, reusing shared pages in O(1) when both
    /// branches point at the exact same `Arc` page and skipping pages absent
    /// from either side.
    pub(super) fn intersect_with(
        &self,
        other: &Self,
        mut merge: impl FnMut(&Value, &Value) -> Value,
    ) -> Self {
        let mut pages = Vec::new();
        let mut left_index = 0;
        let mut right_index = 0;
        while left_index < self.pages.len() && right_index < other.pages.len() {
            let (left_num, left_page) = &self.pages[left_index];
            let (right_num, right_page) = &other.pages[right_index];
            match left_num.cmp(right_num) {
                std::cmp::Ordering::Less => left_index += 1,
                std::cmp::Ordering::Greater => right_index += 1,
                std::cmp::Ordering::Equal => {
                    if Arc::ptr_eq(left_page, right_page) {
                        pages.push((*left_num, Arc::clone(left_page)));
                    } else {
                        let mut merged = BTreeMap::new();
                        for (location, left_value) in left_page.iter() {
                            if let Some(right_value) = right_page.get(location) {
                                merged.insert(location.clone(), merge(left_value, right_value));
                            }
                        }
                        if !merged.is_empty() {
                            pages.push((*left_num, Arc::new(merged)));
                        }
                    }
                    left_index += 1;
                    right_index += 1;
                }
            }
        }
        Self { pages }
    }
}

impl<Value: Clone> Extend<(DescriptorLocation, Value)> for LocationMap<Value> {
    fn extend<Items: IntoIterator<Item = (DescriptorLocation, Value)>>(&mut self, items: Items) {
        for (location, value) in items {
            self.insert(location, value);
        }
    }
}

impl<Value: Clone> FromIterator<(DescriptorLocation, Value)> for LocationMap<Value> {
    fn from_iter<Items: IntoIterator<Item = (DescriptorLocation, Value)>>(items: Items) -> Self {
        let mut locations = Self::new();
        locations.extend(items);
        locations
    }
}

impl<Value: Clone, const COUNT: usize> From<[(DescriptorLocation, Value); COUNT]>
    for LocationMap<Value>
{
    fn from(items: [(DescriptorLocation, Value); COUNT]) -> Self {
        items.into_iter().collect()
    }
}

pub(crate) struct Iter<'map, Value> {
    pages: std::slice::Iter<'map, Page<Value>>,
    current: Option<btree_map::Iter<'map, DescriptorLocation, Value>>,
}

impl<'map, Value> Iterator for Iter<'map, Value> {
    type Item = (&'map DescriptorLocation, &'map Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.current.as_mut().and_then(Iterator::next) {
                return Some(value);
            }
            self.current = Some(self.pages.next()?.1.iter());
        }
    }
}

impl<'map, Value> IntoIterator for &'map LocationMap<Value> {
    type Item = (&'map DescriptorLocation, &'map Value);
    type IntoIter = Iter<'map, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub(crate) struct IntoIter<Value> {
    pages: std::vec::IntoIter<Page<Value>>,
    current: Option<btree_map::IntoIter<DescriptorLocation, Value>>,
}

impl<Value: Clone> Iterator for IntoIter<Value> {
    type Item = (DescriptorLocation, Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.current.as_mut().and_then(Iterator::next) {
                return Some(value);
            }
            let (_, page) = self.pages.next()?;
            let page = Arc::try_unwrap(page).unwrap_or_else(|shared| (*shared).clone());
            self.current = Some(page.into_iter());
        }
    }
}

impl<Value: Clone> IntoIterator for LocationMap<Value> {
    type Item = (DescriptorLocation, Value);
    type IntoIter = IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            pages: self.pages.into_iter(),
            current: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
    use semantic_vocabulary::{PlaceId, StructuralFieldId};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn root(id: u64) -> DescriptorLocation {
        DescriptorLocation::whole(PlaceId::new(id).unwrap())
    }

    #[derive(Debug)]
    struct Counted {
        value: u64,
        copies: Arc<AtomicUsize>,
    }

    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.copies.fetch_add(1, Ordering::Relaxed);
            Self {
                value: self.value,
                copies: Arc::clone(&self.copies),
            }
        }
    }

    #[test]
    fn changing_one_snapshot_keeps_history_without_copying_every_root() {
        let copies = Arc::new(AtomicUsize::new(0));
        let before = (1..=1024)
            .map(|id| {
                (
                    root(id),
                    Counted {
                        value: id,
                        copies: Arc::clone(&copies),
                    },
                )
            })
            .collect::<LocationMap<_>>();
        let mut after = before.clone();
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        after.get_mut(&root(65)).unwrap().value = 700;
        assert_eq!(copies.load(Ordering::Relaxed), ROOTS_PER_PAGE as usize);
        assert_eq!(
            after.unshared_entries(&before).count(),
            ROOTS_PER_PAGE as usize
        );
        assert_eq!(before.get(&root(65)).unwrap().value, 65);
        assert_eq!(after.get(&root(65)).unwrap().value, 700);
        assert_eq!(after.get(&root(64)).unwrap().value, 64);
        assert!(Arc::ptr_eq(&before.pages[0].1, &after.pages[0].1));
        assert!(Arc::ptr_eq(&before.pages[2].1, &after.pages[2].1));
        // Saving another state lets a field change without changing the whole
        // root beside it or either earlier state.
        let mut projected = root(65);
        projected.path.push(DescriptorPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        ));
        let mut last = after.clone();
        last.insert(
            projected.clone(),
            Counted {
                value: 9,
                copies: Arc::clone(&copies),
            },
        );
        assert!(before.get(&projected).is_none());
        assert!(after.get(&projected).is_none());
        assert_eq!(last.get(&projected).unwrap().value, 9);
        assert_eq!(last.get(&root(65)).unwrap().value, 700);
    }

    #[test]
    fn sparse_roots_and_removal_keep_canonical_contents() {
        let mut locations = LocationMap::from([
            (root(u64::MAX), Some(7)),
            (root(65), None),
            (root(1), Some(9)),
        ]);
        assert_eq!(locations.pages.len(), 3);
        let before = locations.clone();
        locations.retain(|location, _| location.root.get() != 65);
        assert_eq!(before.get(&root(65)), Some(&None));
        assert_eq!(locations.get(&root(65)), None);
        assert_eq!(
            before
                .unshared_entries(&locations)
                .map(|(key, _)| key.root.get())
                .collect::<Vec<_>>(),
            [65]
        );
        assert!(locations.unshared_entries(&before).next().is_none());
        assert_eq!(
            locations
                .iter()
                .map(|(key, value)| (key.root.get(), *value))
                .collect::<Vec<_>>(),
            [(1, Some(9)), (u64::MAX, Some(7))]
        );
        locations.retain(|_, _| false);
        assert_eq!(locations, LocationMap::new());
        assert_eq!(
            before
                .into_iter()
                .map(|(key, value)| (key.root.get(), value))
                .collect::<Vec<_>>(),
            [(1, Some(9)), (65, None), (u64::MAX, Some(7))]
        );
    }
}
