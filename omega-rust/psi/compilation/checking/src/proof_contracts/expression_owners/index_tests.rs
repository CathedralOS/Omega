use super::*;
use arena::{Arena, Handle};
use checked_trees::{ContractProofFact, ContractProofFactOwner};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    constant::ConstDeclaration,
    data::DataDefinition,
    domain::ProofFact,
    expression::{CallExpression, Expression, ExpressionNode, NamePath},
    machine::Machine,
    name::Identifier,
    signature::StateParameter,
    state::State,
    statement::StatementNode,
    types::TypeReferenceNode,
};

fn call(program: &mut TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&Expression::Call(Box::new(CallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: Some(Box::new(Expression::Name(NamePath::unresolved(vec![
                Identifier::from("x"),
            ])))),
            target_symbol: SymbolHandle::invalid(),
            target: Identifier::from("go"),
            arguments: Vec::new().into(),
            evidence_arguments: Vec::new().into(),
            operational_acknowledgement: Default::default(),
        })))
}

fn fixture(
    flip: bool,
    second_parameter: bool,
) -> (
    TypedTrees,
    ExpressionHandle,
    ExpressionHandle,
    [SymbolHandle; 2],
    [SymbolHandle; 2],
) {
    let mut symbols = SymbolTableBuilder::default();
    let a = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("A"));
    let b = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("B"));
    let first = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("First"));
    let sa = symbols
        .insert_children(first, [(SymbolKind::State, SymbolNameRef::Static("go"))])
        .handle_at(0);
    let pa = symbols
        .insert_children(sa, [(SymbolKind::Parameter, SymbolNameRef::Static("x"))])
        .handle_at(0);
    let second = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Second"));
    let sb = symbols
        .insert_children(second, [(SymbolKind::State, SymbolNameRef::Static("go"))])
        .handle_at(0);
    let pb = symbols
        .insert_children(sb, [(SymbolKind::Parameter, SymbolNameRef::Static("x"))])
        .handle_at(0);
    let constant = symbols.insert_root(SymbolKind::Const, SymbolNameRef::Static("K"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let ta = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: a,
            name: Identifier::from("A"),
        });
    let tb = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: b,
            name: Identifier::from("B"),
        });
    let expression = call(&mut program);
    let repeated = program
        .expression_table
        .insert_expression_handles([expression, expression]);
    let root = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(repeated));
    let statements = program
        .statement_table
        .insert_statements([StatementNode::Expression(root)]);
    let mut machine = Machine {
        symbol: first,
        attached_data_symbol: a,
        ..Default::default()
    };
    let mut state = State {
        symbol: sa,
        name: Identifier::from("go"),
        statement_nodes: statements,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: pa,
            name: Identifier::from("x"),
            type_reference: if flip { tb } else { ta },
            ..Default::default()
        },
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut machine = Machine {
        symbol: second,
        attached_data_symbol: b,
        ..Default::default()
    };
    let mut state = State {
        symbol: sb,
        name: Identifier::from("go"),
        ..Default::default()
    };
    if second_parameter {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: pb,
                name: Identifier::from("x"),
                type_reference: tb,
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let marker = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let where_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(marker), ProofFact::Expression(marker)]);
    program.push_data_definition(DataDefinition {
        symbol: a,
        name: Identifier::from("A"),
        where_facts,
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: b,
        name: Identifier::from("B"),
        ..Default::default()
    });
    program.push_const_declaration(ConstDeclaration {
        symbol: constant,
        authored_initializer: marker,
        materialized_initializer: marker,
        ..Default::default()
    });
    (program, expression, marker, [first, second], [sa, sb])
}

fn contracts(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    owners: impl IntoIterator<Item = ContractProofFactOwner>,
) -> Arena<ContractProofFact> {
    let fact = program
        .proof_facts
        .append(ProofFact::Expression(expression));
    let mut rows = Arena::default();
    for owner in owners {
        rows.append(ContractProofFact {
            owner,
            fact,
            ..Default::default()
        });
    }
    rows
}

type Row = (
    SymbolHandle,
    Option<InferredType>,
    Option<TypeReferenceHandle>,
    Vec<(String, SymbolHandle, TypeReferenceHandle)>,
);
fn roster(environments: Option<Vec<TypeEnvironment>>) -> Option<Vec<Row>> {
    environments.map(|rows| {
        rows.into_iter()
            .map(|row| {
                (
                    row.owner,
                    row.self_type,
                    row.result_type,
                    row.bindings
                        .into_iter()
                        .map(|b| (b.name, b.symbol, b.type_reference))
                        .collect(),
                )
            })
            .collect()
    })
}

fn compare(
    program: &TypedTrees,
    facts: &Arena<ContractProofFact>,
    expressions: &[ExpressionHandle],
) {
    let index = OwnerEnvironmentIndex::new(program, facts);
    for &expression in expressions {
        assert_eq!(
            roster(index.environments(expression)),
            roster(environments::exact_owner_environments(
                program, facts, expression, None
            ))
        );
        if let Some(call) = program.expression_table.call_node(expression) {
            assert_eq!(
                checked_machine_call_target_from_exact_owner(
                    program,
                    facts,
                    expression,
                    call,
                    Some(&index)
                ),
                checked_machine_call_target_from_exact_owner(
                    program, facts, expression, call, None
                )
            );
        }
        let mut original = Vec::new();
        for (m, machine) in program.machines().iter().enumerate() {
            for (s, state) in program.machine_states(machine).iter().enumerate() {
                for (i, statement) in program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                {
                    let mut reached = Vec::new();
                    crate::expression_traversal::collect_statement_expression_trees(
                        program,
                        statement,
                        &mut reached,
                    );
                    if reached.contains(&expression) {
                        original.push((m, s, i));
                    }
                }
            }
        }
        assert_eq!(index.containing_statement_sites(expression), original);
    }
}

#[test]
fn indexed_owners_match_raw_rosters_targets_and_refusals_for_exact_worlds() {
    for flip in [false, true] {
        let (mut program, expression, marker, machines, targets) = fixture(flip, true);
        let owner = ContractProofFactOwner::Machine {
            machine_symbol: machines[0],
        };
        let equal_owners = contracts(&mut program, expression, [owner, owner]);
        let detached = call(&mut program);
        let other_generation =
            Handle::from_parts(expression.arena_index(), expression.generation() + 1);
        compare(
            &program,
            &equal_owners,
            &[
                expression,
                marker,
                detached,
                other_generation,
                Handle::invalid(),
            ],
        );
        let index = OwnerEnvironmentIndex::new(&program, &equal_owners);
        // Two equal checked owners and the executable site remain THREE rows.
        assert_eq!(index.environments(expression).unwrap().len(), 3);
        // Repeated data roots and overlapping initializer roots each add only
        // one owner; repeated statement edges identify exactly one site.
        assert_eq!(index.environments(marker).unwrap().len(), 2);
        assert_eq!(
            index.containing_statement_sites(expression),
            vec![(0, 0, 0)]
        );
        let node = program.expression_table.call_node(expression).unwrap();
        assert_eq!(
            checked_machine_call_target_from_exact_owner(
                &program,
                &equal_owners,
                expression,
                node,
                Some(&index)
            ),
            Some(targets[usize::from(flip)])
        );
        drop(index);
        let conflicting = contracts(
            &mut program,
            expression,
            [
                owner,
                ContractProofFactOwner::Machine {
                    machine_symbol: machines[1],
                },
            ],
        );
        compare(&program, &conflicting, &[expression, marker]);
        if !flip {
            let node = program.expression_table.call_node(expression).unwrap();
            assert_eq!(
                checked_machine_call_target_from_exact_owner(
                    &program,
                    &conflicting,
                    expression,
                    node,
                    None
                ),
                None
            );
        }
        let signatures = contracts(
            &mut program,
            detached,
            [ContractProofFactOwner::CallSignature {
                owner_symbol: machines[0],
                state_symbol: targets[0],
                application: Handle::from_parts(1, 1),
            }],
        );
        compare(&program, &signatures, &[detached]);
        let node = program.expression_table.call_node(detached).unwrap();
        assert_eq!(
            checked_machine_call_target_from_exact_owner(
                &program,
                &signatures,
                detached,
                node,
                None
            ),
            None
        );
    }
    let (mut program, expression, marker, machines, targets) = fixture(false, false);
    let incomplete = contracts(
        &mut program,
        expression,
        [ContractProofFactOwner::Machine {
            machine_symbol: machines[1],
        }],
    );
    compare(&program, &incomplete, &[expression, marker]);
    let node = program.expression_table.call_node(expression).unwrap();
    // Missing type-owner knowledge keeps the original executable-site fallback.
    assert_eq!(
        checked_machine_call_target_from_exact_owner(&program, &incomplete, expression, node, None),
        Some(targets[0])
    );
}
