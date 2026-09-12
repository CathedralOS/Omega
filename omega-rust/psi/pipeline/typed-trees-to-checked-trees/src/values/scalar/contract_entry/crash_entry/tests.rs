use super::{Reader, lower_machine_entry_crash_contract_expression};
use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedStructuralPredicatePathSegment,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn requirement(program: &TypedTrees) -> ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("one entry requirement")
}

fn read(program: &TypedTrees) -> Option<CheckedBooleanExpression> {
    lower_machine_entry_crash_contract_expression(
        program,
        &CheckedOperatorFacts::default(),
        &program.machines()[0],
        requirement(program),
        &[],
    )
}

fn field_mut(program: &mut TypedTrees, owner_position: usize) -> &mut DataField {
    let members = program.data_definitions()[owner_position].members;
    let DataMember::Field(field) = &mut program.tables.data_members.span_mut_or_empty(members)[0]
    else {
        panic!("first member is a field");
    };
    field
}

fn simple() -> TypedTrees {
    typed(
        "data Input { allowed: bool; } machine value(input: &Input) -> bool requires input.allowed { true }",
    )
}

#[test]
fn structural_entry_roots_preserve_access_independent_authored_and_dense_positions() {
    for root in ["Outer", "&Outer", "&mut Outer"] {
        let program = typed(&format!(
            "data Inner {{ allowed: bool; }} data Outer {{ inner: Inner; }} machine value(first: bool, input: {root}, mut last: bool) -> bool requires input.inner.allowed && last {{ true }}"
        ));
        assert_eq!(
            read(&program),
            Some(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: 1,
                    path: vec![
                        CheckedStructuralPredicatePathSegment::Field("inner".to_owned()),
                        CheckedStructuralPredicatePathSegment::Field("allowed".to_owned())
                    ],
                }),
                right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
            }),
            "{root}"
        );
        assert!(
            crate::values::lower_scalar_contract_predicate(
                &program,
                &CheckedOperatorFacts::default(),
                &program.machines()[0],
                requirement(&program),
                false,
                &mut 4096,
            )
            .is_none()
        );
    }
}

#[test]
fn structural_entry_field_identity_is_retained_without_a_spelling_lookup() {
    let mut program = simple();
    field_mut(&mut program, 0).identity = Some(7);
    assert_eq!(
        read(&program),
        Some(CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path: vec![CheckedStructuralPredicatePathSegment::Field(
                "#7".to_owned()
            )],
        })
    );
}

#[test]
fn structural_entry_self_uses_the_exact_attached_field_alias() {
    for receiver in ["self", "&self", "&mut self"] {
        let program = typed(&format!(
            "data Input {{ allowed: bool; }} machine Input::value({receiver}) -> bool requires self.allowed {{ true }}"
        ));
        assert_eq!(
            read(&program),
            Some(CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 0,
                path: vec![CheckedStructuralPredicatePathSegment::Field(
                    "allowed".to_owned()
                )],
            }),
            "{receiver}"
        );
        let parameters = program.machine_states(&program.machines()[0])[0].parameters;
        for wrong in [
            SymbolHandle::invalid(),
            program.data_definitions()[0].symbol,
        ] {
            let mut invalid = program.clone();
            let fake = invalid
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: wrong,
                    name: "Self".into(),
                });
            invalid
                .tables
                .state_parameters
                .span_mut_or_empty(parameters)[0]
                .type_reference = fake;
            assert!(read(&invalid).is_none());
        }
    }
}

#[test]
fn structural_entry_runtime_self_path_and_boolean_type_queries_agree() {
    for explicit_identity in [false, true] {
        let mut program = typed(
            "data Input { allowed: bool; } machine Input::value(&self) -> bool requires self.allowed { true }",
        );
        if explicit_identity {
            field_mut(&mut program, 0).identity = Some(7);
        }
        let machine = &program.machines()[0];
        let parameters = program.state_parameters(&program.machine_states(machine)[0]);
        let expression = requirement(&program);
        let mut path = Vec::new();
        let position = super::super::super::structural_parameter_field_path(
            &program, parameters, expression, &mut path,
        )
        .expect("runtime path reader joins exact self root and inherited field alias");
        assert_eq!(position, 0);
        assert_eq!(
            path,
            vec![CheckedStructuralPredicatePathSegment::Field(
                if explicit_identity { "#7" } else { "allowed" }.to_owned(),
            )]
        );
        let (_, _, field_type) = super::super::super::resolve_structural_parameter_path(
            &program, parameters, position, &path,
        )
        .expect("runtime structural type owner resolves machine Self to attachment");
        assert_eq!(
            program.primitive_type_reference(field_type),
            Some(typed_trees::types::PrimitiveType::Bool)
        );
        let expected = Some(CheckedBooleanExpression::StructuralParameterField {
            parameter_position: position,
            path,
        });
        assert_eq!(read(&program), expected, "strict entry reader");
        assert_eq!(
            super::super::lower_machine_entry_boolean_expression(
                &program,
                &CheckedOperatorFacts::default(),
                machine,
                expression,
                &[],
            ),
            expected,
            "runtime Boolean field lookup must preserve the same Self attachment and field type"
        );
    }
}

#[test]
fn structural_entry_requires_exact_live_receiver_local_field_symbols() {
    let program = typed(
        "data Input { allowed: bool; } data Other { allowed: bool; } machine value(input: &Input, other: &Other) -> bool requires input.allowed { true }",
    );
    assert!(read(&program).is_some());
    let root = requirement(&program);
    let ExpressionNode::Member(member) = program.expression_table.expression(root) else {
        panic!("field requirement");
    };
    let original = member.member_symbol;
    let foreign = match &program.data_members(&program.data_definitions()[1])[0] {
        DataMember::Field(field) => field.symbol,
        _ => panic!("field"),
    };
    for wrong in [
        SymbolHandle::invalid(),
        SymbolHandle::from_parts(original.arena_index(), original.generation() + 1),
        foreign,
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(root) else {
            unreachable!()
        };
        member.member_symbol = wrong;
        assert!(read(&invalid).is_none());
    }
    let mut invalid = program.clone();
    field_mut(&mut invalid, 0).name = "other".into();
    assert!(read(&invalid).is_none());
    let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(root) else {
        unreachable!()
    };
    member.member = "other".into();
    // Matching two corrupted spellings does not replace the symbol's original
    // declaration-local metadata.
    assert!(read(&invalid).is_none());
}

#[test]
fn structural_entry_rejects_missing_or_foreign_root_identity() {
    let program = typed(
        "data Input { allowed: bool; } machine value(input: &Input, other: &Input) -> bool requires input.allowed { true }",
    );
    let root = requirement(&program);
    let ExpressionNode::Member(member) = program.expression_table.expression(root) else {
        panic!("field requirement");
    };
    let receiver = member.receiver;
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    for wrong in [SymbolHandle::invalid(), parameters[1].symbol] {
        let mut invalid = program.clone();
        let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(receiver) else {
            panic!("formal receiver");
        };
        name.symbol = wrong;
        name.head_symbol = wrong;
        assert!(read(&invalid).is_none());
    }
    let mut renamed = program.clone();
    let parameter_span = renamed.machine_states(&renamed.machines()[0])[0].parameters;
    renamed
        .tables
        .state_parameters
        .span_mut_or_empty(parameter_span)[0]
        .name = "renamed".into();
    let ExpressionNode::Name(name) = renamed.expression_table.expression(receiver) else {
        panic!("formal receiver");
    };
    let members = name.members;
    renamed
        .expression_table
        .set_name_path_member_at_offset(members, 0, "renamed".into());
    assert!(read(&renamed).is_none());
}

#[test]
fn structural_entry_rejects_fake_builtin_types_and_stale_field_types() {
    let program = simple();
    for symbol in [
        SymbolHandle::invalid(),
        program.data_definitions()[0].symbol,
        program.machines()[0].symbol,
    ] {
        let mut invalid = program.clone();
        let fake = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: "bool".into(),
            });
        field_mut(&mut invalid, 0).type_reference = fake;
        assert!(read(&invalid).is_none());
    }
    let mut invalid = program.clone();
    field_mut(&mut invalid, 0).type_reference = TypeReferenceHandle::invalid();
    assert!(read(&invalid).is_none());

    let mut wrong_atom = typed(
        "data Input { allowed: bool; } machine value(input: &Input, number: u8) -> bool requires input.allowed { true }",
    );
    let parameters =
        wrong_atom.state_parameters(&wrong_atom.machine_states(&wrong_atom.machines()[0])[0]);
    let TypeReferenceNode::Named { symbol, .. } = wrong_atom
        .type_reference_table
        .type_reference(parameters[1].type_reference)
    else {
        panic!("u8 type");
    };
    let symbol = *symbol;
    let wrong = wrong_atom
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: "bool".into(),
        });
    field_mut(&mut wrong_atom, 0).type_reference = wrong;
    assert!(read(&wrong_atom).is_none());
}

#[test]
fn structural_entry_does_not_erase_intermediate_borrow_paths_or_generic_owners() {
    for source in [
        "data Inner { allowed: bool; } data Outer { inner: &Inner; } machine value(input: &Outer) -> bool requires input.inner.allowed { true }",
        "data Input<T> { allowed: bool; } machine value<T>(input: &Input<T>) -> bool requires input.allowed { true }",
    ] {
        let program = typed(source);
        assert!(read(&program).is_none());
    }
}

#[test]
fn structural_entry_boolean_operators_keep_exact_builtin_meaning() {
    let custom = typed(
        "data Input { allowed: bool; } boundary operator == bool::custom(left: bool, right: bool) -> bool; machine value(input: &Input) -> bool requires input.allowed == true { true }",
    );
    assert!(read(&custom).is_none());
    let unrelated = typed(
        "data Input { allowed: bool; } boundary operator == f64::custom(left: f64, right: f64) -> bool; machine value(input: &Input) -> bool requires input.allowed == true { true }",
    );
    assert!(read(&unrelated).is_some());
}

#[test]
fn structural_entry_expression_and_type_walks_are_bounded() {
    let program = simple();
    let root = requirement(&program);
    let mut cyclic = program.clone();
    let ExpressionNode::Member(member) = cyclic.expression_table.expression_mut(root) else {
        panic!("field requirement");
    };
    member.receiver = root;
    assert!(read(&cyclic).is_none());

    let mut deep = typed("machine value(unread: i32, flag: bool) -> bool requires flag { true }");
    let parameters = deep.machine_states(&deep.machines()[0])[0].parameters;
    let mut reference = deep.tables.state_parameters.span_or_empty(parameters)[0].type_reference;
    for _ in 0..65 {
        reference = deep
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: reference,
                constraints: Default::default(),
            });
    }
    deep.tables.state_parameters.span_mut_or_empty(parameters)[0].type_reference = reference;
    assert!(read(&deep).is_none());

    let operators = CheckedOperatorFacts::default();
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let mut reader = Reader {
        program: &program,
        operators: &operators,
        machine: Some(machine),
        owner: machine.symbol,
        parameters,
        remaining: 1,
    };
    assert!(reader.charge(64).is_none());
    assert!(reader.charge(0).is_none());
}

fn numeric(signature: &str, predicate: &str) -> TypedTrees {
    typed(&format!(
        "data Record {{ enabled: bool; }} machine value({signature}) -> bool\nrequires {predicate}\n{{ true }}"
    ))
}

#[test]
fn integer_entry_comparisons_reuse_total_landing_and_boolean_composition() {
    for primitive in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        for predicate in [
            "input > 0",
            "0 <= input",
            "input == other",
            "input != other",
            "flag && input < other",
            "!(input >= other) || flag",
            "(input == other) == true",
        ] {
            let program = numeric(
                &format!("input: {primitive}, other: {primitive}, flag: bool"),
                predicate,
            );
            assert!(read(&program).is_some(), "{primitive}: {predicate}");
            assert_eq!(
                crate::values::lower_scalar_contract_predicate(
                    &program,
                    &CheckedOperatorFacts::default(),
                    &program.machines()[0],
                    requirement(&program),
                    false,
                    &mut 4096,
                ),
                read(&program),
                "scalar and crash predicates share the same primitive meaning"
            );
        }
    }
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        let program = numeric(&format!("mut input: i32{policy}"), "input > 0");
        assert!(read(&program).is_some(), "total comparison {policy}");
    }
}

#[test]
fn integer_entry_comparisons_keep_mixed_authored_and_dense_positions() {
    use checked_trees::CheckedScalarExpression;
    use typed_trees::types::PrimitiveType;

    let program = numeric(
        "flag: bool, record: &Record, left: i32, mut right: i32",
        "left < right",
    );
    let Some(CheckedBooleanExpression::IntegerComparison { left, right, .. }) = read(&program)
    else {
        panic!("direct integer comparison");
    };
    assert_eq!(
        *left,
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I32
        }
    );
    assert_eq!(
        *right,
        CheckedScalarExpression::Parameter {
            position: 2,
            primitive_type: PrimitiveType::I32
        }
    );
    let program = numeric(
        "flag: bool, record: &Record, input: i32",
        "record.enabled && input > 0",
    );
    assert!(matches!(
        read(&program),
        Some(CheckedBooleanExpression::And { .. })
    ));
}

#[test]
fn integer_entry_comparisons_reject_unsupported_terms_and_bad_landings() {
    for (signature, predicate) in [
        ("input: i32", "input + 1 > 0"),
        ("input: i32 in Trapping", "input + 1 > 0"),
        ("input: i32 in Trapping", "input / 0 > 0"),
        ("input: i32", "(input as i64) > 0"),
        ("input: u8", "input > 256"),
        ("input: i32, other: u32", "input > other"),
        ("input: f64", "input > 0"),
    ] {
        assert!(
            read(&numeric(signature, predicate)).is_none(),
            "{signature}: {predicate}"
        );
    }
    let program = typed(
        "machine value(input: i32) -> bool requires input > cost() { true } machine cost() -> i32 { 1 }",
    );
    assert!(read(&program).is_none());
}

#[test]
fn integer_entry_comparisons_require_exact_live_operand_and_formal_identity() {
    let program = numeric("input: i32, other: i32", "input > 0");
    let root = requirement(&program);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(root) else {
        panic!("comparison");
    };
    let operand = binary.left;
    let parameters = program.machine_states(&program.machines()[0])[0].parameters;
    let original = program.tables.state_parameters.span_or_empty(parameters)[0].symbol;
    let other = program.tables.state_parameters.span_or_empty(parameters)[1].symbol;
    for wrong in [
        SymbolHandle::invalid(),
        other,
        SymbolHandle::from_parts(original.arena_index(), original.generation() + 1),
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(operand) else {
            panic!("entry operand");
        };
        name.symbol = wrong;
        name.head_symbol = wrong;
        assert!(read(&invalid).is_none());
    }
    let mut invalid = program.clone();
    let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(operand) else {
        panic!("entry operand");
    };
    name.head_symbol = other;
    assert!(read(&invalid).is_none());
    for wrong in [
        ExpressionHandle::invalid(),
        root,
        ExpressionHandle::from_parts(operand.arena_index(), operand.generation() + 1),
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Binary(binary) = invalid.expression_table.expression_mut(root) else {
            panic!("comparison");
        };
        binary.left = wrong;
        assert!(read(&invalid).is_none());
    }
    let mut invalid = program.clone();
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(parameters)[0]
        .name = "renamed".into();
    assert!(read(&invalid).is_none());
    let mut invalid = program.clone();
    let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(operand) else {
        panic!("entry operand");
    };
    name.symbol = SymbolHandle::invalid();
    name.head_symbol = SymbolHandle::invalid();
    let members = name.members;
    invalid
        .expression_table
        .set_name_path_member_at_offset(members, 0, "result".into());
    assert!(
        read(&invalid).is_none(),
        "no result slot in an invocation requirement"
    );

    let mut invalid =
        typed("machine value(input: i32) -> bool requires input > 0 { let local: i32 = 1; true }");
    let state = &invalid.machine_states(&invalid.machines()[0])[0];
    let local = invalid
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .expect("body local symbol");
    let root = requirement(&invalid);
    let ExpressionNode::Binary(binary) = invalid.expression_table.expression(root) else {
        panic!("comparison");
    };
    let operand = binary.left;
    let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(operand) else {
        panic!("entry operand");
    };
    name.symbol = local;
    name.head_symbol = local;
    let members = name.members;
    invalid
        .expression_table
        .set_name_path_member_at_offset(members, 0, "local".into());
    assert!(
        read(&invalid).is_none(),
        "body locals are not invocation operands"
    );
}

#[test]
fn integer_entry_comparisons_reject_false_builtin_types_and_unread_type_cycles() {
    let program = numeric("unread: u32, input: i32", "input > 0");
    let parameters = program.machine_states(&program.machines()[0])[0].parameters;
    let references = program
        .tables
        .state_parameters
        .span_or_empty(parameters)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect::<Vec<_>>();
    let TypeReferenceNode::Named {
        symbol: wrong_atom, ..
    } = program.type_reference_table.type_reference(references[0])
    else {
        panic!("u32 type");
    };
    for symbol in [
        SymbolHandle::invalid(),
        *wrong_atom,
        program.data_definitions()[0].symbol,
    ] {
        let mut invalid = program.clone();
        let fake = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: "i32".into(),
            });
        invalid
            .tables
            .state_parameters
            .span_mut_or_empty(parameters)[1]
            .type_reference = fake;
        assert!(read(&invalid).is_none());
    }
    for position in [0, 1] {
        for mutation in 0..4 {
            let mut invalid = program.clone();
            let reference = references[position];
            let replacement = match mutation {
                0 => TypeReferenceHandle::invalid(),
                1 => TypeReferenceHandle::from_parts(
                    reference.arena_index(),
                    reference.generation() + 1,
                ),
                2 => {
                    let cycle =
                        invalid
                            .type_reference_table
                            .insert(TypeReferenceNode::Constrained {
                                base_type: reference,
                                constraints: Default::default(),
                            });
                    invalid.type_reference_table.substitute_node(
                        cycle,
                        TypeReferenceNode::Constrained {
                            base_type: cycle,
                            constraints: Default::default(),
                        },
                    );
                    cycle
                }
                3 => {
                    let mut deep = reference;
                    for _ in 0..65 {
                        deep =
                            invalid
                                .type_reference_table
                                .insert(TypeReferenceNode::Constrained {
                                    base_type: deep,
                                    constraints: Default::default(),
                                });
                    }
                    deep
                }
                _ => unreachable!(),
            };
            invalid
                .tables
                .state_parameters
                .span_mut_or_empty(parameters)[position]
                .type_reference = replacement;
            assert!(
                read(&invalid).is_none(),
                "parameter {position}, type mutation {mutation}"
            );
        }
    }
}

#[test]
fn integer_entry_comparisons_keep_authored_operator_meaning() {
    for (primitive, admitted) in [("i32", false), ("f64", true)] {
        let program = typed(&format!(
            "boundary operator > {primitive}::custom(left: {primitive}, right: {primitive}) -> bool; machine value(input: i32) -> bool requires input > 0 {{ true }}"
        ));
        assert_eq!(read(&program).is_some(), admitted, "{primitive} operator");
    }
}

fn numeric_field(primitive: &str, root: &str, predicate: &str) -> TypedTrees {
    typed(&format!(
        "data Input {{ number: {primitive}; other: {primitive}; }} machine value(input: {root}, other: &Input, flag: bool) -> bool\nrequires {predicate}\n{{ true }}"
    ))
}

#[test]
fn integer_entry_fields_reuse_fixed_carrier_landing_and_total_comparisons() {
    for primitive in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        for predicate in [
            "input.number > 0",
            "0 <= input.number",
            "input.number < other.other",
            "input.number >= other.number",
            "input.number == other.number",
            "input.number != other.number",
            "flag && input.number > 0",
            "!(input.number < other.number) || flag",
            "(input.number == other.number) == true",
        ] {
            let program = numeric_field(primitive, "&Input", predicate);
            assert!(read(&program).is_some(), "{primitive}: {predicate}");
            assert!(
                crate::values::lower_scalar_contract_predicate(
                    &program,
                    &CheckedOperatorFacts::default(),
                    &program.machines()[0],
                    requirement(&program),
                    false,
                    &mut 4096,
                )
                .is_none(),
                "scalar contract namespace remains closed to structural fields"
            );
        }
    }
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        for root in ["Input", "&Input", "&mut Input"] {
            let program = numeric_field(&format!("i32{policy}"), root, "input.number > 0");
            assert!(read(&program).is_some(), "{root}, {policy}");
        }
    }
}

#[test]
fn integer_entry_fields_keep_nested_identity_and_mixed_scalar_ordinals() {
    use checked_trees::CheckedScalarExpression;
    use typed_trees::types::PrimitiveType;

    for root in ["Outer", "&Outer", "&mut Outer"] {
        let mut program = typed(&format!(
            "data Inner {{ number: i32; }} data Outer {{ inner: Inner; }} machine value(flag: bool, input: {root}, mut limit: i32) -> bool\nrequires input.inner.number < limit\n{{ true }}"
        ));
        field_mut(&mut program, 0).identity = Some(7);
        field_mut(&mut program, 1).identity = Some(9);
        let Some(CheckedBooleanExpression::IntegerComparison { left, right, .. }) = read(&program)
        else {
            panic!("nested field and direct scalar comparison");
        };
        assert_eq!(
            *left,
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: 1,
                path: vec![
                    CheckedStructuralPredicatePathSegment::Field("#9".to_owned()),
                    CheckedStructuralPredicatePathSegment::Field("#7".to_owned()),
                ],
                primitive_type: PrimitiveType::I32,
            }
        );
        assert_eq!(
            *right,
            CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::I32,
            }
        );
    }
    for receiver in ["self", "&self", "&mut self"] {
        let program = typed(&format!(
            "data Input {{ number: i32; }} machine Input::value({receiver}, limit: i32) -> bool\nrequires self.number < limit\n{{ true }}"
        ));
        let Some(CheckedBooleanExpression::IntegerComparison { left, right, .. }) = read(&program)
        else {
            panic!("exact self field comparison: {receiver}");
        };
        assert_eq!(
            *left,
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: 0,
                path: vec![CheckedStructuralPredicatePathSegment::Field(
                    "number".to_owned()
                )],
                primitive_type: PrimitiveType::I32,
            }
        );
        assert_eq!(
            *right,
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::I32,
            }
        );
    }
}

#[test]
fn integer_entry_fields_reject_wrong_namespace_and_field_identity() {
    let program = typed(
        "data Input { number: i32; } data Other { number: i32; } machine value(input: &Input, other: &Other) -> bool\nrequires input.number > 0\n{ true }",
    );
    let root = requirement(&program);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(root) else {
        panic!("comparison");
    };
    let operand = binary.left;
    let ExpressionNode::Member(member) = program.expression_table.expression(operand) else {
        panic!("field");
    };
    let receiver = member.receiver;
    let field_symbol = member.member_symbol;
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign field");
    };
    for wrong in [
        SymbolHandle::invalid(),
        foreign.symbol,
        SymbolHandle::from_parts(field_symbol.arena_index(), field_symbol.generation() + 1),
    ] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(operand)
        else {
            unreachable!();
        };
        member.member_symbol = wrong;
        assert!(read(&invalid).is_none(), "wrong exact field identity");
    }
    let mut invalid = program.clone();
    let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(operand) else {
        unreachable!();
    };
    member.member = "other".into();
    assert!(read(&invalid).is_none());
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    for wrong in [SymbolHandle::invalid(), parameters[1].symbol] {
        let mut invalid = program.clone();
        let ExpressionNode::Name(name) = invalid.expression_table.expression_mut(receiver) else {
            panic!("root name");
        };
        name.symbol = wrong;
        name.head_symbol = wrong;
        assert!(read(&invalid).is_none(), "wrong exact root identity");
    }
}

#[test]
fn integer_entry_fields_reject_unreadable_roots_and_invalid_type_chains() {
    for mut invalid in [simple(), numeric_field("i32", "&Input", "input.number > 0")] {
        let parameters = invalid.machine_states(&invalid.machines()[0])[0].parameters;
        let root_type = invalid.tables.state_parameters.span_or_empty(parameters)[0].type_reference;
        let mut write_only = invalid
            .type_reference_table
            .type_reference(root_type)
            .clone();
        let TypeReferenceNode::Reference { access, .. } = &mut write_only else {
            panic!("borrowed root");
        };
        *access = language_core::ReferenceAccess::WriteOnly;
        invalid
            .type_reference_table
            .substitute_node(root_type, write_only);
        assert!(
            read(&invalid).is_none(),
            "write-only root has no Boolean or integer entry observation"
        );
    }
    let program = numeric_field("i32", "&Input", "input.number > 0");
    let reference = match &program.data_members(&program.data_definitions()[0])[0] {
        DataMember::Field(field) => field.type_reference,
        _ => panic!("integer field"),
    };
    for mutation in 0..5 {
        let mut invalid = program.clone();
        let replacement = match mutation {
            0 => TypeReferenceHandle::invalid(),
            1 => {
                TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1)
            }
            2 => invalid
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: program.data_definitions()[0].symbol,
                    name: "i32".into(),
                }),
            3 => {
                let cycle = invalid
                    .type_reference_table
                    .insert(TypeReferenceNode::Constrained {
                        base_type: reference,
                        constraints: Default::default(),
                    });
                invalid.type_reference_table.substitute_node(
                    cycle,
                    TypeReferenceNode::Constrained {
                        base_type: cycle,
                        constraints: Default::default(),
                    },
                );
                cycle
            }
            4 => {
                let mut deep = reference;
                for _ in 0..65 {
                    deep = invalid
                        .type_reference_table
                        .insert(TypeReferenceNode::Constrained {
                            base_type: deep,
                            constraints: Default::default(),
                        });
                }
                deep
            }
            _ => unreachable!(),
        };
        field_mut(&mut invalid, 0).type_reference = replacement;
        assert!(read(&invalid).is_none(), "field type mutation {mutation}");
    }
}

#[test]
fn integer_entry_fields_reject_partial_syntax_and_incompatible_carriers() {
    for (primitive, predicate) in [
        ("i32", "input.number + 1 > 0"),
        ("i32 in Trapping", "input.number + 1 > 0"),
        ("i32 in Trapping", "input.number / 0 > 0"),
        ("i32", "(input.number as i64) > 0"),
        ("u8", "input.number > 256"),
        ("f64", "input.number > 0"),
        ("i32", "input.number > flag"),
    ] {
        assert!(
            read(&numeric_field(primitive, "&Input", predicate)).is_none(),
            "{primitive}: {predicate}"
        );
    }
    for predicate in [
        "input.signed > input.unsigned",
        "input.signed == input.unsigned",
    ] {
        let program = typed(&format!(
            "data Input {{ signed: i32; unsigned: u32; }} machine value(input: &Input) -> bool\nrequires {predicate}\n{{ true }}"
        ));
        assert!(read(&program).is_none(), "{predicate}");
    }
}

#[test]
fn integer_entry_fields_retain_selected_comparison_meaning() {
    for (primitive, admitted) in [("i32", false), ("f64", true)] {
        let program = typed(&format!(
            "data Input {{ number: i32; }} boundary operator > {primitive}::custom(left: {primitive}, right: {primitive}) -> bool; machine value(input: &Input) -> bool\nrequires input.number > 0\n{{ true }}"
        ));
        assert_eq!(read(&program).is_some(), admitted, "{primitive} operator");
    }
}
