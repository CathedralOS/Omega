use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementNode;

#[test]
fn retired_capability_entry_names_the_boundary_provider_migration() {
    let source = r#"
        capability TestHost {
            entry host_write(fd: i32, count: u64) -> i32 {
                requires true;
                boundary host;
            }
        }
    "#;
    let error = super::parse_source_error(
        source,
        "tokenize retired capability entry",
        "capability entry must be retired",
    );
    assert!(
        error
            .message
            .contains("legacy `capability { entry ... }` host scaffold")
            && error.message.contains("is retired")
            && error.message.contains("`boundary trait`")
            && error
                .message
                .contains("satisfies Trait::requirement via Binding::..."),
        "got: {}",
        error.message
    );

    let current = r#"
        capability Current {
            entry: bool;
            state inspect() {
                requires true;
            }
        }
    "#;
    let tokens = Lexer::new(current)
        .tokenize()
        .expect("tokenize current capability surface");
    let syntax = parse_syntax_trees(&tokens)
        .expect("ordinary `entry` field and capability state must remain accepted");
    let Some(syntax_trees::item::Item::Capability(capability)) = syntax.root_items().next() else {
        panic!("expected capability");
    };
    let [
        syntax_trees::item::CapabilityMember::Field(field),
        syntax_trees::item::CapabilityMember::State(_),
    ] = syntax.items.capability_members(capability.members)
    else {
        panic!("expected ordinary field followed by current state member");
    };
    assert_eq!(field.name.as_str(&syntax), "entry");
}

#[test]
fn ordinary_entry_named_calls_do_not_become_retired_members() {
    for body in [
        "entry();",
        "entry(value);",
        "entry()",
        "let answer: i32 = entry();",
    ] {
        let source = format!("machine entry() -> i32 {{ 7 }} machine run() {{ {body} }}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize ordinary entry call");
        parse_syntax_trees(&tokens).unwrap_or_else(|error| panic!("{body}: {}", error.message));
    }
}

#[test]
fn retired_explicit_machine_entry_members_name_the_machine_body_migration() {
    for retired in [
        "machine run { entry() {} }",
        "machine run { entry begin() {} }",
        "machine run { public entry() {} }",
        "machine run { public entry begin() {} }",
    ] {
        let error = super::parse_source_error(
            retired,
            "tokenize retired explicit machine entry",
            "explicit machine entry members must be retired",
        );
        assert!(
            error
                .message
                .contains("explicit nested `entry` / `public entry` machine members are retired")
                && error.message.contains("`machine` head")
                && error.message.contains("directly in the machine body")
                && error.message.contains("`public machine`"),
            "got for {retired:?}: {}",
            error.message
        );
    }
}

#[test]
fn retired_trailing_boundary_contracts_name_current_boundary_surfaces() {
    for retired in [
        "machine run() boundary host {}",
        "machine run() boundary LegacyHost {}",
        "capability Legacy { state run() { boundary host; } }",
        "capability Legacy { state run() { boundary LegacyHost; } }",
    ] {
        let error = super::parse_source_error(
            retired,
            "tokenize retired trailing boundary contract",
            "trailing boundary contract clauses must be retired",
        );
        assert!(
            error.message.contains(
                "trailing `boundary host` and `boundary Name` contract clauses are retired"
            ) && error.message.contains("leading `boundary trait`")
                && error.message.contains("`boundary machine`")
                && error.message.contains("`boundary operator`")
                && error
                    .message
                    .contains("`satisfies Trait::requirement via Binding::...`"),
            "got for {retired:?}: {}",
            error.message
        );
    }
}

#[test]
fn erased_join_type_is_rejected_but_join_names_are_ordinary() {
    let retired = "machine run(task: Join<i32>) {}";
    let error = super::parse_source_error(
        retired,
        "tokenize should succeed",
        "Join<T> must be retired",
    );
    let rendered = error.message;
    assert!(rendered.contains("Join<T>") && rendered.contains("finish()"));

    let ordinary = r#"
        data Join { value: i32; }
        machine Join::join(join: &Join) -> i32 {
            transition { _ -> join.value }
        }
    "#;
    let tokens = Lexer::new(ordinary)
        .tokenize()
        .expect("tokenize should succeed");
    parse_syntax_trees(&tokens).expect("Join/join are ordinary names after TR1");
}

#[test]
fn linear_property_is_first_class_on_data_and_type_parameters() {
    let source = r#"
        data Receipt [linear] {}
        data Envelope<T [linear]> [linear] { value: T; }
    "#;
    let parsed = super::parse_source(source, "linear properties should parse");
    let data: Vec<_> = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect();

    assert_eq!(data.len(), 2);
    assert_eq!(
        data[0].properties.multiplicity,
        language_core::Multiplicity::Linear
    );
    assert_eq!(
        data[1].properties.multiplicity,
        language_core::Multiplicity::Linear
    );
    let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
    assert_eq!(
        parameter.bounds.multiplicity,
        language_core::Multiplicity::Linear
    );
}

#[test]
fn multiplicity_property_lists_accept_trailing_commas() {
    let source = "data Copy [copy,] {} data Linear [linear,] {}";
    let parsed = super::parse_source(source, "trailing property commas should parse");
    let multiplicities = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data.properties.multiplicity),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        multiplicities,
        vec![
            language_core::Multiplicity::Unrestricted,
            language_core::Multiplicity::Linear,
        ]
    );
}

#[test]
fn copy_and_linear_properties_are_mutually_exclusive() {
    for source in ["data Bad [copy, linear] {}", "data Bad [linear, copy] {}"] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "copy and linear must not coexist on one declaration",
        );
        assert!(error.message.contains("mutually exclusive"));
    }
}

#[test]
fn carry_property_parses_all_four_axes_on_data_and_bounds() {
    use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
    let source = r#"
        data Lease [
            carry(suspension: forbidden, cpu: same, thread: any, address: stable,),
        ] {}
        data Envelope<T [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )]> { value: T; }
    "#;
    let parsed = super::parse_source_verbose(
        source,
        "tokenize",
        "four-axis carry properties should parse",
    );
    let data = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        data[0].properties.carry,
        Some(CarryPolicy {
            suspension: CarrySuspension::Forbidden,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        })
    );
    let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
    assert_eq!(parameter.bounds.carry, Some(CarryPolicy::PERMISSIVE));
}

#[test]
fn carry_property_requires_every_axis_and_retires_send() {
    let missing = "data Bad [carry(suspension: allowed, cpu: any, thread: any)] {}";
    let error = super::parse_source_error(missing, "tokenize", "partial carry policy must reject");
    assert!(error.message.contains("missing address"));

    let retired = "data Bad [send] {}";
    let error = super::parse_source_error(retired, "tokenize", "send must remain retired");
    assert!(error.message.contains("`[send]` is retired"));
}

#[test]
fn boundary_data_parses_as_an_opaque_carrier_without_a_shape() {
    let tokens = Lexer::new("boundary data ProviderToken [linear];")
        .tokenize()
        .expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("opaque boundary data should parse");
    let data = super::root_data(&parsed, "data item");

    assert_eq!(
        data.supply_mode,
        language_core::DataSupplyMode::BoundaryOpaque
    );
    assert!(data.members.is_empty());
    assert_eq!(
        data.properties.multiplicity,
        language_core::Multiplicity::Linear
    );
}

#[test]
fn parses_plain_and_boundary_traits() {
    let source = r#"
        trait Drawable<Shape> {
            machine draw(value: &Shape, canvas: &mut Canvas);
        }

        boundary trait Console {
            machine write_line(text: String)
            reaches
                Console;
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let traits: Vec<_> = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .collect();

    assert_eq!(traits.len(), 2);
    assert_eq!(traits[0].name.as_str(&parsed), "Drawable");
    assert!(!traits[0].is_boundary);
    assert_eq!(traits[0].machines.len(), 1);
    assert_eq!(traits[1].name.as_str(&parsed), "Console");
    assert!(traits[1].is_boundary);
    assert_eq!(traits[1].machines.len(), 1);
    let signature_handle = parsed.items.state_signatures(traits[1].machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(&parsed), "Console");
}

#[test]
fn parses_trait_owned_fixed_operator_requirements() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
            operator equivalent(left: T, right: T) -> bool;
        }
    "#;
    let parsed = super::parse_source(source, "trait operators should parse");
    let trait_definition = super::root_trait(&parsed, "Ranked trait");
    let requirements = parsed.items.state_signatures(trait_definition.machines);

    assert_eq!(requirements.len(), 2);
    assert_eq!(
        parsed.items.state_signature(requirements[0]).spelling,
        Some(language_core::OperatorSpelling::Less)
    );
    assert_eq!(parsed.items.state_signature(requirements[1]).spelling, None);
}

#[test]
fn parses_independent_operational_clauses_on_machines_and_requirements() {
    let source = r#"
        machine run() reaches Console suspends; blocks; {
        }

        trait Worker {
            machine wait() reaches Clock suspends; blocks; ensures true;
        }

        machine schedule<machine Callback>()
        where machine Callback() suspends; blocks;
        {
        }
        "#;

    let parsed = super::parse_source(source, "operational clauses should parse");
    let machine = super::root_machine(&parsed, "machine item");
    assert!(machine.suspends);
    assert!(machine.blocks);
    assert_eq!(machine.suspends_keyword_source_spans.len(), 1);
    assert_eq!(machine.blocks_keyword_source_spans.len(), 1);
    let service_reaches = parsed
        .items
        .identifier_path_members(machine.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(&parsed), "Console");

    let trait_definition = super::root_trait(&parsed, "trait item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    assert!(signature.suspends);
    assert!(signature.blocks);
    assert_eq!(signature.suspends_keyword_source_spans.len(), 1);
    assert_eq!(signature.blocks_keyword_source_spans.len(), 1);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(&parsed), "Clock");

    let structural_machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str(&parsed) == "schedule" =>
            {
                Some(machine)
            }
            _ => None,
        })
        .expect("structural machine-parameter owner");
    let [parameter] = parsed
        .items
        .type_parameters(structural_machine.type_parameters)
    else {
        panic!("one structural machine parameter");
    };
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
    } = &parameter.kind
    else {
        panic!("Callback should retain its structural signature");
    };
    assert!(contract.suspends);
    assert!(contract.blocks);
    assert_eq!(contract.suspends_keyword_source_spans.len(), 1);
    assert_eq!(contract.blocks_keyword_source_spans.len(), 1);

    let suspends_starts = source
        .match_indices("suspends")
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    let blocks_starts = source
        .match_indices("blocks")
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    assert_eq!(suspends_starts.len(), 3);
    assert_eq!(blocks_starts.len(), 3);
    for (index, source_span) in [
        machine.suspends_keyword_source_spans[0],
        signature.suspends_keyword_source_spans[0],
        contract.suspends_keyword_source_spans[0],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(source_span.span.start, suspends_starts[index]);
        assert_eq!(
            &source[source_span.span.start..source_span.span.end],
            "suspends"
        );
    }
    for (index, source_span) in [
        machine.blocks_keyword_source_spans[0],
        signature.blocks_keyword_source_spans[0],
        contract.blocks_keyword_source_spans[0],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(source_span.span.start, blocks_starts[index]);
        assert_eq!(
            &source[source_span.span.start..source_span.span.end],
            "blocks"
        );
    }
}

#[test]
fn parses_machine_requires_before_reaches_on_one_line() {
    // The fact predicate form (`ensures place Predicate`) must not eat a
    // same-line clause keyword: a `requires` fact ends at `reaches` exactly as
    // it does before `;` or a newline.
    let source = r#"
        data App { a: u32; b: u32; }

        machine App::main(app: &mut App) requires app.a <= app.b reaches Console {
        }
        "#;

    let parsed = super::parse_source(source, "requires before reaches should parse");
    let machine = super::root_machine(&parsed, "machine item");

    let service_reaches = parsed
        .items
        .identifier_path_members(machine.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(&parsed), "Console");

    let [contract] = parsed.items.capability_contracts(machine.contracts) else {
        panic!("machine should carry exactly one requires contract");
    };
    assert!(matches!(
        contract.kind,
        syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
}

#[test]
fn parses_contract_clause_orderings_and_predicate_boundaries() {
    let source = r#"
        data App { a: u32; b: u32; }

        machine App::reaches_then_requires(app: &mut App) reaches Console requires app.a <= app.b {
        }
        machine App::requires_semicolon_reaches(app: &mut App) requires app.a <= app.b; reaches Console {
        }
        machine App::requires_newline_reaches(app: &mut App)
            requires app.a <= app.b
            reaches Console {
        }
        machine App::requires_only(app: &mut App) requires app.a <= app.b {
        }
        machine App::reaches_only(app: &mut App) reaches Console {
        }
        machine App::bare(app: &mut App) {
        }
        machine App::predicate_then_reaches(app: &mut App) ensures app.a settled reaches Console {
        }

        trait Visitor<Visited> {
            machine visit(value: &mut Visited) requires value.a <= value.b reaches Console;
        }
        "#;

    let parsed = super::parse_source(source, "clause orderings should parse");

    let machine = |name: &str| {
        parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str(&parsed) == name =>
                {
                    Some(machine)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("machine {name}"))
    };
    let reach_count = |machine: &syntax_trees::item::Machine| {
        parsed
            .items
            .identifier_path_members(machine.service_reaches)
            .len()
    };
    let contracts = |machine: &syntax_trees::item::Machine| {
        parsed.items.capability_contracts(machine.contracts)
    };

    for (name, reaches, contracts_count) in [
        ("App::reaches_then_requires", 1usize, 1usize),
        ("App::requires_semicolon_reaches", 1, 1),
        ("App::requires_newline_reaches", 1, 1),
        ("App::requires_only", 0, 1),
        ("App::reaches_only", 1, 0),
        ("App::bare", 0, 0),
        ("App::predicate_then_reaches", 1, 1),
    ] {
        let machine = machine(name);
        assert_eq!(reach_count(machine), reaches, "{name} service reaches");
        assert_eq!(
            contracts(machine).len(),
            contracts_count,
            "{name} contracts"
        );
    }

    // The same-line predicate form survives beside a following clause:
    // `ensures app.a settled` keeps its `app.a.settled()` call fact.
    let predicate_machine = machine("App::predicate_then_reaches");
    let [contract] = contracts(predicate_machine) else {
        panic!("predicate machine should carry one ensures contract");
    };
    assert!(matches!(
        contract.kind,
        syntax_trees::item::CapabilityContractKind::Ensures
    ));
    let [fact] = parsed.items.proof_facts(contract.facts) else {
        panic!("ensures contract should carry one fact");
    };
    let syntax_trees::item::ProofFact::Expression(expression) = fact else {
        panic!("predicate fact should be an expression");
    };
    assert!(matches!(
        parsed.expressions.expression(*expression),
        ExpressionNode::Call(_)
    ));

    let trait_definition = super::root_trait(&parsed, "trait item");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
    assert_eq!(
        parsed
            .items
            .identifier_path_members(signature.service_reaches)
            .len(),
        1
    );
    let [signature_contract] = parsed.items.capability_contracts(signature.contracts) else {
        panic!("requirement should carry one requires contract");
    };
    assert!(matches!(
        signature_contract.kind,
        syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert_eq!(parsed.items.proof_facts(signature_contract.facts).len(), 1);
}

#[test]
fn rejects_malformed_requires_fact_lists() {
    for source in [
        // A second stray name on the fact line is not a clause separator.
        "machine f(a: u32, b: u32) requires a <= b bogus stray { }",
        // `requires` followed by a separator authors no proposition.
        "machine f() requires; { }",
        // `requires` at end of input has no proposition to parse.
        "machine f() requires",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        assert!(
            parse_syntax_trees(&tokens).is_err(),
            "malformed requires should reject: {source}"
        );
    }
}

#[test]
fn parses_guarded_crash_buckets_on_machines_and_requirements() {
    let source = r#"
        machine divide(numerator: i32, denominator: i32)
        crashes Trap
            denominator == 0
            numerator == 0
        crashes Abort
        {
        }

        trait Fallible {
            machine run(flag: bool)
            crashes Abort
                flag;
        }
        "#;

    let parsed = super::parse_source(source, "crash buckets should parse");
    let machine = super::root_machine(&parsed, "machine item");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    let syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contracts[0].kind else {
        panic!("first contract should be a crash bucket");
    };
    assert_eq!(*cause, syntax_trees::item::CrashCause::Trap);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 2);

    let syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contracts[1].kind else {
        panic!("second contract should be a crash bucket");
    };
    assert_eq!(*cause, syntax_trees::item::CrashCause::Abort);
    assert!(parsed.items.proof_facts(contracts[1].facts).is_empty());

    let trait_definition = super::root_trait(&parsed, "trait item");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
    let [contract] = parsed.items.capability_contracts(signature.contracts) else {
        panic!("requirement should carry one crash bucket");
    };
    let syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contract.kind else {
        panic!("requirement contract should be a crash bucket");
    };
    assert_eq!(*cause, syntax_trees::item::CrashCause::Abort);
    assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
}

#[test]
fn parses_guarded_crash_bucket_on_operator_contract() {
    let source = r#"
        operator / divide(numerator: i32, denominator: i32) -> i32
        crashes Trap
            denominator == 0;
    "#;
    let parsed = super::parse_source(source, "operator crash bucket should parse");
    let operator = super::root_operator(&parsed, "operator item");
    let [contract] = parsed.items.capability_contracts(operator.contracts) else {
        panic!("operator should carry one crash bucket");
    };
    let syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contract.kind else {
        panic!("operator contract should be a crash bucket");
    };
    assert_eq!(*cause, syntax_trees::item::CrashCause::Trap);
    assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    let keyword = contract
        .keyword_source_span
        .expect("operator crash keyword span");
    assert_eq!(&source[keyword.span.start..keyword.span.end], "crashes");
}

#[test]
fn parses_fixed_operator_tokens_in_declaration_heads() {
    let source = r#"
        public operator + add(left: i32, right: i32) -> i32;
        boundary operator [] Slice::index(items: &[u8], index: u64) -> u8;
        boundary operator [..] Slice::range(items: &[u8], start: u64, end: u64) -> &[u8];
        operator named(left: i32, right: i32) -> i32;
    "#;
    let parsed = super::parse_source(source, "fixed-token heads should parse");
    let operators = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Operator(operator) => Some(operator),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(operators.len(), 4);
    assert!(operators[0].is_public);
    assert!(operators[1..].iter().all(|operator| !operator.is_public));
    assert_eq!(
        operators[0].spelling,
        Some(language_core::OperatorSpelling::Add)
    );
    assert_eq!(
        operators[1].spelling,
        Some(language_core::OperatorSpelling::Index)
    );
    assert_eq!(
        operators[2].spelling,
        Some(language_core::OperatorSpelling::Range)
    );
    assert_eq!(operators[3].spelling, None);
}

#[test]
fn rejects_retired_operator_spelling_clause() {
    let source = "operator add(left: i32, right: i32) -> i32 spelling +;";
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "retired clause must reject",
    );

    super::expect_error(&error, "expected `;`");
}

// OPERATOR-MACHINE-SUPPLY front: an optional fixed token immediately after
// `machine` binds operator syntax to an ordinary named declaration. The token
// is recorded on the syntax-tree `Machine::spelling`; downstream selection and
// ownership checks are the next frontier.
#[test]
fn parses_fixed_operator_tokens_on_machine_declarations() {
    let source = r#"
        public machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
            add_vectors(left, right)
        }
        machine [] Slice::index(items: Slice, index: u64) -> u8 {
            0u8
        }
        machine [..] Slice::range(items: Slice, start: u64, end: u64) -> Slice {
            items
        }
        boundary machine == Vec2::equal(left: Vec2, right: Vec2) -> bool;
        linux_x86_64 machine - subtract(left: u64, right: u64) -> u64 {
            left
        }
        machine ordinary(value: u64) -> u64 { value }
    "#;
    let parsed = super::parse_source(source, "machine token heads should parse");
    let machines = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(machines.len(), 6);
    assert_eq!(
        machines[0].spelling,
        Some(language_core::OperatorSpelling::Add)
    );
    assert!(machines[0].is_public);
    assert!(!machines[0].bodyless);
    assert_eq!(machines[0].name.as_str(&parsed), "Vec2::add");
    assert_eq!(
        machines[1].spelling,
        Some(language_core::OperatorSpelling::Index)
    );
    assert_eq!(
        machines[2].spelling,
        Some(language_core::OperatorSpelling::Range)
    );
    assert_eq!(
        machines[3].spelling,
        Some(language_core::OperatorSpelling::Equal)
    );
    assert!(machines[3].boundary && machines[3].bodyless);
    assert_eq!(
        machines[4].spelling,
        Some(language_core::OperatorSpelling::Subtract)
    );
    assert_eq!(
        machines[4]
            .target
            .as_ref()
            .map(|target| target.as_str(&parsed)),
        Some("linux_x86_64")
    );
    assert_eq!(machines[5].spelling, None);
}

#[test]
fn rejects_malformed_operator_tokens_on_machine_declarations() {
    for (source, expected) in [
        // Punctuation outside the closed token vocabulary refuses loudly.
        (
            "machine && add(left: i32, right: i32) -> i32 { left }",
            "unknown operator spelling `&&`",
        ),
        (
            "machine () add(left: i32) -> i32 { left }",
            "unknown operator spelling `()`",
        ),
        (
            "machine += add(left: i32, right: i32) -> i32 { left }",
            "unknown operator spelling `+=`",
        ),
        (
            "machine ! add(left: i32) -> i32 { left }",
            "unknown operator spelling `!`",
        ),
        // A second token where the declaration name belongs still rejects.
        (
            "machine + + add(left: i32, right: i32) -> i32 { left }",
            "expected identifier",
        ),
        // A `satisfies` machine is a realization; the satisfied requirement
        // owns the token binding and realizations never redeclare it.
        (
            "machine + add(left: i32, right: i32) -> i32 satisfies Math::add { left }",
            "cannot declare an operator token",
        ),
        (
            "machine + add(left: i32, right: i32) -> i32 satisfies Math::add via ForeignBinding::CompilerIntrinsic;",
            "cannot declare an operator token",
        ),
        // `boundary requirement` is the tokenless requirement form; a
        // token-bearing requirement is spelled bodyless `boundary machine +`.
        (
            "boundary requirement + add(left: i32, right: i32) -> i32;",
            "does not take a fixed operator token",
        ),
        // Conformance members supply implementations; they never introduce
        // token bindings.
        (
            "Evidence satisfies Shape { machine + area() -> u64 { 1u64 } }",
            "a conformance member supplies an implementation",
        ),
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "malformed token head must reject",
        );
        super::expect_error(&error, expected);
    }
}

#[test]
fn rejects_all_retired_invariant_declaration_forms_with_direction() {
    for (source, expected) in [
        (
            "invariant Positive(value: i32) { value > 0 }",
            "the `invariant` declaration is retired",
        ),
        (
            "trait Counter { invariant self.value > 0; }",
            "the `invariant` clause is retired",
        ),
        (
            "machine Counter { invariant valid { } }",
            "the `invariant` machine member is retired",
        ),
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "retired invariant must reject",
        );
        assert!(error.message.contains(expected), "got: {}", error.message);
    }
}

#[test]
fn rejects_retired_provider_item_and_operator_clause() {
    let tokens = Lexer::new("provider omega::host::WriteBytes : HostAbiCall;")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("provider items must reject");
    super::expect_error(&error, "expected one of");

    let tokens = Lexer::new(
        "boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 provider Slice;",
    )
    .tokenize()
    .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("operator provider clauses must reject");
    super::expect_error(&error, "expected `;`");
}

#[test]
fn rejects_unknown_or_multiple_fixed_operator_tokens() {
    for source in [
        "operator && both(left: bool, right: bool) -> bool;",
        "operator + - ambiguous(left: i32, right: i32) -> i32;",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        assert!(
            parse_syntax_trees(&tokens).is_err(),
            "unknown or multiple fixed tokens must reject: {source}"
        );
    }
}

#[test]
fn rejects_unknown_crash_causes() {
    let source = "machine fail() crashes Panic {}";
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "unknown crash cause must fail",
    );
    super::expect_error(
        &error,
        "unknown crash cause `Panic`; expected `Trap` or `Abort`",
    );
}

#[test]
fn parses_explicit_crash_terminal_and_retires_trap_statement() {
    let source = r#"
        machine fail()
        crashes Abort
        {
            crash Abort;
        }
    "#;
    let parsed = super::parse_source(source, "explicit crash should parse");
    let machine = super::root_machine(&parsed, "machine item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let [statement] = parsed.items.statements(state.statements) else {
        panic!("crash-only body should contain one statement");
    };
    let StatementNode::Transition(transition) = parsed.statements.statement(*statement) else {
        panic!("crash should lower to an explicit transition exit");
    };
    assert_eq!(
        transition.exit,
        syntax_trees::statement::TransitionExit::Crash(syntax_trees::item::CrashCause::Abort)
    );

    let tokens = Lexer::new("machine fail() { trap; }")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("retired trap statement must fail");
    assert!(error.message.contains("write `crash Trap;`"));
}

#[test]
fn rejects_duplicate_operational_clauses() {
    let source = "machine run() suspends; suspends; {}";
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "duplicate clause must fail",
    );
    super::expect_error(&error, "duplicate `suspends;`");
}

#[test]
fn rejects_legacy_effects_reach_clause() {
    let source = "machine run() effects Console {}";
    let error = super::parse_source_error(
        source,
        "tokenize should succeed",
        "legacy reach spelling must fail",
    );
    super::expect_error(&error, "`effects` reach clause is retired");
    assert!(error.message.contains("`reaches <Service> + ...`"));
}

#[test]
fn parses_installation_bound_reach_on_bodyless_boundary_requirement() {
    let source = r#"
        boundary trait InterruptCompletion {
            machine complete(acknowledgement: u64)
            reaches <= MachineControl + PortIo;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens)
        .expect("bounded installation reach should parse on a boundary requirement");
    let trait_definition = super::root_trait(&parsed, "boundary trait");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
    assert!(signature.service_reach_is_installation_bound);
    assert_eq!(signature.service_reaches.len(), 2);
}

#[test]
fn parses_explicit_top_level_boundary_requirement_and_retains_legacy_boundary_machine() {
    let source = r#"
        public boundary requirement InterruptAcknowledgement::complete<T>(acknowledgement: T) -> bool
        reaches <= MachineControl + PortIo
        requires acknowledgement == acknowledgement;
        boundary requirement Task::finish(task: u64);
        boundary machine Transitional::claim();
    "#;
    let parsed = super::parse_source(source, "top-level boundary requirements should parse");
    let machines = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(machines.len(), 3);

    let requirement = machines[0];
    assert_eq!(
        requirement.name.as_str(&parsed),
        "InterruptAcknowledgement::complete"
    );
    assert_eq!(
        requirement
            .attached_data
            .as_ref()
            .expect("qualified requirement owner")
            .as_str(&parsed),
        "InterruptAcknowledgement"
    );
    assert!(requirement.is_public);
    assert!(requirement.is_top_level_boundary_requirement);
    assert!(!requirement.boundary);
    assert!(requirement.bodyless);
    assert!(requirement.satisfies.is_empty());
    assert_eq!(requirement.type_parameters.len(), 1);
    assert!(requirement.service_reach_is_installation_bound);
    assert_eq!(requirement.service_reaches.len(), 2);
    assert_eq!(requirement.contracts.len(), 1);
    let entry = parsed
        .items
        .state(parsed.items.state_handles(requirement.states)[0]);
    assert_eq!(entry.parameters.len(), 1);
    assert!(entry.return_type.is_valid());

    let private_requirement = machines[1];
    assert!(!private_requirement.is_public);
    assert!(private_requirement.is_top_level_boundary_requirement);
    assert!(!private_requirement.boundary);
    assert!(private_requirement.bodyless);

    let transitional = machines[2];
    assert!(transitional.boundary);
    assert!(!transitional.is_top_level_boundary_requirement);
    assert!(transitional.bodyless);

    let snapshot = parsed
        .snapshot_json()
        .expect("boundary requirement snapshot");
    assert!(
        snapshot.contains("\"is_top_level_boundary_requirement\":true"),
        "{snapshot}"
    );
    assert!(snapshot.contains("\"is_public\":true"), "{snapshot}");
    assert!(snapshot.contains("MachineControl"), "{snapshot}");
    assert!(snapshot.contains("PortIo"), "{snapshot}");
}

#[test]
fn contract_terminated_top_level_requirement_stops_before_public_requirement() {
    let source = r#"
        public boundary requirement InterruptMaskGuard::restore(guard: InterruptMaskGuard)
        requires guard in InterruptMaskGuard::Active;

        public boundary requirement InterruptAcknowledgement::complete(acknowledgement: InterruptAcknowledgement);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize consecutive requirements");
    let parsed = parse_syntax_trees(&tokens)
        .expect("a contract-final semicolon must also terminate the first requirement");
    let requirements = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(requirements.len(), 2);
    assert_eq!(
        requirements[0].name.as_str(&parsed),
        "InterruptMaskGuard::restore"
    );
    assert_eq!(requirements[0].contracts.len(), 1);
    assert!(requirements[0].bodyless);
    assert!(requirements[0].is_top_level_boundary_requirement);
    assert_eq!(
        requirements[1].name.as_str(&parsed),
        "InterruptAcknowledgement::complete"
    );
    assert!(requirements[1].bodyless);
    assert!(requirements[1].is_top_level_boundary_requirement);
}

#[test]
fn operational_clause_semicolon_terminates_requirement_before_boundary_item() {
    let source = r#"
        public boundary requirement Task::finish<T>(task: Task<T>) -> T
        suspends; blocks;

        boundary trait TaskRuntime {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize operational requirement");
    let parsed = parse_syntax_trees(&tokens)
        .expect("the final operational semicolon must terminate the requirement");
    let requirement = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("top-level requirement");
    assert_eq!(requirement.name.as_str(&parsed), "Task::finish");
    assert!(requirement.bodyless);
    assert!(requirement.is_top_level_boundary_requirement);
    assert!(requirement.suspends);
    assert!(requirement.blocks);
    assert!(parsed.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Trait(definition)
            if definition.name.as_str(&parsed) == "TaskRuntime"
    )));
}

#[test]
fn rejects_invalid_explicit_top_level_boundary_requirement_forms() {
    for (source, expected) in [
        (
            "boundary requirement Package::operation() {}",
            "is bodyless and must end with `;`",
        ),
        (
            "boundary requirement Package::operation() satisfies Contract::operation;",
            "cannot itself carry a `satisfies` clause",
        ),
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize invalid boundary requirement",
            "invalid explicit top-level boundary requirement must reject",
        );
        assert!(error.message.contains(expected), "got: {}", error.message);
    }

    let tokens = Lexer::new("trait Nested { boundary requirement Package::operation(); }")
        .tokenize()
        .expect("tokenize nested boundary requirement");
    parse_syntax_trees(&tokens)
        .expect_err("an explicit boundary requirement must remain top-level");
}

#[test]
fn rejects_top_level_installation_bound_reach_outside_fresh_boundary_requirement() {
    for source in [
        "machine complete() reaches <= MachineControl {}",
        "boundary machine complete() reaches <= MachineControl {}",
        "x86_64 machine complete() reaches <= MachineControl {}",
        "boundary machine complete() satisfies Completion::complete reaches <= MachineControl;",
        "Named satisfies Trait { machine complete() reaches <= MachineControl {} }",
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "installation-bound reach must stay on a fresh boundary requirement",
        );
        super::expect_error(&error, "`reaches <= Bound`");
    }
}

#[test]
fn rejects_installation_bound_reach_outside_bodyless_boundary_requirement() {
    for source in [
        "trait Completion { machine complete() reaches <= MachineControl; }",
        "boundary trait Completion { machine complete() reaches <= MachineControl {} }",
        "machine run<machine Op>() where machine Op() reaches <= MachineControl; {}",
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "installation-bound reach must stay on a bodyless boundary requirement",
        );
        super::expect_error(&error, "`reaches <= Bound`");
    }
}

#[test]
fn rejects_empty_or_mixed_installation_bound_reach_rows() {
    for source in [
        "boundary trait Completion { machine complete() reaches <=; }",
        "boundary trait Completion { machine complete() reaches MachineControl reaches <= PortIo; }",
        "boundary trait Completion { machine complete() reaches <= MachineControl reaches PortIo; }",
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "empty or mixed installation-bound reach must reject",
        );
        super::expect_error(&error, "installation-bound reach");
    }
}

#[test]
fn rejects_operational_members_in_service_reach_rows() {
    for (retired, replacement) in [
        ("Suspend", "suspends;"),
        ("Block", "blocks;"),
        ("thread_block", "blocks;"),
    ] {
        let source = format!("machine run() reaches Console, {retired} {{}}");
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "retired effect member must fail",
        );
        assert!(error.message.contains("boundary-service identities only"));
        super::expect_error(&error, replacement);
    }
}

#[test]
fn parses_machine_contract_clauses() {
    let source = r#"
        machine distinct_indices(i: usize, j: usize)
        requires
            i < j
        ensures
            i != j
        {
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let contracts = parsed.items.capability_contracts(machine.contracts);

    assert_eq!(contracts.len(), 2);
    assert!(matches!(
        contracts[0].kind,
        syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert!(matches!(
        contracts[1].kind,
        syntax_trees::item::CapabilityContractKind::Ensures
    ));
    assert!(contracts[0].token_count > 0);
    assert!(contracts[1].token_count > 0);
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("authored contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
}

#[test]
fn parses_named_machine_contract_evidence_bindings() {
    let source = r#"
        proposition carries(value: i32) evidence i32;
        machine forward(value: i32)
        requires input_proof: carries(value)
        ensures output_proof: carries(value)
        {
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "named contracts parse");
    let machine = super::root_machine(&parsed, "machine");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0]
            .binding
            .as_ref()
            .map(|name| name.as_str(&parsed)),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1]
            .binding
            .as_ref()
            .map(|name| name.as_str(&parsed)),
        Some("output_proof")
    );
    assert!(
        contracts
            .iter()
            .all(|contract| parsed.items.proof_facts(contract.facts).len() == 1)
    );
}

#[test]
fn parses_outcome_specific_ensures_as_rows_without_group_identity() {
    let source = r#"
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        ensures Outcome::Success -> {
            true;
            selected: true;
        }
        { Outcome::Success }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "outcome-specific ensures parse");
    let machine = super::root_machine(&parsed, "machine");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    for contract in contracts {
        let syntax_trees::item::CapabilityContractKind::EnsuresForResultCase { result_case } =
            &contract.kind
        else {
            panic!("guarded guarantee row")
        };
        let names = parsed.items.identifier_path_members(*result_case);
        assert_eq!(names[0].as_str(&parsed), "Outcome");
        assert_eq!(names[1].as_str(&parsed), "Success");
        assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    }
    assert!(contracts[0].binding.is_none());
    assert_eq!(
        contracts[1]
            .binding
            .as_ref()
            .map(|name| name.as_str(&parsed)),
        Some("selected")
    );
}

#[test]
fn rejects_ambiguous_and_duplicate_outcome_specific_ensures_surfaces() {
    for (source, expected) in [
        (
            "machine choose() ensures result == Outcome::Success -> { true; } -> Outcome {}",
            "rejects Boolean guards",
        ),
        (
            "machine choose() ensures Outcome::Success() -> { true; } -> Outcome {}",
            "rejects case-literal-shaped selectors",
        ),
        (
            "machine choose() -> Outcome ensures Outcome::Success -> { true; } ensures Outcome::Success -> { false; } {}",
            "duplicate outcome-specific ensures group",
        ),
        (
            "machine choose() -> Outcome ensures selected: true; ensures Outcome::Success -> { selected: true; } {}",
            "duplicate machine-wide public ensures selector",
        ),
    ] {
        let error = super::parse_source_error(
            source,
            "tokenize invalid group",
            "invalid group must reject",
        );
        super::expect_error(&error, expected);
    }
}

#[test]
fn rejects_named_contract_with_multiple_propositions() {
    let source = r#"
        proposition carries(value: i32);
        machine invalid(value: i32)
        requires proof: carries(value); carries(value)
        {
        }
    "#;
    let error = super::parse_source_error(
        source,
        "tokenize",
        "one evidence term cannot bind two facts",
    );
    super::expect_error(&error, "exactly one proposition");
}

#[test]
fn parses_named_bodyless_signature_contract_bindings() {
    let source = r#"
        trait Evidence { machine witness(); }
        proposition ready() evidence Evidence;
        trait Worker {
            machine relay()
            requires input_proof: ready()
            ensures output_proof: ready();
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "named signature contracts parse");
    let worker = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .find(|definition| definition.name.as_str(&parsed) == "Worker")
        .expect("worker trait");
    let [requirement] = parsed.items.state_signatures(worker.machines) else {
        panic!("one worker requirement")
    };
    let contracts = parsed
        .items
        .capability_contracts(parsed.items.state_signature(*requirement).contracts);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0]
            .binding
            .as_ref()
            .map(|binding| binding.as_str(&parsed)),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1]
            .binding
            .as_ref()
            .map(|binding| binding.as_str(&parsed)),
        Some("output_proof")
    );
}

#[test]
fn parses_nested_machine_requires() {
    let source = r#"
        machine walk(value: i32) {
            transition value > 0 {
                true -> positive(value)
                false -> done()
            }

            machine positive(value: i32)
            requires
                value > 0
            {
            }

            machine done() {
            }
        }
    "#;

    let parsed = super::parse_source(source, "nested machine requires should parse");
    let machine = super::root_machine(&parsed, "machine root item");
    let positive = machine
        .nested_machines
        .handles()
        .find_map(|handle| match parsed.items.item(handle) {
            syntax_trees::item::Item::Machine(child)
                if child.name.as_str(&parsed) == "positive" =>
            {
                Some(child)
            }
            _ => None,
        })
        .expect("positive machine");
    let contracts = parsed.items.capability_contracts(positive.contracts);
    assert_eq!(contracts.len(), 1);
    assert!(matches!(
        contracts[0].kind,
        syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
}

#[test]
fn parses_unguarded_crash_bucket_on_a_bodyless_machine_head() {
    // A fact-free whole-cause route terminates at the bodyless head's `;`,
    // matching what the `operator` head accepts; the next item still parses.
    let source = r#"
        boundary machine == Float::equal(left: f32, right: f32) -> bool crashes Trap;

        machine compare(left: f32, right: f32) -> bool crashes Abort {
            left == right
        }
    "#;
    let parsed = super::parse_source(source, "bodyless crash bucket should parse");
    let machines = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [equal, compare] = machines.as_slice() else {
        panic!("two machine items");
    };
    assert!(equal.bodyless && equal.boundary);
    assert_eq!(
        equal.spelling,
        Some(syntax_trees::operator_spelling::OperatorSpelling::Equal)
    );
    let [contract] = parsed.items.capability_contracts(equal.contracts) else {
        panic!("the bodyless head carries one crash bucket");
    };
    let syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contract.kind else {
        panic!("crash bucket");
    };
    assert_eq!(*cause, syntax_trees::item::CrashCause::Trap);
    assert!(parsed.items.proof_facts(contract.facts).is_empty());
    assert!(!compare.bodyless);
}

#[test]
fn bare_bodyless_machine_signature_parses_and_bodyless_satisfies_without_via_rejects() {
    // The grammar admits a bare bodyless signature; whether a source may own
    // it as a compiler-catalog primitive is symbol resolution's decision.
    let tokens = Lexer::new("machine Float::meaning32(value: f32) -> FloatMeaning;")
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("bare bodyless signature should parse");
    let machine = super::root_machine(&parsed, "machine item");
    assert!(machine.bodyless && !machine.boundary && machine.spelling.is_none());
    assert_eq!(machine.name.as_str(&parsed), "Float::meaning32");

    let tokens = Lexer::new("machine realize(value: f32) -> f32 satisfies Math::identity;")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("a bodyless realization needs `via`");
    super::expect_error(
        &error,
        "a machine without a body is the ACCEPTED boundary form",
    );
}
