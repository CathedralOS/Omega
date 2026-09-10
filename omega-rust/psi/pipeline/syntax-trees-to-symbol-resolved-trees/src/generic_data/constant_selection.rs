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
        // Machine, trait and other nondata import targets are absent from this
        // partial header table. Complete resolution validates those imports.
        namespaces.register(&mut symbols)?;
        Ok(Self { symbols, retained })
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

    // This selects normalization identity under source strata. Domain lowering
    // retains each authored occurrence; final package admission independently
    // checks its declaration visibility and direct dependency authority.
    pub(super) fn current_domain_source_matches(
        &self,
        definition: &syntax_trees::item::DomainDefinition,
        reference: &Identifier,
    ) -> bool {
        let Some(symbol) = self.symbols.find_top_level_by_name_and_kinds_from_source(
            definition.name.as_str(),
            &[SymbolKind::Domain],
            definition.name.source_span(),
        ) else {
            return false;
        };
        self.symbols
            .source_reference_can_see_symbol(reference.source_span(), symbol)
            && (!reference.as_str().contains("::")
                || self.symbols.find_top_level_by_name_and_kinds_from_source(
                    reference.as_str(),
                    &[SymbolKind::Domain],
                    reference.source_span(),
                ) == Some(symbol))
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

    /// Select the nominal carrier in the authored type/constructor's source.
    /// Header handles remain transient; declaration custody is rejoined after
    /// ordinary resolution allocates the receiving generic slot.
    pub(super) fn data<'syntax>(
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

    pub(super) fn constructor<'syntax>(
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
        None
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

#[cfg(test)]
mod tests {
    use super::*;
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
