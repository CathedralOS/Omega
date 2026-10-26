//! Ordered function storage shared across selected rewrites. A spill changes one
//! function; retaining its replay history must not copy every unrelated body.
//! Mutation detaches only the selected function. Equality and serialization
//! remain content-based; allocation identity is never serialized.
//!
//! Each entry also carries the digest of its function's canonical identity
//! encoding (`plan_identity::selected_function_digest`), computed on first
//! request. The digest is a pure function of the content beside it: every
//! mutable access detaches the entry and clears the memo before handing out
//! the function, so a later request hashes the edited content again. A clone
//! shares the entry, and with it a digest already computed for the same
//! content, so the plan identity of a rewrite hashes only the functions the
//! rewrite touched.

use semantic_vocabulary::MachineId;
use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::{Arc, OnceLock};

use super::plan_identity::{
    CachedFunctionPrefix, CachedPlanPrefix, selected_function_digest_with_prefix,
};
use super::{
    SelectedBlocks, SelectedBoundarySettlement, SelectedFunction, SelectedLocalStorageSlot,
    VirtualRegister,
};

const FUNCTION_PAGE_SIZE: usize = 64;

#[derive(Clone)]
pub(crate) struct FunctionPage {
    entries: Vec<Arc<FunctionEntry>>,
    digest_bytes: OnceLock<Box<[u8]>>,
}

impl FunctionPage {
    fn new(entries: Vec<Arc<FunctionEntry>>) -> Arc<Self> {
        Arc::new(Self {
            entries,
            digest_bytes: OnceLock::new(),
        })
    }

    pub(crate) fn digest_bytes(&self) -> &[u8] {
        self.digest_bytes.get_or_init(|| {
            let mut bytes = Vec::with_capacity(self.entries.len() * 32);
            for entry in &self.entries {
                bytes.extend_from_slice(&entry.digest());
            }
            bytes.into_boxed_slice()
        })
    }
}

#[derive(Default)]
pub struct SelectedFunctions {
    pages: Vec<Arc<FunctionPage>>,
    len: usize,
    cached_plan_prefix: OnceLock<Arc<CachedPlanPrefix>>,
    previous_plan_prefix: Option<Arc<CachedPlanPrefix>>,
}

impl Clone for SelectedFunctions {
    fn clone(&self) -> Self {
        let previous_plan_prefix = self
            .cached_plan_prefix
            .get()
            .cloned()
            .or_else(|| self.previous_plan_prefix.clone());
        Self {
            pages: self.pages.clone(),
            len: self.len,
            cached_plan_prefix: OnceLock::new(),
            previous_plan_prefix,
        }
    }
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

/// One function and the memoized digest of its identity encoding. Equality
/// compares the function only; the digest is derived from it. Public only so
/// the storage iterator types can name it; this module is private.
#[derive(Clone)]
pub struct FunctionEntry {
    function: SelectedFunction,
    digest: OnceLock<[u8; 32]>,
    prefix: OnceLock<Arc<CachedFunctionPrefix>>,
    previous_prefix: Option<Arc<CachedFunctionPrefix>>,
    previous_prefix_verified: bool,
}

impl FunctionEntry {
    fn new(function: SelectedFunction) -> Arc<Self> {
        Arc::new(Self {
            function,
            digest: OnceLock::new(),
            prefix: OnceLock::new(),
            previous_prefix: None,
            previous_prefix_verified: false,
        })
    }

    pub(crate) fn digest(&self) -> [u8; 32] {
        *self.digest.get_or_init(|| {
            let (digest, prefix) = selected_function_digest_with_prefix(
                &self.function,
                self.previous_prefix.as_deref(),
                self.previous_prefix_verified,
            );
            let _ = self.prefix.set(prefix);
            digest
        })
    }
}

impl PartialEq for FunctionEntry {
    fn eq(&self, other: &Self) -> bool {
        self.function == other.function
    }
}

impl Eq for FunctionEntry {}

/// Shows the function alone, like equality.
impl fmt::Debug for FunctionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.function, formatter)
    }
}

/// The only route to a mutable function: detach a shared entry and drop its
/// digest, which no longer describes what the caller may write.
fn detach(entry: &mut Arc<FunctionEntry>) -> &mut SelectedFunction {
    let prior_prefix = entry.prefix.get().cloned();
    let entry = Arc::make_mut(entry);
    entry.digest.take();
    entry.prefix.take();
    entry.previous_prefix_verified = false;
    if prior_prefix.is_some() {
        entry.previous_prefix = prior_prefix;
    }
    &mut entry.function
}

fn function_of(entry: &Arc<FunctionEntry>) -> &SelectedFunction {
    &entry.function
}

impl SelectedFunctions {
    fn invalidate_plan_prefix(&mut self) {
        if let Some(prefix) = self.cached_plan_prefix.take() {
            self.previous_plan_prefix = Some(prefix);
        }
    }

    fn entry(&self, index: usize) -> Option<&Arc<FunctionEntry>> {
        if index >= self.len {
            return None;
        }
        let page = self.pages.get(index / FUNCTION_PAGE_SIZE)?;
        page.entries.get(index % FUNCTION_PAGE_SIZE)
    }

    fn detach_entry(&mut self, index: usize) -> Option<&mut Arc<FunctionEntry>> {
        if index >= self.len {
            return None;
        }
        self.invalidate_plan_prefix();
        let page = Arc::make_mut(self.pages.get_mut(index / FUNCTION_PAGE_SIZE)?);
        page.digest_bytes.take();
        page.entries.get_mut(index % FUNCTION_PAGE_SIZE)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<&SelectedFunction> {
        self.entry(index).map(function_of)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedFunction> {
        self.detach_entry(index).map(detach)
    }

    /// Mutate `index`'s dynamic storage slots, boundary settlements, blocks, and
    /// virtual-register tail without touching static function contracts or
    /// previously declared virtual registers, preserving verified prefix pages.
    pub fn mutate_preserving_register_prefix<R>(
        &mut self,
        index: usize,
        f: impl FnOnce(
            &mut Vec<SelectedLocalStorageSlot>,
            &mut Vec<VirtualRegister>,
            &mut Vec<SelectedBoundarySettlement>,
            &mut SelectedBlocks,
        ) -> R,
    ) -> R {
        let entry_arc = self
            .detach_entry(index)
            .expect("function index must be in bounds");
        let prior_prefix = entry_arc.prefix.get().cloned();
        let entry = Arc::make_mut(entry_arc);
        entry.digest.take();
        entry.prefix.take();
        if prior_prefix.is_some() {
            entry.previous_prefix = prior_prefix;
            entry.previous_prefix_verified = true;
        }
        let old_len = entry.function.virtual_registers.len();
        let result = f(
            &mut entry.function.local_storage_slots,
            &mut entry.function.virtual_registers,
            &mut entry.function.boundary_settlements,
            &mut entry.function.blocks,
        );
        debug_assert!(entry.function.virtual_registers.len() >= old_len);
        result
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            pages: &self.pages,
            front: 0,
            back: self.len,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.invalidate_plan_prefix();
        for page_arc in &mut self.pages {
            let page = Arc::make_mut(page_arc);
            page.digest_bytes.take();
        }
        let mut pages = self.pages.iter_mut();
        let current_front = pages
            .next()
            .map_or_else(|| [].iter_mut(), |page| Arc::make_mut(page).entries.iter_mut());
        IterMut {
            pages,
            current_front,
            current_back: [].iter_mut(),
            remaining: self.len,
        }
    }

    pub fn push(&mut self, function: SelectedFunction) {
        self.invalidate_plan_prefix();
        let entry = FunctionEntry::new(function);
        if let Some(last_page) = self.pages.last_mut()
            && last_page.entries.len() < FUNCTION_PAGE_SIZE
        {
            let page = Arc::make_mut(last_page);
            page.digest_bytes.take();
            page.entries.push(entry);
        } else {
            let mut entries = Vec::with_capacity(FUNCTION_PAGE_SIZE);
            entries.push(entry);
            self.pages.push(FunctionPage::new(entries));
        }
        self.len += 1;
    }

    pub fn clear(&mut self) {
        self.invalidate_plan_prefix();
        self.pages.clear();
        self.len = 0;
    }

    /// The selected function for `machine`, when the storage carries it.
    pub fn function(&self, machine: MachineId) -> Option<&SelectedFunction> {
        self.iter().find(|function| function.machine == machine)
    }

    /// Reports storage reuse for diagnostics and allocation regression controls.
    pub fn shares_function_storage(&self, other: &Self, index: usize) -> bool {
        self.entry(index)
            .zip(other.entry(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }

    /// Each function's identity digest, in order, hashing only the functions
    /// whose digest no clone of this storage has computed yet.
    #[allow(dead_code)]
    pub(crate) fn identity_digests(&self) -> impl ExactSizeIterator<Item = [u8; 32]> + '_ {
        (0..self.len).map(|index| {
            self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE].digest()
        })
    }

    pub(crate) fn pages(&self) -> &[Arc<FunctionPage>] {
        &self.pages
    }

    pub(crate) fn cached_plan_prefix(&self) -> Option<&Arc<CachedPlanPrefix>> {
        self.cached_plan_prefix.get()
    }

    pub(crate) fn previous_plan_prefix(&self) -> Option<&Arc<CachedPlanPrefix>> {
        self.previous_plan_prefix.as_ref()
    }

    pub(crate) fn store_plan_prefix(&self, prefix: Arc<CachedPlanPrefix>) {
        let _ = self.cached_plan_prefix.set(prefix);
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
        Some(&self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE].function)
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
        Some(&self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE].function)
    }
}

impl ExactSizeIterator for Iter<'_> {
    fn len(&self) -> usize {
        self.back - self.front
    }
}

pub struct IterMut<'storage> {
    pages: std::slice::IterMut<'storage, Arc<FunctionPage>>,
    current_front: std::slice::IterMut<'storage, Arc<FunctionEntry>>,
    current_back: std::slice::IterMut<'storage, Arc<FunctionEntry>>,
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
            current.push(FunctionEntry::new(function));
            len += 1;
            if current.len() == FUNCTION_PAGE_SIZE {
                pages.push(FunctionPage::new(current));
                current = Vec::with_capacity(FUNCTION_PAGE_SIZE);
            }
        }
        if !current.is_empty() {
            pages.push(FunctionPage::new(current));
        }
        Self {
            pages,
            len,
            cached_plan_prefix: OnceLock::new(),
            previous_plan_prefix: None,
        }
    }
}

impl Index<usize> for SelectedFunctions {
    type Output = SelectedFunction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.pages[index / FUNCTION_PAGE_SIZE].entries[index % FUNCTION_PAGE_SIZE].function
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
