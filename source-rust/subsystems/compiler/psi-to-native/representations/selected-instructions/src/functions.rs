/*
  Ordered function storage shared across selected-instruction rewrites.

  A spill or split rewrite modifies a single function; retaining replay history
  must not copy every unrelated function body in the plan. Functions live in
  shared `Arc<SelectedFunction>` entries grouped into shared pages. Every
  mutable route detaches only the page and function it edits, so clones share
  unmodified pages and equality short-circuits on shared storage.
*/

use semantic_vocabulary::MachineId;
use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use super::{
    SelectedBlocks, SelectedBoundarySettlement, SelectedFunction, SelectedLocalStorageSlot,
    VirtualRegister,
};

const FUNCTION_PAGE_SIZE: usize = 64;

#[derive(Clone)]
struct FunctionPage {
    entries: Vec<Arc<SelectedFunction>>,
}

impl FunctionPage {
    fn new(entries: Vec<Arc<SelectedFunction>>) -> Arc<Self> {
        Arc::new(Self { entries })
    }
}

#[derive(Clone, Default)]
pub struct SelectedFunctions {
    pages: Vec<Arc<FunctionPage>>,
    len: usize,
}

impl PartialEq for SelectedFunctions {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && self
                .pages
                .iter()
                .zip(&other.pages)
                .all(|(left, right)| Arc::ptr_eq(left, right) || left.entries == right.entries)
    }
}

impl Eq for SelectedFunctions {}

impl fmt::Debug for SelectedFunctions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

fn detach(entry: &mut Arc<SelectedFunction>) -> &mut SelectedFunction {
    Arc::make_mut(entry)
}

impl SelectedFunctions {
    fn entry(&self, index: usize) -> Option<&Arc<SelectedFunction>> {
        if index >= self.len {
            return None;
        }
        let page = self.pages.get(index / FUNCTION_PAGE_SIZE)?;
        page.entries.get(index % FUNCTION_PAGE_SIZE)
    }

    fn detach_entry(&mut self, index: usize) -> Option<&mut Arc<SelectedFunction>> {
        if index >= self.len {
            return None;
        }
        let page = Arc::make_mut(self.pages.get_mut(index / FUNCTION_PAGE_SIZE)?);
        page.entries.get_mut(index % FUNCTION_PAGE_SIZE)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<&SelectedFunction> {
        self.entry(index).map(Arc::as_ref)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedFunction> {
        self.detach_entry(index).map(detach)
    }

    /// Lend a function's frame slots, virtual registers, boundary settlements
    /// and blocks at once, so one rewrite can edit all four together.
    pub fn function_parts_mut<R>(
        &mut self,
        index: usize,
        f: impl FnOnce(
            &mut Vec<SelectedLocalStorageSlot>,
            &mut Vec<VirtualRegister>,
            &mut Vec<SelectedBoundarySettlement>,
            &mut SelectedBlocks,
        ) -> R,
    ) -> R {
        let function = detach(
            self.detach_entry(index)
                .expect("function index must be in bounds"),
        );
        let contracts = Arc::make_mut(&mut function.contracts);
        f(
            &mut contracts.local_storage_slots,
            &mut contracts.virtual_registers,
            &mut contracts.boundary_settlements,
            &mut function.blocks,
        )
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            pages: &self.pages,
            front: 0,
            back: self.len,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        let mut pages = self.pages.iter_mut();
        let current_front = pages.next().map_or_else(
            || [].iter_mut(),
            |page| Arc::make_mut(page).entries.iter_mut(),
        );
        IterMut {
            pages,
            current_front,
            current_back: [].iter_mut(),
            remaining: self.len,
        }
    }

    pub fn push(&mut self, function: SelectedFunction) {
        let entry = Arc::new(function);
        if let Some(last_page) = self.pages.last_mut()
            && last_page.entries.len() < FUNCTION_PAGE_SIZE
        {
            Arc::make_mut(last_page).entries.push(entry);
        } else {
            let mut entries = Vec::with_capacity(FUNCTION_PAGE_SIZE);
            entries.push(entry);
            self.pages.push(FunctionPage::new(entries));
        }
        self.len += 1;
    }

    pub fn clear(&mut self) {
        self.pages.clear();
        self.len = 0;
    }

    pub fn function(&self, machine: MachineId) -> Option<&SelectedFunction> {
        self.iter().find(|function| function.machine == machine)
    }

    pub fn shares_function_storage(&self, other: &Self, index: usize) -> bool {
        self.entry(index)
            .zip(other.entry(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
}

pub struct Iter<'storage> {
    pages: &'storage [Arc<FunctionPage>],
    front: usize,
    back: usize,
}

impl<'storage> Iterator for Iter<'storage> {
    type Item = &'storage SelectedFunction;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        Some(&self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }
}

impl<'storage> DoubleEndedIterator for Iter<'storage> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        let index = self.back;
        Some(&self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE])
    }
}

impl ExactSizeIterator for Iter<'_> {
    fn len(&self) -> usize {
        self.back - self.front
    }
}

pub struct IterMut<'storage> {
    pages: std::slice::IterMut<'storage, Arc<FunctionPage>>,
    current_front: std::slice::IterMut<'storage, Arc<SelectedFunction>>,
    current_back: std::slice::IterMut<'storage, Arc<SelectedFunction>>,
    remaining: usize,
}

impl<'storage> Iterator for IterMut<'storage> {
    type Item = &'storage mut SelectedFunction;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current_front.next() {
                self.remaining -= 1;
                return Some(detach(entry));
            }
            if let Some(page) = self.pages.next() {
                self.current_front = Arc::make_mut(page).entries.iter_mut();
                continue;
            }
            if let Some(entry) = self.current_back.next() {
                self.remaining -= 1;
                return Some(detach(entry));
            }
            return None;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'storage> DoubleEndedIterator for IterMut<'storage> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.current_back.next_back() {
                self.remaining -= 1;
                return Some(detach(entry));
            }
            if let Some(page) = self.pages.next_back() {
                self.current_back = Arc::make_mut(page).entries.iter_mut();
                continue;
            }
            if let Some(entry) = self.current_front.next_back() {
                self.remaining -= 1;
                return Some(detach(entry));
            }
            return None;
        }
    }
}

impl ExactSizeIterator for IterMut<'_> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl From<Vec<SelectedFunction>> for SelectedFunctions {
    fn from(functions: Vec<SelectedFunction>) -> Self {
        Self::from_iter(functions)
    }
}

impl FromIterator<SelectedFunction> for SelectedFunctions {
    fn from_iter<T: IntoIterator<Item = SelectedFunction>>(functions: T) -> Self {
        let iter = functions.into_iter();
        let (lower, _) = iter.size_hint();
        let mut pages = Vec::with_capacity(lower.div_ceil(FUNCTION_PAGE_SIZE));
        let mut current = Vec::with_capacity(FUNCTION_PAGE_SIZE);
        let mut len = 0;
        for function in iter {
            current.push(Arc::new(function));
            len += 1;
            if current.len() == FUNCTION_PAGE_SIZE {
                pages.push(FunctionPage::new(current));
                current = Vec::with_capacity(FUNCTION_PAGE_SIZE);
            }
        }
        if !current.is_empty() {
            pages.push(FunctionPage::new(current));
        }
        Self { pages, len }
    }
}

impl Index<usize> for SelectedFunctions {
    type Output = SelectedFunction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE]
    }
}

impl IndexMut<usize> for SelectedFunctions {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        detach(
            self.detach_entry(index)
                .expect("function index must be in bounds"),
        )
    }
}

impl<'storage> IntoIterator for &'storage SelectedFunctions {
    type Item = &'storage SelectedFunction;
    type IntoIter = Iter<'storage>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'storage> IntoIterator for &'storage mut SelectedFunctions {
    type Item = &'storage mut SelectedFunction;
    type IntoIter = IterMut<'storage>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
