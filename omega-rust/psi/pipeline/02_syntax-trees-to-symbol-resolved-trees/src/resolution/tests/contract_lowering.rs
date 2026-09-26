use crate::resolution::lowerer::Lowerer;
use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn rejects_overloaded_signature_free_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by Issuer::issue;
    trait Issuer {
        machine issue(value: u64) -> Token in Issued;
        machine issue(value: i64) -> Token in Issued;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a signature-free requirement path must not choose among overloads");
    assert_eq!(diagnostic.len(), 2);
    assert!(diagnostic[0].message.contains("declaring trait `Issuer`"));
    assert!(
        diagnostic[1]
            .message
            .contains("does not resolve to one exact trait requirement")
    );
    assert!(
        diagnostic
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
}

#[test]
fn signature_free_overload_reports_one_declaration_and_every_affected_use() {
    let source = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;

        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
            machine issue(value: i64) -> Token in Issued;
        }

        machine register<machine Selected>()
        where machine Selected satisfies Issuer::issue;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let diagnostics =
        resolve(ResolutionRequest::new(&syntax)).expect_err("overload must reject every use");

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("declaring trait `Issuer`"));
    assert!(diagnostics[1].message.starts_with("domain `Token::Issued`"));
    assert!(
        diagnostics[2]
            .message
            .starts_with("nominal machine parameter `Selected`")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
    assert!(
        diagnostics[1].source_span.unwrap().span.start
            < diagnostics[2].source_span.unwrap().span.start
    );
}

#[test]
fn authored_signature_free_requirement_ignores_current_activation_extension_overloads() {
    let base = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;
        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
        }
        machine register<machine Selected>()
        where machine Selected satisfies Issuer::issue;
        {}
    "#;
    let extension = r#"
        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
            machine issue(value: i64) -> Token in Issued;
        }
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            source::SourceOrigin::User,
            source::SourceResolutionStratum::CurrentActivationExtension,
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

    resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect(
        "authored signature-free uses must resolve only against the retained base trait family",
    );
}

#[test]
fn expands_alias_establishment_routes_to_atomic_domains() {
    use language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued
    established by TokenIssuer::issue;
    domain Token::Stamped
    established by TokenIssuer::issue;
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let machine = program.machines.first().expect("machine");
    let contracts = program.machine_contracts(machine);

    assert_eq!(contracts.len(), 2);
    assert!(contracts[0].token_count >= 3);
    assert!(contracts[1].token_count >= 3);
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("resolved contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
    assert_eq!(program.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(program.proof_facts(contracts[1].facts).len(), 1);
}

#[test]
fn lowers_named_contract_evidence_bindings() {
    let source = r#"
    proposition carries(value: i32) evidence i32;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lower");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let contracts = program.machine_contracts(machine);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0].binding.as_ref().map(|name| name.as_str()),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1].binding.as_ref().map(|name| name.as_str()),
        Some("output_proof")
    );
}

#[test]
fn classifies_evidence_forwarding_out_of_runtime_statements() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
        output_proof = input_proof;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lower");
    let [forwarding] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence forwarding expected");
    };
    assert!(forwarding.machine_symbol.is_valid());
    assert!(forwarding.state_symbol.is_valid());
    assert_eq!(forwarding.target.as_str(), "output_proof");
    assert_eq!(forwarding.source.as_str(), "input_proof");
    assert_eq!(forwarding.source_conformance, None);
    assert_eq!(program.snapshot().evidence_forwardings.len(), 1);
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == forwarding.machine_symbol)
        .expect("owner machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    assert!(
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .is_empty(),
        "erased forwarding must not enter runtime statement spans"
    );
}

#[test]
fn resolves_explicit_evidence_producer_to_exact_subjectless_conformance() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    machine produce(value: i32)
    ensures output_proof: carries(value)
    {
        output_proof = ConcreteEvidence;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lower");
    let [assignment] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence assignment expected");
    };
    let producer = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "ConcreteEvidence")
        })
        .expect("subjectless producer conformance");
    assert_eq!(assignment.source_conformance, Some(producer.symbol));
    assert_eq!(
        program.snapshot().evidence_forwardings[0].source_conformance,
        Some(producer.symbol.arena_index())
    );
}

#[test]
fn binds_evidence_forwarding_to_attached_machine_with_duplicate_short_name() {
    let source = r#"
    data Left {}
    data Right {}
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;

    machine Left::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }

    machine Right::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lower");

    assert_eq!(program.evidence_forwardings.len(), 2);
    for (root_index, forwarding) in program.evidence_forwardings.iter().enumerate() {
        let machine = program
            .machines
            .iter()
            .nth(root_index)
            .expect("parallel root machine");
        assert_eq!(forwarding.machine_root_index, root_index);
        assert_eq!(forwarding.machine_symbol, machine.symbol);
    }
    assert_ne!(
        program.evidence_forwardings[0].machine_symbol,
        program.evidence_forwardings[1].machine_symbol
    );
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let witness = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "witness")
        .expect("witness machine");
    let ensures = program
        .machine_contracts(witness)
        .iter()
        .find(|contract| {
            contract.kind == symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("witness ensures");
    let [symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(ensures.facts)
    else {
        panic!("one expression fact")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("equality expression")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");

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
fn transition_target_prefers_state_over_same_named_attached_field() {
    let source = r#"
    data Main { next: bool; }

    machine Main::main(&mut self) {
        transition { _ -> next() }

        state next(&mut self) {}
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
    let machine = program.machines.first().expect("main machine");
    let states = program.machine_state_handles(machine.states);
    let entry = program.machine_state(states[0]);
    let next = program.machine_state(states[1]);
    let symbol_resolved_trees::statement::Statement::Transition(transition) =
        &program.state_statements(entry.statements)[0]
    else {
        panic!("main should transition to next");
    };
    let symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("next should remain a named transition target");
    };

    assert_eq!(target.symbol, next.symbol);
    assert_eq!(
        program.symbols.get(target.symbol).kind,
        symbols::SymbolKind::State
    );
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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
    let symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(issue_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("issue should end in a transition");
    };
    let symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("qualified tail call should remain a named transition");
    };

    assert_eq!(target.symbol, pack_state.symbol);
    assert!(target.head_symbol.is_valid());
}

#[test]
fn attached_calls_and_qualified_transitions_obey_resolution_strata() {
    let base = r#"
        data Gadget {}
        data BaseOnly {}
        data Leaf {}
        data Inner { leaf: Leaf; }
        data Outer { inner: Inner; }
        machine Gadget::work() -> i32 { 1 }
        machine BaseOnly::work() -> i32 { 3 }
        machine Leaf::work(&self) -> i32 { 5 }
        machine authored_call() -> i32 { Gadget::work() }
        machine authored_hidden_call() -> i32 { Ghost::work() }
        machine Outer::nested_call(&self) -> i32 { self.inner.leaf.work() }
        machine authored_transition() -> i32 {
            transition { _ -> Gadget::work() }
        }
    "#;
    let extension = r#"
        data Gadget {}
        data Ghost {}
        data Leaf {}
        machine Gadget::work() -> i32 { 2 }
        machine Ghost::work() -> i32 { 4 }
        machine Leaf::work(&self) -> i32 { 6 }
        machine extension_call() -> i32 { Gadget::work() }
        machine extension_reads_base() -> i32 { BaseOnly::work() }
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
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
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("attached-call resolution must retain stratum boundaries");
    let source_of = |symbol| {
        program
            .symbols
            .symbol_provenance_source_span(symbol)
            .expect("source-backed symbol")
            .source_id
    };
    let calls = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let base_targets = calls
        .iter()
        .filter(|call| call.target.source_span().source_id == base_id)
        .map(|call| call.target_symbol)
        .collect::<Vec<_>>();
    assert_eq!(
        base_targets
            .iter()
            .filter(|symbol| {
                symbol.is_valid()
                    && program.symbols.name(**symbol) == "work"
                    && source_of(**symbol) == base_id
            })
            .count(),
        2,
        "qualified and nested authored calls must select Base"
    );
    assert!(base_targets.iter().any(|symbol| !symbol.is_valid()));
    assert!(calls.iter().any(|call| {
        call.target.source_span().source_id == extension_id
            && call.target_symbol.is_valid()
            && source_of(call.target_symbol) == extension_id
    }));
    assert!(calls.iter().any(|call| {
        call.target.source_span().source_id == extension_id
            && call.target_symbol.is_valid()
            && source_of(call.target_symbol) == base_id
    }));

    let transition_machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored_transition")
        .expect("authored transition machine");
    let transition_state =
        program.machine_state(program.machine_state_handles(transition_machine.states)[0]);
    let symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(transition_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("authored transition remains terminal")
    };
    let symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("authored qualified transition remains named")
    };
    assert_eq!(source_of(target.head_symbol), base_id);
    assert_eq!(source_of(target.symbol), base_id);
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
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("lowering should succeed");
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

    let symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &parameter.type_reference
    else {
        panic!("self parameter should retain its authored reference shell");
    };
    let symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
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
    let program = resolve(ResolutionRequest {
        syntax: &syntax_trees,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
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

#[test]
fn lowerer_keeps_source_free_visibility_checks_permissive() {
    let mut sources = SourceMap::default();
    let declaration = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            "data Extension {}".to_owned(),
            PathBuf::from("."),
            None,
            source::SourceOrigin::User,
            source::SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_span(source::Span::new(5, 14));
    let lowerer = Lowerer::new(Some(Arc::new(sources)), Vec::new());

    assert!(
        lowerer.source_reference_can_see_declaration(source::SourceSpan::default(), declaration,)
    );
}

#[test]
fn authored_base_paths_and_receiverless_calls_ignore_extension_first_declarations() {
    let base = r#"
        data Choice {}
        machine pick() -> i32 { 1 }
        proposition selected(value: i32);
        machine authored(value: Choice) -> i32 { pick() }
        machine authored_missing() -> i32 { generated_pick() }
        proposition authored(value: i32) = selected(value);
        proposition authored_missing(value: i32) = generated_selected(value);
    "#;
    let extension = r#"
        data Choice {}
        machine pick() -> i32 { 2 }
        machine generated_pick() -> i32 { 3 }
        proposition selected(value: i32);
        proposition generated_selected(value: i32);
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            source::SourceOrigin::User,
            source::SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("base references must ignore extension-first declarations");
    let authored = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "authored")
        .expect("authored machine");
    let entry = program
        .machine_state_handles(authored.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("authored entry");
    let parameter = program
        .state_parameters(entry.parameters)
        .first()
        .expect("Choice parameter");
    let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
        &parameter.type_reference
    else {
        panic!("Choice parameter remains nominal")
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(*symbol)
            .expect("Choice declaration provenance")
            .source_id,
        base_id
    );
    let hidden_pick_target = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "generated_pick" =>
            {
                Some(call.target_symbol)
            }
            _ => None,
        })
        .expect("authored receiverless extension-only pick call");
    assert!(!hidden_pick_target.is_valid());
    let pick_target = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "pick" =>
            {
                Some(call.target_symbol)
            }
            _ => None,
        })
        .expect("authored receiverless pick call");
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(pick_target)
            .expect("pick declaration provenance")
            .source_id,
        base_id
    );
    let authored_proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "authored")
        .expect("authored proposition");
    let symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        authored_proposition.body
    else {
        panic!("authored proposition stays transparent")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("authored proposition body stays a call")
    };
    assert_eq!(
        program
            .symbols
            .symbol_provenance_source_span(call.target_symbol)
            .expect("selected proposition provenance")
            .source_id,
        base_id
    );
    let authored_missing_proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "authored_missing")
        .expect("authored missing proposition");
    let symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        authored_missing_proposition.body
    else {
        panic!("authored missing proposition stays transparent")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("authored missing proposition body stays a call")
    };
    assert!(!call.target_symbol.is_valid());
}

#[test]
fn trait_operator_requirement_retains_fixed_token_after_resolution() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let trait_definition = program.traits.first().expect("Ranked trait");
    let [requirement] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("one trait operator requirement expected");
    };

    assert_eq!(
        requirement.spelling.map(|spelling| spelling.symbol()),
        Some("<")
    );
}

#[test]
fn deep_left_associated_boolean_expression_resolves_on_the_default_test_stack() {
    let expression = std::iter::repeat_n("enabled", 128)
        .collect::<Vec<_>>()
        .join(" && ");
    let source =
        format!("data Root {{}} machine Root::measure(enabled: bool) -> bool {{ {expression} }}");
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize deep expression");
    let syntax = parse_syntax_trees(&tokens).expect("parse deep expression");

    resolve(ResolutionRequest::new(&syntax))
        .expect("resolve deep expression on the default test stack");
}
