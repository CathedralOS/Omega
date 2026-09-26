//! Scalar contract entry tests.

use crate::tests::front_end::typed_program;
use crate::values::lower_scalar_contract_predicate;
use crate::values::scalar::contract_entry::EntryOperands;
use crate::values::scalar::contract_entry::entry_parameters;
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
fn scalar_contract_reads_share_and_exhaust_the_supplied_budget() {
    let program = typed_program("machine value(flag: bool) -> bool requires !flag { flag }");
    let expression = requirement(&program);
    let operators = CheckedOperatorFacts::default();
    let machine = &program.machines()[0];
    let mut remaining = 4;
    for expected_remaining in [2, 0] {
        assert!(
            lower_scalar_contract_predicate(
                &program,
                &operators,
                machine,
                expression,
                false,
                &mut remaining,
            )
            .is_some()
        );
        assert_eq!(remaining, expected_remaining);
    }
    assert!(
        lower_scalar_contract_predicate(
            &program,
            &operators,
            machine,
            expression,
            false,
            &mut remaining,
        )
        .is_none()
    );
    assert_eq!(remaining, 0);
    let mut insufficient = 1;
    assert!(
        lower_scalar_contract_predicate(
            &program,
            &operators,
            machine,
            expression,
            false,
            &mut insufficient,
        )
        .is_none()
    );
    assert_eq!(insufficient, 0, "failed reads do not replenish the budget");
}

#[test]
fn scalar_entry_requirement_uses_exact_formals_and_preserves_mutable_snapshots() {
    for requirement_text in ["flag", "!flag", "flag && !other", "flag == other"] {
        let program = typed_program(&format!(
            "machine value(mut flag: bool, other: bool) -> bool requires {requirement_text} {{ flag }}"
        ));
        assert!(
            scalar_requirement(&program, requirement(&program)).is_some(),
            "{requirement_text}"
        );
    }
}

#[test]
fn scalar_contracts_keep_concrete_predicates_on_generic_declarations() {
    let program = typed_program(
        "machine value<T>(input: u16) -> u16\nrequires input < 256u16\nensures result == input\n{ input }",
    );
    let machine = &program.machines()[0];
    assert!(!program.machine_type_parameters(machine).is_empty());
    for clause in program.machine_contracts(machine) {
        let [typed_trees::domain::ProofFact::Expression(expression)] =
            program.proof_facts.span_or_empty(clause.facts)
        else {
            panic!("predicate")
        };
        assert!(
            lower_scalar_contract_predicate(
                &program,
                &CheckedOperatorFacts::default(),
                machine,
                *expression,
                clause.kind == typed_trees::signature::SignatureContractKind::Ensures,
                &mut 4096,
            )
            .is_some()
        );
    }
    assert!(
        entry_parameters(&program, machine).is_none(),
        "structural crash eligibility remains closed"
    );
    for source in [
        "data Box<T> {} machine Box::value(flag: bool) -> bool requires flag { flag }",
        "machine value<T>(flag: bool) -> bool requires flag { flag }",
    ] {
        let program = typed_program(source);
        assert!(scalar_requirement(&program, requirement(&program)).is_some());
        assert!(entry_parameters(&program, &program.machines()[0]).is_none());
    }
}

#[test]
fn boolean_result_predicates_keep_contract_owner_and_poststate_namespace() {
    let program = typed_program(
        "machine value(input: bool) -> bool ensures result == input { input } machine other(input: bool) -> bool ensures result == input { input }",
    );
    let root = requirement(&program);
    let operators = CheckedOperatorFacts::default();
    let machine = &program.machines()[0];
    assert_eq!(
        lower_scalar_contract_predicate(&program, &operators, machine, root, true, &mut 4096),
        Some(CheckedBooleanExpression::Equal {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 0 })
        })
    );
    assert!(
        lower_scalar_contract_predicate(&program, &operators, machine, root, false, &mut 4096)
            .is_none()
    );
    assert!(
        lower_scalar_contract_predicate(
            &program,
            &operators,
            &program.machines()[1],
            root,
            true,
            &mut 4096,
        )
        .is_none()
    );
    let mutable =
        typed_program("machine value(mut input: bool) -> bool ensures result == input { input }");
    assert!(
        lower_scalar_contract_predicate(
            &mutable,
            &operators,
            &mutable.machines()[0],
            requirement(&mutable),
            true,
            &mut 4096,
        )
        .is_none()
    );
}

#[test]
fn scalar_entry_requirement_accepts_attached_and_mixed_exact_namespaces() {
    for requirement_text in ["right", "!right", "left && right", "left == right"] {
        let ordinary = typed_program(&format!(
            "machine value(left: bool, mut right: bool) -> bool requires {requirement_text} {{ true }}"
        ));
        let expected = scalar_requirement(&ordinary, requirement(&ordinary));
        assert!(expected.is_some());
        for signature in [
            "machine Box::value(left: bool, mut right: bool)",
            "machine value(left: bool, record: Box, mut right: bool)",
            "machine Box::value(&self, left: bool, record: Box, mut right: bool)",
        ] {
            let program = typed_program(&format!(
                "data Box {{ flag: bool; }} {signature} -> bool requires {requirement_text} {{ true }}"
            ));
            assert_eq!(
                scalar_requirement(&program, requirement(&program)),
                expected,
                "{signature}: {requirement_text}"
            );
        }
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
