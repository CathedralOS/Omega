use std::sync::Arc;

use psi_arena::{Arena, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles};
use psi_core::PackageKeyIdentity;
use psi_source::{SourceFile, SourceId, SourceMap, SourceOrigin, SourceSpan};

use super::builtin::BUILTIN_TYPE_COUNT;
use super::{
    BuiltinFunction, BuiltinType, BuiltinTypeAtom, Symbol, SymbolHandle, SymbolKind, SymbolName,
    SymbolNameRef, SymbolNameStorageKind, SymbolPath,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    path_members: Arena<SymbolHandle>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTableBuilder {
    symbols: HierarchyArenaBuilder<Symbol>,
    names: Arena<SymbolName>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
}

/// One compiler-owned top-level vocabulary binding for an exact source.
///
/// The authored spelling remains unchanged. Only references originating in
/// `reference_source` select the same-spelled declaration authored in
/// `declaration_source`; every other source retains ordinary lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceScopedTopLevelBinding {
    reference_source: SourceId,
    declaration_source: SourceId,
    name: Arc<str>,
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
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolNameStorageCounts {
    pub missing: usize,
    pub source_names: usize,
    pub static_names: usize,
    pub owned_names: usize,
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
        let names = &mut self.names;
        self.symbols.insert_children(
            parent,
            children.into_iter().map(|(kind, name)| Symbol {
                parent,
                children: HandleSpan::empty(),
                kind,
                name: names.insert(SymbolName::from_ref(name)),
                generated_from: SymbolHandle::invalid(),
            }),
        )
    }

    pub fn child_handles(span: HandleSpan<Symbol>) -> impl Iterator<Item = SymbolHandle> {
        let start = span.start();

        (0..span.count()).map(move |offset| {
            SymbolHandle::from_parts(
                start
                    .arena_index()
                    .checked_add(offset)
                    .expect("symbol child handle overflow"),
                start.generation(),
            )
        })
    }

    pub fn finish(self) -> SymbolTable {
        SymbolTable {
            symbols: self.symbols.finish(),
            names: self.names,
            path_members: Arena::new(),
            sources: self.sources,
            source_scoped_top_level_bindings: self.source_scoped_top_level_bindings,
            root: self.root,
        }
    }

    fn symbol_from_parts(
        &mut self,
        parent: SymbolHandle,
        kind: SymbolKind,
        name: SymbolNameRef<'_>,
    ) -> Symbol {
        Symbol {
            parent,
            children: HandleSpan::empty(),
            kind,
            name: self.names.insert(SymbolName::from_ref(name)),
            generated_from: SymbolHandle::invalid(),
        }
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    /// Mint a compiler-generated root declaration after symbol resolution.
    /// The generated name is owned by the table and authored symbol handles
    /// remain unchanged. Children must be installed once, as one batch, via
    /// [`Self::insert_generated_children`].
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
        let name = self
            .names
            .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name)));
        self.symbols.insert_generated_root(Symbol {
            parent: SymbolHandle::invalid(),
            children: HandleSpan::empty(),
            kind,
            name,
            generated_from,
        })
    }

    /// Mint the complete child range of a freshly generated symbol. Keeping
    /// the batch contiguous preserves the hierarchy arena's compact child-span
    /// representation without allowing late mutation of authored parents.
    pub fn insert_generated_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        let names = &mut self.names;
        self.symbols.insert_generated_children(
            parent,
            children.into_iter().map(|(kind, name)| Symbol {
                parent,
                children: HandleSpan::empty(),
                kind,
                name: names.insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
                generated_from: parent,
            }),
        )
    }

    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.names.get(symbol.name).as_str(self.sources.as_deref())
    }

    pub fn source_text(&self, source_span: SourceSpan) -> &str {
        self.sources
            .as_deref()
            .map(|sources| sources.text_at(source_span))
            .unwrap_or("")
    }

    /// Source declaration span retained for an authored symbol.
    pub fn symbol_source_span(&self, symbol: SymbolHandle) -> Option<SourceSpan> {
        self.names.get(self.get(symbol).name).source_span()
    }

    /// Exact authored provenance span for an authored or compiler-generated
    /// symbol. Generated symbols follow their mandatory derivation origin.
    pub fn symbol_provenance_source_span(&self, symbol: SymbolHandle) -> Option<SourceSpan> {
        self.provenance_source_span(symbol)
    }

    /// File metadata for one retained authored span. Generated/source-free
    /// trees return `None` instead of inventing a presentation path.
    pub fn source_file(&self, source_span: SourceSpan) -> Option<&SourceFile> {
        self.sources.as_deref()?.file_at(source_span)
    }

    /// Compare declaration provenance at the package boundary. Source-free
    /// lowering is used by focused representation tests; those trees model
    /// one package and retain the historical all-local behavior.
    pub fn same_source_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
        self.sources
            .as_deref()
            .is_none_or(|sources| sources.same_package(left, right))
    }

    /// Compare declaration provenance for authored or compiler-generated
    /// symbols. Generated symbols follow their mandatory earlier derivation
    /// origin; they never inherit a package through `SourceSpan::default()`.
    /// Source-free focused trees continue to model one package.
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
        sources.same_package(left, right)
    }

    /// Authored declaration provenance for one symbol. Compiler-generated
    /// symbols follow their mandatory derivation origin. Focused source-free
    /// trees return `None`; semantic consumers that admit them must make that
    /// fallback explicit rather than mistaking a spelling for ownership.
    pub fn symbol_source_origin(&self, symbol: SymbolHandle) -> Option<SourceOrigin> {
        let sources = self.sources.as_deref()?;
        let source_span = self.provenance_source_span(symbol)?;
        sources.file_at(source_span).map(|file| file.origin)
    }

    /// Reconciled package identity for one user-authored declaration.
    /// Generated symbols inherit their exact authored derivation origin.
    /// Source-free, toolchain, and unmanaged symbols return `None`.
    pub fn symbol_package_identity(&self, symbol: SymbolHandle) -> Option<PackageKeyIdentity> {
        let source_span = self.provenance_source_span(symbol)?;
        let source_file = self.source_file(source_span)?;

        (source_file.origin == SourceOrigin::User)
            .then_some(source_file.package_identity)
            .flatten()
    }

    pub fn has_source_metadata(&self) -> bool {
        self.sources.is_some()
    }

    /// Exact frontend source custody retained beside authored symbol spans.
    /// This exposes source-map facts, not symbol identity or a stable package
    /// format; compiler-internal consumers remain responsible for canonical
    /// framing.
    pub fn source_files(&self) -> impl Iterator<Item = &SourceFile> {
        self.sources.iter().flat_map(|sources| sources.files())
    }

    fn provenance_source_span(&self, mut symbol: SymbolHandle) -> Option<SourceSpan> {
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

    pub fn root(&self) -> SymbolHandle {
        self.root
    }

    pub fn child_handles(&self, parent: SymbolHandle) -> Option<HierarchyChildHandles<Symbol>> {
        self.symbols.child_handles(parent)
    }

    pub fn find_child_by_name(&self, parent: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        self.symbols
            .find_child(parent, |symbol, _| self.name(symbol) == name)
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

        self.symbols.find_child(parent, |symbol, symbol_data| {
            symbol_data.kind == kind && self.name(symbol) == name
        })
    }

    /// Resolve one source-backed top-level reference without turning a
    /// presentation spelling into global authority.
    pub fn find_top_level_by_name_and_kinds_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Option<SymbolHandle> {
        let children = self.child_handles(self.root)?;
        let candidates = children
            .filter(|symbol| kinds.contains(&self.get(*symbol).kind) && self.name(*symbol) == name)
            .collect::<Vec<_>>();

        let reference_is_source_backed = reference.span.start != reference.span.end;
        if !reference_is_source_backed {
            return candidates.first().copied();
        }

        if let Some(binding) = self
            .source_scoped_top_level_bindings
            .iter()
            .find(|binding| {
                binding.reference_source == reference.source_id && binding.name.as_ref() == name
            })
        {
            let mut targets = candidates.iter().copied().filter(|symbol| {
                self.symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == binding.declaration_source)
            });
            let target = targets.next()?;
            return targets.next().is_none().then_some(target);
        }

        candidates
            .iter()
            .copied()
            .find(|symbol| {
                self.symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == reference.source_id)
            })
            .or_else(|| candidates.first().copied())
    }

    /// Whether two same-spelled declarations intentionally occupy separate
    /// source-resolution contexts established by an explicit binding.
    pub fn source_scopes_separate(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
        let name = self.name(left);
        if name != self.name(right) {
            return false;
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
        self.source_scoped_top_level_bindings.iter().any(|binding| {
            binding.name.as_ref() == name
                && (binding.declaration_source == left_span.source_id
                    || binding.declaration_source == right_span.source_id)
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

    /// Classify only an exact compiler-installed root builtin-function slot.
    /// Names are deliberately irrelevant: same-spelled package declarations
    /// and generated source-free symbols are not compiler functions.
    pub fn builtin_function_for_symbol(&self, symbol: SymbolHandle) -> Option<BuiltinFunction> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::BuiltinFunction {
            return None;
        }
        self.child_handles(self.root)?
            .skip(BUILTIN_TYPE_COUNT)
            .take(BuiltinFunction::COUNT)
            .position(|candidate| candidate == symbol)
            .and_then(BuiltinFunction::from_ordinal)
    }

    pub fn builtin_type_symbol(&self, builtin_type: BuiltinType) -> Option<SymbolHandle> {
        self.child_handles(self.root)?
            .nth(builtin_type.ordinal())
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinType)
    }

    /// Classify only an exact compiler-installed root builtin slot. A package
    /// declaration with the same spelling, or a generated source-free symbol,
    /// is deliberately not a compiler atom.
    pub fn builtin_type_atom(&self, symbol: SymbolHandle) -> Option<BuiltinTypeAtom> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::BuiltinType {
            return None;
        }
        self.child_handles(self.root)?
            .take(BUILTIN_TYPE_COUNT)
            .position(|candidate| candidate == symbol)
            .and_then(BuiltinTypeAtom::from_ordinal)
    }

    pub fn display_path(&self, symbol: SymbolHandle, separator: &str) -> String {
        if !symbol.is_valid() {
            return String::new();
        }

        let mut names = Vec::new();
        let mut current = symbol;

        while current.is_valid() && current != self.root {
            names.push(self.name(current));
            current = self.get(current).parent;
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

    pub fn name_storage_counts(&self) -> SymbolNameStorageCounts {
        let mut counts = SymbolNameStorageCounts::default();

        for (_, name) in self.names.iter() {
            match name.storage_kind() {
                SymbolNameStorageKind::Missing => counts.missing += 1,
                SymbolNameStorageKind::Source => counts.source_names += 1,
                SymbolNameStorageKind::Static => counts.static_names += 1,
                SymbolNameStorageKind::Owned => counts.owned_names += 1,
            }
        }

        counts
    }

    pub fn path_member_arena(&self) -> &Arena<SymbolHandle> {
        &self.path_members
    }
}

#[cfg(test)]
mod builtin_function_identity_tests {
    use super::*;
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

        for function in BuiltinFunction::ALL {
            let symbol = symbols
                .builtin_function_symbol(function)
                .expect("compiler builtin function must occupy its fixed root slot");
            assert_eq!(symbols.builtin_function_for_symbol(symbol), Some(function));
        }
    }

    #[test]
    fn same_spelled_non_builtin_symbols_do_not_classify() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let root_children = builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain([
                    (SymbolKind::BuiltinFunction, SymbolNameRef::Static("max")),
                    (SymbolKind::Machine, SymbolNameRef::Static("user")),
                ]),
        );
        let mut root_children = SymbolTableBuilder::child_handles(root_children);
        let same_spelled_late_root = root_children
            .nth(BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT)
            .expect("late same-spelled root symbol");
        let user = root_children.next().expect("user parent");
        let same_spelled_non_root = SymbolTableBuilder::child_handles(builder.insert_children(
            user,
            [(SymbolKind::BuiltinFunction, SymbolNameRef::Static("max"))],
        ))
        .next()
        .expect("same-spelled non-root symbol");
        let symbols = builder.finish();

        assert_eq!(symbols.name(same_spelled_late_root), "max");
        assert_eq!(symbols.name(same_spelled_non_root), "max");
        assert_eq!(
            symbols.builtin_function_for_symbol(same_spelled_late_root),
            None
        );
        assert_eq!(
            symbols.builtin_function_for_symbol(same_spelled_non_root),
            None
        );
    }

    #[test]
    fn same_spelled_generated_symbol_does_not_classify() {
        let mut symbols = builtin_symbol_table(builtin_function_symbols());
        let origin = symbols
            .builtin_function_symbol(BuiltinFunction::Max)
            .expect("max builtin");
        let generated = symbols.insert_generated_root_from(
            origin,
            SymbolKind::BuiltinFunction,
            BuiltinFunction::Max.name(),
        );

        assert_eq!(symbols.name(generated), BuiltinFunction::Max.name());
        assert_eq!(symbols.builtin_function_for_symbol(generated), None);
    }

    #[test]
    fn fixed_function_slot_with_wrong_kind_does_not_classify() {
        let function_symbols = builtin_function_symbols().map(|(kind, name)| {
            if name.as_str() == BuiltinFunction::Max.name() {
                (SymbolKind::Function, name)
            } else {
                (kind, name)
            }
        });
        let symbols = builtin_symbol_table(function_symbols);
        let wrong_kind = symbols
            .child_handles(symbols.root())
            .expect("root children")
            .nth(BUILTIN_TYPE_COUNT + BuiltinFunction::Max.ordinal())
            .expect("max fixed slot");

        assert_eq!(symbols.name(wrong_kind), BuiltinFunction::Max.name());
        assert_eq!(symbols.get(wrong_kind).kind, SymbolKind::Function);
        assert_eq!(symbols.builtin_function_for_symbol(wrong_kind), None);
        assert_eq!(
            symbols.builtin_function_for_symbol(
                symbols
                    .builtin_function_symbol(BuiltinFunction::Min)
                    .expect("neighboring min builtin"),
            ),
            Some(BuiltinFunction::Min),
        );
    }
}
