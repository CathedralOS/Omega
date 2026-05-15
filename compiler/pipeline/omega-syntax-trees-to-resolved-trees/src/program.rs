use crate::item::lower_item;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub fn lower_syntax_trees(syntax_trees: &SyntaxTrees) -> Result<SymbolResolvedTrees, Diagnostic> {
    let mut lowerer = Lowerer::default();

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, syntax_trees, item)?;
    }

    lowerer.finish()
}

pub fn lower_program(items: &[syntax::item::Item]) -> Result<SymbolResolvedTrees, Diagnostic> {
    let syntax_trees = SyntaxTrees::from_root_items(Default::default(), items.iter().cloned());
    lower_syntax_trees(&syntax_trees)
}

#[derive(Default)]
pub(crate) struct Lowerer {
    pub(crate) program: SymbolResolvedTrees,
}

impl Lowerer {
    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Diagnostic> {
        crate::symbols::assign_symbols(&mut self.program);
        self.program.rebuild_tables();
        Ok(self.program)
    }
}

#[cfg(test)]
mod tests {
    use super::lower_syntax_trees;
    use omega_source_files_to_tokens::Lexer;
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
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.data_definitions.len(), 1);
        assert_eq!(program.machines.len(), 1);
        assert_eq!(program.machines[0].states.len(), 1);
        assert!(
            program
                .symbols
                .find_child_by_name(program.symbols.root(), "u32")
                .is_some()
        );
    }

    #[test]
    fn merges_machine_fragments_by_machine_name() {
        let source = r#"
        machine Game::new {
            pub entry() {}
        }

        machine Game::running {
            pub entry() {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.machines.len(), 1);
        assert_eq!(program.machines[0].name.as_str(), "Game");
        assert_eq!(program.machines[0].states.len(), 2);
    }

    #[test]
    fn lowers_main_entry_state_name_as_entry() {
        let source = r#"
        machine main {
            pub entry() {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.machines.len(), 1);
        assert_eq!(program.machines[0].name.as_str(), "main");
        assert_eq!(program.machines[0].states.len(), 1);
        assert_eq!(program.machines[0].states[0].name.as_str(), "entry");
    }

    #[test]
    fn resolves_self_parameter_type_to_machine_symbol() {
        let source = r#"
        machine main {
            pub entry(&mut self) {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
        let machine = program.machines.first().expect("machine");
        let parameter = machine
            .states
            .first()
            .expect("entry state")
            .parameters
            .first()
            .expect("self parameter");

        let omega_resolved_trees::types::TypeReference::SelfType { symbol } =
            &parameter.type_reference
        else {
            panic!("self parameter type should stay explicit");
        };

        assert_eq!(*symbol, machine.symbol);
    }
}
