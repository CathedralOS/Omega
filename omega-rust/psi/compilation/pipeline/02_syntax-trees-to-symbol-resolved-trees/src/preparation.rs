/*
    Syntax to syntax, before any symbol exists.

    `declaration_selection` owns the partial namespace used to select constants,
    nominal carriers, domains and traits before complete symbol assignment.
    Preparation and resolution retain that shared selection while their own
    consumers validate modules, synthesize defaults and close applications.

    `module_normalization` validates the forest against the constant selector,
    `generic_data` closes eligible generic data applications, and
    `trait_defaults` materializes default trait machines as ordinary attached
    machines. All three consume syntax and return syntax; their templates,
    substitutions, and evaluation scratch are not program representations.
*/

pub(crate) mod declaration_selection {
    /*
        Shared declaration selection before complete symbol assignment.

        `ConstantSelection::new` collects declaration headers, builds a partial
        namespace with imports and builtin names, and exposes exact lookup for
        constants, nominal carriers, domains and traits. `with_retained_base` extends
        the same namespace for generated source. Preparation, initializer handling
        and resolution share this owner; generic-data synthesis is one consumer.
        Transient symbols remain private and do not escape into normalized syntax.
    */

    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use std::sync::Arc;

    use crate::symbols::constructor_type;
    use diagnostics::Diagnostic;
    use source::SourceMap;
    use std::path::Path;
    use symbols::{
        SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
        builtin_function_symbols, builtin_type_symbols,
    };
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{ConstDefinition, DataDefinition, DataMember, DomainDefinition, Item};

    /* Scratch declaration roles, not parameter payloads or admission evidence. */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum GenericParameterRole {
        Type,
        Implementation,
        Const,
        Value,
        Machine,
        Proposition,
    }

    /* Private selection state; transient symbols never escape into normalized syntax. */
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub(crate) struct ConstantSelection<'base> {
        symbols: SymbolTable,
        retained: Option<SelectionBase<'base>>,
    }

    /*
        Ephemeral syntax normalization borrows its predecessor; a deferred resolver
        owns that same predecessor across later body demands.
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SelectionBase<'base> {
        Borrowed(&'base symbol_resolved_trees::SymbolResolvedTrees),
        Shared(Arc<symbol_resolved_trees::SymbolResolvedTrees>),
    }

    impl std::ops::Deref for SelectionBase<'_> {
        type Target = symbol_resolved_trees::SymbolResolvedTrees;

        fn deref(&self) -> &Self::Target {
            match self {
                Self::Borrowed(base) => base,
                Self::Shared(base) => base,
            }
        }
    }

    /*
        Representation adapters share the same immutable Core declaration contract.
        Neither a display name nor an opaque supply mode alone selects this meaning.
    */
    pub(crate) fn is_core_layout_storage_source(
        symbols: &SymbolTable,
        name: &str,
        supply_mode: language_semantics::DataSupplyMode,
        reference: source::SourceSpan,
    ) -> bool {
        name == language_core::layout_storage::CONSTRUCTOR_NAME
            && supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            && symbols.source_file(reference).is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.package_identity.is_none()
                    && file.path.strip_prefix(&file.package_root).ok()
                        == Some(Path::new(language_core::layout_storage::SOURCE_FILE_NAME))
                    && file.source.as_bytes() == language_core::layout_storage::SOURCE
            })
    }

    impl<'base> ConstantSelection<'base> {
        /*
            Preserve header identities while replacing lookup membership. Pending
            bodies must not resolve against a declaration excluded by this consumer.
        */
        pub(crate) fn activate_sources(
            mut self,
            sources: Arc<SourceMap>,
            bindings: Vec<SourceScopedTopLevelBinding>,
        ) -> Result<Self, Vec<Diagnostic>> {
            self.symbols = self
                .symbols
                .begin_source_activation(Arc::clone(&sources), bindings.clone())
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                .finish();
            if let Some(base) = self.retained.take() {
                let mut base = (*base).clone();
                base.symbols = base
                    .symbols
                    .begin_source_activation(sources, bindings)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                    .finish();
                base.resolution_indexes = Default::default();
                self.retained = Some(SelectionBase::Shared(Arc::new(base)));
            }
            Ok(self)
        }

        pub(crate) fn retained_base(&self) -> Option<&symbol_resolved_trees::SymbolResolvedTrees> {
            self.retained.as_deref()
        }

        pub(crate) fn shared_retained_base(
            &self,
        ) -> Option<Arc<symbol_resolved_trees::SymbolResolvedTrees>> {
            match &self.retained {
                Some(SelectionBase::Shared(base)) => Some(Arc::clone(base)),
                _ => None,
            }
        }

        /*
            The namespace boundary for early declaration consistency, including
            dependency purpose. A logical path alone is not a foreign owner.
        */
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

        pub(crate) fn with_shared_base(
            syntax: &SyntaxTrees,
            sources: Option<Arc<SourceMap>>,
            bindings: Vec<SourceScopedTopLevelBinding>,
            base: Option<Arc<symbol_resolved_trees::SymbolResolvedTrees>>,
        ) -> Result<ConstantSelection<'static>, Vec<Diagnostic>> {
            let symbols =
                ConstantSelection::with_retained_base(syntax, sources, bindings, base.as_deref())?
                    .symbols;
            Ok(ConstantSelection {
                symbols,
                retained: base.map(SelectionBase::Shared),
            })
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
                        crate::constant::semantic_const_name(syntax, definition),
                        definition.name.source_span(),
                        None,
                    )),
                    Item::Data(definition) => declarations.push((
                        SymbolKind::Data,
                        definition.name.as_str(syntax).to_owned(),
                        definition.name.source_span(),
                        Some(definition),
                    )),
                    Item::TypeAlias(definition) => declarations.push((
                        SymbolKind::TypeAlias,
                        definition.name.as_str(syntax).to_owned(),
                        definition.name.source_span(),
                        None,
                    )),
                    Item::Domain(definition) => declarations.push((
                        SymbolKind::Domain,
                        definition.name.as_str(syntax).to_owned(),
                        definition.name.source_span(),
                        None,
                    )),
                    Item::Trait(definition) => declarations.push((
                        SymbolKind::Trait,
                        definition.name.as_str(syntax).to_owned(),
                        definition.name.source_span(),
                        None,
                    )),
                    Item::Machine(definition) => declarations.push((
                        SymbolKind::Machine,
                        definition.name.as_str(syntax).to_owned(),
                        definition.name.source_span(),
                        None,
                    )),
                    Item::Conformance(definition) => {
                        if let Some(name) = &definition.alias {
                            declarations.push((
                                SymbolKind::Conformance,
                                name.as_str(syntax).to_owned(),
                                name.source_span(),
                                None,
                            ));
                        }
                    }
                    Item::Module(module) => namespaces.record_module(syntax, module.path),
                    Item::Use(import) => namespaces.record_import(syntax, import.path),
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
            namespaces.register(&mut symbols)?;
            Ok(Self {
                symbols,
                retained: retained.map(SelectionBase::Borrowed),
            })
        }

        /*
            Select an exposed family after the caller has bound the authored
            carrier prefix to the enclosing declaration's exact type binder.
            Fixed-carrier families cannot classify that arbitrary open carrier.
            The returned address keeps declaration ownership when the carrier is
            substituted; the caller must retain the original occurrence's span.
        */
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
                    let Some(domain) = item.as_domain() else {
                        return false;
                    };
                    if Some(domain.name.source_span()) != source_span
                        || domain.name.as_str(syntax) != leaf
                    {
                        return false;
                    }
                    let Some(parameter) =
                        syntax.items.type_parameters(domain.type_parameters).first()
                    else {
                        return false;
                    };
                    matches!(parameter.kind, syntax_trees::item::TypeParameterKind::Type)
                        && matches!(
                            syntax.type_references.type_reference(domain.target_type),
                            syntax_trees::types::TypeReferenceNode::Named(name)
                                if name.as_str(syntax) == parameter.name.as_str(syntax)
                        )
                });
                let retained_generic_carrier = self.retained.as_deref().is_some_and(|retained| {
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
            syntax_trees: &syntax_trees::SyntaxTrees,
            name: &Identifier,
            kind: SymbolKind,
        ) -> Option<symbols::SymbolHandle> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax_trees),
                &[kind],
                name.source_span(),
            )?;
            let retained = self.retained.as_deref()?;
            if !retained.symbols.symbols().nodes().is_valid(selected) {
                return None;
            }
            (retained.symbols.get(selected).kind == kind).then_some(selected)
        }

        /*
            Rejoin an already selected nominal owner to one retained header. This
            exposes its telescope without selecting again by spelling or deciding
            whether a caller's type application is complete.
        */
        pub(super) fn retained_data_header(
            &self,
            selected: symbols::SymbolHandle,
        ) -> Option<&symbol_resolved_trees::data::DataDefinition> {
            let mut definitions = self
                .retained
                .as_deref()?
                .data_definitions
                .iter()
                .filter(|definition| definition.symbol == selected);
            let definition = definitions.next()?;
            definitions.next().is_none().then_some(definition)
        }

        pub(super) fn retained_domain_identity(
            &self,
            syntax_trees: &syntax_trees::SyntaxTrees,
            carrier: &Identifier,
            name: &Identifier,
        ) -> Option<symbols::SymbolHandle> {
            let carrier = self.symbols.find_top_level_by_name_and_kinds_from_source(
                carrier.as_str(syntax_trees),
                &[SymbolKind::BuiltinType, SymbolKind::Data],
                carrier.source_span(),
            )?;
            let retained = self.retained.as_deref()?;
            if !retained.symbols.symbols().nodes().is_valid(carrier) {
                return None;
            }
            if !matches!(
                retained.symbols.get(carrier).kind,
                SymbolKind::BuiltinType | SymbolKind::Data
            ) {
                return None;
            }
            let qualified = if name.as_str(syntax_trees).contains("::") {
                self.symbols.find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax_trees),
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
                    domain.name.as_str().rsplit("::").next() == Some(name.as_str(syntax_trees))
                })
                && self.symbols.source_reference_can_see_symbol(name.source_span(), domain.symbol)
        });
            let selected = candidates.next()?.symbol;
            candidates.next().is_none().then_some(selected)
        }

        /*
            Whether one top-level domain symbol is visible to `reference` and
            reachable under module name law for the authored spelling — qualified
            paths name their exact declaration, relative and leaf spellings need
            the module's own source or a narrow import, and unmoduled domains keep
            root scope.
        */
        fn domain_symbol_is_reachable(
            &self,
            symbol: symbols::SymbolHandle,
            selected: symbols::SymbolHandle,
            authored: &str,
            reference: source::SourceSpan,
        ) -> bool {
            if selected == symbol {
                return true;
            }
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

        /*
            Whether the authored spelling reaches an in-forest domain declaration
            under module name law, including generic families whose closed
            membership `domain` declines to evaluate. When `domain` returns `None`
            this distinguishes absence — nothing was reachable, so a retained-base
            owner may still be selected — from a contested spelling whose reachable
            in-forest candidates the resolver will pool and reject or own itself.
        */
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
                            && definition.name.as_str(syntax) == self.symbols.name(symbol))
                    })
                })
        }

        /*
            Pool every in-forest domain declaration the authored spelling reaches
            under module name law, narrowed to the candidates module-local
            precedence leaves for `reference`. The pool retains declaration
            custody so each caller can distinguish a generic family from a closed
            domain before deciding what `None` means.
        */
        fn pooled_domains<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            authored: &str,
            reference: source::SourceSpan,
            carrier: Option<&str>,
        ) -> Vec<(symbols::SymbolHandle, &'syntax DomainDefinition)> {
            let selected = self
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    authored,
                    &[SymbolKind::Domain],
                    reference,
                )
                .unwrap_or_else(symbols::SymbolHandle::invalid);
            let mut pool = Vec::new();
            let relative = carrier.map(|carrier| format!("{carrier}::{authored}"));
            let relative_selected = relative
                .as_ref()
                .and_then(|relative| {
                    self.symbols.find_top_level_by_name_and_kinds_from_source(
                        relative,
                        &[SymbolKind::Domain],
                        reference,
                    )
                })
                .unwrap_or_else(symbols::SymbolHandle::invalid);
            for symbol in self
                .symbols
                .child_handles(self.symbols.root())
                .into_iter()
                .flatten()
            {
                if !self.domain_symbol_is_reachable(symbol, selected, authored, reference)
                    && !relative.as_ref().is_some_and(|relative| {
                        self.domain_symbol_is_reachable(
                            symbol,
                            relative_selected,
                            relative,
                            reference,
                        )
                    })
                {
                    continue;
                }
                let Some(span) = self.symbols.symbol_source_span(symbol) else {
                    continue;
                };
                let mut declarations = syntax.root_items().filter_map(|item| match item {
                    Item::Domain(definition)
                        if definition.name.source_span() == span
                            && definition.name.as_str(syntax) == self.symbols.name(symbol) =>
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

        /*
            Select one non-generic declared domain under module name law for a
            pre-resolution fact evaluation. This is the same selection the full
            resolver later applies to the fact's retained path: qualified spellings
            name the exact `module::Carrier::Domain` declaration, relative attached
            spellings reach a module-owned domain only from its own module, a leaf
            reaches a foreign domain only through a narrow import of the exact
            declaration, and unmoduled domains keep root scope.

            `None` means undecidable here — nothing selected, competing owners,
            a reachable generic family (its closed membership belongs to the
            downstream family normalization), or a retained-base domain with no
            declaration in this forest. Callers keep the fact as a checked
            obligation rather than discharge it against a guessed owner.
        */
        pub(super) fn domain<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            authored: &str,
            reference: source::SourceSpan,
            carrier: Option<&str>,
        ) -> Option<&'syntax DomainDefinition> {
            let pool = self.pooled_domains(syntax, authored, reference, carrier);
            if pool
                .iter()
                .any(|(_, definition)| !definition.type_parameters.is_empty())
            {
                return None;
            }
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
            Some(*first)
        }

        /*
            Select one generic domain family under the same law as `domain`, for
            pre-resolution index canonicalization. The spelling's unique preferred
            owner must be a generic declaration — a carrier/indexed family — so
            its telescope owns the argument fold. `None` means the application
            keeps its authored arguments for ordinary resolution: nothing
            reachable, competing distinct owners, or a non-generic owner whose
            declaration carries no index telescope here.
        */
        pub(super) fn domain_family<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            authored: &str,
            reference: source::SourceSpan,
            carrier: Option<&str>,
        ) -> Option<(symbols::SymbolHandle, &'syntax DomainDefinition)> {
            let pool = self.pooled_domains(syntax, authored, reference, carrier);
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

        /*
            Select the nominal carrier in the authored type/constructor's source.
            Header handles remain transient; declaration custody is rejoined after
            ordinary resolution allocates the receiving generic slot.
        */
        pub(crate) fn data<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            name: &Identifier,
        ) -> Result<&'syntax DataDefinition, String> {
            self.data_among(syntax, None, name)
        }

        /*
            `data` for a caller that selects many names in one unchanged forest
            and indexed its root declarations once.
        */
        pub(crate) fn data_in<'syntax>(
            &self,
            roots: &RootDeclarations<'syntax>,
            name: &Identifier,
        ) -> Result<&'syntax DataDefinition, String> {
            self.data_among(roots.syntax, Some(roots), name)
        }

        fn data_among<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            roots: Option<&RootDeclarations<'_>>,
            name: &Identifier,
        ) -> Result<&'syntax DataDefinition, String> {
            let selected = self
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax),
                    &[SymbolKind::Data],
                    name.source_span(),
                )
                .ok_or_else(|| {
                    format!(
                        "`{name}` does not select one declared canonical data type",
                        name = name.as_str(syntax)
                    )
                })?;
            self.data_declaration_among(syntax, roots, name, selected)
        }

        /*
            The selected data declaration's name span. A declaration the cohort no
            longer parses may still be selected through the retained predecessor's
            forest: the same span and name identify the same declaration, and the
            ordinary package-visibility rule still applies to it. `roots`, when
            supplied, indexes `syntax` once for many lookups.
        */
        fn data_span_among(
            &self,
            syntax: &SyntaxTrees,
            roots: Option<&RootDeclarations<'_>>,
            retained: Option<&SyntaxTrees>,
            name: &Identifier,
        ) -> Result<source::SourceSpan, String> {
            let error = match self.data_among(syntax, roots, name) {
                Ok(data) => return Ok(data.name.source_span()),
                Err(error) => error,
            };
            let Some(retained) = retained else {
                return Err(error);
            };
            let selected = self
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax),
                    &[SymbolKind::Data],
                    name.source_span(),
                )
                .ok_or_else(|| error.clone())?;
            let span = self
                .symbols
                .symbol_source_span(selected)
                .ok_or_else(|| error.clone())?;
            let selected_name = self.symbols.name(selected);
            let mut matches = retained.root_items().filter_map(|item| match item {
                Item::Data(data)
                    if data.name.source_span() == span
                        && data.name.as_str(retained) == selected_name =>
                {
                    Some(data.is_public)
                }
                _ => None,
            });
            let (Some(is_public), None) = (matches.next(), matches.next()) else {
                return Err(error);
            };
            if !is_public && !self.symbols.same_source_package(name.source_span(), span) {
                return Err(format!(
                    "canonical declaration `{}` is private in another package",
                    name.as_str(syntax)
                ));
            }
            Ok(span)
        }

        /*
            Leaf scheduling needs the selected carrier's closed telescope, not its
            syntax members. A retained predecessor owns the same authoritative
            declaration header even when its source is outside this resumed cohort.
            Value admission and materialization still check the exact full carrier.
        */
        pub(crate) fn require_closed_data_leaf(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Result<(), String> {
            let selected = self
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax),
                    &[SymbolKind::Data],
                    name.source_span(),
                )
                .ok_or_else(|| {
                    "computed nominal constant requires one selected data declaration".to_owned()
                })?;
            let reference = self
                .symbols
                .symbol_source_span(selected)
                .ok_or_else(|| "computed nominal carrier has no declaration custody".to_owned())?;
            if syntax.root_items().any(|item| matches!(item, Item::Data(data) if data.name.source_span() == reference && data.name.as_str(syntax) == self.symbols.name(selected))) {
            let data = self.data_declaration(syntax, name, selected)?;
            return if data.type_parameters.is_empty() && data.lifetime_parameters.is_empty() {
                Ok(())
            } else { Err("computed nominal constant requires a closed selected declaration".to_owned()) };
        }
            let data = self
                .retained_data_header(selected)
                .filter(|data| data.symbol == selected)
                .ok_or_else(|| {
                    "computed nominal carrier lost its selected declaration header".to_owned()
                })?;
            if !data.is_public
                && !self
                    .symbols
                    .same_source_package(name.source_span(), reference)
            {
                return Err("computed nominal carrier is private in another package".to_owned());
            }
            if !data.type_parameters.is_empty() || !data.lifetime_parameters.is_empty() {
                return Err(
                    "computed nominal constant requires a closed selected declaration".to_owned(),
                );
            }
            Ok(())
        }

        /*
            Recognize the already selected representation constructor, never an
            authored type with the same spelling. Its immutable source also owns
            the semantic-type and explicitly selected policy parameter contract.
        */
        pub(super) fn is_core_layout_storage(
            &self,
            syntax: &SyntaxTrees,
            definition: &DataDefinition,
        ) -> bool {
            is_core_layout_storage_source(
                &self.symbols,
                definition.name.as_str(syntax),
                definition.supply_mode,
                definition.name.source_span(),
            )
        }

        /*
            Select an alias in the ordinary type namespace. A same-scope nominal
            or builtin remains that declaration, never a name-based alias fallback.
        */
        pub(crate) fn type_alias_source(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Option<source::SourceSpan> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax),
                &[
                    SymbolKind::BuiltinType,
                    SymbolKind::Data,
                    SymbolKind::TypeAlias,
                ],
                name.source_span(),
            )?;
            (self.symbols.get(selected).kind == SymbolKind::TypeAlias)
                .then(|| self.symbols.symbol_source_span(selected))
                .flatten()
        }

        /*
            The selected telescope's roles only. Reading a retained header does
            not copy scratch syntax into the receiving forest or create work.
        */
        pub(crate) fn generic_parameter_roles(
            &self,
            syntax: &SyntaxTrees,
            declaration_syntax: &[&SyntaxTrees],
            name: &Identifier,
            kinds: &[SymbolKind],
        ) -> Option<Vec<GenericParameterRole>> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax),
                kinds,
                name.source_span(),
            )?;
            let reference = self.symbols.symbol_source_span(selected)?;
            let selected_kind = self.symbols.get(selected).kind;
            let selected_name = self.symbols.name(selected);
            let parameters = |forest: &SyntaxTrees| {
                let mut headers = forest.root_items().filter_map(|item| {
                    let (kind, declaration, parameters) = match item {
                        Item::Data(data) => (SymbolKind::Data, &data.name, data.type_parameters),
                        Item::Domain(domain) => {
                            (SymbolKind::Domain, &domain.name, domain.type_parameters)
                        }
                        Item::TypeAlias(alias) => {
                            (SymbolKind::TypeAlias, &alias.name, alias.type_parameters)
                        }
                        Item::Machine(machine) => {
                            (SymbolKind::Machine, &machine.name, machine.type_parameters)
                        }
                        Item::Trait(definition) => (
                            SymbolKind::Trait,
                            &definition.name,
                            definition.type_parameters,
                        ),
                        Item::Conformance(definition) => (
                            SymbolKind::Conformance,
                            definition.alias.as_ref()?,
                            definition.type_parameters,
                        ),
                        _ => return None,
                    };
                    (kind == selected_kind
                        && declaration.source_span() == reference
                        && declaration.as_str(forest) == selected_name)
                        .then_some(parameters)
                });
                let Some(result) = headers.next() else {
                    return Ok(None);
                };
                if headers.next().is_some() {
                    return Err(());
                }
                Ok(Some(
                    forest
                        .items
                        .type_parameters(result)
                        .iter()
                        .map(|parameter| match parameter.kind {
                            syntax_trees::item::TypeParameterKind::Type => {
                                GenericParameterRole::Type
                            }
                            syntax_trees::item::TypeParameterKind::Implementation => {
                                GenericParameterRole::Implementation
                            }
                            syntax_trees::item::TypeParameterKind::Const { .. } => {
                                GenericParameterRole::Const
                            }
                            syntax_trees::item::TypeParameterKind::Value { .. } => {
                                GenericParameterRole::Value
                            }
                            syntax_trees::item::TypeParameterKind::Machine { .. } => {
                                GenericParameterRole::Machine
                            }
                            syntax_trees::item::TypeParameterKind::Proposition { .. } => {
                                GenericParameterRole::Proposition
                            }
                        })
                        .collect::<Vec<_>>(),
                ))
            };
            if let Some(parameters) = parameters(syntax).ok()? {
                return Some(parameters);
            }
            for forest in declaration_syntax {
                if let Some(parameters) = parameters(forest).ok()? {
                    return Some(parameters);
                }
            }
            None
        }

        /*
            Read the receiving telescope selected in this invocation's namespace.
            Parsed headers supply carrier syntax only; they never add declarations
            or authorize evaluation. Retained headers are outside the cohort's work
            roots, so copy their operands into its tables before returning handles.
        */
        pub(super) fn const_parameter_carriers(
            &self,
            syntax: &mut SyntaxTrees,
            declaration_syntax: Option<&SyntaxTrees>,
            name: &Identifier,
            kinds: &[SymbolKind],
        ) -> Option<Vec<Option<syntax_trees::types::TypeReferenceHandle>>> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax),
                kinds,
                name.source_span(),
            )?;
            let reference = self.symbols.symbol_source_span(selected)?;
            let selected_kind = self.symbols.get(selected).kind;
            let selected_name = self.symbols.name(selected);
            let parameters = |forest: &SyntaxTrees| {
                let mut declarations = forest.root_items().filter_map(|item| {
                    let (kind, declaration, parameters, evidence_count) = match item {
                        Item::Data(data) => (SymbolKind::Data, &data.name, data.type_parameters, 0),
                        Item::TypeAlias(alias) => {
                            (SymbolKind::TypeAlias, &alias.name, alias.type_parameters, 0)
                        }
                        Item::Trait(definition) => (
                            SymbolKind::Trait,
                            &definition.name,
                            definition.type_parameters,
                            0,
                        ),
                        Item::Machine(machine) => (
                            SymbolKind::Machine,
                            &machine.name,
                            machine.type_parameters,
                            machine.conformance_bounds.len(),
                        ),
                        Item::Conformance(conformance) => (
                            SymbolKind::Conformance,
                            conformance.alias.as_ref()?,
                            conformance.type_parameters,
                            0,
                        ),
                        _ => return None,
                    };
                    (kind == selected_kind
                        && declaration.source_span() == reference
                        && declaration.as_str(forest) == selected_name)
                        .then_some((parameters, evidence_count))
                });
                let Some((parameters, evidence_count)) = declarations.next() else {
                    return Ok(None);
                };
                if declarations.next().is_some() {
                    return Err(());
                }
                let mut carriers = syntax_items(forest)
                    .type_parameters(parameters)
                    .iter()
                    .map(|parameter| match parameter.kind {
                        syntax_trees::item::TypeParameterKind::Const { type_reference } => {
                            Some(type_reference)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                carriers.resize(carriers.len() + evidence_count, None);
                Ok(Some(carriers))
            };
            if let Some(parameters) = parameters(syntax).ok()? {
                return Some(parameters);
            }
            let forest = declaration_syntax?;
            Some(
                parameters(forest)
                    .ok()??
                    .into_iter()
                    .map(|carrier| {
                        carrier.map(|carrier| syntax.copy_type_reference_handle(forest, carrier))
                    })
                    .collect(),
            )
        }

        pub(crate) fn constructor<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            name: &Identifier,
        ) -> Result<(&'syntax DataDefinition, Option<Identifier>), String> {
            let (selected, case) =
                constructor_type(&self.symbols, name.as_str(syntax), name.source_span())?
                    .ok_or_else(|| {
                        format!(
                            "`{name}` does not select one declared constructor carrier",
                            name = name.as_str(syntax)
                        )
                    })?;
            let definition = self.data_declaration(syntax, name, selected)?;
            let case = case.map(|case| Identifier::new(case, name.trailing_span(case.len())));
            Ok((definition, case))
        }

        pub(crate) fn builtin_type(
            &self,
            syntax_trees: &syntax_trees::SyntaxTrees,
            name: &Identifier,
        ) -> Option<symbols::BuiltinTypeAtom> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax_trees),
                &[SymbolKind::BuiltinType, SymbolKind::Data],
                name.source_span(),
            )?;
            self.symbols.builtin_type_atom(selected)
        }

        pub(super) fn type_declaration_kind(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Result<Option<SymbolKind>, Diagnostic> {
            match self
                .symbols
                .lookup_top_level_by_name_and_kinds_from_source_matching(
                    name.as_str(syntax),
                    &[
                        SymbolKind::BuiltinType,
                        SymbolKind::Data,
                        SymbolKind::Domain,
                        SymbolKind::TypeAlias,
                        SymbolKind::Trait,
                    ],
                    name.source_span(),
                    |_| true,
                ) {
                symbols::SymbolLookup::Unique(symbol) => Ok(Some(self.symbols.get(symbol).kind)),
                symbols::SymbolLookup::NotFound => Ok(None),
                symbols::SymbolLookup::Ambiguous { .. } => Err(Diagnostic::error(format!(
                    "type path owner `{}` is ambiguous",
                    name.as_str(syntax)
                ))
                .with_source_span(name.source_span())),
            }
        }

        /*
            Select one trait header in the authored occurrence's source context.
            The lookup distinguishes unique, absent, and ambiguous selection so the
            caller can reject ambiguity rather than guessing a same-leaf template.
        */
        pub(crate) fn trait_lookup(
            &self,
            syntax_trees: &syntax_trees::SyntaxTrees,
            name: &Identifier,
        ) -> symbols::SymbolLookup {
            self.symbols
                .lookup_top_level_by_name_and_kinds_from_source_matching(
                    name.as_str(syntax_trees),
                    &[SymbolKind::Trait],
                    name.source_span(),
                    |_| true,
                )
        }

        /*
            The selected declaration's logical namespace path, for generated
            references that must re-resolve in another source's import scope.
        */
        pub(crate) fn declaration_path(&self, symbol: symbols::SymbolHandle) -> String {
            self.symbols.display_path(symbol, "::")
        }

        /*
            Re-expose a normalized constant's exact retained declaration in the
            original occurrence's lexical scope. Coordinates here are the existing
            normalization custody join, not a new name-based semantic identity.
            Every proposed qualified/imported spelling must select that same header.
        */
        pub(super) fn retained_const_name(
            &self,
            syntax: &SyntaxTrees,
            origin: &syntax_trees::types::ConstArgumentOrigin,
        ) -> Result<Identifier, Diagnostic> {
            let reject = || {
                Diagnostic::error("inferred constant lost its exact retained declaration selection")
                    .with_source_span(origin.reference)
            };
            let mut selected =
                self.symbols
                    .symbols()
                    .nodes()
                    .iter()
                    .filter_map(|(handle, symbol)| {
                        (symbol.kind == SymbolKind::Const
                            && self.symbols.symbol_source_span(handle) == Some(origin.declaration))
                        .then_some(handle)
                    });
            let selected_header = selected.next().ok_or_else(reject)?;
            if selected.next().is_some() {
                return Err(reject());
            }
            let selects = |path: &str| {
                self.symbols.find_top_level_by_name_and_kinds_from_source(
                    path,
                    &[SymbolKind::Const],
                    origin.reference,
                ) == Some(selected_header)
            };
            let path = self.declaration_path(selected_header);
            let mut candidates = vec![path.clone()];
            for item in syntax.root_items() {
                let Item::Use(import) = item else {
                    continue;
                };
                let members = syntax.items.identifier_path_members(import.path);
                if !members.first().is_some_and(|member| {
                    member.source_span().source_id == origin.reference.source_id
                }) {
                    continue;
                }
                let mut prefix = String::new();
                for member in members {
                    if !prefix.is_empty() {
                        prefix.push_str("::");
                    }
                    prefix.push_str(member.as_str(syntax));
                    candidates.push(format!("{prefix}::{path}"));
                }
            }
            candidates.push(self.symbols.name(selected_header).to_owned());
            for candidate in candidates {
                if !selects(&candidate) {
                    continue;
                }
                let name = Identifier::new(candidate, origin.reference);
                let definition = self.select(syntax, &name)?.ok_or_else(reject)?;
                if definition.name.source_span() != origin.declaration
                    || syntax.expressions.source_span(definition.value) != origin.initializer
                {
                    return Err(reject());
                }
                return Ok(name);
            }
            Err(
                Diagnostic::error("inferred constant has no exact source-visible reference")
                    .with_source_span(origin.reference),
            )
        }

        /*
            Whether one authored name selects an exact constant declaration in its
            own source scope. Private or ambiguous candidates still count as
            constants here: the leaf's checked admission and package custody own
            the legality verdict, while this lookup only distinguishes a value
            reference from a literal case or constructor spelling.
        */
        pub(crate) fn selects_const(
            &self,
            syntax_trees: &syntax_trees::SyntaxTrees,
            name: &Identifier,
        ) -> bool {
            self.symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax_trees),
                    &[SymbolKind::Const],
                    name.source_span(),
                )
                .is_some()
        }

        /*
            Bare Names retain value-prefix precedence before specialization changes
            their lookup metadata. The shared resolver distinguishes absence from
            unique or ambiguous constants; braces remain ordinary constructors.
        */
        pub(super) fn constructor_prefix_is_value(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> bool {
            crate::symbols::constructor_prefix_is_value(
                &self.symbols,
                name.as_str(syntax),
                name.source_span(),
            )
            .unwrap_or(true)
        }

        pub(crate) fn qualified_payload_free_case<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            name: &Identifier,
        ) -> Result<Option<(&'syntax DataDefinition, Identifier)>, String> {
            let Some((owner, _, case)) = crate::symbols::qualified_payload_free_case_type(
                &self.symbols,
                name.as_str(syntax),
                name.source_span(),
            )?
            else {
                return Ok(None);
            };
            let definition = self.data_declaration(syntax, name, owner)?;
            Ok(Some((
                definition,
                Identifier::new(case, name.trailing_span(case.len())),
            )))
        }

        fn data_declaration<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            name: &Identifier,
            selected: symbols::SymbolHandle,
        ) -> Result<&'syntax DataDefinition, String> {
            self.data_declaration_among(syntax, None, name, selected)
        }

        fn data_declaration_among<'syntax>(
            &self,
            syntax: &'syntax SyntaxTrees,
            roots: Option<&RootDeclarations<'_>>,
            name: &Identifier,
            selected: symbols::SymbolHandle,
        ) -> Result<&'syntax DataDefinition, String> {
            let declaration = self.source_declaration_item_among(syntax, roots, name, selected)?;
            syntax
                .root_item(declaration)
                .as_data()
                .ok_or_else(|| "selected nominal carrier is not a data declaration".to_owned())
        }

        /*
            Select an implementation in the authored argument's own namespace.
            Return its live syntax identity, never a transient header symbol.
        */
        pub(super) fn implementation_item(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Result<syntax_trees::item::ItemHandle, String> {
            let selected = self
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(syntax),
                    &[SymbolKind::Conformance],
                    name.source_span(),
                )
                .ok_or_else(|| {
                    "implementation argument does not select one declaration".to_owned()
                })?;
            self.source_declaration_item(syntax, name, selected)
        }

        fn source_declaration_item(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
            selected: symbols::SymbolHandle,
        ) -> Result<syntax_trees::item::ItemHandle, String> {
            self.source_declaration_item_among(syntax, None, name, selected)
        }

        /*
            `source_declaration_item`, reading candidates from `roots` when the
            caller indexed the forest once for many lookups. The index only
            narrows candidates to the selected name span; every candidate passes
            the same kind, span and spelling test in root order.
        */
        fn source_declaration_item_among(
            &self,
            syntax: &SyntaxTrees,
            roots: Option<&RootDeclarations<'_>>,
            name: &Identifier,
            selected: symbols::SymbolHandle,
        ) -> Result<syntax_trees::item::ItemHandle, String> {
            let span = self
                .symbols
                .symbol_source_span(selected)
                .ok_or_else(|| "selected canonical declaration has no source".to_owned())?;
            let kind = self.symbols.get(selected).kind;
            let candidates: Box<dyn Iterator<Item = syntax_trees::item::ItemHandle> + '_> =
                match roots {
                    Some(roots) if std::ptr::eq(roots.syntax, syntax) => Box::new(roots.at(span)),
                    _ => Box::new(syntax.root_item_handles().iter().copied()),
                };
            let mut declarations = candidates.filter_map(|handle| {
                let (declaration_kind, declaration_name, is_public) = match syntax.root_item(handle)
                {
                    Item::Data(data) => (SymbolKind::Data, &data.name, data.is_public),
                    Item::Conformance(implementation) => (
                        SymbolKind::Conformance,
                        implementation.alias.as_ref()?,
                        implementation.is_public,
                    ),
                    _ => return None,
                };
                (declaration_kind == kind
                    && declaration_name.source_span() == span
                    && declaration_name.as_str(syntax) == self.symbols.name(selected))
                .then_some((handle, is_public))
            });
            let (declaration, is_public) = declarations.next().ok_or_else(|| {
                "selected canonical declaration lost its source custody".to_owned()
            })?;
            if declarations.next().is_some() {
                return Err(
                    "selected canonical declaration has ambiguous source custody".to_owned(),
                );
            }
            if !is_public && !self.symbols.same_source_package(name.source_span(), span) {
                return Err(format!(
                    "canonical declaration `{}` is private in another package",
                    name.as_str(syntax)
                ));
            }
            Ok(declaration)
        }

        /*
            Find a lookup spelling for the already selected template. This is not
            identity: every candidate is checked against the same selected header.
            Keep requester imports and the use span; never relocate a use to its
            declaration merely to make generated instance lookup succeed.
        */
        pub(super) fn data_lookup_path(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Option<String> {
            self.declaration_lookup_path(syntax, name, SymbolKind::Data)
        }

        pub(super) fn declaration_lookup_path(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
            kind: SymbolKind,
        ) -> Option<String> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax),
                &[kind],
                name.source_span(),
            )?;
            self.lookup_path_for_selected(syntax, name, selected, kind)
        }

        pub(super) fn data_lookup_path_for_declaration(
            &self,
            syntax: &SyntaxTrees,
            declaration: &Identifier,
            requester: &Identifier,
        ) -> Option<String> {
            let selected = self.symbols.find_top_level_by_name_and_kinds_from_source(
                declaration.as_str(syntax),
                &[SymbolKind::Data],
                declaration.source_span(),
            )?;
            if self.symbols.symbol_source_span(selected) != Some(declaration.source_span()) {
                return None;
            }
            self.lookup_path_for_selected(syntax, requester, selected, SymbolKind::Data)
        }

        fn lookup_path_for_selected(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
            selected: symbols::SymbolHandle,
            kind: SymbolKind,
        ) -> Option<String> {
            let path = self.symbols.display_path(selected, "::");
            let declaration = self.symbols.symbol_source_span(selected)?;
            let selects = |path: &str| {
                self.symbols.find_top_level_by_name_and_kinds_from_source(
                    path,
                    &[kind],
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
            if name.as_str(syntax).contains("::") && selects(name.as_str(syntax)) {
                return Some(name.as_str(syntax).to_owned());
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
                    prefix.push_str(member.as_str(syntax));
                    let candidate = format!("{prefix}::{path}");
                    if selects(&candidate) {
                        return Some(candidate);
                    }
                }
            }
            selects(name.as_str(syntax)).then(|| name.as_str(syntax).to_owned())
        }

        /*
            Return exact declaration custody, not a handle from the header table.
            No match includes unresolved or ambiguous selection; callers must defer
            it to ordinary validation, never fall back to a lexical constant map.
        */
        pub(crate) fn select(
            &self,
            syntax: &SyntaxTrees,
            name: &Identifier,
        ) -> Result<Option<ConstDefinition>, Diagnostic> {
            if name.source_span().span.start == name.source_span().span.end {
                return Ok(None);
            }
            let Some(selected) = self.symbols.find_top_level_by_name_and_kinds_from_source(
                name.as_str(syntax),
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
                    name = name.as_str(syntax)
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
            syntax_items(syntax)
                .data_members(definition.members)
                .iter()
                .filter_map(|member| {
                    let DataMember::Variant(variant) = member else {
                        return None;
                    };
                    Some((
                        SymbolKind::Variant,
                        SymbolNameRef::OwnedSource {
                            value: variant.name.as_str(syntax),
                            source_span: variant.name.source_span(),
                        },
                    ))
                }),
        );
    }

    /*
        The argument bindings one closed generic application establishes for its
        selected template's parameters: `Pair<u64>` binds `T` to the `u64` spelling.
        Bindings only name arguments that already exist as handles; a member type
        still spelling an open parameter resolves one application level at a time
        rather than materializing a substituted handle inside an immutable forest.
    */
    pub(crate) type GenericApplicationSubstitution =
        std::collections::HashMap<String, syntax_trees::types::TypeReferenceHandle>;

    /*
        Whether `type_reference` still spells one of the bound parameter names.
        Only structural positions are scanned; an expression-bearing position (a
        deferred const argument or a range bound) conservatively counts as open
        whenever any binding exists, because its expression may name a parameter.
    */
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
            TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str(syntax)),
            TypeReferenceNode::Generic { arguments, .. } => syntax_type_references(syntax)
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| type_mentions_parameters(syntax, *argument, substitution)),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                type_mentions_parameters(syntax, *element_type, substitution)
                    || matches!(length, FixedArrayLength::ConstParameter(name) if substitution.contains_key(name.as_str(syntax)))
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                type_mentions_parameters(syntax, *base_type, substitution)
                    || syntax_type_references(syntax)
                        .constraints(*constraints)
                        .iter()
                        .any(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                                .type_reference_handles(domain.arguments)
                                .iter()
                                .any(|argument| {
                                    type_mentions_parameters(syntax, *argument, substitution)
                                }),

                            TypeConstraintNode::Named(_)
                            | TypeConstraintNode::ArithmeticDomain(_) => false,
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

    /*
        Resolve one generic argument spelling through the enclosing application's
        bindings. A bare parameter name reselects its already-closed argument
        handle; a composite spelling that still names a parameter cannot
        materialize a substituted handle inside an immutable forest and declines.
    */
    pub(crate) fn resolved_generic_argument(
        syntax: &SyntaxTrees,
        argument: syntax_trees::types::TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<syntax_trees::types::TypeReferenceHandle, String> {
        if let Some(name) = syntax.type_references.type_reference(argument).as_named()
            && let Some(resolved) = substitution.get(name.as_str(syntax))
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

    /*
        Builds the substitution a closed generic application yields: every
        parameter name bound to its resolved argument handle.
    */
    pub(crate) fn substituted_generic_arguments(
        syntax: &SyntaxTrees,
        parameters: &[syntax_trees::item::TypeParameter],
        argument_handles: &[syntax_trees::types::TypeReferenceHandle],
        substitution: &GenericApplicationSubstitution,
    ) -> Result<GenericApplicationSubstitution, String> {
        let mut next_substitution = GenericApplicationSubstitution::new();
        for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
            next_substitution.insert(
                parameter.name.as_str(syntax).to_owned(),
                resolved_generic_argument(syntax, *argument, substitution)?,
            );
        }
        Ok(next_substitution)
    }

    /*
        Whether `type_reference` contains a deferred `ConstExpression` argument.
        Probe forests replace those arguments with layout stand-ins, so a carrier
        spelling containing one drifts from the materialized constructor's name;
        closed leaf destinations must be probe-stable.
    */
    pub(crate) fn has_deferred_const_argument(
        syntax: &SyntaxTrees,
        type_reference: syntax_trees::types::TypeReferenceHandle,
    ) -> bool {
        use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};
        match syntax.type_references.type_reference(type_reference) {
            TypeReferenceNode::ConstExpression(_) => true,
            TypeReferenceNode::Generic { arguments, .. } => syntax_type_references(syntax)
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
                    || syntax_type_references(syntax)
                        .constraints(*constraints)
                        .iter()
                        .any(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                                .type_reference_handles(domain.arguments)
                                .iter()
                                .any(|argument| has_deferred_const_argument(syntax, *argument)),
                            _ => false,
                        })
            }
            _ => false,
        }
    }

    /*
        Nominal header selections for pre-resolution consumers. Transient symbols
        stay private; each result rejoins the exact original declaration by source
        custody. This operation supplies no type, placement or execution authority.
    */
    pub fn select_data_declarations(
        syntax: &SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
        references: &[Identifier],
        retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    ) -> Result<Vec<Result<source::SourceSpan, String>>, Vec<Diagnostic>> {
        select_data_declarations_with_retained(
            syntax,
            None,
            sources,
            bindings,
            references,
            retained_base,
        )
    }

    /*
        Like [`select_data_declarations`], but a declaration that ordinary selection
        resolves into the retained predecessor's parsed forest (a unit this cohort
        imports without parsing again, such as `core::layout`) is accepted by its
        exact span and name instead of being reported as lost custody. The forest
        supplies no authority: selection still ran through the receiving namespace.
    */
    pub fn select_data_declarations_with_retained(
        syntax: &SyntaxTrees,
        retained: Option<&SyntaxTrees>,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
        references: &[Identifier],
        retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    ) -> Result<Vec<Result<source::SourceSpan, String>>, Vec<Diagnostic>> {
        let selection =
            ConstantSelection::with_retained_base(syntax, sources, bindings, retained_base)?;
        let roots = RootDeclarations::of(syntax);
        Ok(references
            .iter()
            .map(|reference| selection.data_span_among(syntax, Some(&roots), retained, reference))
            .collect())
    }

    /*
        Root data and conformance declarations of one borrowed forest, ordered by
        their name span. The borrow keeps the forest unchanged while the index
        lives; ties keep root order.
    */
    pub(crate) struct RootDeclarations<'syntax> {
        syntax: &'syntax SyntaxTrees,
        rows: Vec<((usize, usize, usize), usize, syntax_trees::item::ItemHandle)>,
    }

    impl<'syntax> RootDeclarations<'syntax> {
        pub(crate) fn syntax(&self) -> &'syntax SyntaxTrees {
            self.syntax
        }

        pub(crate) fn of(syntax: &'syntax SyntaxTrees) -> Self {
            let mut rows = syntax
                .root_item_handles()
                .iter()
                .enumerate()
                .filter_map(|(position, handle)| {
                    let name = match syntax.root_item(*handle) {
                        Item::Data(data) => &data.name,
                        Item::Conformance(implementation) => implementation.alias.as_ref()?,
                        _ => return None,
                    };
                    Some((span_key(name.source_span()), position, *handle))
                })
                .collect::<Vec<_>>();
            rows.sort_unstable_by_key(|(key, position, _)| (*key, *position));
            Self { syntax, rows }
        }

        /* Root data and conformance items named at `span`, in root order. */
        pub(crate) fn at(
            &self,
            span: source::SourceSpan,
        ) -> impl Iterator<Item = syntax_trees::item::ItemHandle> + '_ {
            let key = span_key(span);
            let start = self.rows.partition_point(|(row, _, _)| *row < key);
            self.rows[start..]
                .iter()
                .take_while(move |(row, _, _)| *row == key)
                .map(|(_, _, handle)| *handle)
        }
    }

    fn span_key(span: source::SourceSpan) -> (usize, usize, usize) {
        (span.source_id.0, span.span.start, span.span.end)
    }

    /*
        Only the deferred frontier's owned predecessor is persistent. Normalization's
        temporary borrow cannot escape as an independently restorable source owner.
    */
    impl compiler_storage::Record for SelectionBase<'_> {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            match self {
                Self::Shared(base) => compiler_storage::Record::write(base, output),
                Self::Borrowed(_) => {
                    Err(output.error("borrowed declaration selection is not a persistent frontier"))
                }
            }
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            compiler_storage::Record::read(input).map(Self::Shared)
        }
    }

    #[cfg(test)]
    mod retained_namespace_tests {
        use super::*;
        use arena::HandleSpan;
        use source::{SourceId, SourceSpan, Span};
        use syntax_trees::item::{ModuleDeclaration, UseItem};

        #[test]
        fn early_declaration_selection_retains_authored_namespace_names() {
            let mut syntax = SyntaxTrees::new(SourceId(1));
            let page = syntax.text.retain(Arc::from("example imported"));
            let module = syntax
                .items
                .append_identifier_path_member(Identifier::source_backed(
                    page,
                    SourceSpan::new(SourceId(1), Span::new(0, 7)),
                ));
            let import = syntax
                .items
                .append_identifier_path_member(Identifier::source_backed(
                    page,
                    SourceSpan::new(SourceId(1), Span::new(8, 16)),
                ));
            syntax.push_root_item(Item::Module(ModuleDeclaration {
                path: HandleSpan::from_parts(module, 1),
            }));
            syntax.push_root_item(Item::Use(UseItem {
                path: HandleSpan::from_parts(import, 1),
            }));
            ConstantSelection::new(&syntax, None, Vec::new())
                .expect("namespace registration retains source ownership before cloning names");
        }
    }
}
#[path = "preparation_callable_types.rs"]
mod callable_types;
#[path = "preparation_generic_data.rs"]
pub(crate) mod generic_data;
#[path = "preparation_machine_equations.rs"]
pub(crate) mod machine_equations;
pub(crate) mod module_normalization {
    /*
        Narrow rejection boundaries for transforms which precede module identity.

        Literal arrays of primitive scalars carry no nominal initializer names to
        normalize. They can use the same exact constant-header selection and canonical
        value route as scalars; lexical and package custody still precede publication.
        Every newly admitted array declaration is also checked by the existing
        canonicalizer, including unused private declarations: unused initializers
        never reach destination checking. Array leaves
        therefore stay within its canonical integer/Boolean subset. Scoped literals
        additionally retain their exact module-local or imported owner declaration
        at constant finalization, where complete symbols exist. The attachment owner
        may be generic; value-carrier normalization independently closes the declared
        type without replacing the owner's identity. Scalar substitution then uses
        resolved declaration identity and the declared numeric landing, just as for
        unscoped constants. Primitive arrays use this same selection for body copies,
        preserving their declared element landings and full array type at destinations.
        Nominal constants use the same structural encoder after the
        shared header resolver selects their carrier and every nested constructor
        in its declaring source. Their uses retain exact declaration custody and
        rejoin the receiving parameter after symbol allocation; equal layouts and
        encoded labels never grant nominal identity. Scoped nominal constants use
        the same exact attachment finalization as scoped scalars. A generic carrier
        — `Box<u64>` under any fixed-array layers — cannot hold a canonical
        const-index identity before its closed instance exists, so those
        declarations defer value admission to lowering exactly as root constants
        do. Their base template still selects one generic data declaration in the
        declaring source at this boundary; unselected or nongeneric spellings
        reject here rather than drifting to a later stage's weaker error.
    */

    use crate::constant::validate_scalar_initializer;
    use crate::preparation::generic_data::canonicalize_declared_const_definition;
    use crate::preparation::generic_data::canonicalize_selected_declared_const_definition;
    use crate::resolution::lowerer::ConstResolutionMode;
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use diagnostics::Diagnostic;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::ConstDefinition;
    use syntax_trees::item::Item;
    use syntax_trees::types::FixedArrayLength;
    use syntax_trees::types::TypeReferenceNode;

    pub(crate) fn validate_with_selection(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_with_const_resolution_mode(syntax, selection, ConstResolutionMode::Complete)
    }

    pub(crate) fn validate_with_const_resolution_mode(
        syntax: &SyntaxTrees,
        selection: &crate::preparation::declaration_selection::ConstantSelection,
        mode: ConstResolutionMode,
    ) -> Result<(), Vec<Diagnostic>> {
        let module_sources = syntax
            .root_items()
            .filter_map(|item| {
                let Item::Module(module) = item else {
                    return None;
                };
                syntax_items(syntax)
                    .identifier_path_members(module.path)
                    .first()
                    .map(|member| member.source_span().source_id)
            })
            .collect::<Vec<_>>();
        if module_sources.is_empty() {
            return Ok(());
        }
        for item in syntax.root_items() {
            let Item::Const(constant) = item else {
                continue;
            };
            if !module_sources.contains(&constant.name.source_span().source_id) {
                continue;
            }
            if mode != ConstResolutionMode::Complete {
            } else if module_literal_constant(syntax, constant) {
                if matches!(
                    syntax_type_references(syntax).type_reference(constant.type_reference),
                    TypeReferenceNode::FixedArray { .. }
                ) {
                    canonicalize_declared_const_definition(syntax, constant).map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module array constant `{}` is invalid: {reason}",
                                constant.name.as_str(syntax)
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    })?;
                } else {
                    validate_scalar_initializer(syntax, constant).map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module scalar constant `{}` is invalid: {reason}",
                                constant.name.as_str(syntax)
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    })?;
                }
            } else if let Some(base_name) = generic_const_carrier_leaf(syntax, constant) {
                let selected = selection.data(syntax, base_name).and_then(|definition| {
                    if definition.type_parameters.is_empty() {
                        Err(format!(
                            "`{base_name}` does not select a generic data template",
                            base_name = base_name.as_str(syntax)
                        ))
                    } else {
                        Ok(())
                    }
                });
                selected.map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "module-owned nominal constant `{}` is invalid: {reason}",
                            constant.name.as_str(syntax)
                        ))
                        .with_source_span(constant.name.source_span()),
                    ]
                })?;
            } else {
                canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                    .map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module-owned nominal constant `{}` is invalid: {reason}",
                                constant.name.as_str(syntax)
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    })?;
            }
        }

        Ok(())
    }

    pub(crate) fn module_literal_constant(
        syntax: &SyntaxTrees,
        constant: &ConstDefinition,
    ) -> bool {
        use syntax_trees::expression::ExpressionNode;
        if !scalar_literal_tree(syntax, constant.value) {
            return false;
        }
        let mut type_reference = constant.type_reference;
        let is_array = matches!(
            syntax.type_references.type_reference(type_reference),
            TypeReferenceNode::FixedArray { .. }
        );
        if !is_array
            && matches!(
                syntax.expressions.expression(constant.value),
                ExpressionNode::ArrayLiteral(_)
            )
        {
            return false;
        }
        loop {
            match syntax.type_references.type_reference(type_reference) {
                TypeReferenceNode::Named(name) => {
                    return crate::constant::is_structural_scalar_carrier(name.as_str(syntax))
                        || (!is_array && matches!(name.as_str(syntax), "f32" | "f64"));
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(_),
                } => type_reference = *element_type,
                _ => return false,
            }
        }
    }

    /*
        The base spelling of a module constant's generic carrier once fixed-array
        layers are peeled: `Box<u64>` and `[Box<u64>; 2]` share this gate. Any
        other leaf — named, constrained, builtin — keeps its existing owner.
    */
    fn generic_const_carrier_leaf<'a>(
        syntax: &'a SyntaxTrees,
        constant: &ConstDefinition,
    ) -> Option<&'a Identifier> {
        let mut carrier = constant.type_reference;
        loop {
            carrier = match syntax.type_references.type_reference(carrier) {
                TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
                TypeReferenceNode::Generic { base_name, .. } => return Some(base_name),
                _ => return None,
            };
        }
    }

    fn scalar_literal_tree(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        use syntax_trees::expression::ExpressionNode;
        match syntax.expressions.expression(expression) {
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::String(_) => true,
            ExpressionNode::ArrayLiteral(elements) => syntax_expressions(syntax)
                .expression_handles(*elements)
                .iter()
                .all(|element| scalar_literal_tree(syntax, *element)),
            _ => false,
        }
    }
}
pub(crate) mod target_families {
    /*
        Give product families stable source identities before assigning symbols.

        Every authored body keeps its exact target identity. A separate bodyless
        Requirement occupies the portable path, so a portable call and a private
        same-target call can never accidentally share an executable identity.
        Ordinary resolution gives the interface its own telescope; the resolved
        family owner joins contracts after member symbols have settled.
        Build helpers arrive with a separately admitted execution selection and
        are not changed here. No native target catalog participates in this pass.
    */

    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use source::{DependencyScope, SourceFile, SourceMap};
    use std::cmp::Ordering;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{Item, ItemHandle};

    struct Member<'source> {
        handle: ItemHandle,
        name: &'source Identifier,
        target: &'source Identifier,
        file: Option<&'source SourceFile>,
        module: &'source [Identifier],
    }

    impl Member<'_> {
        fn compare_family(
            &self,
            syntax_trees: &syntax_trees::SyntaxTrees,
            other: &Self,
        ) -> Ordering {
            let package = self.file.and_then(|file| file.package_identity);
            let other_package = other.file.and_then(|file| file.package_identity);
            package
                .cmp(&other_package)
                .then_with(|| self.is_build().cmp(&other.is_build()))
                .then_with(|| {
                    self.file
                        .filter(|_| package.is_none())
                        .map(|file| &file.package_root)
                        .cmp(
                            &other
                                .file
                                .filter(|_| other_package.is_none())
                                .map(|file| &file.package_root),
                        )
                })
                .then_with(|| {
                    self.module
                        .iter()
                        .map(|name| name.as_str(syntax_trees))
                        .cmp(other.module.iter().map(|name| name.as_str(syntax_trees)))
                })
                .then_with(|| {
                    self.name
                        .as_str(syntax_trees)
                        .cmp(other.name.as_str(syntax_trees))
                })
        }

        fn is_build(&self) -> bool {
            self.file
                .is_some_and(|file| file.dependency_scope == DependencyScope::Build)
        }
    }

    pub(crate) fn normalize(
        syntax: &mut SyntaxTrees,
        sources: Option<&SourceMap>,
    ) -> Result<(), Vec<Diagnostic>> {
        let modules = syntax
            .root_items()
            .filter_map(|item| {
                let Item::Module(module) = item else {
                    return None;
                };
                let path = syntax.items.identifier_path_members(module.path);
                Some((path.first()?.source_span().source_id, path))
            })
            .collect::<Vec<_>>();
        let mut members = Vec::new();
        for &handle in syntax.root_item_handles() {
            let Some(machine) = syntax.root_item(handle).as_machine() else {
                continue;
            };
            let Some(target) = machine.authored_target.as_ref().or(machine.target.as_ref()) else {
                continue;
            };
            let source = machine.name.source_span().source_id;
            let file = sources
                .map(|sources| {
                    sources.get(source).ok_or_else(|| {
                        vec![
                            Diagnostic::error(
                                "target declaration has no source in the supplied source map",
                            )
                            .with_source_span(machine.name.source_span()),
                        ]
                    })
                })
                .transpose()?;
            members.push(Member {
                handle,
                name: &machine.name,
                target,
                file,
                module: modules
                    .iter()
                    .find(|(owner, _)| *owner == source)
                    .map(|(_, path)| *path)
                    .unwrap_or_default(),
            });
        }
        members.sort_by(|left, right| {
            left.compare_family(syntax, right)
                .then_with(|| left.target.as_str(syntax).cmp(right.target.as_str(syntax)))
        });
        let mut declarations = Vec::with_capacity(members.len());
        let mut interfaces = Vec::new();
        for family in members.chunk_by(|left, right| left.compare_family(syntax, right).is_eq()) {
            for same_target in family
                .chunk_by(|left, right| left.target.as_str(syntax) == right.target.as_str(syntax))
            {
                if same_target.len() > 1 {
                    return Err(vec![Diagnostic::error(format!(
                    "machine `{}` is implemented twice for `{}` -- a target supplies exactly one implementation of a contract machine",
                    same_target[0].name.as_str(syntax), same_target[0].target.as_str(syntax),
                )).with_source_span(same_target[1].name.source_span())]);
                }
            }
            if family[0].is_build() {
                continue;
            }
            interfaces.push(family[0].handle);
            declarations.extend(
                family
                    .iter()
                    .map(|member| (member.handle, Some(member.target.clone()))),
            );
        }
        drop(members);
        for prototype in interfaces {
            let Some(machine) = syntax.root_item(prototype).as_machine() else {
                continue;
            };
            if syntax.root_items().any(|item| {
                item.as_machine().is_some_and(|candidate| {
                    candidate.target_family_interface
                        && candidate.name.source_span() == machine.name.source_span()
                        && candidate.name.as_str(syntax) == machine.name.as_str(syntax)
                })
            }) {
                continue;
            }
            let mut interface = machine.clone();
            let Some(&entry) = syntax.items.state_handles(interface.states).first() else {
                return Err(vec![
                    Diagnostic::error("target family has no entry signature")
                        .with_source_span(interface.name.source_span()),
                ]);
            };
            let signature = syntax.items.state(entry);
            let entry = syntax_trees::item::State {
                name: signature.name.clone(),
                parameters: signature.parameters,
                return_type: signature.return_type,
                statements: HandleSpan::empty(),
                contracts: HandleSpan::empty(),
            };
            let entry = syntax.items.insert_state(&entry);
            let entry = syntax.items.append_state_handle(entry);
            interface.states = HandleSpan::from_parts(entry, 1);
            interface.nested_machines = HandleSpan::empty();
            interface.target_family_interface = true;
            interface.bodyless = true;
            interface.target = None;
            interface.authored_target = None;
            interface.boundary = false;
            interface.satisfies = HandleSpan::empty();
            interface.contracts = HandleSpan::empty();
            interface.terminates_guarantee = false;
            interface.service_reach_keyword_source_spans.clear();
            interface.service_reaches = HandleSpan::empty();
            interface.service_reach_is_installation_bound = false;
            interface.invokes = HandleSpan::empty();
            interface.suspends_keyword_source_spans.clear();
            interface.blocks_keyword_source_spans.clear();
            interface.suspends = false;
            interface.blocks = false;
            interface.ranking_subjects = HandleSpan::empty();
            interface.ranking_view = HandleSpan::empty();
            interface.ranking_view_arguments = HandleSpan::empty();
            interface.ranking_range = syntax_trees::expression::ExpressionHandle::invalid();
            syntax.push_root_item(Item::Machine(interface));
        }
        for (handle, target) in declarations {
            let Some(machine) = syntax.root_item(handle).as_machine() else {
                continue;
            };
            if machine.target.as_ref().map(|name| name.as_str(syntax))
                == target.as_ref().map(|name| name.as_str(syntax))
                && machine.authored_target.is_some()
            {
                continue;
            }
            let mut machine = machine.clone();
            machine.authored_target = machine.authored_target.or(machine.target);
            machine.target = target;
            syntax.items.replace_item(handle, Item::Machine(machine));
        }
        Ok(())
    }
}
pub(crate) mod trait_defaults {
    /*
        Pre-resolution synthesis for trait machines with authored bodies, and for
        the sealed core `Equatable<T>` row.

        A named implementation may omit a requirement whose trait supplies a
        default body. Its closed block receives each omitted default as its own
        member row before symbol resolution, so typing, effects, proofs, dispatch
        and both execution engines reuse the ordinary member path.

        A closed `Name satisfies Equatable<T> { }` block that authors no `equals`
        member receives a compiler-emitted `equals` member whose
        body compares its two operands with `==`; typed lowering expands that
        comparison structurally for `T` (core equality acquisition in
        `wiki/spec/language/conformances.md`). The row belongs to the named
        implementation, never to `T`.

        Trait identity joins by exact source selection through the
        shared constant-header table, never by leaf spelling: same-leaf module
        declarations cannot share a default template, and an ambiguous authored
        trait name defers to ordinary resolution instead of guessing an owner.
        Package-aware callers retain the source map and loader-certified import
        bindings: rebuilding a source-free header table could merge same-path
        declarations from different packages during namespace/value validation.
        Synthesized references carry the selected declaration's logical path (or
        the authored occurrence itself) so complete resolution rejoins the same
        declaration; the compiler-derived selection partition still requires
        generated spellings, so requirement edges use qualified paths that resolve
        inside the declaration's package instead of falling back to a same-leaf
        competitor.
    */

    use crate::preparation::declaration_selection::ConstantSelection;
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use source::SourceMap;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;
    use symbols::{SourceScopedTopLevelBinding, SymbolHandle};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
        StaticSymbolApplication, TableBinaryExpression,
    };
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{
        ConformanceItem, ConformanceMember, Item, ItemHandle, Machine, SatisfiesClause, State,
        StateSignature,
    };
    use syntax_trees::statement::StatementNode;
    use syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone)]
    struct TraitDefaultsInput {
        /*
            Logical namespace path of the selected trait declaration. Synthesized
            references spell this path so they rejoin the exact owner instead of a
            same-leaf competitor.
        */
        path: String,
        lifetime_parameters: Vec<Identifier>,
        parameter_names: Vec<String>,
        signatures: Vec<TraitSignatureInput>,
        requirements: Vec<TraitRequirementInput>,
    }

    #[derive(Clone)]
    struct TraitSignatureInput {
        ordinal: usize,
        signature: StateSignature,
    }

    #[derive(Clone)]
    struct TraitRequirementInput {
        /*
            The exact required trait selected in the declaring trait's own source
            context. An invalid handle means the requirement name did not select
            one trait; ordinary resolution owns that diagnostic.
        */
        symbol: SymbolHandle,
        arguments: Vec<TypeReferenceHandle>,
    }

    #[derive(Clone)]
    struct ConformanceInput {
        handle: ItemHandle,
        declaration: ConformanceItem,
    }

    #[derive(Clone)]
    struct RequirementInstance {
        declaring_trait: SymbolHandle,
        requirement_ordinal: usize,
        signature: StateSignature,
        substitution: HashMap<String, TypeReferenceHandle>,
    }

    pub fn synthesize_trait_defaults(
        syntax: &mut SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
        retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    ) -> Result<(), Vec<Diagnostic>> {
        let selection =
            ConstantSelection::with_retained_base(syntax, sources, bindings, retained_base)?;
        crate::preparation::generic_data::domain_heads::normalize(syntax, Some(&selection))?;
        crate::preparation::module_normalization::validate_with_selection(syntax, &selection)?;
        synthesize_trait_defaults_after_module_validation(syntax, &selection)
    }

    /*
        Resolution has already checked the complete namespace frontier in its
        current mode. Rechecking in complete-value mode would reject the deliberately
        unevaluated scalar declarations used only for initializer selection evidence.
        The shared selection supplies exact trait identity: every name this
        pass follows was already admitted by the caller's namespace validation.
    */
    pub(crate) fn synthesize_trait_defaults_after_module_validation(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection,
    ) -> Result<(), Vec<Diagnostic>> {
        let traits = syntax
            .root_items()
            .filter_map(|item| {
                let Item::Trait(trait_definition) = item else {
                    return None;
                };
                let symbol = selection
                    .trait_lookup(syntax, &trait_definition.name)
                    .unique()?;
                let parameter_names = syntax_items(syntax)
                    .type_parameters(trait_definition.type_parameters)
                    .iter()
                    .map(|parameter| parameter.name.as_str(syntax).to_string())
                    .collect();
                let signatures = syntax_items(syntax)
                    .state_signatures(trait_definition.machines)
                    .iter()
                    .enumerate()
                    .map(|(ordinal, handle)| TraitSignatureInput {
                        ordinal,
                        signature: syntax.items.state_signature(*handle).clone(),
                    })
                    .collect::<Vec<_>>();
                let mut requirements = Vec::new();
                let requirement_symbol = |name: &Identifier| {
                    selection
                        .trait_lookup(syntax, name)
                        .unique()
                        .unwrap_or_else(SymbolHandle::invalid)
                };
                for handle in
                    syntax_type_references(syntax).type_reference_handles(trait_definition.parents)
                {
                    match syntax.type_references.type_reference(*handle) {
                        TypeReferenceNode::Named(name) => {
                            requirements.push(TraitRequirementInput {
                                symbol: requirement_symbol(name),
                                arguments: Vec::new(),
                            })
                        }
                        TypeReferenceNode::Generic {
                            base_name,
                            arguments,
                            ..
                        } => requirements.push(TraitRequirementInput {
                            symbol: requirement_symbol(base_name),
                            arguments: syntax_type_references(syntax)
                                .type_reference_handles(*arguments)
                                .to_vec(),
                        }),
                        _ => {}
                    }
                }
                requirements.extend(
                    syntax_items(syntax)
                        .identifier_path_members(trait_definition.requires)
                        .iter()
                        .map(|name| TraitRequirementInput {
                            symbol: requirement_symbol(name),
                            arguments: Vec::new(),
                        }),
                );
                Some((
                    symbol,
                    TraitDefaultsInput {
                        path: selection.declaration_path(symbol),
                        lifetime_parameters: trait_definition.lifetime_parameters.clone(),
                        parameter_names,
                        signatures,
                        requirements,
                    },
                ))
            })
            .collect::<HashMap<_, _>>();

        inherit_refinement_telescopes(syntax, selection, &|symbol| traits.get(&symbol))?;

        let conformances = syntax
            .root_item_handles()
            .iter()
            .filter_map(|handle| match syntax.root_item(*handle) {
                Item::Conformance(conformance) => Some(ConformanceInput {
                    handle: *handle,
                    declaration: conformance.clone(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut diagnostics = Vec::new();
        for conformance in conformances {
            let Some(trait_symbol) = selection
                .trait_lookup(syntax, &conformance.declaration.trait_name)
                .unique()
            else {
                continue;
            };
            let trait_name = conformance
                .declaration
                .trait_name
                .as_str(syntax)
                .to_string();
            let arguments = syntax_type_references(syntax)
                .type_reference_handles(conformance.declaration.trait_arguments)
                .to_vec();
            let Some(conformed_trait) = traits.get(&trait_symbol) else {
                continue;
            };
            let conformance_name = conformance
                .declaration
                .alias
                .as_ref()
                .expect("parsed named implementations carry their name")
                .as_str(syntax)
                .to_string();
            if arguments.len() != conformed_trait.parameter_names.len() {
                diagnostics.push(Diagnostic::error(format!(
                "conformance `{conformance_name} satisfies {trait_name}` expects {} generic argument(s), got {}",
                conformed_trait.parameter_names.len(),
                arguments.len()
            )));
                continue;
            }
            let substitution = conformed_trait
                .parameter_names
                .iter()
                .cloned()
                .zip(arguments.iter().copied())
                .collect::<HashMap<_, _>>();

            let mut closed_members = syntax_items(syntax)
                .conformance_members(conformance.declaration.members)
                .to_vec();
            let mut requirements = Vec::new();
            collect_requirement_instances(
                syntax,
                trait_symbol,
                &substitution,
                &traits,
                &mut HashSet::new(),
                &mut HashSet::new(),
                &mut requirements,
            );
            let mut existing_defaults = HashSet::new();
            for member in &closed_members {
                if let ConformanceMember::TraitDefault {
                    declaring_trait,
                    requirement_ordinal,
                    ..
                } = member
                {
                    existing_defaults.insert((
                        declaring_trait.as_str(syntax).to_string(),
                        *requirement_ordinal,
                    ));
                }
            }

            let mut added = false;
            for requirement in requirements {
                let declaring_path = traits
                    .get(&requirement.declaring_trait)
                    .map(|definition| definition.path.clone())
                    .unwrap_or_else(|| selection.declaration_path(requirement.declaring_trait));
                let key = (declaring_path.clone(), requirement.requirement_ordinal);
                if existing_defaults.contains(&key) || !requirement.signature.is_default {
                    continue;
                }
                let signature = instantiate_default_signature(
                    syntax,
                    &requirement.signature,
                    &requirement.substitution,
                    conformance.handle,
                    &[],
                );
                let exact_requirement_owner = traits
                    .get(&requirement.declaring_trait)
                    .filter(|definition| {
                        definition.lifetime_parameters.is_empty()
                            && definition.parameter_names.is_empty()
                    })
                    .map(|definition| definition.path.as_str());
                let machine = machine_from_signature(
                    syntax,
                    implementation_member_name(
                        &conformance.declaration,
                        signature.name.as_str(syntax),
                    ),
                    &signature,
                    exact_requirement_owner,
                    Identifier::generated(declaring_path.as_str()),
                );
                closed_members.push(ConformanceMember::TraitDefault {
                    declaring_trait: Identifier::generated(declaring_path),
                    requirement_ordinal: requirement.requirement_ordinal,
                    machine,
                });
                existing_defaults.insert(key);
                added = true;
            }
            if selection.declaration_path(trait_symbol) == EQUATABLE_TRAIT_PATH
                && let Some(member) = synthesized_equality_member(
                    syntax,
                    &conformance.declaration,
                    conformed_trait,
                    &closed_members,
                    &substitution,
                    conformance.handle,
                )
            {
                closed_members.push(member);
                added = true;
            }
            if added {
                replace_closed_members(
                    syntax,
                    conformance.handle,
                    conformance.declaration,
                    closed_members,
                );
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    // A targeted postcondition uses the selected base requirement's telescope.
    // Copy its types under this refinement's actuals before resolving its names.
    fn inherit_refinement_telescopes<'catalog>(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection,
        selected_trait: &impl Fn(SymbolHandle) -> Option<&'catalog TraitDefaultsInput>,
    ) -> Result<(), Vec<Diagnostic>> {
        let refinements = syntax
            .root_item_handles()
            .iter()
            .filter_map(|handle| {
                let Item::Trait(declaration) = syntax.root_item(*handle) else {
                    return None;
                };
                declaration
                    .refines
                    .filter(|_| {
                        declaration
                            .refinement_clauses
                            .iter()
                            .any(|clause| !clause.signature.contracts.is_empty())
                    })
                    .map(|base| (*handle, declaration.clone(), base))
            })
            .collect::<Vec<_>>();
        for (handle, mut refinement, base) in refinements {
            let (name, arguments, lifetimes) = match syntax.type_references.type_reference(base) {
                TypeReferenceNode::Named(name) => (name.clone(), Vec::new(), Vec::new()),
                TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    lifetime_arguments,
                } => (
                    base_name.clone(),
                    syntax_type_references(syntax)
                        .type_reference_handles(*arguments)
                        .to_vec(),
                    lifetime_arguments.clone(),
                ),
                _ => {
                    return Err(vec![Diagnostic::error(
                        "a refinement postcondition needs a selected base trait",
                    )]);
                }
            };
            let base = selection
                .trait_lookup(syntax, &name)
                .unique()
                .and_then(selected_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "a refinement postcondition lost its selected base declaration",
                    )]
                })?;
            if arguments.len() != base.parameter_names.len()
                || lifetimes.len() != base.lifetime_parameters.len()
            {
                return Err(vec![Diagnostic::error(
                    "a refinement postcondition needs the complete base trait application",
                )]);
            }
            let substitution = base
                .parameter_names
                .iter()
                .cloned()
                .zip(arguments)
                .collect::<HashMap<_, _>>();
            let lifetimes = base
                .lifetime_parameters
                .iter()
                .zip(lifetimes)
                .map(|(formal, actual)| (formal.as_str(syntax).to_owned(), actual))
                .collect::<Vec<_>>();
            for clause in &mut refinement.refinement_clauses {
                if clause.signature.contracts.is_empty() {
                    continue;
                }
                let Some(requirement) = &clause.requirement else {
                    return Err(vec![Diagnostic::error(
                        "a refinement postcondition names one exact base requirement",
                    )]);
                };
                let matching = base
                    .signatures
                    .iter()
                    .filter(|candidate| {
                        candidate.signature.name.as_str(syntax) == requirement.as_str(syntax)
                    })
                    .collect::<Vec<_>>();
                let [selected] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(
                        "a refinement postcondition must select exactly one base requirement",
                    )]);
                };
                let inherited = instantiate_default_signature(
                    syntax,
                    &selected.signature,
                    &substitution,
                    handle,
                    &lifetimes,
                );
                clause.signature.parameters = inherited.parameters;
                clause.signature.type_parameters = inherited.type_parameters;
                clause.signature.lifetime_parameters = inherited.lifetime_parameters;
                clause.signature.conformance_bounds = inherited.conformance_bounds;
                clause.signature.native_callback_parameters = inherited.native_callback_parameters;
                clause.signature.return_type = inherited.return_type;
            }
            syntax.items.replace_item(handle, Item::Trait(refinement));
        }
        Ok(())
    }

    /*
        Collect every exact requirement identity in the inherited closure together
        with the generic substitution at this conformance. Closed conformances keep
        exact `(declaring trait, requirement)` rows, so unlike bodyless attached
        lookup this deliberately does not collapse or shadow same-leaf names.
    */
    fn collect_requirement_instances(
        syntax: &mut SyntaxTrees,
        owner: SymbolHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
        traits: &HashMap<SymbolHandle, TraitDefaultsInput>,
        visiting: &mut HashSet<SymbolHandle>,
        seen: &mut HashSet<(SymbolHandle, usize)>,
        output: &mut Vec<RequirementInstance>,
    ) {
        if !visiting.insert(owner) {
            return;
        }
        let Some(trait_definition) = traits.get(&owner) else {
            visiting.remove(&owner);
            return;
        };

        for declaration in &trait_definition.signatures {
            let key = (owner, declaration.ordinal);
            if seen.insert(key) {
                output.push(RequirementInstance {
                    declaring_trait: owner,
                    requirement_ordinal: declaration.ordinal,
                    signature: declaration.signature.clone(),
                    substitution: substitution.clone(),
                });
            }
        }

        for requirement in &trait_definition.requirements {
            let Some(required_trait) = traits.get(&requirement.symbol) else {
                continue;
            };
            if requirement.arguments.len() != required_trait.parameter_names.len() {
                continue;
            }
            let required_arguments = requirement
                .arguments
                .iter()
                .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                .collect::<Vec<_>>();
            let required_substitution = required_trait
                .parameter_names
                .iter()
                .cloned()
                .zip(required_arguments)
                .collect::<HashMap<_, _>>();
            collect_requirement_instances(
                syntax,
                requirement.symbol,
                &required_substitution,
                traits,
                visiting,
                seen,
                output,
            );
        }

        visiting.remove(&owner);
    }

    fn replace_closed_members(
        syntax: &mut SyntaxTrees,
        handle: ItemHandle,
        mut conformance: ConformanceItem,
        members: Vec<ConformanceMember>,
    ) {
        let mut start = arena::Handle::invalid();
        let mut count = 0u32;
        for member in members {
            let member = syntax.items.append_conformance_member(member);
            if count == 0 {
                start = member;
            }
            count = count
                .checked_add(1)
                .expect("conformance member span count overflow");
        }
        conformance.members = HandleSpan::from_parts_or_empty(start, count);
        syntax
            .items
            .replace_item(handle, Item::Conformance(conformance));
    }

    /*
        The root trait whose closed implementations acquire a synthesized
        structural `equals` row. Selection is by the exact declaration path, so a
        module-scoped trait that merely shares the leaf name gains no privilege.
    */
    const EQUATABLE_TRAIT_PATH: &str = "Equatable";

    /*
        The `equals` member a closed `Equatable<T>` implementation receives when it
        authors no `equals` member for it: the trait's
        requirement signature instantiated at `T`, with the body `left == right`
        over its two parameters. Typed lowering expands that comparison
        structurally and routes every use of this row to the same expansion. A
        trait without exactly one type parameter and one two-parameter `equals`
        requirement synthesizes nothing; ordinary row completeness reports it.
    */
    fn synthesized_equality_member(
        syntax: &mut SyntaxTrees,
        declaration: &ConformanceItem,
        equatable: &TraitDefaultsInput,
        members: &[ConformanceMember],
        substitution: &HashMap<String, TypeReferenceHandle>,
        supplying_declaration: ItemHandle,
    ) -> Option<ConformanceMember> {
        const EQUALS: &str = "equals";
        if equatable.parameter_names.len() != 1
            || members.iter().any(|member| match member {
                ConformanceMember::Machine(machine) => machine.name.as_str(syntax) == EQUALS,
                ConformanceMember::Selection { requirement, .. } => {
                    requirement.as_str(syntax) == EQUALS
                }
                ConformanceMember::Synthesized(_) => true,
                ConformanceMember::TraitDefault { .. } => false,
            })
        {
            return None;
        }
        let requirement = equatable
            .signatures
            .iter()
            .map(|declaration| &declaration.signature)
            .find(|signature| signature.name.as_str(syntax) == EQUALS && !signature.is_default)?;
        if syntax.items.state_parameters(requirement.parameters).len() != 2 {
            return None;
        }
        let mut signature = instantiate_default_signature(
            syntax,
            requirement,
            substitution,
            supplying_declaration,
            &[],
        );
        let [left, right] = syntax.items.state_parameters(signature.parameters) else {
            return None;
        };
        let [left, right] = [*left, *right].map(|parameter| {
            let name =
                Identifier::generated(syntax.items.state_parameter(parameter).name.as_str(syntax));
            let member = syntax.expressions.append_identifier_path_member(name);
            syntax
                .expressions
                .insert(ExpressionNode::Name(HandleSpan::from_parts(member, 1)))
        });
        let equality = syntax
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left,
                operator: BinaryOperator::Equal,
                right,
            }));
        let statement = syntax
            .statements
            .insert(StatementNode::Expression(equality));
        let statement = syntax.items.append_statement_handle(statement);
        signature.default_body = HandleSpan::from_parts(statement, 1);
        Some(ConformanceMember::Synthesized(machine_from_signature(
            syntax,
            implementation_member_name(declaration, EQUALS),
            &signature,
            None,
            Identifier::default(),
        )))
    }

    /*
        A member the compiler adds to a named implementation is named at the
        implementation's own name occurrence. That occurrence gives the row its
        package provenance and exact declaration owner: a generated name would
        belong to no package that may select it, and Terminal production could not
        identify the machine it emits for the row.
    */
    fn implementation_member_name(declaration: &ConformanceItem, member: &str) -> Identifier {
        declaration.alias.as_ref().map_or_else(
            || Identifier::generated(member),
            |alias| Identifier::new(member, alias.source_span()),
        )
    }

    fn substitute_type_reference(
        syntax: &mut SyntaxTrees,
        handle: TypeReferenceHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        let node = syntax.type_references.type_reference(handle).clone();
        match node {
            TypeReferenceNode::Named(name) => substitution
                .get(name.as_str(syntax))
                .copied()
                .unwrap_or(handle),
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let substituted = substitute_type_reference(syntax, referee, substitution);
                if substituted == referee {
                    handle
                } else {
                    syntax
                        .tables
                        .type_references
                        .insert_reference_with_lifetime(substituted, access, lifetime)
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let substituted = substitute_type_reference(syntax, base_type, substitution);
                if substituted == base_type {
                    handle
                } else {
                    syntax
                        .tables
                        .type_references
                        .insert_constrained(substituted, constraints)
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let substituted_element =
                    substitute_type_reference(syntax, element_type, substitution);
                let substituted_length = match &length {
                    FixedArrayLength::ConstParameter(name) => substitution
                        .get(name.as_str(syntax))
                        .and_then(|argument| {
                            match syntax.type_references.type_reference(*argument) {
                                TypeReferenceNode::Named(value) => {
                                    value.as_str(syntax).parse().ok()
                                }
                                _ => None,
                            }
                        })
                        .map(FixedArrayLength::Literal)
                        .unwrap_or_else(|| length.clone()),
                    _ => length.clone(),
                };
                if substituted_element == element_type
                    && substituted_length.same_value(&length, syntax)
                {
                    handle
                } else {
                    syntax
                        .tables
                        .type_references
                        .insert_fixed_array(substituted_element, substituted_length)
                }
            }
            TypeReferenceNode::Slice { element_type } => {
                let substituted = substitute_type_reference(syntax, element_type, substitution);
                if substituted == element_type {
                    handle
                } else {
                    syntax.type_references.insert_slice(substituted)
                }
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                lifetime_arguments,
            } => {
                let original = syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .to_vec();
                let substituted = original
                    .iter()
                    .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                    .collect::<Vec<_>>();
                if substituted == original {
                    handle
                } else {
                    let arguments = syntax
                        .type_references
                        .insert_type_reference_handles(substituted);
                    syntax.type_references.insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments,
                    })
                }
            }
            TypeReferenceNode::DynamicTrait {
                trait_name,
                trait_arguments,
            } => {
                let original = syntax_type_references(syntax)
                    .type_reference_handles(trait_arguments)
                    .to_vec();
                let substituted = original
                    .iter()
                    .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                    .collect::<Vec<_>>();
                if substituted == original {
                    handle
                } else {
                    let trait_arguments = syntax
                        .type_references
                        .insert_type_reference_handles(substituted);
                    syntax
                        .type_references
                        .insert(TypeReferenceNode::DynamicTrait {
                            trait_name,
                            trait_arguments,
                        })
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                let mut changed = false;
                let selected =
                    rewritten_static_argument(syntax, &selection, substitution, &mut changed);
                if changed {
                    syntax
                        .type_references
                        .insert(TypeReferenceNode::StaticSelection(selected))
                } else {
                    handle
                }
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => handle,
        }
    }

    /*
        Copy one default for one implementation, binding the trait's generic
        parameters to that implementation's arguments. The copy includes the body,
        so each instantiation owns its type nodes while its authored identifiers
        keep their source custody.
    */
    fn instantiate_default_signature(
        syntax: &mut SyntaxTrees,
        signature: &StateSignature,
        substitution: &HashMap<String, TypeReferenceHandle>,
        supplying_declaration: ItemHandle,
        lifetimes: &[(String, Identifier)],
    ) -> StateSignature {
        let substitution = substitution
            .iter()
            .filter(|(name, _)| {
                !syntax_items(syntax)
                    .type_parameters(signature.type_parameters)
                    .iter()
                    .any(|parameter| parameter.name.as_str(syntax) == name.as_str())
            })
            .map(|(name, reference)| (name.clone(), *reference))
            .collect::<HashMap<_, _>>();
        if substitution.is_empty() && lifetimes.is_empty() {
            return signature.clone();
        }
        let snapshot = syntax.clone();
        let type_watermark = syntax.type_references.node_count();
        let copied = syntax.copy_state_signature_in_lifetime_scope(&snapshot, signature, lifetimes);
        for (handle, name) in
            syntax_type_references(syntax).named_nodes_from(type_watermark, syntax)
        {
            let Some(argument) = substitution.get(&name) else {
                continue;
            };
            let supplied = syntax.copy_type_reference_handle(&snapshot, *argument);
            let applications = syntax
                .type_references
                .type_alias_applications(supplied)
                .map(|alias| alias.application)
                .collect::<Vec<_>>();
            for application in applications {
                if matches!(syntax.type_references.supplied_operand_owner(application), Ok(owner) if !owner.is_valid())
                {
                    syntax
                        .type_references
                        .retain_supplied_operand_owner(application, supplying_declaration);
                }
            }
            let authored_aliases = syntax
                .type_references
                .type_alias_applications(handle)
                .cloned()
                .collect::<Vec<_>>();
            syntax
                .type_references
                .replace_type_occurrence(handle, supplied);
            for alias in authored_aliases {
                syntax.type_references.retain_type_alias_application(alias);
            }
            if matches!(
                syntax.type_references.supplied_operand_owner(supplied),
                Ok(owner) if !owner.is_valid()
            ) {
                syntax
                    .tables
                    .type_references
                    .retain_supplied_operand_owner(handle, supplying_declaration);
            }
        }
        substitute_static_binder_arguments(syntax, copied.default_body, &substitution);
        copied
    }

    /*
        Replace trait-telescope binders used as static arguments with the
        conformance's actuals. A named declaration, an application, or an integer
        const becomes that actual. Any other actual keeps the authored path, and
        resolution reports it.
    */
    fn substitute_static_binder_arguments(
        syntax: &mut SyntaxTrees,
        statements: HandleSpan<syntax_trees::statement::StatementHandle>,
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) {
        if substitution.is_empty() || statements.is_empty() {
            return;
        }
        let handles = syntax.items.statements(statements).to_vec();
        let mut seen = HashSet::new();
        for handle in handles {
            substitute_statement_static_arguments(syntax, handle, substitution, &mut seen);
        }
    }

    fn substitute_statement_static_arguments(
        syntax: &mut SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
        seen: &mut HashSet<ExpressionHandle>,
    ) {
        use syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
        let node = syntax.statements.statement(statement).clone();
        match node {
            StatementNode::Call(mut call) => {
                if let Some(arguments) =
                    rewritten_static_arguments(syntax, &call.machine_arguments, substitution)
                {
                    call.machine_arguments = arguments.into_boxed_slice();
                    syntax
                        .statements
                        .replace_statement(statement, StatementNode::Call(call.clone()));
                }
                for argument in syntax
                    .statements
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    substitute_expression_static_arguments(syntax, argument, substitution, seen);
                }
            }
            StatementNode::Expression(expression) => {
                substitute_expression_static_arguments(syntax, expression, substitution, seen);
            }
            StatementNode::Assignment(assignment) => {
                substitute_expression_static_arguments(
                    syntax,
                    assignment.target,
                    substitution,
                    seen,
                );
                substitute_expression_static_arguments(
                    syntax,
                    assignment.value,
                    substitution,
                    seen,
                );
            }
            StatementNode::LocalData(local) => {
                substitute_expression_static_arguments(
                    syntax,
                    local.initial_value,
                    substitution,
                    seen,
                );
            }
            StatementNode::ProofOutputBindingStatement(binding) => {
                substitute_expression_static_arguments(syntax, binding.call, substitution, seen);
            }
            StatementNode::AssemblyFact(fact) => {
                substitute_expression_static_arguments(syntax, fact.expression, substitution, seen);
            }
            StatementNode::RootBinding(binding) => {
                substitute_expression_static_arguments(
                    syntax,
                    binding.receiver,
                    substitution,
                    seen,
                );
                if binding.implementation_operand.is_valid() {
                    substitute_expression_static_arguments(
                        syntax,
                        binding.implementation_operand,
                        substitution,
                        seen,
                    );
                }
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    substitute_expression_static_arguments(syntax, guard, substitution, seen);
                }
                for target in transition.targets() {
                    match syntax.statements.transition_target(target).clone() {
                        TransitionTargetNode::Named { arguments, .. } => {
                            for argument in syntax.statements.expression_handles(arguments).to_vec()
                            {
                                substitute_expression_static_arguments(
                                    syntax,
                                    argument,
                                    substitution,
                                    seen,
                                );
                            }
                        }
                        TransitionTargetNode::Value(value) => {
                            substitute_expression_static_arguments(
                                syntax,
                                value,
                                substitution,
                                seen,
                            );
                        }
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
            StatementNode::Assembly(handle) => {
                let instructions = syntax.statements.assembly(handle).blocks;
                let blocks = syntax.statements.assembly_blocks(instructions).to_vec();
                for block in blocks {
                    for expression in syntax
                        .statements
                        .assembly_expressions(block.requires)
                        .iter()
                        .chain(syntax.statements.assembly_expressions(block.ensures))
                        .copied()
                        .collect::<Vec<_>>()
                    {
                        substitute_expression_static_arguments(
                            syntax,
                            expression,
                            substitution,
                            seen,
                        );
                    }
                    for instruction in syntax
                        .statements
                        .assembly_instructions(block.instructions)
                        .to_vec()
                    {
                        substitute_statement_static_arguments(
                            syntax,
                            instruction,
                            substitution,
                            seen,
                        );
                    }
                }
            }
        }
    }

    fn substitute_expression_static_arguments(
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
        seen: &mut HashSet<ExpressionHandle>,
    ) {
        if !expression.is_valid() || !seen.insert(expression) {
            return;
        }
        let node = syntax.expressions.expression(expression).clone();
        match node {
            ExpressionNode::Call(mut call) => {
                if let Some(arguments) =
                    rewritten_static_arguments(syntax, &call.machine_arguments, substitution)
                {
                    call.machine_arguments = arguments.into_boxed_slice();
                    syntax
                        .expressions
                        .replace_expression(expression, ExpressionNode::Call(call.clone()));
                }
                substitute_expression_static_arguments(syntax, call.receiver, substitution, seen);
                for argument in syntax_expressions(syntax)
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    substitute_expression_static_arguments(syntax, argument, substitution, seen);
                }
            }
            ExpressionNode::Match(dispatch) => {
                substitute_expression_static_arguments(
                    syntax,
                    dispatch.subject,
                    substitution,
                    seen,
                );
                let arms = syntax.expressions.match_arms(dispatch.arms).to_vec();
                for arm in arms {
                    let patterns = syntax_expressions(syntax)
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>();
                    for pattern in patterns {
                        substitute_expression_static_arguments(syntax, pattern, substitution, seen);
                    }
                    substitute_expression_static_arguments(syntax, arm.value, substitution, seen);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                substitute_expression_static_arguments(
                    syntax,
                    syntax.expressions.match_subject(saved.capture).subject,
                    substitution,
                    seen,
                );
                substitute_expression_static_arguments(
                    syntax,
                    saved.pattern_head,
                    substitution,
                    seen,
                );
            }
            ExpressionNode::ArrayLiteral(expressions) => {
                for child in syntax.expressions.expression_handles(expressions).to_vec() {
                    substitute_expression_static_arguments(syntax, child, substitution, seen);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                substitute_expression_static_arguments(syntax, atomic.value, substitution, seen);
                substitute_expression_static_arguments(syntax, atomic.result, substitution, seen);
            }
            ExpressionNode::Binary(binary) => {
                substitute_expression_static_arguments(syntax, binary.left, substitution, seen);
                substitute_expression_static_arguments(syntax, binary.right, substitution, seen);
            }
            ExpressionNode::Cast(cast) => {
                substitute_expression_static_arguments(syntax, cast.value, substitution, seen);
            }
            ExpressionNode::Indexed(indexed) => {
                substitute_expression_static_arguments(
                    syntax,
                    indexed.collection,
                    substitution,
                    seen,
                );
                substitute_expression_static_arguments(syntax, indexed.index, substitution, seen);
            }
            ExpressionNode::Membership(membership) => {
                substitute_expression_static_arguments(
                    syntax,
                    membership.value,
                    substitution,
                    seen,
                );
            }
            ExpressionNode::Member(member) => {
                substitute_expression_static_arguments(syntax, member.receiver, substitution, seen);
            }
            ExpressionNode::Borrow(inner) => {
                substitute_expression_static_arguments(syntax, inner.target, substitution, seen);
            }
            ExpressionNode::Range(range) => {
                substitute_expression_static_arguments(syntax, range.start, substitution, seen);
                substitute_expression_static_arguments(syntax, range.end, substitution, seen);
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = syntax.expressions.struct_fields(literal.fields).to_vec();
                for field in fields {
                    substitute_expression_static_arguments(syntax, field.value, substitution, seen);
                }
            }
            ExpressionNode::Unary(unary) => {
                substitute_expression_static_arguments(syntax, unary.operand, substitution, seen);
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_)
            | ExpressionNode::TypeExpression(_) => {}
        }
    }

    fn rewritten_static_arguments(
        syntax: &mut SyntaxTrees,
        arguments: &[StaticMachineArgument],
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) -> Option<Vec<StaticMachineArgument>> {
        let mut changed = false;
        let mut output = Vec::with_capacity(arguments.len());
        for argument in arguments {
            output.push(rewritten_static_argument(
                syntax,
                argument,
                substitution,
                &mut changed,
            ));
        }
        changed.then_some(output)
    }

    fn rewritten_static_argument(
        syntax: &mut SyntaxTrees,
        argument: &StaticMachineArgument,
        substitution: &HashMap<String, TypeReferenceHandle>,
        changed: &mut bool,
    ) -> StaticMachineArgument {
        if argument.type_reference.is_valid()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
            || argument.application.is_some()
        {
            let mut kept = argument.clone();
            if let Some(application) = &argument.application {
                let nested =
                    rewritten_static_arguments(syntax, &application.arguments, substitution);
                let mut member = application.member;
                let mut member_changed = false;
                if let Some(source) = syntax.expressions.static_member(member).cloned() {
                    let rewritten = rewritten_static_argument(
                        syntax,
                        &source,
                        substitution,
                        &mut member_changed,
                    );
                    if member_changed {
                        member = syntax.expressions.insert_static_member(rewritten);
                    }
                }
                if nested.is_some() || member_changed {
                    kept.application = Some(Box::new(StaticSymbolApplication {
                        member,
                        lifetime_arguments: application.lifetime_arguments.clone(),
                        arguments: nested
                            .map_or_else(|| application.arguments.clone(), Vec::into_boxed_slice),
                    }));
                    *changed = true;
                }
            }
            return kept;
        }
        if let [name] = argument.path.as_ref()
            && let Some(actual) = substitution.get(name.as_str(syntax))
            && let Some(replacement) = static_argument_for_actual(syntax, *actual)
        {
            *changed = true;
            return replacement;
        }
        argument.clone()
    }

    /*
        The conformance actual, as a static operand. A path selects a declaration.
        An application keeps its own telescope. An integer const is a literal.
        Other type forms are not static operands and stay unreplaced.
    */
    fn static_argument_for_actual(
        syntax: &SyntaxTrees,
        actual: TypeReferenceHandle,
    ) -> Option<StaticMachineArgument> {
        match syntax.type_references.type_reference(actual) {
            TypeReferenceNode::Named(name) => Some(bare_static_argument(name.clone())),
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let nested = syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| static_argument_for_actual(syntax, *argument))
                    .collect::<Option<Vec<_>>>()?;
                Some(StaticMachineArgument {
                    type_reference: TypeReferenceHandle::invalid(),
                    path: Box::new([base_name.clone()]),
                    application: Some(Box::new(StaticSymbolApplication {
                        member: arena::Handle::invalid(),
                        lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                        arguments: nested.into_boxed_slice(),
                    })),
                    const_literal: None,
                    evidence_projection: None,
                })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                match syntax.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => Some(StaticMachineArgument {
                        type_reference: TypeReferenceHandle::invalid(),
                        path: Box::new([]),
                        application: None,
                        const_literal: Some(literal.clone()),
                        evidence_projection: None,
                    }),
                    ExpressionNode::Name(path) => {
                        let members = syntax.expressions.identifier_path_members(*path);
                        let [name] = members else {
                            return None;
                        };
                        Some(bare_static_argument(name.clone()))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn bare_static_argument(name: Identifier) -> StaticMachineArgument {
        StaticMachineArgument {
            type_reference: TypeReferenceHandle::invalid(),
            path: Box::new([name]),
            application: None,
            const_literal: None,
            evidence_projection: None,
        }
    }

    /*
        `default_body_trait` names the trait that authored `signature`'s body. The
        body keeps that trait's lexical scope for bare requirement names; it is
        empty for compiler-written bodies such as synthesized structural equality.
    */
    fn machine_from_signature(
        syntax: &mut SyntaxTrees,
        machine_name: Identifier,
        signature: &StateSignature,
        requirement_owner: Option<&str>,
        default_body_trait: Identifier,
    ) -> Machine {
        let state = State {
            name: Identifier::generated(signature.name.as_str(syntax)),
            parameters: signature.parameters,
            return_type: signature.return_type,
            contracts: HandleSpan::empty(),
            statements: signature.default_body,
        };
        let state = syntax.items.insert_state(&state);
        let state = syntax.items.append_state_handle(state);
        let satisfies = requirement_owner.map_or_else(HandleSpan::empty, |trait_path| {
            let requirement = Identifier::generated(signature.name.as_str(syntax));
            let clause = syntax.items.append_satisfies_clause(SatisfiesClause {
                trait_name: Identifier::generated(trait_path),
                lifetime_arguments: Vec::new(),
                arguments: HandleSpan::empty(),
                requirement: Some(requirement),
                alias: None,
                via: None,
                via_expression: ExpressionHandle::invalid(),
                via_keyword_source_span: None,
            });
            HandleSpan::from_parts(clause, 1)
        });
        let ordinary_parameters = syntax
            .items
            .type_parameters(signature.type_parameters)
            .iter()
            .filter(|parameter| {
                !matches!(
                    parameter.kind,
                    syntax_trees::item::TypeParameterKind::Implementation
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut type_parameters = HandleSpan::empty();
        for parameter in ordinary_parameters {
            let handle = syntax.items.append_type_parameter(parameter);
            type_parameters.push_contiguous(handle);
        }
        Machine {
            target_family_interface: false,
            name: machine_name,
            default_body_trait,
            attached_data: None,
            spelling: None,
            is_public: false,
            bodyless: false,
            target: None,
            authored_target: None,
            boundary: false,
            generic_data_template: Default::default(),
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters,
            satisfies,
            conformance_bounds: signature.conformance_bounds.clone(),
            terminates_guarantee: signature.terminates_guarantee,
            ranking_subjects: HandleSpan::<ExpressionHandle>::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: ExpressionHandle::invalid(),
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: signature
                .service_reach_keyword_source_spans
                .clone(),
            service_reaches: signature.service_reaches,
            invokes: signature.invokes,
            suspends: signature.suspends,
            suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
            blocks: signature.blocks,
            blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
            admitted_same_stack_bytes: 0,
            declares_no_crash_route: signature.declares_no_crash_route,
            contracts: signature.contracts,
            states: HandleSpan::from_parts(state, 1),
            nested_machines: HandleSpan::empty(),
        }
    }
}
pub(crate) mod declared_types {
    /*
        Declaration-owned type normalization before nominal synthesis.

        Alias expansion and const callable classification share the existing
        lexical header walk. Both need the selected namespace before constant
        discovery; a requirement path must not enter scalar const evaluation.

        Source application rows retain the alias telescope independently from the
        ordinary type replacing it. This stage forms no checked admission receipt.
    */

    use super::declaration_selection::{ConstantSelection, GenericParameterRole};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use source::SourceSpan;
    use symbols::SymbolKind;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{Item, TypeAliasDefinition, TypeParameterKind};
    use syntax_trees::types::{TypeAliasApplicationOrigin, TypeReferenceHandle, TypeReferenceNode};

    mod expression_bindings {
        /*
            Alias-formal occurrences substitute supplying expressions before resolution.
            No operator runs here. The original alias application still owns argument
            carrier/formation admission, and ordinary checking admits the copied RHS.
        */

        use super::{
            Diagnostic, SyntaxTrees, TypeAliasDefinition, TypeParameterKind, TypeReferenceHandle,
        };
        use crate::preparation::declaration_selection::ConstantSelection;
        use crate::preparation::generic_data::{
            collect_expression_handles, visit_expression_type_inputs,
        };
        use arena::HandleSpan;
        use source::SourceSpan;
        use std::collections::{HashMap, HashSet};
        use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
        use syntax_trees::identifier::Identifier;
        use syntax_trees::types::TypeReferenceNode;

        #[derive(Default)]
        pub(super) struct ExpressionBindings {
            pub(super) uses: Vec<(ExpressionHandle, TypeReferenceHandle)>,
            pub(super) declaration_references: Vec<SourceSpan>,
            pub(super) unresolved: Vec<ExpressionHandle>,
        }

        pub(super) fn bind_const_expression(
            templates: &SyntaxTrees,
            selection: &ConstantSelection<'_>,
            root: ExpressionHandle,
            alias: &TypeAliasDefinition,
            arguments: &[TypeReferenceHandle],
            bindings: &mut ExpressionBindings,
        ) -> Vec<TypeReferenceHandle> {
            let mut expressions = HashSet::new();
            collect_expression_handles(templates, root, &mut expressions);
            let proofs = super::static_operands::proof_bindings(templates, &expressions);
            let mut expressions = expressions.into_iter().collect::<Vec<_>>();
            expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
            let parameters = templates.items.type_parameters(alias.type_parameters);
            let mut children = Vec::new();
            for expression in expressions {
                visit_expression_type_inputs(templates, expression, &mut |reference| {
                    children.push(reference)
                });
                match templates.expressions.expression(expression) {
                    ExpressionNode::Name(path) => {
                        let [name] = templates.expressions.identifier_path_members(*path) else {
                            bindings.unresolved.push(expression);
                            continue;
                        };
                        let shadowed = proofs.get(&expression).is_some_and(|bindings| {
                            bindings
                                .iter()
                                .any(|binding| binding.as_str() == name.as_str(templates))
                        });
                        let bound_parameter = parameters.iter().any(|parameter| {
                            parameter.name.as_str(templates) == name.as_str(templates)
                        });
                        let parameter = (!shadowed)
                            .then(|| {
                                parameters.iter().position(|parameter| {
                                    matches!(&parameter.kind, TypeParameterKind::Const { .. })
                                        && parameter.name.as_str(templates)
                                            == name.as_str(templates)
                                })
                            })
                            .flatten();
                        if let Some(position) = parameter {
                            bindings.uses.push((expression, arguments[position]));
                        } else if !shadowed
                            && !bound_parameter
                            && declaration_constant(templates, selection, expression)
                        {
                            bindings
                                .declaration_references
                                .push(templates.expressions.source_span(expression));
                        } else {
                            bindings.unresolved.push(expression);
                        }
                    }
                    ExpressionNode::Call(_) => bindings.unresolved.push(expression),
                    _ => {}
                }
            }
            children
        }

        fn declaration_constant(
            syntax: &SyntaxTrees,
            selection: &ConstantSelection<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let ExpressionNode::Name(path) = syntax.expressions.expression(expression) else {
                return false;
            };
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str(syntax))
                .collect::<Vec<_>>()
                .join("::");
            selection.selects_const(
                syntax,
                &Identifier::new(name, syntax.expressions.source_span(expression)),
            )
        }

        pub(super) fn supplied_expressions(
            syntax: &mut SyntaxTrees,
            uses: &[(ExpressionHandle, TypeReferenceHandle)],
        ) -> Result<Vec<(ExpressionHandle, ExpressionHandle)>, Diagnostic> {
            let mut actuals: HashMap<TypeReferenceHandle, ExpressionHandle> = HashMap::new();
            let mut substitutions = Vec::with_capacity(uses.len());
            for &(source, argument) in uses {
                let actual = if let Some(actual) = actuals.get(&argument) {
                    *actual
                } else {
                    let owner = syntax
                        .type_references
                        .supplied_operand_owner(argument)
                        .map_err(Diagnostic::error)?;
                    if owner.is_valid() {
                        return Err(Diagnostic::error(
                            "transparent alias RHS expression cannot yet preserve explicitly supplied declaration custody",
                        ));
                    }
                    let expression = match syntax.type_references.type_reference(argument).clone() {
                        TypeReferenceNode::ConstExpression(expression) => expression,
                        TypeReferenceNode::Named(name) => {
                            let span = name.source_span();
                            let members = name
                                .as_str(syntax)
                                .split("::")
                                .map(|member| Identifier::new(member, span))
                                .collect::<Vec<_>>();
                            let mut path = HandleSpan::empty();
                            for member in members {
                                syntax
                                    .expressions
                                    .append_identifier_path_member_to_span(&mut path, member);
                            }
                            let expression = syntax.expressions.insert(ExpressionNode::Name(path));
                            syntax.expressions.set_source_span(expression, span);
                            expression
                        }
                        _ => {
                            return Err(Diagnostic::error(
                                "transparent alias Const expression requires a value argument",
                            ));
                        }
                    };
                    actuals.insert(argument, expression);
                    expression
                };
                substitutions.push((source, actual));
            }
            Ok(substitutions)
        }
    }
    mod static_operands {
        /*
            Static alias selections enter the same type-use expansion as annotations.
            Projection retains the authored tuple without evaluating values or selecting
            code. The alias declaration and its receiving slots still own admission.
        */

        use super::{Expansion, Identifier, SyntaxTrees, TypeReferenceHandle, TypeReferenceNode};
        use crate::preparation::generic_data::{
            collect_expression_handles, collect_statement_expression_handles,
            visit_expression_type_inputs,
        };
        use diagnostics::Diagnostic;
        use std::collections::{HashMap, HashSet};
        use syntax_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
        };
        use syntax_trees::identifier::JoinedSyntaxPath;
        use syntax_trees::statement::{StatementHandle, StatementNode};

        impl Expansion<'_, '_> {
            pub(super) fn statement_static_operands(
                &mut self,
                syntax: &mut SyntaxTrees,
                statement: StatementHandle,
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                if let StatementNode::Call(mut call) =
                    syntax.statements.statement(statement).clone()
                {
                    self.static_arguments(syntax, &mut call.machine_arguments, bound)?;
                    syntax
                        .statements
                        .replace_statement(statement, StatementNode::Call(call));
                }
                let mut expressions = HashSet::new();
                collect_statement_expression_handles(syntax, statement, &mut expressions);
                if let StatementNode::Transition(transition) =
                    syntax.statements.statement(statement)
                {
                    let selectors = syntax
                        .statements
                        .outcome_proof_selectors(transition.proof_selectors);
                    if !selectors.is_empty() {
                        let mut target_expressions = HashSet::new();
                        let mut target_bound = bound.to_vec();
                        target_bound.extend(
                            selectors
                                .iter()
                                .map(|selector| selector.binding.as_str(syntax).to_owned()),
                        );
                        for target in transition.targets() {
                            match syntax.statements.transition_target(target) {
                                syntax_trees::statement::TransitionTargetNode::Value(value) => {
                                    collect_expression_handles(
                                        syntax,
                                        *value,
                                        &mut target_expressions,
                                    )
                                }
                                syntax_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for value in syntax.statements.expression_handles(*arguments) {
                                        collect_expression_handles(
                                            syntax,
                                            *value,
                                            &mut target_expressions,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                        expressions.retain(|expression| !target_expressions.contains(expression));
                        self.static_expression_inputs(syntax, target_expressions, &target_bound)?;
                    }
                }
                self.static_expression_inputs(syntax, expressions, bound)
            }

            pub(super) fn expression_static_operands(
                &mut self,
                syntax: &mut SyntaxTrees,
                expression: ExpressionHandle,
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                let mut expressions = HashSet::new();
                collect_expression_handles(syntax, expression, &mut expressions);
                self.static_expression_inputs(syntax, expressions, bound)
            }

            fn static_expression_inputs(
                &mut self,
                syntax: &mut SyntaxTrees,
                expressions: HashSet<ExpressionHandle>,
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                let proof_scopes = proof_bindings(syntax, &expressions);
                let mut expressions = expressions.into_iter().collect::<Vec<_>>();
                expressions.sort_unstable_by_key(|expression| {
                    (expression.arena_index(), expression.generation())
                });
                for expression in expressions {
                    let scoped_bound = proof_scopes.get(&expression).map(|proofs| {
                        let mut scoped = bound.to_vec();
                        scoped.extend(proofs.iter().cloned());
                        scoped
                    });
                    let bound = scoped_bound.as_deref().unwrap_or(bound);
                    if let ExpressionNode::Binary(binary) =
                        syntax.expressions.expression(expression).clone()
                        && matches!(
                            binary.operator,
                            BinaryOperator::Equal | BinaryOperator::NotEqual
                        )
                    {
                        self.predicate_type_operand(syntax, binary.left, bound)?;
                        self.predicate_type_operand(syntax, binary.right, bound)?;
                    }
                    if let ExpressionNode::Call(mut call) =
                        syntax.expressions.expression(expression).clone()
                    {
                        self.static_arguments(syntax, &mut call.machine_arguments, bound)?;
                        syntax
                            .expressions
                            .replace_expression(expression, ExpressionNode::Call(call));
                    }
                    let mut references = Vec::new();
                    visit_expression_type_inputs(syntax, expression, &mut |reference| {
                        references.push(reference)
                    });
                    for reference in references {
                        self.reference(syntax, reference, bound)?;
                    }
                }
                Ok(())
            }

            /*
                A selected alias in a type comparison takes the ordinary source-use
                expansion, retaining its original application and formation obligations.
                Lexical binders remain values/types in their own scope, never aliases
                selected merely because a declaration has the same spelling.
            */
            fn predicate_type_operand(
                &mut self,
                syntax: &mut SyntaxTrees,
                expression: ExpressionHandle,
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                match syntax.expressions.expression(expression).clone() {
                    ExpressionNode::Name(path) => {
                        let path = syntax.expressions.identifier_path_members(path).to_vec();
                        let Some(first) = path.first() else {
                            return Ok(());
                        };
                        let argument = StaticMachineArgument {
                            path: path.clone().into_boxed_slice(),
                            ..Default::default()
                        };
                        let name = argument_name(syntax, &argument, first)?;
                        if bound
                            .iter()
                            .any(|parameter| parameter == name.as_str(syntax))
                            || self.selection.type_alias_source(syntax, &name).is_none()
                        {
                            return Ok(());
                        }
                        let reference = argument_reference(syntax, &argument)?;
                        self.reference(syntax, reference, bound)?;
                        syntax.expressions.replace_expression(
                            expression,
                            ExpressionNode::TypeExpression(reference),
                        );
                    }
                    ExpressionNode::Borrow(borrow) => {
                        self.predicate_type_operand(syntax, borrow.target, bound)?
                    }
                    ExpressionNode::ArrayLiteral(elements) => {
                        if let [element] = syntax.expressions.expression_handles(elements) {
                            let element = *element;
                            self.predicate_type_operand(syntax, element, bound)?;
                        }
                    }
                    ExpressionNode::Indexed(indexed)
                        if matches!(
                            syntax.expressions.expression(indexed.index),
                            ExpressionNode::Range(_)
                        ) =>
                    {
                        self.predicate_type_operand(syntax, indexed.collection, bound)?
                    }
                    _ => {}
                }
                Ok(())
            }

            pub(super) fn retain_contract_bindings(
                syntax: &SyntaxTrees,
                contracts: arena::HandleSpan<syntax_trees::item::CapabilityContract>,
                bound: &mut Vec<String>,
            ) {
                bound.extend(
                    syntax
                        .items
                        .capability_contracts(contracts)
                        .iter()
                        .filter_map(|contract| contract.binding.as_ref())
                        .map(|binding| binding.as_str(syntax).to_owned()),
                );
            }

            pub(super) fn retain_statement_bindings(
                syntax: &SyntaxTrees,
                statement: StatementHandle,
                bound: &mut Vec<String>,
            ) {
                match syntax.statements.statement(statement) {
                    StatementNode::LocalData(local) => {
                        bound.push(local.name.as_str(syntax).to_owned())
                    }
                    StatementNode::ProofOutputBindingStatement(output) => bound.extend(
                        output
                            .bindings
                            .iter()
                            .map(|selector| selector.binding.as_str(syntax).to_owned()),
                    ),
                    _ => {}
                }
            }

            pub(super) fn contract_static_operands(
                &mut self,
                syntax: &mut SyntaxTrees,
                contracts: arena::HandleSpan<syntax_trees::item::CapabilityContract>,
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                let contracts = syntax.items.capability_contracts(contracts).to_vec();
                for contract in contracts {
                    let facts = syntax.items.proof_facts(contract.facts).to_vec();
                    for fact in facts {
                        match fact {
                            syntax_trees::item::ProofFact::Expression(expression) => {
                                self.expression_static_operands(syntax, expression, bound)?
                            }
                            syntax_trees::item::ProofFact::Membership(membership) => {
                                self.expression_static_operands(syntax, membership.value, bound)?;
                                let arguments = syntax
                                    .type_references
                                    .type_reference_handles(membership.domain_arguments)
                                    .to_vec();
                                for reference in arguments {
                                    self.reference(syntax, reference, bound)?;
                                }
                            }
                        }
                    }
                }
                Ok(())
            }

            pub(super) fn static_arguments(
                &mut self,
                syntax: &mut SyntaxTrees,
                arguments: &mut [StaticMachineArgument],
                bound: &[String],
            ) -> Result<(), Diagnostic> {
                for argument in arguments {
                    if argument.type_reference.is_valid() {
                        self.reference(syntax, argument.type_reference, bound)?;
                        continue;
                    }
                    if let Some(application) = &mut argument.application {
                        self.static_arguments(syntax, &mut application.arguments, bound)?;
                        if application.member.is_valid() {
                            let mut member = syntax
                                .expressions
                                .static_member(application.member)
                                .ok_or_else(|| {
                                    Diagnostic::error(
                                        "static alias operand has no retained qualified member",
                                    )
                                })?
                                .clone();
                            self.static_arguments(
                                syntax,
                                std::slice::from_mut(&mut member),
                                bound,
                            )?;
                            let target = syntax
                                .expressions
                                .static_member_mut(application.member)
                                .ok_or_else(|| {
                                Diagnostic::error("static alias operand lost its qualified member")
                            })?;
                            *target = member;
                            continue;
                        }
                    }
                    let Some(first) = argument.path.first() else {
                        continue;
                    };
                    let name = argument_name(syntax, argument, first)?;
                    if bound
                        .iter()
                        .any(|parameter| parameter == name.as_str(syntax))
                        || self.selection.type_alias_source(syntax, &name).is_none()
                    {
                        continue;
                    }
                    let reference = argument_reference(syntax, argument)?;
                    self.reference(syntax, reference, bound)?;
                    *argument = StaticMachineArgument {
                        type_reference: reference,
                        ..Default::default()
                    };
                }
                Ok(())
            }
        }

        /*
            Transport a supplied operand into ordinary type-application syntax. The
            receiving telescope decides Type/Const/Value/Machine/Implementation roles;
            this projection neither normalizes its value nor expands a code selection.
        */
        fn argument_reference(
            syntax: &mut SyntaxTrees,
            argument: &StaticMachineArgument,
        ) -> Result<TypeReferenceHandle, Diagnostic> {
            if argument.type_reference.is_valid() {
                return Ok(argument.type_reference);
            }
            if let Some(literal) = &argument.const_literal {
                let expression = syntax
                    .expressions
                    .insert(ExpressionNode::Integer(literal.clone()));
                return Ok(syntax
                    .type_references
                    .insert(TypeReferenceNode::ConstExpression(expression)));
            }
            if let Some(projection) = &argument.evidence_projection {
                let term_span = projection.term.source_span();
                let member_span = projection.member.source_span();
                if term_span.source_id != member_span.source_id {
                    return Err(Diagnostic::error(
                        "static alias projection mixes supplying sources",
                    )
                    .with_source_span(term_span));
                }
                let head = syntax
                    .expressions
                    .append_identifier_path_member(projection.term.clone());
                let receiver = syntax
                    .expressions
                    .insert(ExpressionNode::Name(arena::HandleSpan::from_parts(head, 1)));
                syntax.expressions.set_source_span(receiver, term_span);
                let expression = syntax.expressions.insert(ExpressionNode::Member(
                    syntax_trees::expression::TableMemberExpression {
                        receiver,
                        access_source_span: source::SourceSpan::new(
                            term_span.source_id,
                            source::Span::new(term_span.span.end, member_span.span.end),
                        ),
                        member: projection.member.clone(),
                        case_variant: None,
                    },
                ));
                syntax.expressions.set_source_span(
                    expression,
                    source::SourceSpan::new(
                        term_span.source_id,
                        source::Span::new(term_span.span.start, member_span.span.end),
                    ),
                );
                return Ok(syntax
                    .type_references
                    .insert(TypeReferenceNode::ConstExpression(expression)));
            }
            let first = argument.path.first().ok_or_else(|| {
                Diagnostic::error("static alias argument has no retained operand")
            })?;
            let name = argument_name(syntax, argument, first)?;
            let node = if let Some(application) = &argument.application {
                if application.member.is_valid() {
                    return Err(Diagnostic::error("qualified static member requires its exact applied owner before type materialization").with_source_span(first.source_span()));
                }
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| argument_reference(syntax, argument))
                    .collect::<Result<Vec<_>, _>>()?;
                TypeReferenceNode::Generic {
                    base_name: name,
                    lifetime_arguments: application.lifetime_arguments.to_vec(),
                    arguments: syntax
                        .type_references
                        .insert_type_reference_handles(arguments),
                }
            } else {
                TypeReferenceNode::Named(name)
            };
            Ok(syntax.type_references.insert(node))
        }

        fn argument_name(
            syntax: &SyntaxTrees,
            argument: &StaticMachineArgument,
            first: &Identifier,
        ) -> Result<Identifier, Diagnostic> {
            let mut span = first.source_span();
            for member in &argument.path {
                let member_span = member.source_span();
                if member_span.source_id != span.source_id {
                    return Err(
                        Diagnostic::error("static alias path mixes supplying sources")
                            .with_source_span(span),
                    );
                }
                span.span.end = member_span.span.end;
            }
            Ok(Identifier::new(argument.path.joined_path_in(syntax), span))
        }

        /*
            Proof selectors own only their guard and result expression subtrees. Both
            alias selection and alias-formal substitution must respect that lexical scope.
        */
        pub(super) fn proof_bindings(
            syntax: &SyntaxTrees,
            expressions: &HashSet<ExpressionHandle>,
        ) -> HashMap<ExpressionHandle, Vec<String>> {
            let mut proof_scopes: HashMap<ExpressionHandle, Vec<String>> = HashMap::new();
            for &expression in expressions {
                if let ExpressionNode::Match(dispatch) = syntax.expressions.expression(expression) {
                    for arm in syntax.expressions.match_arms(dispatch.arms) {
                        let syntax_trees::expression::MatchPattern::Destructure(pattern) =
                            arm.pattern
                        else {
                            continue;
                        };
                        let selectors = syntax
                            .statements
                            .outcome_proof_selectors(pattern.proof_selectors);
                        if selectors.is_empty() {
                            continue;
                        }
                        let mut scoped = HashSet::new();
                        collect_expression_handles(syntax, arm.value, &mut scoped);
                        collect_expression_handles(syntax, pattern.guard, &mut scoped);
                        for expression in scoped {
                            proof_scopes.entry(expression).or_default().extend(
                                selectors
                                    .iter()
                                    .map(|selector| selector.binding.as_str(syntax).to_owned()),
                            );
                        }
                    }
                }
            }
            proof_scopes
        }
    }

    struct Expansion<'a, 'base> {
        selection: &'a ConstantSelection<'base>,
        declarations: Vec<&'a SyntaxTrees>,
        active: Vec<SourceSpan>,
        supplied: Vec<TypeReferenceHandle>,
    }

    impl Expansion<'_, '_> {
        fn reference(
            &mut self,
            syntax: &mut SyntaxTrees,
            reference: TypeReferenceHandle,
            bound: &[String],
        ) -> Result<(), Diagnostic> {
            if !reference.is_valid() || self.supplied.contains(&reference) {
                return Ok(());
            }
            let node = syntax.type_references.type_reference(reference).clone();
            let (head, lifetimes, arguments) = match &node {
                TypeReferenceNode::Named(name) => (Some(name.clone()), Vec::new(), Vec::new()),
                TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                } => (
                    Some(base_name.clone()),
                    lifetime_arguments.clone(),
                    syntax
                        .type_references
                        .type_reference_handles(*arguments)
                        .to_vec(),
                ),
                _ => (None, Vec::new(), Vec::new()),
            };
            if let Some(head) =
                head.filter(|head| !bound.iter().any(|name| name == head.as_str(syntax)))
            {
                let declaration = self.selection.type_alias_source(syntax, &head);
                let parameters = self.selection.generic_parameter_roles(
                    syntax,
                    &self.declarations,
                    &head,
                    &[
                        SymbolKind::BuiltinType,
                        SymbolKind::Data,
                        SymbolKind::TypeAlias,
                        SymbolKind::Trait,
                        SymbolKind::Machine,
                    ],
                );
                for (position, argument) in arguments.iter().enumerate() {
                    if parameters
                        .as_ref()
                        .and_then(|parameters| parameters.get(position))
                        .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                    {
                        self.reference(syntax, *argument, bound)?;
                    }
                }
                if let Some(declaration) = declaration {
                    return self.application(
                        syntax,
                        reference,
                        &head,
                        declaration,
                        &lifetimes,
                        &arguments,
                        bound,
                    );
                }
            }
            match node {
                TypeReferenceNode::StaticSelection(mut selection) => {
                    self.static_arguments(syntax, std::slice::from_mut(&mut selection), bound)?;
                    syntax.type_references.replace_type_reference(
                        reference,
                        TypeReferenceNode::StaticSelection(selection),
                    );
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    self.reference(syntax, referee, bound)?
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    self.reference(syntax, base_type, bound)?;
                    let constraints = syntax.type_references.constraints(constraints).to_vec();
                    for constraint in constraints {
                        if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint
                        {
                            let arguments = syntax
                                .type_references
                                .type_reference_handles(domain.arguments)
                                .to_vec();
                            self.application_arguments(
                                syntax,
                                &domain.name,
                                &arguments,
                                &[SymbolKind::Domain],
                                bound,
                            )?;
                        }
                    }
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    self.reference(syntax, element_type, bound)?
                }
                TypeReferenceNode::DynamicTrait {
                    trait_name,
                    trait_arguments,
                } => {
                    let arguments = syntax
                        .type_references
                        .type_reference_handles(trait_arguments)
                        .to_vec();
                    let parameters = self.selection.generic_parameter_roles(
                        syntax,
                        &self.declarations,
                        &trait_name,
                        &[SymbolKind::Trait],
                    );
                    for (position, argument) in arguments.iter().enumerate() {
                        if parameters
                            .as_ref()
                            .and_then(|parameters| parameters.get(position))
                            .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                        {
                            self.reference(syntax, *argument, bound)?;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        }

        fn application_arguments(
            &mut self,
            syntax: &mut SyntaxTrees,
            name: &Identifier,
            arguments: &[TypeReferenceHandle],
            kinds: &[SymbolKind],
            bound: &[String],
        ) -> Result<(), Diagnostic> {
            let parameters =
                self.selection
                    .generic_parameter_roles(syntax, &self.declarations, name, kinds);
            for (position, argument) in arguments.iter().enumerate() {
                if parameters
                    .as_ref()
                    .and_then(|parameters| parameters.get(position))
                    .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                {
                    self.reference(syntax, *argument, bound)?;
                }
            }
            Ok(())
        }

        fn application(
            &mut self,
            syntax: &mut SyntaxTrees,
            reference: TypeReferenceHandle,
            name: &Identifier,
            declaration: SourceSpan,
            lifetimes: &[Identifier],
            arguments: &[TypeReferenceHandle],
            _bound: &[String],
        ) -> Result<(), Diagnostic> {
            let mut selected = None;
            for forest in &self.declarations {
                let mut declarations = forest.root_items().filter_map(|item| match item {
                    Item::TypeAlias(alias) if alias.name.source_span() == declaration => {
                        Some(alias)
                    }
                    _ => None,
                });
                if let Some(alias) = declarations.next() {
                    if declarations.next().is_some() {
                        return Err(Diagnostic::error(
                            "selected type alias has ambiguous source declarations",
                        )
                        .with_source_span(name.source_span()));
                    }
                    selected = Some((*forest, alias.clone()));
                    break;
                }
            }
            let (forest, alias) = selected.ok_or_else(|| {
                Diagnostic::error("selected type alias has no retained source declaration")
                    .with_source_span(name.source_span())
            })?;
            if alias.lifetime_parameters.len() != lifetimes.len()
                || alias.type_parameters.count() as usize != arguments.len()
            {
                return Err(Diagnostic::error(
                    "type alias application does not supply its complete declared telescope",
                )
                .with_source_span(name.source_span()));
            }
            if self.active.contains(&declaration) {
                return Err(Diagnostic::error(
                    "transparent type alias expansion contains a recursive declaration cycle",
                )
                .with_source_span(name.source_span()));
            }
            let mut types = Vec::new();
            let mut lifetime_uses = Vec::new();
            let mut array_length_uses = Vec::new();
            let mut expressions = expression_bindings::ExpressionBindings::default();
            bind_type_occurrences(
                forest,
                self.selection,
                alias.authored_expansion,
                &alias,
                arguments,
                lifetimes,
                &mut types,
                &mut lifetime_uses,
                &mut array_length_uses,
                &mut expressions,
            );
            if let Some(operand) = expressions.unresolved.first() {
                return Err(Diagnostic::error("transparent alias expansion cannot yet retain the declaration and supplying scopes of this expression operand").with_source_span(forest.expressions.source_span(*operand)));
            }
            let supplied_expressions =
                expression_bindings::supplied_expressions(syntax, &expressions.uses)
                    .map_err(|diagnostic| diagnostic.with_source_span(name.source_span()))?;
            let authored = syntax.type_references.type_reference(reference).clone();
            let raw = syntax.type_references.insert(authored);
            syntax
                .type_references
                .retain_copied_operand_owner(reference, raw);
            let (expanded, supplied_copies) = syntax.instantiate_type_reference_from(
                forest,
                alias.authored_expansion,
                &types,
                &supplied_expressions,
                &lifetime_uses,
                &array_length_uses,
            );
            self.active.push(declaration);
            let prior_supplied = self.supplied.len();
            self.supplied.extend_from_slice(arguments);
            self.supplied.extend(supplied_copies);
            let bound = forest
                .items
                .type_parameters(alias.type_parameters)
                .iter()
                .map(|parameter| parameter.name.as_str(forest).to_owned())
                .collect::<Vec<_>>();
            let result = self.reference(syntax, expanded, &bound);
            self.supplied.truncate(prior_supplied);
            self.active.pop();
            result?;
            syntax
                .type_references
                .replace_type_occurrence(reference, expanded);
            let mut definition_references = definition_reference_spans(forest, &alias);
            definition_references.extend(expressions.declaration_references);
            syntax
                .type_references
                .retain_type_alias_application(TypeAliasApplicationOrigin {
                    instance: reference,
                    application: raw,
                    declaration,
                    definition_references,
                });
            Ok(())
        }
    }

    /*
        These are lexical references in the original declaration, not rendered
        identities. Supplied actuals carry their own distinct source occurrences.
    */
    fn definition_reference_spans(
        forest: &SyntaxTrees,
        alias: &TypeAliasDefinition,
    ) -> Vec<SourceSpan> {
        let parameters = forest.items.type_parameters(alias.type_parameters);
        let mut positions = Vec::new();
        super::generic_data::collect_type_positions(
            forest,
            alias.authored_expansion,
            &mut positions,
            true,
        );
        let mut references = Vec::new();
        for position in positions {
            let name = match forest.type_references.type_reference(position) {
                TypeReferenceNode::Named(name) => Some(name),
                TypeReferenceNode::Generic { base_name, .. } => Some(base_name),
                TypeReferenceNode::DynamicTrait { trait_name, .. } => Some(trait_name),
                TypeReferenceNode::FixedArray {
                    length: syntax_trees::types::FixedArrayLength::ConstParameter(name),
                    ..
                } => Some(name),
                _ => None,
            };
            if let Some(name) = name
                && !parameters
                    .iter()
                    .any(|parameter| parameter.name.as_str(forest) == name.as_str(forest))
                && !references.contains(&name.source_span())
            {
                references.push(name.source_span());
            }
        }
        references
    }

    fn bind_type_occurrences(
        templates: &SyntaxTrees,
        selection: &ConstantSelection<'_>,
        reference: TypeReferenceHandle,
        alias: &TypeAliasDefinition,
        arguments: &[TypeReferenceHandle],
        lifetimes: &[Identifier],
        types: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        lifetime_uses: &mut Vec<(SourceSpan, String, Identifier)>,
        array_length_uses: &mut Vec<(SourceSpan, String, TypeReferenceHandle)>,
        expressions: &mut expression_bindings::ExpressionBindings,
    ) {
        for application in templates.type_references.type_alias_applications(reference) {
            bind_type_occurrences(
                templates,
                selection,
                application.application,
                alias,
                arguments,
                lifetimes,
                types,
                lifetime_uses,
                array_length_uses,
                expressions,
            );
        }
        let mut lifetime = |name: &Identifier| {
            if let Some(position) = alias
                .lifetime_parameters
                .iter()
                .position(|parameter| parameter.as_str(templates) == name.as_str(templates))
            {
                lifetime_uses.push((
                    name.source_span(),
                    name.as_str(templates).to_owned(),
                    lifetimes[position].clone(),
                ));
            }
        };
        let children = match templates.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name) => {
                if let Some(position) = templates
                    .items
                    .type_parameters(alias.type_parameters)
                    .iter()
                    .position(|parameter| {
                        parameter.name.as_str(templates) == name.as_str(templates)
                    })
                {
                    types.push((reference, arguments[position]));
                }
                Vec::new()
            }
            TypeReferenceNode::Reference {
                referee,
                lifetime: region,
                ..
            } => {
                if let Some(region) = region {
                    lifetime(region);
                }
                vec![*referee]
            }
            TypeReferenceNode::Generic {
                lifetime_arguments,
                arguments,
                ..
            } => {
                for region in lifetime_arguments {
                    lifetime(region);
                }
                templates
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec()
            }
            TypeReferenceNode::ConstExpression(expression) => {
                expression_bindings::bind_const_expression(
                    templates,
                    selection,
                    *expression,
                    alias,
                    arguments,
                    expressions,
                )
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let mut children = vec![*base_type];
                for constraint in templates.type_references.constraints(*constraints) {
                    if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                        children.extend_from_slice(
                            templates
                                .type_references
                                .type_reference_handles(domain.arguments),
                        );
                    }
                }
                children
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let mut children = vec![*element_type];
                match length {
                    syntax_trees::types::FixedArrayLength::Expression(reference) => {
                        children.push(*reference)
                    }
                    syntax_trees::types::FixedArrayLength::ConstParameter(name) => {
                        if let Some(position) = templates
                            .items
                            .type_parameters(alias.type_parameters)
                            .iter()
                            .position(|parameter| {
                                parameter.name.as_str(templates) == name.as_str(templates)
                            })
                        {
                            array_length_uses.push((
                                name.source_span(),
                                name.as_str(templates).to_owned(),
                                arguments[position],
                            ));
                        }
                    }
                    _ => {}
                }
                children
            }
            TypeReferenceNode::Slice { element_type } => vec![*element_type],
            TypeReferenceNode::DynamicTrait {
                trait_arguments, ..
            } => templates
                .type_references
                .type_reference_handles(*trait_arguments)
                .to_vec(),
            _ => Vec::new(),
        };
        for child in children {
            bind_type_occurrences(
                templates,
                selection,
                child,
                alias,
                arguments,
                lifetimes,
                types,
                lifetime_uses,
                array_length_uses,
                expressions,
            );
        }
    }

    /*
        Run before generic-data applications are demanded. Re-entry is idempotent:
        only authored alias heads expand, while retained application rows are not
        themselves traversal roots.
    */
    pub(crate) fn normalize(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        normalize_with_declarations(syntax, selection, &[])
    }

    pub(super) fn normalize_with_declarations(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection<'_>,
        declarations: &[&SyntaxTrees],
    ) -> Result<(), Vec<Diagnostic>> {
        if !needs_normalization(syntax, declarations) {
            return Ok(());
        }
        let templates = syntax.clone();
        let mut forests = vec![&templates];
        forests.extend_from_slice(declarations);
        let mut expansion = Expansion {
            declarations: forests,
            selection,
            active: Vec::new(),
            supplied: Vec::new(),
        };
        for handle in templates.root_item_handles() {
            let mut item = templates.root_item(*handle).clone();
            if let Item::TypeAlias(alias) = &mut item {
                if alias.expansion == alias.authored_expansion {
                    let (working, _) = syntax.instantiate_type_reference_from(
                        &templates,
                        alias.authored_expansion,
                        &[],
                        &[],
                        &[],
                        &[],
                    );
                    alias.expansion = working;
                    syntax.items.replace_item(*handle, item.clone());
                }
                if let Item::TypeAlias(alias) = &item {
                    mark_header_references(syntax, alias.authored_expansion);
                }
            }
            if let Err(error) = expansion.item(syntax, &item, &[]) {
                return Err(vec![error]);
            }
            if let Item::TypeAlias(alias) = item {
                mark_header_references(syntax, alias.expansion);
            }
        }
        Ok(())
    }

    fn needs_normalization(syntax: &SyntaxTrees, declarations: &[&SyntaxTrees]) -> bool {
        has_alias_declarations(syntax, declarations)
            || syntax.items.type_parameter_entries().any(|parameter| {
                let TypeParameterKind::Const { type_reference } = parameter.kind else {
                    return false;
                };
                match syntax.type_references.type_reference(type_reference) {
                    TypeReferenceNode::StaticSelection(_) => true,
                    TypeReferenceNode::Named(name)
                    | TypeReferenceNode::Generic {
                        base_name: name, ..
                    } => name.as_str(syntax).contains("::"),
                    _ => false,
                }
            })
    }

    fn has_alias_declarations(syntax: &SyntaxTrees, declarations: &[&SyntaxTrees]) -> bool {
        std::iter::once(syntax)
            .chain(declarations.iter().copied())
            .any(|forest| {
                forest
                    .root_items()
                    .any(|item| matches!(item, Item::TypeAlias(_)))
            })
    }

    /*
        Build-time evaluation expands type heads before discovering constant and
        representation work. Imported declaration forests lend headers only.
    */
    pub fn normalize_declared_types(
        mut request: super::generic_data::GenericDataRequest<'_>,
    ) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        let declarations = request
            .declaration_syntax
            .into_iter()
            .chain(request.retained_declaration_syntax)
            .collect::<Vec<_>>();
        if !needs_normalization(&request.syntax, &declarations) {
            return Ok(request.syntax);
        }
        let selection = ConstantSelection::with_retained_base(
            &request.syntax,
            request.sources,
            request.top_level_bindings,
            request.retained_base,
        )?;
        normalize_with_declarations(&mut request.syntax, &selection, &declarations)?;
        Ok(request.syntax)
    }

    fn mark_header_references(syntax: &mut SyntaxTrees, root: TypeReferenceHandle) {
        let mut pending = vec![root];
        let mut seen = Vec::new();
        while let Some(root) = pending.pop() {
            if seen.contains(&root) {
                continue;
            }
            seen.push(root);
            let mut references = Vec::new();
            super::generic_data::collect_type_positions(syntax, root, &mut references, true);
            for reference in references {
                pending.extend(
                    syntax
                        .type_references
                        .type_alias_applications(reference)
                        .map(|origin| origin.application),
                );
                syntax
                    .type_references
                    .retain_alias_header_reference(reference);
            }
        }
    }

    impl Expansion<'_, '_> {
        fn parameters(
            &mut self,
            syntax: &mut SyntaxTrees,
            parameters: HandleSpan<syntax_trees::item::TypeParameter>,
            inherited: &[String],
        ) -> Result<Vec<String>, Diagnostic> {
            let parameter_span = parameters;
            let parameters = syntax.items.type_parameters(parameters).to_vec();
            let mut bound = inherited.to_vec();
            bound.extend(
                parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str(syntax).to_owned()),
            );
            for (index, parameter) in parameters.into_iter().enumerate() {
                match parameter.kind {
                    TypeParameterKind::Const { type_reference } => {
                        self.reference(syntax, type_reference, &bound)?;
                        if let Some(contract) = super::callable_types::nominal_contract(
                            syntax,
                            self.selection,
                            type_reference,
                            &bound,
                        )? {
                            syntax.items.type_parameters_mut(parameter_span)[index].kind =
                                TypeParameterKind::Machine {
                                    contract: Some(contract),
                                };
                        }
                    }
                    TypeParameterKind::Value { type_reference } => {
                        self.reference(syntax, type_reference, &bound)?
                    }
                    TypeParameterKind::Machine {
                        contract:
                            Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
                    } => self.signature(syntax, &signature, &bound)?,
                    _ => {}
                }
            }
            Ok(bound)
        }

        fn signature(
            &mut self,
            syntax: &mut SyntaxTrees,
            signature: &syntax_trees::item::StateSignature,
            inherited: &[String],
        ) -> Result<(), Diagnostic> {
            let bound = self.parameters(syntax, signature.type_parameters, inherited)?;
            self.reference(syntax, signature.return_type, &bound)?;
            let parameters = syntax.items.state_parameters(signature.parameters).to_vec();
            for parameter in parameters {
                self.reference(
                    syntax,
                    syntax.items.state_parameter(parameter).type_reference,
                    &bound,
                )?;
            }
            let mut expression_bound = bound.clone();
            expression_bound.extend(
                syntax
                    .items
                    .state_parameters(signature.parameters)
                    .iter()
                    .map(|handle| {
                        syntax
                            .items
                            .state_parameter(*handle)
                            .name
                            .as_str(syntax)
                            .to_owned()
                    }),
            );
            Self::retain_contract_bindings(syntax, signature.contracts, &mut expression_bound);
            self.contract_static_operands(syntax, signature.contracts, &expression_bound)?;
            let statements = syntax.items.statements(signature.default_body).to_vec();
            self.statements(syntax, &statements, &expression_bound)
        }

        fn statements(
            &mut self,
            syntax: &mut SyntaxTrees,
            statements: &[syntax_trees::statement::StatementHandle],
            bound: &[String],
        ) -> Result<(), Diagnostic> {
            let mut bound = bound.to_vec();
            for statement in statements {
                if let syntax_trees::statement::StatementNode::Assembly(handle) =
                    syntax.statements.statement(*statement)
                {
                    let blocks = syntax
                        .statements
                        .assembly_blocks(syntax.statements.assembly(*handle).blocks)
                        .to_vec();
                    for block in blocks {
                        let requires = syntax
                            .statements
                            .assembly_expressions(block.requires)
                            .to_vec();
                        for expression in requires {
                            self.expression_static_operands(syntax, expression, &bound)?;
                        }
                        let instructions = syntax
                            .statements
                            .assembly_instructions(block.instructions)
                            .to_vec();
                        self.statements(syntax, &instructions, &bound)?;
                        let mut completed_bound = bound.clone();
                        for instruction in instructions {
                            Self::retain_statement_bindings(
                                syntax,
                                instruction,
                                &mut completed_bound,
                            );
                        }
                        let ensures = syntax
                            .statements
                            .assembly_expressions(block.ensures)
                            .to_vec();
                        for expression in ensures {
                            self.expression_static_operands(syntax, expression, &completed_bound)?;
                        }
                    }
                    continue;
                }
                self.statement_static_operands(syntax, *statement, &bound)?;
                for reference in super::generic_data::statement_type_inputs(syntax, *statement) {
                    self.reference(syntax, reference, &bound)?;
                }
                Self::retain_statement_bindings(syntax, *statement, &mut bound);
            }
            Ok(())
        }

        fn machine(
            &mut self,
            syntax: &mut SyntaxTrees,
            machine: &syntax_trees::item::Machine,
            inherited: &[String],
        ) -> Result<(), Diagnostic> {
            let mut bound = self.parameters(syntax, machine.type_parameters, inherited)?;
            Self::retain_contract_bindings(syntax, machine.contracts, &mut bound);
            let states = syntax.items.state_handles(machine.states).to_vec();
            let mut entry_bound = bound.clone();
            if let Some(entry) = states.first() {
                entry_bound.extend(
                    syntax
                        .items
                        .state_parameters(syntax.items.state(*entry).parameters)
                        .iter()
                        .map(|handle| {
                            syntax
                                .items
                                .state_parameter(*handle)
                                .name
                                .as_str(syntax)
                                .to_owned()
                        }),
                );
            }
            self.contract_static_operands(syntax, machine.contracts, &entry_bound)?;
            for handle in states {
                let state = syntax.items.state(handle).clone();
                self.reference(syntax, state.return_type, &bound)?;
                let parameters = syntax.items.state_parameters(state.parameters).to_vec();
                for parameter in parameters {
                    self.reference(
                        syntax,
                        syntax.items.state_parameter(parameter).type_reference,
                        &bound,
                    )?;
                }
                let mut expression_bound = bound.clone();
                expression_bound.extend(
                    syntax
                        .items
                        .state_parameters(state.parameters)
                        .iter()
                        .map(|handle| {
                            syntax
                                .items
                                .state_parameter(*handle)
                                .name
                                .as_str(syntax)
                                .to_owned()
                        }),
                );
                Self::retain_contract_bindings(syntax, state.contracts, &mut expression_bound);
                self.contract_static_operands(syntax, state.contracts, &expression_bound)?;
                let statements = syntax.items.statements(state.statements).to_vec();
                self.statements(syntax, &statements, &expression_bound)?;
            }
            for position in 0..machine.nested_machines.count() {
                let item = syntax
                    .items
                    .item(machine.nested_machines.handle_at(position))
                    .clone();
                self.item(syntax, &item, &bound)?;
            }
            Ok(())
        }

        fn item(
            &mut self,
            syntax: &mut SyntaxTrees,
            item: &Item,
            inherited: &[String],
        ) -> Result<(), Diagnostic> {
            use syntax_trees::item::{ConformanceMember, DataMember};
            match item {
                Item::TypeAlias(alias) => {
                    let bound = self.parameters(syntax, alias.type_parameters, inherited)?;
                    self.active.push(alias.name.source_span());
                    let result = self.reference(syntax, alias.expansion, &bound);
                    self.active.pop();
                    result?;
                }
                Item::Data(data) => {
                    let bound = self.parameters(syntax, data.type_parameters, inherited)?;
                    let members = syntax.items.data_members(data.members).to_vec();
                    for member in members {
                        match member {
                            DataMember::Field(field) => {
                                self.reference(syntax, field.type_reference, &bound)?
                            }
                            DataMember::Variant(variant) => {
                                let fields =
                                    syntax.items.data_payload_fields(variant.payload).to_vec();
                                for field in fields {
                                    self.reference(syntax, field.type_reference, &bound)?;
                                }
                            }
                            DataMember::Retired(_) => {}
                        }
                    }
                }
                Item::Machine(machine) => self.machine(syntax, machine, inherited)?,
                Item::Trait(definition) => {
                    let bound = self.parameters(syntax, definition.type_parameters, inherited)?;
                    let parents = syntax
                        .type_references
                        .type_reference_handles(definition.parents)
                        .to_vec();
                    for parent in parents {
                        self.reference(syntax, parent, &bound)?;
                    }
                    if let Some(reference) = definition.refines {
                        self.reference(syntax, reference, &bound)?;
                    }
                    let signatures = syntax.items.state_signatures(definition.machines).to_vec();
                    for signature in signatures {
                        let signature = syntax.items.state_signature(signature).clone();
                        self.signature(syntax, &signature, &bound)?;
                    }
                }
                Item::Conformance(definition) => {
                    let bound = self.parameters(syntax, definition.type_parameters, inherited)?;
                    let arguments = syntax
                        .type_references
                        .type_reference_handles(definition.trait_arguments)
                        .to_vec();
                    for argument in arguments {
                        self.reference(syntax, argument, &bound)?;
                    }
                    let members = syntax
                        .items
                        .conformance_members(definition.members)
                        .to_vec();
                    for member in members {
                        let (ConformanceMember::Machine(machine)
                        | ConformanceMember::Synthesized(machine)
                        | ConformanceMember::TraitDefault { machine, .. }) = member
                        else {
                            // A selection row names an existing machine and has no body.
                            continue;
                        };
                        self.machine(syntax, &machine, &bound)?;
                    }
                }
                Item::Const(definition) => {
                    self.reference(syntax, definition.type_reference, inherited)?;
                    self.expression_static_operands(syntax, definition.value, inherited)?;
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod type_alias_tests {
        /* Real parser trees exercise caller-first expansion and declaration cycles. */

        use super::*;

        fn normalized(text: &str) -> Result<SyntaxTrees, Vec<Diagnostic>> {
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            let mut sources = source::SourceMap::default();
            let id = sources
                .add("alias/main.omg".into(), text.to_owned())
                .source_id;
            let mut syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
            let selection =
                ConstantSelection::new(&syntax, Some(std::sync::Arc::new(sources)), Vec::new())?;
            normalize(&mut syntax, &selection)?;
            Ok(syntax)
        }

        #[test]
        fn nominal_const_types_resolve_the_owner_before_callable_classification() {
            let syntax = normalized(
                "trait Contract<T> { machine invoke(value: T) -> T suspends; } \
                 machine retain<const Callback: Contract<u32>::invoke>() {}",
            )
            .unwrap();
            let machine = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Machine(machine) => Some(machine),
                    _ => None,
                })
                .unwrap();
            let TypeParameterKind::Machine {
                contract:
                    Some(syntax_trees::item::MachineParameterContract::Nominal {
                        trait_arguments, ..
                    }),
            } = &syntax.items.type_parameters(machine.type_parameters)[0].kind
            else {
                panic!("nominal callable");
            };
            let [argument] = syntax
                .type_references
                .type_reference_handles(*trait_arguments)
            else {
                panic!("trait argument");
            };
            assert_eq!(
                syntax
                    .type_references
                    .type_reference(*argument)
                    .as_named()
                    .unwrap()
                    .as_str(&syntax),
                "u32"
            );

            let syntax = normalized(
                "module records; data Index {} machine use_index<const Value: records::Index>() {}",
            )
            .unwrap();
            let machine = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Machine(machine) => Some(machine),
                    _ => None,
                })
                .unwrap();
            assert!(matches!(
                syntax.items.type_parameters(machine.type_parameters)[0].kind,
                TypeParameterKind::Const { .. }
            ));

            let syntax = normalized("trait Contract<T> { machine invoke(value: T) -> T; } machine shadow<Contract, const Callback: Contract<u32>::invoke>() {}").unwrap();
            let machine = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Machine(machine) => Some(machine),
                    _ => None,
                })
                .unwrap();
            assert!(matches!(
                syntax.items.type_parameters(machine.type_parameters)[1].kind,
                TypeParameterKind::Const { .. }
            ));
        }

        #[test]
        fn nominal_const_types_refuse_member_argument_reparenting() {
            let errors = normalized("trait Contract<T> { machine invoke(value: T) -> T; } machine retain<const Callback: Contract::invoke<u32>>() {}").unwrap_err();
            assert!(format!("{errors:?}").contains("applies the trait before"));
        }

        #[test]
        fn aliases_expand_supplied_arguments_before_rhs_cycles_and_keep_obligations() {
            let syntax =
                normalized("data Identity<T> = T; data Cell { value: Identity<Identity<u8>>; }")
                    .unwrap();
            let data = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(data) => Some(data),
                    _ => None,
                })
                .unwrap();
            let syntax_trees::item::DataMember::Field(field) =
                &syntax.items.data_members(data.members)[0]
            else {
                panic!("field");
            };
            let TypeReferenceNode::Named(name) =
                syntax.type_references.type_reference(field.type_reference)
            else {
                panic!("transparent expansion");
            };
            assert_eq!(name.as_str(&syntax), "u8");
            assert_eq!(
                syntax
                    .type_references
                    .type_alias_applications(field.type_reference)
                    .count(),
                2
            );
            let mut repeated = normalized(
        "data T = bool; data Id<X> = X; data Wrap<U> = Id<U>; data Box<T> { value: Wrap<T>; }",
    )
    .unwrap();
            let new_source = "data Other<T> { value: Wrap<T>; }";
            let tokens = source_files_to_tokens::Lexer::new(new_source)
                .tokenize()
                .unwrap();
            let later =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(77), &tokens)
                    .unwrap();
            repeated.extend_from(&later);
            let selection = ConstantSelection::new(&repeated, None, Vec::new()).unwrap();
            normalize(&mut repeated, &selection).unwrap();
            let data = repeated
                .root_items()
                .find_map(|item| match item {
                    Item::Data(data) if data.name.as_str(&repeated) == "Other" => Some(data),
                    _ => None,
                })
                .unwrap();
            let syntax_trees::item::DataMember::Field(field) =
                &repeated.items.data_members(data.members)[0]
            else {
                panic!("field");
            };
            let TypeReferenceNode::Named(name) = repeated
                .type_references
                .type_reference(field.type_reference)
            else {
                panic!("caller type parameter");
            };
            assert_eq!(name.as_str(&repeated), "T");
            let settled = repeated.clone();
            normalize(&mut repeated, &selection).unwrap();
            assert_eq!(
                repeated, settled,
                "a settled source phase must not allocate more header work"
            );

            let mut global = normalized("data Real { value: u8; } data Global<X> = Real; data Outer<Real> = Global<u8>; data View { value: Outer<bool>; }").unwrap();
            let tokens = source_files_to_tokens::Lexer::new("data Later { value: Outer<bool>; }")
                .tokenize()
                .unwrap();
            let later =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(78), &tokens)
                    .unwrap();
            global.extend_from(&later);
            let selection = ConstantSelection::new(&global, None, Vec::new()).unwrap();
            normalize(&mut global, &selection).unwrap();
            let later = global
                .root_items()
                .find_map(|item| match item {
                    Item::Data(data) if data.name.as_str(&global) == "Later" => Some(data),
                    _ => None,
                })
                .unwrap();
            let syntax_trees::item::DataMember::Field(field) =
                &global.items.data_members(later.members)[0]
            else {
                panic!("field");
            };
            let TypeReferenceNode::Named(name) =
                global.type_references.type_reference(field.type_reference)
            else {
                panic!("global nominal");
            };
            assert_eq!(
                name.as_str(&global),
                "Real",
                "a normalized global must not become Outer’s same-spelled formal"
            );
            for source in [
                "data Recursive<T> = Recursive<[T; 2]>;",
                "data First<T> = Second<T>; data Second<T> = First<[T; 2]>;",
            ] {
                let errors = normalized(source).unwrap_err();
                assert!(
                    errors
                        .iter()
                        .any(|error| error.to_string().contains("recursive declaration cycle"))
                );
            }
        }
    }
}
#[path = "preparation_type_equations.rs"]
pub(crate) mod type_equations;

use declaration_selection::ConstantSelection;
use diagnostics::Diagnostic;
use module_normalization::validate_with_const_resolution_mode;
use source::SourceMap;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use trait_defaults::synthesize_trait_defaults_after_module_validation;

/*
    A forest ready to lower, with the constant selector its declarations
    resolved under.
*/
pub(crate) struct Prepared {
    pub(crate) syntax: SyntaxTrees,
    pub(crate) constant_selection: ConstantSelection<'static>,
}

/*
    Select constants, validate the forest for the requested constant
    resolution, and materialize default trait machines. Generic data
    normalization is the caller's earlier step, not part of this one.

    The forest arrives by value and is rewritten in place. Preparation used to
    clone the whole forest it was handed by reference, which charged every
    resolution one arena copy of the entire program to reach the same
    rewritten syntax the caller then stopped reading.
*/
pub(crate) fn prepare(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    constants: crate::resolution::lowerer::ConstResolutionMode,
    base: Option<Arc<symbol_resolved_trees::SymbolResolvedTrees>>,
) -> Result<Prepared, Vec<Diagnostic>> {
    target_families::normalize(&mut syntax, sources.as_deref())?;
    let constant_selection = declaration_selection::ConstantSelection::with_shared_base(
        &syntax,
        sources,
        top_level_bindings,
        base,
    )?;
    declared_types::normalize(&mut syntax, &constant_selection)?;
    generic_data::domain_heads::normalize(&mut syntax, Some(&constant_selection))?;
    validate_with_const_resolution_mode(&syntax, &constant_selection, constants)?;
    synthesize_trait_defaults_after_module_validation(&mut syntax, &constant_selection)?;
    Ok(Prepared {
        syntax,
        constant_selection,
    })
}
