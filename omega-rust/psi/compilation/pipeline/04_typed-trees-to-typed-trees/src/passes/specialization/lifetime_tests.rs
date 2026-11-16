//! Specialization erases type selections, while each copied declaration keeps
//! its own lifetime binders and every call keeps its actual receiving regions.

use super::*;
use symbols::{
    BUILTIN_TYPE_COUNT, BuiltinTypeAtom, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
};
use typed_trees::signature::StateParameter;
use typed_trees::static_applications::StaticMachineLifetimeBinding;
use typed_trees::type_identity::LifetimeParameterIdentity as Region;

struct Source {
    program: TypedTrees,
    root: SymbolHandle,
    child: SymbolHandle,
    caller: SymbolHandle,
    applications: [StaticMachineApplicationHandle; 2],
    source_types: [TypeReferenceHandle; 3],
}

fn reference(
    program: &mut TypedTrees,
    referee: TypeReferenceHandle,
    owner: SymbolHandle,
    tag: &'static str,
) -> TypeReferenceHandle {
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            lifetime: Some(Identifier::from(tag)),
        });
    program
        .type_reference_table
        .retain_operand_lifetime_owner(reference, owner)
        .unwrap();
    program
        .type_reference_table
        .retain_operand_lifetimes(
            reference,
            [Region {
                declaration: owner,
                ordinal: 0,
            }],
        )
        .unwrap();
    reference
}

fn name(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
    program.expression_table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            symbol,
            head_symbol: symbol,
            ..Default::default()
        },
    ))
}

fn returned(program: &mut TypedTrees, symbol: SymbolHandle) -> HandleSpan<StatementNode> {
    let value = name(program, symbol);
    let target = program
        .statement_table
        .insert_transition_target(typed_trees::statement::TransitionTargetNode::Value(value));
    program
        .statement_table
        .insert_statements([StatementNode::Transition(
            typed_trees::statement::TableTransition {
                target,
                ..Default::default()
            },
        )])
}

fn source() -> Source {
    let mut symbols = SymbolTableBuilder::new();
    let module = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let top = symbols.insert_children(
        module,
        builtin_type_symbols().into_iter().chain([
            (SymbolKind::Machine, SymbolNameRef::Static("borrowed")),
            (SymbolKind::Machine, SymbolNameRef::Static("caller")),
        ]),
    );
    let root = top.handle_at(BUILTIN_TYPE_COUNT as u32);
    let caller = top.handle_at(BUILTIN_TYPE_COUNT as u32 + 1);
    let root_children = symbols.insert_children(
        root,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
            (SymbolKind::State, SymbolNameRef::Static("entry")),
        ],
    );
    let parameter = root_children.handle_at(0);
    let root_entry = root_children.handle_at(1);
    let root_formals = symbols.insert_children(
        root_entry,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("item")),
            (SymbolKind::Machine, SymbolNameRef::Static("child")),
        ],
    );
    let item = root_formals.handle_at(0);
    let child = root_formals.handle_at(1);
    let child_entry = symbols
        .insert_children(child, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
        .handle_at(0);
    let child_formals = symbols.insert_children(
        child_entry,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("captured")),
            (SymbolKind::Parameter, SymbolNameRef::Static("own")),
        ],
    );
    let caller_entry = symbols
        .insert_children(
            caller,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .handle_at(0);
    let caller_formals = symbols.insert_children(
        caller_entry,
        [
            (SymbolKind::Parameter, SymbolNameRef::Static("first")),
            (SymbolKind::Parameter, SymbolNameRef::Static("second")),
        ],
    );
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let byte = program
        .symbols
        .builtin_atom_symbol(BuiltinTypeAtom::Integer(
            numerics::integer_carrier::FixedInteger::unsigned(8),
        ))
        .unwrap();
    let byte_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: byte,
            name: Identifier::from("u8"),
        });
    let parameter_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: parameter,
            name: Identifier::from("T"),
        });
    let root_type = reference(&mut program, parameter_type, root, "captured");
    let captured_type = reference(&mut program, parameter_type, root, "captured");
    let own_type = reference(&mut program, parameter_type, child, "input");
    let parameters = program
        .data_type_parameters
        .insert_many([typed_trees::data::TypeParameter {
            symbol: parameter,
            name: Identifier::from("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }]);
    for (symbol, entry, label, lifetime, formals, result, returned_symbol) in [
        (
            root,
            root_entry,
            "borrowed",
            "captured",
            vec![(item, root_type)],
            root_type,
            item,
        ),
        (
            child,
            child_entry,
            "child",
            "input",
            vec![
                (child_formals.handle_at(0), captured_type),
                (child_formals.handle_at(1), own_type),
            ],
            own_type,
            child_formals.handle_at(1),
        ),
    ] {
        let formals = formals
            .into_iter()
            .map(|(symbol, type_reference)| StateParameter {
                symbol,
                name: Identifier::generated(program.symbols.name(symbol)),
                type_reference,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let formals = program.state_parameters.insert_many(formals);
        let statements = returned(&mut program, returned_symbol);
        let states = program.machine_states.insert_many([State {
            symbol: entry,
            parameters: formals,
            return_type: result,
            statement_nodes: statements,
            ..Default::default()
        }]);
        program.push_machine(Machine {
            symbol,
            name: Identifier::from(label),
            lifetime_parameters: vec![Identifier::from(lifetime)],
            type_parameters: parameters,
            states,
            body_is_present: true,
            ..Default::default()
        });
    }
    let mut applications = Vec::new();
    let mut statements = Vec::new();
    let mut formals = Vec::new();
    for (position, lifetime) in ["input", "other"].into_iter().enumerate() {
        let symbol = caller_formals.handle_at(position as u32);
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: byte_type,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from(lifetime)),
            });
        formals.push(StateParameter {
            symbol,
            type_reference: reference,
            ..Default::default()
        });
        let argument = StaticMachineArgument {
            symbol: byte,
            path: Box::new([Identifier::from("u8")]),
            ..Default::default()
        };
        let application = program.expression_table.retain_static_machine_application(
            root_entry,
            Identifier::from("borrowed"),
            std::slice::from_ref(&argument),
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = caller;
        program
            .expression_table
            .set_static_machine_application_lifetimes(
                application,
                root,
                &[StaticMachineLifetimeBinding {
                    parameter: Identifier::from("captured"),
                    argument: Identifier::from(lifetime),
                }],
            );
        let value = name(&mut program, symbol);
        let arguments = program.statement_table.insert_expression_handles([value]);
        statements.push(StatementNode::Call(typed_trees::statement::TableCall {
            authored_machine_application: application,
            target_symbol: root_entry,
            target: Identifier::from("borrowed"),
            machine_arguments: Box::new([argument]),
            arguments,
            discards_result: true,
            ..Default::default()
        }));
        applications.push(application);
    }
    let parameters = program.state_parameters.insert_many(formals);
    let statements = program.statement_table.insert_statements(statements);
    let states = program.machine_states.insert_many([State {
        symbol: caller_entry,
        parameters,
        statement_nodes: statements,
        ..Default::default()
    }]);
    program.push_machine(Machine {
        symbol: caller,
        name: Identifier::from("caller"),
        lifetime_parameters: vec![Identifier::from("input"), Identifier::from("other")],
        states,
        body_is_present: true,
        ..Default::default()
    });
    Source {
        program,
        root,
        child,
        caller,
        applications: applications.try_into().unwrap(),
        source_types: [root_type, captured_type, own_type],
    }
}

fn targets(program: &TypedTrees, caller: SymbolHandle) -> Vec<SymbolHandle> {
    let machine = program.machine_by_symbol(caller).unwrap();
    program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| {
            statement
                .as_call()
                .map(|call| program.symbols.get(call.target_symbol).parent)
        })
        .collect()
}

#[test]
fn copied_lifetimes_keep_root_captured_and_independent_child_owners() {
    let Source {
        mut program,
        root,
        child,
        caller,
        applications,
        source_types,
    } = source();
    prepare(&mut program).unwrap();
    assert!(program.deferred_refusal(caller).is_none());
    let target = targets(&program, caller)[0];
    let copied = program.machine_by_symbol(target).unwrap();
    assert_eq!(copied.lifetime_parameters, [Identifier::from("captured")]);
    assert_eq!(program.lifetime_declaration_owner(target), root);
    let copied_state = program.machine_states(copied).first().unwrap();
    let root_type = program.state_parameters(copied_state)[0].type_reference;
    assert_ne!(root_type, source_types[0]);
    assert_eq!(
        program.type_reference_table.operand_lifetimes(root_type),
        [Region {
            declaration: root,
            ordinal: 0
        }]
    );
    let (parameters, result) = program.machine_definition_types(copied).unwrap();
    assert_eq!(
        parameters[0], result,
        "input and result keep one declared lifetime relation"
    );
    let copied_child = program
        .machine_specializations
        .iter()
        .find(|row| row.template == child)
        .unwrap()
        .instance;
    assert_eq!(
        program.symbols.get(copied_child).parent,
        copied_state.symbol
    );
    let copied = program.machine_by_symbol(copied_child).unwrap();
    assert_eq!(copied.lifetime_parameters, [Identifier::from("input")]);
    assert_eq!(program.lifetime_declaration_owner(copied_child), child);
    let state = program.machine_states(copied).first().unwrap();
    for (parameter, owner) in program.state_parameters(state).iter().zip([root, child]) {
        assert_eq!(
            program
                .type_reference_table
                .operand_lifetimes(parameter.type_reference),
            [Region {
                declaration: owner,
                ordinal: 0
            }]
        );
        assert_eq!(
            program
                .type_reference_table
                .operand_lifetime_owner(parameter.type_reference),
            owner
        );
    }
    assert_eq!(
        program
            .type_reference_table
            .operand_lifetimes(state.return_type),
        [Region {
            declaration: child,
            ordinal: 0
        }]
    );
    assert!(
        program
            .static_application_lifetimes_for_declaration(applications[0], copied_child)
            .is_err(),
        "an enclosing call group cannot bind a child's same-spelled own lifetime"
    );
}

#[test]
fn copied_lifetimes_preserve_each_call_group_without_specializing_by_region() {
    let Source {
        mut program,
        caller,
        applications,
        root,
        ..
    } = source();
    prepare(&mut program).unwrap();
    let selected = targets(&program, caller);
    assert_eq!(selected[0], selected[1]);
    for (application, argument) in applications.into_iter().zip(["input", "other"]) {
        let group = program
            .static_application_lifetimes_for_declaration(application, selected[0])
            .unwrap();
        assert_eq!(
            *group,
            [StaticMachineLifetimeBinding {
                parameter: Identifier::from("captured"),
                argument: Identifier::from(argument)
            }]
        );
        let source = program
            .expression_table
            .static_machine_application(application);
        assert_eq!(source.lexical_owner, caller);
        assert_eq!(source.lifetime_binding_declaration, root);
    }
}

#[test]
fn copied_lifetimes_leave_implicit_invocation_groups_for_the_call_checker() {
    let Source {
        mut program,
        root,
        caller,
        applications,
        ..
    } = source();
    for application in applications {
        program
            .expression_table
            .set_static_machine_application_lifetimes(application, root, &[]);
    }
    prepare(&mut program).unwrap();
    assert!(program.deferred_refusal(caller).is_none());
    assert_eq!(targets(&program, caller)[0], targets(&program, caller)[1]);
    for application in applications {
        assert!(
            program
                .expression_table
                .static_machine_application_lifetimes(application)
                .is_empty()
        );
    }
}

#[test]
fn copied_lifetimes_refuse_missing_or_malformed_explicit_group_ownership() {
    for corruption in 0..4 {
        let Source {
            mut program,
            root,
            caller,
            applications,
            ..
        } = source();
        // Corrupt every call so no valid neighboring call can allocate a copy.
        for application in applications {
            let (declaration, parameter, argument) = match corruption {
                0 => (SymbolHandle::invalid(), "captured", "input"),
                1 => (root, "missing", "input"),
                2 => (root, "captured", "missing"),
                _ => (root, "captured", "input"),
            };
            let mut group = vec![StaticMachineLifetimeBinding {
                parameter: Identifier::from(parameter),
                argument: Identifier::from(argument),
            }];
            if corruption == 3 {
                group.push(group[0].clone());
            }
            program
                .expression_table
                .set_static_machine_application_lifetimes(application, declaration, &group);
        }
        let symbols_before = program.symbols.symbols().len();
        prepare(&mut program).unwrap();
        assert!(program.deferred_refusal(caller).is_some());
        assert_eq!(program.symbols.symbols().len(), symbols_before);
        assert!(
            !program
                .machine_specializations
                .iter()
                .any(|row| row.template == root)
        );
    }
}
