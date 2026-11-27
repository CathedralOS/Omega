/*
    Carriers a later phase continues from.

    `ConstInitializerSelection` is what semantic initializer evaluation reads:
    resolved trees whose computed initializers are still authored expression
    roots, beside the selector that named their declarations. The seeded
    carriers hold an extension resolved against a retained base; the typed
    continuation must rebase the extension's authored selections onto its own
    ledger before the trees can enter it.
*/

use crate::source_resolved::SymbolResolvedTrees;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;

/*
    Private preparation evidence for semantic initializer evaluation. The forest
    retains unresolved values and operator obligations; it is not a completed
    resolution result and grants no authority to type, execute, or publish it.
    Declaration relationships and their pending source/body work remain under
    the same owner; a read-only graph query does not discard that frontier.
*/
pub struct ConstInitializerSelection {
    pub(super) frontier: crate::resolution::DeferredResolvedFrontier,
}

/*
    Encoding for private initializer preparation. Its bytes carry no domain
    evidence; the exact pending destination must still pass ordinary checking.
*/
pub struct ConstInitializerPayload {
    pub value: application_identity::const_value::CanonicalConstValue,
    pub pending_destination: Option<ConstDestinationObligation>,
}

use crate::resolution::constants::initializer_dependencies::ConstInitializerDependencies;
use crate::resolution::constants::initializer_dependencies::DependencyPurpose;
use crate::resolution::constants::pending_const_initializer_leaves;
pub use syntax_trees::item::ConstDestinationObligation;

impl ConstInitializerSelection {
    pub fn retained_constant(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<&crate::source_resolved::constant::ConstDeclaration, Vec<Diagnostic>> {
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
        purpose: crate::resolution::constants::initializer_normalization::InitializerCustodyPurpose,
    ) -> Result<syntax_trees::item::ConstInitializerCustody, Vec<Diagnostic>> {
        let base = self.frontier.selection.retained_base().ok_or_else(|| {
            vec![Diagnostic::error(
                "constant input has no retained predecessor",
            )]
        })?;
        let declaration = self.retained_constant(symbol)?;
        crate::resolution::constants::initializer_normalization::retained_constant_custody(
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
                .preparation()
                .normalization
                .as_ref()
                .map(|receipt| receipt.declaration_custody.call_selections.as_slice()),
        )
    }

    fn declaration_dependencies(
        &self,
        definition: &syntax_trees::item::ConstDefinition,
        purpose: DependencyPurpose,
        selected_calls: Option<&[(source_text::SourceSpan, source_text::SourceSpan)]>,
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
        crate::resolution::constants::initializer_dependencies::collect_declaration(
            &self.frontier.trees,
            if declaration.authored_initializer.is_valid() {
                declaration.authored_initializer
            } else {
                declaration.initializer
            },
            declaration.declared_type.as_ref(),
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
                if crate::resolution::constants::requires_const_initializer_evaluation(
                    syntax, definition,
                ) =>
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
    ) -> Result<Vec<crate::resolution::constants::PendingConstInitializerLeaf>, Vec<Diagnostic>>
    {
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
    use crate::resolution::pre_resolution::prepare_const_initializer_selection;
    use crate::resolution::{ResolutionRequest, resolve};
    use std::sync::Arc;

    fn parse(text: &str, source: source_text::SourceId) -> SyntaxTrees {
        let tokens = crate::resolution::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap()
    }

    #[test]
    fn initializer_normalization_keeps_new_work_and_retained_call_dependencies() {
        let mut sources = crate::source_inputs::SourceMap::default();
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
        let text = "use base.omg; machine local(value: u64) -> u64 { |base|::bound(value) }\nconst Count: u64 = local(3u64);";
        let source = sources
            .add("initializers/current.omg".into(), text.into())
            .source_id;
        let syntax = parse(text, source);
        let selected = prepare_const_initializer_selection(ResolutionRequest {
            syntax: syntax.clone(),
            sources: Some(Arc::new(sources)),
            top_level_bindings: vec![
                crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                    source,
                    base_source,
                    "base",
                ),
            ],
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
        let mut sources = crate::source_inputs::SourceMap::default();
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
            crate::resolution::pre_resolution::InitializerCustodyPurpose::Authored,
            crate::resolution::pre_resolution::InitializerCustodyPurpose::Declaration,
            crate::resolution::pre_resolution::InitializerCustodyPurpose::Value,
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
    fn extension_instantiates_an_alias_from_a_saved_base() {
        use crate::source_resolved::data::DataMember;
        use crate::source_resolved::types::{FixedArrayLength, TypeReference};
        use source_text::{SourceSpan, Span};

        let mut sources = crate::source_inputs::SourceMap::default();
        let text = "public data Pair<T> = [T; 2];";
        let source = sources.add("base.omg".into(), text.into()).source_id;
        let base = resolve(ResolutionRequest {
            syntax: parse(text, source),
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();

        // Reopen each predecessor before appending a file. The last file contains no aliases.
        let extend = |base: &SymbolResolvedTrees,
                      path: &str,
                      text: &str,
                      imports: &[(&str, &str, &str)]| {
            let bytes = record_storage::encode(base, Default::default()).unwrap();
            let base: SymbolResolvedTrees =
                record_storage::decode(&bytes, Default::default()).unwrap();
            let mut sources = Arc::unwrap_or_clone(base.symbols.source_map().unwrap());
            let source = sources.add(path.into(), text.into()).source_id;
            let result =
                crate::resolution::resolve_extension(crate::resolution::ExtensionRequest {
                    base: Arc::new(base),
                    syntax: parse(text, source),
                    top_level_bindings: crate::resolution::fixture_file_bindings(&sources, imports),
                    sources: Arc::new(sources),
                });
            (result, source)
        };

        let (middle, _) = extend(&base, "middle.omg", "public data Byte = u8;", &[]);
        let middle = middle.unwrap();
        let (extended, _) = extend(
            middle.trees(),
            "use.omg",
            "use base.omg; use middle.omg; public data Holder { value: |base|::Pair<|middle|::Byte>; }",
            &[
                ("use.omg", "base.omg", "base"),
                ("use.omg", "middle.omg", "middle"),
            ],
        );
        let extended = extended.unwrap();
        let trees = extended.trees();
        let holder = trees
            .data_definitions
            .iter()
            .find(|data| data.name.as_str() == "Holder")
            .unwrap();
        let [DataMember::Field(field)] = trees.data_members(holder.members) else {
            panic!("expected Holder.value")
        };
        fn expanded(mut ty: &TypeReference) -> &TypeReference {
            while let TypeReference::AliasUse { expansion, .. } = ty {
                ty = expansion;
            }
            ty
        }
        let TypeReference::FixedArray(array) = expanded(&field.type_reference) else {
            panic!("expected Pair<Byte> to expand to an array")
        };
        assert_eq!(array.length, FixedArrayLength::Literal(2));
        let TypeReference::Named { symbol, .. } =
            expanded(trees.child_type_reference(array.element_type))
        else {
            panic!("expected a named array element")
        };
        assert_eq!(trees.symbols.name(*symbol), "u8");

        let invalid = "use base.omg; public data Holder { value: |base|::Pair; }";
        let (refused, source) = extend(
            middle.trees(),
            "missing-argument.omg",
            invalid,
            &[("missing-argument.omg", "base.omg", "base")],
        );
        let reference = "|base|::Pair";
        let start = invalid.find(reference).unwrap();
        let span = SourceSpan::new(source, Span::new(start, start + reference.len()));
        let Err(diagnostics) = refused else {
            panic!("Pair requires a type argument");
        };
        assert!(
            diagnostics
                .iter()
                .any(|error| error.source_span == Some(span)
                    && error.message.contains(
                        "type alias application does not supply its complete declared telescope"
                    )),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn implicit_call_identity_uses_the_selected_entry_regardless_of_child_order() {
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::{SymbolKind, SymbolNameRef};

        let declaration =
            source_text::SourceSpan::new(source_text::SourceId(0), source_text::Span::new(0, 6));
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
            let custody = crate::resolution::selections::call_identity::call_declaration_source;
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
        let mut sources = crate::source_inputs::SourceMap::default();
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
            crate::resolution::selections::call_identity::call_declaration_source(&program, entry)
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
        let extra = crate::source_symbols::SymbolTableBuilder::child_handles(extra)
            .next()
            .unwrap();
        assert!(
            crate::resolution::selections::call_identity::call_declaration_source(&program, extra)
                .unwrap_err()
                .contains("not its exact owning entry")
        );
    }
}
