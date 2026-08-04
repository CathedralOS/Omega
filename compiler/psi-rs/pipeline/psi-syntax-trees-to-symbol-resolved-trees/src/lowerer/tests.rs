use super::{lower_syntax_trees, lower_syntax_trees_with_sources};
use psi_source::SourceMap;
use psi_source_files_to_tokens::Lexer;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees_with_id;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn proposition_declarations_resolve_as_a_distinct_proof_category() {
    let source = r#"
        proposition related(left: i32, right: i32);
        proposition witnessed<machine Generator>(value: i32) { i32; }
        proposition reflexive(value: i32) = related(value, value);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");

    assert_eq!(program.propositions.len(), 3);
    assert_eq!(program.machines.len(), 0);
    assert!(
        program
            .propositions
            .iter()
            .all(|item| item.symbol.is_valid())
    );
    assert!(
        program
            .propositions
            .iter()
            .all(|item| program.symbols.get(item.symbol).kind
                == psi_symbols::SymbolKind::Proposition)
    );

    let witnessed = &program.propositions[1];
    let [generator] = program
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(witnessed.binders)
    else {
        panic!("witnessed proposition should retain one binder");
    };
    assert!(matches!(
        generator.kind,
        psi_symbol_resolved_trees::proposition::PropositionBinderKind::Machine
    ));
    assert_eq!(
        program.symbols.get(generator.symbol).kind,
        psi_symbols::SymbolKind::PropositionMachineParameter
    );
    let psi_symbol_resolved_trees::proposition::PropositionBody::Witness { evidence } =
        &witnessed.body
    else {
        panic!("witness evidence should remain distinct from a body");
    };
    assert!(matches!(
        evidence,
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if symbol.is_valid() && name.as_str() == "i32"
    ));

    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        program.propositions[2].body
    else {
        panic!("transparent proposition should retain its source expansion");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("transparent expansion should remain a proposition call");
    };
    assert_eq!(call.target_symbol, program.propositions[0].symbol);
    for argument in program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    {
        let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            program.tables.bodies.expressions.expression(*argument)
        else {
            panic!("alias arguments should remain parameter names");
        };
        assert!(path.symbol.is_valid());
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            psi_symbols::SymbolKind::Parameter
        );
    }
}

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
        machine inspect() reaches Readable;
    }

    trait Policy {
    }

    machine backup() reaches Filesystem + Policy {
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
    domain Player::Valid
    requires
        self.health >= 0

    domain Player::Alive
    requires
        self in Player::Valid;
        self.health > 0

    domain Player::Tagged;

    domain Player::Usable =
        Player::Valid & Player::Alive;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.domain_definitions.len(), 4);
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = program.proof_facts(domain.facts);
    assert_eq!(facts.len(), 2);
    let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.body_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Tagged")
        .expect("tagged domain should lower");
    assert_eq!(
        tagged.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(tagged.body_token_count, 0);
    assert!(tagged.semantic_roles.is_empty());
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "Player::Alive")
            .is_some()
    );
    let usable = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Usable")
        .expect("usable alias should lower");
    let alias = usable.alias.as_ref().expect("alias theory");
    assert_eq!(alias.constituents.len(), 2);
    assert!(
        alias
            .constituents
            .iter()
            .all(|constituent| constituent.domain_symbol.is_valid())
    );
    assert!(usable.facts.is_empty(), "aliases are not predicate facts");
}

#[test]
fn resolves_free_machine_calls_in_domain_predicates() {
    let source = r#"
    boundary machine no_wrap(base: addr, length: u64) -> bool;

    data Region {
        base: addr;
        length: u64;
    }

    domain Region::Valid
    requires
        no_wrap(self.base, self.length);
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Valid")
        .expect("valid domain");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(predicate)] =
        program.proof_facts(domain.facts)
    else {
        panic!("one predicate call");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*predicate)
    else {
        panic!("predicate should remain a call");
    };
    assert!(call.target_symbol.is_valid());
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "no_wrap")
        .expect("predicate machine");
    assert!(
        program
            .machine_state_handles(machine.states)
            .iter()
            .any(|state| program.machine_state(*state).symbol == call.target_symbol)
    );
}

#[test]
fn resolves_repeated_capacity_specializations_as_one_domain_identity() {
    let source = r#"
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    domain [u8; 16]::Utf8
    requires
        valid_utf8(self);

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
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("fill should retain its ensures contract");
    let [psi_symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
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

    domain Quantity::Additive
    requires
        self.value >= 0;

    operator Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;
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
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert!(domain.semantic_roles.arithmetic_policy.is_none());
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
fn infers_top_level_operator_home_from_qualified_operands() {
    let source = r#"
    domain i32::Degrees;

    operator add(left: i32 in Degrees, right: i32 in Degrees) -> i32 in Degrees spelling +;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "i32::Degrees")
        .expect("domain should lower");
    let operator = program
        .operator_definitions(domain.operators)
        .first()
        .expect("qualified operands should supply one semantic home");

    assert!(program.operators.is_empty());
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert_eq!(
        program
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
}

#[test]
fn rejects_ambiguous_inferred_domain_operator_home() {
    let source = r#"
    domain i32::Degrees;
    domain i32::Radians;

    operator add(left: i32 in Degrees, right: i32 in Radians) -> i32 spelling +;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("competing operand domains must not infer an operator home");
    assert!(
        diagnostic
            .message
            .contains("has more than one possible domain home")
    );
}

#[test]
fn does_not_infer_domain_establishment_from_contract_placement() {
    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued;

    machine Token::issue(value: u64) -> Token
    ensures
        result in Token::Issued
    {
        Token { value: value }
    }

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Issued;
    }

    domain Token::Stamped;

    operator Token::Stamped::stamp(value: Token) -> Token
    ensures
        result in Token::Stamped;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    let issued = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    assert!(issued.establishment_routes.is_empty());

    let stamped = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Stamped")
        .expect("stamped domain");
    assert!(program.operator_definitions(stamped.operators).len() == 1);
    assert!(stamped.establishment_routes.is_empty());
}

#[test]
fn normalizes_authored_checked_and_boundary_requirement_routes() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Checked {
        CheckedIssuer::issue;
    }
    domain Token::Admitted {
        BoundaryIssuer::issue;
    }

    trait CheckedIssuer {
        machine issue(value: u64) -> Token in Checked;
    }
    boundary trait BoundaryIssuer {
        machine issue(value: u64) -> Token
        ensures result in Token::Admitted;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    for (domain_name, trait_name, is_boundary) in [
        ("Token::Checked", "CheckedIssuer", false),
        ("Token::Admitted", "BoundaryIssuer", true),
    ] {
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == domain_name)
            .expect("domain");
        let definition = program
            .traits
            .iter()
            .find(|definition| definition.name.as_str() == trait_name)
            .expect("trait");
        let requirement = program
            .trait_machine_signatures(definition.machines)
            .first()
            .expect("requirement");
        let expected = if is_boundary {
            DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: definition.symbol,
                requirement: requirement.symbol,
            }
        } else {
            DomainEstablishmentRoute::CheckedRequirement {
                trait_definition: definition.symbol,
                requirement: requirement.symbol,
            }
        };
        assert!(domain.establishment_routes.contains(&expected));
    }
}

#[test]
fn boundary_requirement_route_accepts_exact_non_self_parameter_domain() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }
    domain Token::Pending {
        BoundaryIngress::enter;
    }
    boundary trait BoundaryIngress {
        machine enter(token: Token in Pending);
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
        .find(|domain| domain.name.as_str() == "Token::Pending")
        .expect("pending domain");
    let ingress = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "BoundaryIngress")
        .expect("boundary ingress trait");
    let enter = program
        .trait_machine_signatures(ingress.machines)
        .first()
        .expect("entry requirement");
    assert!(
        domain
            .establishment_routes
            .contains(&DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: ingress.symbol,
                requirement: enter.symbol,
            })
    );
}

#[test]
fn ordinary_requirement_route_rejects_parameter_domain_as_introduction() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Pending {
        OrdinaryIngress::enter;
    }
    trait OrdinaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("an ordinary call must treat its parameter domain as a precondition");
    assert!(diagnostic.message.contains(
        "does not name the domain on its exact result or an exact non-self external-root parameter"
    ));
}

#[test]
fn rejects_unresolved_authored_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued {
        MissingIssuer::issue;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees).expect_err("route must resolve exactly");
    assert!(
        diagnostic
            .message
            .contains("does not resolve to one exact trait")
    );
}

#[test]
fn expands_alias_establishment_routes_to_atomic_domains() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued {
        TokenIssuer::issue;
    }
    domain Token::Stamped {
        TokenIssuer::issue;
    }
    domain Token::Ready = Token::Issued & Token::Stamped;

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Ready;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let issuer = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "TokenIssuer")
        .expect("boundary trait");
    let requirement = program
        .trait_machine_signatures(issuer.machines)
        .first()
        .expect("issue requirement");
    let route = DomainEstablishmentRoute::BoundaryRequirement {
        boundary_trait: issuer.symbol,
        requirement: requirement.symbol,
    };

    for name in ["Token::Issued", "Token::Stamped"] {
        let atom = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .expect("atomic domain");
        assert_eq!(atom.establishment_routes, [route]);
    }
    let alias = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Ready")
        .expect("alias domain");
    assert!(
        alias.establishment_routes.is_empty(),
        "routes belong to normalized atomic facts, not alias spellings"
    );
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
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("witness ensures");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(ensures.facts)
    else {
        panic!("one expression fact")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("equality expression")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
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
fn resolves_qualified_attached_machine_tail_transition() {
    let source = r#"
    data Main {}

    machine Main::pack(left: i32, right: i32) -> i32 {
        left + right
    }

    machine Main::issue() -> i32 {
        transition { _ -> Main::pack(1, 2) }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let pack = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pack")
        .expect("pack machine");
    let pack_state = program
        .machine_state_handles(pack.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("pack state");
    let issue = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::issue")
        .expect("issue machine");
    let issue_state = program
        .machine_state_handles(issue.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("issue state");
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(issue_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("issue should end in a transition");
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("qualified tail call should remain a named transition");
    };

    assert_eq!(target.symbol, pack_state.symbol);
    assert!(target.head_symbol.is_valid());
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

    let psi_symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &parameter.type_reference
    else {
        panic!("self parameter should retain its authored reference shell");
    };
    let psi_symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
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
