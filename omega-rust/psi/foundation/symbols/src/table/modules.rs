//! Logical module ownership without relocating physical declaration slots.
//!
//! Local declarations precede imported and root names, including relative
//! attached paths. A narrow import may expose an attached declaration's leaf;
//! retaining that candidate does not expose sibling declarations or make its
//! leaf a bare local name. Every candidate still needs its exact source owner.
//! A loader-certified package alias can qualify other already-loaded sources
//! of that same dependency; that lookup does not change which exact source
//! must satisfy an authored import or expose additional unqualified names.

use std::sync::Arc;

use arena::HandleSpan;
use source::{SourceId, SourceSpan};

use super::{SourceScopedTopLevelBinding, SymbolLookup, SymbolTable};
use crate::{Symbol, SymbolHandle, SymbolKind, SymbolName, SymbolNameRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleImport {
    path: Arc<str>,
    package_prefix_members: usize,
    exact_source: bool,
}

/// One module-import binding with the strings the candidate loop needs
/// precomputed once per lookup instead of per (candidate, binding) pair.
struct ImportBindingScope<'a> {
    binding: &'a SourceScopedTopLevelBinding,
    /// `logical_import_path` of the binding's import.
    logical: String,
    /// `import.path` members up to `package_prefix_members`, joined.
    package_prefix: String,
    /// `name(module)` when the binding's declaration source declares a
    /// module whose display path is exactly `logical`.
    module_leaf: Option<String>,
}

impl SourceScopedTopLevelBinding {
    /// Bind one authored import to the exact source loaded through the
    /// requester's reconciled package graph. Only the loader may strip the
    /// certified package-prefix members from the logical module path.
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
            }),
        }
    }
}

impl SymbolTable {
    /// Authored import spellings retained for source lookup diagnostics.
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

    /// Publish the logical namespace of one source without relocating any
    /// existing declaration or builtin slot in the symbol hierarchy.
    pub fn register_source_module<'name>(
        &mut self,
        source: SourceId,
        members: impl IntoIterator<Item = (&'name str, SourceSpan)>,
    ) -> Result<(), &'static str> {
        if self.source_module(source).is_valid() {
            return Err("a source file may declare only one module");
        }
        self.root_names.clear();
        self.module_path_index.clear();
        let mut parent = self.root;
        for (name, span) in members {
            let existing = self
                .module_symbols
                .iter()
                .map(|(_, handle)| *handle)
                .find(|handle| {
                    self.get(*handle).parent == parent
                        && self.name(*handle) == name
                        && self
                            .symbol_provenance_source_span(*handle)
                            .is_some_and(|previous| self.same_source_package(previous, span))
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

    /// Retain an import for syntax-only callers. Compiler-loaded imports
    /// already carry exact source custody and take precedence over this row.
    pub fn register_source_import(&mut self, reference_source: SourceId, path: &str) {
        if self.source_scoped_top_level_bindings.iter().any(|binding| {
            binding.reference_source == reference_source
                && binding
                    .module_import
                    .as_ref()
                    .is_some_and(|import| import.path.as_ref() == path)
        }) {
            return;
        }
        let mut binding =
            SourceScopedTopLevelBinding::module_import(reference_source, reference_source, path, 0);
        if let Some(import) = &mut binding.module_import {
            import.exact_source = false;
        }
        self.source_scoped_top_level_bindings.push(binding);
        self.binding_source_index.clear();
    }

    /// An imported source with an authored module must supply the exact
    /// logical namespace or declaration requested by the import.
    pub fn validate_source_module_import(
        &self,
        reference_source: SourceId,
        path: &str,
    ) -> Result<(), String> {
        for binding in self.source_scoped_bindings_for(reference_source) {
            let Some(import) = &binding.module_import else {
                continue;
            };
            if import.path.as_ref() != path || !import.exact_source {
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
        Ok(())
    }

    /// Return the declaration selected by a validated module import. Legacy
    /// file imports have no independently selected namespace declaration.
    pub fn source_module_import_target(
        &self,
        reference_source: SourceId,
        path: &str,
    ) -> Option<SymbolHandle> {
        for binding in self.source_scoped_bindings_for(reference_source) {
            let Some(import) = &binding.module_import else {
                continue;
            };
            if import.path.as_ref() != path {
                continue;
            }
            if import.exact_source && !self.source_module(binding.declaration_source).is_valid() {
                return None;
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

    /// Whether an authored domain reference may select `domain` from
    /// `reference` under module name law. A fully qualified spelling always
    /// selects its exact declaration. A relative spelling reaches a
    /// module-owned domain inside its own module or through an exposing
    /// import: a narrow import of the exact declaration exposes its leaf and
    /// its declared carrier-qualified spelling, while importing the declaring
    /// module exposes its directly declared domains to the carrier-qualified
    /// spelling. The carrier-qualified form additionally requires the
    /// authored carrier to resolve to the exact carrier the declaring context
    /// attached the domain to -- importing the carrier type, an ordinary
    /// sibling declaration, or merely loading another source exposes nothing.
    /// Unmoduled domains keep root scope; generated references carry no
    /// lexical module and select only qualified or unmoduled domains.
    /// Resolution-stratum and package visibility stay with the caller.
    pub fn domain_name_reaches(
        &self,
        domain: SymbolHandle,
        authored: &str,
        reference: SourceSpan,
    ) -> bool {
        let qualified = self.indexed_display_path(domain);
        if qualified.as_ref() == authored {
            return true;
        }
        let domain_name = self.name(domain);
        let domain_module = self.symbol_module(domain);
        // Generic families have leaf-only declaration names. A qualified
        // reference may abbreviate their carrier, but a module prefix is not
        // a carrier: `bounds::Below` must not also select a root `Below`.
        // Keep ordinary role-aware lookup here so all domain candidate pools
        // apply the same rule before deciding ambiguity or local precedence.
        if !domain_name.contains("::")
            && let Some((carrier, leaf)) = authored.rsplit_once("::")
            && (leaf != domain_name
                || self
                    .find_top_level_by_name_and_kinds_from_source(
                        carrier,
                        &[
                            SymbolKind::BuiltinType,
                            SymbolKind::Data,
                            SymbolKind::Machine,
                            SymbolKind::Trait,
                        ],
                        reference,
                    )
                    .is_none())
        {
            return false;
        }
        if !domain_module.is_valid() {
            return same_semantic_name(domain_name, authored);
        }
        // A generated or source-free reference carries no lexical module; it
        // can only select the domain through its complete qualified path
        // above.
        if reference.span.start == reference.span.end {
            return false;
        }
        if self.source_module(reference.source_id) == domain_module {
            return same_semantic_name(domain_name, authored);
        }
        if !authored.contains("::") {
            // A narrow import of the exact declaration exposes its leaf
            // spelling, just like ordinary name resolution. A module import
            // does not turn the leaf into a bare local name.
            return same_semantic_name(domain_name, authored)
                && self
                    .source_module_import_paths(reference.source_id)
                    .any(|path| {
                        self.source_module_import_target(reference.source_id, path) == Some(domain)
                    });
        }
        self.carrier_qualified_domain_reaches(domain, authored, reference)
    }

    /// Whether a carrier-qualified spelling (`Carrier::Leaf`) selects an
    /// exposed module-owned domain. The authored carrier must resolve to the
    /// same exact carrier the declaring source attached the domain to -- a
    /// caller's same-spelled type cannot redirect the attachment -- and an
    /// import must expose the domain: either a narrow import of the exact
    /// declaration or a broad import of its declaring module, which exposes
    /// only the domains that module directly declares.
    pub fn carrier_qualified_domain_reaches(
        &self,
        domain: SymbolHandle,
        authored: &str,
        reference: SourceSpan,
    ) -> bool {
        let domain_module = self.symbol_module(domain);
        if !domain_module.is_valid() || reference.span.start == reference.span.end {
            return false;
        }
        let Some((declared_carrier, declared_leaf)) = self.name(domain).rsplit_once("::") else {
            // A leaf-named declaration (a generic domain family) has no
            // carrier-qualified spelling.
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
        let declared_carrier_symbol = self.symbol_provenance_source_span(domain).and_then(|span| {
            self.find_top_level_by_name_and_kinds_from_source(
                declared_carrier,
                &CARRIER_KINDS,
                span,
            )
        });
        if authored_carrier_symbol != declared_carrier_symbol.unwrap_or_else(SymbolHandle::invalid)
        {
            return false;
        }
        self.source_module_import_paths(reference.source_id)
            .any(|path| {
                matches!(
                    self.source_module_import_target(reference.source_id, path),
                    Some(target) if target == domain || target == domain_module
                )
            })
    }

    /// Whether one authored import edge in `reference` brings `symbol`'s
    /// leaf name into that source's resolution scope. Importing an
    /// unmoduled source exposes every leaf it declares; a moduled source
    /// exposes a leaf only through a narrow import of that exact
    /// declaration, so a broad `module::name` edge never collides with a
    /// package-local `name`.
    pub(super) fn import_exposes_symbol(&self, reference: SourceId, symbol: SymbolHandle) -> bool {
        let Some(declaration) = self.symbol_source_span(symbol) else {
            return false;
        };
        self.source_scoped_bindings_for(reference)
            .iter()
            .any(|binding| {
                let Some(import) = &binding.module_import else {
                    return false;
                };
                binding.declaration_source == declaration.source_id
                    && ((import.exact_source
                        && !self.source_module(declaration.source_id).is_valid())
                        || self.indexed_display_path(symbol) == logical_import_path(import))
            })
    }

    pub fn symbol_module(&self, symbol: SymbolHandle) -> SymbolHandle {
        self.symbol_provenance_source_span(symbol)
            .map(|span| self.source_module(span.source_id))
            .unwrap_or_else(SymbolHandle::invalid)
    }

    /// Semantic path ownership includes the authored module while physical
    /// declaration storage preserves its original root child range.
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
            .collect()
    }

    pub(super) fn has_namespace_context(&self, reference: SourceSpan) -> bool {
        !self.module_symbols.is_empty()
            || self.source_scoped_top_level_bindings.iter().any(|binding| {
                binding.reference_source == reference.source_id
                    && binding
                        .module_import
                        .as_ref()
                        .is_some_and(|import| import.exact_source)
            })
    }

    pub(super) fn select_namespace_candidate(
        &self,
        candidates: &[SymbolHandle],
        name: &str,
        reference: SourceSpan,
    ) -> SymbolLookup {
        let current_module = self.source_module(reference.source_id);
        let local = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                self.name(*candidate) == name
                    && self.symbol_module(*candidate) == current_module
                    && self
                        .symbol_provenance_source_span(*candidate)
                        .is_some_and(|span| self.same_source_package(reference, span))
            })
            .collect::<Vec<_>>();
        if !local.is_empty() {
            return SymbolLookup::from_candidates(local.into_iter());
        }
        let import_scopes: Vec<(&SourceScopedTopLevelBinding, String)> = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .filter(|binding| binding.module_import.is_some())
            .map(|binding| {
                (
                    binding,
                    logical_import_path(
                        binding
                            .module_import
                            .as_ref()
                            .expect("import bindings carry module imports"),
                    ),
                )
            })
            .collect();
        let imported = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                import_scopes.iter().any(|(binding, logical)| {
                    let import = binding
                        .module_import
                        .as_ref()
                        .expect("import bindings carry module imports");
                    if !self.import_owner_matches(binding, *candidate) {
                        return false;
                    }
                    self.indexed_display_path(*candidate) == logical.as_str()
                        || (import.exact_source
                            && self.name(*candidate) == name
                            && !self.source_module(binding.declaration_source).is_valid()
                            && self
                                .symbol_provenance_source_span(*candidate)
                                .is_some_and(|span| span.source_id == binding.declaration_source))
                })
            })
            .collect::<Vec<_>>();
        if !imported.is_empty() {
            return SymbolLookup::from_candidates(imported.into_iter());
        }
        // In a module-less closure this selector is reached only because an
        // exact loader import supplied namespace custody. A missing imported
        // name must not fall back to the other checked scope. Preserve ordinary
        // unmoduled discovery within the scope: package admission still checks
        // whether a selected declaration is a direct dependency. Toolchain
        // vocabulary and declared-module fallback retain their existing rules.
        let exact_file_context = self.module_symbols.is_empty();
        SymbolLookup::from_candidates(candidates.iter().copied().filter(|candidate| {
            self.name(*candidate) == name
                && !self.symbol_module(*candidate).is_valid()
                && (!exact_file_context
                    || self.symbol_source_origin(*candidate) != Some(source::SourceOrigin::User)
                    || self
                        .symbol_provenance_source_span(*candidate)
                        .and_then(|span| self.source_file(span))
                        .zip(self.source_file(reference))
                        .is_none_or(|(declaration, reference)| {
                            declaration.dependency_scope == reference.dependency_scope
                        }))
        }))
    }

    fn import_owner_matches(
        &self,
        binding: &SourceScopedTopLevelBinding,
        candidate: SymbolHandle,
    ) -> bool {
        let Some(import) = &binding.module_import else {
            return false;
        };
        if !import.exact_source {
            return true;
        }
        self.symbol_provenance_source_span(candidate)
            .is_some_and(|span| {
                if self.get(candidate).kind == SymbolKind::Module {
                    self.same_source_package(
                        span,
                        SourceSpan::new(binding.declaration_source, span.span),
                    )
                } else {
                    span.source_id == binding.declaration_source
                }
            })
    }

    /// `Some(None)` is an authoritative failed or ambiguous logical lookup;
    /// `None` leaves an ordinary unqualified legacy lookup to its existing path.
    pub(super) fn find_module_qualified_reference(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: &mut impl FnMut(SymbolHandle) -> bool,
    ) -> Option<SymbolLookup> {
        if self.module_symbols.is_empty() {
            return None;
        }
        let qualified = name.contains("::");
        // An unqualified reference can only resolve here through a top-level
        // module of the same name or a module import; both require Module in
        // `kinds`. Type-only lookups provably find nothing — returning early
        // skips the whole candidate walk and the import-binding filter.
        if !qualified && !kinds.contains(&SymbolKind::Module) {
            return None;
        }
        // `display_path` walks the candidate's parent chain and allocates, so
        // it is built only when a consult below can engage: qualified
        // spellings, or a module-import binding for this source that reaches
        // the imported-path arms. Bare-name lookups against unimported scopes
        // skip the walk entirely.
        let current_module = self.source_module(reference.source_id);
        // The imported-path arm only ever consults this source's module-import
        // bindings; filter once here instead of walking the whole binding list
        // for every candidate. Each binding also carries its precomputed
        // logical import path and the string fragments the candidate loop
        // would otherwise re-derive per (candidate, binding) pair.
        let import_bindings: Vec<ImportBindingScope<'_>> = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .filter(|binding| binding.module_import.is_some())
            .map(|binding| {
                let import = binding
                    .module_import
                    .as_ref()
                    .expect("import bindings carry module imports");
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
                ImportBindingScope {
                    binding,
                    logical,
                    package_prefix,
                    module_leaf,
                }
            })
            .collect();
        let needs_candidate_path = qualified || !import_bindings.is_empty();
        let mut matches = Vec::new();
        let mut module_local_matches = Vec::new();
        let mut carrier_domain_matches = Vec::new();
        // Every match arm selects by full name, exact display path, a path
        // derived from an import binding's own strings, or (for the carrier
        // arm) the declared Domain leaf — gather just those roster positions
        // instead of walking the whole roster per reference. Order is the
        // original scan order; a handle occupying both roster halves keeps
        // its two positions.
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
            for scope in &import_bindings {
                let import = scope
                    .binding
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
            let candidate_path = needs_candidate_path.then(|| self.indexed_display_path(candidate));
            // A relative attached path first belongs to the current module,
            // just like a bare declaration name. A root legacy spelling must
            // not compete with that local owner; competing local declarations
            // still reject together. Absolute/imported paths use the ordinary
            // candidate set below.
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
            let legacy_path = qualified
                && !is_module
                && !self.symbol_module(candidate).is_valid()
                && self.name(candidate) == name;
            let imported_path = import_bindings.iter().any(|scope| {
                let binding = scope.binding;
                let import = binding
                    .module_import
                    .as_ref()
                    .expect("import bindings carry module imports");
                let logical = scope.logical.as_str();
                // The loader certifies the requester's package prefix against
                // this imported source. A full alias-qualified path may name
                // another loaded source of that exact package, while ordinary
                // import exposure below still requires the exact loaded file.
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
                {
                    if name
                        .strip_prefix(scope.package_prefix.as_str())
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        == candidate_path.as_deref()
                    {
                        return true;
                    }
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
            // A domain's declared carrier-qualified spelling
            // (`u64::Distance` authored for `units::u64::Distance`) is not a
            // lexical module path; it reaches through the domain exposure
            // law instead -- a narrow import of the exact declaration or a
            // broad import of its declaring module, with the authored carrier
            // resolving to the exact attached carrier. These candidates wait
            // outside `matches` so a contested spelling keeps the ordinary
            // not-found result and pooled callers retain their own ambiguity
            // and checked-obligation behavior.
            if qualified
                && self.get(candidate).kind == SymbolKind::Domain
                && self.carrier_qualified_domain_reaches(candidate, name, reference)
            {
                carrier_domain_matches.push(candidate);
            }
            if legacy_path
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
            Some(SymbolLookup::from_candidates(
                module_local_matches.into_iter(),
            ))
        } else if qualified {
            if matches.is_empty() && carrier_domain_matches.len() == 1 {
                return Some(SymbolLookup::Unique(carrier_domain_matches[0]));
            }
            Some(SymbolLookup::from_candidates(matches.into_iter()))
        } else if matches.is_empty() {
            None
        } else {
            Some(SymbolLookup::from_candidates(matches.into_iter()))
        }
    }
}

/// A relative domain spelling matches its declared name either exactly or by
/// leaf, mirroring the semantic-name rule the resolved-lookup exposure pool
/// shares with this table law.
fn same_semantic_name(left: &str, right: &str) -> bool {
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
