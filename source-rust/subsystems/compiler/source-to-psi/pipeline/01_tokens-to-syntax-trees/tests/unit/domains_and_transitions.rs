use language_core::ReferenceAccess;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceNode;

fn assert_expression_count(
    trees: &syntax_trees::SyntaxTrees,
    handles: arena::HandleSpan<syntax_trees::expression::ExpressionHandle>,
    expected: usize,
) {
    assert_eq!(
        trees.expressions.expression_handles(handles).len(),
        expected
    );
}

fn assert_path_member_count(
    trees: &syntax_trees::SyntaxTrees,
    path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    expected: usize,
) {
    assert_eq!(trees.items.identifier_path_members(path).len(), expected);
}

#[test]
fn logical_domain_subject_preserves_ordinary_machine_operands() {
    let source = r#"
        data Player { health: u64; }
        domain Player::Alive requires self.health > 0;
        machine health(player: Player) -> u64 { player.health }
    "#;
    let parsed = crate::tests::parse_source(source);
    let mut copied = syntax_trees::SyntaxTrees::default();
    copied.extend_from(&parsed);
    for trees in [&parsed, &copied] {
        crate::tests::root_domain(trees, "domain");
        for name in ["self", "player"] {
            assert_eq!(trees.expressions.iter_expressions().filter(|(_, node)| {
                let ExpressionNode::Name { members: path, application: None } = node else { return false; };
                matches!(trees.expressions.identifier_path_members(path.to_owned()), [member] if member.as_str(trees) == name)
            }).count(), 1, "logical and ordinary subjects remain separate through copying");
        }
    }
}

#[test]
fn logical_domain_subject_preserves_generic_references() {
    let parsed = crate::tests::parse_source(
        "domain<const Limit: u64> u64::AtMost<Limit> requires self <= Limit;",
    );
    for name in ["self", "Limit"] {
        assert!(parsed.expressions.iter_expressions().any(|(_, node)| {
            let ExpressionNode::Name { members: path, application: None } = node else { return false; };
            matches!(parsed.expressions.identifier_path_members(path.to_owned()), [member] if member.as_str(&parsed) == name)
        }));
    }
}

#[test]
fn rejects_every_authored_domain_subject_shape() {
    for source in [
        "domain u64::Positive(value) requires value > 0;",
        "domain u64::Positive(self) requires self > 0;",
        "domain u64::Positive(value: u64) requires value > 0;",
        "domain u64::Positive(left, right) requires left > right;",
        "domain<const Limit: u64> u64::AtMost<Limit>(Limit) requires Limit > 0;",
        "domain u64::Tag(unused);",
    ] {
        crate::tests::parse_error(source);
    }
}

#[test]
fn accepts_exit_contract_clauses_on_nested_machines() {
    let source = r#"
        machine walk() {
            machine done()
            ensures
                true
            {
            }
        }
    "#;

    crate::tests::parse_source(source);
}

#[test]
fn parses_machine_termination_clauses() {
    let source = r#"
        machine walk(items: @[Item], remaining: usize)
        terminates by remaining -> Nat::Descending
        {
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");

    assert_expression_count(&parsed, machine.ranking_subjects, 1);
    assert_path_member_count(&parsed, machine.ranking_view, 2);
}

#[test]
fn parses_machine_termination_tuple_subjects() {
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by (index, limit) -> Nat::BoundedDistance
        {
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");

    assert_expression_count(&parsed, machine.ranking_subjects, 2);
    assert_path_member_count(&parsed, machine.ranking_view, 2);
}

#[test]
fn parses_machine_termination_argumented_view() {
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by index -> Nat::IncreasingTo(limit)
        {
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");

    assert!(!machine.terminates_guarantee);
    assert_expression_count(&parsed, machine.ranking_subjects, 1);
    assert_path_member_count(&parsed, machine.ranking_view, 2);
    assert_expression_count(&parsed, machine.ranking_view_arguments, 1);
}

#[test]
fn parses_trait_requirement_termination_guarantee() {
    let source = r#"
        trait Worker<Work> {
            machine run(value: @mutable Work, n: u64) -> u64 terminates;
            machine peek(value: @Work) -> u64;
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let trait_definition = crate::tests::root_trait(&parsed, "trait root item");

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
    let source = r#"
        trait Worker<Work> {
            machine run(value: @mutable Work, n: u64) -> u64 terminates by n;
        }
        "#;

    let error = crate::tests::parse_error(source);
    crate::tests::expect_error(&error, "does not belong on a bodyless requirement");
}

#[test]
fn parses_data_default_domain_requires_clause() {
    let retired =
        crate::tests::parse_error("data MemoryMap where count <= len { len: u32; count: u32; }");
    crate::tests::expect_error(&retired, "expected `{`, found identifier `where`");
    let source = r#"
        data MemoryMap
        requires
            count <= len,
            stride >= 40,
        {
            len: u32;
            stride: u32;
            count: u32;
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let data = crate::tests::root_data(&parsed, "data root item");

    assert_eq!(parsed.items.proof_facts(data.requires_facts).len(), 2);
    assert_eq!(parsed.items.data_members(data.members).len(), 3);
}

#[test]
fn retains_complete_default_domain_predicate() {
    let source = r#"
        data Interval
            requires (self in Interval::Empty)
               || ((self in Interval::Range) && self.lo <= self.hi)
        {
            case Empty;
            case Range(lo: u64, hi: u64);
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let data = crate::tests::root_data(&parsed, "data root item");

    let mut fact_counts =
        parsed
            .items
            .data_members(data.members)
            .iter()
            .map(|member| match member {
                DataMember::Variant(variant) => {
                    parsed.items.proof_facts(variant.requires_facts).len()
                }
                _ => panic!("expected only variant members"),
            });
    assert_eq!(
        fact_counts.next(),
        Some(0),
        "payload-less case without facts"
    );
    assert_eq!(fact_counts.next(), Some(0), "no case-local predicate copy");
    assert!(fact_counts.next().is_none());
}

#[test]
fn rejects_case_local_requires_clause() {
    let error = crate::tests::parse_error(
        "data Interval { case Range(lo: u64, hi: u64) requires lo <= hi; }",
    );
    assert!(
        error
            .message
            .contains("case-local predicate clauses are retired"),
        "got: {}",
        error.message
    );
}

#[test]
fn parses_value_and_policy_domain_chain_as_one_constrained_type() {
    let source = r#"
        data Sample {
            value: f32 in Finite & Saturating;
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let data = crate::tests::root_data(&parsed, "data root item");
    let field = parsed
        .items
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) => Some(field),
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
        [TypeConstraintNode::Domain(value), TypeConstraintNode::Domain(policy)]
            if value.name.as_str(&parsed) == "Finite"
                && policy.name.as_str(&parsed) == "Saturating"
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

    let parsed = crate::tests::parse_source(source);
    let rate = parsed
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str(&parsed) == "Rate" => Some(data),
            _ => None,
        })
        .expect("Rate data");
    let field = parsed
        .items
        .data_members(rate.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("Rate::value field");
    let TypeReferenceNode::Constrained { constraints, .. } =
        parsed.type_references.type_reference(field.type_reference)
    else {
        panic!("quantity should be a constrained carrier");
    };
    let [TypeConstraintNode::Domain(domain)] = parsed.type_references.constraints(*constraints)
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
    assert_eq!(binary.operator, BinaryOperator::Divide);
    assert_eq!(parsed.display_expression_name(binary.left), "A");
    assert_eq!(parsed.display_expression_name(binary.right), "B");
}

#[test]
fn qualified_field_domains_keep_their_written_names() {
    let source = r#"
        data Sample {
            local: u64 in Carry::MovableAddress;
            portable: u64 in Carry::Portable;
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let data = crate::tests::root_data(&parsed, "data root item");
    let fields = parsed
        .items
        .data_members(data.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
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
                TypeConstraintNode::Domain(name) => name.name.as_str(&parsed).to_owned(),
                other => panic!("expected a domain, got {other:?}"),
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(names(fields[0]), ["Carry::MovableAddress"]);
    assert_eq!(names(fields[1]), ["Carry::Portable"]);
}

#[test]
fn an_ensures_clause_keeps_its_single_qualified_domain() {
    let source = r#"
        machine grant(value: u64) -> u64
        ensures
            result in Carry::Portable
        {
            transition { _ -> value }
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");
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
                .map(|member| member.as_str(&parsed))
                .collect::<Vec<_>>()
                .join("::"),
            other => panic!("expected domain membership, got {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(domains, ["Carry::Portable"]);
}

#[test]
fn parses_slice_range_indexing_into_range_expression() {
    let source = r#"
        data Main {}

        machine Main::main(main: @mutable Main) -> usize {
            let values: [usize; 4] = [1, 2, 3, 4];
            let view: @[usize] = values.as_slice();
            let tail: @[usize] = view[1..];
            tail.length
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");
    let state = crate::tests::entry_state(&parsed, machine);
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
        parsed.display_expression_name(indexed.index),
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
fn borrows_bind_before_casts_and_after_field_and_index_access() {
    let parsed = crate::tests::parse_source(
        r#"machine view(packet: @mutable Packet) {
            let shared: @u8 = @packet.parts[0] as -Marked;
            let mutable: @mutable u8 = @mutable packet.parts[1] as -Marked;
        }"#,
    );
    let machine = crate::tests::root_machine(&parsed, "machine root item");
    let state = crate::tests::entry_state(&parsed, machine);
    let locals = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    for (local, access) in locals
        .iter()
        .zip([ReferenceAccess::Shared, ReferenceAccess::Mutable])
    {
        let ExpressionNode::Cast(cast) = parsed.expressions.expression(local.initial_value) else {
            panic!("the cast applies to the completed borrow");
        };
        let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(cast.value) else {
            panic!("the cast operand retains its borrow");
        };
        assert_eq!(borrow.access, access);
        let ExpressionNode::Indexed(indexed) = parsed.expressions.expression(borrow.target) else {
            panic!("the borrow applies after indexing");
        };
        assert!(matches!(
            parsed.expressions.expression(indexed.collection),
            ExpressionNode::Member(_)
        ));
    }
    let error = crate::tests::parse_error("machine unfinished() -> u8 { 1 +");
    assert!(error.message.contains("expected expression"), "{error:?}");
}

#[test]
fn parses_structural_recast_targets_as_type_references() {
    let source = r#"
        machine inspect(bytes: [u8; 4]) {
            let fixed: @[u8; 4] = @bytes as @[u8; 4];
            let slice: @[u8] = @bytes as @[u8];
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");
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

    let ExpressionNode::Cast(fixed) = parsed.expressions.expression(locals[0].initial_value) else {
        panic!("the fixed-array recast applies to its completed borrow");
    };
    let ExpressionNode::Borrow(fixed_borrow) = parsed.expressions.expression(fixed.value) else {
        panic!("the fixed-array recast retains its shared borrow operand");
    };
    assert_eq!(fixed_borrow.access, ReferenceAccess::Shared);
    let TypeReferenceNode::FixedArray {
        element_type,
        length: syntax_trees::types::FixedArrayLength::Literal(4),
    } = parsed.type_references.type_reference(fixed.target_type)
    else {
        panic!("fixed-array target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str(&parsed) == "u8"
    ));

    let ExpressionNode::Cast(slice) = parsed.expressions.expression(locals[1].initial_value) else {
        panic!("the slice recast applies to its completed borrow");
    };
    let ExpressionNode::Borrow(slice_borrow) = parsed.expressions.expression(slice.value) else {
        panic!("the slice recast retains its shared borrow operand");
    };
    assert_eq!(slice_borrow.access, ReferenceAccess::Shared);
    let TypeReferenceNode::Slice { element_type } =
        parsed.type_references.type_reference(slice.target_type)
    else {
        panic!("slice target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str(&parsed) == "u8"
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
                Filesystem
            crashes none;
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let trait_definition = crate::tests::root_trait(&parsed, "trait root item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let contracts = parsed.items.capability_contracts(signature.contracts);
    let service_reaches = parsed
        .type_references
        .type_reference_handles(signature.service_reaches);

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
    assert!(
        matches!(parsed.type_references.type_reference(service_reaches[0]), syntax_trees::types::TypeReferenceNode::Named(name) if name.as_str(&parsed) == "Filesystem")
    );
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

        machine Main::main(main: @mutable Main) {
            transition main.player {
                Player::Alive -> done(),
                Player { health, .. } if health > 5 -> done(),
                _ -> done()
            }

            machine done() {}
        }
        "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");
    let entry = crate::tests::entry_state_handle(&parsed, machine);
    let state = parsed.items.state(entry);
    let transition =
        &crate::tests::dispatch(&parsed, parsed.items.statements(state.statements)[0]).arms[1];
    assert!(matches!(
        transition.guard,
        syntax_trees::statement::TransitionGuardNode::Always
    ));
    let [pattern] = transition.patterns.as_ref() else {
        panic!("data-pattern arm retains one subject pattern");
    };
    let syntax_trees::expression::MatchPattern::Destructure(pattern) = pattern.pattern else {
        panic!("structured pattern")
    };
    let ExpressionNode::Binary(comparison) = parsed.expressions.expression(pattern.guard) else {
        panic!("data-pattern guard should be a comparison");
    };
    assert_eq!(comparison.operator, BinaryOperator::Greater);
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
                Outcome::Success { value; selected: local, shorthand } -> value,
                Outcome::Failure { } -> 0
            }
        }
    "#;
    let parsed = crate::tests::parse_source(source);
    let transition = parsed
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) if machine.name.as_str(&parsed) == "inspect" => Some(machine),
            _ => None,
        })
        .into_iter()
        .flat_map(|machine| parsed.items.state_handles(machine.states))
        .flat_map(|state| {
            parsed
                .items
                .statements(parsed.items.state(*state).statements)
        })
        .flat_map(|statement| &crate::tests::dispatch(&parsed, *statement).arms)
        .find(|arm| !arm.proof_selectors().is_empty())
        .expect("one transition arm with erased selectors");
    let selectors = parsed
        .statements
        .outcome_proof_selectors(transition.proof_selectors());
    assert_eq!(selectors.len(), 2);
    assert_eq!(selectors[0].output_field.as_str(&parsed), "selected");
    assert_eq!(selectors[0].binding.as_str(&parsed), "local");
    assert_eq!(selectors[1].output_field.as_str(&parsed), "shorthand");
    assert_eq!(selectors[1].binding.as_str(&parsed), "shorthand");
}

#[test]
fn outcome_proof_selectors_preserve_one_subject_and_the_same_arm_shape() {
    let sources = [
        r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; selected: local } -> {},
                Outcome::Failure { } -> {}
            }
        }
        "#,
        r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; } -> {},
                Outcome::Failure { } -> {}
            }
        }
        "#,
    ];
    let mut plans = Vec::new();
    for source in sources {
        let parsed = crate::tests::parse_source(source);
        let inspect = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "inspect" => {
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
        let [statement] = statements else {
            panic!("one authored dispatch")
        };
        let dispatch = crate::tests::dispatch(&parsed, *statement);
        let [subject] = parsed.statements.expression_handles(dispatch.subjects) else {
            panic!("one produced subject")
        };
        assert!(
            matches!(parsed.expressions.expression(*subject),ExpressionNode::Call(call) if call.target.as_str(&parsed) == "produce")
        );
        let producer_calls = parsed
            .expressions
            .iter_expressions()
            .filter(|(_, expression)| {
                matches!(
                    expression,
                    ExpressionNode::Call(call) if call.target.as_str(&parsed) == "produce"
                )
            })
            .count();
        assert_eq!(
            producer_calls, 1,
            "the dispatch contains one authored producer call"
        );
        plans.push((statements.len(), dispatch.arms.len()));
    }
    assert_eq!(
        plans[0], plans[1],
        "erased proof selectors preserve the authored dispatch shape"
    );
}

#[test]
fn rejects_duplicate_and_defers_noncase_outcome_proof_selectors() {
    let error = crate::tests::parse_error(
        "machine inspect(result: Outcome) { transition result { Outcome::Success { ; selected: local, selected: other } -> {} } }",
    );
    assert!(
        error
            .message
            .contains("outcome proof selector `selected` is bound more than once"),
        "got: {}",
        error.message
    );

    let parsed = crate::tests::parse_source(
        "machine inspect(result: Record) { transition result { Record { ; selected: local } -> {} } }",
    );
    let machine = crate::tests::root_machine(&parsed, "machine root item");
    let state = crate::tests::entry_state(&parsed, machine);
    let transition =
        &crate::tests::dispatch(&parsed, parsed.items.statements(state.statements)[0]).arms[0];
    let selectors = parsed
        .statements
        .outcome_proof_selectors(transition.proof_selectors());
    assert_eq!(selectors.len(), 1);
    assert_eq!(selectors[0].output_field.as_str(&parsed), "selected");
    assert_eq!(selectors[0].binding.as_str(&parsed), "local");
}

#[test]
fn parses_record_field_value_pattern_as_subject_member_equality() {
    let source = r#"
        data Header [copy] { ok: i32; version: i32; }
        machine inspect(h: Header) -> i32 {
            transition h {
                Header { ok: 0, version } -> version,
                _ -> 1
            }
        }
    "#;

    let parsed = crate::tests::parse_source(source);
    let machine = crate::tests::root_machine(&parsed, "machine root item");
    let state = crate::tests::entry_state(&parsed, machine);
    let transition =
        &crate::tests::dispatch(&parsed, parsed.items.statements(state.statements)[0]).arms[0];
    assert!(matches!(
        transition.guard,
        syntax_trees::statement::TransitionGuardNode::Always
    ));
    let [pattern] = transition.patterns.as_ref() else {
        panic!("field-value arm retains one subject pattern");
    };
    let syntax_trees::expression::MatchPattern::Destructure(pattern) = pattern.pattern else {
        panic!("structured pattern")
    };
    let [ok, version] = parsed.expressions.match_fields(pattern.fields) else {
        panic!("both authored fields are retained");
    };
    assert_eq!(ok.member.as_str(&parsed), "ok");
    let syntax_trees::expression::MatchFieldAction::Equal(value) = &ok.action else {
        panic!("field-value pattern should compare the subject member");
    };
    assert!(matches!(
        parsed.expressions.expression(*value),
        ExpressionNode::Integer(_)
    ));
    assert_eq!(version.member.as_str(&parsed), "version");
    assert!(matches!(
        version.action,
        syntax_trees::expression::MatchFieldAction::Bind(_)
    ));
    let syntax_trees::statement::TransitionTargetNode::Value(target) =
        parsed.statements.transition_target(transition.target)
    else {
        panic!("value arm");
    };
    assert!(matches!(
        parsed.expressions.expression(*target),
        ExpressionNode::Member(_)
    ));
}
