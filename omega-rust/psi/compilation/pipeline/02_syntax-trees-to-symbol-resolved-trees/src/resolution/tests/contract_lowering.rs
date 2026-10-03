use crate::resolution::lowerer::Lowerer;
use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

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
        resolve(ResolutionRequest::new(syntax)).expect_err("overload must reject every use");

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
    let program = resolve(ResolutionRequest::new(syntax_trees)).expect("lowering should succeed");
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
    let program = resolve(ResolutionRequest::new(syntax_trees)).expect("lower");
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
    let program = resolve(ResolutionRequest::new(syntax_trees)).expect("lower");
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
        syntax,
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
        syntax: syntax_trees,
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
fn trait_operator_requirement_retains_fixed_token_after_resolution() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve");
    let trait_definition = program.traits.first().expect("Ranked trait");
    let [requirement] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("one trait operator requirement expected");
    };

    assert_eq!(
        requirement.spelling.map(|spelling| spelling.symbol()),
        Some("<")
    );
}
