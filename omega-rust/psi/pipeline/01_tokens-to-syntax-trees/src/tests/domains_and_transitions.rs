use crate::parser::parse_syntax_trees;
use language_core::ReferenceAccess;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementNode;
use syntax_trees::types::TypeReferenceNode;

#[test]
fn rejects_exit_contract_clauses_on_explicit_states() {
    let source = r#"
        machine walk() {
            state done()
            ensures
                true
            {
            }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("states admit arrival requires rather than exit contracts");
    assert!(
        error
            .message
            .contains("state signatures admit only arrival `requires`")
    );
}

#[test]
fn parses_machine_termination_clauses() {
    let source = r#"
        machine walk(items: &[Item], remaining: usize)
        terminates by remaining -> Nat::Descending;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
            .len(),
        2
    );
}

#[test]
fn parses_machine_termination_tuple_subjects() {
    // The argumented ranking-view spelling: the arrow's left side is the
    // ranked-subject tuple, bound in order to the named view's parameters.
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by (index, limit) -> Nat::BoundedDistance;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
            .len(),
        2
    );
}

#[test]
fn parses_machine_termination_argumented_view() {
    // TPR3: an ARGUMENTED view names its bound as an argument
    // (`Nat::IncreasingTo(limit)`) -- the bound is part of the view; the
    // subject stays alone on the arrow's left.
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by index -> Nat::IncreasingTo(limit);
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    // The by-form supplies only the witness -- never the public guarantee.
    assert!(!machine.terminates_guarantee);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_view_arguments)
            .len(),
        1
    );
}

#[test]
fn parses_trait_requirement_termination_guarantee() {
    // TPR4 (decision 23): a bodyless requirement authors the PUBLIC
    // guarantee with bare `terminates` -- previously the signature clause
    // parser's skip-any-token fallback ATE it silently.
    let source = r#"
        trait Worker {
            machine run(&mut self, n: u64) -> u64 terminates;
            machine peek(&self) -> u64;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait root item");

    let signatures: Vec<_> = parsed
        .items
        .state_signatures(trait_definition.machines)
        .iter()
        .map(|handle| parsed.items.state_signature(*handle))
        .collect();
    assert_eq!(signatures.len(), 2);
    assert!(
        signatures[0].terminates_guarantee,
        "run authored `terminates`"
    );
    assert!(!signatures[1].terminates_guarantee, "peek promised nothing");
}

#[test]
fn rejects_ranking_witness_on_trait_requirement() {
    // The witness belongs to implementations: a bodyless requirement has no
    // body to prove.
    let source = r#"
        trait Worker {
            machine run(&mut self, n: u64) -> u64 terminates by n;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("the witness must be rejected");
    assert!(
        error
            .message
            .contains("does not belong on a bodyless requirement"),
        "got: {}",
        error.message
    );
}

#[test]
fn parses_data_default_domain_where_clause() {
    // R2 rung 1 (ch12 "Dependent Data"): the where clause between the data
    // signature and the body -- bare field names, comma-separated facts,
    // trailing comma tolerated.
    let source = r#"
        data MemoryMap
        where
            count <= len,
            stride >= 40,
        {
            len: u32;
            stride: u32;
            count: u32;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");

    assert_eq!(parsed.items.proof_facts(data.where_facts).len(), 2);
    assert_eq!(parsed.items.data_members(data.members).len(), 3);
}

#[test]
fn parses_case_local_where_clause() {
    // CASE-CONSTRAINTS (ch12 active-case indexed constraint): the `where`
    // clause sits after the payload parens and before the case's `;`; a case
    // without it has an empty fact span.
    let source = r#"
        data Interval {
            case Empty;
            case Range(lo: u64, hi: u64) where lo <= hi;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");

    let mut fact_counts =
        parsed
            .items
            .data_members(data.members)
            .iter()
            .map(|member| match member {
                syntax_trees::item::DataMember::Variant(variant) => {
                    parsed.items.proof_facts(variant.where_facts).len()
                }
                _ => panic!("expected only variant members"),
            });
    assert_eq!(
        fact_counts.next(),
        Some(0),
        "payload-less case without `where`"
    );
    assert_eq!(fact_counts.next(), Some(1), "case `where lo <= hi`");
    assert!(fact_counts.next().is_none());
}

#[test]
fn parses_value_and_policy_domain_chain_as_one_constrained_type() {
    let source = r#"
        data Sample {
            value: f32 in Finite & Saturating;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let field = parsed
        .items
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("data field");
    let TypeReferenceNode::Constrained { constraints, .. } =
        parsed.type_references.type_reference(field.type_reference)
    else {
        panic!("domain chain should produce one constrained type");
    };
    let constraints = parsed.type_references.constraints(*constraints);
    assert!(matches!(
        constraints,
        [
            syntax_trees::types::TypeConstraintNode::Domain(name),
            syntax_trees::types::TypeConstraintNode::ArithmeticDomain(
                numerics::arithmetic::ArithmeticDomain::Saturating
            )
        ] if name.name.as_str() == "Finite"
    ));
}

#[test]
fn preserves_open_index_operator_expression_in_domain_argument() {
    let source = r#"
        data Unit {}
        domain<T, const U: Unit> T::Quantity<U>;
        data Rate<const A: Unit, const B: Unit> {
            value: f64 in Quantity<A / B>;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("open index expression should parse");
    let rate = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Rate" => Some(data),
            _ => None,
        })
        .expect("Rate data");
    let field = parsed
        .items
        .data_members(rate.members)
        .iter()
        .find_map(|member| match member {
            syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("Rate::value field");
    let TypeReferenceNode::Constrained { constraints, .. } =
        parsed.type_references.type_reference(field.type_reference)
    else {
        panic!("quantity should be a constrained carrier");
    };
    let [syntax_trees::types::TypeConstraintNode::Domain(domain)] =
        parsed.type_references.constraints(*constraints)
    else {
        panic!("quantity domain constraint");
    };
    let [argument] = parsed
        .type_references
        .type_reference_handles(domain.arguments)
    else {
        panic!("one quantity index");
    };
    let TypeReferenceNode::ConstExpression(expression) =
        parsed.type_references.type_reference(*argument)
    else {
        panic!("A / B should remain an open const expression");
    };
    let ExpressionNode::Binary(binary) = parsed.expressions.expression(*expression) else {
        panic!("open index should retain the divide expression");
    };
    assert_eq!(
        binary.operator,
        syntax_trees::expression::BinaryOperator::Divide
    );
    assert_eq!(parsed.expressions.display_name(binary.left), "A");
    assert_eq!(parsed.expressions.display_name(binary.right), "B");
}

#[test]
fn parses_compiler_owned_carry_atoms_and_expands_portable() {
    let source = r#"
        data Sample {
            local: u64 in Carry::MovableAddress;
            portable: u64 in Carry::Portable;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let fields = parsed
        .items
        .data_members(data.members)
        .iter()
        .filter_map(|member| match member {
            syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();

    let names = |field: &syntax_trees::item::DataField| {
        let TypeReferenceNode::Constrained { constraints, .. } =
            parsed.type_references.type_reference(field.type_reference)
        else {
            panic!("carry permission should produce a constrained type");
        };
        parsed
            .type_references
            .constraints(*constraints)
            .iter()
            .map(|constraint| match constraint {
                syntax_trees::types::TypeConstraintNode::Domain(name) => {
                    name.name.as_str().to_owned()
                }
                other => panic!("carry permission became {other:?}"),
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(names(fields[0]), ["Carry::MovableAddress"]);
    assert_eq!(
        names(fields[1]),
        language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
    );
}

#[test]
fn expands_carry_portable_contract_guarantee_to_four_atomic_facts() {
    let source = r#"
        machine grant(value: u64) -> u64
        ensures
            result in Carry::Portable;
        {
            transition { _ -> value }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let [contract] = parsed.items.capability_contracts(machine.contracts) else {
        panic!("one ensures contract");
    };
    let domains = parsed
        .items
        .proof_facts(contract.facts)
        .iter()
        .map(|fact| match fact {
            syntax_trees::item::ProofFact::Membership(membership) => parsed
                .items
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
            other => panic!("portable atom became {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        domains,
        language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
    );
}

#[test]
fn rejects_bare_arrow_transition_in_explicit_state_body() {
    let source = r#"
        machine Main::main(&mut self) {
            transition { _ -> running() }

            state running(&mut self) {
                -> finished();
            }

            state finished(&mut self) {
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("parse should reject bare arrows in explicit state bodies");
    assert!(
        error
            .message
            .contains("explicit state bodies must use the `transition` keyword"),
        "unexpected parse error: {}",
        error.message
    );
}

#[test]
fn parses_slice_range_indexing_into_range_expression() {
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
    let parsed = parse_syntax_trees(&tokens).expect("parse should accept slice range surface");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state_handle = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(state_handle);
    let statement_handle = parsed
        .items
        .statements(state.statements)
        .get(2)
        .copied()
        .expect("tail local");
    let statement = parsed.statements.statement(statement_handle);
    let StatementNode::LocalData(local) = statement else {
        panic!("expected local data statement");
    };
    let ExpressionNode::Indexed(indexed) = parsed.expressions.expression(local.initial_value)
    else {
        panic!("expected indexed initializer");
    };
    let ExpressionNode::Range(range) = parsed.expressions.expression(indexed.index) else {
        panic!("expected range index expression");
    };
    assert_eq!(
        parsed.expressions.display_name(indexed.index),
        "1..",
        "unexpected range display"
    );
    assert!(range.start.is_valid(), "expected explicit range start");
    assert!(!range.end.is_valid(), "expected open-ended range");
    let operator_span = parsed.expressions.source_span(local.initial_value).span;
    assert_eq!(
        &source[operator_span.start..operator_span.end],
        "[",
        "indexed syntax must retain its authored operator token"
    );
}

#[test]
fn parses_structural_recast_targets_as_type_references() {
    let source = r#"
        machine inspect(bytes: [u8; 4]) {
            let fixed: &[u8; 4] = &bytes as &[u8; 4];
            let slice: &[u8] = &bytes as &[u8];
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("array and slice recast targets should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let locals = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();

    let ExpressionNode::Borrow(fixed_borrow) =
        parsed.expressions.expression(locals[0].initial_value)
    else {
        panic!("fixed-array initializer should retain its shared borrow");
    };
    assert_eq!(fixed_borrow.access, ReferenceAccess::Shared);
    let ExpressionNode::Cast(fixed) = parsed.expressions.expression(fixed_borrow.target) else {
        panic!("fixed-array borrow target should be a recast");
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length: syntax_trees::types::FixedArrayLength::Literal(4),
    } = parsed.type_references.type_reference(fixed.target_type)
    else {
        panic!("fixed-array target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u8"
    ));

    let ExpressionNode::Borrow(slice_borrow) =
        parsed.expressions.expression(locals[1].initial_value)
    else {
        panic!("slice initializer should retain its shared borrow");
    };
    assert_eq!(slice_borrow.access, ReferenceAccess::Shared);
    let ExpressionNode::Cast(slice) = parsed.expressions.expression(slice_borrow.target) else {
        panic!("slice borrow target should be a recast");
    };
    let TypeReferenceNode::Slice { element_type } =
        parsed.type_references.type_reference(slice.target_type)
    else {
        panic!("slice target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u8"
    ));
}

#[test]
fn parses_trait_machine_contract_clauses() {
    let source = r#"
        boundary trait Filesystem {
            machine open(path: String)
            requires
                path in String::NonEmpty
            ensures
                handle in FileHandle::Open
            reaches
                Filesystem;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .expect("trait root item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let contracts = parsed.items.capability_contracts(signature.contracts);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);

    assert_eq!(contracts.len(), 2);
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("trait contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Filesystem");
}

#[test]
fn parses_executable_domain_membership_expression() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_data_destructure_transition_guard_as_subject_member_guard() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) {
            transition self.player {
                Player::Alive -> done()
                Player { health, .. } if health > 5 -> done()
                _ -> done()
            }

            state done() {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    // The destructure arm ALSO mints an exhaustiveness-marker let
    // (`__arm_destructure#...`) ahead of the transition statements; index
    // among the TRANSITIONS only.
    let statement = parsed
        .items
        .statements(state.statements)
        .iter()
        .copied()
        .filter(|handle| {
            matches!(
                parsed.statements.statement(*handle),
                StatementNode::Transition(_)
            )
        })
        .nth(1)
        .expect("data-pattern transition");
    let StatementNode::Transition(transition) = parsed.statements.statement(statement) else {
        panic!("second arm should be a transition")
    };
    let syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("data-pattern arm should lower to a guard expression");
    };
    let ExpressionNode::Binary(comparison) = parsed.expressions.expression(guard) else {
        panic!("data-pattern guard should be a comparison");
    };
    assert!(matches!(
        parsed.expressions.expression(comparison.left),
        ExpressionNode::Member(_)
    ));
}

#[test]
fn parses_outcome_arm_payload_and_erased_proof_selectors() {
    let source = r#"
        data Outcome { case Success(value: i32); case Failure; }
        machine inspect(result: Outcome) -> i32 {
            transition result {
                Outcome::Success { value; selected: local, shorthand } -> value
                Outcome::Failure { } -> 0
            }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize outcome arm");
    let parsed = parse_syntax_trees(&tokens).expect("outcome selector arm should parse");
    let transition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "inspect" => {
                Some(machine)
            }
            _ => None,
        })
        .into_iter()
        .flat_map(|machine| parsed.items.state_handles(machine.states))
        .flat_map(|state| {
            parsed
                .items
                .statements(parsed.items.state(*state).statements)
        })
        .find_map(|statement| match parsed.statements.statement(*statement) {
            StatementNode::Transition(transition) if !transition.proof_selectors.is_empty() => {
                Some(transition)
            }
            _ => None,
        })
        .expect("one transition arm with erased selectors");
    let selectors = parsed
        .statements
        .outcome_proof_selectors(transition.proof_selectors);
    assert_eq!(selectors.len(), 2);
    assert_eq!(selectors[0].output_field.as_str(), "selected");
    assert_eq!(selectors[0].binding.as_str(), "local");
    assert_eq!(selectors[1].output_field.as_str(), "shorthand");
    assert_eq!(selectors[1].binding.as_str(), "shorthand");
}

#[test]
fn outcome_proof_selectors_do_not_change_the_runtime_statement_plan() {
    let sources = [
        r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; selected: local } -> {}
                Outcome::Failure { } -> {}
            }
        }
        "#,
        r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; } -> {}
                Outcome::Failure { } -> {}
            }
        }
        "#,
    ];
    let mut plans = Vec::new();
    for source in sources {
        let tokens = Lexer::new(source).tokenize().expect("tokenize outcome arm");
        let parsed = parse_syntax_trees(&tokens).expect("outcome arm should parse");
        let inspect = parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "inspect" =>
                {
                    Some(machine)
                }
                _ => None,
            })
            .expect("inspect machine");
        let entry = parsed
            .items
            .state_handles(inspect.states)
            .first()
            .copied()
            .expect("inspect entry state");
        let statements = parsed
            .items
            .statements(parsed.items.state(entry).statements);
        let producer_locals = statements
            .iter()
            .filter(|statement| {
                let StatementNode::LocalData(local) = parsed.statements.statement(**statement)
                else {
                    return false;
                };
                matches!(
                    parsed.expressions.expression(local.initial_value),
                    ExpressionNode::Call(call) if call.target.as_str() == "produce"
                )
            })
            .count();
        let transitions = statements
            .iter()
            .filter(|statement| {
                matches!(
                    parsed.statements.statement(**statement),
                    StatementNode::Transition(_)
                )
            })
            .count();
        let producer_calls = parsed
            .expressions
            .iter_expressions()
            .filter(|(_, expression)| {
                matches!(
                    expression,
                    ExpressionNode::Call(call) if call.target.as_str() == "produce"
                )
            })
            .count();
        assert_eq!(producer_locals, 1, "the captured producer call is retained");
        assert_eq!(
            producer_calls, 1,
            "the producer call is evaluated exactly once"
        );
        plans.push((statements.len(), producer_locals, transitions));
    }
    assert_eq!(
        plans[0], plans[1],
        "erased proof selectors must not add or remove runtime statements"
    );
}

#[test]
fn rejects_duplicate_and_noncase_outcome_proof_selectors() {
    for (source, expected) in [
        (
            "machine inspect(result: Outcome) { transition result { Outcome::Success { ; selected: local, selected: other } -> {} } }",
            "outcome proof selector `selected` is bound more than once",
        ),
        (
            "machine inspect(result: Record) { transition result { Record { ; selected: local } -> {} } }",
            "outcome proof selectors require an exact",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize rejected selector arm");
        let error = parse_syntax_trees(&tokens).expect_err("invalid selector arm must reject");
        assert!(error.message.contains(expected), "got: {}", error.message);
    }
}

#[test]
fn parses_record_field_value_pattern_as_subject_member_equality() {
    let source = r#"
        data Header [copy] { ok: i32; version: i32; }
        machine inspect(h: Header) -> i32 {
            transition h {
                Header { ok: 0, version } -> version
                _ -> 1
            }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("field-value pattern should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .map(|handle| parsed.items.state(handle))
        .expect("entry state");
    let transition = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            syntax_trees::statement::StatementNode::Transition(transition) => Some(transition),
            _ => None,
        })
        .next()
        .expect("first transition arm");
    let syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("field-value arm must have an equality guard");
    };
    let syntax_trees::expression::ExpressionNode::Binary(equality) =
        parsed.expressions.expression(guard)
    else {
        panic!("field-value pattern should lower to binary equality");
    };
    assert_eq!(
        equality.operator,
        syntax_trees::expression::BinaryOperator::Equal
    );
    assert!(matches!(
        parsed.expressions.expression(equality.left),
        syntax_trees::expression::ExpressionNode::Member(_)
    ));
}

#[test]
fn parses_asm_jmp_block_as_transition_statement() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            asm {
                jmp other()
            }

            state other() {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("asm transition");
    assert!(matches!(
        parsed.statements.statement(statement),
        StatementNode::Transition(_)
    ));
}

/// The asm mnemonic desugar: each known-contract instruction lowers to a call
/// on its unnameable `asm#...` intrinsic (`in` through an assignment); unknown
/// mnemonics -- including `db` -- are rejected at parse time.
#[test]
fn parses_asm_mnemonics_as_intrinsic_calls() {
    let source = r#"
        data Main {
            port: u16;
        }

        machine Main::main(&mut self) {
            let mut status: u8 = 0;
            asm { hlt }
            asm { out self.port, status }
            asm { in status, self.port }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statements = parsed.items.statements(state.statements).to_vec();
    assert_eq!(statements.len(), 4, "let + three asm statements");

    let StatementNode::Call(hlt) = parsed.statements.statement(statements[1]).clone() else {
        panic!("asm {{ hlt }} should desugar to a call statement");
    };
    assert_eq!(hlt.target.as_str(), "asm#hlt");
    assert!(hlt.receiver.is_empty());
    assert_eq!(hlt.arguments.count(), 0);

    let StatementNode::Call(out) = parsed.statements.statement(statements[2]).clone() else {
        panic!("asm {{ out .. }} should desugar to a call statement");
    };
    assert_eq!(out.target.as_str(), "asm#port_out");
    assert_eq!(out.arguments.count(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[3]).clone() else {
        panic!("asm {{ in .. }} should desugar to an assignment");
    };
    let ExpressionNode::Call(port_in) = parsed.expressions.expression(read.value).clone() else {
        panic!("asm {{ in .. }} assignment value should be the intrinsic call");
    };
    assert_eq!(port_in.target.as_str(), "asm#port_in");
    assert_eq!(port_in.arguments.count(), 1);
    assert!(!port_in.receiver.is_valid());
}
