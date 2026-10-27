//! Source and package queries: span-to-text lookup, symbol provenance,
//! and package-identity checks, extracted from `table` so storage and
//! name navigation stay the headline.

use super::*;

impl SymbolTable {
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

    /// The retained authored source map, shared without copying. Consumers
    /// that resolve diagnostic locations on a path where the program moves
    /// away clone this handle rather than reading storage back.
    pub fn source_map(&self) -> Option<std::sync::Arc<source::SourceMap>> {
        self.sources.clone()
    }

    /// Whether `symbol` was authored in the toolchain-owned build prelude
    /// translation unit.
    pub fn is_build_prelude_symbol(&self, symbol: SymbolHandle) -> bool {
        self.symbol_source_span(symbol)
            .and_then(|span| self.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path == std::path::Path::new("<build-prelude>")
            })
    }

    /// Product-scope dependency `alias` authorized for the package that owns
    /// `occurrence`. `None` means the occurrence carries no package identity,
    /// no retained sources, or no such product edge -- callers treat every
    /// one of those as "not an authorized product alias", never as license
    /// to widen the search.
    pub fn product_dependency_target(
        &self,
        occurrence: SourceSpan,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.sources
            .as_deref()?
            .product_dependency_at(occurrence, alias)
    }

    /// Resolve a designated product operand under its author's package scope.
    /// Callers supply declaration candidates and their public visibility; this
    /// never extends ordinary source lookup. Exact module paths take precedence
    /// over same-package short names. Operator overloads at one exact path name
    /// one family, represented by its first declaration in candidate order.
    pub fn find_product_declaration_from_source(
        &self,
        path: &str,
        occurrence: SourceSpan,
        candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
    ) -> Option<SymbolHandle> {
        self.find_declaration_from_source(path, occurrence, candidates, false, &[])
    }

    /// [`Self::find_product_declaration_from_source`] for a BUILD operand,
    /// which may also name a public toolchain declaration.
    ///
    /// The toolchain injects its own core into every product without an
    /// authored dependency, so a core declaration has no package instance an
    /// unqualified operand could share and no manifest an alias could name.
    /// A build that selects a provider for `ExtentRootProvider` is naming that
    /// exact declaration. Ordinary call resolution keeps the narrower rule:
    /// admitting core there changes which declaration a call selects.
    /// `token_bound_machines` identifies the machine declarations whose token
    /// views share this family-selection rule. Ordinary same-path machines
    /// remain ambiguous; exact path, package and source scope must agree even
    /// for declared family members. This selects a family representative, not
    /// a callable overload or provider.
    pub fn find_build_subject_declaration_from_source(
        &self,
        path: &str,
        occurrence: SourceSpan,
        candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
        token_bound_machines: &[SymbolHandle],
    ) -> Option<SymbolHandle> {
        self.find_declaration_from_source(path, occurrence, candidates, true, token_bound_machines)
    }

    fn find_declaration_from_source(
        &self,
        path: &str,
        occurrence: SourceSpan,
        candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
        admit_toolchain: bool,
        token_bound_machines: &[SymbolHandle],
    ) -> Option<SymbolHandle> {
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
                || !match dependency {
                    Some(package) => {
                        is_public && self.symbol_product_package_identity(symbol) == Some(package)
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

    /// A designated Product bridge keeps the ordinary temporal/stratum policy
    /// while its caller separately admits exact Product package visibility.
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

    /// Compare declaration provenance at the lexical package boundary: the
    /// same package identity AND the same checked dependency scope. Two
    /// checked instances of one source are the same package nominally but
    /// share no lexical context — module namespaces, package-private
    /// visibility, and same-package selection never cross instances
    /// (wiki/spec/build/scoped_execution.md, "Two checked contexts").
    /// Source-free lowering is used by focused representation tests; those
    /// trees model one package and retain the historical all-local behavior.
    pub fn same_source_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
        self.sources
            .as_deref()
            .is_none_or(|sources| sources.same_checked_instance(left, right))
    }

    /// Whether `declaration` is the product-scope checked instance inside
    /// `reference`'s package — the scope-crossing match for the bridges that
    /// deliberately name a product declaration from a build occurrence (root
    /// bindings, product entry/schema/provider queries). Ordinary resolution
    /// never uses this.
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
        sources.same_checked_instance(left, right)
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

    /// Reconciled package identity only for its product checked instance.
    /// A product dependency authorizes selection from that instance, not from
    /// an independently checked build copy of the same package/source bytes.
    /// Generated symbols retain the scope of their authored derivation origin.
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

    /// Exact frontend source custody retained beside authored symbol spans.
    /// This exposes source-map facts, not symbol identity or a stable package
    /// format; compiler-internal consumers remain responsible for canonical
    /// framing.
    pub fn source_files(&self) -> impl Iterator<Item = &SourceFile> {
        self.sources.iter().flat_map(|sources| sources.files())
    }

    /// The authored symbol a generated symbol derives from, or `symbol`
    /// itself when it was authored. Specialization mints machine symbols whose
    /// names carry a selection fingerprint, and that spelling belongs to the
    /// compiler, not to a reader: a diagnostic about a specialized callee names
    /// the declaration its author wrote. The walk is the same derivation chain
    /// `provenance_source_span` follows, and a generated symbol always records
    /// one origin that precedes it, so it terminates.
    pub fn authored_origin(&self, mut symbol: SymbolHandle) -> SymbolHandle {
        while symbol.is_valid() {
            let generated_from = self.get(symbol).generated_from;
            if !generated_from.is_valid() || generated_from.arena_index() >= symbol.arena_index() {
                return symbol;
            }
            symbol = generated_from;
        }
        symbol
    }

    pub(super) fn provenance_source_span(&self, mut symbol: SymbolHandle) -> Option<SourceSpan> {
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
