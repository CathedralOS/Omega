use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::operator::lower_operator_definition;
use crate::platform::lower_platform;
use crate::trait_definition::lower_trait_definition;
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

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lower_domain_definition(&mut lowerer, domain_definition)?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.typed_trees.push_machine(machine);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lower_operator_definition(operator);
        lowerer.typed_trees.push_operator(operator);
    }

    for platform in &symbol_resolved_trees.platforms {
        let platform = lower_platform(&mut lowerer, platform)?;
        lowerer.typed_trees.push_platform(platform);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        let trait_definition = lower_trait_definition(&mut lowerer, trait_definition)?;
        lowerer.typed_trees.push_trait_definition(trait_definition);
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
    fn lowers_slice_range_surface_into_typed_trees() {
        let source = r#"
        data Main {}

        machine Main::main(&mut self) -> usize {
            let values: [usize; 4] = [1, 2, 3, 4];
            let view: &[usize] = values.as_slice();
            let tail: &[usize] = view[1..];
            tail.len
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program =
            lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
        let typed_trees =
            lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");

        assert!(
            typed_trees
                .machines()
                .first()
                .is_some_and(|machine| !typed_trees.machine_states(machine).is_empty())
        );
    }

    #[test]
    fn lowers_domain_definitions() {
        let source = r#"
        domain Player::Valid {
            self.health >= 0
        }

        domain Player::Alive {
            self in Player::Valid;
            self.health > 0
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

        assert_eq!(typed_trees.domain_definitions().len(), 2);
        let domain = typed_trees
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str() == "Player::Alive")
            .expect("alive domain should lower");
        assert!(domain.symbol.is_valid());
        assert_eq!(domain.name.as_str(), "Player::Alive");
        let facts = typed_trees.proof_facts(domain);
        assert_eq!(facts.len(), 2);
        let omega_typed_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
            panic!("first domain fact should be membership")
        };
        assert!(membership.domain_symbol.is_valid());
        assert!(domain.body_token_count >= 3);
        assert!(domain.target_type.is_valid());
    }

    #[test]
    fn preserves_operator_declarations() {
        let source = r#"
        operator Slice::index<T>(items: &[T], index: usize) -> T
        requires
            index < items.len
        intrinsic;
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program =
            lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
        let typed_trees =
            lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

        assert_eq!(typed_trees.operators().len(), 1);
        assert!(typed_trees.operators()[0].token_count > 0);
    }

    #[test]
    fn lowers_machine_contract_clauses() {
        let source = r#"
        machine distinct_indices(i: usize, j: usize)
        requires
            i < j
        ensures
            i != j
        {
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
        let machine = typed_trees.machines().first().expect("machine");
        let contracts = typed_trees.machine_contracts(machine);

        assert_eq!(contracts.len(), 2);
        assert!(contracts[0].token_count >= 3);
        assert!(contracts[1].token_count >= 3);
        assert_eq!(
            typed_trees
                .proof_facts
                .span_or_empty(contracts[0].facts)
                .len(),
            1
        );
        assert_eq!(
            typed_trees
                .proof_facts
                .span_or_empty(contracts[1].facts)
                .len(),
            1
        );
    }

    #[test]
    fn lowers_statement_argument_spans_from_statement_table() {
        let source = r#"
        data Parser {}

        machine Parser::start(&mut self, level: i32, cell: i32, line: i32) -> i32 {
            -> self.resolve_exit(level, cell, line);

            state resolve_exit(&mut self, level: i32, cell: i32, line: i32) -> i32 {
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
