/*
    Shared declaration selection before complete symbol assignment.

    `ConstantSelection::new` collects declaration headers, builds a partial
    namespace with imports and builtin names, and exposes exact lookup for
    constants, nominal carriers, domains and traits. `with_retained_base` extends
    the same namespace for generated source. Preparation, initializer handling
    and resolution share this owner; generic-data synthesis is one consumer.
    Transient symbols remain private and do not escape into normalized syntax.
*/

use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use std::sync::Arc;

use crate::resolution::names::constructor_type;
use crate::source_inputs::SourceMap;
use crate::source_resolved::type_aliases::AliasBody;
use crate::source_symbols::{
    SourceScopedTopLevelBinding, SymbolTable, SymbolTableBuilder, builtin_function_symbols,
    builtin_type_symbols,
};
use diagnostics::Diagnostic;
use std::path::Path;
use symbols::{SymbolKind, SymbolNameRef};
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
    Proof,
}

/* Private selection state; transient symbols never escape into normalized syntax. */
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct ConstantSelection<'base> {
    symbols: SymbolTable,
    alias_bodies: Arc<[(source_text::SourceSpan, AliasBody)]>,
    retained: Option<SelectionBase<'base>>,
    // Geometry is derived for one structural-completion pass from its actual
    // resolved owner. A resumed pass reconstructs it from that owner's tree.
    #[record(skip)]
    array_lengths: Vec<(source_text::SourceSpan, Option<usize>)>,
}

/*
    Ephemeral syntax normalization borrows its predecessor; a deferred resolver
    owns that same predecessor across later body demands.
*/
#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectionBase<'base> {
    Borrowed(&'base crate::source_resolved::SymbolResolvedTrees),
    Shared(Arc<crate::source_resolved::SymbolResolvedTrees>),
}

impl std::ops::Deref for SelectionBase<'_> {
    type Target = crate::source_resolved::SymbolResolvedTrees;

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
    supply_mode: language_core::DataSupplyMode,
    reference: source_text::SourceSpan,
) -> bool {
    name == language_core::layout_storage::CONSTRUCTOR_NAME
        && supply_mode == language_core::DataSupplyMode::BoundaryOpaque
        && symbols.source_file(reference).is_some_and(|file| {
            file.origin == source_admission::SourceOrigin::Toolchain
                && file.package_identity.is_none()
                && file.path.strip_prefix(&file.package_root).ok()
                    == Some(Path::new(language_core::layout_storage::SOURCE_FILE_NAME))
                && file.source.as_bytes()
                    == bundled_declarations::bytes(
                        language_core::bundled_files::BundledFile::CoreLayout,
                    )
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

    pub(crate) fn retained_base(&self) -> Option<&crate::source_resolved::SymbolResolvedTrees> {
        self.retained.as_deref()
    }

    /*
        Early declaration consumers share the prepared namespace's visibility
        and import identities. These handles belong to this header table;
        consumers rejoin declarations before publishing resolved symbols.
    */
    pub(crate) fn header_symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /*
        The namespace boundary for early declaration consistency, including
        dependency purpose. A logical path alone is not a foreign owner.
    */
    pub(crate) fn same_checked_package(
        &self,
        left: source_text::SourceSpan,
        right: source_text::SourceSpan,
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
        base: Option<Arc<crate::source_resolved::SymbolResolvedTrees>>,
    ) -> Result<ConstantSelection<'static>, Vec<Diagnostic>> {
        let ConstantSelection {
            symbols,
            alias_bodies,
            ..
        } = ConstantSelection::with_retained_base(syntax, sources, bindings, base.as_deref())?;
        Ok(ConstantSelection {
            symbols,
            alias_bodies,
            array_lengths: Vec::new(),
            retained: base.map(SelectionBase::Shared),
        })
    }

    pub(crate) fn with_retained_base(
        syntax: &SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
        retained: Option<&'base crate::source_resolved::SymbolResolvedTrees>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut namespaces = crate::resolution::names::NamespaceDeclarations::default();
        let mut declarations = Vec::new();
        for item in syntax.root_items() {
            match item {
                Item::Const(definition) => declarations.push((
                    SymbolKind::Const,
                    crate::resolution::constants::semantic_const_name(syntax, definition),
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
                Item::Use(import) => namespaces.record_import(syntax, import),
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
        crate::resolution::names::spelled_integers(syntax).install(&mut symbols);
        namespaces.register(&mut symbols)?;
        Ok(Self {
            symbols,
            alias_bodies: AliasBody::collect(syntax).into(),
            retained: retained.map(SelectionBase::Borrowed),
            array_lengths: Vec::new(),
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
        reference: source_text::SourceSpan,
    ) -> Option<String> {
        if reference.span.start == reference.span.end || leaf.contains("::") {
            return None;
        }
        let mut candidates = Vec::new();
        for symbol in self.symbols.root_domain_children() {
            if self.symbols.name(symbol) != leaf
                || !self
                    .symbols
                    .source_reference_can_see_symbol(reference, symbol)
            {
                continue;
            }
            let exposed = self.symbols.source_domain_is_exposed(reference, symbol);
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
                let Some(parameter) = syntax.items.type_parameters(domain.type_parameters).first()
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
                        crate::source_resolved::data::TypeParameterKind::Type
                    ) && matches!(
                        &domain.target_type,
                        crate::source_resolved::types::TypeReference::Named { symbol, .. }
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
        for path in self.symbols.prelude_module_paths(reference.source_id) {
            let target = self
                .symbols
                .prelude_module_target(reference.source_id, path);
            let address = if target == Some(first) {
                path.to_owned()
            } else if self
                .symbols
                .prelude_module_exposes_domain(reference, path, first)
            {
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
    ) -> Option<&crate::source_resolved::data::DataDefinition> {
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
        matches!(&domain.target_type, crate::source_resolved::types::TypeReference::Named { symbol, .. } if *symbol == carrier)
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
        reference: source_text::SourceSpan,
    ) -> bool {
        if selected == symbol {
            return true;
        }
        // Test the kind before rendering a path: most root symbols are not
        // domains, and `display_path` allocates for each one it renders.
        if self.symbols.get(symbol).kind != SymbolKind::Domain {
            return false;
        }
        if authored.contains("::") && self.symbols.display_path(symbol, "::") == authored {
            return false;
        }
        self.symbols
            .source_reference_can_see_symbol(reference, symbol)
            && crate::resolution::names::domain_name_reaches(
                &self.symbols,
                symbol,
                self.symbols.name(symbol),
                authored,
                reference,
            )
    }

    /*
        The root symbols `domain_symbol_is_reachable` can accept for these
        selections, in root order. Only a selection itself passes without being
        a domain, so the domain roster serves whenever every valid selection is
        a domain; any other selection keeps the whole root roster.
    */
    fn reachable_domain_candidates(
        &self,
        selected: &[symbols::SymbolHandle],
    ) -> Box<dyn Iterator<Item = symbols::SymbolHandle> + '_> {
        if selected.iter().all(|symbol| {
            !symbol.is_valid() || self.symbols.get(*symbol).kind == SymbolKind::Domain
        }) {
            Box::new(self.symbols.root_domain_children())
        } else {
            Box::new(
                self.symbols
                    .child_handles(self.symbols.root())
                    .into_iter()
                    .flatten(),
            )
        }
    }

    /*
        Whether the authored spelling reaches any domain declaration visible
        from `reference` under module name law, in this forest or the
        retained base, whatever its carrier or index telescope.
    */
    pub(super) fn names_a_domain(
        &self,
        authored: &str,
        reference: source_text::SourceSpan,
    ) -> bool {
        let selected = self
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                authored,
                &[SymbolKind::Domain],
                reference,
            )
            .unwrap_or_else(symbols::SymbolHandle::invalid);
        selected.is_valid()
            || self
                .reachable_domain_candidates(&[selected])
                .any(|symbol| {
                    self.domain_symbol_is_reachable(symbol, selected, authored, reference)
                })
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
        reference: source_text::SourceSpan,
    ) -> bool {
        let selected = self
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                authored,
                &[SymbolKind::Domain],
                reference,
            )
            .unwrap_or_else(symbols::SymbolHandle::invalid);
        self.reachable_domain_candidates(&[selected])
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
        reference: source_text::SourceSpan,
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
        for symbol in self.reachable_domain_candidates(&[selected, relative_selected]) {
            if !self.domain_symbol_is_reachable(symbol, selected, authored, reference)
                && !relative.as_ref().is_some_and(|relative| {
                    self.domain_symbol_is_reachable(symbol, relative_selected, relative, reference)
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
        pool
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
        reference: source_text::SourceSpan,
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
        reference: source_text::SourceSpan,
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

    /// A bare family may share its leaf with a family on another nominal type.
    /// Its carrier can distinguish those already visible in the reference file.
    pub(super) fn domain_family_on_type<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
        carrier: syntax_trees::types::TypeReferenceHandle,
    ) -> Option<&'syntax DomainDefinition> {
        let authored = name.as_str(syntax);
        let reference = name.source_span();
        if let Some((_, family)) = self.domain_family(syntax, authored, reference, None) {
            return Some(family);
        }
        if authored.contains("::") {
            return None;
        }
        use syntax_trees::types::TypeReferenceNode;
        let owner = match syntax_type_references(syntax).type_reference(carrier) {
            TypeReferenceNode::Named(name)
            | TypeReferenceNode::Generic {
                base_name: name, ..
            } => name,
            _ => return None,
        };
        let qualified = format!("{}::{authored}", owner.as_str(syntax));
        let (symbol, family) = self.domain_family(syntax, &qualified, reference, None)?;
        self.domain_symbol_is_reachable(
            symbol,
            symbols::SymbolHandle::invalid(),
            authored,
            reference,
        )
        .then_some(family)
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
    ) -> Option<source_text::SourceSpan> {
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

    pub(crate) fn alias_body(&self, span: source_text::SourceSpan) -> Option<&AliasBody> {
        let mut local = self.alias_bodies.iter().filter(|(name, _)| *name == span);
        if let Some((_, body)) = local.next() {
            return local.next().is_none().then_some(body);
        }
        let mut retained = self
            .retained_base()?
            .type_aliases
            .iter()
            .filter(|alias| alias.name.source_span() == span);
        let selected = retained.next()?;
        retained.next().is_none().then_some(&selected.body)
    }

    pub(crate) fn has_aliases(&self) -> bool {
        !self.alias_bodies.is_empty()
            || self
                .retained_base()
                .is_some_and(|base| !base.type_aliases.is_empty())
    }

    pub(crate) fn generic_parameter_roles(
        &self,
        syntax: &SyntaxTrees,
        name: &Identifier,
        kinds: &[SymbolKind],
    ) -> Option<Vec<GenericParameterRole>> {
        self.generic_parameter_roles_among(syntax, None, name, kinds)
    }

    /*
        `generic_parameter_roles` for a caller that asks about many names in
        one forest and indexed its root headers once. Every candidate passes
        the same kind, span and spelling test in root order, read from the
        forest as it is now.
    */
    pub(crate) fn generic_parameter_roles_in(
        &self,
        syntax: &SyntaxTrees,
        headers: &RootHeaders,
        name: &Identifier,
        kinds: &[SymbolKind],
    ) -> Option<Vec<GenericParameterRole>> {
        self.generic_parameter_roles_among(syntax, Some(headers), name, kinds)
    }

    fn generic_parameter_roles_among(
        &self,
        syntax: &SyntaxTrees,
        headers: Option<&RootHeaders>,
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
        let candidates: Box<dyn Iterator<Item = syntax_trees::item::ItemHandle> + '_> =
            match headers {
                Some(headers) if headers.serves(syntax) => Box::new(headers.at(reference)),
                _ => Box::new(syntax.root_item_handles().iter().copied()),
            };
        let mut headers = candidates.filter_map(|handle| {
            let (kind, declaration, parameters) = match syntax.root_item(handle) {
                Item::Data(data) => (SymbolKind::Data, &data.name, data.type_parameters),
                Item::Domain(domain) => (SymbolKind::Domain, &domain.name, domain.type_parameters),
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
                && declaration.as_str(syntax) == selected_name)
                .then_some(parameters)
        });
        if let Some(result) = headers.next() {
            if headers.next().is_some() {
                return None;
            }
            return Some(
                syntax
                    .items
                    .type_parameters(result)
                    .iter()
                    .map(|parameter| match parameter.kind {
                        syntax_trees::item::TypeParameterKind::Type => GenericParameterRole::Type,
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
                        syntax_trees::item::TypeParameterKind::Proof { .. } => {
                            GenericParameterRole::Proof
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
        use crate::source_resolved::data::TypeParameterKind as Kind;
        let base = self.retained_base()?;
        let parameters = match selected_kind {
            SymbolKind::Data => base.data_definition_by_symbol(selected)?.type_parameters,
            SymbolKind::Domain => base.domain_definition_by_symbol(selected)?.type_parameters,
            SymbolKind::TypeAlias => {
                base.type_aliases
                    .iter()
                    .find(|row| row.symbol == selected)?
                    .type_parameters
            }
            SymbolKind::Machine => base.machines.unique_by_symbol(selected)?.type_parameters,
            SymbolKind::Trait => {
                base.traits
                    .iter()
                    .find(|row| row.symbol == selected)?
                    .type_parameters
            }
            SymbolKind::Conformance => {
                base.conformances
                    .iter()
                    .find(|row| row.symbol == selected)?
                    .type_parameters
            }
            _ => return None,
        };
        Some(
            base.data_type_parameters(parameters)
                .iter()
                .map(|parameter| match parameter.kind {
                    Kind::Type => GenericParameterRole::Type,
                    Kind::Implementation => GenericParameterRole::Implementation,
                    Kind::Const { .. } => GenericParameterRole::Const,
                    Kind::Value { .. } => GenericParameterRole::Value,
                    Kind::Machine { .. } => GenericParameterRole::Machine,
                    Kind::Proposition { .. } => GenericParameterRole::Proposition,
                    Kind::Proof { .. } => GenericParameterRole::Proof,
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
            constructor_type(&self.symbols, name.as_str(syntax), name.source_span())?.ok_or_else(
                || {
                    format!(
                        "`{name}` does not select one declared constructor carrier",
                        name = name.as_str(syntax)
                    )
                },
            )?;
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

    pub(super) fn install_integer_types(
        &mut self,
        carriers: impl IntoIterator<Item = numerics::integer_carrier::FixedInteger>,
    ) {
        self.symbols.install_integer_types(carriers);
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
            crate::source_symbols::SymbolLookup::Unique(symbol) => {
                Ok(Some(self.symbols.get(symbol).kind))
            }
            crate::source_symbols::SymbolLookup::NotFound => Ok(None),
            crate::source_symbols::SymbolLookup::Ambiguous { .. } => Err(Diagnostic::error(
                format!("type path owner `{}` is ambiguous", name.as_str(syntax)),
            )
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
    ) -> crate::source_symbols::SymbolLookup {
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
        let mut selected = self
            .symbols
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
        let candidates = [path, self.symbols.name(selected_header).to_owned()];
        for candidate in candidates.into_iter().chain(self.symbols.file_import_names(
            origin.reference.source_id,
            origin.declaration.source_id,
            selected_header,
        )) {
            if !selects(&candidate) {
                continue;
            }
            let name = Identifier::new(candidate, origin.reference);
            let definition = self.select(syntax, &name)?.ok_or_else(reject)?;
            if definition.name.source_span() != origin.declaration
                || syntax.expressions.source_span(definition.value()) != origin.initializer
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

    pub(crate) fn qualified_payload_free_case<'syntax>(
        &self,
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<Option<(&'syntax DataDefinition, Identifier)>, String> {
        let Some((owner, _, case)) = crate::resolution::names::qualified_payload_free_case_type(
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
            .ok_or_else(|| "implementation argument does not select one declaration".to_owned())?;
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
        let candidates: Box<dyn Iterator<Item = syntax_trees::item::ItemHandle> + '_> = match roots
        {
            Some(roots) if std::ptr::eq(roots.syntax, syntax) => Box::new(roots.at(span)),
            _ => Box::new(syntax.root_item_handles().iter().copied()),
        };
        let mut declarations = candidates.filter_map(|handle| {
            let (declaration_kind, declaration_name, is_public) = match syntax.root_item(handle) {
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
        let (declaration, is_public) = declarations
            .next()
            .ok_or_else(|| "selected canonical declaration lost its source custody".to_owned())?;
        if declarations.next().is_some() {
            return Err("selected canonical declaration has ambiguous source custody".to_owned());
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
        self.symbols
            .file_import_names(
                name.source_span().source_id,
                declaration.source_id,
                selected,
            )
            .find(|candidate| selects(candidate))
            .or_else(|| selects(name.as_str(syntax)).then(|| name.as_str(syntax).to_owned()))
    }

    /*
        The constant header one authored name selects from its own source
        scope, named by its declaration's source span. The header may
        belong to this forest or to the retained base, so the caller joins
        the span to whichever holds the declaration. A name that selects no
        constant, or more than one, has no header.
    */
    pub(crate) fn const_header(
        &self,
        syntax: &SyntaxTrees,
        name: &Identifier,
    ) -> Result<Option<source_text::SourceSpan>, Diagnostic> {
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
        self.symbols
            .symbol_source_span(selected)
            .map(Some)
            .ok_or_else(|| {
                Diagnostic::error("selected constant header has no retained declaration source")
                    .with_source_span(name.source_span())
            })
    }

    /* Geometry borrows completed values from this pass's resolved owner. */
    pub(crate) fn with_array_lengths(
        &self,
        mut lengths: Vec<(source_text::SourceSpan, Option<usize>)>,
    ) -> Self {
        lengths.sort_by_key(|(reference, _)| {
            (
                reference.source_id.0,
                reference.span.start,
                reference.span.end,
            )
        });
        let mut selection = self.clone();
        selection.array_lengths = lengths;
        selection
    }

    pub(crate) fn selected_array_length(
        &self,
        reference: source_text::SourceSpan,
    ) -> Result<usize, String> {
        let key = |reference: source_text::SourceSpan| {
            (
                reference.source_id.0,
                reference.span.start,
                reference.span.end,
            )
        };
        let first = self
            .array_lengths
            .partition_point(|(known, _)| key(*known) < key(reference));
        let mut selected = self.array_lengths[first..]
            .iter()
            .take_while(|(known, _)| *known == reference);
        let length = selected
            .next()
            .and_then(|(_, value)| *value)
            .ok_or_else(|| {
                "array extent requires a completed integer value selected in its lexical scope"
                    .to_owned()
            })?;
        if selected.any(|(_, other)| *other != Some(length)) {
            return Err("array extent occurrence has conflicting selected values".to_owned());
        }
        Ok(length)
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
        let Some(declaration_span) = self.const_header(syntax, name)? else {
            return Ok(None);
        };
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
    builder: &mut impl crate::source_symbols::SymbolTableAppender,
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
    Root data and conformance declarations of one borrowed forest, ordered by
    their name span. The borrow keeps the forest unchanged while the index
    lives; ties keep root order.
*/
pub(crate) struct RootDeclarations<'syntax> {
    syntax: &'syntax SyntaxTrees,
    headers: RootHeaders,
}

impl<'syntax> RootDeclarations<'syntax> {
    pub(crate) fn syntax(&self) -> &'syntax SyntaxTrees {
        self.syntax
    }

    pub(crate) fn of(syntax: &'syntax SyntaxTrees) -> Self {
        Self {
            syntax,
            headers: RootHeaders::of(syntax),
        }
    }

    /* Root declaration items named at `span`, in root order. */
    pub(crate) fn at(
        &self,
        span: source_text::SourceSpan,
    ) -> impl Iterator<Item = syntax_trees::item::ItemHandle> + '_ {
        self.headers.at(span)
    }
}

/*
    The root items of one forest that declare a generic header (data, domain,
    alias, machine, trait or named conformance), ordered by their name span;
    ties keep root order.

    The rows name items, not their contents. A caller that rewrites headers
    in place, such as declared-type normalization changing a parameter's
    kind, keeps the index and reads each candidate's current header. A caller
    holding the index must still re-test every candidate, because the index
    only narrows the search to one name span. `serves` refuses a forest
    whose root roster has grown since the index was built.
*/
pub(crate) struct RootHeaders {
    root_count: usize,
    rows: Vec<((usize, usize, usize), usize, syntax_trees::item::ItemHandle)>,
}

impl RootHeaders {
    pub(crate) fn of(syntax: &SyntaxTrees) -> Self {
        let mut rows = syntax
            .root_item_handles()
            .iter()
            .enumerate()
            .filter_map(|(position, handle)| {
                let name = match syntax.root_item(*handle) {
                    Item::Data(data) => &data.name,
                    Item::Domain(domain) => &domain.name,
                    Item::TypeAlias(alias) => &alias.name,
                    Item::Machine(machine) => &machine.name,
                    Item::Trait(definition) => &definition.name,
                    Item::Conformance(implementation) => implementation.alias.as_ref()?,
                    _ => return None,
                };
                Some((span_key(name.source_span()), position, *handle))
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(key, position, _)| (*key, *position));
        Self {
            root_count: syntax.root_item_handles().len(),
            rows,
        }
    }

    /* Whether these rows still list every root item of `syntax`. */
    pub(crate) fn serves(&self, syntax: &SyntaxTrees) -> bool {
        syntax.root_item_handles().len() == self.root_count
    }

    /* Root header items named at `span`, in root order. */
    pub(crate) fn at(
        &self,
        span: source_text::SourceSpan,
    ) -> impl Iterator<Item = syntax_trees::item::ItemHandle> + '_ {
        let key = span_key(span);
        let start = self.rows.partition_point(|(row, _, _)| *row < key);
        self.rows[start..]
            .iter()
            .take_while(move |(row, _, _)| *row == key)
            .map(|(_, _, handle)| *handle)
    }
}

fn span_key(span: source_text::SourceSpan) -> (usize, usize, usize) {
    (span.source_id.0, span.span.start, span.span.end)
}

/*
    Only the deferred frontier's owned predecessor is persistent. Normalization's
    temporary borrow cannot escape as an independently restorable source owner.
*/
impl record_storage::Record for SelectionBase<'_> {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        match self {
            Self::Shared(base) => record_storage::Record::write(base, output),
            Self::Borrowed(_) => {
                Err(output.error("borrowed declaration selection is not a persistent frontier"))
            }
        }
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        record_storage::Record::read(input).map(Self::Shared)
    }
}

#[cfg(test)]
mod retained_namespace_tests {
    use super::*;
    #[test]
    fn early_declaration_selection_retains_directory_namespace_names() {
        let mut files = SourceMap::default();
        let source = files.add_with_metadata(
            "package/example/definitions.omg".into(), "imported".into(), "package".into(),
            None, source_admission::SourceOrigin::User,
        ).source_id;
        let syntax = SyntaxTrees::new(source);
        let selection = ConstantSelection::new(&syntax, Some(Arc::new(files)), Vec::new()).unwrap();
        let symbols = selection.header_symbols();
        assert_eq!(symbols.display_path(symbols.source_module(source), "::"), "example");
    }
}
