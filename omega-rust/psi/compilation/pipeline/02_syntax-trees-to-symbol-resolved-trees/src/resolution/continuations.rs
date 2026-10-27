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
    pub(crate) selection: crate::preparation::declaration_selection::ConstantSelection<'static>,
}

/// Encoding for private initializer preparation. Its bytes carry no domain
/// evidence; the exact pending destination must still pass ordinary checking.
pub struct ConstInitializerPayload {
    pub value: language_semantics::const_value::CanonicalConstValue,
    pub pending_destination: Option<ConstDestinationObligation>,
}

pub use syntax_trees::item::ConstDestinationObligation;

impl ConstInitializerSelection {
    pub fn retained_constant(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<&symbol_resolved_trees::constant::ConstDeclaration, Vec<Diagnostic>> {
        let base = self.selection.retained_base().ok_or_else(|| {
            vec![Diagnostic::error(
                "constant input has no retained predecessor",
            )]
        })?;
        base.const_declaration(symbol)
            .filter(|_| base.symbols.symbol_source_is_selected(symbol))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "constant input is not a selected predecessor declaration",
                )]
            })
    }

    /// Exact completed input from this selection's predecessor. New declarations
    /// cannot borrow readiness or custody merely by having the same name.
    pub fn retained_constant_custody(
        &self,
        symbol: symbols::SymbolHandle,
        purpose: crate::constant::initializer_normalization::InitializerCustodyPurpose,
    ) -> Result<syntax_trees::item::ConstInitializerCustody, Vec<Diagnostic>> {
        let base = self.selection.retained_base().ok_or_else(|| {
            vec![Diagnostic::error(
                "constant input has no retained predecessor",
            )]
        })?;
        let declaration = self.retained_constant(symbol)?;
        crate::constant::initializer_normalization::retained_constant_custody(
            base,
            declaration,
            purpose,
        )
        .map_err(|diagnostic| vec![diagnostic])
    }

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
            .const_declaration_at_source_span(definition.name.source_span())
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
        self.declaration_dependencies(
            definition,
            crate::constant::initializer_dependencies::DependencyPurpose::DeclarationReadiness,
        )
    }

    pub fn initializer_authored_dependencies(
        &self,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<
        crate::constant::initializer_dependencies::ConstInitializerDependencies,
        Vec<Diagnostic>,
    > {
        self.declaration_dependencies(
            definition,
            crate::constant::initializer_dependencies::DependencyPurpose::Authored,
        )
    }

    fn declaration_dependencies(
        &self,
        definition: &syntax_trees::item::ConstDefinition,
        purpose: crate::constant::initializer_dependencies::DependencyPurpose,
    ) -> Result<
        crate::constant::initializer_dependencies::ConstInitializerDependencies,
        Vec<Diagnostic>,
    > {
        let declaration = self
            .trees
            .const_declaration_at_source_span(definition.name.source_span())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "initializer dependencies lost their exact declaration",
                )]
            })?;
        crate::constant::initializer_dependencies::collect_declaration(
            &self.trees,
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            },
            &declaration.declared_type,
            purpose,
            self.selection.retained_base(),
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
                .const_declaration_at_source_span(definition.name.source_span())
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

    /// Encode completed initializer payloads, without asserting the destination's
    /// qualification. The compile-time owner checks that separate result judgment
    /// before returning the authoritative declaration to normal resolution.
    pub fn canonicalize_initializer_payload(
        &self,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<ConstInitializerPayload, String> {
        crate::preparation::generic_data::canonicalize_pending_initializer_payload(
            syntax,
            definition,
            Some(&self.selection),
        )
    }

    pub fn initializer_requires_destination_proof(
        &self,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> bool {
        crate::preparation::generic_data::const_destination_requires_proof(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolution::{ResolutionRequest, prepare_const_initializer_selection, resolve};
    use std::sync::Arc;

    fn parse(text: &str, source: source::SourceId) -> SyntaxTrees {
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap()
    }

    #[test]
    fn initializer_normalization_keeps_new_work_and_retained_call_dependencies() {
        let mut sources = source::SourceMap::default();
        let base_text = "public machine bound(value: u64) -> u64 { value }";
        let base_source = sources
            .add("initializers/base.omg".into(), base_text.into())
            .source_id;
        let base = Arc::new(
            resolve(ResolutionRequest {
                syntax: parse(base_text, base_source),
                sources: Some(Arc::new(sources.clone())),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap(),
        );
        let text =
            "machine local(value: u64) -> u64 { bound(value) }\nconst Count: u64 = local(3u64);";
        let source = sources
            .add("initializers/current.omg".into(), text.into())
            .source_id;
        let syntax = parse(text, source);
        let selected = prepare_const_initializer_selection(ResolutionRequest {
            syntax: syntax.clone(),
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: Some(base),
        })
        .unwrap();
        let definition = syntax
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Const(definition) => Some(definition),
                _ => None,
            })
            .unwrap();
        let dependencies = selected
            .initializer_dependencies(&syntax, definition)
            .unwrap();
        assert_eq!(
            dependencies.calls.len(),
            2,
            "both helper calls remain obligations"
        );
        assert_eq!(dependencies.normalization_owners.len(), 1);
        assert_eq!(dependencies.normalization_owners[0].source_id, source);
        assert!(
            dependencies
                .calls
                .iter()
                .any(|(_, declaration)| declaration.source_id == base_source)
        );
    }

    #[test]
    fn retained_constant_custody_cannot_supply_a_new_declarations_value() {
        let mut sources = source::SourceMap::default();
        let text = "public const Ceiling: u64 = 3u64;";
        let source = sources.add("inputs/base.omg".into(), text.into()).source_id;
        let base = Arc::new(
            resolve(ResolutionRequest {
                syntax: parse(text, source),
                sources: Some(Arc::new(sources.clone())),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap(),
        );
        let retained = base.const_declarations[0].symbol;
        let current_text = "const Fresh: u64 = Ceiling;";
        let current = sources
            .add("inputs/current.omg".into(), current_text.into())
            .source_id;
        let selected = prepare_const_initializer_selection(ResolutionRequest {
            syntax: parse(current_text, current),
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: Some(base),
        })
        .unwrap();
        assert!(
            selected
                .retained_constant(retained)
                .unwrap()
                .evaluated_value
                .is_some()
        );
        for purpose in [
            crate::pre_resolution::InitializerCustodyPurpose::Authored,
            crate::pre_resolution::InitializerCustodyPurpose::Declaration,
            crate::pre_resolution::InitializerCustodyPurpose::Value,
        ] {
            let custody = selected
                .retained_constant_custody(retained, purpose)
                .unwrap();
            assert!(custody.selections.is_empty());
            assert!(custody.builtin_operators.is_empty());
            assert!(custody.call_selections.is_empty());
        }
        let fresh = selected
            .trees()
            .const_declarations
            .iter()
            .find(|declaration| {
                selected
                    .trees()
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .is_some_and(|span| span.source_id == current)
            })
            .unwrap();
        assert!(selected.retained_constant(fresh.symbol).is_err());
    }

    #[test]
    fn implicit_call_identity_requires_the_exact_first_state() {
        let text = "machine helper(value: u64) -> u64 { value }";
        let mut sources = source::SourceMap::default();
        let source = sources.add("calls/main.omg".into(), text.into()).source_id;
        let mut program = resolve(ResolutionRequest {
            syntax: parse(text, source),
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let machine = &program.machines[0];
        let machine_symbol = machine.symbol;
        let entry = program
            .machine_state(program.machine_state_handles(machine.states)[0])
            .symbol;
        assert!(program.symbols.symbol_source_span(entry).is_none());
        assert_eq!(
            crate::constant::initializer_dependencies::call_declaration_source(&program, entry)
                .unwrap(),
            program.symbols.symbol_source_span(machine_symbol).unwrap()
        );
        let extra = program.symbols.append_children(
            machine_symbol,
            [(
                symbols::SymbolKind::State,
                symbols::SymbolNameRef::Static("not_entry"),
            )],
        );
        let extra = symbols::SymbolTableBuilder::child_handles(extra)
            .next()
            .unwrap();
        assert!(
            crate::constant::initializer_dependencies::call_declaration_source(&program, extra)
                .unwrap_err()
                .contains("not its exact owning entry")
        );
    }
}
