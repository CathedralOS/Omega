use crate::data::lower_data_definition;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::platform::lower_platform;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees::TypedTrees;

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
    };

    for invariant_definition in &symbol_resolved_trees.invariant_definitions {
        let invariant_definition = lower_invariant_definition(&mut lowerer, invariant_definition)?;
        lowerer
            .typed_trees
            .push_invariant_definition(invariant_definition);
    }

    for data_definition in &symbol_resolved_trees.data_definitions {
        let data_definition = lower_data_definition(&mut lowerer, data_definition)?;
        lowerer.typed_trees.push_data_definition(data_definition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.typed_trees.push_machine(machine);
    }

    for platform in &symbol_resolved_trees.platforms {
        let platform = lower_platform(&mut lowerer, platform)?;
        lowerer.typed_trees.push_platform(platform);
    }

    lowerer.finish()
}

pub fn lower_symbol_resolved_trees_owned(
    symbol_resolved_trees: SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut typed_trees = lower_symbol_resolved_trees(&symbol_resolved_trees)?;
    typed_trees.symbols = symbol_resolved_trees.symbols;
    Ok(typed_trees)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
}

impl Lowerer<'_> {
    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        Ok(self.typed_trees)
    }
}

#[cfg(test)]
mod tests {
    use super::lower_symbol_resolved_trees;
    use omega_source_files_to_tokens::Lexer;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn lowers_dungeon_style_machine_program() {
        let source = r#"
        data Inventory {
            gold: u32[exact];
        }

        machine Inventory::clear {
            pub entry(&mut self, inventory: &mut Inventory) {
                inventory.gold = 0;
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program =
            lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
        let typed_trees =
            lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

        assert_eq!(typed_trees.data_definitions().len(), 1);
        assert_eq!(typed_trees.machines().len(), 1);
        assert_eq!(
            typed_trees.machine_states(&typed_trees.machines()[0]).len(),
            1
        );
        assert!(
            typed_trees
                .symbols
                .find_child_by_name(typed_trees.symbols.root(), "u32")
                .is_some()
        );
    }

    #[test]
    fn lowers_statement_argument_spans_from_statement_table() {
        let source = r#"
        data Parser {}

        machine Parser::start -> i32 {
            pub entry(&mut self, level: i32, cell: i32, line: i32) {
                -> self.resolve_exit(level, cell, line);
            }

            entry resolve_exit(&mut self, level: i32, cell: i32, line: i32) -> i32 {
                0
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program =
            lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
        let typed_trees =
            lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
        let machine = &typed_trees.machines()[0];
        let entry = &typed_trees.machine_states(machine)[0];
        let statements = typed_trees
            .statement_table
            .statements(entry.statement_nodes);

        let omega_typed_trees::statement::StatementNode::Transition(transition) = &statements[0]
        else {
            panic!("entry should lower to transition statement");
        };
        let omega_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
            typed_trees
                .statement_table
                .transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };
        let arguments = typed_trees.statement_table.expression_handles(*arguments);
        let argument_names = arguments
            .iter()
            .map(|argument| typed_trees.expression_table.display_name(*argument))
            .collect::<Vec<_>>();

        assert_eq!(argument_names, ["level", "cell", "line"]);
    }
}
