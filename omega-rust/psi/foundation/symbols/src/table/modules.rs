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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SourceModule {
    source: SourceId,
    module: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleImport {
    path: Arc<str>,
    package_prefix_members: usize,
    exact_source: bool,
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
        self.source_scoped_top_level_bindings
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
        self.source_modules.insert(SourceModule {
            source,
            module: parent,
        });
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
    }

    /// An imported source with an authored module must supply the exact
    /// logical namespace or declaration requested by the import.
    pub fn validate_source_module_import(
        &self,
        reference_source: SourceId,
        path: &str,
    ) -> Result<(), String> {
        for binding in &self.source_scoped_top_level_bindings {
            let Some(import) = &binding.module_import else {
                continue;
            };
            if binding.reference_source != reference_source
                || import.path.as_ref() != path
                || !import.exact_source
            {
                continue;
            }
            let module = self.source_module(binding.declaration_source);
            if !module.is_valid() {
                continue;
            }
            let logical = logical_import_path(import);
            let namespace = self.display_path(module, "::");
            if logical == namespace {
                continue;
            }
            let declarations =
                self.child_handles(self.root)
                    .into_iter()
                    .flatten()
                    .filter(|candidate| {
                        self.import_owner_matches(binding, *candidate)
                            && self.display_path(*candidate, "::") == logical
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
        for binding in &self.source_scoped_top_level_bindings {
            let Some(import) = &binding.module_import else {
                continue;
            };
            if binding.reference_source != reference_source || import.path.as_ref() != path {
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
                            && self.display_path(*candidate, "::") == logical
                    }),
            );
        }
        None
    }

    pub fn source_module(&self, source: SourceId) -> SymbolHandle {
        self.source_modules
            .iter()
            .find(|(_, binding)| binding.source == source)
            .map(|(_, binding)| binding.module)
            .unwrap_or_else(SymbolHandle::invalid)
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

    pub(super) fn has_namespace_context(&self, _reference: SourceSpan) -> bool {
        !self.module_symbols.is_empty()
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
        let imported = candidates
            .iter()
            .copied()
            .filter(|candidate| {
                self.source_scoped_top_level_bindings.iter().any(|binding| {
                    if binding.reference_source != reference.source_id {
                        return false;
                    }
                    let Some(import) = &binding.module_import else {
                        return false;
                    };
                    if !self.import_owner_matches(binding, *candidate) {
                        return false;
                    }
                    let logical = logical_import_path(import);
                    self.display_path(*candidate, "::") == logical
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
        SymbolLookup::from_candidates(candidates.iter().copied().filter(|candidate| {
            self.name(*candidate) == name && !self.symbol_module(*candidate).is_valid()
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
        let mut matches = Vec::new();
        let mut module_local_matches = Vec::new();
        for candidate in self
            .child_handles(self.root)
            .into_iter()
            .flatten()
            .chain(self.module_symbols.iter().map(|(_, handle)| *handle))
        {
            let is_module = self.get(candidate).kind == SymbolKind::Module;
            if !kinds.contains(&self.get(candidate).kind)
                || !matches_candidate(candidate)
                || !self.source_reference_can_see_symbol(reference, candidate)
            {
                continue;
            }
            let candidate_path = self.display_path(candidate, "::");
            let current_module = self.source_module(reference.source_id);
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
                && (candidate_path == name
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
            let imported_path = self.source_scoped_top_level_bindings.iter().any(|binding| {
                if binding.reference_source != reference.source_id {
                    return false;
                }
                let Some(import) = &binding.module_import else {
                    return false;
                };
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
                    let prefix = import
                        .path
                        .split("::")
                        .take(import.package_prefix_members)
                        .collect::<Vec<_>>()
                        .join("::");
                    if name
                        .strip_prefix(&prefix)
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        == Some(candidate_path.as_str())
                    {
                        return true;
                    }
                }
                if !self.import_owner_matches(binding, candidate) {
                    return false;
                }
                let logical = logical_import_path(import);
                if !qualified {
                    return is_module && candidate_path == logical && self.name(candidate) == name;
                }
                if candidate_path == name {
                    return true;
                }
                if let Some(short) = logical.rsplit("::").next()
                    && name
                        .strip_prefix(short)
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        .is_some_and(|suffix| candidate_path == format!("{logical}::{suffix}"))
                {
                    return true;
                }
                if is_module {
                    return false;
                }
                let module = self.source_module(binding.declaration_source);
                if module.is_valid() && self.display_path(module, "::") == logical {
                    let short = self.name(module);
                    return name
                        .strip_prefix(short)
                        .and_then(|suffix| suffix.strip_prefix("::"))
                        .is_some_and(|suffix| candidate_path == format!("{logical}::{suffix}"));
                }
                false
            });
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
            Some(SymbolLookup::from_candidates(matches.into_iter()))
        } else if matches.is_empty() {
            None
        } else {
            Some(SymbolLookup::from_candidates(matches.into_iter()))
        }
    }
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
