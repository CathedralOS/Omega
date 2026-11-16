//! Symbol snapshots share bounded pages from their first construction. Point
//! writes copy only the selected page; appends retain earlier pages unchanged.

use arena::{Handle, HandleSpan, HierarchyArena, HierarchyNode, HierarchyStorage, RetainedArena};
use compiler_storage::{Record, RecordError, RecordReader, RecordWriter};

use crate::Symbol;

const PAGE_ROWS: usize = 128;

#[derive(Debug, Clone)]
pub struct SymbolRows<T> {
    rows: RetainedArena<T>,
    pending: usize,
}

impl<T: Default> Default for SymbolRows<T> {
    fn default() -> Self {
        Self {
            rows: RetainedArena::new(),
            pending: 0,
        }
    }
}

impl<T: Default> SymbolRows<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        if self.pending == PAGE_ROWS {
            self.rows.finish_append_page();
            self.pending = 0;
        }
        let handle = self.rows.insert(value);
        self.pending += 1;
        handle
    }

    pub fn insert_many(&mut self, values: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let mut span = HandleSpan::empty();
        for value in values {
            span.push_contiguous(self.insert(value));
        }
        span
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        self.rows.get(handle)
    }
    pub fn try_get(&self, handle: Handle<T>) -> Option<&T> {
        self.rows.try_get(handle)
    }
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.rows.is_valid(handle)
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn revision(&self) -> u64 {
        self.rows.revision()
    }
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.rows.iter()
    }
}

impl<T: Default + Clone> SymbolRows<T> {
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.rows.get_mut(handle)
    }
}

impl<T: Default + PartialEq> PartialEq for SymbolRows<T> {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}
impl<T: Default + Eq> Eq for SymbolRows<T> {}

impl<T: Default + Record + 'static> Record for SymbolRows<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.rows.write(output)
    }

    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let rows = RetainedArena::<T>::read(input)?;
        if !rows.pages_fit(PAGE_ROWS) {
            return Err(input.error("symbol storage page exceeds its row bound"));
        }
        if !rows.has_dense_generation() {
            return Err(input.error("symbol rows must be dense and retain one generation"));
        }
        // A restored candidate starts a private bounded append page. No old
        // span needs to be extended through this table's insertion surface.
        Ok(Self {
            rows,
            pending: PAGE_ROWS,
        })
    }
}

impl<T: HierarchyNode> HierarchyStorage<T> for SymbolRows<T> {
    fn get(&self, handle: Handle<T>) -> &T {
        self.get(handle)
    }
    fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.get_mut(handle)
    }
    fn is_valid(&self, handle: Handle<T>) -> bool {
        self.is_valid(handle)
    }
    fn append(&mut self, value: T) -> Handle<T> {
        self.insert(value)
    }
    fn insert_many(&mut self, values: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        self.insert_many(values)
    }
    fn contains_span(&self, children: HandleSpan<T>) -> bool {
        // Symbol rows are never retired, and decoding checks dense coordinates
        // and one generation. Endpoints therefore validate the whole range,
        // including a range crossing storage pages, without scanning children.
        if children.is_empty() {
            return true;
        }
        let Some(last) = children
            .start()
            .arena_index()
            .checked_add(children.count() - 1)
        else {
            return false;
        };
        self.is_valid(children.start())
            && self.is_valid(Handle::from_parts(last, children.start().generation()))
    }
    fn iter<'rows>(&'rows self) -> impl Iterator<Item = (Handle<T>, &'rows T)>
    where
        T: 'rows,
    {
        self.iter()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn storage_len(&self) -> usize {
        self.len()
    }
}

pub type SymbolHierarchy = HierarchyArena<Symbol, SymbolRows<Symbol>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
    use compiler_storage::{RecordLimits, decode, encode};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Default)]
    struct Counted(Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::Relaxed);
            Self(Arc::clone(&self.0))
        }
    }

    #[test]
    fn forks_share_rows_and_point_writes_copy_only_a_bounded_page() {
        let copies = Arc::new(AtomicUsize::new(0));
        let mut original = SymbolRows::new();
        let handles =
            original.insert_many((0..3 * PAGE_ROWS + 1).map(|_| Counted(Arc::clone(&copies))));
        let mut first = original.clone();
        let mut last = original.clone();
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        first.get_mut(handles.start());
        assert_eq!(copies.load(Ordering::Relaxed), PAGE_ROWS);
        let middle = handles.handle_at(PAGE_ROWS as u32);
        assert!(std::ptr::eq(original.get(middle), first.get(middle)));
        last.get_mut(handles.handle_at((3 * PAGE_ROWS) as u32));
        assert_eq!(copies.load(Ordering::Relaxed), PAGE_ROWS + 1);
        let appended = first.insert(Counted(Arc::clone(&copies)));
        assert_eq!(copies.load(Ordering::Relaxed), PAGE_ROWS + 1);
        assert_eq!(appended.arena_index() as usize, original.len() + 1);
        assert!(!original.is_valid(appended));
        assert!(!last.is_valid(appended));
        assert!(std::ptr::eq(original.get(middle), last.get(middle)));
    }

    #[test]
    fn symbol_snapshots_keep_cross_page_children_names_and_independent_writes() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            (0..3 * PAGE_ROWS).map(|_| (SymbolKind::Machine, SymbolNameRef::Static("child"))),
        );
        let original = builder.finish();
        let mut fork = original.clone();
        let sibling = original.clone();
        let first = children.start();
        let middle = children.handle_at(PAGE_ROWS as u32);
        let last = children.handle_at(children.count() - 1);
        assert!(std::ptr::eq(original.get(first), fork.get(first)));
        assert!(std::ptr::eq(
            original.names().get(original.get(first).name),
            fork.names().get(fork.get(first).name)
        ));
        fork.rename(first, "first");
        fork.rename(last, "last");
        assert_eq!(original.name(first), "child");
        assert_eq!(sibling.name(last), "child");
        assert_eq!(fork.name(first), "first");
        assert_eq!(fork.name(last), "last");
        assert!(std::ptr::eq(original.get(middle), fork.get(middle)));
        let appended = fork
            .append_children(
                root,
                [(SymbolKind::Machine, SymbolNameRef::Static("appended"))],
            )
            .start();
        assert_eq!(
            fork.child_handles(root).unwrap().collect::<Vec<_>>(),
            children.handles().chain([appended]).collect::<Vec<_>>()
        );
        assert_eq!(
            original.child_handles(root).unwrap().count(),
            children.len()
        );
        assert_eq!(sibling, original);
        let limits = RecordLimits::default();
        let mut restored: SymbolTable = decode(&encode(&fork, limits).unwrap(), limits).unwrap();
        assert_eq!(restored, fork);
        let next =
            restored.append_children(last, [(SymbolKind::Local, SymbolNameRef::Static("local"))]);
        let expected =
            fork.append_children(last, [(SymbolKind::Local, SymbolNameRef::Static("local"))]);
        assert_eq!(next, expected);
        assert_eq!(restored, fork);
        assert!(std::ptr::eq(original.get(middle), sibling.get(middle)));
        let (mut decoded, decoded_sibling): (SymbolTable, SymbolTable) =
            decode(&encode(&(fork, original), limits).unwrap(), limits).unwrap();
        assert!(std::ptr::eq(
            decoded.get(middle),
            decoded_sibling.get(middle)
        ));
        assert!(std::ptr::eq(
            decoded.names().get(decoded.get(middle).name),
            decoded_sibling
                .names()
                .get(decoded_sibling.get(middle).name),
        ));
        decoded.rename(middle, "detached after decoding");
        assert_eq!(decoded_sibling.name(middle), "child");
    }

    #[test]
    fn hierarchy_codec_refuses_stale_children_and_cycles() {
        refuse_malformed_hierarchy::<SymbolRows<Symbol>>();
        refuse_malformed_hierarchy::<arena::Arena<Symbol>>();
    }

    fn refuse_malformed_hierarchy<Storage>()
    where
        Storage: HierarchyStorage<Symbol> + Record + Clone + std::fmt::Debug,
    {
        let mut hierarchy = HierarchyArena::<Symbol, Storage>::default();
        let root = hierarchy.insert_root(Symbol::default());
        let child = hierarchy.insert_children(root, [Symbol::default()]).start();
        let limits = RecordLimits::default();
        let mut stale = hierarchy.clone();
        stale.get_mut(root).children = HandleSpan::from_parts(
            Handle::from_parts(child.arena_index(), child.generation() + 1),
            1,
        );
        assert!(
            decode::<HierarchyArena<Symbol, Storage>>(&encode(&stale, limits).unwrap(), limits)
                .unwrap_err()
                .reason
                .contains("child range does not name live storage")
        );
        let mut invalid_parent = hierarchy.clone();
        invalid_parent.get_mut(root).parent =
            Handle::from_parts(child.arena_index(), child.generation() + 1);
        assert!(
            decode::<HierarchyArena<Symbol, Storage>>(
                &encode(&invalid_parent, limits).unwrap(),
                limits
            )
            .unwrap_err()
            .reason
            .contains("parent does not name live storage")
        );
        let mut cyclic = hierarchy;
        cyclic.get_mut(root).parent = child;
        assert!(
            decode::<HierarchyArena<Symbol, Storage>>(&encode(&cyclic, limits).unwrap(), limits)
                .unwrap_err()
                .reason
                .contains("parent chain is cyclic")
        );
    }

    #[test]
    fn codec_refuses_oversized_pages_and_retired_slots() {
        let limits = RecordLimits::default();
        let mut oversized = RetainedArena::<u32>::new();
        oversized.insert_many(0..PAGE_ROWS as u32 + 1);
        let bytes = encode(&oversized, limits).unwrap();
        assert!(
            decode::<SymbolRows<u32>>(&bytes, limits)
                .unwrap_err()
                .reason
                .contains("page exceeds its row bound")
        );
        let mut retired = SymbolRows::<u32>::new();
        let first = retired.insert(1);
        retired.insert(2);
        assert!(retired.rows.free(first));
        let bytes = encode(&retired, limits).unwrap();
        assert!(
            decode::<SymbolRows<u32>>(&bytes, limits)
                .unwrap_err()
                .reason
                .contains("dense and retain one generation")
        );
    }
}
