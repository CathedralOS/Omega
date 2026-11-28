//! Activation-local cells receive distinct identities across forks. Copies of
//! the store share untouched pages. A join visits cells only on changed pages.
use super::{Allocation, Arrivals, Stored};
use crate::synchronization::facts::Facts;
use std::rc::Rc;

const CELLS_PER_PAGE: usize = 32;
type Page = Facts<usize, Stored>;

#[derive(Clone, Default)]
pub(super) struct Allocations {
    // Keep small checks in one page without allocating a page directory.
    first_page: Rc<Page>,
    later_pages: Option<Rc<Facts<usize, Rc<Page>>>>,
}

impl Allocations {
    pub(super) fn get(&self, home: Allocation) -> Option<&Stored> {
        let id = home.local_id()?;
        let page = if id < CELLS_PER_PAGE {
            &self.first_page
        } else {
            self.later_pages.as_ref()?.get(&(id / CELLS_PER_PAGE))?
        };
        page.get(&(id % CELLS_PER_PAGE))
    }

    pub(super) fn slot(&mut self, id: usize) -> &mut Stored {
        let page = if id < CELLS_PER_PAGE {
            &mut self.first_page
        } else {
            let pages = self.later_pages.get_or_insert_with(Rc::default);
            Rc::make_mut(pages).get_or_insert_with(id / CELLS_PER_PAGE, Rc::default)
        };
        Rc::make_mut(page).get_or_insert_with(id % CELLS_PER_PAGE, Stored::default)
    }

    pub(super) fn join(&mut self, incoming: &Self, arrivals: &mut Arrivals<'_>) {
        join_page(&mut self.first_page, &incoming.first_page, arrivals);
        let Some(other_pages) = &incoming.later_pages else {
            return;
        };
        let Some(pages) = &mut self.later_pages else {
            self.later_pages = Some(other_pages.clone());
            return;
        };
        if Rc::ptr_eq(pages, other_pages) {
            return;
        }
        let pages = Rc::make_mut(pages);
        for (index, other) in other_pages.entries() {
            if let Some(current) = pages.get_mut(index) {
                join_page(current, other, arrivals);
            } else {
                pages.insert(*index, other.clone());
            }
        }
    }
}

fn join_page(current: &mut Rc<Page>, other: &Rc<Page>, arrivals: &mut Arrivals<'_>) {
    if Rc::ptr_eq(current, other) {
        return;
    }
    // An allocation present on only one arm remains reachable through
    // a reference returned by that arm.
    let current = Rc::make_mut(current);
    for (slot, right) in other.entries() {
        if let Some(left) = current.get_mut(slot) {
            arrivals.value(left, right);
        } else {
            current.insert(*slot, right.clone());
        }
    }
}
