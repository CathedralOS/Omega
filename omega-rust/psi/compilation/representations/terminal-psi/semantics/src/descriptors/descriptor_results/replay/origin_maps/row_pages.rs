//! Ordered immutable cells for one storage root. Small roots use one sorted
//! vector, avoiding a page directory and tree allocation for a single slot.
//! Wide roots share bounded leaves. Keys and payloads remain shared across
//! snapshots; editing a value detaches only that cell. Page boundaries never
//! participate in logical equality or normalize symbolic contents facts.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
const PAGE_ROWS: usize = 64;
type Cell<Key, Element> = (Arc<Key>, Arc<Element>);
type Rows<Key, Element> = Vec<Cell<Key, Element>>;
#[derive(Clone, Debug)]
pub(super) enum RowPages<Key: Ord + Clone, Element: Clone + Eq> {
    Small(Rows<Key, Element>),
    Paged(BTreeMap<Key, Arc<Rows<Key, Element>>>),
}
impl<Key: Ord + Clone, Element: Clone + Eq> Default for RowPages<Key, Element> {
    fn default() -> Self {
        Self::Small(Vec::new())
    }
}
impl<Key: Ord + Clone, Element: Clone + Eq> PartialEq for RowPages<Key, Element> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl<Key: Ord + Clone, Element: Clone + Eq> Eq for RowPages<Key, Element> {}
fn position<Key: Ord, Element>(rows: &[Cell<Key, Element>], key: &Key) -> Result<usize, usize> {
    rows.binary_search_by(|(candidate, _)| candidate.as_ref().cmp(key))
}
fn insert<Key: Ord, Element>(
    rows: &mut Rows<Key, Element>,
    key: Key,
    value: Element,
) -> Option<Arc<Element>> {
    match position(rows, &key) {
        Ok(position) => Some(std::mem::replace(&mut rows[position].1, Arc::new(value))),
        Err(position) => {
            rows.insert(position, (Arc::new(key), Arc::new(value)));
            None
        }
    }
}
impl<Key: Ord + Clone, Element: Clone + Eq> RowPages<Key, Element> {
    pub(super) fn keys_in_changed_pages(&self, other: &Self) -> BTreeSet<Key> {
        let mut keys = BTreeSet::new();
        if std::ptr::eq(self, other) {
            return keys;
        }
        if let (Self::Paged(left), Self::Paged(right)) = (self, other) {
            for (floor, rows) in left {
                if !right.get(floor).is_some_and(|peer| Arc::ptr_eq(rows, peer)) {
                    keys.extend(rows.iter().map(|(key, _)| key.as_ref().clone()));
                }
            }
            for (floor, rows) in right {
                if !left.get(floor).is_some_and(|peer| Arc::ptr_eq(rows, peer)) {
                    keys.extend(rows.iter().map(|(key, _)| key.as_ref().clone()));
                }
            }
        } else if self != other {
            keys.extend(self.keys().chain(other.keys()).cloned());
        }
        keys
    }
    pub(super) fn get(&self, key: &Key) -> Option<&Element> {
        let rows = match self {
            Self::Small(rows) => rows,
            Self::Paged(pages) => pages.range(..=key).next_back()?.1,
        };
        Some(rows[position(rows, key).ok()?].1.as_ref())
    }
    pub(super) fn get_mut(&mut self, key: &Key) -> Option<&mut Element> {
        let rows = match self {
            Self::Small(rows) => rows,
            Self::Paged(pages) => {
                let floor = pages.range(..=key).next_back()?.0.clone();
                Arc::make_mut(pages.get_mut(&floor)?)
            }
        };
        let position = position(rows, key).ok()?;
        Some(Arc::make_mut(&mut rows[position].1))
    }
    pub(super) fn insert(&mut self, key: Key, value: Element) -> Option<Element> {
        let previous = match self {
            Self::Small(rows) => {
                let previous = insert(rows, key, value);
                if rows.len() > PAGE_ROWS {
                    let mut lower = std::mem::take(rows);
                    let upper = lower.split_off(lower.len() / 2);
                    *self = Self::Paged(BTreeMap::from([
                        (lower[0].0.as_ref().clone(), Arc::new(lower)),
                        (upper[0].0.as_ref().clone(), Arc::new(upper)),
                    ]));
                }
                previous
            }
            Self::Paged(pages) => {
                let mut floor = pages
                    .range(..=key.clone())
                    .next_back()
                    .map(|(key, _)| key.clone())
                    .or_else(|| pages.first_key_value().map(|(key, _)| key.clone()));
                if floor.as_ref().is_some_and(|floor| &key < floor) {
                    let rows = pages.remove(floor.as_ref().unwrap()).unwrap();
                    pages.insert(key.clone(), rows);
                    floor = Some(key.clone());
                }
                let rows = Arc::make_mut(
                    pages
                        .entry(floor.unwrap_or_else(|| key.clone()))
                        .or_default(),
                );
                let previous = insert(rows, key, value);
                if rows.len() > PAGE_ROWS {
                    let upper = rows.split_off(rows.len() / 2);
                    pages.insert(upper[0].0.as_ref().clone(), Arc::new(upper));
                }
                previous
            }
        };
        previous.map(Arc::unwrap_or_clone)
    }
    pub(super) fn remove(&mut self, key: &Key) -> Option<Element> {
        let previous = match self {
            Self::Small(rows) => {
                let position = position(rows, key).ok()?;
                rows.remove(position).1
            }
            Self::Paged(pages) => {
                let floor = pages.range(..=key).next_back()?.0.clone();
                let rows = Arc::make_mut(pages.get_mut(&floor)?);
                let position = position(rows, key).ok()?;
                let previous = rows.remove(position).1;
                if rows.is_empty() {
                    pages.remove(&floor);
                }
                previous
            }
        };
        Some(Arc::unwrap_or_clone(previous))
    }
    pub(super) fn is_empty(&self) -> bool {
        match self {
            Self::Small(rows) => rows.is_empty(),
            Self::Paged(pages) => pages.is_empty(),
        }
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (&Key, &Element)> {
        let small = match self {
            Self::Small(rows) => Some(rows.iter()),
            _ => None,
        };
        let pages = match self {
            Self::Paged(pages) => Some(pages),
            _ => None,
        };
        small
            .into_iter()
            .flatten()
            .chain(
                pages
                    .into_iter()
                    .flat_map(|pages| pages.values())
                    .flat_map(|rows| rows.iter()),
            )
            .map(|(key, value)| (key.as_ref(), value.as_ref()))
    }
    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &Key> {
        self.iter().map(|(key, _)| key)
    }
}
pub(super) struct IntoIter<Key: Ord + Clone, Element: Clone + Eq> {
    pages: std::collections::btree_map::IntoValues<Key, Arc<Rows<Key, Element>>>,
    current: std::vec::IntoIter<Cell<Key, Element>>,
}
impl<Key: Ord + Clone, Element: Clone + Eq> Iterator for IntoIter<Key, Element> {
    type Item = (Key, Element);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((key, value)) = self.current.next() {
                return Some((Arc::unwrap_or_clone(key), Arc::unwrap_or_clone(value)));
            }
            self.current = Arc::unwrap_or_clone(self.pages.next()?).into_iter();
        }
    }
}
impl<Key: Ord + Clone, Element: Clone + Eq> IntoIterator for RowPages<Key, Element> {
    type Item = (Key, Element);
    type IntoIter = IntoIter<Key, Element>;
    fn into_iter(self) -> Self::IntoIter {
        let (current, pages) = match self {
            Self::Small(rows) => (rows, BTreeMap::new()),
            Self::Paged(pages) => (Vec::new(), pages),
        };
        IntoIter {
            pages: pages.into_values(),
            current: current.into_iter(),
        }
    }
}
