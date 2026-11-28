//! Sparse facts on a control-flow branch. Forks share pages until the first
//! write. A join can skip an identical page because joining a fact with
//! itself preserves it. External IDs can be sparse without allocating gaps.
use super::Facts;
use semantic_vocabulary::{BlockId, PlaceId, ValueId};
use std::rc::Rc;

const ROWS_PER_PAGE: u64 = 32;

#[derive(Clone, Copy)]
enum Membership {
    Either,
    Both,
}

pub(in super::super) trait PageKey: Copy + Ord {
    fn page(self) -> u64;
}
macro_rules! semantic_key {
    ($id:ty) => {
        impl PageKey for $id {
            fn page(self) -> u64 {
                self.get() / ROWS_PER_PAGE
            }
        }
    };
}
semantic_key!(BlockId);
semantic_key!(PlaceId);
semantic_key!(ValueId);
impl PageKey for usize {
    fn page(self) -> u64 {
        self as u64 / ROWS_PER_PAGE
    }
}

type Page<K, V> = Rc<Facts<K, V>>;

#[derive(Clone)]
pub(in super::super) struct BranchFacts<K, V> {
    first: Option<Page<K, V>>,
    later: Option<Page<u64, Page<K, V>>>,
}

impl<K, V> Default for BranchFacts<K, V> {
    fn default() -> Self {
        Self {
            first: None,
            later: None,
        }
    }
}

impl<K: PageKey, V: Clone> BranchFacts<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        let page = match key.page() {
            0 => self.first.as_ref()?,
            n => self.later.as_ref()?.get(&n)?,
        };
        page.get(key)
    }

    fn page_mut(&mut self, key: K) -> &mut Facts<K, V> {
        let page = match key.page() {
            0 => self.first.get_or_insert_with(Rc::default),
            n => Rc::make_mut(self.later.get_or_insert_with(Rc::default))
                .get_or_insert_with(n, Rc::default),
        };
        Rc::make_mut(page)
    }

    pub fn get_or_insert_with(&mut self, key: K, default: impl FnOnce() -> V) -> &mut V {
        self.page_mut(key).get_or_insert_with(key, default)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.page_mut(key).insert(key, value)
    }

    pub fn remove(&mut self, key: &K) {
        if self.get(key).is_some() {
            self.page_mut(*key).remove(key);
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.join(&other, Membership::Either, |left, right| {
            left.clone_from(right);
            true
        });
    }

    pub fn retain_equal(&mut self, other: &Self)
    where
        V: PartialEq,
    {
        self.intersect(other, |left, right| left == right);
    }

    pub fn intersect(&mut self, other: &Self, merge: impl FnMut(&mut V, &V) -> bool) {
        self.join(other, Membership::Both, merge);
    }

    pub fn union(&mut self, other: &Self, mut merge: impl FnMut(&mut V, &V)) {
        self.join(other, Membership::Either, |left, right| {
            merge(left, right);
            true
        });
    }

    fn join(
        &mut self,
        other: &Self,
        membership: Membership,
        mut merge: impl FnMut(&mut V, &V) -> bool,
    ) {
        combine_options(&mut self.first, &other.first, membership, |left, right| {
            combine_rows(left, right, membership, &mut merge)
        });
        combine_options(&mut self.later, &other.later, membership, |left, right| {
            combine_rows(left, right, membership, |left_page, right_page| {
                if !Rc::ptr_eq(left_page, right_page) {
                    combine_rows(Rc::make_mut(left_page), right_page, membership, &mut merge);
                }
                !left_page.is_empty()
            });
        });
    }
}

fn combine_options<K: Ord + Clone, V: Clone>(
    left: &mut Option<Page<K, V>>,
    right: &Option<Page<K, V>>,
    membership: Membership,
    merge: impl FnOnce(&mut Facts<K, V>, &Facts<K, V>),
) {
    match (left.as_mut(), right) {
        (Some(current), Some(other)) if !Rc::ptr_eq(current, other) => {
            merge(Rc::make_mut(current), other);
            if current.is_empty() {
                *left = None;
            }
        }
        (None, Some(_)) if matches!(membership, Membership::Either) => left.clone_from(right),
        (_, None) if matches!(membership, Membership::Both) => *left = None,
        _ => {}
    }
}

fn combine_rows<K: Ord + Copy, V: Clone>(
    left: &mut Facts<K, V>,
    right: &Facts<K, V>,
    membership: Membership,
    mut merge: impl FnMut(&mut V, &V) -> bool,
) {
    if matches!(membership, Membership::Either) {
        for (key, value) in right.entries() {
            if let Some(current) = left.get_mut(key) {
                merge(current, value);
            } else {
                left.insert(*key, value.clone());
            }
        }
    } else {
        left.retain(|key, value| right.get(key).is_some_and(|other| merge(value, other)));
    }
}
