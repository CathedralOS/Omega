use super::{lower_syntax_trees, lower_syntax_trees_with_sources};
use omega_core::source::SourceMap;
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
fn normalizes_service_rows_from_resolved_boundary_trait_symbols() {
    let source = r#"
    boundary trait Readable {
    }

    boundary trait Filesystem: Readable {
        machine inspect() effects Readable;
    }

    trait Policy {
    }

    machine backup() effects Filesystem + Policy {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    let readable = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Readable")
        .expect("Readable boundary trait");
    let filesystem = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Filesystem")
        .expect("Filesystem boundary trait");
    let readable_id = program
        .service_reaches
        .id_for_symbol(readable.symbol)
        .expect("Readable service id");
    let filesystem_id = program
        .service_reaches
        .id_for_symbol(filesystem.symbol)
        .expect("Filesystem service id");
    assert!(
        program
            .service_reaches
            .id_for_symbol(
                program
                    .traits
                    .iter()
                    .find(|definition| definition.name.as_str() == "Policy")
                    .expect("ordinary policy trait")
                    .symbol,
            )
            .is_none(),
        "ordinary traits must not mint service identities",
    );

    let backup = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "backup")
        .expect("backup machine");
    let mut backup_services = vec![readable_id, filesystem_id];
    backup_services.sort_by_key(|service| service.0);
    assert_eq!(
        program
            .service_reach_rows
            .services(backup.service_reach_row),
        backup_services,
        "authored service rows include normalized boundary-parent closure",
    );

    let inspect = program
        .trait_machine_signatures(filesystem.machines)
        .first()
        .expect("Filesystem::inspect signature");
    assert_eq!(
        program
            .service_reach_rows
            .services(inspect.service_reach_row),
        &[readable_id],
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

    domain Player::Tagged {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.domain_definitions.len(), 3);
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = program.proof_facts(domain.facts);
    assert_eq!(facts.len(), 2);
    let omega_symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.body_token_count >= 3);
    assert!(domain.facets.predicate, "factful domains are hybrids");
    assert_eq!(domain.facets.semantic, Some(domain.semantic_id));
    let tagged = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Tagged")
        .expect("tagged domain should lower");
    assert!(
        !tagged.facets.predicate,
        "factless domains are semantic-only"
    );
    assert_eq!(tagged.facets.semantic, Some(tagged.semantic_id));
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "Player::Alive")
            .is_some()
    );
}

#[test]
fn resolves_repeated_capacity_specializations_as_one_domain_identity() {
    let source = r#"
    domain [u8; 8]::Utf8 {
        valid_utf8(self);
    }

    domain [u8; 16]::Utf8 {
        valid_utf8(self);
    }

    data Holder {
        label: [u8; 8] in Utf8;
    }

    machine fill(out: &mut Holder)
    ensures
        out.label in Utf8
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    assert_eq!(
        program.domain_definitions[0].semantic_id, program.domain_definitions[1].semantic_id,
        "capacity-specialized declarations with the same normalized predicate should share semantic identity",
    );

    let machine = program.machines.first().expect("fill machine");
    let contract = program
        .machine_contracts(machine)
        .iter()
        .find(|contract| {
            contract.kind == omega_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("fill should retain its ensures contract");
    let [omega_symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
        program.proof_facts(contract.facts)
    else {
        panic!("ensures should contain one domain membership")
    };
    assert!(membership.domain_symbol.is_valid());
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
fn resolves_generic_calls_inside_machine_contracts() {
    let source = r#"
    data Index {
        case Zero;
        case Next(previous: Index);
    }

    machine generic<machine S>(value: Index) -> Index
    where machine S(index: Index) -> Index;
    {
        value
    }

    machine witness<machine Selected>(value: Index)
    where machine Selected(index: Index) -> Index;
    ensures generic<Selected>(value) == value
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let witness = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "witness")
        .expect("witness machine");
    let ensures = program
        .machine_contracts(witness)
        .iter()
        .find(|contract| {
            contract.kind == omega_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("witness ensures");
    let [omega_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(ensures.facts)
    else {
        panic!("one expression fact")
    };
    let omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("equality expression")
    };
    let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(binary.left)
    else {
        panic!("generic call on equality left")
    };
    assert!(call.target_symbol.is_valid());
    assert_eq!(call.machine_arguments.len(), 1);
    assert!(call.machine_arguments[0].symbol.is_valid());
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

    let omega_symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &parameter.type_reference
    else {
        panic!("self parameter should retain its authored reference shell");
    };
    let omega_symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
        program.child_type_reference(reference.referee)
    else {
        panic!("self parameter referee should stay explicit");
    };

    assert_eq!(*symbol, machine.symbol);
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
