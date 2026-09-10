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
use syntax_trees::item::{ConstDefinition, DataDefinition, Item};

/// Private selection state; transient symbols never escape into normalized syntax.
pub(crate) struct ConstantSelection {
    symbols: SymbolTable,
}

impl ConstantSelection {
    pub(crate) fn new(
        syntax: &SyntaxTrees,
        sources: Option<Arc<SourceMap>>,
        bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut namespaces = crate::symbols::NamespaceDeclarations::default();
        let mut declarations = Vec::new();
        for item in syntax.root_items() {
            match item {
                Item::Const(definition) => declarations.push((
                    SymbolKind::Const,
                    crate::constant::semantic_const_name(definition),
                    definition.name.source_span(),
                )),
                Item::Data(definition) => declarations.push((
                    SymbolKind::Data,
                    definition.name.as_str().to_owned(),
                    definition.name.source_span(),
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
        let mut builder =
            SymbolTableBuilder::with_sources_and_top_level_bindings(sources, bindings);
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(declarations.iter().map(|(kind, name, source_span)| {
                    (
                        *kind,
                        SymbolNameRef::OwnedSource {
                            value: name,
                            source_span: *source_span,
                        },
                    )
                })),
        );
        let mut symbols = builder.finish();
        // Machine, trait and other nondata import targets are absent from this
        // partial header table. Complete resolution validates those imports.
        namespaces.register(&mut symbols)?;
        Ok(Self { symbols })
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
