/*
    Carriers a later phase continues from.

    `ConstInitializerSelection` is what semantic initializer evaluation reads:
    resolved trees whose computed initializers are still authored expression
    roots, beside the selector that named their declarations. The seeded
    carriers hold an extension resolved against a retained base; the typed
    continuation must rebase the extension's authored selections onto its own
    ledger before the trees can enter it.
*/

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;

/*
    Private preparation evidence for semantic initializer evaluation. The forest
    retains unresolved values and operator obligations; it is not a completed
    resolution result and grants no authority to type, execute, or publish it.
    Declaration relationships and their pending source/body work remain under
    the same owner; a read-only graph query does not discard that frontier.
*/
pub struct ConstInitializerSelection {
    pub(super) frontier: crate::DeferredResolvedFrontier,
}

/*
    Encoding for private initializer preparation. Its bytes carry no domain
    evidence; the exact pending destination must still pass ordinary checking.
*/
pub struct ConstInitializerPayload {
    pub value: language_semantics::const_value::CanonicalConstValue,
    pub pending_destination: Option<ConstDestinationObligation>,
}

use crate::constants::initializer_dependencies::ConstInitializerDependencies;
use crate::constants::initializer_dependencies::DependencyPurpose;
use crate::constants::pending_const_initializer_leaves;
pub use syntax_trees::item::ConstDestinationObligation;

impl ConstInitializerSelection {
    pub fn retained_constant(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<&symbol_resolved_trees::constant::ConstDeclaration, Vec<Diagnostic>> {
        let base = self.frontier.selection.retained_base().ok_or_else(|| {
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

    /*
        Exact completed input from this selection's predecessor. New declarations
        cannot borrow readiness or custody merely by having the same name.
    */
    pub fn retained_constant_custody(
        &self,
        symbol: symbols::SymbolHandle,
        purpose: crate::constants::initializer_normalization::InitializerCustodyPurpose,
    ) -> Result<syntax_trees::item::ConstInitializerCustody, Vec<Diagnostic>> {
        let base = self.frontier.selection.retained_base().ok_or_else(|| {
            vec![Diagnostic::error(
                "constant input has no retained predecessor",
            )]
        })?;
        let declaration = self.retained_constant(symbol)?;
        crate::constants::initializer_normalization::retained_constant_custody(
            base,
            declaration,
            purpose,
        )
        .map_err(|diagnostic| vec![diagnostic])
    }

    pub fn initializer_dependencies(
        &self,
        _syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
        self.declaration_dependencies(
            definition,
            DependencyPurpose::DeclarationReadiness,
            definition
                .normalization
                .as_ref()
                .map(|receipt| receipt.declaration_custody.call_selections.as_slice()),
        )
    }

    fn declaration_dependencies(
        &self,
        definition: &syntax_trees::item::ConstDefinition,
        purpose: DependencyPurpose,
        selected_calls: Option<&[(source::SourceSpan, source::SourceSpan)]>,
    ) -> Result<ConstInitializerDependencies, Vec<Diagnostic>> {
        let declaration = self
            .frontier
            .trees
            .const_declaration_at_source_span(definition.name.source_span())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "initializer dependencies lost their exact declaration",
                )]
            })?;
        crate::constants::initializer_dependencies::collect_declaration(
            &self.frontier.trees,
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            },
            &declaration.declared_type,
            purpose,
            self.frontier.selection.retained_base(),
            selected_calls,
        )
        .map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
        })
    }

    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.frontier.trees
    }

    /*
        Every declaration that needs initializer evaluation kept its exact
        declaration and can name its pending scalar leaves.
    */
    pub(crate) fn validate_initializer_leaves(
        &self,
        syntax: &SyntaxTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        for definition in syntax.root_items().filter_map(|item| match item {
            syntax_trees::item::Item::Const(definition)
                if crate::constants::requires_const_initializer_evaluation(syntax, definition) =>
            {
                Some(definition)
            }
            _ => None,
        }) {
            self.frontier
                .trees
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
    ) -> Result<Vec<crate::constants::PendingConstInitializerLeaf>, Vec<Diagnostic>> {
        pending_const_initializer_leaves(syntax, definition, &self.frontier.selection).map_err(
            |reason| {
                vec![Diagnostic::error(reason).with_source_span(definition.name.source_span())]
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pre_resolution::prepare_const_initializer_selection;
    use crate::{ResolutionRequest, resolve};
    use std::sync::Arc;

    fn parse(text: &str, source: source::SourceId) -> SyntaxTrees {
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap()
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

    /*
        A later stratum applies a transparent alias its retained base
        declares, as std's macOS filesystem ABI applies core's
        `CLayout<T>`. The alias expands from the base's authored
        declaration; without that forest the application is still refused
        rather than left unexpanded.
    */
    #[test]
    fn extension_expands_an_alias_declared_by_its_retained_base() {
        let mut sources = source::SourceMap::default();
        let base_text = "public data Pair<T> = [T; 2];";
        let base_source = sources
            .add("aliases/base.omg".into(), base_text.into())
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
        // The extension declares an alias of its own, as std does, so it
        // enters alias expansion at all.
        let text = "public data Byte = u8; public data Holder { value: Pair<Byte>; }";
        let source = sources
            .add("aliases/current.omg".into(), text.into())
            .source_id;
        let sources = Arc::new(sources);
        let extend = |base_declarations: SyntaxTrees| {
            crate::resolve_extension(crate::ExtensionRequest {
                base: Arc::clone(&base),
                base_declarations: Arc::new(base_declarations),
                syntax: parse(text, source),
                sources: Arc::clone(&sources),
                top_level_bindings: Vec::new(),
            })
        };
        let extended = extend(parse(base_text, base_source)).unwrap();
        assert!(
            extended
                .trees()
                .data_definitions
                .iter()
                .any(|data| data.name.as_str() == "Holder")
        );
        let refused = extend(SyntaxTrees::new(base_source)).err().unwrap();
        assert!(refused.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("selected type alias has no retained source declaration")
        }));
    }

    #[test]
    fn implicit_call_identity_uses_the_selected_entry_regardless_of_child_order() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        let declaration = source::SourceSpan::new(source::SourceId(0), source::Span::new(0, 6));
        for (names, selected) in [(["other", "entry"], 1), (["entry", "helper"], 1)] {
            let mut builder = SymbolTableBuilder::default();
            let machine = builder.insert_root(
                SymbolKind::Machine,
                SymbolNameRef::OwnedSource {
                    value: "helper",
                    source_span: declaration,
                },
            );
            let children = builder.insert_children(
                machine,
                names.map(|name| (SymbolKind::State, SymbolNameRef::Static(name))),
            );
            let program = SymbolResolvedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let custody = crate::selections::call_identity::call_declaration_source;
            assert_eq!(
                custody(&program, children.handle_at(selected)),
                Ok(declaration)
            );
            assert!(
                custody(&program, children.handle_at(0))
                    .unwrap_err()
                    .contains("not its exact owning entry")
            );
        }
    }

    #[test]
    fn implicit_call_entry_retains_its_authored_machine_source() {
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
            crate::selections::call_identity::call_declaration_source(&program, entry).unwrap(),
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
            crate::selections::call_identity::call_declaration_source(&program, extra)
                .unwrap_err()
                .contains("not its exact owning entry")
        );
    }
}
