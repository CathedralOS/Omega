//! Scalar contract entry tests.

use crate::tests::front_end::typed_program;
use crate::values::lower_scalar_contract_predicate;
use crate::values::scalar::contract_entry::EntryOperands;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

fn requirement(program: &TypedTrees) -> ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| {
            if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                Some(*expression)
            } else {
                None
            }
        })
        .expect("fixture has one entry requirement")
}

fn scalar_requirement(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    lower_scalar_contract_predicate(
        program,
        &CheckedOperatorFacts::default(),
        &program.machines()[0],
        expression,
        false,
        &mut 4096,
    )
}

#[test]
fn scalar_state_contract_lands_anonymous_signed_bounds() {
    let program = typed_program(
        "machine decode(wide: i64) {
            transition { _ -> write(wide) }
            state write(wide: i64)
                requires (-2147483648 <= wide && wide <= 2147483647)
                    && 0 - 10 <= wide && wide <= 10
            {}
        }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[1];
    let expression = program
        .state_contracts(state)
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("authored non-entry requirement");
    let predicate = crate::values::lower_state_scalar_contract_predicate(
        &program,
        &CheckedOperatorFacts::default(),
        machine,
        state,
        expression,
        &mut 4096,
    )
    .expect("both negative bound spellings retain the state contract");
    let mut pending = vec![&predicate];
    let mut bounds = Vec::new();
    while let Some(predicate) = pending.pop() {
        match predicate {
            CheckedBooleanExpression::And { left, right } => {
                pending.extend([right.as_ref(), left.as_ref()]);
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                for operand in [left.as_ref(), right.as_ref()] {
                    match operand {
                        CheckedScalarExpression::IntegerLiteral { literal } => {
                            assert_eq!(
                                crate::values::scalar_expression_type(operand),
                                Some(PrimitiveType::I64)
                            );
                            bounds.push(literal.value_bignum().unwrap().to_i64().unwrap());
                        }
                        CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: PrimitiveType::I64,
                        } => {}
                        other => panic!("unexpected contract operand: {other:?}"),
                    }
                }
            }
            other => panic!("unexpected state contract: {other:?}"),
        }
    }
    assert_eq!(bounds, [-2147483648, 2147483647, -10, 10]);
}

#[test]
fn state_byte_predicate_contract_retains_its_exact_structural_formal() {
    let program = typed_program(
        "machine validate(unrelated: &[u8], input: &[u8]) {
            transition valid_utf8(input) {
                true -> consume(0, unrelated, input)
                _ -> reject()
            }
            state consume(tag: u64, other: &[u8], selected: &[u8])
                requires valid_utf8(selected)
            {}
            state reject() {}
        }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[1];
    let expression = program
        .state_contracts(state)
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("authored byte-predicate state requirement");
    let predicate = crate::values::lower_state_scalar_contract_predicate(
        &program,
        &CheckedOperatorFacts::default(),
        machine,
        state,
        expression,
        &mut 4096,
    )
    .expect("a byte predicate retains its exact state-entry operand");
    assert!(matches!(
        predicate,
        CheckedBooleanExpression::ByteSequencePredicate {
            predicate: language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8,
            root: checked_trees::CheckedStorageRoot::Parameter { index: 2 },
            ref path,
        } if path.is_empty()
    ));
}

#[test]
fn byte_contract_rejects_resolved_lookalikes_and_foreign_subjects() {
    let program = typed_program(
        "machine inspect(bytes: &[u8]) requires valid_utf8(bytes) {}
         machine foreign(bytes: &[u8]) {}",
    );
    let root = requirement(&program);
    assert!(scalar_requirement(&program, root).is_some());
    let ExpressionNode::Call(call) = program.expression_table.expression(root) else {
        panic!("byte predicate call");
    };
    let argument = program.expression_table.expression_handles(call.arguments)[0];
    let mut resolved = program.clone();
    let ExpressionNode::Call(call) = resolved.expression_table.expression_mut(root) else {
        unreachable!();
    };
    call.target_symbol = program.machines()[1].symbol;
    assert!(
        scalar_requirement(&resolved, root).is_none(),
        "a resolved declaration wins over builtin spelling"
    );
    let foreign =
        program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
    for subject in [symbols::SymbolHandle::invalid(), foreign] {
        let mut invalid = program.clone();
        let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(argument) else {
            panic!("exact formal operand");
        };
        name.symbol = subject;
        name.head_symbol = subject;
        assert!(
            scalar_requirement(&invalid, root).is_none(),
            "spelling does not authenticate a missing or foreign formal"
        );
    }
}

#[test]
fn scalar_entry_contract_lands_anonymous_arithmetic_at_its_typed_peer() {
    for (predicate, arithmetic) in [
        ("(2 * 3 - 10) <= wide", false),
        ("wide + (2 * 3 - 10) <= wide", true),
    ] {
        let program = typed_program(&format!(
            "machine value(wide: i64) -> bool requires {predicate} {{ true }}"
        ));
        let lowered = scalar_requirement(&program, requirement(&program)).unwrap();
        let CheckedBooleanExpression::IntegerComparison { left, .. } = lowered else {
            panic!("integer comparison retained");
        };
        let literal = if arithmetic {
            let CheckedScalarExpression::IntegerBinary { left, right, .. } = *left else {
                panic!("typed addition retains its operation");
            };
            assert!(matches!(
                *left,
                CheckedScalarExpression::Parameter { position: 0, .. }
            ));
            right
        } else {
            left
        };
        let CheckedScalarExpression::IntegerLiteral { literal } = *literal else {
            panic!("anonymous peer lands as one exact literal");
        };
        assert_eq!(literal.value_bignum().unwrap().to_i64(), Some(-4));
    }
}

#[test]
fn scalar_contract_anonymous_landing_rejects_invalid_values_and_selected_operators() {
    for predicate in ["(0 - 129) <= wide", "(1 / 0) <= wide", "(1 / 2) <= wide"] {
        let program = typed_program(&format!(
            "machine value(wide: i8) -> bool requires {predicate} {{ true }}"
        ));
        assert!(
            scalar_requirement(&program, requirement(&program)).is_none(),
            "invalid anonymous endpoint: {predicate}"
        );
    }

    let program =
        typed_program("machine value(wide: i64) -> bool requires (0 - 10) <= wide { true }");
    let comparison = requirement(&program);
    assert!(scalar_requirement(&program, comparison).is_some());
    let ExpressionNode::Binary(binary) = program.expression_table.expression(comparison) else {
        panic!("comparison source");
    };
    for (expression, spelling) in [
        (comparison, language_core::OperatorSpelling::LessEqual),
        (binary.left, language_core::OperatorSpelling::Subtract),
    ] {
        let mut uses = arena::Arena::new();
        uses.append(checked_trees::CheckedOperatorUseFact {
            expression,
            spelling,
            status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
            selected_operator_symbol: symbols::SymbolHandle::from_parts(1, 1),
            ..Default::default()
        });
        let operators =
            CheckedOperatorFacts::with_roots(uses, arena::Arena::new(), arena::Arena::new());
        assert!(
            lower_scalar_contract_predicate(
                &program,
                &operators,
                &program.machines()[0],
                comparison,
                false,
                &mut 4096,
            )
            .is_none(),
            "selected {spelling:?} was interpreted as anonymous builtin arithmetic"
        );
    }
}

#[test]
fn scalar_entry_requirement_rejects_symbol_and_spelling_disagreement() {
    let mut program =
        typed_program("machine value(left: bool, right: bool) -> bool requires left { true }");
    let root = requirement(&program);
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    let left = parameters[0].symbol;
    let right = parameters[1].symbol;
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression_mut(expression) {
            ExpressionNode::Name(path) if path.symbol == left => {
                path.symbol = right;
                path.head_symbol = right;
                assert!(scalar_requirement(&program, root).is_none());
                return;
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            _ => {}
        }
    }
    panic!("requirement has the left formal occurrence");
}

#[test]
fn scalar_entry_requirement_rejects_ambiguous_retained_formal_names() {
    let mut program =
        typed_program("machine value(left: bool, right: bool) -> bool requires right { true }");
    let root = requirement(&program);
    let parameters = program.machine_states(&program.machines()[0])[0].parameters;
    let names = program
        .tables
        .state_parameters
        .span_mut_or_empty(parameters);
    names[0].name = names[1].name.clone();
    assert!(scalar_requirement(&program, root).is_none());
}

#[test]
fn scalar_entry_requirement_requires_one_live_consistent_attachment_owner() {
    let program = typed_program(
        "data Box {} data Other {} machine Box::value(flag: bool) -> bool requires flag { flag }",
    );
    let root = requirement(&program);
    assert!(scalar_requirement(&program, root).is_some());
    let owner = program.machines()[0].attached_data_symbol;
    for wrong in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1),
        program.data_definitions()[1].symbol,
        program.machines()[0].symbol,
    ] {
        let mut invalid = program.clone();
        invalid.machines_mut()[0].attached_data_symbol = wrong;
        assert!(scalar_requirement(&invalid, root).is_none());
    }
    let mut detached = program.clone();
    detached.machines_mut()[0].attached_data = None;
    assert!(scalar_requirement(&detached, root).is_none());
    let mut duplicate = program.clone();
    let definitions = duplicate.roots.data_definitions;
    duplicate
        .tables
        .data_definitions
        .span_mut_or_empty(definitions)[1] = program.data_definitions()[0].clone();
    assert!(scalar_requirement(&duplicate, root).is_none());
    let mut stale = program.clone();
    let definitions = stale.roots.data_definitions;
    let stale_owner =
        symbols::SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1);
    stale.tables.data_definitions.span_mut_or_empty(definitions)[0].symbol = stale_owner;
    stale.machines_mut()[0].attached_data_symbol = stale_owner;
    assert!(scalar_requirement(&stale, root).is_none());
}

#[test]
fn scalar_entry_requirement_rejects_unresolved_or_non_owned_boolean_leaves() {
    for source in [
        "machine value(flag: &bool) -> bool requires flag { true }",
        "data Box { flag: bool; } machine Box::value(&self) -> bool requires self.flag { true }",
    ] {
        let program = typed_program(source);
        assert!(
            scalar_requirement(&program, requirement(&program)).is_none(),
            "{source}"
        );
    }
}

#[test]
fn scalar_entry_requirement_rejects_corrupted_formal_namespace_rows() {
    let program = typed_program(
        "machine value(left: bool, right: bool) -> bool requires left { true } machine foreign(right: bool) -> bool { right }",
    );
    let root = requirement(&program);
    let entry = &program.machine_states(&program.machines()[0])[0];
    let parameters = program.state_parameters(entry);
    let right = parameters[1].symbol;
    for wrong in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(right.arena_index(), right.generation() + 1),
        parameters[0].symbol,
        program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol,
    ] {
        let mut invalid = program.clone();
        invalid
            .tables
            .state_parameters
            .span_mut_or_empty(entry.parameters)[1]
            .symbol = wrong;
        assert!(scalar_requirement(&invalid, root).is_none());
    }
}

#[test]
fn scalar_entry_requirement_does_not_recover_missing_or_foreign_name_symbols() {
    let program = typed_program(
        "machine value(flag: bool) -> bool requires flag { let local: bool = false; flag } machine foreign(flag: bool) -> bool { flag }",
    );
    let root = requirement(&program);
    let symbol =
        program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0].symbol;
    let mut pending = vec![root];
    let name = loop {
        let expression = pending
            .pop()
            .expect("requirement has its exact flag occurrence");
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol == symbol => break expression,
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            _ => {}
        }
    };
    let foreign =
        program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
    let local = program
        .statement_table
        .statements(program.machine_states(&program.machines()[0])[0].statement_nodes)
        .iter()
        .find_map(|statement| {
            if let StatementNode::LocalData(local) = statement {
                Some(local.symbol)
            } else {
                None
            }
        })
        .unwrap();
    for wrong in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1),
        foreign,
        local,
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Name(path) = invalid.expression_table.expression_mut(name) else {
            unreachable!()
        };
        path.symbol = wrong;
        path.head_symbol = wrong;
        assert!(scalar_requirement(&invalid, root).is_none());
    }
    let mut invalid = program.clone();
    let ExpressionNode::Name(path) = invalid.expression_table.expression_mut(name) else {
        unreachable!()
    };
    path.head_symbol = foreign;
    assert!(scalar_requirement(&invalid, root).is_none());
}

#[test]
fn scalar_entry_requirement_rejects_cycles_structural_parameters_and_authored_operators() {
    let program = typed_program("machine value(flag: bool) -> bool requires flag && true { flag }");
    let root = requirement(&program);
    let mut cyclic = program.clone();
    let ExpressionNode::Binary(binary) = cyclic.expression_table.expression_mut(root) else {
        panic!("normalized Boolean fact")
    };
    binary.left = root;
    assert!(scalar_requirement(&cyclic, root).is_none());
    assert!(
        scalar_requirement(
            &program,
            ExpressionHandle::from_parts(root.arena_index(), root.generation() + 1)
        )
        .is_none()
    );
    let structural = typed_program(
        "data Box { flag: bool; } machine value(input: Box) -> bool requires input.flag { true }",
    );
    assert!(scalar_requirement(&structural, requirement(&structural)).is_none());
    let authored = typed_program(
        "boundary operator == bool::custom(left: bool, right: bool) -> bool; machine value(flag: bool) -> bool requires flag == true { flag }",
    );
    assert!(scalar_requirement(&authored, requirement(&authored)).is_none());
}

#[test]
fn entry_snapshot_rejects_foreign_stale_duplicate_and_wrong_typed_storage() {
    let source = "machine value(first: bool, mut input: bool) -> bool { let mut other: bool = false; input }";
    let program = typed_program(source);
    let state = &program.machine_states(&program.machines()[0])[0];
    let parameters = program.state_parameters(state);
    let entry = EntryOperands {
        program: &program,
        parameters,
    };
    let symbol = parameters[1].symbol;
    let mut valid = CheckedBooleanExpression::StorageRead { symbol };
    assert_eq!(entry.boolean(&mut valid), Some(()));
    assert_eq!(valid, CheckedBooleanExpression::Parameter { position: 1 });
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            if let StatementNode::LocalData(local) = statement {
                Some(local.symbol)
            } else {
                None
            }
        })
        .unwrap();
    for wrong in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1),
        parameters[0].symbol,
        local,
    ] {
        assert_eq!(
            entry.boolean(&mut CheckedBooleanExpression::StorageRead { symbol: wrong }),
            None
        );
    }
    assert_eq!(
        entry.scalar(&mut CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type: PrimitiveType::I32
        }),
        None
    );
    assert_eq!(
        entry.boolean(&mut CheckedBooleanExpression::Local { position: 1 }),
        None
    );
    let duplicate_parameters = [parameters[1].clone(), parameters[1].clone()];
    let duplicate = EntryOperands {
        program: &program,
        parameters: &duplicate_parameters,
    };
    assert_eq!(
        duplicate.boolean(&mut CheckedBooleanExpression::StorageRead { symbol }),
        None
    );
}
