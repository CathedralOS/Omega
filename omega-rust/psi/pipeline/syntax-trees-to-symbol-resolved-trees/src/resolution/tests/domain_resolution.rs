use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

mod semantic_identity_tests {
    use crate::resolution::{ResolutionRequest, resolve};
    use language_semantics::SemanticDomainTable;
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum};
    use source_files_to_tokens::Lexer;
    use std::path::PathBuf;
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    fn add_domain_source(
        sources: &mut SourceMap,
        root: &str,
        filename: &str,
        text: &str,
        package_marker: u8,
        scope: DependencyScope,
    ) {
        sources.add_checked_instance(
            PathBuf::from(root).join(filename),
            text.to_owned(),
            PathBuf::from(root),
            PackageKeyIdentity::from_digest([package_marker; 32]),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            scope,
        );
    }

    fn resolve_domain_sources(sources: SourceMap) -> SymbolResolvedTrees {
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for file in sources.files() {
            let tokens = Lexer::new(&file.source)
                .tokenize()
                .expect("tokenize domain source");
            let parsed =
                parse_syntax_trees_with_id(file.source_id, &tokens).expect("parse domain source");
            syntax.extend_from(&parsed);
        }
        resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        })
        .expect("resolve owned domain declarations")
    }

    fn canonical_names(program: &SymbolResolvedTrees) -> Vec<String> {
        program
            .domain_definitions
            .iter()
            .map(|domain| {
                program
                    .semantic_domains
                    .name(domain.semantic_id)
                    .expect("retained canonical domain identity")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn managed_packages_keep_equal_module_and_domain_spellings_distinct() {
        let mut sources = SourceMap::default();
        for (root, marker) in [("first", 1), ("second", 2)] {
            add_domain_source(
                &mut sources,
                root,
                "codec.omg",
                "module codec; domain [u8; 8]::Utf8;",
                marker,
                DependencyScope::Product,
            );
        }
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 2);
        assert_eq!(
            program.domain_definitions[0].name,
            program.domain_definitions[1].name
        );
        assert_ne!(
            program.domain_definitions[0].semantic_id,
            program.domain_definitions[1].semantic_id
        );
        let names = canonical_names(&program);
        assert_ne!(
            names[0], names[1],
            "indexed and Terminal readers consume these names"
        );
    }

    #[test]
    fn managed_build_and_product_domains_keep_distinct_checked_identities() {
        let mut sources = SourceMap::default();
        for scope in [DependencyScope::Build, DependencyScope::Product] {
            add_domain_source(
                &mut sources,
                "shared",
                "codec.omg",
                "module codec; domain [u8; 8]::Utf8;",
                1,
                scope,
            );
        }
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 2);
        assert_ne!(
            program.domain_definitions[0].semantic_id,
            program.domain_definitions[1].semantic_id
        );
        let names = canonical_names(&program);
        assert_ne!(names[0], names[1]);
    }

    #[test]
    fn managed_domain_names_survive_relocation_and_source_order_changes() {
        let resolve_at = |root: &str, reverse: bool| {
            let mut sources = SourceMap::default();
            let mut modules = [("codec", 1), ("policy", 2)];
            if reverse {
                modules.reverse();
            }
            for (module, marker) in modules {
                add_domain_source(
                    &mut sources,
                    root,
                    &format!("{module}.omg"),
                    &format!("module {module}; domain [u8; 8]::Utf8;"),
                    marker,
                    DependencyScope::Product,
                );
            }
            let mut names = canonical_names(&resolve_domain_sources(sources));
            names.sort();
            names
        };
        assert_eq!(
            resolve_at("checkout/original", false),
            resolve_at("relocated/project", true)
        );
    }

    #[test]
    fn managed_sibling_sources_share_capacity_specialized_domain_identity() {
        let mut sources = SourceMap::default();
        for (filename, capacity) in [("small.omg", 8), ("large.omg", 16)] {
            add_domain_source(
                &mut sources,
                "codec",
                filename,
                &format!("module codec; domain [u8; {capacity}]::Utf8;"),
                1,
                DependencyScope::Product,
            );
        }
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 2);
        assert_ne!(
            program.domain_definitions[0].symbol,
            program.domain_definitions[1].symbol
        );
        assert_eq!(
            program.domain_definitions[0].semantic_id,
            program.domain_definitions[1].semantic_id
        );
    }

    #[test]
    fn managed_authored_policy_names_do_not_acquire_builtin_identity() {
        let mut sources = SourceMap::default();
        add_domain_source(
            &mut sources,
            "policies",
            "main.omg",
            "domain [u8; 8]::Wrapping; domain [u8; 8]::Saturating; domain [u8; 8]::Trapping;",
            1,
            DependencyScope::Product,
        );
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 3);
        for (name, builtin) in [
            ("Wrapping", SemanticDomainTable::WRAPPING),
            ("Saturating", SemanticDomainTable::SATURATING),
            ("Trapping", SemanticDomainTable::TRAPPING),
        ] {
            assert_eq!(program.semantic_domains.lookup(name), Some(builtin));
            let authored = program
                .domain_definitions
                .iter()
                .find(|domain| domain.name.as_str().rsplit("::").next() == Some(name))
                .expect("authored lookalike");
            assert_ne!(authored.semantic_id, builtin);
            assert!(authored.semantic_roles.is_empty());
        }
    }

    #[test]
    fn unmanaged_domains_do_not_invent_portable_package_ownership() {
        let mut sources = SourceMap::default();
        for root in ["first", "second"] {
            add_domain_source(
                &mut sources,
                root,
                "codec.omg",
                "module codec; domain [u8; 8]::Utf8;",
                0,
                DependencyScope::Product,
            );
        }
        let program = resolve_domain_sources(sources);
        assert_eq!(program.domain_definitions.len(), 2);
        assert_eq!(
            program.domain_definitions[0].semantic_id, program.domain_definitions[1].semantic_id,
            "unmanaged collisions remain subject to the independent validation guard"
        );
        assert!(!program.symbols.same_symbol_source_package(
            program.domain_definitions[0].symbol,
            program.domain_definitions[1].symbol
        ));
    }

    #[test]
    fn mapped_domain_rejects_missing_source_custody() {
        let tokens = Lexer::new("domain [u8; 8]::Utf8;")
            .tokenize()
            .expect("tokenize domain");
        let syntax =
            parse_syntax_trees_with_id(source::SourceId(0), &tokens).expect("parse domain");
        let diagnostics = resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(SourceMap::default())),
            top_level_bindings: Vec::new(),
        })
        .expect_err("a mapped declaration cannot fall back to source-free identity");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "domain declaration is missing its checked source owner"
        }));
    }
}

#[test]
fn rejects_authored_empty_service_reach_on_external_realization_before_resolved_trees() {
    let source = r#"
        boundary trait Process {
            machine exit(code: i32)
            reaches Process;
        }

        machine exit_leaf(code: i32)
        satisfies Process::exit
        via Binding::Syscall(60)
        reaches;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("an explicit empty external reach must not collapse into omission");

    assert!(
        diagnostic[0]
            .message
            .contains("repeats an authored `reaches` row")
    );
}

#[test]
fn retains_external_realization_mechanism_without_rendering_classification() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve external realization");
    let leaf = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");

    let language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism } =
        leaf.supply_mode
    else {
        panic!("bodyless via leaf must retain external supply");
    };
    let binding = binding.expect("bootstrap binding identity");
    assert!(binding.is_valid());
    assert_eq!(
        mechanism,
        Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic)
    );
    let [conformance] = program.machine_trait_conformances(leaf.satisfies) else {
        panic!("external leaf must retain one exact satisfaction row");
    };
    assert_eq!(conformance.external_binding, Some(binding));
}

#[test]
fn retains_ordinary_via_call_as_resolved_expression_without_fabricated_binding() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding() -> i32 {
            0
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve ordinary via call");
    let leaf = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");
    assert_eq!(
        leaf.supply_mode,
        language_semantics::MachineSupplyMode::ExternalRealization {
            binding: None,
            mechanism: None,
        }
    );
    let [conformance] = program.machine_trait_conformances(leaf.satisfies) else {
        panic!("external leaf must retain one exact satisfaction row");
    };
    assert!(conformance.external_binding.is_none());
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(conformance.via_expression)
    else {
        panic!("ordinary via source must retain its resolved call");
    };
    assert_eq!(call.target.as_str(), "binding");
    assert!(call.target_symbol.is_valid());
}

#[test]
fn keeps_attached_machines_as_distinct_callables() {
    let source = r#"
    pub machine Game::new() {}

    pub machine Game::running() {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

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
    let symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.semantic_clause_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Tagged")
        .expect("tagged domain should lower");
    assert_eq!(
        tagged.predicate_body,
        language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(tagged.semantic_clause_token_count, 0);
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
fn resolves_exact_case_symbols_in_domain_proof_expressions() {
    let source = r#"
    data Command {
        case Move(dx: i32);
        case Say(volume: i32);
    }

    domain Command::Interactive
    requires
        self in Command::Move | Command::Say;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let command = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Command")
        .expect("Command data");
    let expected_cases = program
        .data_members(command.members)
        .iter()
        .filter_map(|member| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Command::Interactive")
        .expect("interactive domain");
    let [symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(domain.facts)
    else {
        panic!("case union should remain one proof expression");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Binary(union) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("proof expression should remain a case union");
    };

    for (expression, expected_case) in [union.left, union.right].into_iter().zip(expected_cases) {
        let symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
            program.tables.bodies.expressions.expression(expression)
        else {
            panic!("union operand should remain a case membership");
        };
        assert!(!membership.domain_symbol.is_valid());
        assert_eq!(membership.case_type_symbol, command.symbol);
        assert_eq!(membership.case_symbol, expected_case);
    }
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Valid")
        .expect("valid domain");
    let [symbol_resolved_trees::domain::ProofFact::Expression(predicate)] =
        program.proof_facts(domain.facts)
    else {
        panic!("one predicate call");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    assert_eq!(
        program.domain_definitions[0].semantic_id, program.domain_definitions[1].semantic_id,
        "capacity-specialized declarations with the same normalized predicate should share semantic identity",
    );

    let machine = program.machines.first().expect("fill machine");
    let contract = program
        .machine_contracts(machine)
        .iter()
        .find(|contract| {
            contract.kind == symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("fill should retain its ensures contract");
    let [symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
        program.proof_facts(contract.facts)
    else {
        panic!("ensures should contain one domain membership")
    };
    assert!(membership.domain_symbol.is_valid());
}

#[test]
fn preserves_operator_declarations() {
    let source = r#"
    pub operator Slice::index<T>(items: &[T], index: usize) -> T
    requires
        index < items.len;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

    assert_eq!(program.operators.len(), 1);
    let operator = &program.operators[0];
    assert!(operator.is_public);
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
fn resolves_operator_const_parameter_carriers() {
    let source = r#"
    pub operator ConstSurface::identity<const Count: u64>(
        value: [u8; Count]
    ) -> [u8; Count];
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let operator = program.operators.first().expect("const-generic operator");
    let [parameter] = program.data_type_parameters(operator.type_parameters) else {
        panic!("one const parameter")
    };
    let symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: symbol_resolved_trees::types::TypeReference::Named { symbol, name },
    } = &parameter.kind
    else {
        panic!("const parameter carrier")
    };
    assert_eq!(name.as_str(), "u64");
    assert!(
        symbol.is_valid(),
        "const carrier must retain semantic identity"
    );
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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

    operator + add(left: i32 in Degrees, right: i32 in Degrees) -> i32 in Degrees;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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

    operator + add(left: i32 in Degrees, right: i32 in Radians) -> i32;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("competing operand domains must not infer an operator home");
    assert!(
        diagnostic[0]
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

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
    use language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Checked
    established by CheckedIssuer::issue;
    domain Token::Admitted
    established by BoundaryIssuer::issue;

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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

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
fn normalizes_authored_exact_machine_route() {
    use language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Token::stamp;

    machine Token::stamp(value: u64) -> Token
    ensures result in Token::Stamped
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Stamped")
        .expect("stamped domain");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Token::stamp")
        .expect("stamp machine");
    assert_eq!(
        domain.establishment_routes,
        [DomainEstablishmentRoute::ExactMachine {
            machine: machine.symbol,
        }]
    );
}

#[test]
fn exact_machine_route_rejects_a_machine_without_domain_authority() {
    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Token::stamp;

    machine Token::stamp(value: u64) -> Token
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a machine route must authorize the domain on its exact result");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not name the domain on its exact result")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn establishment_route_rejects_ambiguity_across_declaration_kinds() {
    let source = r#"
    data Token { value: u64; }

    domain Token::Stamped
    established by Issuer::issue;

    trait Issuer {
        machine issue(value: u64) -> Token in Stamped;
    }

    machine Issuer::issue(value: u64) -> Token
    ensures result in Token::Stamped
    {
        Token { value: value }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostics = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a route naming both a requirement and a machine must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("ambiguous across declaration kinds")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn authored_establishment_route_cannot_use_an_extension_only_domain_constraint() {
    let base = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;
        trait Issuer {
            machine issue(value: u64) -> Token in Later;
        }
    "#;
    let extension = "domain Token::Later = Token::Issued;";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let mut syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let extension_syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    syntax.extend_from(&extension_syntax);

    let diagnostics = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("an authored route must not gain authority from a hidden domain alias");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not name the domain on its exact result")),
        "unexpected domain-establishment diagnostics: {diagnostics:?}"
    );
}

#[test]
fn preserves_explicit_progress_profile_classification_during_resolution() {
    let source = r#"
    data SchedulerHandle {}
    domain SchedulerHandle::WeakFair
    satisfies ProgressProfile
    established by SchedulerAdmission::grant;
    boundary trait SchedulerAdmission {
        machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "SchedulerHandle::WeakFair")
        .expect("profile domain");

    assert_eq!(
        domain.classification,
        Some(language_semantics::DomainClassification::ProgressProfile)
    );
    assert!(matches!(
        domain.establishment_routes.as_slice(),
        [language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }]
    ));
}

#[test]
fn boundary_requirement_route_accepts_exact_non_self_parameter_domain() {
    use language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }
    domain Token::Pending
    established by BoundaryIngress::enter;
    boundary trait BoundaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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
    domain Token::Pending
    established by OrdinaryIngress::enter;
    trait OrdinaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("an ordinary call must treat its parameter domain as a precondition");
    assert!(diagnostic[0].message.contains(
        "does not name the domain on its exact result or an exact non-self external-root parameter"
    ));
}

#[test]
fn signature_free_route_selects_the_occurrence_package_scope() {
    // A dependency's unmoduled trait must not collide with the importing
    // package's same-leaf declaration: signature-free requirement resolution
    // scopes its candidates to the occurrence's module/package before pooling.
    // The route authored inside the dependency therefore keeps its own owner
    // while the package's `reaches` row keeps the package's trait. This
    // witnessed regression appeared when hosted package compilations seeded
    // `core`: a package's own `ExtentRootProvider` collided with core's.
    let dependency = r#"
        pub data Extent { base: u64; length: u64; }
        pub domain Extent::Granted
        established by
            ExtentRootProvider::grant;
        pub boundary trait ExtentRootProvider {
            machine grant(root: Extent) -> Extent
            ensures
                result in Extent::Granted;
        }
    "#;
    let package = r#"
        pub boundary trait ExtentRootProvider {}
        pub machine exercise() reaches ExtentRootProvider {}
    "#;
    let mut sources = SourceMap::default();
    let dependency_id = sources
        .add_with_metadata(
            PathBuf::from("dependency/extent.omg"),
            dependency.to_owned(),
            PathBuf::from("dependency"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let package_id = sources
        .add_with_metadata(
            PathBuf::from("package/main.omg"),
            package.to_owned(),
            PathBuf::from("package"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let dependency_tokens = Lexer::new(dependency)
        .tokenize()
        .expect("tokenize dependency");
    let mut syntax =
        parse_syntax_trees_with_id(dependency_id, &dependency_tokens).expect("parse dependency");
    let package_tokens = Lexer::new(package).tokenize().expect("tokenize package");
    let package_syntax =
        parse_syntax_trees_with_id(package_id, &package_tokens).expect("parse package");
    syntax.extend_from(&package_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("same-leaf traits in separate packages must not collide");

    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Extent::Granted")
        .expect("granted domain");
    let [
        language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        },
    ] = domain.establishment_routes.as_slice()
    else {
        panic!(
            "one boundary-requirement route: {:?}",
            domain.establishment_routes
        )
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(*boundary_trait)
            .map(|span| span.source_id),
        Some(dependency_id),
        "the dependency's route selects its own trait"
    );
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(*requirement)
            .map(|span| span.source_id),
        Some(dependency_id),
        "the requirement belongs to the dependency's trait"
    );

    let exercise = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("package machine");
    let reach_row = program
        .authored_service_reach_rows
        .iter()
        .find(|row| row.owner == exercise.symbol)
        .expect("authored reach row");
    let [target] = reach_row.targets.as_slice() else {
        panic!("one authored reach target: {:?}", reach_row.targets)
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(target.service)
            .map(|span| span.source_id),
        Some(package_id),
        "the package's `reaches` selects its own trait"
    );
}

#[test]
fn signature_free_route_still_rejects_a_contested_same_package_leaf() {
    // Scoping narrows the pool to the occurrence's dependency scope; it does
    // not weaken the exact-selection contract. Two same-package declarations
    // competing for one leaf remain ambiguous.
    let first = "pub boundary trait Shared {}";
    let second = "pub boundary trait Shared {}";
    let user = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Shared::issue;
    "#;
    let mut sources = SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (index, text) in [first, second, user].into_iter().enumerate() {
        let source_id = sources
            .add_with_metadata(
                PathBuf::from(format!("package/{index}.omg")),
                text.to_owned(),
                PathBuf::from("package"),
                None,
                SourceOrigin::User,
            )
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse");
    }
    let diagnostics = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("same-package leaf collision still rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not resolve to one exact trait")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn rejects_unresolved_authored_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by MissingIssuer::issue;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic =
        resolve(ResolutionRequest::new(&syntax_trees)).expect_err("route must resolve exactly");
    assert!(
        diagnostic[0]
            .message
            .contains("does not resolve to one exact trait")
    );
}
