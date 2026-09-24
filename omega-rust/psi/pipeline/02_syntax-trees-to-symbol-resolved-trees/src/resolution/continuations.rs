//! Carriers a later phase continues from.
//!
//! `ConstInitializerSelection` is what semantic initializer evaluation reads:
//! resolved trees whose computed initializers are still authored expression
//! roots, beside the selector that named their declarations. The seeded
//! carriers hold an extension resolved against a retained base; the typed
//! continuation must rebase the extension's authored selections onto its own
//! ledger before the trees can enter it.

use diagnostics::Diagnostic;
use symbol_resolved_trees::{
    AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
    AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees,
};
use syntax_trees::SyntaxTrees;

/// Private preparation evidence for semantic initializer evaluation. The forest
/// retains unresolved values and operator obligations; it is not a completed
/// resolution result and grants no authority to type, execute, or publish it.
pub struct ConstInitializerSelection {
    pub(crate) trees: SymbolResolvedTrees,
    pub(crate) selection:
        crate::preparation::generic_data::constant_selection::ConstantSelection<'static>,
}

impl ConstInitializerSelection {
    pub fn initializer_expression_dependencies(
        &self,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
        expression: syntax_trees::expression::ExpressionHandle,
    ) -> Result<
        crate::constant::initializer_dependencies::ConstInitializerDependencies,
        Vec<Diagnostic>,
    > {
        if !self
            .pending_leaves(syntax, definition)?
            .iter()
            .any(|leaf| leaf.expression == expression)
        {
            return Err(vec![
                Diagnostic::error("dependency request is not an authored scalar initializer leaf")
                    .with_source_span(definition.name.source_span()),
            ]);
        }
        let declaration = self
            .trees
            .const_declarations
            .iter()
            .find(|declaration| {
                self.trees.symbols.symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| vec![Diagnostic::error("initializer leaf lost its declaration")])?;
        let root = if declaration.authored_initializer.is_valid() {
            declaration.authored_initializer
        } else {
            declaration.initializer
        };
        crate::constant::initializer_dependencies::expression_at_source(
            &self.trees,
            root,
            syntax.expressions.source_span(expression),
        )
        .and_then(|expression| {
            crate::constant::initializer_dependencies::collect(&self.trees, expression)
        })
        .map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
        })
    }

    pub fn initializer_dependencies(
        &self,
        _syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<
        crate::constant::initializer_dependencies::ConstInitializerDependencies,
        Vec<Diagnostic>,
    > {
        let declaration = self
            .trees
            .const_declarations
            .iter()
            .find(|declaration| {
                self.trees.symbols.symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "initializer dependencies lost their exact declaration",
                )]
            })?;
        crate::constant::initializer_dependencies::collect(
            &self.trees,
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            },
        )
        .map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
        })
    }

    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    /// Every declaration that needs initializer evaluation kept its exact
    /// declaration and can name its pending scalar leaves.
    pub(crate) fn validate_initializer_leaves(
        &self,
        syntax: &SyntaxTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        for definition in syntax.root_items().filter_map(|item| match item {
            syntax_trees::item::Item::Const(definition)
                if crate::constant::requires_const_initializer_evaluation(syntax, definition) =>
            {
                Some(definition)
            }
            _ => None,
        }) {
            self.trees
                .const_declarations
                .iter()
                .find(|declaration| {
                    self.trees.symbols.symbol_source_span(declaration.symbol)
                        == Some(definition.name.source_span())
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "initializer preparation lost its exact declaration",
                    )]
                })?;
            self.pending_leaves(syntax, definition)?;
        }
        Ok(())
    }

    pub fn pending_leaves(
        &self,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<Vec<crate::constant::PendingConstInitializerLeaf>, Vec<Diagnostic>> {
        crate::constant::pending_const_initializer_leaves(syntax, definition, &self.selection)
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
            })
    }

    /// Synthesize one closed zero literal for a pending structured leaf so the
    /// surrounding probe forest can type. The placeholder is preparation-only:
    /// it is replaced by the evaluated literal before any value publishes.
    pub fn pending_value_placeholder(
        &self,
        syntax: &mut SyntaxTrees,
        destination: syntax_trees::types::TypeReferenceHandle,
        reference: source::SourceSpan,
    ) -> Result<syntax_trees::expression::ExpressionHandle, Vec<Diagnostic>> {
        crate::constant::pending_aggregate_placeholder(
            syntax,
            &self.selection,
            destination,
            reference,
        )
        .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])
    }

    pub fn canonicalize_value(
        &self,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<language_semantics::const_value::CanonicalConstValue, String> {
        crate::preparation::generic_data::canonicalize_selected_declared_const_definition(
            syntax,
            definition,
            Some(&self.selection),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SeededSymbolResolvedTrees {
    pub(crate) trees: SymbolResolvedTrees,
    pub(crate) authored_selection_frontier: AuthoredSelectionExtensionFrontier,
    pub(crate) retained_base: Box<SymbolResolvedTrees>,
}

/// A seeded resolved extension whose authored-selection suffix has been
/// joined to the destination typed base ledger. The exact resolved base stays
/// inside the carrier so the next phase can reject cross-paired continuations.
#[derive(Debug, PartialEq, Eq)]
pub struct RebasedSeededSymbolResolvedTrees {
    trees: SymbolResolvedTrees,
    retained_base: Box<SymbolResolvedTrees>,
}

impl SeededSymbolResolvedTrees {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    pub fn rebase_authored_selections_for_typed_continuation(
        self,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<RebasedSeededSymbolResolvedTrees, (Self, AuthoredSelectionExtensionRebaseError)>
    {
        // This is a representation join, not an authority boundary. The
        // seeded typed continuation must supply the ledger owned by its exact
        // retained base rather than accepting one from compilation input.
        match self
            .trees
            .rebase_authored_selection_extension(self.authored_selection_frontier, destination_base)
        {
            Ok(trees) => Ok(RebasedSeededSymbolResolvedTrees {
                trees,
                retained_base: self.retained_base,
            }),
            Err((trees, error)) => Err((Self { trees, ..self }, error)),
        }
    }
}

impl RebasedSeededSymbolResolvedTrees {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    pub fn into_typing_continuation_parts(self) -> (SymbolResolvedTrees, SymbolResolvedTrees) {
        (self.trees, *self.retained_base)
    }
}
