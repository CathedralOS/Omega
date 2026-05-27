use crate::item::lower_item;
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceMap;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::SyntaxTrees;
use std::sync::Arc;

pub fn lower_syntax_trees(syntax_trees: &SyntaxTrees) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, None)
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources))
}

fn lower_syntax_trees_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> Result<SymbolResolvedTrees, Diagnostic> {
    let mut lowerer = Lowerer::new(sources);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, syntax_trees, item)?;
    }

    lowerer.finish()
}

pub(crate) struct Lowerer {
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
}

impl Lowerer {
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            sources,
        }
    }

    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Diagnostic> {
        crate::symbols::assign_symbols(&mut self.symbol_resolved_trees, self.sources);
        self.symbol_resolved_trees.rebuild_tables();
        Ok(self.symbol_resolved_trees)
    }
}

#[cfg(test)]
mod tests {
    use super::{lower_syntax_trees, lower_syntax_trees_with_sources};
    use omega_core::source::SourceMap;
    use omega_core::symbols::BuiltinTypeMember;
    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees_with_id;
    use std::path::PathBuf;
    use std::sync::Arc;

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
        assert!(program.machines[0].symbol.is_valid());
        assert_eq!(
            program
                .machine_state_handles(program.machines[0].states)
                .len(),
            1
        );
        let state = program.machine_state_handles(program.machines[0].states)[0];
        assert!(program.machine_state(state).symbol.is_valid());
        assert!(
            program
                .symbols
                .find_child_by_name(program.symbols.root(), "u32")
                .is_some()
        );
    }

    #[test]
    fn keeps_attached_machines_as_distinct_callables() {
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

        assert_eq!(program.machines.len(), 2);
        assert_eq!(program.machines[0].name.as_str(), "Game::new");
        assert_eq!(
            program.machines[0]
                .attached_data
                .as_ref()
                .map(|name| name.as_str()),
            Some("Game")
        );
        assert_eq!(program.machines[1].name.as_str(), "Game::running");
        assert_eq!(
            program
                .machine_state_handles(program.machines[0].states)
                .len(),
            1
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
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.domain_definitions.len(), 2);
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == "Player::Alive")
            .expect("alive domain should lower");
        assert!(domain.symbol.is_valid());
        assert_eq!(domain.name.as_str(), "Player::Alive");
        let facts = program.proof_facts(domain.facts);
        assert_eq!(facts.len(), 2);
        let omega_symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0]
        else {
            panic!("first domain fact should be membership")
        };
        assert!(membership.domain_symbol.is_valid());
        assert!(domain.body_token_count >= 3);
        assert!(
            program
                .symbols
                .find_child_by_name(program.symbols.root(), "Player::Alive")
                .is_some()
        );
    }

    #[test]
    fn preserves_operator_declarations() {
        let source = r#"
        operator Slice::index<T>(items: &[T], index: usize) -> T
        requires
            index < items.len;
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.operators.len(), 1);
        let operator = &program.operators[0];
        assert_eq!(
            program
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>(),
            ["Slice", "index"]
        );
        assert_eq!(
            program.data_type_parameters(operator.type_parameters).len(),
            1
        );
        assert_eq!(program.state_parameters(operator.parameters).len(), 2);
        assert!(operator.symbol.is_valid());
        assert!(operator.return_type.is_some());
        assert_eq!(program.signature_contracts(operator.contracts).len(), 1);
        assert!(operator.token_count > 0);
    }

    #[test]
    fn preserves_domain_operator_declarations() {
        let source = r#"
        data Quantity {
            value: i32;
        }

        domain Quantity::Additive {
            self.value >= 0;

            operator add(left: Quantity, right: Quantity) -> Quantity;
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == "Quantity::Additive")
            .expect("domain should lower");
        let operators = program.operator_definitions(domain.operators);

        assert_eq!(operators.len(), 1);
        assert!(operators[0].symbol.is_valid());
        assert_eq!(
            program
                .operator_path_members(operators[0].name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>(),
            ["add"]
        );
        assert_eq!(program.proof_facts(domain.facts).len(), 1);
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
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
        let machine = program.machines.first().expect("machine");
        let contracts = program.machine_contracts(machine);

        assert_eq!(contracts.len(), 2);
        assert!(contracts[0].token_count >= 3);
        assert!(contracts[1].token_count >= 3);
        assert_eq!(program.proof_facts(contracts[0].facts).len(), 1);
        assert_eq!(program.proof_facts(contracts[1].facts).len(), 1);
    }

    #[test]
    fn lowers_attached_main_state_name_as_main() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

        assert_eq!(program.machines.len(), 1);
        assert_eq!(program.machines[0].name.as_str(), "Main::main");
        assert_eq!(
            program.machines[0]
                .attached_data
                .as_ref()
                .map(|name| name.as_str()),
            Some("Main")
        );
        let state = program
            .machine_state_handles(program.machines[0].states)
            .first()
            .map(|state| program.machine_state(*state))
            .expect("entry state");
        assert_eq!(state.name.as_str(), "main");
    }

    #[test]
    fn resolves_self_parameter_type_to_machine_symbol() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
        let machine = program.machines.first().expect("machine");
        let entry = program
            .machine_state_handles(machine.states)
            .first()
            .map(|state| program.machine_state(*state))
            .expect("entry state");
        let parameter = program
            .state_parameters(entry.parameters)
            .first()
            .expect("self parameter");

        let omega_symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
            &parameter.type_reference
        else {
            panic!("self parameter type should stay explicit");
        };

        assert_eq!(*symbol, machine.symbol);
    }

    #[test]
    fn resolves_builtin_type_member_call_symbols() {
        let source = r#"
        data Main {
            value: Real;
        }

        machine Main::main(&mut self) {
            self.value = Real::from(1);
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
        let machine = program.machines.first().expect("machine");
        let entry = program
            .machine_state_handles(machine.states)
            .first()
            .map(|state| program.machine_state(*state))
            .expect("entry state");
        let statement = program
            .state_statements(entry.statements)
            .first()
            .expect("assignment statement");
        let omega_symbol_resolved_trees::statement::Statement::Assignment(assignment) = statement
        else {
            panic!("expected assignment statement");
        };
        let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(assignment.value)
        else {
            panic!("expected Real::from call expression");
        };
        let real_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "Real")
            .expect("Real builtin symbol");
        let real_from_symbol = program
            .symbols
            .find_child_by_name(real_symbol, BuiltinTypeMember::RealFrom.name())
            .expect("Real::from builtin symbol");

        assert_eq!(call.target_symbol, real_from_symbol);
    }

    #[test]
    fn source_backed_names_are_used_when_sources_are_available() {
        let source = r#"
        data Inventory {
            gold: u32;
        }
        "#;
        let mut sources = SourceMap::default();
        let source_id = sources
            .add(PathBuf::from("main.omg"), source.to_owned())
            .source_id;
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees =
            parse_syntax_trees_with_id(source_id, &tokens).expect("parse should succeed");
        let program = lower_syntax_trees_with_sources(&syntax_trees, Arc::new(sources))
            .expect("lowering should succeed");
        let counts = program.symbols.name_storage_counts();

        assert!(
            counts.source_names > 0,
            "source identifiers should be stored by source span"
        );
        assert!(
            counts.owned_names == 0,
            "loaded source-backed identifiers should not allocate owned symbol names"
        );
        assert!(
            counts.static_names > 0,
            "builtins and synthetic roots should stay static"
        );
    }
}
