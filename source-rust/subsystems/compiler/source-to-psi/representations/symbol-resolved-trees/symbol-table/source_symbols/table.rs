/*
    Start here. The symbol table: the hierarchy arena scoped lookup walks,
    its builder, appender and extension.
*/

use std::sync::Arc;

use crate::source_inputs::SourceMap;
use crate::source_symbols::storage::{SymbolHierarchy, SymbolRows};
use arena::{HandleSpan, SpanHandles};
use source_admission::SourceFile;
use source_admission::SourceOrigin;
use source_provenance::PackageKeyIdentity;
use source_text::{SourceId, SourceSpan};

use numerics::integer_carrier::{FixedInteger, IntegerSign};
use symbols::BUILTIN_TYPE_COUNT;

/* The spelling of the head of an integer family application, `u<N>` or `i<N>`. */
fn integer_family_name(sign: IntegerSign) -> &'static str {
    match sign {
        IntegerSign::Unsigned => "u",
        IntegerSign::Signed => "i",
    }
}
mod child_batches {
    /*
        Append-only child batches beneath frozen hierarchy nodes.

        A dense parent index links arena-owned spans. Appending another batch never
        copies earlier children or relocates a declaration's original child range.
    */

    use crate::source_symbols::storage::SymbolRows;
    use crate::source_symbols::{Symbol, SymbolHandle};
    use arena::{Handle, HandleSpan};
    use std::sync::Arc;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    struct ChildBatch {
        children: HandleSpan<Symbol>,
        next: Handle<ChildBatch>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    struct ChildChain {
        first: Handle<ChildBatch>,
        last: Handle<ChildBatch>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(super) struct SupplementalChildren {
        batches: SymbolRows<ChildBatch>,
        parents: Arc<Vec<ChildChain>>,
    }

    impl record_storage::Record for SupplementalChildren {
        fn write(
            &self,
            output: &mut record_storage::RecordWriter,
        ) -> Result<(), record_storage::RecordError> {
            output.record(|output| {
                self.batches.write(output)?;
                self.parents.write(output)
            })
        }

        fn read<Context>(
            input: &mut record_storage::RecordReader<'_, Context>,
        ) -> Result<Self, record_storage::RecordError> {
            input.record(|input| {
                let batches = SymbolRows::<ChildBatch>::read(input)?;
                let parents = Arc::<Vec<ChildChain>>::read(input)?;

                // Restored links must not turn missing declarations into empty
                // lookup results, loop forever, or append through the dummy row.
                // Every retained batch belongs to exactly one terminating chain
                // whose recorded tail is its final batch. Zero denotes no chain.
                input.allocate(batches.len())?;
                let mut visited = vec![false; batches.len()];
                let mut visited_count = 0;
                for chain in parents.iter() {
                    if chain.first.is_valid() != chain.last.is_valid() {
                        return Err(input.error("supplemental child chain has only one endpoint"));
                    }
                    let mut current = chain.first;
                    let mut tail = Handle::invalid();
                    while current.is_valid() {
                        let batch = batches.try_get(current).ok_or_else(|| {
                            input.error("supplemental child batch link does not name live storage")
                        })?;
                        let position = current.arena_index() as usize - 1;
                        if visited[position] {
                            return Err(input.error(
                                "supplemental child batch is cyclic or belongs to multiple chains",
                            ));
                        }
                        visited[position] = true;
                        visited_count += 1;
                        tail = current;
                        current = batch.next;
                    }
                    if tail != chain.last && chain.last.is_valid() {
                        return Err(
                            input.error("supplemental child chain tail does not match its links")
                        );
                    }
                }
                if visited_count != batches.len() {
                    return Err(input.error("supplemental child batch has no owning chain"));
                }
                Ok(Self { batches, parents })
            })
        }
    }

    impl SupplementalChildren {
        pub(super) fn append(&mut self, parent: SymbolHandle, children: HandleSpan<Symbol>) {
            if children.is_empty() {
                return;
            }
            let parent_index = parent.arena_index() as usize;
            if self.parents.len() <= parent_index {
                Arc::make_mut(&mut self.parents).resize(parent_index + 1, ChildChain::default());
            }
            let next = self.batches.insert(ChildBatch {
                children,
                next: Handle::invalid(),
            });
            let chain = &mut Arc::make_mut(&mut self.parents)[parent_index];
            if chain.last.is_valid() {
                self.batches.get_mut(chain.last).next = next;
            } else {
                chain.first = next;
            }
            chain.last = next;
        }

        pub(super) fn children(&self, parent: SymbolHandle) -> SupplementalChildHandles<'_> {
            SupplementalChildHandles {
                batches: &self.batches,
                next_batch: self
                    .parents
                    .get(parent.arena_index() as usize)
                    .map(|chain| chain.first)
                    .unwrap_or_default(),
                children: HandleSpan::empty(),
            }
        }
    }

    pub(super) struct SupplementalChildHandles<'table> {
        batches: &'table SymbolRows<ChildBatch>,
        next_batch: Handle<ChildBatch>,
        children: HandleSpan<Symbol>,
    }

    impl Iterator for SupplementalChildHandles<'_> {
        type Item = SymbolHandle;

        fn next(&mut self) -> Option<Self::Item> {
            self.nth(0)
        }

        fn nth(&mut self, mut ordinal: usize) -> Option<Self::Item> {
            loop {
                if ordinal < self.children.len() {
                    let selected = self.children.handle_at(ordinal as u32);
                    self.children = self.children.suffix(ordinal as u32 + 1);
                    return Some(selected);
                }
                ordinal -= self.children.len();
                self.children = HandleSpan::empty();
                if !self.next_batch.is_valid() {
                    return None;
                }
                let batch = self.batches.get(self.next_batch);
                self.next_batch = batch.next;
                self.children = batch.children;
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::source_symbols::{SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
        use record_storage::{RecordLimits, decode, encode};

        fn table_with_batches() -> SymbolTable {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let original = builder.insert_children(
                root,
                [(SymbolKind::Data, SymbolNameRef::Static("original"))],
            );
            let mut table = builder.finish();
            for name in ["first", "second"] {
                table.append_children(root, [(SymbolKind::Data, SymbolNameRef::Static(name))]);
            }
            table.append_children(
                original.start(),
                [(SymbolKind::Field, SymbolNameRef::Static("nested"))],
            );
            table
        }

        fn restore(table: &SymbolTable) -> Result<SymbolTable, record_storage::RecordError> {
            let limits = RecordLimits::default();
            decode(&encode(table, limits).expect("encode candidate"), limits)
        }

        fn root_chain(table: &SymbolTable) -> ChildChain {
            table.supplemental_children.parents[table.root().arena_index() as usize]
        }

        #[test]
        fn supplemental_storage_preserves_lookup_and_append() {
            let original = table_with_batches();
            let table = original.clone();
            let mut restored = restore(&table).expect("valid child chains");
            assert_eq!(restored, original);
            restored.append_children(
                restored.root(),
                [(SymbolKind::Data, SymbolNameRef::Static("third"))],
            );
            let names = restored
                .child_handles(restored.root())
                .expect("root children")
                .map(|child| restored.name(child).to_owned())
                .collect::<Vec<_>>();
            assert_eq!(names, ["original", "first", "second", "third"]);
            assert!(restore(&restored).is_ok());
            assert!(restore(&SymbolTable::default()).is_ok());
        }

        #[test]
        fn supplemental_storage_rejects_missing_batch_links() {
            for broken in [Handle::from_parts(2, 9), Handle::from_parts(99, 1)] {
                let mut table = table_with_batches();
                let first = root_chain(&table).first;
                table.supplemental_children.batches.get_mut(first).next = broken;
                let error = restore(&table).expect_err("stale or out-of-range batch");
                assert!(
                    error
                        .reason
                        .contains("batch link does not name live storage")
                );
            }
        }

        #[test]
        fn supplemental_storage_rejects_cycles_and_shared_chains() {
            let mut cyclic = table_with_batches();
            let chain = root_chain(&cyclic);
            cyclic
                .supplemental_children
                .batches
                .get_mut(chain.last)
                .next = chain.first;
            let error = restore(&cyclic).expect_err("cyclic chain");
            assert!(
                error
                    .reason
                    .contains("cyclic or belongs to multiple chains")
            );

            let mut shared = table_with_batches();
            let chain = root_chain(&shared);
            Arc::make_mut(&mut shared.supplemental_children.parents).push(chain);
            let error = restore(&shared).expect_err("shared chain");
            assert!(
                error
                    .reason
                    .contains("cyclic or belongs to multiple chains")
            );
        }

        #[test]
        fn supplemental_storage_rejects_inexact_tails() {
            for tail in [
                Handle::invalid(),
                Handle::from_parts(1, 1),
                Handle::from_parts(2, 9),
            ] {
                let mut table = table_with_batches();
                let parent = table.root().arena_index() as usize;
                Arc::make_mut(&mut table.supplemental_children.parents)[parent].last = tail;
                let error = restore(&table).expect_err("missing, early or stale tail");
                assert!(
                    error.reason.contains("only one endpoint")
                        || error.reason.contains("tail does not match")
                );
            }
        }

        #[test]
        fn supplemental_storage_rejects_unowned_batches() {
            let mut table = table_with_batches();
            let parent = table.root().arena_index() as usize;
            Arc::make_mut(&mut table.supplemental_children.parents)[parent] = ChildChain::default();
            let error = restore(&table).expect_err("unowned batch storage");
            assert!(error.reason.contains("batch has no owning chain"));
        }

        #[test]
        fn supplemental_storage_charges_chain_validation_scratch() {
            #[derive(record_storage::Record)]
            struct UncheckedChildren {
                batches: SymbolRows<ChildBatch>,
                parents: Arc<Vec<ChildChain>>,
            }
            let children = table_with_batches().supplemental_children;
            let bytes = encode(&children, RecordLimits::default()).unwrap();
            let prior_shape = UncheckedChildren {
                batches: children.batches.clone(),
                parents: children.parents.clone(),
            };
            assert_eq!(
                bytes,
                encode(&prior_shape, RecordLimits::default()).unwrap()
            );
            let mut low = 0;
            let mut high = RecordLimits::default().allocation;
            while low < high {
                let middle = low + (high - low) / 2;
                let limits = RecordLimits {
                    allocation: middle,
                    ..Default::default()
                };
                if decode::<UncheckedChildren>(&bytes, limits).is_ok() {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            let scratch = children.batches.len();
            let tight = RecordLimits {
                allocation: low + scratch - 1,
                ..Default::default()
            };
            let error = decode::<SupplementalChildren>(&bytes, tight).unwrap_err();
            assert!(error.reason.contains("allocation limit"));
            let exact = RecordLimits {
                allocation: low + scratch,
                ..Default::default()
            };
            assert_eq!(
                decode::<SupplementalChildren>(&bytes, exact).unwrap(),
                children
            );
        }
    }
}
mod indices {
    /*
        Deferred lookup indices: root-name, binding-source, module-path and
        module-scope maps built on first lookup and cleared by mutation. The
        caches ignore themselves for equality so a warmed table still compares
        equal to a cold one.
    */

    use super::{SourceScopedTopLevelBinding, SymbolHandle};
    use source_text::SourceId;
    use std::sync::Arc;

    /*
        Two views of the root roster, each built on its first use and both
        cleared together: name lookups need not build the module rosters,
        and a module member lookup need not hash every root name.
    */
    #[derive(Clone, Default)]
    pub(super) struct RootNameIndex {
        pub(super) names: std::sync::OnceLock<RootNameRows>,
        /*
            The members of every module namespace, before source selection.
            A member lookup through a module used to rescan every root child.
        */
        pub(super) module_children: std::sync::OnceLock<ModuleChildRoster>,
    }

    #[derive(Clone, Default)]
    pub(super) struct RootNameRows {
        pub(super) by_name: arena::HandleKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        pub(super) by_leaf: arena::HandleKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        /* Root children of kind `Domain`, in root roster order. */
        pub(super) domains: Vec<SymbolHandle>,
    }

    /*
        Each module's members in `module_children` order: nested module
        symbols in `module_symbols` order, then the root children whose
        authored file declares that module, in root roster order. `modules`
        is sorted by module handle and names each module's run in `children`.
        A module without a run has no members.
    */
    #[derive(Clone, Default)]
    pub(super) struct ModuleChildRoster {
        pub(super) modules: Vec<((u32, u32), usize, usize)>,
        pub(super) children: Vec<SymbolHandle>,
    }

    impl ModuleChildRoster {
        /* Group `(module, member)` pairs given in roster order. */
        pub(super) fn from_members(
            members: impl IntoIterator<Item = (SymbolHandle, SymbolHandle)>,
        ) -> Self {
            let key = |module: SymbolHandle| (module.arena_index(), module.generation());
            let mut members = members
                .into_iter()
                .map(|(module, member)| (key(module), member))
                .collect::<Vec<_>>();
            // Stable, so each module keeps its members in roster order.
            members.sort_by_key(|(module, _)| *module);
            let mut roster = Self {
                modules: Vec::new(),
                children: Vec::with_capacity(members.len()),
            };
            for (module, member) in members {
                let start = roster.children.len();
                roster.children.push(member);
                match roster.modules.last_mut() {
                    Some((last, _, end)) if *last == module => *end = start + 1,
                    _ => roster.modules.push((module, start, start + 1)),
                }
            }
            roster
        }

        pub(super) fn members(&self, module: SymbolHandle) -> &[SymbolHandle] {
            let key = (module.arena_index(), module.generation());
            match self
                .modules
                .binary_search_by_key(&key, |(module, _, _)| *module)
            {
                Ok(position) => {
                    let (_, start, end) = self.modules[position];
                    &self.children[start..end]
                }
                Err(_) => &[],
            }
        }
    }

    /*
        Selected bindings and their derived namespace environment belong to one
        immutable table frontier. Queries borrow these rows without rebuilding paths.
    */
    #[derive(Clone, Default)]
    pub(super) struct BindingSourceIndex {
        pub(super) by_source: arena::HandleKeyMap<SourceId, SourceLookupScope>,
        pub(super) has_selected_module: bool,
    }

    #[derive(Clone, Default)]
    pub(super) struct SourceLookupScope {
        pub(super) bindings: Vec<SourceScopedTopLevelBinding>,
        pub(super) imports: Vec<PreparedModuleImport>,
        pub(super) has_exact_import: bool,
    }

    /*
        `binding_position` addresses this scope's ordered binding batch. Derived
        spellings describe lookup only; visibility and package authority remain queries.
    */
    #[derive(Clone)]
    pub(super) struct PreparedModuleImport {
        pub(super) binding_position: usize,
        pub(super) logical: String,
        pub(super) module_leaf: Option<String>,
    }

    #[derive(Clone, Default)]
    pub(super) struct ModulePathIndex {
        pub(super) paths: arena::HandleKeyMap<SymbolHandle, Arc<str>>,
    }

    /*
        Candidates for a module-qualified lookup, keyed by how the match arms
        select them. `order` is the position in the original roster walk —
        root children then module symbols — so gathering buckets and sorting
        reproduces the scan's candidate order exactly, including a handle that
        appears in both halves (its two positions stay distinct).
    */
    #[derive(Clone, Default)]
    pub(super) struct ModuleScopeIndex {
        /* `name` → (order, handle). */
        pub(super) by_name: arena::HandleKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        /* `display_path("::")` → (order, handle). */
        pub(super) by_path: arena::HandleKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        /*
            Declared leaf name → (order, handle), for Domain-kind candidates
            only: the carrier-qualified arm is the sole leaf-shaped selector.
        */
        pub(super) domain_leaves: arena::HandleKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
    }

    /*
        Clones share one immutable lookup frontier, including its deferred work.
        A mutation detaches a shared cell; clearing a unique cell needs no new
        allocation. The cache is invisible to semantic equality and debug output.
    */
    macro_rules! index_cache {
    ($(#[$cache_meta:meta])* $cache:ident, $index:ty, $label:literal) => {
        $(#[$cache_meta])*
        #[derive(Clone, Default)]
        pub(super) struct $cache(pub(super) Arc<std::sync::OnceLock<$index>>);

        /*
            Lookup is derived from the owning symbol/source tables, including
            inactive selection. A restored graph starts with a cold local index.
        */
        impl record_storage::Record for $cache {
            fn write(&self, _: &mut record_storage::RecordWriter) -> Result<(), record_storage::RecordError> { Ok(()) }
            fn read<Context>(_: &mut record_storage::RecordReader<'_, Context>) -> Result<Self, record_storage::RecordError> { Ok(Self::default()) }
        }


        impl $cache {
            pub(super) fn clear(&mut self) {
                if let Some(cell) = Arc::get_mut(&mut self.0) {
                    cell.take();
                } else {
                    self.0 = Arc::default();
                }
            }
        }

        impl PartialEq for $cache {
            fn eq(&self, _: &Self) -> bool {
                true
            }
        }

        impl Eq for $cache {}

        impl std::fmt::Debug for $cache {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str($label)
            }
        }
    };
}

    index_cache!(
        /*
            The table's root children bucketed by full name and by leaf, built on the
            first top-level lookup. Every mutation that can change the root roster
            clears it, so it always describes this table; equality ignores it.
        */
        RootNameIndexCache,
        RootNameIndex,
        "RootNameIndexCache"
    );

    index_cache!(
        /*
            `source_scoped_top_level_bindings` bucketed by `reference_source`, built on
            the first scoped lookup. A finished table still gains bindings when an
            extension begins or a syntax-only import is registered, and both clear
            the index; equality ignores it.
        */
        BindingSourceIndexCache,
        BindingSourceIndex,
        "BindingSourceIndexCache"
    );

    index_cache!(
        /*
            `display_path("::")` per root child and module symbol, snapshotted on the
            first module-qualified or imported-path lookup. Handles minted after the
            snapshot miss the map and fall back to a walk. Every roster mutation
            clears it alongside `root_names`; equality ignores it.
        */
        ModulePathIndexCache,
        ModulePathIndex,
        "ModulePathIndexCache"
    );

    index_cache!(
        /*
            The module-qualified roster index is built on the first
            `find_module_qualified_reference` and cleared wherever `module_path_index`
            or `root_names` clears; equality ignores it.
        */
        ModuleScopeIndexCache,
        ModuleScopeIndex,
        "ModuleScopeIndexCache"
    );
}
use indices::{
    BindingSourceIndexCache, ModuleChildRoster, ModulePathIndex, ModulePathIndexCache,
    ModuleScopeIndex, ModuleScopeIndexCache, RootNameIndexCache, RootNameRows,
};
mod modules {
    /*
        Static declaration owners keep their source identities. The Build prelude
        supplies its admitted namespaces directly. Authored file aliases select
        members of their exact source through the file-qualified lookup.
    */

    use std::sync::Arc;

    use arena::HandleSpan;
    use source_text::{SourceId, SourceSpan};

    use super::indices::{BindingSourceIndex, PreparedModuleImport, SourceLookupScope};
    use super::name_from_source;
    use super::{SourceScopedTopLevelBinding, SymbolLookup, SymbolTable};
    use crate::source_symbols::{Symbol, SymbolHandle, SymbolKind, SymbolNameRef};

    impl SourceScopedTopLevelBinding {
        /// Supply an admitted Build protocol namespace to this stage.
        pub fn prelude_module(
            reference_source: SourceId,
            declaration_source: SourceId,
            namespace: &str,
        ) -> Self {
            Self {
                reference_source,
                declaration_source,
                target: super::BindingTarget::PreludeModule(Arc::from(namespace)),
                producer: super::BindingProducer::Loader,
                dependency: None,
            }
        }
    }

    impl SymbolTable {
        /// File aliases available for source lookup diagnostics.
        pub fn source_file_aliases(&self, source: SourceId) -> impl Iterator<Item = &str> {
            self.source_scoped_bindings_for(source)
                .iter()
                .filter_map(|binding| match &binding.target {
                    super::BindingTarget::FileAlias(alias) => Some(alias.as_ref()),
                    _ => None,
                })
        }

        /// Compiler-supplied namespace paths visible in a Build source.
        pub fn prelude_module_paths(&self, source: SourceId) -> impl Iterator<Item = &str> {
            self.source_scoped_bindings_for(source)
                .iter()
                .filter_map(SourceScopedTopLevelBinding::prelude_namespace)
        }

        /*
            Publish the logical namespace of one source without relocating any
            existing declaration or builtin slot in the symbol hierarchy.
        */
        pub fn register_source_module<'name>(
            &mut self,
            source: SourceId,
            members: impl IntoIterator<Item = (&'name str, SourceSpan)>,
        ) -> Result<(), &'static str> {
            if self.source_module(source).is_valid() {
                return Err("a source file may declare only one module");
            }
            self.root_names.clear();
            self.binding_source_index.clear();
            self.module_path_index.clear();
            self.module_scope_index.clear();
            let mut parent = self.root;
            for (name, span) in members {
                let existing =
                    self.module_symbols
                        .iter()
                        .map(|(_, handle)| *handle)
                        .find(|handle| {
                            self.get(*handle).parent == parent
                                && self.name(*handle) == name
                                && self.symbol_provenance_source_span(*handle).is_some_and(
                                    |previous| self.same_source_package(previous, span),
                                )
                        });
                parent = if let Some(existing) = existing {
                    existing
                } else {
                    let symbol_name = self.names.insert(name_from_source(
                        SymbolNameRef::OwnedSource {
                            value: name,
                            source_span: span,
                        },
                        self.sources.as_deref(),
                    ));
                    let handle = self.symbols.insert_supplemental_child(
                        parent,
                        Symbol {
                            parent,
                            declaration_owner: SymbolHandle::invalid(),
                            children: HandleSpan::empty(),
                            kind: SymbolKind::Module,
                            name: symbol_name,
                            generated_from: SymbolHandle::invalid(),
                        },
                    );
                    self.module_symbols.insert(handle);
                    handle
                };
            }
            if parent == self.root {
                return Err("module declaration requires a nonempty logical path");
            }
            if source.0 >= self.source_module_index.len() {
                Arc::make_mut(&mut self.source_module_index)
                    .resize(source.0 + 1, SymbolHandle::invalid());
            }
            Arc::make_mut(&mut self.source_module_index)[source.0] = parent;
            Ok(())
        }

        /*
            Bind `use path.omg as alias;` for syntax-only callers, against the
            sources this table was given (`wiki/spec/language/modules.md`,
            "Names and imports"). An unrooted path names a file of the
            importer's own package, `{omega}/language/core/...` a toolchain
            Core file, and `{dep}/...` a file of the package the importer's
            direct dependency `dep` names. The importer's own dependency scope
            breaks a tie between instances of one file. A compiler-loaded alias
            already carries exact custody and takes precedence. A path that
            selects no one file binds nothing, so its uses stay unresolved.
        */
        pub fn register_source_file_import(
            &mut self,
            reference_source: SourceId,
            root: Option<&str>,
            path: &str,
            alias: &str,
        ) {
            enum Owner {
                Package,
                Core,
                Dependency(source_provenance::PackageKeyIdentity),
            }
            if self.source_scoped_top_level_bindings.iter().any(|binding| {
                binding.reference_source == reference_source
                    && matches!(&binding.target, super::BindingTarget::FileAlias(bound) if bound.as_ref() == alias)
            }) {
                return;
            }
            let Some(sources) = self.sources.as_deref() else {
                return;
            };
            let Some(importer) = sources.get(reference_source) else {
                return;
            };
            let (owner, relative) = match root {
                None => (Owner::Package, path),
                Some("omega") => match path.strip_prefix("language/core/") {
                    Some(relative) => (Owner::Core, relative),
                    None => return,
                },
                Some(dependency) => match importer
                    .package_identity
                    .and_then(|requester| sources.product_dependency_target(requester, dependency))
                {
                    Some(target) => (Owner::Dependency(target), path),
                    None => return,
                },
            };
            let candidates = sources
                .selected_files()
                .filter(|file| {
                    let owned = match owner {
                        Owner::Package => {
                            file.package_root == importer.package_root
                                && file.package_identity == importer.package_identity
                        }
                        Owner::Core => file.origin == source_admission::SourceOrigin::Toolchain && file.path != file.package_root,
                        Owner::Dependency(target) => file.package_identity == Some(target),
                    };
                    owned && file.logical_filename().is_ok_and(|name| name == relative)
                })
                .collect::<Vec<_>>();
            let same_scope = candidates
                .iter()
                .filter(|file| file.dependency_scope == importer.dependency_scope)
                .collect::<Vec<_>>();
            let target = match (same_scope.as_slice(), candidates.as_slice()) {
                ([file], _) => file.source_id,
                ([], [file]) => file.source_id,
                _ => return,
            };
            let mut binding =
                SourceScopedTopLevelBinding::file_import(reference_source, target, alias);
            binding.producer = super::BindingProducer::AuthoredFile {
                root: root.map(Arc::from),
                path: Arc::from(path),
            };
            Arc::make_mut(&mut self.source_scoped_top_level_bindings).push(binding);
            self.binding_source_index.clear();
        }

        /// Find the declaration named by a supplied Build namespace.
        pub fn prelude_module_target(
            &self,
            reference_source: SourceId,
            path: &str,
        ) -> Option<SymbolHandle> {
            for binding in self.source_scoped_bindings_for(reference_source) {
                let Some(import) = binding.prelude_namespace() else {
                    continue;
                };
                if import != path {
                    continue;
                }
                if !self.source_module(binding.declaration_source).is_valid() {
                    return None;
                }
                return unique(
                    self.child_handles(self.root)
                        .into_iter()
                        .flatten()
                        .chain(self.module_symbols.iter().map(|(_, symbol)| *symbol))
                        .filter(|candidate| {
                            self.import_owner_matches(binding, *candidate)
                                && self.indexed_display_path(*candidate) == import
                        }),
                );
            }
            None
        }

        pub fn source_module(&self, source: SourceId) -> SymbolHandle {
            self.source_module_index
                .get(source.0)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid)
        }

        pub(super) fn source_binding_is_selected(
            &self,
            binding: &SourceScopedTopLevelBinding,
        ) -> bool {
            match self.sources.as_deref() {
                Some(sources) => {
                    sources.source_is_selected(binding.reference_source)
                        && sources.source_is_selected(binding.declaration_source)
                }
                None => true,
            }
        }

        /*
            Selected files contributing to this namespace or any nested namespace.
            The module's original spelling owner is not its activation membership.
        */
        pub(super) fn module_source_ids(
            &self,
            module: SymbolHandle,
        ) -> impl Iterator<Item = SourceId> + '_ {
            self.source_module_index
                .iter()
                .enumerate()
                .filter_map(move |(position, leaf)| {
                    let source = SourceId(position);
                    if self
                        .sources
                        .as_deref()
                        .is_some_and(|sources| !sources.source_is_selected(source))
                    {
                        return None;
                    }
                    let mut ancestor = *leaf;
                    while ancestor.is_valid() {
                        if ancestor == module {
                            return Some(source);
                        }
                        ancestor = self.get(ancestor).parent;
                    }
                    None
                })
        }

        /// Resolve a domain in the importing file or against the carrier named at the use.
        pub fn domain_name_reaches(
            &self,
            domain: SymbolHandle,
            authored: &str,
            reference: SourceSpan,
        ) -> bool {
            if authored.starts_with('|') {
                return self.top_level_candidates_from_source(
                    authored,
                    &[SymbolKind::Domain],
                    reference,
                ).contains(&domain);
            }
            let qualified = self.indexed_display_path(domain);
            if qualified.as_ref() == authored {
                return if authored.contains("::") {
                    self.find_top_level_by_name_and_kinds_from_source(
                        authored,
                        &[SymbolKind::Domain],
                        reference,
                    ) == Some(domain)
                } else {
                    self.source_domain_is_exposed(reference, domain)
                };
            }
            if self.exposed_refinement_path(domain, authored, reference) {
                return true;
            }
            let domain_name = self.name(domain);
            let domain_module = self.symbol_module(domain);
            if !domain_name.contains("::")
                && let Some((carrier, leaf)) = authored.rsplit_once("::")
            {
                if leaf != domain_name {
                    return false;
                }
                let carrier_symbol = self.find_top_level_by_name_and_kinds_from_source(
                    carrier,
                    &[
                        SymbolKind::BuiltinType,
                        SymbolKind::Data,
                        SymbolKind::Machine,
                        SymbolKind::Trait,
                    ],
                    reference,
                );
                let Some(carrier_symbol) = carrier_symbol else {
                    return false;
                };
                if self.get(carrier_symbol).kind != SymbolKind::BuiltinType {
                    let domain_source = self
                        .symbol_provenance_source_span(domain)
                        .map(|span| span.source_id);
                    let carrier_source = self
                        .symbol_provenance_source_span(carrier_symbol)
                        .map(|span| span.source_id);
                    if domain_source.is_none() || domain_source != carrier_source {
                        return false;
                    }
                }
            }
            if !domain_module.is_valid() {
                return same_semantic_name(domain_name, authored)
                    && self.source_domain_is_exposed(reference, domain);
            }
            if reference.span.start == reference.span.end {
                return false;
            }
            if self.source_module(reference.source_id) == domain_module {
                return same_semantic_name(domain_name, authored);
            }
            if !authored.contains("::") {
                return same_semantic_name(domain_name, authored)
                    && self.source_domain_is_exposed(reference, domain);
            }
            self.carrier_qualified_domain_reaches(domain, authored, reference)
        }

        /// A carrier-relative refinement keeps the complete chain of parent
        /// names: `Positive::Small` abbreviates `i32::Positive::Small`.
        fn exposed_refinement_path(
            &self,
            domain: SymbolHandle,
            authored: &str,
            reference: SourceSpan,
        ) -> bool {
            authored.contains("::")
                && self
                    .name(domain)
                    .split_once("::")
                    .is_some_and(|(_, members)| members == authored)
                && reference.span.start != reference.span.end
                && self.source_domain_is_exposed(reference, domain)
        }

        /*
            Whether a carrier-qualified spelling (`Carrier::Leaf`) selects an
            exposed module-owned domain. The authored carrier must resolve to the
            same exact carrier the declaring source attached the domain to -- a
            caller's same-spelled type cannot redirect the attachment -- and an
            import must expose the domain: either a narrow import of the exact
            declaration or a broad import of its declaring module, which exposes
            only the domains that module directly declares.
        */
        pub fn carrier_qualified_domain_reaches(
            &self,
            domain: SymbolHandle,
            authored: &str,
            reference: SourceSpan,
        ) -> bool {
            if reference.span.start == reference.span.end {
                return false;
            }
            if self.exposed_refinement_path(domain, authored, reference) {
                return true;
            }
            let Some((declared_carrier, declared_leaf)) = self.name(domain).rsplit_once("::")
            else {
                return false;
            };
            let Some((authored_carrier, authored_leaf)) = authored.rsplit_once("::") else {
                return false;
            };
            if authored_leaf != declared_leaf {
                return false;
            }
            const CARRIER_KINDS: [SymbolKind; 4] = [
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::Machine,
                SymbolKind::Trait,
            ];
            let Some(authored_carrier_symbol) = self.find_top_level_by_name_and_kinds_from_source(
                authored_carrier,
                &CARRIER_KINDS,
                reference,
            ) else {
                return false;
            };
            let declared_carrier_symbol =
                self.symbol_provenance_source_span(domain).and_then(|span| {
                    self.find_top_level_by_name_and_kinds_from_source(
                        declared_carrier,
                        &CARRIER_KINDS,
                        span,
                    )
                });
            if authored_carrier_symbol
                != declared_carrier_symbol.unwrap_or_else(SymbolHandle::invalid)
            {
                return false;
            }
            self.source_domain_is_exposed(reference, domain)
        }

        /// Whether a compiler-supplied prelude binding exposes this declaration.
        pub(super) fn prelude_exposes_symbol(
            &self,
            reference: SourceId,
            symbol: SymbolHandle,
        ) -> bool {
            let Some(declaration) = self.symbol_source_span(symbol) else {
                return false;
            };
            self.source_scoped_bindings_for(reference)
                .iter()
                .any(|binding| {
                    let Some(import) = binding.prelude_namespace() else {
                        return false;
                    };
                    (self.import_owner_matches(binding, symbol)
                        && self.indexed_display_path(symbol) == import)
                        || (!self.source_module(declaration.source_id).is_valid()
                            && (binding.declaration_source == declaration.source_id))
                })
        }

        /*
            Loading a package unit does not expose its domains. This shared
            predicate follows the importing file's exact selection: a domain,
            its declaring module, or the module-less package root. Visibility
            and the attached carrier's authority remain checked at the use site.
        */
        pub fn prelude_module_exposes_domain(
            &self,
            reference: SourceSpan,
            path: &str,
            domain: SymbolHandle,
        ) -> bool {
            self.get(domain).kind == SymbolKind::Domain
                && self.source_reference_can_see_symbol(reference, domain)
                && self
                    .source_scoped_bindings_for(reference.source_id)
                    .iter()
                    .any(|binding| {
                        binding
                            .prelude_namespace()
                            .is_some_and(|import| import == path)
                            && self.binding_exposes_member(binding, domain)
                    })
        }

        pub fn source_domain_is_exposed(
            &self,
            reference: SourceSpan,
            domain: SymbolHandle,
        ) -> bool {
            if !self.source_reference_can_see_symbol(reference, domain) {
                return false;
            }
            let module = self.symbol_module(domain);
            let local = self
                .symbol_provenance_source_span(domain)
                .is_some_and(|span| {
                    span.source_id == reference.source_id
                });
            local
                || (!module.is_valid() && self.is_ambient_toolchain_symbol(domain))
                || self
                    .prelude_module_paths(reference.source_id)
                    .any(|path| self.prelude_module_exposes_domain(reference, path, domain))
        }

        pub(super) fn binding_exposes_member(
            &self,
            binding: &SourceScopedTopLevelBinding,
            symbol: SymbolHandle,
        ) -> bool {
            let Some(import) = binding.prelude_namespace() else {
                return false;
            };
            if !self.import_owner_matches(binding, symbol) {
                return false;
            }
            let module = self.source_module(binding.declaration_source);
            if !module.is_valid() {
                return true;
            }
            self.indexed_display_path(symbol) == import
                || (self.symbol_module(symbol).is_valid()
                    && self.indexed_display_path(self.symbol_module(symbol)) == import)
        }

        pub fn symbol_module(&self, symbol: SymbolHandle) -> SymbolHandle {
            self.symbol_provenance_source_span(symbol)
                .map(|span| self.source_module(span.source_id))
                .unwrap_or_else(SymbolHandle::invalid)
        }

        /*
            Semantic path ownership includes the authored module while physical
            declaration storage preserves its original root child range.
        */
        pub fn namespace_parent(&self, symbol: SymbolHandle) -> SymbolHandle {
            let parent = self.get(symbol).parent;
            if (parent == self.root || !parent.is_valid())
                && self.get(symbol).kind != SymbolKind::Module
            {
                let module = self.symbol_module(symbol);
                if module.is_valid() {
                    return module;
                }
            }
            parent
        }

        /*
            Nested module symbols, then the root children declared in this
            module's files, read from the root name index rather than a scan
            of every root child per lookup. Source selection applies here
            because activation does not rebuild that index.
        */
        pub(super) fn module_children(&self, module: SymbolHandle) -> Vec<SymbolHandle> {
            self.module_child_roster()
                .members(module)
                .iter()
                .copied()
                .filter(|symbol| self.symbol_source_is_selected(*symbol))
                .collect()
        }

        /*
            Materialize the selected import environment once for this table frontier.
            It owns no authority: all candidate-specific admission remains below.
        */
        pub(super) fn source_binding_index(&self) -> &BindingSourceIndex {
            self.binding_source_index.0.get_or_init(|| {
                let mut index = BindingSourceIndex {
                    has_selected_module: self
                        .module_symbols
                        .iter()
                        .any(|(_, symbol)| self.symbol_source_is_selected(*symbol)),
                    ..Default::default()
                };
                for binding in self.source_scoped_top_level_bindings.iter() {
                    if !self.source_binding_is_selected(binding) {
                        continue;
                    }
                    let scope = index.by_source.entry(binding.reference_source).or_default();
                    if let Some(import) = binding.prelude_namespace() {
                        let logical = import.to_owned();
                        let module = self.source_module(binding.declaration_source);
                        let module_leaf = (module.is_valid()
                            && self.indexed_display_path(module) == logical.as_str())
                        .then(|| self.name(module).to_string());
                        scope.has_exact_import = true;
                        scope.imports.push(PreparedModuleImport {
                            binding_position: scope.bindings.len(),
                            logical,
                            module_leaf,
                        });
                    }
                    scope.bindings.push(binding.clone());
                }
                index
            })
        }

        fn source_lookup_scope(&self, reference_source: SourceId) -> &SourceLookupScope {
            static EMPTY: SourceLookupScope = SourceLookupScope {
                bindings: Vec::new(),
                imports: Vec::new(),
                has_exact_import: false,
            };
            self.source_binding_index()
                .by_source
                .get(&reference_source)
                .unwrap_or(&EMPTY)
        }

        pub(super) fn has_namespace_context(&self, reference: SourceSpan) -> bool {
            self.source_file(reference).is_some()
                || self.source_binding_index().has_selected_module
                || self
                    .source_lookup_scope(reference.source_id)
                    .has_exact_import
        }

        pub(super) fn select_namespace_candidates(
            &self,
            candidates: &[SymbolHandle],
            name: &str,
            reference: SourceSpan,
        ) -> Vec<SymbolHandle> {
            let current_module = self.source_module(reference.source_id);
            let local = candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    self.name(*candidate) == name
                        && self.symbol_module(*candidate) == current_module
                        && self
                            .symbol_provenance_source_span(*candidate)
                            .is_some_and(|span| span.source_id == reference.source_id)
                })
                .collect::<Vec<_>>();
            /*
                A module's own declaration outranks an import, except for
                domains: distinct visible domains competing for one spelling
                reject, and declaring one in this module wins nothing
                (`wiki/spec/language/modules.md#resolution`). Both stay
                candidates so the lookup reports the ambiguity.
            */
            let local_domains = local
                .iter()
                .any(|candidate| self.get(*candidate).kind == SymbolKind::Domain);
            if !local.is_empty() && !local_domains {
                return local;
            }
            let source_scope = self.source_lookup_scope(reference.source_id);
            let imported = candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    source_scope.imports.iter().any(|scope| {
                        let binding = &source_scope.bindings[scope.binding_position];
                        let logical = &scope.logical;
                        if !self.import_owner_matches(binding, *candidate) {
                            return false;
                        }
                        self.indexed_display_path(*candidate) == logical.as_str()
                            || (self.name(*candidate) == name
                                && !self.source_module(binding.declaration_source).is_valid()
                                && self.symbol_provenance_source_span(*candidate).is_some_and(
                                    |span| {
                                        binding.declaration_source == span.source_id
                                    },
                                ))
                    })
                })
                .collect::<Vec<_>>();
            if local_domains {
                let mut competing = local;
                for candidate in imported {
                    if self.get(candidate).kind == SymbolKind::Domain
                        && !competing.contains(&candidate)
                    {
                        competing.push(candidate);
                    }
                }
                return competing;
            }
            if !imported.is_empty() {
                return imported;
            }
            candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    self.name(*candidate) == name
                        && !self.symbol_module(*candidate).is_valid()
                        && self.is_ambient_toolchain_symbol(*candidate)
                })
                .collect()
        }

        fn import_owner_matches(
            &self,
            binding: &SourceScopedTopLevelBinding,
            candidate: SymbolHandle,
        ) -> bool {
            if binding.prelude_namespace().is_none() {
                return false;
            }
            self.symbol_provenance_source_span(candidate)
                .is_some_and(|span| {
                    if self.get(candidate).kind == SymbolKind::Module {
                        self.same_source_package(
                            span,
                            SourceSpan::new(binding.declaration_source, span.span),
                        )
                    } else {
                        binding.declaration_source == span.source_id
                    }
                })
        }

        /*
            An empty roster is an authoritative failed logical lookup. `None`
            leaves an unqualified lookup to its ordinary source-scope precedence.
        */
        pub(super) fn find_module_qualified_candidates(
            &self,
            name: &str,
            kinds: &[SymbolKind],
            reference: SourceSpan,
            matches_candidate: &mut impl FnMut(SymbolHandle) -> bool,
        ) -> Option<Vec<SymbolHandle>> {
            if self.module_symbols.is_empty() && !self.has_namespace_context(reference) {
                return None;
            }
            let qualified = name.contains("::");
            if !qualified && !kinds.contains(&SymbolKind::Module) {
                return None;
            }
            let current_module = self.source_module(reference.source_id);
            let source_scope = self.source_lookup_scope(reference.source_id);
            let import_bindings = &source_scope.imports;
            let needs_candidate_path = qualified || !import_bindings.is_empty();
            let mut matches = Vec::new();
            let mut module_local_matches = Vec::new();
            let mut carrier_domain_matches = Vec::new();
            let index = self.module_scope_index();
            let mut candidate_pool: Vec<(u32, SymbolHandle)> = Vec::new();
            let gather = |pool: &mut Vec<(u32, SymbolHandle)>, key: &str| {
                if let Some(entries) = index.by_name.get(key) {
                    pool.extend_from_slice(entries);
                }
            };
            gather(&mut candidate_pool, name);
            if qualified {
                if let Some(entries) = index.by_path.get(name) {
                    candidate_pool.extend_from_slice(entries);
                }
                if let Some((_, authored_leaf)) = name.rsplit_once("::")
                    && let Some(entries) = index.domain_leaves.get(authored_leaf)
                {
                    candidate_pool.extend_from_slice(entries);
                }
                for scope in import_bindings {
                    let logical = scope.logical.as_str();
                    if let Some(short) = logical.rsplit("::").next()
                        && let Some(suffix) = name
                            .strip_prefix(short)
                            .and_then(|rest| rest.strip_prefix("::"))
                    {
                        let key = format!("{logical}::{suffix}");
                        if let Some(entries) = index.by_path.get(key.as_str()) {
                            candidate_pool.extend_from_slice(entries);
                        }
                    }
                    if let Some(module_leaf) = scope.module_leaf.as_deref()
                        && let Some(suffix) = name
                            .strip_prefix(module_leaf)
                            .and_then(|rest| rest.strip_prefix("::"))
                    {
                        let key = format!("{logical}::{suffix}");
                        if let Some(entries) = index.by_path.get(key.as_str()) {
                            candidate_pool.extend_from_slice(entries);
                        }
                    }
                }
            }
            candidate_pool.sort_unstable_by_key(|(order, _)| *order);
            candidate_pool.dedup_by_key(|(order, _)| *order);
            for (_, candidate) in candidate_pool {
                let candidate_kind = self.get(candidate).kind;
                let is_module = candidate_kind == SymbolKind::Module;
                if !kinds.contains(&candidate_kind)
                    || !matches_candidate(candidate)
                    || !self.source_reference_can_see_symbol(reference, candidate)
                {
                    continue;
                }
                let candidate_path =
                    needs_candidate_path.then(|| self.indexed_display_path(candidate));
                let same_file = self.symbol_provenance_source_span(candidate)
                    .is_some_and(|span| span.source_id == reference.source_id);
                if qualified && same_file
                    && reference.span.start != reference.span.end
                    && current_module.is_valid()
                    && self.symbol_module(candidate) == current_module
                    && self.name(candidate) == name
                {
                    module_local_matches.push(candidate);
                }
                let direct_path = qualified
                    && (candidate_path.as_deref() == Some(name)
                        || (current_module.is_valid()
                            && self.symbol_module(candidate) == current_module
                            && self.name(candidate) == name));
                let same_package = self
                    .symbol_provenance_source_span(candidate)
                    .is_some_and(|span| self.same_source_package(reference, span));
                let installed_path = qualified
                    && !is_module
                    && !self.symbol_module(candidate).is_valid()
                    && self.name(candidate) == name
                    && self.is_ambient_toolchain_symbol(candidate);
                let imported_path = import_bindings.iter().any(|scope| {
                    let binding = &source_scope.bindings[scope.binding_position];
                    let logical = scope.logical.as_str();
                    if !self.import_owner_matches(binding, candidate) {
                        return false;
                    }
                    if !qualified {
                        return is_module
                            && candidate_path.as_deref() == Some(logical)
                            && self.name(candidate) == name;
                    }
                    if candidate_path.as_deref() == Some(name) {
                        return true;
                    }
                    if let Some(short) = logical.rsplit("::").next()
                        && name
                            .strip_prefix(short)
                            .and_then(|suffix| suffix.strip_prefix("::"))
                            .is_some_and(|suffix| {
                                candidate_path
                                    .as_deref()
                                    .and_then(|path| path.strip_prefix(logical))
                                    .and_then(|rest| rest.strip_prefix("::"))
                                    == Some(suffix)
                            })
                    {
                        return true;
                    }
                    if is_module {
                        return false;
                    }
                    if let Some(module_leaf) = scope.module_leaf.as_deref() {
                        return name
                            .strip_prefix(module_leaf)
                            .and_then(|suffix| suffix.strip_prefix("::"))
                            .is_some_and(|suffix| {
                                candidate_path
                                    .as_deref()
                                    .and_then(|path| path.strip_prefix(logical))
                                    .and_then(|rest| rest.strip_prefix("::"))
                                    == Some(suffix)
                            });
                    }
                    false
                });
                if qualified
                    && self.get(candidate).kind == SymbolKind::Domain
                    && self.carrier_qualified_domain_reaches(candidate, name, reference)
                {
                    carrier_domain_matches.push(candidate);
                }
                if installed_path
                    || (qualified && same_file && self.name(candidate) == name)
                    || (direct_path && reference.span.start == reference.span.end && same_package)
                    || imported_path
                    || (!qualified
                        && is_module
                        && self.get(candidate).parent == self.root
                        && self.name(candidate) == name
                        && same_package)
                {
                    matches.push(candidate);
                }
            }
            if !module_local_matches.is_empty() {
                /*
                    A module's own domain does not outrank a visible imported
                    domain of the same spelling; both stay candidates so the
                    lookup reports the ambiguity
                    (`wiki/spec/language/modules.md#resolution`).
                */
                if module_local_matches
                    .iter()
                    .any(|candidate| self.get(*candidate).kind == SymbolKind::Domain)
                {
                    for candidate in matches.into_iter().chain(carrier_domain_matches) {
                        if self.get(candidate).kind == SymbolKind::Domain
                            && !module_local_matches.contains(&candidate)
                        {
                            module_local_matches.push(candidate);
                        }
                    }
                }
                Some(module_local_matches)
            } else if qualified {
                if matches.is_empty() && carrier_domain_matches.len() == 1 {
                    return Some(carrier_domain_matches);
                }
                Some(matches)
            } else if matches.is_empty() {
                None
            } else {
                Some(matches)
            }
        }
    }

    /*
        The `a::b::c` spelling of a displayable member path, shared by the path
        member rosters that previously spelled the iter/map/collect chain inline.
    */
    pub trait JoinedPath {
        /* Members joined with `::`. */
        fn joined_path(&self) -> String;
        /* Members joined with a caller-chosen separator. */
        fn joined_path_with(&self, separator: &str) -> String;
    }

    impl<T: std::fmt::Display> JoinedPath for [T] {
        fn joined_path(&self) -> String {
            self.joined_path_with("::")
        }

        fn joined_path_with(&self, separator: &str) -> String {
            joined_members(self.iter(), separator)
        }
    }

    impl<T: std::fmt::Display> JoinedPath for arena::RootRows<'_, T> {
        fn joined_path(&self) -> String {
            self.joined_path_with("::")
        }

        fn joined_path_with(&self, separator: &str) -> String {
            joined_members(self.iter(), separator)
        }
    }

    fn joined_members<T: std::fmt::Display>(
        members: impl Iterator<Item = T>,
        separator: &str,
    ) -> String {
        members
            .map(|member| member.to_string())
            .collect::<Vec<_>>()
            .join(separator)
    }

    /*
        A relative domain spelling matches its declared name either exactly or by
        leaf, mirroring the semantic-name rule the resolved-lookup exposure pool
        shares with this table law.
    */
    pub fn same_semantic_name(left: &str, right: &str) -> bool {
        left == right
            || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
            || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
    }

    fn unique(candidates: impl Iterator<Item = SymbolHandle>) -> Option<SymbolHandle> {
        SymbolLookup::from_candidates(candidates).unique()
    }
    #[cfg(test)]
    mod retained_path_tests {
        use super::JoinedPath;

        #[test]
        fn logical_and_contiguous_paths_have_identical_spelling() {
            let mut paths = arena::RootedArena::new();
            let span = paths.insert_many(["root".to_owned(), "member".to_owned()]);
            let retained = paths.clone();
            paths.get_mut(span.handle_at(1)).push_str("_changed");
            let old = retained.span_or_empty(span);
            let current = paths.span_or_empty(span);
            assert_eq!(old.joined_path(), ["root", "member"].joined_path());
            assert_eq!(current.joined_path_with("."), "root.member_changed");
            let (last, prefix) = current.split_last().unwrap();
            assert_eq!(last, "member_changed");
            assert_eq!(prefix.joined_path(), "root");
            assert!(prefix.slice(0..0).split_last().is_none());
            assert_eq!(prefix.slice(0..0).joined_path(), "");
        }
    }
}

pub use modules::{JoinedPath, same_semantic_name};
mod source_queries {
    /*
        Source and package queries: span-to-text lookup, symbol provenance,
        and package-identity checks, extracted from `table` so storage and
        name navigation stay the headline.
    */

    use super::*;

    impl SymbolTable {
        /*
            Generated declarations follow authored provenance. Missing provenance
            retains the existing unscoped declaration behavior.
        */
        #[inline]
        pub fn symbol_is_in_dependency_scope(
            &self,
            symbol: crate::source_symbols::SymbolHandle,
            scope: source_provenance::DependencyScope,
        ) -> bool {
            self.symbol_provenance_source_span(symbol)
                .and_then(|span| self.source_file(span))
                .is_none_or(|file| file.dependency_scope == scope)
        }

        pub fn source_text(&self, source_span: SourceSpan) -> &str {
            self.sources
                .as_deref()
                .map(|sources| sources.text_at(source_span))
                .unwrap_or("")
        }

        /* Source declaration span retained for an authored symbol. */
        pub fn symbol_source_span(&self, symbol: SymbolHandle) -> Option<SourceSpan> {
            self.names.get(self.get(symbol).name).source_span()
        }

        /*
            Exact authored provenance span for an authored or compiler-generated
            symbol. Generated symbols follow their mandatory derivation origin.
        */
        pub fn symbol_provenance_source_span(&self, symbol: SymbolHandle) -> Option<SourceSpan> {
            self.provenance_source_span(symbol)
        }

        /*
            File metadata for one retained authored span. Generated/source-free
            trees return `None` instead of inventing a presentation path.
        */
        pub fn source_file(&self, source_span: SourceSpan) -> Option<&SourceFile> {
            self.sources.as_deref()?.file_at(source_span)
        }

        pub fn bundled_core_file(
            &self,
            source_span: SourceSpan,
        ) -> Option<language_core::bundled_files::BundledFile> {
            self.sources
                .as_deref()?
                .bundled_core_file(source_span.source_id)
        }

        /*
            The retained authored source map, shared without copying. Consumers
            that resolve diagnostic locations on a path where the program moves
            away clone this handle rather than reading storage back.
        */
        pub fn source_map(&self) -> Option<std::sync::Arc<crate::source_inputs::SourceMap>> {
            self.sources.clone()
        }

        /*
            Whether `symbol` was authored in the toolchain-owned build prelude
            translation unit.
        */
        /*
            The sealed intrinsic catalog row `symbol` is declared by, if any.
            Generated symbols have no authored declaration span and so never
            match: compiler custody follows the authored bundled declaration.
        */
        pub fn sealed_intrinsic(
            &self,
            symbol: SymbolHandle,
        ) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
            // A handle this table did not mint, or no longer holds, declares
            // nothing here; reading its span would refuse the handle itself.
            if !self.symbols().nodes().is_valid(symbol) {
                return None;
            }
            let bundled = self.bundled_core_file(self.symbol_source_span(symbol)?)?;
            let declaration = self.display_path(symbol, "::");
            language_core::sealed_intrinsics::SealedIntrinsicCatalog::classify_bundled(
                bundled,
                &declaration,
            )
        }

        /*
            The bundled core source `symbol` is declared in, if any. Callers
            pair this with their own exact identity check.
        */
        pub fn core_source(
            &self,
            symbol: SymbolHandle,
        ) -> Option<language_core::core_vocabulary::CoreSource> {
            // A handle this table did not mint, or no longer holds, declares
            // nothing here; reading its span would refuse the handle itself.
            if !self.symbols().nodes().is_valid(symbol) {
                return None;
            }
            let bundled = self.bundled_core_file(self.symbol_source_span(symbol)?)?;
            language_core::core_vocabulary::CoreVocabularyCatalog::source_bundled(bundled)
        }

        /*
            The core vocabulary row `symbol` is declared by, if any. This is
            recognition only; it never implies compiler custody.
        */
        pub fn core_vocabulary(
            &self,
            symbol: SymbolHandle,
        ) -> Option<language_core::core_vocabulary::CoreVocabulary> {
            // A handle this table did not mint, or no longer holds, declares
            // nothing here; reading its span would refuse the handle itself.
            if !self.symbols().nodes().is_valid(symbol) {
                return None;
            }
            let bundled = self.bundled_core_file(self.symbol_source_span(symbol)?)?;
            let declaration = self.display_path(symbol, "::");
            language_core::core_vocabulary::CoreVocabularyCatalog::classify_bundled(
                bundled,
                &declaration,
            )
        }

        /*
            The catalog row for `symbol` under the authored declaration path
            its owning tree records (a machine's declared name, for example,
            which keeps target-selected realizations in their source spelling).
            The symbol's own file, origin and bytes still decide custody.
        */
        pub fn sealed_declaration(
            &self,
            symbol: SymbolHandle,
            declaration: &str,
        ) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
            // A handle this table did not mint, or no longer holds, declares
            // nothing here; reading its span would refuse the handle itself.
            if !self.symbols().nodes().is_valid(symbol) {
                return None;
            }
            let bundled = self.bundled_core_file(self.symbol_source_span(symbol)?)?;
            language_core::sealed_intrinsics::SealedIntrinsicCatalog::classify_bundled(
                bundled,
                declaration,
            )
        }

        /*
            The catalog row for member `member` of the authored declaration
            `owner`. Specialized copies of a trait requirement carry no
            authored span or namespace of their own, so callers that hold one
            ask about the authored owner and the member's name.
        */
        pub fn sealed_member_intrinsic(
            &self,
            owner: SymbolHandle,
            member: &str,
        ) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
            if !owner.is_valid() {
                return None;
            }
            let bundled = self.bundled_core_file(self.symbol_source_span(owner)?)?;
            let declaration = format!("{}::{member}", self.display_path(owner, "::"));
            language_core::sealed_intrinsics::SealedIntrinsicCatalog::classify_bundled(
                bundled,
                &declaration,
            )
        }

        pub fn is_build_prelude_symbol(&self, symbol: SymbolHandle) -> bool {
            self.symbol_provenance_source_span(symbol)
                .and_then(|span| self.source_file(span))
                .is_some_and(|file| {
                    file.origin == source_admission::SourceOrigin::Toolchain
                        && source_admission::source_identity::is_build_prelude_path(&file.path)
                })
        }

        /*
            Product-scope dependency `alias` authorized for the package that owns
            `occurrence`. `None` means the occurrence carries no package identity,
            no retained sources, or no such product edge -- callers treat every
            one of those as "not an authorized product alias", never as license
            to widen the search.
        */
        pub fn product_dependency_target(
            &self,
            occurrence: SourceSpan,
            alias: &str,
        ) -> Option<PackageKeyIdentity> {
            self.sources
                .as_deref()?
                .product_dependency_at(occurrence, alias)
        }

        /*
            Executed Build descriptions keep their original source occurrence.
            Product admission may seal that anonymous source as a new instance;
            only the source producer's exact correspondence rejoins its namespace.
            Without one, the original occurrence remains subject to ordinary refusal.
        */
        pub fn rejoin_product_occurrence(&self, occurrence: SourceSpan) -> SourceSpan {
            let selected = self.sources.as_deref().and_then(|sources| {
                let original = sources.get(occurrence.source_id)?;
                sources.rejoined_source_id(original)
            });
            selected.map_or(occurrence, |source| {
                SourceSpan::new(source, occurrence.span)
            })
        }

        /*
            Resolve a designated product operand under its author's package scope.
            Callers supply declaration candidates and their public visibility; this
            never extends ordinary source lookup. Exact module paths take precedence
            over same-package short names. Operator overloads at one exact path name
            one family, represented by its first declaration in candidate order.
        */
        pub fn find_product_declaration_from_source(
            &self,
            path: &str,
            occurrence: SourceSpan,
            candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
        ) -> Option<SymbolHandle> {
            self.find_declaration_from_source(path, occurrence, candidates, false, &[])
        }

        /*
            [`Self::find_product_declaration_from_source`] for a BUILD operand,
            which may also name a public toolchain declaration.

            The toolchain injects its own core into every product without an
            authored dependency, so a core declaration has no package instance an
            unqualified operand could share and no manifest an alias could name.
            A build that selects a provider for `ExtentRootProvider` is naming that
            exact declaration. Ordinary call resolution keeps the narrower rule:
            admitting core there changes which declaration a call selects.
            `token_bound_machines` identifies the machine declarations whose token
            views share this family-selection rule. Ordinary same-path machines
            remain ambiguous; exact path, package and source scope must agree even
            for declared family members. This selects a family representative, not
            a callable overload or provider.
        */
        pub fn find_build_subject_declaration_from_source(
            &self,
            path: &str,
            occurrence: SourceSpan,
            candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
            token_bound_machines: &[SymbolHandle],
        ) -> Option<SymbolHandle> {
            self.find_declaration_from_source(
                path,
                occurrence,
                candidates,
                true,
                token_bound_machines,
            )
        }

        fn find_declaration_from_source(
            &self,
            path: &str,
            occurrence: SourceSpan,
            candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
            admit_toolchain: bool,
            token_bound_machines: &[SymbolHandle],
        ) -> Option<SymbolHandle> {
            let occurrence = self.rejoin_product_occurrence(occurrence);
            if occurrence.span.start != occurrence.span.end
                && self
                    .sources
                    .as_deref()
                    .is_some_and(|sources| !sources.source_is_selected(occurrence.source_id))
            {
                return None;
            }
            let file = self.product_file_selection(path, occurrence);
            let qualified = path.split_once("::");
            let dependency =
                qualified.and_then(|(alias, _)| self.product_dependency_target(occurrence, alias));
            let declaration_path = match (qualified, dependency) {
                (Some((_, remaining)), Some(_)) => remaining,
                _ => path,
            };
            let mut selected = None;
            let mut selected_exact = false;
            let mut ambiguous = false;
            for (symbol, is_public) in candidates {
                if !symbol.is_valid() || !self.symbol_source_is_selected(symbol) {
                    continue;
                }
                let authorized = match (file, dependency) {
                    (Some(file), _) => file.matches(self, occurrence, symbol, is_public),
                    (None, Some(package)) => {
                        is_public && self.symbol_product_package_identity(symbol) == Some(package)
                    }
                    (None, None) => {
                        self.symbol_provenance_source_span(symbol)
                            .is_some_and(|declaration| {
                                self.same_product_package_instance(occurrence, declaration)
                                    || (admit_toolchain
                                        && is_public
                                        && self.source_file(declaration).is_some_and(|file| {
                                            file.origin == SourceOrigin::Toolchain
                                                && file.dependency_scope
                                                    == source_provenance::DependencyScope::Product
                                        }))
                            })
                            || !self.has_source_metadata()
                    }
                };
                if !authorized
                    || !self.source_reference_can_describe_product_symbol(occurrence, symbol)
                {
                    continue;
                }
                let exact = file.is_some() || self.display_path(symbol, "::") == declaration_path;
                if !exact && (dependency.is_some() || self.name(symbol) != declaration_path) {
                    continue;
                }
                if selected.is_none() || (exact && !selected_exact) {
                    selected = Some(symbol);
                    selected_exact = exact;
                    ambiguous = false;
                } else if exact == selected_exact && selected != Some(symbol) {
                    let same_operator_family = selected.is_some_and(|previous| {
                        ((self.get(previous).kind == SymbolKind::Operator
                            && self.get(symbol).kind == SymbolKind::Operator)
                            || (self.get(previous).kind == SymbolKind::Machine
                                && self.get(symbol).kind == SymbolKind::Machine
                                && token_bound_machines.contains(&previous)
                                && token_bound_machines.contains(&symbol)))
                            && self.display_path(previous, "::") == self.display_path(symbol, "::")
                            && self.symbol_package_identity(previous)
                                == self.symbol_package_identity(symbol)
                            && self.same_symbol_source_package(previous, symbol)
                    });
                    ambiguous |= !same_operator_family;
                }
            }
            if ambiguous { None } else { selected }
        }

        /*
            A designated Product bridge keeps the ordinary temporal/stratum policy
            while its caller separately admits exact Product package visibility.
        */
        pub fn source_reference_can_describe_product_symbol(
            &self,
            reference: SourceSpan,
            symbol: SymbolHandle,
        ) -> bool {
            match (&self.sources, self.symbol_provenance_source_span(symbol)) {
                (Some(sources), Some(declaration)) => {
                    sources.reference_can_see_stratum(reference, declaration)
                }
                _ => !self.has_source_metadata(),
            }
        }

        /*
            Compare declaration provenance at the lexical package boundary: the
            same package identity AND the same checked dependency scope. Two
            checked instances of one source are the same package nominally but
            share no lexical context — module namespaces, package-private
            visibility, and same-package selection never cross instances
            (wiki/spec/build/scoped_execution.md, "Two checked contexts").
            Source-free lowering is used by focused representation tests; those
            trees model one package and retain the historical all-local behavior.
        */
        pub fn same_source_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
            self.sources
                .as_deref()
                .is_none_or(|sources| sources.same_checked_instance(left, right))
        }

        /*
            Whether `declaration` is the product-scope checked instance inside
            `reference`'s package — the scope-crossing match for the bridges that
            deliberately name a product declaration from a build occurrence (root
            bindings, product entry/schema/provider queries). Ordinary resolution
            never uses this.
        */
        pub fn same_product_package_instance(
            &self,
            reference: SourceSpan,
            declaration: SourceSpan,
        ) -> bool {
            let Some(sources) = self.sources.as_deref() else {
                return true;
            };
            (sources.same_package(reference, declaration)
                || sources.describes_own_product(reference, declaration))
                && sources.file_at(declaration).is_none_or(|file| {
                    file.dependency_scope
                        == source_provenance::DependencyScope::Product
                })
        }

        /*
            Compare declaration provenance for authored or compiler-generated
            symbols. Generated symbols follow their mandatory earlier derivation
            origin; they never inherit a package through `SourceSpan::default()`.
            Source-free focused trees continue to model one package.
        */
        pub fn same_symbol_source_package(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
            let Some(sources) = self.sources.as_deref() else {
                return true;
            };
            let Some(left) = self.provenance_source_span(left) else {
                return false;
            };
            let Some(right) = self.provenance_source_span(right) else {
                return false;
            };
            sources.same_checked_instance(left, right)
        }

        // Installed core declarations and source-free builtin symbols are ambient.
        // Loading a closed contract cohort grants its siblings local scope only.
        pub(super) fn is_ambient_toolchain_symbol(&self, symbol: SymbolHandle) -> bool {
            match self.symbol_source_origin(symbol) {
                Some(SourceOrigin::Toolchain) => true,
                None => self.symbol_provenance_source_span(symbol).is_none(),
                Some(SourceOrigin::User | SourceOrigin::ToolchainContract | SourceOrigin::ToolchainLibrary) => false,
            }
        }

        /*
            Authored declaration provenance for one symbol. Compiler-generated
            symbols follow their mandatory derivation origin. Focused source-free
            trees return `None`; semantic consumers that admit them must make that
            fallback explicit rather than mistaking a spelling for ownership.
        */
        pub fn symbol_source_origin(&self, symbol: SymbolHandle) -> Option<SourceOrigin> {
            let sources = self.sources.as_deref()?;
            let source_span = self.provenance_source_span(symbol)?;
            sources.file_at(source_span).map(|file| file.origin)
        }

        /// Custody records the supplier of this declaration. The selected occurrence
        /// and its checked scope determine whether an invocation may use it.
        pub fn declaration_custody(
            &self,
            symbol: SymbolHandle,
        ) -> semantic_vocabulary::compiler_evaluation::DeclarationCustody {
            use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
            if !self.symbols().nodes().is_valid(symbol) {
                return DeclarationCustody::Missing;
            }
            if let Some(package) = self.symbol_package_identity(symbol) {
                return DeclarationCustody::Package(package);
            }
            match self.symbol_source_origin(symbol) {
                Some(SourceOrigin::Toolchain | SourceOrigin::ToolchainLibrary) => {
                    DeclarationCustody::Toolchain
                }
                Some(SourceOrigin::ToolchainContract) => DeclarationCustody::ToolchainContract,
                Some(SourceOrigin::User) => DeclarationCustody::UnownedUser,
                None if self.builtin_type_atom(symbol).is_some() => DeclarationCustody::Toolchain,
                None => DeclarationCustody::Missing,
            }
        }

        /*
            Reconciled package identity for one user-authored declaration.
            Generated symbols inherit their exact authored derivation origin.
            Source-free, toolchain, and unmanaged symbols return `None`.
        */
        pub fn symbol_package_identity(&self, symbol: SymbolHandle) -> Option<PackageKeyIdentity> {
            let source_span = self.provenance_source_span(symbol)?;
            let source_file = self.source_file(source_span)?;

            (source_file.origin == SourceOrigin::User)
                .then_some(source_file.package_identity)
                .flatten()
        }

        /*
            Reconciled package identity only for its product checked instance.
            A product dependency authorizes selection from that instance, not from
            an independently checked build copy of the same package/source bytes.
            Generated symbols retain the scope of their authored derivation origin.
        */
        pub fn symbol_product_package_identity(
            &self,
            symbol: SymbolHandle,
        ) -> Option<PackageKeyIdentity> {
            let source_span = self.provenance_source_span(symbol)?;
            let source_file = self.source_file(source_span)?;
            (source_file.dependency_scope
                == source_provenance::DependencyScope::Product)
                .then(|| self.symbol_package_identity(symbol))
                .flatten()
        }

        pub fn has_source_metadata(&self) -> bool {
            self.sources.is_some()
        }

        /*
            Exact frontend source custody retained beside authored symbol spans.
            This exposes source-map facts, not symbol identity or a stable package
            format; compiler-internal consumers remain responsible for canonical
            framing.
        */
        pub fn source_files(&self) -> impl Iterator<Item = &SourceFile> {
            self.sources
                .iter()
                .flat_map(|sources| sources.selected_files())
        }

        /*
            The authored symbol a generated symbol derives from, or `symbol`
            itself when it was authored. Specialization mints machine symbols whose
            names carry a selection fingerprint, and that spelling belongs to the
            compiler, not to a reader: a diagnostic about a specialized callee names
            the declaration its author wrote. The walk is the same derivation chain
            `provenance_source_span` follows, and a generated symbol always records
            one origin that precedes it, so it terminates.
        */
        pub fn authored_origin(&self, mut symbol: SymbolHandle) -> SymbolHandle {
            while symbol.is_valid() {
                let generated_from = self.get(symbol).generated_from;
                if !generated_from.is_valid()
                    || generated_from.arena_index() >= symbol.arena_index()
                {
                    return symbol;
                }
                symbol = generated_from;
            }
            symbol
        }

        pub(super) fn provenance_source_span(
            &self,
            mut symbol: SymbolHandle,
        ) -> Option<SourceSpan> {
            while symbol.is_valid() {
                let data = self.get(symbol);
                if let Some(source_span) = self.names.get(data.name).source_span() {
                    return Some(source_span);
                }
                let provenance_parent = if data.generated_from.is_valid() {
                    data.generated_from
                } else {
                    data.parent
                };
                if !provenance_parent.is_valid()
                    || provenance_parent.arena_index() >= symbol.arena_index()
                {
                    return None;
                }
                symbol = provenance_parent;
            }
            None
        }
    }
}
#[cfg(test)]
mod tests {
    /*
        Symbol-table unit tests, extracted from `table` so the storage and
        lookup machinery stays the file's headline.
    */

    use super::*;

    /*
        Module member lookups read each module's roster from the root name
        index. The roster must name exactly what a scan of every module symbol
        and root child names, in the same order, and must follow the
        mutations that clear that index.
    */
    #[cfg(test)]
    mod module_child_roster_tests {
        use super::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
        use crate::source_inputs::SourceMap;
        use source_admission::SourceOrigin;
        use source_provenance::PackageKeyIdentity;
        use source_text::{SourceId, SourceSpan, Span};
        use std::path::PathBuf;
        use std::sync::Arc;

        fn reference(source: usize) -> SourceSpan {
            SourceSpan::new(SourceId(source), Span::new(0, 4))
        }

        fn scanned_children(symbols: &SymbolTable, module: SymbolHandle) -> Vec<SymbolHandle> {
            symbols
                .module_symbols
                .iter()
                .map(|(_, handle)| *handle)
                .filter(|handle| symbols.get(*handle).parent == module)
                .chain(
                    symbols
                        .child_handles(symbols.root)
                        .into_iter()
                        .flatten()
                        .filter(|handle| symbols.symbol_module(*handle) == module),
                )
                .filter(|symbol| symbols.symbol_source_is_selected(*symbol))
                .collect()
        }

        fn assert_rosters_match(symbols: &SymbolTable) -> usize {
            let modules = symbols
                .module_symbols
                .iter()
                .map(|(_, handle)| *handle)
                .collect::<Vec<_>>();
            let mut members = 0;
            for module in modules.iter().copied().chain([symbols.root]) {
                let indexed = symbols.module_children(module);
                assert_eq!(indexed, scanned_children(symbols, module));
                members += indexed.len();
            }
            members
        }

        #[test]
        fn module_rosters_match_a_full_scan_through_index_clearing_mutations() {
            let mut sources = SourceMap::default();
            for source in 0..4 {
                let root = PathBuf::from("package");
                sources.add_with_metadata(
                    root.join(format!("source-{source}.omg")),
                    String::from("Item"),
                    root,
                    Some(PackageKeyIdentity::from_digest([1; 32]).expect("nonzero owner")),
                    SourceOrigin::User,
                );
            }
            let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
                root,
                (0..4).map(|source| {
                    (
                        SymbolKind::Data,
                        SymbolNameRef::OwnedSource {
                            value: "Item",
                            source_span: reference(source),
                        },
                    )
                }),
            ))
            .collect::<Vec<_>>();
            let mut symbols = builder.finish();
            for (source, path) in [
                (0, &["combat", "ai"][..]),
                (1, &["combat"][..]),
                (2, &["combat", "ai"][..]),
            ] {
                symbols
                    .register_source_module(
                        SourceId(source),
                        path.iter().map(|name| (*name, reference(source))),
                    )
                    .expect("one namespace per source");
                assert_rosters_match(&symbols);
            }
            let combat = symbols.source_module(SourceId(1));
            let ai = symbols.source_module(SourceId(0));
            assert!(combat.is_valid() && ai.is_valid() && combat != ai);
            assert_eq!(symbols.source_module(SourceId(2)), ai);
            let combat_members = symbols.module_children(combat);
            assert_eq!(combat_members.first(), Some(&ai));
            assert!(combat_members.contains(&declarations[1]));
            let ai_members = symbols.module_children(ai);
            assert!(ai_members.contains(&declarations[0]) && ai_members.contains(&declarations[2]));
            assert!(!ai_members.contains(&declarations[3]));
            // A generated symbol is its own root, not a root child, so no module
            // lists it; generating one and renaming a member both clear the index.
            let generated =
                symbols.insert_generated_root_from(declarations[2], SymbolKind::Machine, "made");
            assert_rosters_match(&symbols);
            assert!(!symbols.module_children(ai).contains(&generated));
            symbols.rename(declarations[0], "Renamed");
            let members = assert_rosters_match(&symbols);
            assert!(members >= 4, "only {members} module members");
            symbols
                .register_source_module(SourceId(3), [("combat", reference(3))])
                .expect("a fourth file joins the outer module");
            assert_eq!(symbols.source_module(SourceId(3)), combat);
            assert!(symbols.module_children(combat).contains(&declarations[3]));
            assert_rosters_match(&symbols);
        }
    }

    #[cfg(test)]
    mod builtin_function_identity_tests {
        use super::{BuiltinFunction, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
        use crate::source_symbols::{builtin_function_symbols, builtin_type_symbols};

        fn builtin_symbol_table(
            function_symbols: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'static>)>,
        ) -> SymbolTable {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            builder.insert_children(
                root,
                builtin_type_symbols().into_iter().chain(function_symbols),
            );
            builder.finish()
        }

        #[test]
        fn every_installed_builtin_function_round_trips_by_exact_slot() {
            let symbols = builtin_symbol_table(builtin_function_symbols());

            for (function, _) in BuiltinFunction::ALL {
                let symbol = symbols
                    .builtin_function_symbol(function)
                    .expect("compiler builtin function must occupy its fixed root slot");
                assert_eq!(symbols.builtin_function_for_symbol(symbol), Some(function));
            }
        }

        #[test]
        fn a_fixed_core_declaration_is_its_installed_slot_and_never_a_same_named_one() {
            let authored = (
                SymbolKind::MathematicalDefinition,
                SymbolNameRef::Static("core::Strict"),
            );
            let symbols = builtin_symbol_table(
                builtin_function_symbols()
                    .into_iter()
                    .chain(crate::source_symbols::fixed_core_declaration_symbols())
                    .chain([authored]),
            );
            for declaration in crate::source_symbols::FixedCoreDeclaration::ALL {
                let symbol = symbols
                    .fixed_core_declaration_symbol(declaration)
                    .expect("fixed core declaration must occupy its root slot");
                assert_eq!(
                    symbols.fixed_core_declaration_for_symbol(symbol),
                    Some(declaration)
                );
            }
            let same_named = symbols
                .child_handles(symbols.root())
                .expect("root children")
                .last()
                .expect("authored declaration");
            assert_eq!(symbols.name(same_named), "core::Strict");
            assert_eq!(symbols.fixed_core_declaration_for_symbol(same_named), None);
            let function = symbols
                .builtin_function_symbol(BuiltinFunction::ALL[0].0)
                .expect("builtin function");
            assert_eq!(symbols.fixed_core_declaration_for_symbol(function), None);
        }
    }

    #[cfg(test)]
    mod source_name_storage_tests {
        use super::{Arc, SourceMap, SourceSpan, SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use source_text::Span;

        #[test]
        fn builder_and_extension_names_borrow_retained_source_bytes() {
            let mut sources = SourceMap::default();
            let source_id = sources.add("main.omg".into(), "Ready".into()).source_id;
            let source_span = SourceSpan::new(source_id, Span::new(0, 5));
            let sources = Arc::new(sources);
            let expected_pointer = sources.text_at(source_span).as_ptr();
            let seed = SymbolNameRef::OwnedSource {
                value: "Ready",
                source_span,
            };
            let mut builder = SymbolTableBuilder::with_sources(Some(sources.clone()));
            let root = builder.insert_root(SymbolKind::Root, seed);
            let child = SymbolTableBuilder::child_handles(
                builder.insert_children(root, [(SymbolKind::Machine, seed)]),
            )
            .next()
            .expect("authored child");
            let mut extension = builder.finish().begin_extension(Some(sources), Vec::new());
            let additional = extension.insert_top_level([(SymbolKind::Machine, seed)])[0];
            let additional_child = SymbolTableBuilder::child_handles(
                extension.insert_children(additional, [(SymbolKind::State, seed)]),
            )
            .next()
            .expect("extension child");
            let table = extension
                .finish()
                .begin_extension(None, Vec::new())
                .finish();
            for symbol in [root, child, additional, additional_child] {
                assert_eq!(table.name(symbol), "Ready");
                assert_eq!(table.name(symbol).as_ptr(), expected_pointer);
                assert_eq!(table.symbol_source_span(symbol), Some(source_span));
            }
        }
    }
}
use super::{
    BuiltinFunction, BuiltinType, BuiltinTypeAtom, BuiltinTypeMember, FixedCoreDeclaration, Symbol,
    SymbolHandle, SymbolKind, SymbolName, SymbolNameRef,
};
use child_batches::{SupplementalChildHandles, SupplementalChildren};

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolTable {
    symbols: SymbolHierarchy,
    names: SymbolRows<SymbolName>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Arc<Vec<SourceScopedTopLevelBinding>>,
    root: SymbolHandle,
    supplemental_children: SupplementalChildren,
    module_symbols: SymbolRows<SymbolHandle>,
    /*
        Dense `SourceId`-indexed map from a source to its declared module:
        a linear scan per lookup would cost `O(#sources)` on every
        module-qualified name walk.
    */
    source_module_index: Arc<Vec<SymbolHandle>>,
    /*
        Declaration membership at the last activation frontier. Later allocations
        belong to the new cohort. Selecting a source again does not resurrect
        an older declaration that already left its predecessor's view.
    */
    inherited_symbols: Arc<Vec<bool>>,
    root_names: RootNameIndexCache,
    /*
        `source_scoped_top_level_bindings` bucketed by reference source,
        because three lookup paths scanned the whole binding list per query.
        Cleared wherever bindings are appended after `finish`.
    */
    binding_source_index: BindingSourceIndexCache,
    /*
        `display_path("::")` snapshot for the top-level and module roster,
        because module-qualified lookups walked every candidate's parent
        chain -- allocating a `String` per candidate per reference. Cleared
        alongside the other roster indexes.
    */
    module_path_index: ModulePathIndexCache,
    /*
        The top-level + module roster bucketed by full name, display path,
        and (for Domain kinds) declared leaf, so `find_module_qualified_reference`
        consults only name- or path-bearing candidates instead of walking the
        roster per reference. Cleared alongside `module_path_index`.
    */
    module_scope_index: ModuleScopeIndexCache,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTableBuilder {
    symbols: SymbolHierarchy,
    names: SymbolRows<SymbolName>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
}

/*
    One compiler-owned top-level vocabulary binding for an exact source.

    The authored spelling remains unchanged. Only references originating in
    `reference_source` select the same-spelled declaration authored in
    `declaration_source`; every other source retains ordinary lookup.
*/
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceScopedTopLevelBinding {
    reference_source: SourceId,
    declaration_source: SourceId,
    target: BindingTarget,
    producer: BindingProducer,
    dependency: Option<(Arc<str>, PackageKeyIdentity)>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
enum BindingTarget {
    Declaration(Arc<str>),
    PreludeModule(Arc<str>),
    FileAlias(Arc<str>),
    ProductFileAlias(Arc<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
enum BindingProducer {
    Loader,
    AuthoredFile {
        root: Option<Arc<str>>,
        path: Arc<str>,
    },
}

impl SourceScopedTopLevelBinding {
    pub fn remap_sources(&mut self, remap: impl Fn(SourceId) -> SourceId) {
        self.reference_source = remap(self.reference_source);
        self.declaration_source = remap(self.declaration_source);
    }

    pub fn new(
        reference_source: SourceId,
        declaration_source: SourceId,
        name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            reference_source,
            declaration_source,
            target: BindingTarget::Declaration(name.into()),
            producer: BindingProducer::Loader,
            dependency: None,
        }
    }

    /// A file alias selects declarations from exactly one admitted source.
    pub fn file_import(
        reference_source: SourceId,
        declaration_source: SourceId,
        alias: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            reference_source,
            declaration_source,
            target: BindingTarget::FileAlias(alias.into()),
            producer: BindingProducer::Loader,
            dependency: None,
        }
    }

    /// Target of a designated product operand in Build code.
    pub fn product_file_import(
        reference_source: SourceId,
        declaration_source: SourceId,
        alias: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            reference_source,
            declaration_source,
            target: BindingTarget::ProductFileAlias(alias.into()),
            producer: BindingProducer::Loader,
            dependency: None,
        }
    }

    /// The local file alias can name a different, requester-local package edge.
    /// Only the loader that actually selected that package supplies this evidence.
    pub fn from_dependency(mut self, selected: Option<(&str, PackageKeyIdentity)>) -> Self {
        self.dependency = selected.map(|(alias, package)| (alias.into(), package));
        self
    }

    fn prelude_namespace(&self) -> Option<&str> {
        match &self.target {
            BindingTarget::PreludeModule(import) => Some(import),
            _ => None,
        }
    }

    fn binds_declaration(&self, name: &str) -> bool {
        matches!(&self.target, BindingTarget::Declaration(bound) if bound.as_ref() == name)
    }

    /* The source whose top-level references this binding scopes. */
    pub const fn reference_source(&self) -> SourceId {
        self.reference_source
    }
}

/// A file-qualified declaration reference at a designated product operand.
#[derive(Clone, Copy)]
pub struct ImportedProductMember<'a> {
    source: Option<SourceId>,
    member: &'a str,
}

impl ImportedProductMember<'_> {
    pub fn matches(
        self,
        symbols: &SymbolTable,
        reference: SourceSpan,
        symbol: SymbolHandle,
        is_public: bool,
    ) -> bool {
        let Some(declaration) = symbols.symbol_provenance_source_span(symbol) else {
            return false;
        };
        let Some(file) = symbols.source_file(declaration) else {
            return false;
        };
        if self.source != Some(declaration.source_id)
            || file.dependency_scope != source_provenance::DependencyScope::Product
            || !symbols.symbol_source_is_selected(symbol)
            || !symbols.source_reference_can_describe_product_symbol(reference, symbol)
        {
            return false;
        }
        let visible = symbols.same_product_package_instance(reference, declaration)
            || (is_public
                && (file.origin.is_toolchain()
                    || file.package_identity.is_some_and(|owner| {
                        symbols
                            .sources
                            .as_deref()
                            .and_then(|sources| sources.product_dependencies_at(reference))
                            .is_some_and(|dependencies| {
                                dependencies.values().any(|target| *target == owner)
                            })
                    })));
        visible
            && symbols.file_member_path(symbol, &symbols.display_path(symbol, "::")) == self.member
    }
}

/*
    Outcome after declaration eligibility and source lookup precedence.
    Ambiguity must remain distinct from absence when composing static names.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLookup {
    NotFound,
    Unique(SymbolHandle),
    Ambiguous {
        first: SymbolHandle,
        second: SymbolHandle,
    },
}

/// Scoped lookup evidence retained by a source occurrence that has no selected
/// declaration. Absence is not reconstructed from other declarations' spellings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
pub enum UnresolvedName {
    /// No lookup outcome was established, or selection needs a later judgment.
    #[default]
    Unserved,
    NoVisibleDeclaration,
    Ambiguous,
}

impl From<SymbolLookup> for UnresolvedName {
    fn from(lookup: SymbolLookup) -> Self {
        match lookup {
            SymbolLookup::NotFound => Self::NoVisibleDeclaration,
            SymbolLookup::Ambiguous { .. } => Self::Ambiguous,
            SymbolLookup::Unique(_) => Self::Unserved,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopLevelSelection {
    Ordinary,
    SignatureFree,
}

impl SymbolLookup {
    pub fn unique(self) -> Option<SymbolHandle> {
        match self {
            Self::Unique(symbol) => Some(symbol),
            Self::NotFound | Self::Ambiguous { .. } => None,
        }
    }

    fn from_candidates(mut candidates: impl Iterator<Item = SymbolHandle>) -> Self {
        let Some(first) = candidates.next() else {
            return Self::NotFound;
        };
        if let Some(second) = candidates.find(|candidate| *candidate != first) {
            Self::Ambiguous { first, second }
        } else {
            Self::Unique(first)
        }
    }
}

impl SymbolTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sources(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            sources,
            ..Self::default()
        }
    }

    pub fn with_sources_and_top_level_bindings(
        sources: Option<Arc<SourceMap>>,
        source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Self {
        Self {
            sources,
            source_scoped_top_level_bindings,
            ..Self::default()
        }
    }

    pub fn insert_root(&mut self, kind: SymbolKind, name: SymbolNameRef<'_>) -> SymbolHandle {
        let symbol = self.symbol_from_parts(SymbolHandle::invalid(), kind, name);
        let root = self.symbols.insert_root(symbol);
        self.root = root;

        root
    }

    pub fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        let children = children
            .into_iter()
            .map(|(kind, name)| self.symbol_from_parts(parent, kind, name))
            .collect::<Vec<_>>();
        self.symbols.insert_children(parent, children)
    }

    pub fn child_handles(span: HandleSpan<Symbol>) -> impl Iterator<Item = SymbolHandle> {
        span.handles()
    }

    pub fn finish(self) -> SymbolTable {
        SymbolTable {
            symbols: self.symbols,
            names: self.names,
            sources: self.sources,
            source_scoped_top_level_bindings: Arc::new(self.source_scoped_top_level_bindings),
            root: self.root,
            supplemental_children: SupplementalChildren::default(),
            module_symbols: SymbolRows::new(),
            source_module_index: Arc::default(),
            inherited_symbols: Arc::default(),
            root_names: RootNameIndexCache::default(),
            binding_source_index: BindingSourceIndexCache::default(),
            module_path_index: ModulePathIndexCache::default(),
            module_scope_index: ModuleScopeIndexCache::default(),
        }
    }

    fn symbol_from_parts(
        &mut self,
        parent: SymbolHandle,
        kind: SymbolKind,
        name: SymbolNameRef<'_>,
    ) -> Symbol {
        child_symbol(&mut self.names, self.sources.as_deref(), parent, kind, name)
    }
}

fn child_symbol(
    names: &mut SymbolRows<SymbolName>,
    sources: Option<&SourceMap>,
    parent: SymbolHandle,
    kind: SymbolKind,
    name: SymbolNameRef<'_>,
) -> Symbol {
    Symbol {
        parent,
        declaration_owner: SymbolHandle::invalid(),
        children: HandleSpan::empty(),
        kind,
        name: names.insert(name_from_source(name, sources)),
        generated_from: SymbolHandle::invalid(),
    }
}

/*
    Common append surface used while first constructing a symbol hierarchy and
    while extending one retained hierarchy. Both routes preserve contiguous
    child ranges; only top-level insertion differs.
*/
pub trait SymbolTableAppender {
    /* Source custody used while inserting explicit declaration namespaces. */
    fn source_map(&self) -> Option<Arc<SourceMap>>;

    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol>;
}

impl SymbolTableAppender for SymbolTableBuilder {
    fn source_map(&self) -> Option<Arc<SourceMap>> {
        self.sources.clone()
    }

    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        SymbolTableBuilder::insert_children(self, parent, children)
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /*
        Whether this table retains `base` exactly and appends only generated
        declarations with earlier derivation origins. A later frontend may
        adopt this allocation frontier before minting authored body symbols;
        this does not turn generated declarations into authored source roots.
    */
    pub fn is_generated_extension_of(&self, base: &Self) -> bool {
        let Self {
            symbols,
            names,
            sources,
            source_scoped_top_level_bindings,
            root,
            supplemental_children,
            module_symbols,
            source_module_index,
            inherited_symbols,
            root_names: _,
            binding_source_index: _,
            module_path_index: _,
            module_scope_index: _,
        } = self;
        if symbols.len() < base.symbols.len()
            || !symbols
                .nodes()
                .iter()
                .take(base.symbols.len())
                .eq(base.symbols.nodes().iter())
            || names.len() < base.names.len()
            || !names.iter().take(base.names.len()).eq(base.names.iter())
            || sources != &base.sources
            || source_scoped_top_level_bindings != &base.source_scoped_top_level_bindings
            || root != &base.root
            || module_symbols != &base.module_symbols
            || source_module_index != &base.source_module_index
            || inherited_symbols != &base.inherited_symbols
        {
            return false;
        }
        for (parent, _) in symbols.nodes().iter() {
            let mut children = supplemental_children.children(parent);
            if base
                .supplemental_children
                .children(parent)
                .any(|retained| children.next() != Some(retained))
                || children.any(|child| {
                    child.arena_index() as usize <= base.symbols.len()
                        || symbols.get(child).parent != parent
                })
            {
                return false;
            }
        }
        for (handle, symbol) in symbols.nodes().iter().skip(base.symbols.len()) {
            let origin = symbol.generated_from;
            if !origin.is_valid()
                || symbol.declaration_owner.is_valid()
                || origin.generation() != handle.generation()
                || origin.arena_index() >= handle.arena_index()
                || symbols.get(origin).kind == SymbolKind::Unknown
                || symbol.kind == SymbolKind::Unknown
                || !names.is_valid(symbol.name)
                || names.get(symbol.name).source_span().is_some()
            {
                return false;
            }
            if symbol.parent.is_valid() {
                if symbol.parent.generation() != handle.generation()
                    || symbol.parent.arena_index() >= handle.arena_index()
                {
                    return false;
                }
                let siblings = symbols.get(symbol.parent).children;
                let in_primary = siblings.start().generation() == handle.generation()
                    && handle
                        .arena_index()
                        .checked_sub(siblings.start().arena_index())
                        .is_some_and(|offset| offset < siblings.count());
                let supplemental_count = supplemental_children
                    .children(symbol.parent)
                    .filter(|&child| child == handle)
                    .count();
                if usize::from(in_primary) + supplemental_count != 1 {
                    return false;
                }
            }
            let Some(mut children) = self.child_handles(handle) else {
                return false;
            };
            if children.any(|child| {
                child.arena_index() <= handle.arena_index() || symbols.get(child).parent != handle
            }) {
                return false;
            }
        }
        true
    }

    /*
        Append an authored child batch without replacing the parent's original
        children or rebinding any existing handle. Lookup visits the original
        declaration children first, then appended batches in insertion order.
        This is also the route for body locals and synthesized state headers.
    */
    pub fn append_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        self.append_children_from(
            parent,
            children
                .into_iter()
                .map(|(kind, name)| (SymbolHandle::invalid(), kind, name)),
        )
    }

    /*
        Append generated locals to an existing lexical scope while preserving
        both its frozen child span and each new child's derivation origin.
    */
    pub fn append_generated_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        let children = children.into_iter().collect::<Vec<_>>();
        assert!(
            children.iter().all(|(origin, _, _)| origin.is_valid()
                && self.symbols.get(*origin).kind != SymbolKind::Unknown),
            "compiler-generated symbols require existing derivation origins"
        );
        self.append_children_from(
            parent,
            children
                .into_iter()
                .map(|(origin, kind, name)| (origin, kind, SymbolNameRef::Borrowed(name))),
        )
    }

    fn append_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        let _ = self.get(parent);
        let mut appended = HandleSpan::empty();
        for (generated_from, kind, name) in children {
            let name = self
                .names
                .insert(name_from_source(name, self.sources.as_deref()));
            let child = self.symbols.insert_supplemental_child(
                parent,
                Symbol {
                    parent,
                    declaration_owner: SymbolHandle::invalid(),
                    children: HandleSpan::empty(),
                    kind,
                    name,
                    generated_from,
                },
            );
            appended.push_contiguous(child);
        }
        if appended.is_empty() {
            return appended;
        }
        self.supplemental_children.append(parent, appended);
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        appended
    }

    #[inline]
    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    /*
        Record the inverse of an owning declaration row. Resolution establishes
        uniqueness before publication; a retained frontier may rejoin the same
        relationship, but another owner cannot replace it.
    */
    pub fn record_declaration_owner(
        &mut self,
        declaration: SymbolHandle,
        owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        if !self.symbols.nodes().is_valid(declaration)
            || !self.symbols.nodes().is_valid(owner)
            || self.get(declaration).kind != SymbolKind::Machine
            || self.get(owner).kind != SymbolKind::Conformance
        {
            return Err("owned realization lost its declaration or enclosing capture scope");
        }
        let previous = self.get(declaration).declaration_owner;
        if previous.is_valid() && previous != owner {
            return Err("owned conformance realization has ambiguous enclosing capture scope");
        }
        self.symbols.get_mut(declaration).declaration_owner = owner;
        Ok(())
    }

    /* A discarded synthesized declaration cannot lend its old capture scope. */
    pub fn clear_declaration_owner(&mut self, declaration: SymbolHandle) {
        self.symbols.get_mut(declaration).declaration_owner = SymbolHandle::invalid();
    }

    /*
        Give one symbol a new spelling. Target-family selection promotes the
        realized target's body to the family path and demotes the body it
        replaces to its `<path>::<target>` spelling; handles are unchanged.
        The declaration's source span stays attached to the new spelling, so
        provenance and diagnostics are unchanged.
    */
    pub fn rename(&mut self, symbol: SymbolHandle, name: &str) {
        let name = match self.names.get(self.get(symbol).name).source_span() {
            Some(source_span) => {
                self.names
                    .insert(SymbolName::from_ref(SymbolNameRef::OwnedSource {
                        value: name,
                        source_span,
                    }))
            }
            None => self
                .names
                .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
        };
        self.symbols.get_mut(symbol).name = name;
        self.binding_source_index.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        self.root_names.clear();
    }

    /*
        Start an append-only extension of an already resolved symbol table.
        Existing symbol handles and child ranges remain unchanged.
    */
    pub fn begin_extension(
        mut self,
        sources: Option<Arc<SourceMap>>,
        additional_source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> SymbolTableExtension {
        self.sources = sources;
        Arc::make_mut(&mut self.source_scoped_top_level_bindings)
            .extend(additional_source_scoped_top_level_bindings);
        self.binding_source_index.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        SymbolTableExtension { table: self }
    }

    /*
        Start a new compilation view over retained declaration coordinates.
        Unlike an extension within one activation, its complete loader-supplied
        loader import roster replaces historical loader bindings, even when both
        endpoints remain selected for unrelated imports. Selected declarations
        retain their own authored imports; backing is not loader authority.
    */
    pub fn begin_source_activation(
        mut self,
        sources: Arc<SourceMap>,
        source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Result<SymbolTableExtension, String> {
        if self
            .sources
            .as_deref()
            .is_some_and(|previous| !sources.retains_source_storage(previous))
        {
            return Err(String::from(
                "source activation changed retained source coordinates",
            ));
        }
        let mut inherited = vec![false; self.symbols.len() + 1];
        for (handle, _) in self.symbols.nodes().iter() {
            inherited[handle.arena_index() as usize] = self.symbol_source_is_selected(handle)
                && self
                    .provenance_source_span(handle)
                    .is_none_or(|span| sources.source_is_selected(span.source_id));
        }
        if let Some(previous) = self.sources.as_deref() {
            for (position, module) in Arc::make_mut(&mut self.source_module_index)
                .iter_mut()
                .enumerate()
            {
                let source = SourceId(position);
                if sources.source_is_selected(source) && !previous.source_is_selected(source) {
                    *module = SymbolHandle::invalid();
                }
            }
        }
        let authored_imports = self
            .source_scoped_top_level_bindings
            .iter()
            .filter(|binding| {
                matches!(binding.producer, BindingProducer::AuthoredFile { .. })
                    && sources.source_is_selected(binding.reference_source)
            })
            .cloned()
            .collect::<Vec<_>>();
        self.inherited_symbols = Arc::new(inherited);
        self.sources = Some(sources);
        self.source_scoped_top_level_bindings = Arc::new(source_scoped_top_level_bindings);
        self.binding_source_index.clear();
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        for binding in authored_imports {
            if let (BindingProducer::AuthoredFile { root, path }, BindingTarget::FileAlias(alias)) =
                (&binding.producer, &binding.target)
            {
                self.register_source_file_import(
                    binding.reference_source,
                    root.as_deref(),
                    path,
                    alias,
                );
            }
        }
        Ok(SymbolTableExtension { table: self })
    }

    /*
        Mint a compiler-generated root declaration after symbol resolution.
        The generated name is owned by the table and authored symbol handles
        remain unchanged. Children must be installed once, as one batch, via
        [`Self::insert_generated_children`].
    */
    pub fn insert_generated_root_from(
        &mut self,
        generated_from: SymbolHandle,
        kind: SymbolKind,
        name: &str,
    ) -> SymbolHandle {
        assert!(
            generated_from.is_valid()
                && self.symbols.get(generated_from).kind != SymbolKind::Unknown,
            "compiler-generated symbols require one existing derivation origin"
        );
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        let name = self
            .names
            .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name)));
        self.symbols.insert_root(Symbol {
            parent: SymbolHandle::invalid(),
            declaration_owner: SymbolHandle::invalid(),
            children: HandleSpan::empty(),
            kind,
            name,
            generated_from,
        })
    }

    /*
        Mint the complete child range of a freshly generated symbol. Keeping
        the batch contiguous preserves the hierarchy arena's compact child-span
        representation without allowing late mutation of authored parents.
    */
    pub fn insert_generated_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        self.insert_generated_children_from(
            parent,
            children
                .into_iter()
                .map(|(kind, name)| (parent, kind, name)),
        )
    }

    /*
        Mint one contiguous child range while retaining each child's exact
        existing derivation origin. Lexical parentage still belongs to `parent`;
        specialization provenance belongs to the corresponding source child.
    */
    pub fn insert_generated_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        let children = children.into_iter().collect::<Vec<_>>();
        assert!(
            children.iter().all(|(origin, _, _)| origin.is_valid()
                && self.symbols.get(*origin).kind != SymbolKind::Unknown),
            "compiler-generated symbols require existing derivation origins"
        );
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        let names = &mut self.names;
        self.symbols.insert_children(
            parent,
            children
                .into_iter()
                .map(|(generated_from, kind, name)| Symbol {
                    parent,
                    declaration_owner: SymbolHandle::invalid(),
                    children: HandleSpan::empty(),
                    kind,
                    name: names.insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
                    generated_from,
                }),
        )
    }

    #[inline]
    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        {
            let name = self.names.get(symbol.name);
            name.spelling().unwrap_or_else(|| {
                name.source_span()
                    .and_then(|span| self.sources.as_deref().map(|sources| sources.text_at(span)))
                    .unwrap_or("")
            })
        }
    }

    #[inline]
    pub fn root(&self) -> SymbolHandle {
        self.root
    }

    #[inline]
    pub fn child_handles(&self, parent: SymbolHandle) -> Option<SymbolChildHandles<'_>> {
        if self.get(parent).kind == SymbolKind::Module {
            return Some(SymbolChildHandles {
                contiguous: None,
                supplemental: self.supplemental_children.children(parent),
                namespace: self.module_children(parent).into_iter(),
            });
        }
        Some(SymbolChildHandles {
            contiguous: Some(self.symbols.child_handles(parent)?),
            supplemental: self.supplemental_children.children(parent),
            namespace: Vec::new().into_iter(),
        })
    }

    /*
        Root-level children of kind `Domain`, in the order `child_handles(root())`
        yields them. Domain selection asks which families an authored spelling
        reaches; reading this roster skips every declaration that cannot be one.
        Like `child_handles`, it applies no source-activation filter.
    */
    pub fn root_domain_children(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.root_name_rows().domains.iter().copied()
    }

    /*
        Root-level children whose full name is exactly `name`, in the order
        `child_handles(root())` yields them, probed through the root name
        index rather than a scan of the whole root roster.
    */
    pub fn root_children_named(&self, name: &str) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.root_name_rows()
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|&(_, symbol)| symbol)
            .filter(|symbol| self.symbol_source_is_selected(*symbol))
    }

    /// The complete nearest lexical declaration cohort, without crossing into
    /// root/module lookup. Locals and invocation arguments are not declarations
    /// in this namespace. An ambiguous inner cohort shadows every outer one.
    pub fn lexical_declaration_candidates(
        &self,
        scope: SymbolHandle,
        name: &str,
        kinds: &[SymbolKind],
    ) -> Vec<SymbolHandle> {
        let mut candidates = Vec::new();
        self.visit_lexical_declarations(scope, name, kinds, |symbol| candidates.push(symbol));
        candidates
    }

    /// A unique/ambiguous answer uses the same cohort without allocating its
    /// complete roster for ordinary single-declaration binding.
    pub fn lexical_declaration_lookup(
        &self,
        scope: SymbolHandle,
        name: &str,
        kinds: &[SymbolKind],
    ) -> SymbolLookup {
        let mut answer = SymbolLookup::NotFound;
        self.visit_lexical_declarations(scope, name, kinds, |symbol| {
            answer = match answer {
                SymbolLookup::NotFound => SymbolLookup::Unique(symbol),
                SymbolLookup::Unique(first) => SymbolLookup::Ambiguous {
                    first,
                    second: symbol,
                },
                ambiguous => ambiguous,
            };
        });
        answer
    }

    fn visit_lexical_declarations(
        &self,
        mut scope: SymbolHandle,
        name: &str,
        kinds: &[SymbolKind],
        mut visit: impl FnMut(SymbolHandle),
    ) {
        for _ in 0..self.symbols.nodes().len() {
            if !self.symbols.nodes().is_valid(scope) || self.get(scope).kind != SymbolKind::Machine
            {
                return;
            }
            let mut found = false;
            for child in self.child_handles(scope).into_iter().flatten() {
                if kinds.contains(&self.get(child).kind)
                    && self.name(child) == name
                    && self.symbol_source_is_selected(child)
                {
                    found = true;
                    visit(child);
                }
            }
            if found {
                return;
            }
            scope = self.get(scope).parent;
        }
    }

    pub fn find_child_by_name(&self, parent: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        let mut candidates = self
            .child_handles(parent)?
            .filter(|symbol| self.name(*symbol) == name && self.symbol_source_is_selected(*symbol));
        let first = candidates.next()?;
        if self.get(parent).kind == SymbolKind::Module && candidates.next().is_some() {
            return None;
        }
        Some(first)
    }

    pub fn find_child_by_name_and_kind(
        &self,
        parent: SymbolHandle,
        name: &str,
        kind: SymbolKind,
    ) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        let mut candidates = self.child_handles(parent)?.filter(|symbol| {
            self.get(*symbol).kind == kind
                && self.name(*symbol) == name
                && self.symbol_source_is_selected(*symbol)
        });
        let first = candidates.next()?;
        if self.get(parent).kind == SymbolKind::Module && candidates.next().is_some() {
            return None;
        }
        Some(first)
    }

    /*
        Resolve one source-backed top-level reference without turning a
        presentation spelling into global authority.
    */
    pub fn lookup_source_type(&self, name: &str, reference: SourceSpan) -> SymbolLookup {
        let builtin = crate::source_symbols::is_builtin_type_spelling(name);
        let kinds: &[SymbolKind] = if builtin {
            &[SymbolKind::BuiltinType]
        } else {
            &[
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::TypeAlias,
                SymbolKind::Machine,
                SymbolKind::Trait,
                SymbolKind::MathematicalDefinition,
            ]
        };
        let lookup = self.lookup_top_level_by_name_and_kinds_from_source_matching(
            name,
            kinds,
            reference,
            |symbol| {
                self.get(symbol).kind != SymbolKind::MathematicalDefinition
                    || self.fixed_core_declaration_for_symbol(symbol).is_some()
            },
        );
        match lookup {
            SymbolLookup::NotFound if !builtin => self
                .lookup_top_level_by_name_and_kinds_from_source_matching(
                    name,
                    &[SymbolKind::Conformance],
                    reference,
                    |_| true,
                ),
            other => other,
        }
    }

    pub fn find_top_level_by_name_and_kinds_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Option<SymbolHandle> {
        self.find_top_level_by_name_and_kinds_from_source_matching(name, kinds, reference, |_| true)
    }

    /*
        Apply declaration eligibility before source-scoped binding and lookup
        precedence. In particular, a receiver's nominal owner can exclude a
        same-spelled method without hiding another eligible declaration.
    */
    pub fn find_top_level_by_name_and_kinds_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> Option<SymbolHandle> {
        self.lookup_top_level_by_name_and_kinds_from_source_matching(
            name,
            kinds,
            reference,
            matches_candidate,
        )
        .unique()
    }

    /*
        Retain ambiguity when a caller composes multiple declaration meanings.
        Eligibility still precedes the ordinary source and namespace precedence.
    */
    pub fn lookup_top_level_by_name_and_kinds_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> SymbolLookup {
        self.lookup_top_level(
            name,
            kinds,
            reference,
            matches_candidate,
            TopLevelSelection::Ordinary,
        )
    }

    /*
        Apply ordinary namespace selection without choosing among declarations
        that require a call signature to distinguish. Flat source forests retain
        the occurrence's package scope, and synthetic references retain the whole
        eligible pool; neither may silently select its first declaration.
    */
    pub fn lookup_signature_free_top_level_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> SymbolLookup {
        self.lookup_top_level(
            name,
            kinds,
            reference,
            matches_candidate,
            TopLevelSelection::SignatureFree,
        )
    }

    /*
        Root-level children whose full name or leaf name selects `name`,
        in declaration order. The whole-roster filter behind every top-level
        lookup is O(children) per call; bucketing once per table keeps the
        same candidate pool without rescanning.
    */
    fn top_level_children_matching_name(
        &self,
        name: &str,
        include_leaf: bool,
    ) -> Vec<SymbolHandle> {
        let index = self.root_name_rows();
        let mut merged = index.by_name.get(name).cloned().unwrap_or_default();
        if include_leaf && let Some(leaves) = index.by_leaf.get(name) {
            merged.extend_from_slice(leaves);
        }
        merged.sort_unstable_by_key(|(position, _)| *position);
        merged.dedup_by_key(|(_, symbol)| *symbol);
        merged.into_iter().map(|(_, symbol)| symbol).collect()
    }

    /*
        The bindings visible to `reference_source`, in binding order, via the
        per-source index instead of a whole-list scan per lookup. Activation
        is projected once here: lookup and collision consumers cannot recover
        authority from a historical row by forgetting their own filter.
    */
    pub(super) fn source_scoped_bindings_for(
        &self,
        reference_source: SourceId,
    ) -> &[SourceScopedTopLevelBinding] {
        const EMPTY: &[SourceScopedTopLevelBinding] = &[];
        self.source_binding_index()
            .by_source
            .get(&reference_source)
            .map_or(EMPTY, |scope| scope.bindings.as_slice())
    }

    /* The root children bucketed by name, built on the first name lookup. */
    fn root_name_rows(&self) -> &RootNameRows {
        let index = self.root_names.0.get_or_init(Default::default);
        index.names.get_or_init(|| self.root_name_index())
    }

    fn root_name_index(&self) -> RootNameRows {
        let mut index = RootNameRows::default();
        if let Some(children) = self.child_handles(self.root) {
            for (position, symbol) in children.enumerate() {
                let full_name = self.name(symbol);
                index
                    .by_name
                    .entry(full_name.into())
                    .or_default()
                    .push((position as u32, symbol));
                if let Some((_, leaf)) = full_name.rsplit_once("::") {
                    index
                        .by_leaf
                        .entry(leaf.into())
                        .or_default()
                        .push((position as u32, symbol));
                }
                if self.get(symbol).kind == SymbolKind::Domain {
                    index.domains.push(symbol);
                }
            }
        }
        index
    }

    /*
        Every module's members, built on the first module member lookup.
        Nested module symbols precede the root children a module declares.
        Building it reads no module's children, so it cannot reenter itself.
    */
    fn module_child_roster(&self) -> &ModuleChildRoster {
        self.root_names
            .0
            .get_or_init(Default::default)
            .module_children
            .get_or_init(|| {
                let nested = self
                    .module_symbols
                    .iter()
                    .map(|(_, module)| (self.get(*module).parent, *module));
                let declared = self
                    .child_handles(self.root)
                    .into_iter()
                    .flatten()
                    .map(|symbol| (self.symbol_module(symbol), symbol))
                    .filter(|(module, _)| module.is_valid());
                ModuleChildRoster::from_members(nested.chain(declared))
            })
    }

    /*
        The `find_module_qualified_reference` candidate roster — root
        children then module symbols, in the same order — bucketed by every
        key the match arms can select on. Built once per table state.
    */
    fn module_scope_index(&self) -> &ModuleScopeIndex {
        self.module_scope_index.0.get_or_init(|| {
            let mut index = ModuleScopeIndex::default();
            let candidates = self
                .child_handles(self.root)
                .into_iter()
                .flatten()
                .chain(self.module_symbols.iter().map(|(_, handle)| *handle));
            for (order, candidate) in (0u32..).zip(candidates) {
                let name = self.name(candidate);
                index
                    .by_name
                    .entry(name.into())
                    .or_default()
                    .push((order, candidate));
                let path = self.indexed_display_path(candidate);
                index
                    .by_path
                    .entry(path.as_ref().into())
                    .or_default()
                    .push((order, candidate));
                if self.get(candidate).kind == SymbolKind::Domain
                    && let Some((_, leaf)) = name.rsplit_once("::")
                {
                    index
                        .domain_leaves
                        .entry(leaf.into())
                        .or_default()
                        .push((order, candidate));
                }
            }
            index
        })
    }

    /*
        Preserve the complete declaration roster after namespace and import
        precedence. Typed call selection consumes this roster; a signature-free
        reference still requires the existing unique/ambiguous lookup verdict.
    */
    pub fn top_level_candidates_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Vec<SymbolHandle> {
        self.lookup_top_level_candidates(
            name,
            kinds,
            reference,
            |_| true,
            TopLevelSelection::SignatureFree,
        )
    }

    fn lookup_top_level(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
        selection: TopLevelSelection,
    ) -> SymbolLookup {
        SymbolLookup::from_candidates(
            self.lookup_top_level_candidates(name, kinds, reference, matches_candidate, selection)
                .into_iter(),
        )
    }

    /// Retrieve the dependency edge of this file-qualified declaration use.
    pub fn dependency_file_use(
        &self,
        source: SourceId,
        path: &str,
    ) -> Option<(&str, PackageKeyIdentity)> {
        let (alias, _) = path.strip_prefix('|')?.split_once("|::")?;
        let mut candidates = self.source_scoped_bindings_for(source).iter().filter(|binding| {
            matches!(&binding.target, BindingTarget::FileAlias(file) if file.as_ref() == alias)
        });
        let binding = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        binding
            .dependency
            .as_ref()
            .map(|(alias, package)| (alias.as_ref(), *package))
    }

    fn file_alias_source(
        &self,
        reference: SourceId,
        alias: &str,
        product_operand: bool,
    ) -> Option<SourceId> {
        let mut sources = self
            .source_scoped_bindings_for(reference)
            .iter()
            .filter_map(|binding| {
                let name = match (&binding.target, product_operand) {
                    (BindingTarget::ProductFileAlias(name), true)
                    | (BindingTarget::FileAlias(name), false) => name,
                    _ => return None,
                };
                (name.as_ref() == alias).then_some(binding.declaration_source)
            });
        let source = sources.next()?;
        sources.next().is_none().then_some(source)
    }

    /// An explicit alias on a Build operand names only the admitted Product
    /// instance. A missing alias leaves an empty selection in that scope.
    pub fn product_file_selection<'a>(
        &self,
        path: &'a str,
        occurrence: SourceSpan,
    ) -> Option<ImportedProductMember<'a>> {
        let (alias, member) = path.strip_prefix('|')?.split_once("|::")?;
        let is_build = self
            .source_file(occurrence)
            .is_some_and(|file| file.dependency_scope == source_provenance::DependencyScope::Build);
        Some(ImportedProductMember {
            source: self.file_alias_source(occurrence.source_id, alias, is_build),
            member,
        })
    }

    /// Spell a retained declaration through each exact-file alias visible at the reference.
    pub fn file_import_names(
        &self,
        reference: SourceId,
        declaration: SourceId,
        selected: SymbolHandle,
    ) -> impl Iterator<Item = String> + '_ {
        let displayed = self.display_path(selected, "::");
        let member = self.file_member_path(selected, &displayed).to_owned();
        self.source_scoped_bindings_for(reference)
            .iter()
            .filter_map(move |binding| {
                if let BindingTarget::FileAlias(alias) = &binding.target
                    && binding.declaration_source == declaration
                {
                    Some(format!("|{alias}|::{member}"))
                } else {
                    None
                }
            })
    }

    /// Return a declaration's path within its owning file.
    pub fn file_member_path<'a>(&self, symbol: SymbolHandle, displayed: &'a str) -> &'a str {
        let module = self.symbol_module(symbol);
        let namespace = self.display_path(module, "::");
        displayed
            .strip_prefix(&namespace)
            .and_then(|name| name.strip_prefix("::"))
            .unwrap_or(displayed)
    }

    fn lookup_top_level_candidates(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        mut matches_candidate: impl FnMut(SymbolHandle) -> bool,
        selection: TopLevelSelection,
    ) -> Vec<SymbolHandle> {
        let reference_is_source_backed = reference.span.start != reference.span.end;
        if reference_is_source_backed
            && self
                .sources
                .as_deref()
                .is_some_and(|sources| !sources.source_is_selected(reference.source_id))
        {
            return Vec::new();
        }
        if let Some((alias, declaration)) = name
            .strip_prefix('|')
            .and_then(|path| path.split_once("|::"))
        {
            let Some(source) = self.file_alias_source(reference.source_id, alias, false) else {
                return Vec::new();
            };
            if !reference_is_source_backed {
                return Vec::new();
            }
            // The kernel supplies the declarations in the installed foundation file.
            // Its alias has already selected one exact source in this scope.
            if kinds.contains(&SymbolKind::MathematicalDefinition)
                && self
                    .sources
                    .as_deref()
                    .and_then(|sources| sources.bundled_core_file(source))
                    == Some(language_core::bundled_files::BundledFile::CoreFoundation)
            {
                return FixedCoreDeclaration::from_foundation_member(declaration)
                    .and_then(|fixed| self.fixed_core_declaration_symbol(fixed))
                    .filter(|symbol| matches_candidate(*symbol))
                    .into_iter()
                    .collect();
            }
            // A cast or qualification already supplies the carrier. Within the
            // selected file its domain may be named by its declared leaf.
            let domain_leaf = kinds.contains(&SymbolKind::Domain) && !declaration.contains("::");
            return self
                .top_level_children_matching_name(declaration, domain_leaf)
                .into_iter()
                .filter(|symbol| {
                    self.symbol_provenance_source_span(*symbol)
                        .is_some_and(|span| span.source_id == source)
                        && kinds.contains(&self.get(*symbol).kind)
                        && (self.name(*symbol) == declaration
                            || (domain_leaf && self.get(*symbol).kind == SymbolKind::Domain))
                        && self.source_reference_can_see_symbol(reference, *symbol)
                        && matches_candidate(*symbol)
                })
                .collect();
        }
        // Only the foundation file exports kernel slots. Their internal labels
        // have no source namespace of their own.
        let mut matches_candidate = |symbol| {
            self.fixed_core_declaration_for_symbol(symbol).is_none() && matches_candidate(symbol)
        };
        if (reference_is_source_backed || selection == TopLevelSelection::Ordinary)
            && let Some(result) = self.find_module_qualified_candidates(
                name,
                kinds,
                reference,
                &mut matches_candidate,
            )
        {
            return result;
        }
        if self.child_handles(self.root).is_none() {
            return Vec::new();
        }
        let candidates = self
            .top_level_children_matching_name(
                name,
                reference_is_source_backed && self.has_namespace_context(reference),
            )
            .into_iter()
            .filter(|symbol| {
                kinds.contains(&self.get(*symbol).kind)
                    && matches_candidate(*symbol)
                    && self.source_reference_can_see_symbol(reference, *symbol)
            })
            .collect::<Vec<_>>();

        if !reference_is_source_backed {
            if selection == TopLevelSelection::SignatureFree {
                return candidates;
            }
            return candidates.into_iter().take(1).collect();
        }

        if let Some(binding) = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .find(|binding| binding.binds_declaration(name))
        {
            let targets = candidates.iter().copied().filter(|symbol| {
                self.name(*symbol) == name
                    && self
                        .symbol_source_span(*symbol)
                        .is_some_and(|span| span.source_id == binding.declaration_source)
            });
            return targets.collect();
        }

        if self.has_namespace_context(reference) {
            return self.select_namespace_candidates(&candidates, name, reference);
        }
        if selection == TopLevelSelection::SignatureFree {
            let local = candidates
                .iter()
                .copied()
                .filter(|symbol| {
                    self.symbol_provenance_source_span(*symbol)
                        .is_some_and(|declaration| self.same_source_package(reference, declaration))
                })
                .collect::<Vec<_>>();
            return if local.is_empty() { candidates } else { local };
        }
        candidates
            .iter()
            .copied()
            .find(|symbol| {
                self.symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == reference.source_id)
            })
            .or_else(|| {
                self.sources.as_deref().and_then(|sources| {
                    candidates.iter().copied().find(|symbol| {
                        self.symbol_source_span(*symbol).is_some_and(|declaration| {
                            sources.same_checked_instance(reference, declaration)
                        })
                    })
                })
            })
            .or_else(|| candidates.first().copied())
            .into_iter()
            .collect()
    }

    /*
        Resolve either one exact declaration or one source-scoped overloaded
        operator family. The returned operator is only a representative;
        typed consumers must reconstruct and validate the complete same-path
        family before granting it authority.
    */
    pub fn find_top_level_declaration_or_operator_family_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Option<SymbolHandle> {
        if let Some(exact) =
            self.find_top_level_by_name_and_kinds_from_source(name, kinds, reference)
        {
            return Some(exact);
        }
        if reference.span.start == reference.span.end {
            return None;
        }
        let binding = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .find(|binding| binding.binds_declaration(name))?;
        let mut family = self.child_handles(self.root)?.filter(|symbol| {
            self.get(*symbol).kind == SymbolKind::Operator
                && kinds.contains(&SymbolKind::Operator)
                && self.name(*symbol) == name
                && self.source_reference_can_see_symbol(reference, *symbol)
                && self
                    .symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == binding.declaration_source)
        });
        let representative = family.next()?;
        family.next().map(|_| representative)
    }

    /*
        Whether two declarations occupy separate source-resolution contexts
        through the activation stratum or an explicit same-name binding.
    */
    pub fn source_scopes_separate(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
        let left = self.authored_origin(left);
        let right = self.authored_origin(right);
        let left_parent = self.get(left).parent;
        let right_parent = self.get(right).parent;
        if left_parent != right_parent
            && (self.get(left_parent).kind == SymbolKind::Machine
                || self.get(right_parent).kind == SymbolKind::Machine)
        {
            return true;
        }
        let (Some(left_span), Some(right_span)) = (
            self.symbol_source_span(left),
            self.symbol_source_span(right),
        ) else {
            return false;
        };
        if left_span.source_id == right_span.source_id {
            return false;
        }
        if self.sources.as_deref().is_some_and(|sources| {
            sources.same_package(left_span, right_span)
                && !sources.same_checked_instance(left_span, right_span)
        }) {
            return true;
        }
        if let Some(sources) = self.sources.as_deref()
            && sources.file_at(left_span).is_some()
            && sources.file_at(right_span).is_some()
            && !sources.same_package(left_span, right_span)
        {
            return !(self.prelude_exposes_symbol(left_span.source_id, right)
                || self.prelude_exposes_symbol(right_span.source_id, left));
        }
        true
    }

    /*
        Whether a source-backed reference may use this exact declaration under
        checked-purpose and current activation resolution-stratum boundaries.

        Source-free consumers retain their existing behavior. Compiler-generated
        symbols inherit the visibility of their exact authored derivation.
    */
    pub fn source_reference_can_see_symbol(
        &self,
        reference: SourceSpan,
        symbol: SymbolHandle,
    ) -> bool {
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        if self.get(symbol).kind != SymbolKind::Module && !self.inherited_symbol_is_selected(symbol)
        {
            return false;
        }
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        if self.get(symbol).kind == SymbolKind::Module {
            return self.module_source_ids(symbol).any(|source| {
                sources.reference_can_see_declaration(
                    reference,
                    SourceSpan::new(source, source_text::Span::default()),
                )
            });
        }
        sources.reference_can_see_declaration(reference, declaration)
    }

    /*
        Current declaration membership; source-free vocabulary remains present.
        Logical modules are present while any selected source contributes to them.
    */
    pub fn symbol_source_is_selected(&self, symbol: SymbolHandle) -> bool {
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        if self.get(symbol).kind == SymbolKind::Module {
            return self.provenance_source_span(symbol).is_none()
                || self.module_source_ids(symbol).next().is_some();
        }
        if !self.inherited_symbol_is_selected(symbol) {
            return false;
        }
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        sources.source_is_selected(declaration.source_id)
    }

    fn inherited_symbol_is_selected(&self, symbol: SymbolHandle) -> bool {
        self.inherited_symbols
            .get(symbol.arena_index() as usize)
            .copied()
            .unwrap_or(true)
    }

    /*
        Whether a source-backed reference may select this declaration as a
        candidate: the declaration is authored in the reference's own source,
        or in a source the reference's file imported. A declaration is never a candidate
        merely because its source was loaded (modules spec, "Import scope and
        exposure"): every target's entry contract joins every compilation, and
        the bundled spellings and names it pulls in must not change the
        resolution of a program that never imported them. Source-free
        references and symbols without authored provenance keep the whole
        program, as `source_reference_can_see_symbol` does.
    */
    pub fn source_reference_may_select_symbol(
        &self,
        reference: SourceSpan,
        symbol: SymbolHandle,
    ) -> bool {
        if !self.source_reference_can_see_symbol(reference, symbol) {
            return false;
        }
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        declaration.source_id == reference.source_id
            || self
                .source_scoped_bindings_for(reference.source_id)
                .iter()
                .any(|binding| match &binding.target {
                    BindingTarget::PreludeModule(_) => self.binding_exposes_member(binding, symbol),
                    BindingTarget::Declaration(_) => {
                        binding.declaration_source == declaration.source_id
                    }
                    BindingTarget::FileAlias(_) => {
                        binding.declaration_source == declaration.source_id
                    }
                    BindingTarget::ProductFileAlias(_) => false,
                })
    }

    pub fn find_descendant_by_path<'name>(
        &self,
        root: SymbolHandle,
        names: impl IntoIterator<Item = &'name str>,
    ) -> Option<SymbolHandle> {
        if !root.is_valid() {
            return None;
        }

        let mut current = root;

        for name in names {
            current = self.find_child_by_name(current, name)?;
        }

        Some(current)
    }

    pub fn builtin_function_symbol(&self, function: BuiltinFunction) -> Option<SymbolHandle> {
        let builtin_offset = BUILTIN_TYPE_COUNT.checked_add(function.ordinal())?;
        self.child_handles(self.root)?
            .nth(builtin_offset)
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinFunction)
    }

    /*
        Classify only an exact compiler-installed root builtin-function slot.
        Names are deliberately irrelevant: same-spelled package declarations
        and generated source-free symbols are not compiler functions.
    */
    pub fn builtin_function_for_symbol(&self, symbol: SymbolHandle) -> Option<BuiltinFunction> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::BuiltinFunction {
            return None;
        }
        self.root_slot_ordinal(symbol, BUILTIN_TYPE_COUNT, BuiltinFunction::COUNT)
            .and_then(BuiltinFunction::from_ordinal)
    }

    /*
        The root child installed for `declaration`, if this table carries the
        fixed `core::*` vocabulary at its canonical slots.
    */
    pub fn fixed_core_declaration_symbol(
        &self,
        declaration: FixedCoreDeclaration,
    ) -> Option<SymbolHandle> {
        let offset = BUILTIN_TYPE_COUNT
            .checked_add(BuiltinFunction::COUNT)?
            .checked_add(declaration.ordinal())?;
        self.child_handles(self.root)?
            .nth(offset)
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::MathematicalDefinition)
    }

    /*
        Classify only an exact compiler-installed fixed `core::*` slot. An
        authored mathematical definition with the same spelling is not one.
    */
    pub fn fixed_core_declaration_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<FixedCoreDeclaration> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::MathematicalDefinition {
            return None;
        }
        self.root_slot_ordinal(
            symbol,
            BUILTIN_TYPE_COUNT.checked_add(BuiltinFunction::COUNT)?,
            FixedCoreDeclaration::COUNT,
        )
        .and_then(FixedCoreDeclaration::from_ordinal)
    }

    /// The compiler's mathematical universe sorts. Their sole operand is a
    /// level in the proof calculus, with no ordinary constant carrier.
    pub fn universe_sort_for_symbol(&self, symbol: SymbolHandle) -> Option<FixedCoreDeclaration> {
        self.fixed_core_declaration_for_symbol(symbol)
            .filter(|sort| {
                matches!(
                    sort,
                    FixedCoreDeclaration::Type | FixedCoreDeclaration::Strict
                )
            })
    }

    /*
        The member's compiler-installed slot beneath its exact builtin owner.
        A package member with the same spelling never supplies this identity.
    */
    pub fn builtin_type_member_symbol(&self, member: BuiltinTypeMember) -> Option<SymbolHandle> {
        self.child_handles(
            self.child_handles(self.root)?
                .nth(member.owner().ordinal())?,
        )?
        .nth(member.ordinal())
        .filter(|symbol| self.get(*symbol).kind == SymbolKind::MathematicalDefinition)
    }

    pub fn builtin_type_member(&self, symbol: SymbolHandle) -> Option<BuiltinTypeMember> {
        symbol.is_valid().then_some(())?;
        let member = BuiltinTypeMember::AddressBound;
        (self.builtin_type_member_symbol(member) == Some(symbol)).then_some(member)
    }

    pub fn builtin_type_symbol(&self, builtin_type: BuiltinType) -> Option<SymbolHandle> {
        self.child_handles(self.root)?
            .nth(builtin_type.ordinal())
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinType)
    }

    /// Scalar meaning is derived from the selected builtin, never its display name.
    pub fn primitive_type(
        &self,
        symbol: SymbolHandle,
    ) -> Option<crate::source_symbols::PrimitiveType> {
        self.builtin_type_atom(symbol)
            .and_then(crate::source_symbols::PrimitiveType::from_builtin_atom)
    }

    /*
        Classify only a compiler-installed root builtin type. A package
        declaration with the same spelling has a declaration kind, and a
        generated symbol has a derivation origin, so neither is a compiler
        atom.

        A fixed root slot answers for the roster. Every other installed
        builtin type is an integer carrier the compilation names
        (`integer_type_symbol`), and its name is that carrier's decimal
        spelling: the carrier is read back from it, so `u13` needs no row
        anywhere and has the same kind of identity `u32` has.
    */
    pub fn builtin_type_atom(&self, symbol: SymbolHandle) -> Option<BuiltinTypeAtom> {
        if !symbol.is_valid() {
            return None;
        }
        let entry = self.get(symbol);
        if entry.kind != SymbolKind::BuiltinType {
            return None;
        }
        if let Some(slot) = self.root_slot_ordinal(symbol, 0, BUILTIN_TYPE_COUNT) {
            return BuiltinTypeAtom::from_ordinal(slot);
        }
        if entry.parent != self.root || entry.generated_from.is_valid() {
            return None;
        }
        FixedInteger::from_decimal_spelling(self.name(symbol))
            .map(BuiltinTypeAtom::Integer)
            .filter(|atom| atom.roster_ordinal().is_none())
    }

    /*
        The symbol of a builtin type atom, when this table has one: the fixed
        slot of a roster member, otherwise the builtin type installed under
        an integer carrier's spelling. `None` for a carrier means no source
        of this compilation named it, so nothing can refer to it yet.
    */
    pub fn builtin_atom_symbol(&self, atom: BuiltinTypeAtom) -> Option<SymbolHandle> {
        if let Some(slot) = atom.roster_ordinal() {
            return self
                .child_handles(self.root)?
                .nth(slot)
                .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinType);
        }
        let name = atom.symbol_name();
        self.root_name_rows()
            .by_name
            .get(name.as_ref())
            .into_iter()
            .flatten()
            .map(|&(_, symbol)| symbol)
            .find(|symbol| self.builtin_type_atom(*symbol) == Some(atom))
    }

    /*
        Install a builtin type for each integer carrier that has no symbol
        yet, so a name such as `u13` resolves the way `u32` does. Carriers
        already present, including the eight with fixed slots, are left
        alone: one carrier has one symbol for the life of the table.
    */
    pub fn install_integer_types(&mut self, carriers: impl IntoIterator<Item = FixedInteger>) {
        let mut missing = carriers
            .into_iter()
            .filter(|carrier| {
                self.builtin_atom_symbol(BuiltinTypeAtom::Integer(*carrier))
                    .is_none()
            })
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return;
        }
        let names = missing
            .iter()
            .map(|carrier| carrier.to_string())
            .collect::<Vec<_>>();
        self.append_children(
            self.root,
            names
                .iter()
                .map(|name| (SymbolKind::BuiltinType, SymbolNameRef::Borrowed(name))),
        );
    }

    /*
        `u<N>` and `i<N>` apply a family of integers to a width, and the head
        of such an application needs a declaration to name, the way
        `Pair<u8>` names `Pair`. The two heads are builtin types installed
        beside the program's declarations under the spellings `u` and `i`, so
        they resolve like any other type name. They are installed only for a
        compilation that keeps such an application: every position a table
        without one publishes stays where it was.

        A head is not a type by itself here: it has no atom
        (`builtin_type_atom` answers `None`), and only an application at a
        closed width names a carrier.
    */
    pub fn install_integer_families(&mut self) {
        let missing = [IntegerSign::Unsigned, IntegerSign::Signed]
            .into_iter()
            .filter(|sign| self.integer_family_symbol(*sign).is_none())
            .map(|sign| {
                (
                    SymbolKind::BuiltinType,
                    SymbolNameRef::Static(integer_family_name(sign)),
                )
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.append_children(self.root, missing);
        }
    }

    /* The sign of the integer family `symbol` is the head of, if it is one. */
    pub fn integer_family(&self, symbol: SymbolHandle) -> Option<IntegerSign> {
        if !symbol.is_valid() {
            return None;
        }
        let entry = self.get(symbol);
        if entry.kind != SymbolKind::BuiltinType
            || entry.parent != self.root
            || entry.generated_from.is_valid()
        {
            return None;
        }
        [IntegerSign::Unsigned, IntegerSign::Signed]
            .into_iter()
            .find(|sign| self.name(symbol) == integer_family_name(*sign))
    }

    pub fn integer_family_symbol(&self, sign: IntegerSign) -> Option<SymbolHandle> {
        self.root_name_rows()
            .by_name
            .get(integer_family_name(sign))
            .into_iter()
            .flatten()
            .map(|&(_, symbol)| symbol)
            .find(|symbol| self.integer_family(*symbol) == Some(sign))
    }

    /*
        Ordinal of `symbol` among the `count` compiler-installed root slots
        starting at root child `first`. The builder installs those slots as
        the root's first contiguous children, so an ordinal is an arena
        distance from the first child, confirmed by `nth` rather than a scan.
    */
    fn root_slot_ordinal(&self, symbol: SymbolHandle, first: usize, count: usize) -> Option<usize> {
        let start = self.child_handles(self.root)?.next()?;
        let ordinal = symbol.arena_index().checked_sub(start.arena_index())? as usize;
        let slot = ordinal.checked_sub(first).filter(|slot| *slot < count)?;
        (self.child_handles(self.root)?.nth(ordinal) == Some(symbol)).then_some(slot)
    }

    /*
        `display_path` for a roster member through the per-table snapshot:
        zero allocation on a hit, and handles minted after the snapshot
        (generated roots, late module registrations, extension children)
        miss the map and fall back to the parent walk.
    */
    pub(super) fn indexed_display_path(&self, symbol: SymbolHandle) -> std::borrow::Cow<'_, str> {
        let index = self.module_path_index.0.get_or_init(|| {
            let mut paths = arena::HandleKeyMap::default();
            for candidate in self
                .child_handles(self.root)
                .into_iter()
                .flatten()
                .chain(self.module_symbols.iter().map(|(_, handle)| *handle))
            {
                paths.insert(candidate, Arc::from(self.display_path(candidate, "::")));
            }
            ModulePathIndex { paths }
        });
        match index.paths.get(&symbol) {
            Some(path) => std::borrow::Cow::Borrowed(path.as_ref()),
            None => std::borrow::Cow::Owned(self.display_path(symbol, "::")),
        }
    }

    pub fn display_path(&self, symbol: SymbolHandle, separator: &str) -> String {
        if !symbol.is_valid() {
            return String::new();
        }

        let mut names = Vec::new();
        let mut current = symbol;

        while current.is_valid() && current != self.root {
            names.push(self.name(current));
            current = self.namespace_parent(current);
        }

        let byte_count = names.iter().map(|name| name.len()).sum::<usize>()
            + separator
                .len()
                .saturating_mul(names.len().saturating_sub(1));
        let mut path = String::with_capacity(byte_count);

        for (index, name) in names.iter().rev().enumerate() {
            if index > 0 {
                path.push_str(separator);
            }

            path.push_str(name);
        }

        path
    }

    pub fn symbols(&self) -> &SymbolHierarchy {
        &self.symbols
    }

    pub fn names(&self) -> &SymbolRows<SymbolName> {
        &self.names
    }
}

pub struct SymbolChildHandles<'table> {
    contiguous: Option<SpanHandles<Symbol>>,
    supplemental: SupplementalChildHandles<'table>,
    namespace: std::vec::IntoIter<SymbolHandle>,
}

impl Iterator for SymbolChildHandles<'_> {
    type Item = SymbolHandle;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.contiguous
            .as_mut()
            .and_then(Iterator::next)
            .or_else(|| self.namespace.next())
            .or_else(|| self.supplemental.next())
    }

    /*
        Skip whole parts by length: builtin lookups index the root's children
        by ordinal, and stepping through them one at a time was measurable.
    */
    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if let Some(contiguous) = self.contiguous.as_mut() {
            let length = contiguous.len();
            if n < length {
                return contiguous.nth(n);
            }
            n -= length;
            let _ = contiguous.nth(length);
        }
        let length = self.namespace.len();
        if n < length {
            return self.namespace.nth(n);
        }
        n -= length;
        let _ = self.namespace.nth(length);
        self.supplemental.nth(n)
    }
}

pub struct SymbolTableExtension {
    table: SymbolTable,
}

impl SymbolTableExtension {
    pub fn insert_top_level<'name>(
        &mut self,
        declarations: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> Vec<SymbolHandle> {
        SymbolTableBuilder::child_handles(self.table.append_children(self.table.root, declarations))
            .collect()
    }

    /* Append beneath an existing declaration while retaining its exact node. */
    pub fn append_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        self.table.append_children(parent, children)
    }

    pub fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        let children = children
            .into_iter()
            .map(|(kind, name)| {
                child_symbol(
                    &mut self.table.names,
                    self.table.sources.as_deref(),
                    parent,
                    kind,
                    name,
                )
            })
            .collect::<Vec<_>>();
        self.table.symbols.insert_children(parent, children)
    }

    pub fn finish(mut self) -> SymbolTable {
        self.table.root_names.clear();
        self.table.module_path_index.clear();
        self.table.module_scope_index.clear();
        self.table
    }
}

impl SymbolTableAppender for SymbolTableExtension {
    fn source_map(&self) -> Option<Arc<SourceMap>> {
        self.table.source_map()
    }

    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        SymbolTableExtension::insert_children(self, parent, children)
    }
}

fn name_from_source(name: SymbolNameRef<'_>, sources: Option<&SourceMap>) -> SymbolName {
    let text = match name {
        SymbolNameRef::OwnedSource { source_span, .. } => sources
            .and_then(|sources| sources.file_at(source_span))
            .map(|file| &file.source),
        _ => None,
    };
    SymbolName::from_ref_with_text(name, text)
}
