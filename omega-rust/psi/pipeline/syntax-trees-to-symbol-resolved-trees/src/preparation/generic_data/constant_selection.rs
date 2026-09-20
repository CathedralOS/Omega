//! Exact constant and nominal-carrier selection before closed argument names are erased.

use std::sync::Arc;

use diagnostics::Diagnostic;
use source::SourceMap;
use symbols::{
    SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
    builtin_function_symbols, builtin_type_symbols,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ConstDefinition, DataDefinition, DataMember, Item};

/// Private selection state; transient symbols never escape into normalized syntax.
pub(crate) struct ConstantSelection<'base> {
    symbols: SymbolTable,
    retained: Option<&'base symbol_resolved_trees::SymbolResolvedTrees>,
}

impl<'base> ConstantSelection<'base> {
    /// The namespace boundary for early declaration consistency, including
    /// dependency purpose. A logical path alone is not a foreign owner.
    pub(super) fn same_checked_package(
        &self,
        left: source::SourceSpan,
        right: source::SourceSpan,
    ) -> bool {
        self.symbols.same_source_package(left, right)
    }

    pub(crate) fn new(
        syntax: &SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::with_retained_base(syntax, sources, bindings, None)
    }

    pub(crate) fn with_retained_base(
        syntax: &SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
        retained: Option<&'base symbol_resolved_trees::SymbolResolvedTrees>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut namespaces = crate::symbols::NamespaceDeclarations::default();
        let mut declarations = Vec::new();
        for item in syntax.root_items() {
            match item {
                Item::Const(definition) => declarations.push((
                    SymbolKind::Const,
                    crate::constant::semantic_const_name(definition),
                    definition.name.source_span(),
                    None,
                )),
                Item::Data(definition) => declarations.push((
                    SymbolKind::Data,
                    definition.name.as_str().to_owned(),
                    definition.name.source_span(),
                    Some(definition),
                )),
                Item::Domain(definition) => declarations.push((
                    SymbolKind::Domain,
                    definition.name.as_str().to_owned(),
                    definition.name.source_span(),
                    None,
                )),
                Item::Trait(definition) => declarations.push((
                    SymbolKind::Trait,
                    definition.name.as_str().to_owned(),
                    definition.name.source_span(),
                    None,
                )),
                Item::Module(module) => namespaces
                    .modules
                    .push(syntax.items.identifier_path_members(module.path).to_vec()),
                Item::Use(import) => namespaces
                    .imports
                    .push(syntax.items.identifier_path_members(import.path).to_vec()),
                _ => {}
            }
        }
        let header_names = || {
            declarations.iter().map(|(kind, name, span, _)| {
                (
                    *kind,
                    SymbolNameRef::OwnedSource {
                        value: name,
                        source_span: *span,
                    },
                )
            })
        };
        let mut symbols = if let Some(base) = retained {
            // One combined resolver preserves local/import precedence and
            // ambiguity. A failed current lookup never falls back to the base.
            let mut builder = base.symbols.clone().begin_extension(sources, bindings);
            let symbols = builder.insert_top_level(header_names());
            for (symbol, (_, _, _, definition)) in symbols.into_iter().zip(&declarations) {
                if let Some(definition) = definition {
                    append_variants(&mut builder, syntax, symbol, definition);
                }
            }
            builder.finish()
        } else {
            let mut builder =
                SymbolTableBuilder::with_sources_and_top_level_bindings(sources, bindings);
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let builtin_types = builtin_type_symbols();
            let builtin_functions = builtin_function_symbols();
            let builtin_count = builtin_types.len() + builtin_functions.len();
            let children = builder.insert_children(
                root,
                builtin_types
                    .into_iter()
                    .chain(builtin_functions)
                    .chain(header_names()),
            );
            for (symbol, (_, _, _, definition)) in SymbolTableBuilder::child_handles(children)
                .skip(builtin_count)
                .zip(&declarations)
            {
                if let Some(definition) = definition {
                    append_variants(&mut builder, syntax, symbol, definition);
                }
            }
            builder.finish()
        };
        // Machine and other nondata import targets are absent from this
        // partial header table. Complete resolution validates those imports.
        namespaces.register(&mut symbols)?;
        Ok(Self { symbols, retained })
    }

    /// Select an exposed family after the caller has bound the authored
    /// carrier prefix to the enclosing declaration's exact type binder.
    /// Fixed-carrier families cannot classify that arbitrary open carrier.
    /// The returned address keeps declaration ownership when the carrier is
    /// substituted; the caller must retain the original occurrence's span.
    pub(super) fn generic_carrier_domain_address(
        &self,
        syntax: &SyntaxTrees,
        leaf: &str,
        reference: source::SourceSpan,
    ) -> Option<String> {
        if reference.span.start == reference.span.end || leaf.contains("::") {
            return None;
        }
        let mut candidates = Vec::new();
        for symbol in self
            .symbols
            .child_handles(self.symbols.root())
            .into_iter()
            .flatten()
        {
            if self.symbols.get(symbol).kind != SymbolKind::Domain
                || self.symbols.name(symbol) != leaf
                || !self
                    .symbols
                    .source_reference_can_see_symbol(reference, symbol)
            {
                continue;
            }
            let module = self.symbols.symbol_module(symbol);
            let exposed = !module.is_valid()
                || self.symbols.source_module(reference.source_id) == module
                || self
                    .symbols
                    .source_module_import_paths(reference.source_id)
                    .any(|path| {
                        matches!(
                            self.symbols.source_module_import_target(reference.source_id, path),
                            Some(target) if target == symbol || target == module
                        )
                    });
            if !exposed {
                continue;
            }
            let source_span = self.symbols.symbol_source_span(symbol);
            let syntax_generic_carrier = syntax.root_items().any(|item| {
                let Item::Domain(domain) = item else {
                    return false;
                };
                if Some(domain.name.source_span()) != source_span || domain.name.as_str() != leaf {
                    return false;
                }
                let Some(parameter) = syntax.items.type_parameters(domain.type_parameters).first()
                else {
                    return false;
                };
                matches!(parameter.kind, syntax_trees::item::TypeParameterKind::Type)
                    && matches!(
                        syntax.type_references.type_reference(domain.target_type),
                        syntax_trees::types::TypeReferenceNode::Named(name)
                            if name.as_str() == parameter.name.as_str()
                    )
            });
            let retained_generic_carrier = self.retained.is_some_and(|retained| {
                retained.domain_definitions.iter().any(|domain| {
                    if domain.symbol != symbol {
                        return false;
                    }
                    let Some(parameter) = retained
                        .data_type_parameters(domain.type_parameters)
                        .first()
                    else {
                        return false;
                    };
                    matches!(
                        parameter.kind,
                        symbol_resolved_trees::data::TypeParameterKind::Type
                    ) && matches!(
                        &domain.target_type,
                        symbol_resolved_trees::types::TypeReference::Named { symbol, .. }
                            if parameter.symbol.is_valid() && *symbol == parameter.symbol
                    )
                })
            });
            if syntax_generic_carrier || retained_generic_carrier {
                candidates.push(symbol);
            }
        }
        // A carrier-qualified selection is not a bare local domain name.
        // Distinct exposed families compete even when one is module-local.
        let first = *candidates.first()?;
        let address = self.symbols.display_path(first, "::");
        if candidates.iter().any(|candidate| {
            self.symbols.display_path(*candidate, "::") != address
                || !self.symbols.same_symbol_source_package(first, *candidate)
        }) {
            return None;
        }
        let selects_family = |address: &str| {
            self.symbols.find_top_level_by_name_and_kinds_from_source(
                address,
                &[SymbolKind::Domain],
                reference,
            ) == Some(first)
        };
        if selects_family(&address) {
            return Some(address);
        }
        // Logical paths omit package identity. An unimported local module can
        // share the selected foreign family's path, so replay must retain the
        // requester's certified dependency prefix when that path collides.
        // Import targets establish ownership before constructing an address;
        // ordinary lookup must then select the same exact declaration again.
        let module = self.symbols.symbol_module(first);
        for path in self.symbols.source_module_import_paths(reference.source_id) {
            let target = self
                .symbols
                .source_module_import_target(reference.source_id, path);
            let address = if target == Some(first) {
                path.to_owned()
            } else if module.is_valid() && target == Some(module) {
                format!("{path}::{leaf}")
            } else {
                continue;
            };
            if selects_family(&address) {
                return Some(address);
            }
        }
        None
    }

    pub(super) fn retained_identity(
        &self,
        name: &Identifier,
        kind: SymbolKind,
    ) -> Option<symbols::SymbolHandle> {
        let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[kind],
            name.source_span(),
        )?;
        let retained = self.retained?;
        (retained.symbols.get(selected).kind == kind).then_some(selected)
    }

    pub(super) fn retained_domain_identity(
        &self,
        carrier: &Identifier,
        name: &Identifier,
    ) -> Option<symbols::SymbolHandle> {
        let carrier = self.symbols.find_top_level_by_name_and_kinds_from_source(
            carrier.as_str(),
            &[SymbolKind::BuiltinType, SymbolKind::Data],
            carrier.source_span(),
        )?;
        let retained = self.retained?;
        if !matches!(
            retained.symbols.get(carrier).kind,
            SymbolKind::BuiltinType | SymbolKind::Data
        ) {
            return None;
        }
        let qualified = if name.as_str().contains("::") {
            self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(),
                &[SymbolKind::Domain],
                name.source_span(),
            )?
        } else {
            symbols::SymbolHandle::invalid()
        };
        let mut candidates = retained.domain_definitions.iter().filter(|domain| {
            matches!(&domain.target_type, symbol_resolved_trees::types::TypeReference::Named { symbol, .. } if *symbol == carrier)
                && (if qualified.is_valid() {
                    domain.symbol == qualified
                } else {
                    domain.name.as_str().rsplit("::").next() == Some(name.as_str())
                })
                && self.symbols.source_reference_can_see_symbol(name.source_span(), domain.symbol)
        });
        let selected = candidates.next()?.symbol;
        candidates.next().is_none().then_some(selected)
    }

    /// Whether one top-level domain symbol is visible to `reference` and
    /// reachable under module name law for the authored spelling — qualified
    /// paths name their exact declaration, relative and leaf spellings need
    /// the module's own source or a narrow import, and unmoduled domains keep
    /// root scope.
    fn domain_symbol_is_reachable(
        &self,
        symbol: symbols::SymbolHandle,
        selected: symbols::SymbolHandle,
        authored: &str,
        reference: source::SourceSpan,
    ) -> bool {
        // Ordinary package-qualified paths and exact imports join the same
        // candidate pool as attached-domain exposure, as in membership lookup.
        // A dependency alias need not equal the declaration's logical module
        // path, so display-path matching alone cannot establish this route.
        if selected == symbol {
            return true;
        }
        // A complete logical path can occur in several checked packages.
        // Rejoin ordinary source-owned lookup before accepting that spelling;
        // matching display text alone cannot select the declaration's owner.
        if authored.contains("::") && self.symbols.display_path(symbol, "::") == authored {
            return false;
        }
        self.symbols.get(symbol).kind == SymbolKind::Domain
            && self
                .symbols
                .source_reference_can_see_symbol(reference, symbol)
            && crate::symbols::domain_name_reaches(
                &self.symbols,
                symbol,
                self.symbols.name(symbol),
                authored,
                reference,
            )
    }

    /// Whether the authored spelling reaches an in-forest domain declaration
    /// under module name law, including generic families whose closed
    /// membership `domain` declines to evaluate. When `domain` returns `None`
    /// this distinguishes absence — nothing was reachable, so a retained-base
    /// owner may still be selected — from a contested spelling whose reachable
    /// in-forest candidates the resolver will pool and reject or own itself.
    pub(super) fn contested_domain(
        &self,
        syntax: &SyntaxTrees,
        authored: &str,
        reference: source::SourceSpan,
    ) -> bool {
        let selected = self
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                authored,
                &[SymbolKind::Domain],
                reference,
            )
            .unwrap_or_else(symbols::SymbolHandle::invalid);
        self.symbols
            .child_handles(self.symbols.root())
            .into_iter()
            .flatten()
            .filter(|symbol| {
                self.domain_symbol_is_reachable(*symbol, selected, authored, reference)
            })
            .any(|symbol| {
                let Some(span) = self.symbols.symbol_source_span(symbol) else {
                    return false;
                };
                syntax.root_items().any(|item| {
                    matches!(item, Item::Domain(definition)
                        if definition.name.source_span() == span
                            && definition.name.as_str() == self.symbols.name(symbol))
                })
            })
    }

    /// Pool every in-forest domain declaration the authored spelling reaches
    /// under module name law, narrowed to the candidates module-local
    /// precedence leaves for `reference`. The pool retains declaration
    /// custody so each caller can distinguish a generic family from a closed
    /// domain before deciding what `None` means.
    fn pooled_domains<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        authored: &str,
        reference: source::SourceSpan,
    ) -> Vec<(
        symbols::SymbolHandle,
        &'syntax syntax_trees::item::DomainDefinition,
    )> {
        let selected = self
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                authored,
                &[SymbolKind::Domain],
                reference,
            )
            .unwrap_or_else(symbols::SymbolHandle::invalid);
        let mut pool = Vec::new();
        for symbol in self
            .symbols
            .child_handles(self.symbols.root())
            .into_iter()
            .flatten()
        {
            if !self.domain_symbol_is_reachable(symbol, selected, authored, reference) {
                continue;
            }
            let Some(span) = self.symbols.symbol_source_span(symbol) else {
                continue;
            };
            let mut declarations = syntax.root_items().filter_map(|item| match item {
                Item::Domain(definition)
                    if definition.name.source_span() == span
                        && definition.name.as_str() == self.symbols.name(symbol) =>
                {
                    Some(definition)
                }
                _ => None,
            });
            let Some(definition) = declarations.next() else {
                continue;
            };
            if declarations.next().is_some() {
                continue;
            }
            // Generic families pool too, exactly as the resolver's candidate
            // set does: beside a same-leaf sibling they contest the spelling,
            // and module-local preference ranks them ahead of foreign or root
            // candidates. Skipping them here would silently cede the spelling
            // to a non-generic sibling the resolver never selects.
            pool.push((symbol, definition));
        }
        let preferred = crate::symbols::prefer_module_local_domain(
            &self.symbols,
            pool.iter().map(|(symbol, _)| *symbol).collect(),
            reference,
        );
        pool.into_iter()
            .filter(|(symbol, _)| preferred.contains(symbol))
            .collect()
    }

    /// Select one non-generic declared domain under module name law for a
    /// pre-resolution fact evaluation. This is the same selection the full
    /// resolver later applies to the fact's retained path: qualified spellings
    /// name the exact `module::Carrier::Domain` declaration, relative attached
    /// spellings reach a module-owned domain only from its own module, a leaf
    /// reaches a foreign domain only through a narrow import of the exact
    /// declaration, and unmoduled domains keep root scope.
    ///
    /// `None` means undecidable here — nothing selected, competing owners,
    /// a reachable generic family (its closed membership belongs to the
    /// downstream family normalization), or a retained-base domain with no
    /// declaration in this forest. Callers keep the fact as a checked
    /// obligation rather than discharge it against a guessed owner.
    pub(super) fn domain<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        authored: &str,
        reference: source::SourceSpan,
    ) -> Option<&'syntax syntax_trees::item::DomainDefinition> {
        let pool = self.pooled_domains(syntax, authored, reference);
        // A preferred generic family owns or contests this selection, but its
        // closed membership still belongs to the downstream family
        // normalization; decline rather than evaluate against it or cede the
        // spelling to a pooled sibling.
        if pool
            .iter()
            .any(|(_, definition)| !definition.type_parameters.is_empty())
        {
            return None;
        }
        let (first_symbol, first) = pool.first()?;
        // Same complete logical path within one checked package is one
        // semantic candidate, matching post-resolution lookup. Competing owners
        // decline; the retained fact still owes its exact selection.
        let first_path = self.symbols.display_path(*first_symbol, "::");
        if pool.iter().any(|(symbol, _)| {
            self.symbols.display_path(*symbol, "::") != first_path
                || !self
                    .symbols
                    .same_symbol_source_package(*first_symbol, *symbol)
        }) {
            return None;
        }
        Some(*first)
    }

    /// Select one generic domain family under the same law as `domain`, for
    /// pre-resolution index canonicalization. The spelling's unique preferred
    /// owner must be a generic declaration — a carrier/indexed family — so
    /// its telescope owns the argument fold. `None` means the application
    /// keeps its authored arguments for ordinary resolution: nothing
    /// reachable, competing distinct owners, or a non-generic owner whose
    /// declaration carries no index telescope here.
    pub(super) fn domain_family<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        authored: &str,
        reference: source::SourceSpan,
    ) -> Option<(
        symbols::SymbolHandle,
        &'syntax syntax_trees::item::DomainDefinition,
    )> {
        let pool = self.pooled_domains(syntax, authored, reference);
        let (first_symbol, first) = pool.first()?;
        let first_path = self.symbols.display_path(*first_symbol, "::");
        if pool.iter().any(|(symbol, _)| {
            self.symbols.display_path(*symbol, "::") != first_path
                || !self
                    .symbols
                    .same_symbol_source_package(*first_symbol, *symbol)
        }) {
            return None;
        }
        (!first.type_parameters.is_empty()).then_some((*first_symbol, *first))
    }

    /// Select the nominal carrier in the authored type/constructor's source.
    /// Header handles remain transient; declaration custody is rejoined after
    /// ordinary resolution allocates the receiving generic slot.
    pub(crate) fn data<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<&'syntax DataDefinition, String> {
        let selected = self
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                name.as_str(),
                &[SymbolKind::Data],
                name.source_span(),
            )
            .ok_or_else(|| format!("`{name}` does not select one declared canonical data type"))?;
        self.data_declaration(syntax, name, selected)
    }

    pub(crate) fn constructor<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<(&'syntax DataDefinition, Option<Identifier>), String> {
        let (selected, case) =
            crate::symbols::constructor_type(&self.symbols, name.as_str(), name.source_span())?
                .ok_or_else(|| {
                    format!("`{name}` does not select one declared constructor carrier")
                })?;
        let definition = self.data_declaration(syntax, name, selected)?;
        let case = case.map(|case| {
            let mut span = name.source_span();
            if span.span.end >= case.len() {
                span.span.start = span.span.end - case.len();
            }
            Identifier::new(case, span)
        });
        Ok((definition, case))
    }

    pub(crate) fn builtin_type(&self, name: &Identifier) -> Option<symbols::BuiltinTypeAtom> {
        let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[SymbolKind::BuiltinType, SymbolKind::Data],
            name.source_span(),
        )?;
        self.symbols.builtin_type_atom(selected)
    }

    /// Select one trait header in the authored occurrence's source context.
    /// The lookup distinguishes unique, absent, and ambiguous selection so the
    /// caller can reject ambiguity rather than guessing a same-leaf template.
    pub(crate) fn trait_lookup(&self, name: &Identifier) -> symbols::SymbolLookup {
        self.symbols
            .lookup_top_level_by_name_and_kinds_from_source_matching(
                name.as_str(),
                &[SymbolKind::Trait],
                name.source_span(),
                |_| true,
            )
    }

    /// Select one data header in the authored occurrence's source context.
    /// Absent and ambiguous selections both return `None`; the declaring
    /// stage that owns the subject emits the exact diagnostic.
    pub(crate) fn data_symbol(&self, name: &Identifier) -> Option<symbols::SymbolHandle> {
        self.symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[SymbolKind::Data],
            name.source_span(),
        )
    }

    /// The selected declaration's logical namespace path, for generated
    /// references that must re-resolve in another source's import scope.
    pub(crate) fn declaration_path(&self, symbol: symbols::SymbolHandle) -> String {
        self.symbols.display_path(symbol, "::")
    }

    /// Whether one authored name selects an exact constant declaration in its
    /// own source scope. Private or ambiguous candidates still count as
    /// constants here: the leaf's checked admission and package custody own
    /// the legality verdict, while this lookup only distinguishes a value
    /// reference from a literal case or constructor spelling.
    pub(crate) fn selects_const(&self, name: &Identifier) -> bool {
        self.symbols
            .find_top_level_by_name_and_kinds_from_source(
                name.as_str(),
                &[SymbolKind::Const],
                name.source_span(),
            )
            .is_some()
    }

    /// Bare Names retain value-prefix precedence before specialization changes
    /// their lookup metadata. The shared resolver distinguishes absence from
    /// unique or ambiguous constants; braces remain ordinary constructors.
    pub(crate) fn bare_case<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<Option<(&'syntax DataDefinition, Identifier)>, String> {
        let Some((owner, _, case)) =
            crate::symbols::bare_case_type(&self.symbols, name.as_str(), name.source_span())?
        else {
            return Ok(None);
        };
        let definition = self.data_declaration(syntax, name, owner)?;
        let mut span = name.source_span();
        if span.span.end >= case.len() {
            span.span.start = span.span.end - case.len();
        }
        Ok(Some((definition, Identifier::new(case, span))))
    }

    fn data_declaration<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
        selected: symbols::SymbolHandle,
    ) -> Result<&'syntax DataDefinition, String> {
        let span = self
            .symbols
            .symbol_source_span(selected)
            .ok_or_else(|| "selected nominal carrier has no declaration source".to_owned())?;
        let mut declarations = syntax.root_items().filter_map(|item| match item {
            Item::Data(definition)
                if definition.name.source_span() == span
                    && definition.name.as_str() == self.symbols.name(selected) =>
            {
                Some(definition)
            }
            _ => None,
        });
        let definition = declarations
            .next()
            .ok_or_else(|| "selected nominal carrier lost its declaration".to_owned())?;
        if declarations.next().is_some() {
            return Err("selected nominal carrier has ambiguous declaration custody".to_owned());
        }
        if !definition.is_public && !self.symbols.same_source_package(name.source_span(), span) {
            return Err(format!(
                "nominal carrier `{name}` selects a private declaration in another package"
            ));
        }
        Ok(definition)
    }

    /// Find a lookup spelling for the already selected template. This is not
    /// identity: every candidate is checked against the same selected header.
    /// Keep requester imports and the use span; never relocate a use to its
    /// declaration merely to make generated instance lookup succeed.
    pub(super) fn data_lookup_path(
        &self,
        syntax: &SyntaxTrees,
        name: &Identifier,
    ) -> Option<String> {
        let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[SymbolKind::Data],
            name.source_span(),
        )?;
        let path = self.symbols.display_path(selected, "::");
        let declaration = self.symbols.symbol_source_span(selected)?;
        let selects = |path: &str| {
            self.symbols.find_top_level_by_name_and_kinds_from_source(
                path,
                &[SymbolKind::Data],
                name.source_span(),
            ) == Some(selected)
        };
        if self
            .symbols
            .same_source_package(name.source_span(), declaration)
            && selects(&path)
        {
            return Some(path);
        }
        if name.as_str().contains("::") && selects(name.as_str()) {
            return Some(name.as_str().to_owned());
        }
        for item in syntax.root_items() {
            let Item::Use(import) = item else {
                continue;
            };
            let members = syntax.items.identifier_path_members(import.path);
            if members.first()?.source_span().source_id != name.source_span().source_id {
                continue;
            }
            let mut prefix = String::new();
            for member in members {
                if !prefix.is_empty() {
                    prefix.push_str("::");
                }
                prefix.push_str(member.as_str());
                let candidate = format!("{prefix}::{path}");
                if selects(&candidate) {
                    return Some(candidate);
                }
            }
        }
        // A loader-bound source need not declare a logical module. Its broad
        // import can expose this exact carrier without making any qualified
        // spelling valid. Preserve that authored lookup only after trying the
        // qualified paths; declaration/argument custody still owns identity.
        Some(name.as_str().to_owned())
    }

    /// Return exact declaration custody, not a handle from the header table.
    /// No match includes unresolved or ambiguous selection; callers must defer
    /// it to ordinary validation, never fall back to a lexical constant map.
    pub(super) fn select(
        &self,
        syntax: &SyntaxTrees,
        name: &Identifier,
    ) -> Result<Option<ConstDefinition>, Diagnostic> {
        if name.source_span().span.start == name.source_span().span.end {
            return Ok(None);
        }
        let Some(selected) = self.symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[SymbolKind::Const],
            name.source_span(),
        ) else {
            return Ok(None);
        };
        let declaration_span = self.symbols.symbol_source_span(selected).ok_or_else(|| {
            Diagnostic::error("selected constant header has no retained declaration source")
                .with_source_span(name.source_span())
        })?;
        let mut declarations = syntax.root_items().filter_map(|item| {
            let Item::Const(definition) = item else {
                return None;
            };
            (definition.name.source_span() == declaration_span).then_some(definition)
        });
        let declaration = declarations.next().ok_or_else(|| {
            Diagnostic::error("selected constant header lost its exact source declaration")
                .with_source_span(name.source_span())
        })?;
        if declarations.next().is_some() {
            return Err(Diagnostic::error(
                "selected constant header has ambiguous source declaration custody",
            )
            .with_source_span(name.source_span()));
        }
        if !declaration.is_public
            && !self
                .symbols
                .same_source_package(name.source_span(), declaration_span)
        {
            return Err(Diagnostic::error(format!(
                "constant `{name}` selects a private declaration in another package",
            ))
            .with_source_span(name.source_span()));
        }
        Ok(Some(declaration.clone()))
    }
}

fn append_variants(
    builder: &mut impl symbols::SymbolTableAppender,
    syntax: &SyntaxTrees,
    symbol: symbols::SymbolHandle,
    definition: &DataDefinition,
) {
    builder.insert_children(
        symbol,
        syntax
            .items
            .data_members(definition.members)
            .iter()
            .filter_map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                Some((
                    SymbolKind::Variant,
                    SymbolNameRef::OwnedSource {
                        value: variant.name.as_str(),
                        source_span: variant.name.source_span(),
                    },
                ))
            }),
    );
}

/// The argument bindings one closed generic application establishes for its
/// selected template's parameters: `Pair<u64>` binds `T` to the `u64` spelling.
/// Bindings only name arguments that already exist as handles; a member type
/// still spelling an open parameter resolves one application level at a time
/// rather than materializing a substituted handle inside an immutable forest.
pub(crate) type GenericApplicationSubstitution =
    std::collections::HashMap<String, syntax_trees::types::TypeReferenceHandle>;

/// Whether `type_reference` still spells one of the bound parameter names.
/// Only structural positions are scanned; an expression-bearing position (a
/// deferred const argument or a range bound) conservatively counts as open
/// whenever any binding exists, because its expression may name a parameter.
pub(crate) fn type_mentions_parameters(
    syntax: &SyntaxTrees,
    type_reference: syntax_trees::types::TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
) -> bool {
    use syntax_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
    if substitution.is_empty() {
        return false;
    }
    match syntax.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str()),
        TypeReferenceNode::Generic { arguments, .. } => syntax
            .type_references
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_mentions_parameters(syntax, *argument, substitution)),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            type_mentions_parameters(syntax, *element_type, substitution)
                || matches!(length, FixedArrayLength::ConstParameter(name) if substitution.contains_key(name.as_str()))
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_mentions_parameters(syntax, *base_type, substitution)
                || syntax
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Domain(domain) => syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                            .iter()
                            .any(|argument| {
                                type_mentions_parameters(syntax, *argument, substitution)
                            }),
                        TypeConstraintNode::Range { .. } => true,
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            false
                        }
                    })
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_mentions_parameters(syntax, *referee, substitution)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_mentions_parameters(syntax, *element_type, substitution)
        }
        TypeReferenceNode::ConstExpression(_) => true,
        _ => false,
    }
}

/// Resolve one generic argument spelling through the enclosing application's
/// bindings. A bare parameter name reselects its already-closed argument
/// handle; a composite spelling that still names a parameter cannot
/// materialize a substituted handle inside an immutable forest and declines.
pub(crate) fn resolved_generic_argument(
    syntax: &SyntaxTrees,
    argument: syntax_trees::types::TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
) -> Result<syntax_trees::types::TypeReferenceHandle, String> {
    if let syntax_trees::types::TypeReferenceNode::Named(name) =
        syntax.type_references.type_reference(argument)
        && let Some(resolved) = substitution.get(name.as_str())
    {
        return Ok(*resolved);
    }
    if type_mentions_parameters(syntax, argument, substitution) {
        return Err(
            "computed constant carrier member is not yet a closed structural type".to_owned(),
        );
    }
    Ok(argument)
}

/// Whether `type_reference` contains a deferred `ConstExpression` argument.
/// Probe forests replace those arguments with layout stand-ins, so a carrier
/// spelling containing one drifts from the materialized constructor's name;
/// closed leaf destinations must be probe-stable.
pub(crate) fn has_deferred_const_argument(
    syntax: &SyntaxTrees,
    type_reference: syntax_trees::types::TypeReferenceHandle,
) -> bool {
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};
    match syntax.type_references.type_reference(type_reference) {
        TypeReferenceNode::ConstExpression(_) => true,
        TypeReferenceNode::Generic { arguments, .. } => syntax
            .type_references
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| has_deferred_const_argument(syntax, *argument)),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            has_deferred_const_argument(syntax, *element_type)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            has_deferred_const_argument(syntax, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            has_deferred_const_argument(syntax, *base_type)
                || syntax
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Domain(domain) => syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                            .iter()
                            .any(|argument| has_deferred_const_argument(syntax, *argument)),
                        _ => false,
                    })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Identifier, SourceScopedTopLevelBinding, SyntaxTrees};
    use crate::preparation::generic_data::constant_selection::ConstantSelection;
    use source::SourceId;
    use source_files_to_tokens::Lexer;
    use syntax_trees::types::TypeReferenceNode;

    fn parse_sources(sources: &[(SourceId, &str)]) -> SyntaxTrees {
        let mut syntax = SyntaxTrees::new(SourceId::default());
        for (source_id, text) in sources {
            let tokens = Lexer::new(text)
                .tokenize()
                .expect("tokenize constant headers");
            tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
                &mut syntax,
                *source_id,
                &tokens,
            )
            .expect("parse constant headers");
        }
        syntax
    }

    fn argument_for_source(syntax: &SyntaxTrees, source: SourceId) -> Identifier {
        syntax
            .type_references
            .generic_nodes()
            .into_iter()
            .find_map(|handle| {
                let TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } = syntax.type_references.type_reference(handle)
                else {
                    return None;
                };
                if base_name.source_span().source_id != source {
                    return None;
                }
                let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
                    return None;
                };
                match syntax.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(name) => Some(name.clone()),
                    _ => None,
                }
            })
            .expect("one authored named argument")
    }

    #[test]
    fn shared_header_selection_retains_distinct_root_and_module_declarations() {
        for reverse in [false, true] {
            let mut sources = [
                (
                    SourceId(1),
                    "const SIZE: u64 = 1; data Buffer<const N: u64> { value: u64; } data Root { value: Buffer<SIZE>; }",
                ),
                (
                    SourceId(2),
                    "module combat; const SIZE: u64 = 2; data Combat { value: Buffer<SIZE>; }",
                ),
                (
                    SourceId(3),
                    "module rooms; const SIZE: u64 = 3; data Rooms { value: Buffer<SIZE>; }",
                ),
            ];
            if reverse {
                sources.reverse();
            }
            let syntax = parse_sources(&sources);
            let selection = ConstantSelection::new(&syntax, None, Vec::new()).expect("headers");
            for source in [SourceId(1), SourceId(2), SourceId(3)] {
                let name = argument_for_source(&syntax, source);
                let declaration = selection
                    .select(&syntax, &name)
                    .expect("selection")
                    .expect("constant");
                assert_eq!(declaration.name.source_span().source_id, source);
                let syntax_trees::expression::ExpressionNode::Integer(value) =
                    syntax.expressions.expression(declaration.value)
                else {
                    panic!("integer initializer");
                };
                assert_eq!(value.value_u64(), Some(source.0 as u64));
            }
        }
    }

    #[test]
    fn shared_header_selection_never_falls_back_from_ambiguous_imports() {
        let syntax = parse_sources(&[
            (
                SourceId(1),
                "use combat::SIZE; use rooms::SIZE; data Root { value: Buffer<SIZE>; }",
            ),
            (SourceId(2), "module combat; pub const SIZE: u64 = 2;"),
            (SourceId(3), "module rooms; pub const SIZE: u64 = 3;"),
        ]);
        let selection = ConstantSelection::new(&syntax, None, Vec::new()).expect("headers");
        let name = argument_for_source(&syntax, SourceId(1));
        assert!(
            selection
                .select(&syntax, &name)
                .expect("unresolved selection")
                .is_none()
        );
    }

    #[test]
    fn nonconstant_import_targets_wait_for_complete_resolution() {
        let syntax = parse_sources(&[
            (
                SourceId(1),
                "use combat::Damage; data Root { value: Buffer<combat::SIZE>; }",
            ),
            (
                SourceId(2),
                "module combat; pub const SIZE: u64 = 2; pub data Damage { value: u64; }",
            ),
        ]);
        let selection = ConstantSelection::new(
            &syntax,
            None,
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(1),
                SourceId(2),
                "combat::Damage",
                0,
            )],
        )
        .expect("nonconstant import validation is deferred");
        let name = argument_for_source(&syntax, SourceId(1));
        let declaration = selection
            .select(&syntax, &name)
            .expect("qualified selection")
            .expect("constant");
        assert_eq!(declaration.name.source_span().source_id, SourceId(2));
    }
}
