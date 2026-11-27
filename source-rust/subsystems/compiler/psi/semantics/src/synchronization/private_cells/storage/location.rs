//! A borrowed place can select one of several concrete homes. Copying that
//! borrow preserves the selection. Another choice over the same homes can
//! independently select a different one.

use semantic_vocabulary::PlaceId;
use std::rc::Rc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Allocation {
    Private(usize),
    ExclusiveInput(usize),
    SharedInput(PlaceId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Home {
    pub allocation: Allocation,
    pub path: Vec<Segment>,
}

#[derive(Clone)]
enum Root {
    Allocation(Allocation),
    Choice(Rc<[Home]>),
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Allocation(left), Self::Allocation(right)) => left == right,
            (Self::Choice(left), Self::Choice(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}
impl Eq for Root {}

#[derive(Clone, PartialEq, Eq)]
pub(in super::super) struct Location {
    root: Root,
    pub(super) path: Vec<Segment>,
}

impl Location {
    pub(super) fn allocation(id: Allocation) -> Self {
        Self {
            root: Root::Allocation(id),
            path: Vec::new(),
        }
    }

    pub(in super::super) fn input(place: PlaceId) -> Self {
        Self::allocation(Allocation::SharedInput(place))
    }

    pub(super) fn includes_shared(&self) -> bool {
        let mut shared = false;
        self.each_home(|home, _| shared |= matches!(home, Allocation::SharedInput(_)));
        shared
    }

    pub(super) fn is_choice(&self) -> bool {
        matches!(self.root, Root::Choice(_))
    }

    pub(super) fn each_home(&self, mut visit: impl FnMut(Allocation, &[Segment])) {
        match &self.root {
            Root::Allocation(n) => visit(*n, &self.path),
            Root::Choice(homes) => {
                for home in homes.iter() {
                    if self.path.is_empty() {
                        visit(home.allocation, &home.path);
                    } else {
                        let mut path = home.path.clone();
                        path.extend_from_slice(&self.path);
                        visit(home.allocation, &path);
                    }
                }
            }
        }
    }

    pub(in super::super) fn join(&mut self, other: &Self) {
        if self == other {
            return;
        }
        let mut candidates = Vec::new();
        for selection in [&*self, other] {
            selection.each_home(|allocation, path| {
                candidates.push(Home {
                    allocation,
                    path: path.to_vec(),
                });
            });
        }
        candidates.sort_unstable();
        candidates.dedup();
        if let [only] = candidates.as_slice() {
            self.root = Root::Allocation(only.allocation);
            self.path = only.path.clone();
        } else {
            self.root = Root::Choice(candidates.into());
            self.path.clear();
        }
    }

    pub(super) fn may_be_written_by(&self, other: &Self) -> bool {
        let mut overlap = false;
        self.each_home(|left, first| {
            other.each_home(|right, second| {
                overlap |= left == right && crate::structural_paths_may_overlap(first, second);
            });
        });
        overlap
    }

    /// The extra projections below this exact selection, when it contains
    /// the other place. Equal candidate sets alone cannot establish this.
    pub(super) fn suffix<'a>(&self, other: &'a Self) -> Option<&'a [Segment]> {
        if self.root == other.root {
            other.path.strip_prefix(self.path.as_slice())
        } else {
            None
        }
    }
}
