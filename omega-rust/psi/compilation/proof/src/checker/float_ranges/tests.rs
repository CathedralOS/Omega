use super::float_range_at;
use crate::obligations::{FloatRange, ProofPlan};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, FloatLiteral, NamePath, TableCallExpression,
    TableMemberExpression,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment, TableCall, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct Fixture {
    program: TypedTrees,
    machine: Machine,
    state: State,
    root: ExpressionHandle,
    value: ExpressionHandle,
    other: ExpressionHandle,
    mutator: SymbolHandle,
    alias: SymbolHandle,
    replacement: ExpressionHandle,
    reference_type: TypeReferenceHandle,
}

impl Fixture {
    fn new() -> Self {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("Sample")),
                (SymbolKind::Machine, SymbolNameRef::Static("update")),
                (SymbolKind::Machine, SymbolNameRef::Static("mutate")),
            ]),
        ))
        .collect::<Vec<_>>();
        let data_symbol = children[children.len() - 3];
        let machine_symbol = children[children.len() - 2];
        let mutator_machine = children[children.len() - 1];
        let fields = SymbolTableBuilder::child_handles(builder.insert_children(
            data_symbol,
            [
                (SymbolKind::Field, SymbolNameRef::Static("value")),
                (SymbolKind::Field, SymbolNameRef::Static("other")),
            ],
        ))
        .collect::<Vec<_>>();
        let state_symbol = SymbolTableBuilder::child_handles(builder.insert_children(
            machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        ))
        .next()
        .unwrap();
        let mutator = SymbolTableBuilder::child_handles(builder.insert_children(
            mutator_machine,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        ))
        .next()
        .unwrap();
        let target_parameter = SymbolTableBuilder::child_handles(builder.insert_children(
            mutator,
            [(SymbolKind::Parameter, SymbolNameRef::Static("target"))],
        ))
        .next()
        .unwrap();
        let bindings = SymbolTableBuilder::child_handles(builder.insert_children(
            state_symbol,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("sample")),
                (SymbolKind::Parameter, SymbolNameRef::Static("replacement")),
                (SymbolKind::Local, SymbolNameRef::Static("alias")),
            ],
        ))
        .collect::<Vec<_>>();
        let parameter = bindings[0];
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        };
        let float_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: children[symbols::BuiltinTypeAtom::F32.ordinal()],
                name: Identifier::generated("f32"),
            });
        let mut definition = DataDefinition {
            symbol: data_symbol,
            name: Identifier::generated("Sample"),
            ..DataDefinition::default()
        };
        for (name, symbol) in [("value", fields[0]), ("other", fields[1])] {
            program.push_data_member(
                &mut definition,
                DataMember::Field(DataField {
                    symbol,
                    name: Identifier::generated(name),
                    type_reference: float_type,
                    ..DataField::default()
                }),
            );
        }
        program.push_data_definition(definition);
        let data_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: data_symbol,
                name: Identifier::generated("Sample"),
            });
        let reference_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: data_type,
                access: language_core::ReferenceAccess::Mutable,
                lifetime: None,
            });
        let mut state = State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..State::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated("sample"),
                type_reference: reference_type,
                ..StateParameter::default()
            },
        );
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: bindings[1],
                name: Identifier::generated("replacement"),
                type_reference: reference_type,
                ..StateParameter::default()
            },
        );
        let replacement =
            program
                .expression_table
                .insert_tree(&Expression::Name(NamePath::resolved(
                    vec![Identifier::generated("replacement")],
                    bindings[1],
                    bindings[1],
                )));
        let root = program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("sample")],
                parameter,
                parameter,
            )));
        let mut member = |symbol, name| {
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: root,
                    member_symbol: symbol,
                    member: Identifier::generated(name),
                    case_variant: None,
                }))
        };
        let value = member(fields[0], "value");
        let other = member(fields[1], "other");
        let target = program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("target")],
                target_parameter,
                target_parameter,
            )));
        let target_field =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: target,
                    member_symbol: fields[0],
                    member: Identifier::generated("value"),
                    case_variant: None,
                }));
        let poison = program.expression_table.insert(ExpressionNode::Float(
            FloatLiteral::parse("2.0f32").unwrap(),
        ));
        let mut mutator_state = State {
            symbol: mutator,
            name: Identifier::generated("entry"),
            return_type: float_type,
            ..State::default()
        };
        program.push_state_parameter(
            &mut mutator_state,
            StateParameter {
                symbol: target_parameter,
                name: Identifier::generated("target"),
                type_reference: reference_type,
                ..StateParameter::default()
            },
        );
        program.statement_table.push_statement(
            &mut mutator_state.statement_nodes,
            StatementNode::Assignment(TableAssignment {
                target: target_field,
                value: poison,
                ..TableAssignment::default()
            }),
        );
        program.statement_table.push_statement(
            &mut mutator_state.statement_nodes,
            StatementNode::Expression(poison),
        );
        let mut mutator_definition = Machine {
            symbol: mutator_machine,
            name: Identifier::generated("mutate"),
            body_is_present: true,
            ..Machine::default()
        };
        program.push_machine_state(&mut mutator_definition, mutator_state);
        program.push_machine(mutator_definition);
        Self {
            program,
            state,
            root,
            value,
            other,
            mutator,
            alias: bindings[2],
            replacement,
            reference_type,
            machine: Machine {
                symbol: machine_symbol,
                name: Identifier::generated("update"),
                body_is_present: true,
                ..Machine::default()
            },
        }
    }

    fn push(&mut self, statement: StatementNode) {
        self.program
            .statement_table
            .push_statement(&mut self.state.statement_nodes, statement);
    }

    fn store(&mut self, target: ExpressionHandle, value: &str) {
        let value = self
            .program
            .expression_table
            .insert(ExpressionNode::Float(FloatLiteral::parse(value).unwrap()));
        self.push(StatementNode::Assignment(TableAssignment {
            target,
            value,
            ..TableAssignment::default()
        }));
    }

    fn range(&self, statement_index: usize) -> Option<FloatRange> {
        let mut program = self.program.clone();
        let mut machine = self.machine.clone();
        program.push_machine_state(&mut machine, self.state.clone());
        program.push_machine(machine);
        float_range_at(
            &ProofPlan::new(&program),
            self.machine.symbol,
            self.state.symbol,
            statement_index,
            self.value,
        )
    }

    fn mutating_call(&mut self) -> TableCall {
        TableCall {
            authored_machine_application: Default::default(),
            target_symbol: self.mutator,
            target: Identifier::generated("mutate"),
            arguments: self
                .program
                .statement_table
                .insert_expression_handles([self.root]),
            ..TableCall::default()
        }
    }
}

#[test]
fn reaching_float_store_uses_latest_repair_at_the_observation() {
    let mut fixture = Fixture::new();
    fixture.store(fixture.value, "2.0f32");
    fixture.store(fixture.value, "0.5f32");
    fixture.store(fixture.other, "9.0f32");
    fixture.push(StatementNode::Expression(fixture.value));
    fixture.store(fixture.value, "7.0f32");
    assert_eq!(fixture.range(3), Some(FloatRange::closed(0.5)));
    // The consuming assignment's destination write occurs after its RHS read.
    assert_eq!(fixture.range(4), Some(FloatRange::closed(0.5)));
}

#[test]
fn reaching_float_store_does_not_borrow_a_forward_repair_or_wrong_format() {
    let mut fixture = Fixture::new();
    fixture.push(StatementNode::Expression(fixture.value));
    fixture.store(fixture.value, "0.5f32");
    assert_eq!(fixture.range(0), None);
    for spelling in ["0.5f64", "0.5", "1.0e50f32"] {
        fixture.store(fixture.value, spelling);
        fixture.push(StatementNode::Expression(fixture.value));
        assert_eq!(
            fixture.range(fixture.state.statement_nodes.len() - 1),
            None,
            "{spelling}"
        );
    }
}

#[test]
fn reaching_float_store_loses_unknown_parent_and_call_writes() {
    for invalidation in 0..3 {
        let mut fixture = Fixture::new();
        fixture.store(fixture.value, "0.5f32");
        let statement = match invalidation {
            0 => StatementNode::Assignment(TableAssignment {
                target: fixture.value,
                value: fixture.other,
                ..TableAssignment::default()
            }),
            1 => StatementNode::Assignment(TableAssignment {
                target: fixture.root,
                value: fixture.root,
                ..TableAssignment::default()
            }),
            _ => StatementNode::Call(fixture.mutating_call()),
        };
        fixture.push(statement);
        fixture.push(StatementNode::Expression(fixture.value));
        assert_eq!(fixture.range(2), None, "invalidation {invalidation}");
    }
}

#[test]
fn reaching_float_store_checks_sibling_argument_effects() {
    let mut fixture = Fixture::new();
    fixture.store(fixture.value, "0.5f32");
    let mutating_arguments = fixture
        .program
        .expression_table
        .insert_expression_handles([fixture.root]);
    let mutation = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            authored_machine_application: Default::default(),
            exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
            receiver: ExpressionHandle::invalid(),
            target_symbol: fixture.mutator,
            static_machine_parameter: SymbolHandle::invalid(),
            target: Identifier::generated("mutate"),
            static_requirement_dispatch: None,
            layout_query: None,
            machine_arguments: Box::default(),
            quotient_operation: None,
            private_layout_operation: None,
            arguments: mutating_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));
    let arguments = fixture
        .program
        .statement_table
        .insert_expression_handles([fixture.value, mutation]);
    fixture.push(StatementNode::Call(TableCall {
        authored_machine_application: Default::default(),
        arguments,
        ..TableCall::default()
    }));
    assert_eq!(fixture.range(1), None);
}

#[test]
fn reaching_float_store_invalidates_alias_rebinding_and_origin_writes() {
    for rebind in [false, true] {
        let mut fixture = Fixture::new();
        let original_value = fixture.value;
        let alias = fixture
            .program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("alias")],
                fixture.alias,
                fixture.alias,
            )));
        let Some(original) = fixture.program.expression_table.member_node(original_value) else {
            panic!("field place");
        };
        let original = original.clone();
        fixture.value = fixture
            .program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver: alias,
                ..original
            }));
        fixture.push(StatementNode::LocalData(TableLocalData {
            symbol: fixture.alias,
            name: Identifier::generated("alias"),
            type_reference: fixture.reference_type,
            initial_value: fixture.root,
            is_mutable: true,
            ..TableLocalData::default()
        }));
        fixture.store(fixture.value, "0.5f32");
        fixture.push(StatementNode::Expression(fixture.value));
        assert_eq!(fixture.range(2), Some(FloatRange::closed(0.5)));
        if rebind {
            fixture.push(StatementNode::Assignment(TableAssignment {
                target: alias,
                value: fixture.replacement,
                ..TableAssignment::default()
            }));
        } else {
            fixture.store(original_value, "2.0f32");
        }
        fixture.push(StatementNode::Expression(fixture.value));
        assert_eq!(fixture.range(4), None, "rebind={rebind}");
    }
}
