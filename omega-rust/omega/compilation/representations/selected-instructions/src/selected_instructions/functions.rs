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

use super::plan_identity::{CachedFunctionPrefix, selected_function_digest_with_prefix};
use super::{
    SelectedBlocks, SelectedBoundarySettlement, SelectedFunction, SelectedLocalStorageSlot,
    VirtualRegister,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedFunctions(Vec<Arc<FunctionEntry>>);

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

    fn digest(&self) -> [u8; 32] {
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
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&SelectedFunction> {
        self.0.get(index).map(function_of)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedFunction> {
        self.0.get_mut(index).map(detach)
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
        let entry_arc = &mut self.0[index];
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

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SelectedFunction> + DoubleEndedIterator {
        self.0.iter().map(function_of)
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = &mut SelectedFunction> + DoubleEndedIterator {
        self.0.iter_mut().map(detach)
    }

    pub fn push(&mut self, function: SelectedFunction) {
        self.0.push(FunctionEntry::new(function));
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// The selected function for `machine`, when the storage carries it.
    pub fn function(&self, machine: MachineId) -> Option<&SelectedFunction> {
        self.iter().find(|function| function.machine == machine)
    }

    /// Reports storage reuse for diagnostics and allocation regression controls.
    pub fn shares_function_storage(&self, other: &Self, index: usize) -> bool {
        self.0
            .get(index)
            .zip(other.0.get(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }

    /// Each function's identity digest, in order, hashing only the functions
    /// whose digest no clone of this storage has computed yet.
    pub(crate) fn identity_digests(&self) -> impl ExactSizeIterator<Item = [u8; 32]> + '_ {
        self.0.iter().map(|entry| entry.digest())
    }
}

impl From<Vec<SelectedFunction>> for SelectedFunctions {
    fn from(functions: Vec<SelectedFunction>) -> Self {
        Self::from_iter(functions)
    }
}

impl FromIterator<SelectedFunction> for SelectedFunctions {
    fn from_iter<T: IntoIterator<Item = SelectedFunction>>(functions: T) -> Self {
        Self(functions.into_iter().map(FunctionEntry::new).collect())
    }
}

impl Index<usize> for SelectedFunctions {
    type Output = SelectedFunction;

    fn index(&self, index: usize) -> &Self::Output {
        function_of(&self.0[index])
    }
}

impl IndexMut<usize> for SelectedFunctions {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        detach(&mut self.0[index])
    }
}

impl<'storage> IntoIterator for &'storage SelectedFunctions {
    type Item = &'storage SelectedFunction;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'storage, Arc<FunctionEntry>>,
        fn(&'storage Arc<FunctionEntry>) -> &'storage SelectedFunction,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(function_of)
    }
}

impl<'storage> IntoIterator for &'storage mut SelectedFunctions {
    type Item = &'storage mut SelectedFunction;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'storage, Arc<FunctionEntry>>,
        fn(&'storage mut Arc<FunctionEntry>) -> &'storage mut SelectedFunction,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut().map(detach)
    }
}
