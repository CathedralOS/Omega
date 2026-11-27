//! Join borrows together with the storage from their own control-flow
//! arms. If each arm clears the cell it returns, that cell is clear after
//! the join even though either physical cell could still be held.

use super::{Location, Storage, Stored};

pub(in super::super) struct Arrivals<'a> {
    left: &'a Storage,
    right: &'a Storage,
    selections: Vec<Selection>,
}

struct Selection {
    left: Location,
    right: Location,
    result: Location,
    contents: Stored,
}

impl<'a> Arrivals<'a> {
    pub(in super::super) fn new(left: &'a Storage, right: &'a Storage) -> Self {
        Self {
            left,
            right,
            selections: Vec::new(),
        }
    }

    pub(in super::super) fn place(&mut self, left: &mut Location, right: &Location) {
        if left == right {
            return;
        }
        if let Some(known) = self
            .selections
            .iter()
            .find(|selection| selection.left == *left && selection.right == *right)
        {
            *left = known.result.clone();
            return;
        }
        let mut result = left.clone();
        result.join(right);
        let mut contents = self.left.get(left);
        contents.join(&self.right.get(right));
        self.selections.push(Selection {
            left: left.clone(),
            right: right.clone(),
            result: result.clone(),
            contents,
        });
        *left = result;
    }

    pub(in super::super) fn value(&mut self, left: &mut Stored, right: &Stored) {
        left.join_with(right, &mut |left, right| self.place(left, right));
    }

    pub(super) fn record(self, storage: &mut Storage) {
        let known = std::rc::Rc::make_mut(&mut storage.selected);
        known.extend(self.selections.into_iter().filter_map(|selection| {
            (selection.result.is_choice() && !matches!(selection.contents, Stored::Unknown))
                .then_some((selection.result, selection.contents))
        }));
    }
}
