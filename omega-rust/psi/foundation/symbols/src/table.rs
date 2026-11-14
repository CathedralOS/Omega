/*
    Start here. The symbol table: the hierarchy arena scoped lookup walks,
    its builder, appender and extension.
*/

use std::sync::Arc;

use arena::{Arena, HandleSpan, HierarchyArena, SpanHandles};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceFile, SourceId, SourceMap, SourceOrigin, SourceSpan};

use super::builtin::BUILTIN_TYPE_COUNT;
use numerics::integer_carrier::{FixedInteger, IntegerSign};

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

    use crate::{Symbol, SymbolHandle};
    use arena::{Arena, Handle, HandleSpan};

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    struct ChildBatch {
        children: HandleSpan<Symbol>,
        next: Handle<ChildBatch>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    struct ChildChain {
        first: Handle<ChildBatch>,
        last: Handle<ChildBatch>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(super) struct SupplementalChildren {
        batches: Arena<ChildBatch>,
        parents: Vec<ChildChain>,
    }

    impl compiler_storage::Record for SupplementalChildren {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            output.record(|output| {
                self.batches.write(output)?;
                self.parents.write(output)
            })
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            input.record(|input| {
                let batches = Arena::<ChildBatch>::read(input)?;
                let parents = Vec::<ChildChain>::read(input)?;

                // Restored links must not turn missing declarations into empty
                // lookup results, loop forever, or append through the dummy row.
                // Every retained batch belongs to exactly one terminating chain
                // whose recorded tail is its final batch. Zero denotes no chain.
                input.allocate(batches.storage_slice().len())?;
                let mut visited = vec![false; batches.storage_slice().len()];
                let mut visited_count = 0;
                for chain in &parents {
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
                self.parents.resize(parent_index + 1, ChildChain::default());
            }
            let next = self.batches.insert(ChildBatch {
                children,
                next: Handle::invalid(),
            });
            let chain = &mut self.parents[parent_index];
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
        batches: &'table Arena<ChildBatch>,
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
                    let remaining = self.children.count() - ordinal as u32 - 1;
                    self.children = HandleSpan::from_parts_or_empty(
                        self.children.handle_at(ordinal as u32 + 1),
                        remaining,
                    );
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
        use crate::{SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
        use compiler_storage::{RecordLimits, decode, encode};

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

        fn restore(table: &SymbolTable) -> Result<SymbolTable, compiler_storage::RecordError> {
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
            shared
                .supplemental_children
                .parents
                .push(root_chain(&shared));
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
                table.supplemental_children.parents[parent].last = tail;
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
            table.supplemental_children.parents[parent] = ChildChain::default();
            let error = restore(&table).expect_err("unowned batch storage");
            assert!(error.reason.contains("batch has no owning chain"));
        }

        #[test]
        fn supplemental_storage_charges_chain_validation_scratch() {
            #[derive(compiler_storage::Record)]
            struct UncheckedChildren {
                batches: Arena<ChildBatch>,
                parents: Vec<ChildChain>,
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
            let scratch = children.batches.storage_slice().len();
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
    use source::SourceId;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    pub(super) struct RootNameIndex {
        pub(super) by_name: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        pub(super) by_leaf: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
    }

    /*
        Selected bindings and their derived namespace environment belong to one
        immutable table frontier. Queries borrow these rows without rebuilding paths.
    */
    #[derive(Clone, Default)]
    pub(super) struct BindingSourceIndex {
        pub(super) by_source: crate::SymbolKeyMap<SourceId, SourceLookupScope>,
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
        pub(super) package_prefix: String,
        pub(super) module_leaf: Option<String>,
    }

    #[derive(Clone, Default)]
    pub(super) struct ModulePathIndex {
        pub(super) paths: crate::SymbolKeyMap<SymbolHandle, Arc<str>>,
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
        pub(super) by_name: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        /* `display_path("::")` → (order, handle). */
        pub(super) by_path: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
        /*
            Declared leaf name → (order, handle), for Domain-kind candidates
            only: the carrier-qualified arm is the sole leaf-shaped selector.
        */
        pub(super) domain_leaves: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
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
        impl compiler_storage::Record for $cache {
            fn write(&self, _: &mut compiler_storage::RecordWriter) -> Result<(), compiler_storage::RecordError> { Ok(()) }
            fn read<Context>(_: &mut compiler_storage::RecordReader<'_, Context>) -> Result<Self, compiler_storage::RecordError> { Ok(Self::default()) }
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
    BindingSourceIndexCache, ModulePathIndex, ModulePathIndexCache, ModuleScopeIndex,
    ModuleScopeIndexCache, RootNameIndex, RootNameIndexCache,
};
mod modules {
    /*
        Logical module ownership without relocating physical declaration slots.

        Local declarations precede imported and root names, including relative
        attached paths. A narrow import may expose an attached declaration's leaf;
        retaining that candidate does not expose sibling declarations or make its
        leaf a bare local name. Every candidate still needs its exact source owner.
        A loader-certified package alias can qualify other already-loaded sources
        of that same dependency; that lookup does not change which exact source
        must satisfy an authored import or expose additional unqualified names.
    */

    use std::sync::Arc;

    use arena::HandleSpan;
    use source::{SourceId, SourceSpan};

    use super::indices::{BindingSourceIndex, PreparedModuleImport, SourceLookupScope};
    use super::{SourceScopedTopLevelBinding, SymbolLookup, SymbolTable};
    use crate::{Symbol, SymbolHandle, SymbolKind, SymbolName, SymbolNameRef};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub(super) struct ModuleImport {
        pub(super) path: Arc<str>,
        package_prefix_members: usize,
        exact_source: bool,
        logical_selection: bool,
    }

    impl SourceScopedTopLevelBinding {
        /*
            Bind one authored import to the exact source loaded through the
            requester's reconciled package graph. Only the loader may strip the
            certified package-prefix members from the logical module path.
        */
        pub fn module_import(
            reference_source: SourceId,
            declaration_source: SourceId,
            authored_path: &str,
            package_prefix_members: usize,
        ) -> Self {
            Self {
                reference_source,
                declaration_source,
                name: Arc::from(""),
                module_import: Some(ModuleImport {
                    path: Arc::from(authored_path),
                    package_prefix_members,
                    exact_source: true,
                    logical_selection: false,
                }),
                producer: super::BindingProducer::Loader,
            }
        }
    }

    impl SourceScopedTopLevelBinding {
        /*
            A logical import selects declarations across the admitted units of
            one checked package. The loader supplies an anchor for that package;
            its filename grants neither a namespace nor unqualified exposure.
        */
        pub fn logical_module_import(
            reference_source: SourceId,
            declaration_source: SourceId,
            authored_path: &str,
            package_prefix_members: usize,
        ) -> Self {
            let mut binding = Self::module_import(
                reference_source,
                declaration_source,
                authored_path,
                package_prefix_members,
            );
            binding
                .module_import
                .as_mut()
                .expect("module import")
                .logical_selection = true;
            binding
        }
    }

    impl SymbolTable {
        /*
            Whether this source has a loader-certified package qualifier. Package
            aliases select namespaces without manufacturing a Module declaration.
            Type and trait qualifiers retain their separate call semantics.
        */
        pub fn is_source_package_qualifier(&self, reference: SourceSpan, qualifier: &str) -> bool {
            self.source_scoped_bindings_for(reference.source_id)
                .iter()
                .filter_map(|binding| binding.module_import.as_ref())
                .any(|import| {
                    import.exact_source
                        && import.package_prefix_members != 0
                        && import
                            .path
                            .split("::")
                            .take(import.package_prefix_members)
                            .eq(qualifier.split("::"))
                })
        }

        /* Authored import spellings retained for source lookup diagnostics. */
        pub fn source_module_import_paths(&self, source: SourceId) -> impl Iterator<Item = &str> {
            self.source_scoped_bindings_for(source)
                .iter()
                .filter_map(move |binding| {
                    (binding.reference_source == source)
                        .then_some(binding.module_import.as_ref())
                        .flatten()
                        .map(|import| import.path.as_ref())
                })
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
                    let symbol_name = self.names.insert(SymbolName::from_ref_with_sources(
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
                self.source_module_index
                    .resize(source.0 + 1, SymbolHandle::invalid());
            }
            self.source_module_index[source.0] = parent;
            Ok(())
        }

        /*
            Retain an import for syntax-only callers. Compiler-loaded imports
            already carry exact source custody and take precedence over this row.
        */
        pub fn register_source_import(&mut self, reference_source: SourceId, path: &str) {
            if self.source_scoped_top_level_bindings.iter().any(|binding| {
                self.source_binding_is_selected(binding)
                    && binding.reference_source == reference_source
                    && binding
                        .module_import
                        .as_ref()
                        .is_some_and(|import| import.path.as_ref() == path)
            }) {
                return;
            }
            let mut binding = SourceScopedTopLevelBinding::module_import(
                reference_source,
                reference_source,
                path,
                0,
            );
            if let Some(import) = &mut binding.module_import {
                import.exact_source = false;
            }
            binding.producer = super::BindingProducer::AuthoredImport;
            self.source_scoped_top_level_bindings.push(binding);
            self.binding_source_index.clear();
        }

        /*
            An authored logical import must select a unique declaration or
            namespace in its certified checked package, including module-less
            siblings. Exact-file inputs retain their existing file admission.
        */
        pub fn validate_source_module_import(
            &self,
            reference_source: SourceId,
            path: &str,
        ) -> Result<(), String> {
            let mut inactive_import = false;
            let mut selected_import = false;
            for binding in self
                .source_scoped_top_level_bindings
                .iter()
                .filter(|binding| binding.reference_source == reference_source)
            {
                let Some(import) = &binding.module_import else {
                    continue;
                };
                if import.path.as_ref() != path || !import.exact_source {
                    continue;
                }
                if !self.source_binding_is_selected(binding) {
                    inactive_import = true;
                    continue;
                }
                selected_import = true;
                if import.logical_selection {
                    if !self.binding_imports_package_root(binding)
                        && self.logical_import_targets(binding).is_empty()
                    {
                        return Err(format!(
                            "import `{path}` does not select one declaration, callable family, or module in its checked package"
                        ));
                    }
                    continue;
                }
                let module = self.source_module(binding.declaration_source);
                if !module.is_valid() {
                    continue;
                }
                let logical = logical_import_path(import);
                let namespace = self.indexed_display_path(module);
                if logical == namespace.as_ref() {
                    continue;
                }
                let declarations =
                    self.child_handles(self.root)
                        .into_iter()
                        .flatten()
                        .filter(|candidate| {
                            self.import_owner_matches(binding, *candidate)
                                && self.indexed_display_path(*candidate) == logical.as_str()
                        });
                if unique(declarations).is_none() {
                    return Err(format!(
                        "import `{path}` does not select one exact declaration or module in declared namespace `{namespace}`"
                    ));
                }
            }
            if inactive_import && !selected_import {
                Err(format!(
                    "import `{path}` refers to an inactive source instance"
                ))
            } else {
                Ok(())
            }
        }

        /*
            Return the declaration selected by a validated import. A package
            root is a namespace, not a fabricated declaration. Exact-file inputs
            likewise have no independently selected namespace declaration.
        */
        pub fn source_module_import_target(
            &self,
            reference_source: SourceId,
            path: &str,
        ) -> Option<SymbolHandle> {
            unique(
                self.source_module_import_targets(reference_source, path)
                    .into_iter(),
            )
        }

        /*
            An imported callable name retains every overload, not an arbitrary
            representative. Each member receives ordinary declaration custody;
            call-site typing still selects its complete normalized signature.
        */
        pub fn source_module_import_targets(
            &self,
            reference_source: SourceId,
            path: &str,
        ) -> Vec<SymbolHandle> {
            for binding in self.source_scoped_bindings_for(reference_source) {
                let Some(import) = &binding.module_import else {
                    continue;
                };
                if import.path.as_ref() != path {
                    continue;
                }
                if import.logical_selection {
                    return self.logical_import_targets(binding);
                }
                if import.exact_source && !self.source_module(binding.declaration_source).is_valid()
                {
                    return Vec::new();
                }
                let logical = logical_import_path(import);
                return unique(
                    self.child_handles(self.root)
                        .into_iter()
                        .flatten()
                        .chain(self.module_symbols.iter().map(|(_, symbol)| *symbol))
                        .filter(|candidate| {
                            self.import_owner_matches(binding, *candidate)
                                && self.indexed_display_path(*candidate) == logical.as_str()
                        }),
                )
                .into_iter()
                .collect();
            }
            Vec::new()
        }

        fn binding_imports_package_root(&self, binding: &SourceScopedTopLevelBinding) -> bool {
            binding.module_import.as_ref().is_some_and(|import| {
                import.logical_selection
                    && import.package_prefix_members != 0
                    && logical_import_path(import).is_empty()
                    && self.source_binding_is_selected(binding)
                    && self.sources.as_deref().is_some_and(|sources| {
                        let span = source::Span::new(0, 1);
                        sources.reference_can_see_declaration(
                            SourceSpan::new(binding.reference_source, span),
                            SourceSpan::new(binding.declaration_source, span),
                        )
                    })
            })
        }

        fn logical_import_targets(
            &self,
            binding: &SourceScopedTopLevelBinding,
        ) -> Vec<SymbolHandle> {
            let Some(import) = &binding.module_import else {
                return Vec::new();
            };
            let logical = logical_import_path(import);
            if logical.is_empty() || !self.source_binding_is_selected(binding) {
                return Vec::new();
            }
            let index = self.module_scope_index();
            let Some(entries) = index.by_path.get(logical.as_str()) else {
                return Vec::new();
            };
            let mut candidates = entries
                .iter()
                .filter_map(|(_, candidate)| {
                    self.import_owner_matches(binding, *candidate)
                        .then_some(*candidate)
                })
                .collect::<Vec<_>>();
            candidates.dedup();
            if candidates.len() > 1
                && !candidates.iter().all(|candidate| {
                    matches!(
                        self.get(*candidate).kind,
                        SymbolKind::Machine | SymbolKind::Operator
                    )
                })
            {
                return Vec::new();
            }
            candidates
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
                None => !binding
                    .module_import
                    .as_ref()
                    .is_some_and(|import| import.logical_selection),
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

        /*
            Whether an authored domain reference may select `domain` from
            `reference` under module name law. A fully qualified spelling always
            selects its exact declaration. A relative spelling reaches a
            module-owned domain inside its own module or through an exposing
            import: a narrow import of the exact declaration exposes its leaf and
            its declared carrier-qualified spelling, while importing the declaring
            module exposes its directly declared domains to the carrier-qualified
            spelling. The carrier-qualified form additionally requires the
            authored carrier to resolve to the exact carrier the declaring context
            attached the domain to -- importing the carrier type, an ordinary
            sibling declaration, or merely loading another source exposes nothing.
            Module-less domains belong to their checked package's root module;
            merely loading a foreign root does not expose its domains.
            Resolution-stratum and package visibility stay with the caller.
        */
        pub fn domain_name_reaches(
            &self,
            domain: SymbolHandle,
            authored: &str,
            reference: SourceSpan,
        ) -> bool {
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
                    && self
                        .source_module_import_paths(reference.source_id)
                        .any(|path| {
                            self.source_module_import_target(reference.source_id, path)
                                == Some(domain)
                        });
            }
            self.carrier_qualified_domain_reaches(domain, authored, reference)
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

        /*
            Whether one authored import edge in `reference` brings `symbol`'s
            leaf name into that source's resolution scope. Importing an
            unmoduled source exposes every leaf it declares; a moduled source
            exposes a leaf only through a narrow import of that exact
            declaration, so a broad `module::name` edge never collides with a
            package-local `name`.
        */
        pub(super) fn import_exposes_symbol(
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
                    let Some(import) = &binding.module_import else {
                        return false;
                    };
                    (self.import_owner_matches(binding, symbol)
                        && self.indexed_display_path(symbol) == logical_import_path(import))
                        || (import.exact_source
                            && !import.logical_selection
                            && !self.source_module(declaration.source_id).is_valid()
                            && self.exact_import_exposes_source(
                                binding.declaration_source,
                                declaration,
                            ))
                })
        }

        /*
            Whether an exact-source import of `imported_source` exposes a
            declaration at `declaration`: it is authored in that file, or it is a
            generated unit of the same checked package instance. A package's
            module-less files form one logical module, and the units its build
            handed off (`SourceResolutionStratum::DependencyActivationExtension`)
            are that module's later stratum, so importing the package's root file
            imports them too. Two checked instances of one package never expose
            each other's units.
        */
        pub(super) fn exact_import_exposes_source(
            &self,
            imported_source: SourceId,
            declaration: SourceSpan,
        ) -> bool {
            if declaration.source_id == imported_source {
                return true;
            }
            !self.source_module(imported_source).is_valid()
                && self.source_file(declaration).is_some_and(|file| {
                    file.resolution_stratum
                        == source::SourceResolutionStratum::DependencyActivationExtension
                })
                && self.sources.as_deref().is_some_and(|sources| {
                    sources.same_checked_instance(
                        declaration,
                        SourceSpan::new(imported_source, declaration.span),
                    )
                })
        }

        /*
            Loading a package unit does not expose its domains. This shared
            predicate follows the importing file's exact selection: a domain,
            its declaring module, or the module-less package root. Visibility
            and the attached carrier's authority remain checked at the use site.
        */
        pub fn source_import_exposes_domain(
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
                            .module_import
                            .as_ref()
                            .is_some_and(|import| import.path.as_ref() == path)
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
                    self.same_source_package(reference, span)
                        && self.source_module(reference.source_id) == module
                });
            local
                || (!module.is_valid() && self.is_ambient_toolchain_symbol(domain))
                || self
                    .source_module_import_paths(reference.source_id)
                    .any(|path| self.source_import_exposes_domain(reference, path, domain))
        }

        pub(super) fn binding_exposes_member(
            &self,
            binding: &SourceScopedTopLevelBinding,
            symbol: SymbolHandle,
        ) -> bool {
            let Some(import) = &binding.module_import else {
                return false;
            };
            if !self.import_owner_matches(binding, symbol) {
                return false;
            }
            if import.logical_selection {
                if self.binding_imports_package_root(binding) {
                    return !self.symbol_module(symbol).is_valid();
                }
                return self
                    .logical_import_targets(binding)
                    .into_iter()
                    .any(|target| {
                        target == symbol
                            || (self.get(target).kind == SymbolKind::Module
                                && self.symbol_module(symbol) == target)
                    });
            }
            let module = self.source_module(binding.declaration_source);
            if import.exact_source && !module.is_valid() {
                return true;
            }
            let logical = logical_import_path(import);
            self.indexed_display_path(symbol) == logical.as_str()
                || (self.symbol_module(symbol).is_valid()
                    && self.indexed_display_path(self.symbol_module(symbol)) == logical.as_str())
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

        pub(super) fn module_children(&self, module: SymbolHandle) -> Vec<SymbolHandle> {
            self.module_symbols
                .iter()
                .map(|(_, handle)| *handle)
                .filter(|handle| self.get(*handle).parent == module)
                .chain(
                    self.child_handles(self.root)
                        .into_iter()
                        .flatten()
                        .filter(|handle| self.symbol_module(*handle) == module),
                )
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
                for binding in &self.source_scoped_top_level_bindings {
                    if !self.source_binding_is_selected(binding) {
                        continue;
                    }
                    let scope = index.by_source.entry(binding.reference_source).or_default();
                    if let Some(import) = &binding.module_import {
                        let logical = logical_import_path(import);
                        let package_prefix = import
                            .path
                            .split("::")
                            .take(import.package_prefix_members)
                            .collect::<Vec<_>>()
                            .join("::");
                        let module = self.source_module(binding.declaration_source);
                        let module_leaf = (module.is_valid()
                            && self.indexed_display_path(module) == logical.as_str())
                        .then(|| self.name(module).to_string());
                        scope.has_exact_import |= import.exact_source;
                        scope.imports.push(PreparedModuleImport {
                            binding_position: scope.bindings.len(),
                            logical,
                            package_prefix,
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
            self.source_file(reference).is_some_and(|file| {
                file.origin.is_toolchain()
                    || (file.origin == source::SourceOrigin::User
                        && file.resolution_stratum
                            != source::SourceResolutionStratum::CurrentActivationExtension)
            }) || self.source_binding_index().has_selected_module
                || self
                    .source_lookup_scope(reference.source_id)
                    .has_exact_import
        }

        /*
            A current-activation extension file sees its module-less package
            siblings' declarations as local rather than file-private.
        */
        pub(super) fn shares_activation_extension(&self, source_span: SourceSpan) -> bool {
            self.source_file(source_span).is_some_and(|file| {
                file.resolution_stratum
                    == source::SourceResolutionStratum::CurrentActivationExtension
            })
        }

        pub(super) fn select_namespace_candidates(
            &self,
            candidates: &[SymbolHandle],
            name: &str,
            reference: SourceSpan,
        ) -> Vec<SymbolHandle> {
            let current_module = self.source_module(reference.source_id);
            let shared_extension = self.shares_activation_extension(reference);
            let local = candidates
                .iter()
                .copied()
                .filter(|candidate| {
                    self.name(*candidate) == name
                        && self.symbol_module(*candidate) == current_module
                        && self
                            .symbol_provenance_source_span(*candidate)
                            .is_some_and(|span| {
                                if current_module.is_valid() {
                                    self.same_source_package(reference, span)
                                } else {
                                    span.source_id == reference.source_id
                                        || (shared_extension
                                            && self.same_source_package(reference, span)
                                            && self.sources.as_deref().is_some_and(|sources| {
                                                !sources.resolution_strata_separate(reference, span)
                                            }))
                                }
                            })
                })
                .collect::<Vec<_>>();
            if !local.is_empty() {
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
                        let import = binding
                            .module_import
                            .as_ref()
                            .expect("import bindings carry module imports");
                        if !self.import_owner_matches(binding, *candidate) {
                            return false;
                        }
                        self.indexed_display_path(*candidate) == logical.as_str()
                            || (import.exact_source
                                && !import.logical_selection
                                && self.name(*candidate) == name
                                && !self.source_module(binding.declaration_source).is_valid()
                                && self.symbol_provenance_source_span(*candidate).is_some_and(
                                    |span| {
                                        self.exact_import_exposes_source(
                                            binding.declaration_source,
                                            span,
                                        )
                                    },
                                ))
                    })
                })
                .collect::<Vec<_>>();
            if !imported.is_empty() {
                return imported;
            }
            if !current_module.is_valid() {
                let package = candidates.iter().copied().filter(|candidate| {
                    self.name(*candidate) == name
                        && !self.symbol_module(*candidate).is_valid()
                        && self
                            .symbol_provenance_source_span(*candidate)
                            .is_some_and(|span| self.same_source_package(reference, span))
                });
                let package = package.collect::<Vec<_>>();
                if !package.is_empty() {
                    return package;
                }
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
            let Some(import) = &binding.module_import else {
                return false;
            };
            if import.logical_selection {
                let Some(declaration) = self.symbol_provenance_source_span(candidate) else {
                    return false;
                };
                return self.source_binding_is_selected(binding)
                    && self.same_source_package(
                        declaration,
                        SourceSpan::new(binding.declaration_source, declaration.span),
                    )
                    && self.source_reference_can_see_symbol(
                        SourceSpan::new(binding.reference_source, declaration.span),
                        candidate,
                    );
            }
            if !import.exact_source {
                return self
                    .symbol_provenance_source_span(candidate)
                    .is_some_and(|span| {
                        self.same_source_package(
                            SourceSpan::new(binding.reference_source, span.span),
                            span,
                        )
                    });
            }
            self.symbol_provenance_source_span(candidate)
                .is_some_and(|span| {
                    if self.get(candidate).kind == SymbolKind::Module {
                        self.same_source_package(
                            span,
                            SourceSpan::new(binding.declaration_source, span.span),
                        )
                    } else {
                        self.exact_import_exposes_source(binding.declaration_source, span)
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
                    let import = source_scope.bindings[scope.binding_position]
                        .module_import
                        .as_ref()
                        .expect("import bindings carry module imports");
                    let logical = scope.logical.as_str();
                    if import.exact_source
                        && import.package_prefix_members != 0
                        && let Some(suffix) = name
                            .strip_prefix(scope.package_prefix.as_str())
                            .and_then(|rest| rest.strip_prefix("::"))
                        && let Some(entries) = index.by_path.get(suffix)
                    {
                        candidate_pool.extend_from_slice(entries);
                    }
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
                if qualified
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
                    let import = binding
                        .module_import
                        .as_ref()
                        .expect("import bindings carry module imports");
                    let logical = scope.logical.as_str();
                    if qualified
                        && import.exact_source
                        && import.package_prefix_members != 0
                        && self
                            .symbol_provenance_source_span(candidate)
                            .is_some_and(|span| {
                                self.same_source_package(
                                    span,
                                    SourceSpan::new(binding.declaration_source, span.span),
                                )
                            })
                        && name
                            .strip_prefix(scope.package_prefix.as_str())
                            .and_then(|suffix| suffix.strip_prefix("::"))
                            == candidate_path.as_deref()
                    {
                        return true;
                    }
                    if !self.import_owner_matches(binding, candidate) {
                        return false;
                    }
                    if !qualified {
                        return is_module
                            && candidate_path.as_deref() == Some(logical)
                            && self.name(candidate) == name;
                    }
                    if candidate_path.as_deref() == Some(name) {
                        if !import.logical_selection {
                            return true;
                        }
                        // A carrier spelling or raw module path does not supply
                        // the dependency qualifier omitted by this occurrence.
                        if candidate_kind == SymbolKind::Domain
                            && !self.symbol_module(candidate).is_valid()
                        {
                            return self
                                .carrier_qualified_domain_reaches(candidate, name, reference);
                        }
                        return self.binding_exposes_member(binding, candidate)
                            || self
                                .logical_import_targets(binding)
                                .into_iter()
                                .any(|target| {
                                    self.get(target).kind == SymbolKind::Module
                                        && name
                                            .strip_prefix(logical)
                                            .is_some_and(|suffix| suffix.starts_with("::"))
                                });
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
                    || (direct_path && same_package)
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
            self.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(separator)
        }
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

    fn logical_import_path(import: &ModuleImport) -> String {
        import
            .path
            .split("::")
            .skip(import.package_prefix_members)
            .collect::<Vec<_>>()
            .join("::")
    }

    fn unique(candidates: impl Iterator<Item = SymbolHandle>) -> Option<SymbolHandle> {
        SymbolLookup::from_candidates(candidates).unique()
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
            symbol: crate::SymbolHandle,
            scope: source::DependencyScope,
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

        /*
            The retained authored source map, shared without copying. Consumers
            that resolve diagnostic locations on a path where the program moves
            away clone this handle rather than reading storage back.
        */
        pub fn source_map(&self) -> Option<std::sync::Arc<source::SourceMap>> {
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
            if !symbol.is_valid() {
                return None;
            }
            let file = self.source_file(self.symbol_source_span(symbol)?)?;
            crate::sealed_intrinsic_at(file, &self.display_path(symbol, "::"))
        }

        /*
            The bundled core source `symbol` is declared in, if any. Callers
            pair this with their own exact identity check.
        */
        pub fn core_source(
            &self,
            symbol: SymbolHandle,
        ) -> Option<language_core::core_vocabulary::CoreSource> {
            if !symbol.is_valid() {
                return None;
            }
            crate::core_source_at(self.source_file(self.symbol_source_span(symbol)?)?)
        }

        /*
            The core vocabulary row `symbol` is declared by, if any. This is
            recognition only; it never implies compiler custody.
        */
        pub fn core_vocabulary(
            &self,
            symbol: SymbolHandle,
        ) -> Option<language_core::core_vocabulary::CoreVocabulary> {
            if !symbol.is_valid() {
                return None;
            }
            let file = self.source_file(self.symbol_source_span(symbol)?)?;
            crate::core_vocabulary_at(file, &self.display_path(symbol, "::"))
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
            if !symbol.is_valid() {
                return None;
            }
            let file = self.source_file(self.symbol_source_span(symbol)?)?;
            crate::sealed_intrinsic_at(file, declaration)
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
            let file = self.source_file(self.symbol_source_span(owner)?)?;
            let declaration = format!("{}::{member}", self.display_path(owner, "::"));
            crate::sealed_intrinsic_at(file, &declaration)
        }

        pub fn is_build_prelude_symbol(&self, symbol: SymbolHandle) -> bool {
            self.symbol_source_span(symbol)
                .and_then(|span| self.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
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

        /*
            Exact completed-Build scope, authenticated by package output custody.
            Candidates are Product declarations only; this grants no ordinary host
            visibility and does not make a dependency's private members public.
        */
        pub fn find_captured_product_declaration(
            &self,
            path: &str,
            package: PackageKeyIdentity,
            stratum: source::SourceResolutionStratum,
            candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
        ) -> Option<SymbolHandle> {
            let sources = self.sources.as_deref()?;
            let qualified = path.split_once("::");
            let dependency =
                qualified.and_then(|(alias, _)| sources.product_dependency_target(package, alias));
            let target = dependency.unwrap_or(package);
            let path = match (qualified, dependency) {
                (Some((_, remaining)), Some(_)) => remaining,
                _ => path,
            };
            let mut selected = None;
            let mut selected_exact = false;
            let mut ambiguous = false;
            for (symbol, public) in candidates {
                if !self.symbol_source_is_selected(symbol)
                    || self.symbol_product_package_identity(symbol) != Some(target)
                    || dependency.is_some() && !public
                {
                    continue;
                }
                let file = self
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| self.source_file(span))?;
                if !source::SourceMap::captured_reference_can_see_stratum(package, stratum, file) {
                    continue;
                }
                let exact = self.display_path(symbol, "::") == path;
                if !exact && (dependency.is_some() || self.name(symbol) != path) {
                    continue;
                }
                if selected.is_none() || exact && !selected_exact {
                    selected = Some(symbol);
                    selected_exact = exact;
                    ambiguous = false;
                } else if exact == selected_exact && selected != Some(symbol) {
                    ambiguous = true;
                }
            }
            if ambiguous { None } else { selected }
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
                if !symbol.is_valid()
                    || !self.symbol_source_is_selected(symbol)
                    || !match dependency {
                        Some(package) => {
                            is_public
                                && self.symbol_product_package_identity(symbol) == Some(package)
                        }
                        None => {
                            self.symbol_provenance_source_span(symbol)
                                .is_some_and(|declaration| {
                                    self.same_product_package_instance(occurrence, declaration)
                                })
                                || (admit_toolchain
                                    && is_public
                                    && self
                                        .symbol_provenance_source_span(symbol)
                                        .and_then(|span| self.source_file(span))
                                        .is_some_and(|file| {
                                            file.origin == SourceOrigin::Toolchain
                                                && file.dependency_scope
                                                    == source::DependencyScope::Product
                                        }))
                                || !self.has_source_metadata()
                        }
                    }
                {
                    continue;
                }
                if !self.source_reference_can_describe_product_symbol(occurrence, symbol) {
                    continue;
                }
                let exact = self.display_path(symbol, "::") == declaration_path;
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
            sources.same_package(reference, declaration)
                && sources
                    .file_at(declaration)
                    .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
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
                Some(SourceOrigin::User | SourceOrigin::ToolchainContract) => false,
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
            (source_file.dependency_scope == source::DependencyScope::Product)
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

    #[cfg(test)]
    mod builtin_function_identity_tests {
        use super::{BuiltinFunction, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
        use crate::{builtin_function_symbols, builtin_type_symbols};

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
                    .chain(crate::fixed_core_declaration_symbols())
                    .chain([authored]),
            );
            for declaration in crate::FixedCoreDeclaration::ALL {
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
        use source::Span;

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
    SymbolHandle, SymbolKind, SymbolName, SymbolNameRef, SymbolPath,
};
use child_batches::{SupplementalChildHandles, SupplementalChildren};

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct SymbolTable {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    path_members: Arena<SymbolHandle>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
    supplemental_children: SupplementalChildren,
    module_symbols: Arena<SymbolHandle>,
    /*
        Dense `SourceId`-indexed map from a source to its declared module:
        a linear scan per lookup would cost `O(#sources)` on every
        module-qualified name walk.
    */
    source_module_index: Vec<SymbolHandle>,
    /*
        Declaration membership at the last activation frontier. Later allocations
        belong to the new cohort. Selecting a source again does not resurrect
        an older declaration that already left its predecessor's view.
    */
    inherited_symbols: Vec<bool>,
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
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
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
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceScopedTopLevelBinding {
    reference_source: SourceId,
    declaration_source: SourceId,
    name: Arc<str>,
    module_import: Option<modules::ModuleImport>,
    producer: BindingProducer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
enum BindingProducer {
    Loader,
    AuthoredImport,
}

impl SourceScopedTopLevelBinding {
    pub fn new(
        reference_source: SourceId,
        declaration_source: SourceId,
        name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            reference_source,
            declaration_source,
            name: name.into(),
            module_import: None,
            producer: BindingProducer::Loader,
        }
    }

    /* The source whose top-level references this binding scopes. */
    pub const fn reference_source(&self) -> SourceId {
        self.reference_source
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
            path_members: Arena::new(),
            sources: self.sources,
            source_scoped_top_level_bindings: self.source_scoped_top_level_bindings,
            root: self.root,
            supplemental_children: SupplementalChildren::default(),
            module_symbols: Arena::new(),
            source_module_index: Vec::new(),
            inherited_symbols: Vec::new(),
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
    names: &mut Arena<SymbolName>,
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
        name: names.insert(SymbolName::from_ref_with_sources(name, sources)),
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
            path_members,
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
            || path_members.len() < base.path_members.len()
            || !path_members
                .iter()
                .take(base.path_members.len())
                .eq(base.path_members.iter())
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
        path_members
            .iter()
            .skip(base.path_members.len())
            .all(|(_, symbol)| {
                symbol.is_valid() && symbols.get(*symbol).kind != SymbolKind::Unknown
            })
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
            let name = self.names.insert(SymbolName::from_ref_with_sources(
                name,
                self.sources.as_deref(),
            ));
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
        self.source_scoped_top_level_bindings
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
            for (position, module) in self.source_module_index.iter_mut().enumerate() {
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
                binding.producer == BindingProducer::AuthoredImport
                    && sources.source_is_selected(binding.reference_source)
            })
            .cloned()
            .collect::<Vec<_>>();
        self.inherited_symbols = inherited;
        self.sources = Some(sources);
        self.source_scoped_top_level_bindings = source_scoped_top_level_bindings;
        self.binding_source_index.clear();
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        for binding in authored_imports {
            if let Some(import) = &binding.module_import {
                self.register_source_import(binding.reference_source, &import.path);
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

        self.names.get(symbol.name).as_str(self.sources.as_deref())
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
        Root-level children whose full name is exactly `name`, in the order
        `child_handles(root())` yields them, probed through the root name
        index rather than a scan of the whole root roster.
    */
    pub fn root_children_named(&self, name: &str) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.root_names
            .0
            .get_or_init(|| self.root_name_index())
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|&(_, symbol)| symbol)
            .filter(|symbol| self.symbol_source_is_selected(*symbol))
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
        let index = self.root_names.0.get_or_init(|| self.root_name_index());
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

    fn root_name_index(&self) -> RootNameIndex {
        let mut index = RootNameIndex::default();
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
            }
        }
        index
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
            .find(|binding| binding.module_import.is_none() && binding.name.as_ref() == name)
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
                            !sources.resolution_strata_separate(reference, declaration)
                                && sources.same_checked_instance(reference, declaration)
                        })
                    })
                })
            })
            .or_else(|| {
                self.sources.as_deref().and_then(|sources| {
                    candidates.iter().copied().find(|symbol| {
                        self.symbol_source_span(*symbol).is_some_and(|declaration| {
                            !sources.resolution_strata_separate(reference, declaration)
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
            .find(|binding| binding.module_import.is_none() && binding.name.as_ref() == name)?;
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
            return !(self.import_exposes_symbol(left_span.source_id, right)
                || self.import_exposes_symbol(right_span.source_id, left));
        }
        if self.source_module(left_span.source_id) != self.source_module(right_span.source_id) {
            return true;
        }
        // Files that declare no module form one logical module, yet each
        // file's own declarations take precedence over a sibling's
        // same-spelled one (`select_namespace_candidates`), so two such files
        // each select their own declaration. Files of a shared activation
        // extension see each other's declarations as local and stay one scope.
        if !self.source_module(left_span.source_id).is_valid()
            && !self.shares_activation_extension(left_span)
            && !self.shares_activation_extension(right_span)
        {
            return true;
        }
        if self
            .sources
            .as_deref()
            .is_some_and(|sources| sources.resolution_strata_separate(left_span, right_span))
        {
            return true;
        }
        let name = self.name(left);
        if name != self.name(right) {
            return false;
        }
        self.source_scoped_top_level_bindings.iter().any(|binding| {
            self.source_binding_is_selected(binding)
                && binding.module_import.is_none()
                && binding.name.as_ref() == name
                && (binding.declaration_source == left_span.source_id
                    || binding.declaration_source == right_span.source_id)
        })
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
                    SourceSpan::new(source, source::Span::default()),
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
        in the same logical module of its checked package instance, or in a
        source the reference's file imported. A declaration is never a candidate
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
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        declaration.source_id == reference.source_id
            || (sources.same_checked_instance(reference, declaration)
                && self.source_module(reference.source_id)
                    == self.source_module(declaration.source_id))
            || self
                .source_scoped_bindings_for(reference.source_id)
                .iter()
                .any(|binding| {
                    if binding.module_import.is_some() {
                        self.binding_exposes_member(binding, symbol)
                    } else {
                        binding.declaration_source == declaration.source_id
                    }
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
        self.root_names
            .0
            .get_or_init(|| self.root_name_index())
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
        self.root_names
            .0
            .get_or_init(|| self.root_name_index())
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
            let mut paths = crate::SymbolKeyMap::default();
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

    pub fn path_from_members(
        &mut self,
        root: SymbolHandle,
        members: impl IntoIterator<Item = SymbolHandle>,
    ) -> SymbolPath {
        SymbolPath {
            root,
            members: self.path_members.insert_many(members),
        }
    }

    pub fn resolve_child_path<'name>(
        &mut self,
        root: SymbolHandle,
        names: impl IntoIterator<Item = &'name str>,
    ) -> SymbolPath {
        let mut current = root;
        let mut members = Vec::new();

        if !root.is_valid() {
            return SymbolPath::default();
        }

        for name in names {
            let Some(child) = self.find_child_by_name(current, name) else {
                return SymbolPath::default();
            };

            members.push(child);
            current = child;
        }

        if members.is_empty() {
            SymbolPath::default()
        } else {
            self.path_from_members(root, members)
        }
    }

    pub fn path_members(&self, path: SymbolPath) -> &[SymbolHandle] {
        self.path_members.span_or_empty(path.members)
    }

    pub fn symbols(&self) -> &HierarchyArena<Symbol> {
        &self.symbols
    }

    pub fn names(&self) -> &Arena<SymbolName> {
        &self.names
    }

    pub fn path_member_arena(&self) -> &Arena<SymbolHandle> {
        &self.path_members
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
