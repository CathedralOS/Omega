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
            super::super::lower_machine_entry_scalar_contract_expression(
                &program,
                &CheckedOperatorFacts::default(),
                &program.machines()[0],
                requirement(&program),
                &[]
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
        machine,
        parameters,
        remaining: 1,
    };
    assert!(reader.charge(64).is_none());
    assert!(reader.charge(0).is_none());
}
