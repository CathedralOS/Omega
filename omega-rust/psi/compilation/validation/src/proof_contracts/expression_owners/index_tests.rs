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
                    crate::queries::expression_traversal::collect_statement_expression_trees(
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

#[test]
fn preparation_retains_fresh_applied_signature_and_parameter_scopes() {
    use typed_trees::data::{TypeParameter, TypeParameterKind};
    use typed_trees::domain::DomainDefinition;
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateSignature};
    use typed_trees::trait_definition::{Conformance, TraitDefinition};
    use typed_trees::types::{DomainConstraint, TypeConstraintNode};

    let mut symbols = SymbolTableBuilder::default();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = symbols.insert_children(
        root,
        [
            (SymbolKind::Trait, SymbolNameRef::Static("Other")),
            (SymbolKind::Trait, SymbolNameRef::Static("Generic")),
            (SymbolKind::Conformance, SymbolNameRef::Static("UnitView")),
            (SymbolKind::Domain, SymbolNameRef::Static("AtLimit")),
        ],
    );
    let other = declarations.handle_at(0);
    let generic = declarations.handle_at(1);
    let implementation = declarations.handle_at(2);
    let at_limit = declarations.handle_at(3);
    let bound = symbols
        .insert_children(
            at_limit,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("bound"))],
        )
        .handle_at(0);
    let other_law = symbols
        .insert_children(other, [(SymbolKind::Machine, SymbolNameRef::Static("law"))])
        .handle_at(0);
    let other_value = symbols
        .insert_children(
            other_law,
            [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
        )
        .handle_at(0);
    let members = symbols.insert_children(
        generic,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
            (SymbolKind::Machine, SymbolNameRef::Static("predicate")),
            (SymbolKind::Machine, SymbolNameRef::Static("law")),
        ],
    );
    let t = members.handle_at(0);
    let predicate = members.handle_at(1);
    let law = members.handle_at(2);
    let predicate_value = symbols
        .insert_children(
            predicate,
            [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
        )
        .handle_at(0);
    let parameters = symbols.insert_children(
        law,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("value")),
            (SymbolKind::Parameter, SymbolNameRef::Static("limit")),
            (SymbolKind::Parameter, SymbolNameRef::Static("amount")),
        ],
    );
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let boolean = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::from("bool"),
        });
    let integer = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::from("u64"),
        });
    let formal = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: t,
            name: Identifier::from("T"),
        });
    let unresolved_value =
        || Expression::Name(NamePath::unresolved(vec![Identifier::from("value")]));
    let other_root = program.expression_table.insert_tree(&unresolved_value());
    let mut other_signature = StateSignature {
        symbol: other_law,
        name: Identifier::from("law"),
        ..Default::default()
    };
    program.push_state_signature_parameter(
        &mut other_signature,
        StateParameter {
            symbol: other_value,
            name: Identifier::from("value"),
            type_reference: boolean,
            ..Default::default()
        },
    );
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(other_root)]);
    program.push_state_signature_contract(
        &mut other_signature,
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts,
            ..Default::default()
        },
    );
    let mut other_definition = TraitDefinition {
        symbol: other,
        name: Identifier::from("Other"),
        ..Default::default()
    };
    program.push_trait_machine_signature(&mut other_definition, other_signature);
    program.push_trait_definition(other_definition);

    let root = program
        .expression_table
        .insert_tree(&Expression::Call(Box::new(CallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: None,
            target_symbol: predicate,
            target: Identifier::from("predicate"),
            arguments: vec![unresolved_value()].into(),
            evidence_arguments: Vec::new().into(),
            operational_acknowledgement: Default::default(),
        })));
    let limit = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::from("limit")],
            parameters.handle_at(1),
            parameters.handle_at(1),
        )));
    let domain_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: bound,
        name: Identifier::from("bound"),
        kind: TypeParameterKind::Const {
            type_reference: integer,
        },
        ..Default::default()
    }]);
    let domain_identity = program.semantic_domains.intern("root::AtLimit");
    program.push_domain_definition(DomainDefinition {
        symbol: at_limit,
        name: Identifier::from("AtLimit"),
        type_parameters: domain_parameters,
        target_type: integer,
        semantic_id: domain_identity,
        ..Default::default()
    });
    let limit_argument = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(limit));
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                symbol: at_limit,
                name: Identifier::from("AtLimit"),
                arguments: vec![limit_argument],
                semantic_id: domain_identity,
                ..Default::default()
            })]);
    let bounded = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: integer,
            constraints,
        });
    let mut signature = StateSignature {
        symbol: law,
        name: Identifier::from("law"),
        ..Default::default()
    };
    for (position, (name, type_reference)) in
        [("value", formal), ("limit", integer), ("amount", bounded)]
            .into_iter()
            .enumerate()
    {
        program.push_state_signature_parameter(
            &mut signature,
            StateParameter {
                symbol: parameters.handle_at(position as u32),
                name: Identifier::from(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(root)]);
    program.push_state_signature_contract(
        &mut signature,
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts,
            ..Default::default()
        },
    );
    let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: t,
        name: Identifier::from("T"),
        ..Default::default()
    }]);
    let mut definition = TraitDefinition {
        symbol: generic,
        name: Identifier::from("Generic"),
        type_parameters,
        ..Default::default()
    };
    let mut predicate_signature = StateSignature {
        symbol: predicate,
        name: Identifier::from("predicate"),
        return_type: boolean,
        ..Default::default()
    };
    program.push_state_signature_parameter(
        &mut predicate_signature,
        StateParameter {
            symbol: predicate_value,
            name: Identifier::from("value"),
            type_reference: formal,
            ..Default::default()
        },
    );
    program.push_trait_machine_signature(&mut definition, predicate_signature);
    program.push_trait_machine_signature(&mut definition, signature);
    program.push_trait_definition(definition);
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles([unit]);
    program.push_conformance(Conformance {
        symbol: implementation,
        trait_name: Identifier::from("Generic"),
        trait_symbol: generic,
        arguments,
        ..Default::default()
    });
    program.prepare_applied_callable_signatures().unwrap();
    let applied = program
        .prepared_callable_signatures()
        .find(|signature| signature.symbol == law)
        .unwrap();
    let actuals = program.state_signature_parameters(applied);
    assert_ne!(actuals[0].symbol, parameters.handle_at(0));
    assert_ne!(actuals[0].symbol, other_value);
    assert_eq!(
        program.normalized_type_identity(actuals[0].type_reference),
        program.normalized_type_identity(unit)
    );
    let contract = &program.state_signature_contracts(applied)[0];
    let applied_root = program.proof_facts.span_or_empty(contract.facts)[0]
        .as_expression()
        .copied()
        .unwrap();
    assert_ne!(applied_root, root);
    let call = program.expression_table.call_node(applied_root).unwrap();
    let value = program.expression_table.expression_handles(call.arguments)[0];
    let TypeReferenceNode::Constrained { constraints, .. } = program
        .type_reference_table
        .type_reference(actuals[2].type_reference)
    else {
        panic!("prepared parameter lost its complete constraint");
    };
    let TypeConstraintNode::Domain(domain) =
        &program.type_reference_table.constraints(*constraints)[0]
    else {
        panic!("prepared parameter lost its domain application");
    };
    assert_eq!(domain.symbol, at_limit);
    let [argument] = domain.arguments.as_slice() else {
        panic!("prepared domain lost its complete static argument");
    };
    let TypeReferenceNode::ConstExpression(copied_limit) =
        program.type_reference_table.type_reference(*argument)
    else {
        panic!("prepared domain lost its dependent expression");
    };
    let copied_limit = *copied_limit;
    assert_ne!(copied_limit, limit);
    let path = program
        .expression_table
        .name_path_node(copied_limit)
        .unwrap();
    assert_eq!(path.symbol, actuals[1].symbol);
    assert_ne!(path.symbol, parameters.handle_at(1));
    let facts = Arena::default();
    let owners = OwnerEnvironmentIndex::for_preparation(&program);
    assert_eq!(
        source_declaration_owner(&program, applied_root, &owners),
        Some(law)
    );
    let infer = |owner, expression| {
        checked_expression_type_reference_from_exact_owner(
            &program,
            &facts,
            owner,
            expression,
            Some(&owners),
        )
        .map(|reference| program.normalized_type_identity(reference))
    };
    assert_eq!(
        infer(applied_root, value),
        Some(program.normalized_type_identity(unit))
    );
    assert_ne!(
        infer(applied_root, value),
        Some(program.normalized_type_identity(boolean))
    );
    assert_eq!(
        infer(other_root, other_root),
        Some(program.normalized_type_identity(boolean))
    );
    assert_eq!(
        infer(copied_limit, copied_limit),
        Some(program.normalized_type_identity(integer))
    );
    assert!(owners.containing_contract_facts(applied_root).is_empty());
    assert!(owners.containing_contract_facts(copied_limit).is_empty());
}
