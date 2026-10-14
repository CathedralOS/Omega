use crate::parser::parse_syntax_trees;
use language_core::ReferenceAccess;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementNode;
use syntax_trees::types::TypeReferenceNode;

/// The first expression argument of the first call statement in the named
/// machine's entry state — the shared extraction the borrow-access tests use.
fn machine_first_call_argument(
    parsed: &syntax_trees::SyntaxTrees,
    machine_name: &str,
) -> syntax_trees::expression::ExpressionHandle {
    let machine = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("caller machine");
    let state = parsed.items.state(
        *parsed
            .items
            .state_handles(machine.states)
            .first()
            .expect("caller entry state"),
    );
    let StatementNode::Call(call) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("caller statement should be a call");
    };
    parsed.statements.expression_handles(call.arguments)[0]
}

#[test]
fn parses_domain_definition_surface() {
    let source = r#"
        domain Player::Alive(player)
        requires
            player in Player::Valid;
            player.health > 0
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name.as_str(), "Player::Alive");
    assert!(domains[0].target_type.is_valid());
    assert_eq!(
        domains[0].predicate_body,
        language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[0].facts).len(), 2);
    assert!(domains[0].semantic_clause_token_count > 3);

    let facts = parsed.items.proof_facts(domains[0].facts);
    assert!(matches!(
        facts[0],
        syntax_trees::item::ProofFact::Membership(_)
    ));
    assert!(matches!(
        facts[1],
        syntax_trees::item::ProofFact::Expression(_)
    ));
    let source_slices = domains[0]
        .facts
        .handles()
        .map(|handle| {
            let span = parsed
                .items
                .proof_fact_source_span(handle)
                .expect("authored proof fact source span");
            &source[span.span.start..span.span.end]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        source_slices,
        ["player in Player::Valid", "player.health > 0"]
    );
}

#[test]
fn parses_domain_requires_and_requirement_routes_independently() {
    let source = r#"
        domain Reservation::Confirmed(reservation)
        requires
            reservation.seats > 0
        established by
            Reservations::confirm,
            Imported::Reservations::restore;
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let domain = super::root_domain(&parsed, "domain");

    assert_eq!(
        domain.predicate_body,
        language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domain.facts).len(), 1);
    assert_eq!(
        domain
            .authored_routes
            .iter()
            .map(|route| {
                route
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .collect::<Vec<_>>(),
        ["Reservations::confirm", "Imported::Reservations::restore"]
    );
    assert!(domain.operators.is_empty());
}

#[test]
fn parses_unqualified_machine_establishment_routes() {
    let source = "domain u64::Issued established by issue, Factory::issue;";
    let parsed = super::parse_source_verbose(source, "tokens", "free and attached issuer paths");
    let domain = super::root_domain(&parsed, "domain");
    let routes = domain
        .authored_routes
        .iter()
        .map(|route| {
            route
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(routes, [vec!["issue"], vec!["Factory", "issue"]]);

    for malformed in ["", "issue::", "issue,", "issue()", "issue<u64>"] {
        let source = format!("domain u64::Issued established by {malformed};");
        let tokens = Lexer::new(&source).tokenize().expect("tokens");
        assert!(parse_syntax_trees(&tokens).is_err(), "{source}");
    }
}

#[test]
fn parses_explicit_progress_profile_classification() {
    let source = r#"
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant_weak_fair;
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let domain = super::root_domain(&parsed, "domain");

    assert_eq!(
        domain.classification,
        Some(language_core::DomainClassification::ProgressProfile)
    );
    assert_eq!(domain.authored_routes.len(), 1);
    assert!(domain.semantic_clause_token_count > 0);
}

#[test]
fn rejects_unknown_or_duplicate_domain_classifications() {
    for (source, expected) in [
        (
            "domain Scheduler::Fair satisfies UserProfile;",
            "unknown compiler-owned domain classification `UserProfile`",
        ),
        (
            "domain Scheduler::Fair satisfies ProgressProfile satisfies ProgressProfile;",
            "at most one compiler-owned classification",
        ),
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "classification must reject",
        );
        assert!(error.message.contains(expected), "got: {}", error.message);
    }
}

#[test]
fn rejects_domain_classification_after_requires() {
    let source = r#"
        domain Scheduler::Fair
        requires true
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        "#;
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "classification order must reject",
    );
    super::expect_error(
        &error,
        "classification must appear immediately after the domain head",
    );
}

#[test]
fn rejects_legacy_domain_route_bodies_with_migration_guidance() {
    let source = r#"
        domain Reservation::Issued {
            Reservations::issue;
        }
        "#;

    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "route bodies must be retired",
    );
    super::expect_error(&error, "domain route bodies are retired");
    assert!(error.message.contains("`established by Trait::requirement"));
}

#[test]
fn rejects_legacy_domain_body_predicates_with_migration_guidance() {
    let source = r#"
        domain Player::Alive {
            self.health > 0;
        }
        "#;

    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "body predicates must be retired",
    );
    super::expect_error(&error, "domain predicates must be written in `requires`");
    assert!(error.message.contains("`established by Trait::requirement"));
}

#[test]
fn rejects_nested_domain_operators_with_top_level_home_guidance() {
    let source = r#"
        domain i32::Degrees {
            operator + add(left: i32, right: i32) -> i32;
        }
        "#;

    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "nested operators must be retired",
    );
    super::expect_error(
        &error,
        "domain operators must be ordinary top-level declarations",
    );
    assert!(
        error
            .message
            .contains("`operator Type::Domain::operation ...`")
    );
}

#[test]
fn parses_equivalent_bodyless_domain_spellings_distinct_from_true_predicate() {
    let source = r#"
        domain Reservation::Issued;
        domain Reservation::Recorded;
        domain Reservation::Universal
        requires
            true;
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 3);
    for domain in &domains[..2] {
        assert_eq!(
            domain.predicate_body,
            language_core::DomainPredicateBody::Bodyless
        );
        assert!(domain.facts.is_empty());
        assert_eq!(domain.semantic_clause_token_count, 0);
    }
    assert_eq!(
        domains[2].predicate_body,
        language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[2].facts).len(), 1);
    assert!(domains[2].semantic_clause_token_count > 0);
}

#[test]
fn parses_transparent_domain_alias_as_an_independent_record() {
    let source = r#"
        pub domain Socket::Usable =
            Socket::Connected & Socket::Authenticated;
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let domain = super::root_domain(&parsed, "alias domain");

    let alias = domain.alias.as_ref().expect("transparent alias record");
    assert!(domain.is_public);
    let paths = alias
        .constituents
        .iter()
        .map(|path| {
            parsed
                .items
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths, ["Socket::Connected", "Socket::Authenticated"]);
    assert_eq!(
        domain.predicate_body,
        language_core::DomainPredicateBody::Bodyless
    );
    assert!(domain.facts.is_empty());
    assert!(domain.operators.is_empty());
}

#[test]
fn parses_explicit_parameter_with_named_carrier_type() {
    let source = r#"
        data Main {
        }

        machine Main::main(main: &mut Main) {}
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = super::entry_state(&parsed, machine);
    let parameter = parsed.items.state_parameter(
        parsed
            .items
            .state_parameters(state.parameters)
            .first()
            .copied()
            .expect("explicit parameter"),
    );

    let TypeReferenceNode::Reference {
        referee, access, ..
    } = parsed
        .type_references
        .type_reference(parameter.type_reference)
    else {
        panic!("explicit mutable borrow should retain its reference ownership mode");
    };
    assert_eq!(*access, ReferenceAccess::Mutable);
    assert!(matches!(
        parsed.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str() == "Main"
    ));
}

#[test]
fn parses_explicit_write_only_borrow_with_exact_access_mode() {
    let source = r#"
        machine fill(destination: &write bool, value: bool) {
            destination = value;
        }

        machine caller(destination: &mut bool) {
            fill(&write destination, true);
        }
    "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let argument = machine_first_call_argument(&parsed, "caller");
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(argument) else {
        panic!("argument should retain one closed borrow-expression node");
    };
    assert_eq!(borrow.access, ReferenceAccess::WriteOnly);
    assert_eq!(
        parsed.expressions.display_name(argument),
        "&write destination"
    );
}

#[test]
fn parses_explicit_shared_borrow_with_exact_access_mode() {
    let source = r#"
        machine observe(value: &bool) {}

        machine caller(source: &bool) {
            observe(&source);
        }
    "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let argument = machine_first_call_argument(&parsed, "caller");
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(argument) else {
        panic!("argument should retain one closed shared-borrow expression node");
    };
    assert_eq!(borrow.access, ReferenceAccess::Shared);
    assert_eq!(parsed.expressions.display_name(argument), "&source");
}

#[test]
fn parses_explicit_parameter_use_as_ordinary_name() {
    let source = r#"
        data Main {
        }

        machine Main::main(main: &mut Main) {
            main;
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = super::entry_state(&parsed, machine);
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("expression statement"),
    );
    let StatementNode::Expression(expression) = statement else {
        panic!("expected expression statement");
    };

    let ExpressionNode::Name(path) = parsed.expressions.expression(*expression) else {
        panic!("explicit parameter must remain an ordinary name");
    };
    assert_eq!(
        parsed.expressions.identifier_path_members(*path)[0].as_str(),
        "main"
    );
}

#[test]
fn parses_nested_call_arguments_as_contiguous_expression_spans() {
    let source = r#"
        data Player {
            xp: i32;
            level: i32;
        }

        data Main {
            xp_table: Player;
        }

        machine Main::main(main: &mut Main, player: &mut Player) {
            player.xp = max(0, player.xp - xp_required(player.level));

            state xp_required(level: i32) -> i32 {
                10
            }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = super::entry_state(&parsed, machine);
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("assignment statement"),
    );
    let StatementNode::Assignment(assignment) = statement else {
        panic!("expected assignment statement");
    };

    assert_eq!(
        parsed.expressions.display_name(assignment.value),
        "max(0, player.xp - xp_required(player.level))"
    );
}

#[test]
fn parses_positional_erased_evidence_call_lane_after_semicolon() {
    let source = r#"
        machine Main::main(value: i32, first_proof: Evidence, second_proof: Evidence) {
            consume(value; first_proof, second_proof);
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "evidence lane should parse");
    let machine = super::root_machine(&parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statement = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0]);
    let StatementNode::Call(call) = statement else {
        panic!("expected call statement");
    };

    assert_eq!(
        parsed.statements.expression_handles(call.arguments).len(),
        1
    );
    assert_eq!(
        call.evidence_arguments
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["first_proof", "second_proof"]
    );
}

#[test]
fn parses_evidence_only_call_lane_with_leading_semicolon() {
    let source = r#"
        machine Main::main(proof: Evidence) {
            consume(; proof);
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "evidence-only lane should parse");
    let machine = super::root_machine(&parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::Call(call) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("expected call statement");
    };
    assert!(
        parsed
            .statements
            .expression_handles(call.arguments)
            .is_empty()
    );
    assert_eq!(call.evidence_arguments[0].as_str(), "proof");
}

#[test]
fn parses_evidence_lane_on_named_transition_without_dropping_it() {
    let source = r#"
        machine Main::main(proof: Evidence) {
            transition { _ -> next(; proof) }
            state next() {}
        }
    "#;
    let parsed = super::parse_source_verbose(
        source,
        "tokenize",
        "named transition evidence lane should parse",
    );
    let machine = super::root_machine(&parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::Transition(transition) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("expected transition statement");
    };
    let syntax_trees::statement::TransitionTargetNode::Named {
        evidence_arguments, ..
    } = parsed.statements.transition_target(transition.target)
    else {
        panic!("expected named transition target");
    };
    assert_eq!(
        evidence_arguments
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["proof"]
    );
}

#[test]
fn tail_self_call_rewrite_retains_the_authored_target_span() {
    let source = r#"
        machine repeat(remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            repeat(remaining - 1)
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "tail self call should parse");
    let machine = super::root_machine(&parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::Transition(transition) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("tail self call should rewrite to a transition");
    };
    let syntax_trees::statement::TransitionTargetNode::Named { source_span, .. } =
        parsed.statements.transition_target(transition.target)
    else {
        panic!("tail self call should retain a named target");
    };
    assert_eq!(
        &source[source_span.span.start..source_span.span.end],
        "repeat"
    );
    assert_eq!(source_span.span.start, source.rfind("repeat").unwrap());
}

#[test]
fn rejects_self_as_ordinary_declaration_name() {
    let source = r#"
        data self {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    assert!(parse_syntax_trees(&tokens).is_err());
}

#[test]
fn parses_machine_parameter_with_mandatory_contract() {
    let source = r#"
        data Card {}
        data Deck {}

        machine Deck::best<machine Key>(deck: &Deck, card: &Card) -> u64
        where machine Key(value: &Card) -> u64
        reaches Console
        requires value in Card::Scorable
        {
            0
        }
        "#;

    let parsed = super::parse_source(source, "machine parameter should parse");
    let machine = super::root_machine(&parsed, "generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name.as_str(), "Key");
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
    } = &parameters[0].kind
    else {
        panic!("Key should carry its authored machine contract");
    };
    assert_eq!(contract.name.as_str(), "Key");
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
    assert!(contract.return_type.is_valid());
    assert_eq!(
        parsed
            .items
            .identifier_path_members(contract.service_reaches)
            .len(),
        1
    );
    assert_eq!(
        parsed.items.capability_contracts(contract.contracts).len(),
        1
    );
}

#[test]
fn parses_nominal_machine_parameter_requirement() {
    let source = r#"
        machine register<machine Selected>()
        where machine Selected satisfies platform::WindowProcedure::call;
        {
            Selected()
        }
        "#;

    let parsed = super::parse_source(source, "nominal machine contract should parse");
    let machine = super::root_machine(&parsed, "generic machine");
    let [parameter] = parsed.items.type_parameters(machine.type_parameters) else {
        panic!("expected one machine parameter");
    };
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Nominal { requirement, .. }),
    } = &parameter.kind
    else {
        panic!("Selected should retain its nominal requirement path");
    };
    assert_eq!(
        parsed
            .items
            .identifier_path_members(*requirement)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["platform", "WindowProcedure", "call"]
    );
    assert!(!machine.bodyless);
}

#[test]
fn parses_bodyless_nominal_machine_parameter_with_one_semicolon() {
    let source = r#"
        boundary machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        "#;

    let parsed = super::parse_source(source, "bodyless nominal binder should parse");
    let machine = super::root_machine(&parsed, "bodyless generic machine");
    assert!(machine.bodyless);
    assert!(matches!(
        parsed.items.type_parameters(machine.type_parameters)[0].kind,
        syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Nominal { .. })
        }
    ));
}

#[test]
fn parses_target_scoped_bodyless_boundary_machine() {
    let source = r#"
        boundary trait Console { machine exit_process(code: i32); }
        data ConsoleNativeProvider {}
        linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(code: i32)
            satisfies Console::exit_process;
        "#;

    let parsed = super::parse_source_verbose(
        source,
        "tokenize target-scoped catalog leaf",
        "parse target-scoped catalog leaf",
    );
    let machine = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::exit_process")
        .expect("target-scoped catalog leaf");
    assert!(machine.boundary);
    assert!(machine.bodyless);
    assert_eq!(
        machine.target.as_ref().map(|target| target.as_str()),
        Some("linux_x86_64"),
    );
    let [conformance] = parsed.items.satisfies_clauses(machine.satisfies) else {
        panic!("one exact satisfies edge")
    };
    assert!(conformance.via.is_none());
    assert!(!conformance.via_expression.is_valid());
}

#[test]
fn parses_structural_and_nominal_machine_parameter_contracts_together() {
    let source = r#"
        machine apply<machine Schema, machine Selected>()
        where machine Schema(value: u64) -> u64;
        where machine Selected satisfies WindowProcedure::call;
        {
        }
        "#;

    let parsed = super::parse_source(source, "mixed machine contracts should parse");
    let machine = super::root_machine(&parsed, "generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert!(matches!(
        parameters[0].kind,
        syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(_))
        }
    ));
    assert!(matches!(
        parameters[1].kind,
        syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Nominal { .. })
        }
    ));
}

#[test]
fn retains_nominal_machine_parameter_trait_application() {
    let source = r#"
        trait Visitor<Context> {
            machine visit(context: &mut Context);
        }
        machine walk<Context, machine Step>(context: &mut Context)
        where machine Step satisfies Visitor<Context>::visit;
        {
            Step(context);
        }
    "#;
    let parsed = super::parse_source(source, "an applied nominal binder should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "walk" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("generic machine");
    let step = parsed
        .items
        .type_parameters(machine.type_parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == "Step")
        .expect("machine parameter");
    let syntax_trees::item::TypeParameterKind::Machine {
        contract:
            Some(syntax_trees::item::MachineParameterContract::Nominal {
                requirement,
                trait_arguments,
            }),
    } = &step.kind
    else {
        panic!("Step should retain its nominal requirement");
    };
    assert_eq!(
        parsed
            .items
            .identifier_path_members(*requirement)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Visitor", "visit"]
    );
    assert_eq!(trait_arguments.len(), 1);
}

#[test]
fn rejects_malformed_nominal_machine_parameter_requirements() {
    let cases = [
        (
            "where machine Selected satisfies WindowProcedure;",
            "must name an exact `Trait::requirement`",
        ),
        (
            "where machine Selected satisfies WindowProcedure::call as Choice;",
            "do not accept `as Name`",
        ),
        (
            "where machine Selected satisfies WindowProcedure::call via Native;",
            "cannot use `via`",
        ),
        (
            "where machine Selected satisfies WindowProcedure<u64>;",
            "must be followed by `::requirement`",
        ),
    ];

    for (contract, expected) in cases {
        let source = format!("machine register<machine Selected>() {contract} {{ Selected() }}");
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "malformed nominal binder must reject",
        );
        super::expect_error(&error, expected);
    }
}

#[test]
fn rejects_one_off_machine_member_requirements_instead_of_discarding_them() {
    let source = r#"
        data Device {}

        machine poll_once<T>(device: &mut T)
        where machine T::poll(value: &mut T)
        {
        }
        "#;

    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "a one-off member requirement has no semantic carrier and must reject",
    );
    assert!(
        error
            .message
            .contains("one-off `where machine T::member(...)` requirements are unsupported"),
        "unexpected diagnostic: {}",
        error.message
    );
}

#[test]
fn parses_higher_order_machine_parameter_contract() {
    let source = r#"
        machine apply<machine Schema, machine Sample>(value: u64) -> u64
        where machine Schema<machine Inner>(value: u64) -> u64
        where machine Inner(value: u64) -> u64;
        where machine Sample(value: u64) -> u64;
        {
            Schema<Sample>(value)
        }
        "#;

    let parsed = super::parse_source(source, "higher-order contract should parse");
    let machine = super::root_machine(&parsed, "generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 2);
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(schema)),
    } = &parameters[0].kind
    else {
        panic!("Schema should carry its authored machine contract");
    };
    let nested = parsed.items.type_parameters(schema.type_parameters);
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name.as_str(), "Inner");
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(inner)),
    } = &nested[0].kind
    else {
        panic!("Inner should carry its authored nested contract");
    };
    assert_eq!(parsed.items.state_parameters(inner.parameters).len(), 1);
}

#[test]
fn rejects_higher_order_parameter_without_nested_contract() {
    let source = r#"
        machine apply<machine Schema>()
        where machine Schema<machine Inner>()
        {
        }
        "#;
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "missing nested contract must fail",
    );
    super::expect_error(
        &error,
        "machine parameter `Inner` requires an authored declaration-site contract",
    );
}

#[test]
fn rejects_machine_parameter_without_authored_contract() {
    let source = r#"
        machine map<machine F>() -> u64 {
            0
        }
        "#;
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "missing contract must fail",
    );
    super::expect_error(&error, "requires an authored declaration-site contract");
}

#[test]
fn parses_machine_parameter_on_proof_data_declaration() {
    let source = r#"
        data Stream<machine S>
        where machine S(index: u64) -> u64;
        {
            case More(tail: Stream<S>);
        }
    "#;
    let parsed = super::parse_source(source, "proof-data machine parameter should parse");
    let data = super::root_data(&parsed, "Stream declaration");
    let parameters = parsed.items.type_parameters(data.type_parameters);
    assert_eq!(parameters.len(), 1);
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
    } = &parameters[0].kind
    else {
        panic!("S should retain its authored callable contract");
    };
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
    assert!(contract.return_type.is_valid());
}

#[test]
fn parses_proof_quotient_data_declaration() {
    let source = r#"
        data Carrier {
            case Value;
        }

        data Quotient = Carrier % Laws::equivalent;
    "#;
    let parsed = super::parse_source(source, "quotient declaration should parse");
    let quotient = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Quotient" => {
                data.quotient.as_ref()
            }
            _ => None,
        })
        .expect("Quotient metadata");
    assert!(matches!(
        parsed.type_references.type_reference(quotient.carrier),
        TypeReferenceNode::Named(name) if name.as_str() == "Carrier"
    ));
    assert_eq!(
        parsed
            .items
            .identifier_path_members(quotient.relation)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Laws", "equivalent"]
    );
}

#[test]
fn parses_static_machine_symbol_call_argument() {
    let source = r#"
        data Card {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {
        }

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
    "#;
    let parsed = super::parse_source(source, "static machine argument should parse");
    let (_, call) = parsed
        .expressions
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            syntax_trees::expression::ExpressionNode::Call(call)
                if !call.machine_arguments.is_empty() =>
            {
                Some((handle, call))
            }
            _ => None,
        })
        .expect("generic call expression");
    assert_eq!(call.machine_arguments.len(), 1);
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Card", "power"]
    );
}

#[test]
fn parses_nested_static_conformance_application() {
    let source = r#"
        trait Encodes<T> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output> satisfies Encodes<Output> {}

        machine send<T, Encoding satisfies Encodes<T>>() {}
        machine caller() {
            send<Message, SequenceEncoding<'scope, Bytes, Message>>();
        }
    "#;
    let parsed = super::parse_source(source, "nested static application should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            syntax_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "send" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("generic call expression");
    let application = call.machine_arguments[1]
        .application
        .as_ref()
        .expect("nested conformance application");
    assert_eq!(
        call.machine_arguments[1].path[0].as_str(),
        "SequenceEncoding"
    );
    assert_eq!(application.lifetime_arguments[0].as_str(), "scope");
    assert_eq!(application.arguments.len(), 2);
    assert_eq!(application.arguments[0].path[0].as_str(), "Bytes");
    assert_eq!(application.arguments[1].path[0].as_str(), "Message");
    assert_eq!(
        call.display_name(&parsed.expressions),
        "send<Message, SequenceEncoding<'scope, Bytes, Message>>()"
    );
}

#[test]
fn parses_evidence_term_member_as_a_distinct_proof_static_argument() {
    let source = r#"
        machine consume<machine Witness>()
        where machine Witness();
        {}

        machine caller() {
            consume<proof.modulus>();
        }
    "#;
    let parsed = super::parse_source(source, "evidence projection should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            syntax_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "consume" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("generic call expression");
    let [argument] = call.machine_arguments.as_ref() else {
        panic!("one proof-static argument")
    };
    assert!(argument.path.is_empty());
    assert!(argument.const_literal.is_none());
    let projection = argument
        .evidence_projection
        .as_ref()
        .expect("term.member projection");
    assert_eq!(projection.term.as_str(), "proof");
    assert_eq!(projection.member.as_str(), "modulus");
    assert_eq!(
        call.display_name(&parsed.expressions),
        "consume<proof.modulus>()"
    );
}

#[test]
fn destructure_marker_preserves_double_underscore_field_as_one_component() {
    let source = r#"
        data Pair { left__value: i32; right: i32; }
        machine inspect(pair: Pair) {
            let { left__value, right as _ } = pair;
        }
    "#;
    let parsed = super::parse_source(source, "destructure should parse");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = super::entry_state(&parsed, machine);
    let marker = parsed
        .items
        .statements(state.statements)
        .iter()
        .find_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__destructure#") =>
            {
                Some(local)
            }
            _ => None,
        })
        .expect("destructure marker local");

    assert_eq!(
        marker.name.as_str(),
        "__destructure#left__value#right",
        "the internal delimiter must not split repeated underscores"
    );
}

#[test]
fn proof_output_binding_separates_type_and_prop_lanes() {
    let source = r#"
        machine produce() -> i32
        ensures proof: true
        { proof = true; 7 }

        machine consume() -> i32 {
            let (value; proof: local_proof) = produce();
            value
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "proof-output lane should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "consume" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("consume machine");
    let state = super::entry_state(&parsed, machine);
    let package = parsed
        .items
        .statements(state.statements)
        .iter()
        .find_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::ProofOutputBindingStatement(package) => Some(package),
            _ => None,
        })
        .expect("one internal proof-output binding carrier");
    assert_eq!(package.bindings.len(), 2);
    assert_eq!(package.bindings[0].output_field.as_str(), "value");
    assert_eq!(package.bindings[0].binding.as_str(), "value");
    assert_eq!(package.bindings[1].output_field.as_str(), "proof");
    assert_eq!(package.bindings[1].binding.as_str(), "local_proof");
}

#[test]
fn retired_generated_proof_package_has_directed_migration() {
    let source = r#"
        machine produce() ensures proof: true { proof = true; }
        machine consume() { let { proof: local } = produce(); }
    "#;
    let error = super::parse_source_error(source, "tokenize", "old package spelling must reject");
    assert!(error.message.contains("proof-output packages are retired"));
    assert!(
        error
            .message
            .contains("let (value; public_output: local_term)")
    );
}

fn parse_domain_facts(
    source: &str,
) -> (
    syntax_trees::SyntaxTrees,
    Vec<syntax_trees::item::ProofFact>,
) {
    let parsed = super::parse_source(source, "parse should succeed");
    let facts = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Domain(domain) => Some(domain.facts),
            _ => None,
        })
        .expect("one domain");
    let facts = parsed.items.proof_facts(facts).to_vec();
    (parsed, facts)
}

#[test]
fn proof_fact_membership_carries_indexed_domain_application_by_argument() {
    let (parsed, facts) = parse_domain_facts(
        r#"
        domain Extent::Placed(extent)
        requires
            extent in Granted & Resident<SlotPlacement, Slot>
        "#,
    );
    let [
        syntax_trees::item::ProofFact::Membership(granted),
        syntax_trees::item::ProofFact::Membership(resident),
    ] = facts.as_slice()
    else {
        panic!("an `&` chain lowers to one membership fact per domain, got {facts:?}");
    };
    assert_eq!(
        parsed.items.identifier_path_members(granted.domain)[0].as_str(),
        "Granted"
    );
    assert!(granted.domain_arguments.is_empty());
    assert_eq!(
        parsed.items.identifier_path_members(resident.domain)[0].as_str(),
        "Resident"
    );
    let arguments = parsed
        .type_references
        .type_reference_handles(resident.domain_arguments)
        .iter()
        .map(
            |argument| match parsed.type_references.type_reference(*argument) {
                TypeReferenceNode::Named(name) => name.as_str().to_owned(),
                other => panic!("index argument should be a named leaf, got {other:?}"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(arguments, ["SlotPlacement", "Slot"]);
}

#[test]
fn proof_fact_pipe_alternative_rejects_indexed_domain_application() {
    let tokens = Lexer::new(
        r#"
        domain Extent::Placed(extent)
        requires
            extent in Granted | Resident<SlotPlacement, Slot>
        "#,
    )
    .tokenize()
    .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("a `|` alternative has no argument slot");
    super::expect_error(
        &error,
        "indexed domain applications are not carried by `|` proof-fact alternatives",
    );
}

#[test]
fn proof_fact_carry_permission_rejects_index_arguments() {
    let tokens = Lexer::new(
        r#"
        domain Extent::Placed(extent)
        requires
            extent in Carry::AnyCpu<SlotPlacement>
        "#,
    )
    .tokenize()
    .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("carry permissions have no index");
    super::expect_error(
        &error,
        "compiler carry permissions do not take index arguments",
    );
}
