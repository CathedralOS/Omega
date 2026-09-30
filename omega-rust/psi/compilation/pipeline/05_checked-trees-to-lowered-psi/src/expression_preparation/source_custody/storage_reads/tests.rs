use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    ExpressionHandle, ExpressionNode, LoweringError, PrimitiveType, StatementNode,
    validate_entry_read_expression, validate_expression, validate_normal_result_read_expression,
};
use checked_trees::CheckedStructuralPredicatePathSegment;

#[test]
fn case_array_payload_element_reads_retain_their_projected_storage() {
    let checked = crate::front_end::checked_program(
        "data Packet { case Empty; case Samples(values: [i32; 2]); }
         machine read(packet: &Packet) -> i32 {
             transition packet {
                 Packet::Empty -> 0
                 Packet::Samples { values } -> finish(values[0], values[1])
             }
             state finish(first: i32, second: i32) -> i32 { first }
         }",
    );
    let plans = &checked.facts.values.scalar_expressions;
    let mut indexed_reads = 0;
    for (_, binding) in plans.source_bindings.iter() {
        let Some((_, retained)) =
            plans.bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        else {
            continue;
        };
        if !matches!(retained, CheckedScalarExpression::StructuralParameterField { path, .. }
            if path.iter().any(|segment| matches!(segment, CheckedStructuralPredicatePathSegment::FixedIndex(_))))
        {
            continue;
        }
        indexed_reads += 1;
        validate_expression(
            &checked,
            binding.state,
            binding.statement_ordinal,
            binding.expression,
            retained,
        )
        .expect("the authored case payload owns the indexed read");
        for component in 0..5 {
            let mut changed = retained.clone();
            let CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } = &mut changed
            else {
                panic!("projected payload element");
            };
            match component {
                0 => *parameter_position = 1,
                1 => path[0] = CheckedStructuralPredicatePathSegment::Case("Empty".into()),
                2 => path[1] = CheckedStructuralPredicatePathSegment::Field("other".into()),
                3 => path[2] = CheckedStructuralPredicatePathSegment::FixedIndex(2),
                _ => *primitive_type = PrimitiveType::U32,
            }
            assert!(
                validate_expression(
                    &checked,
                    binding.state,
                    binding.statement_ordinal,
                    binding.expression,
                    &changed
                )
                .is_err(),
                "changed component {component}"
            );
        }
    }
    assert_eq!(indexed_reads, 2);
}

#[test]
fn runtime_array_read_retains_its_exact_selector_binding() {
    let checked = crate::front_end::checked_program(
        r#"
        machine read(values: [i32; 3], index: u64, other: u64) -> i32
        requires index <= 2 {
            values[index]
        }
    "#,
    );
    let (state, expression) = source(&checked);
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state, 0, CheckedScalarExpressionRole::Return)
        .expect("indexed return");
    validate_expression(&checked, state, 0, expression, retained)
        .expect("dynamic index is a selector read, not a fixed path segment");
    let mut changed = retained.clone();
    let CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } = &mut changed else {
        panic!("runtime indexed read");
    };
    **index = CheckedScalarExpression::Parameter {
        position: 1,
        primitive_type: PrimitiveType::U64,
    };
    assert!(
        validate_expression(&checked, state, 0, expression, &changed).is_err(),
        "a same-typed different selector cannot replace the authored index"
    );
}

fn indexed_case_fixture(nested: bool) -> CheckedTrees {
    let source = r#"
        boundary trait Output { machine observe(value: i64) reaches Output; }
        data Payload { value: i64; }
        data Plan { case At(offset: i64, width: i64); case Skip; }
        data Root { plans: [Plan; 4]; }
        machine Root::enter(&mut self) reaches Output {
            self.plans[0] = Plan::At { offset: 8, width: 4 };
            transition self.plans[0] {
                Plan::At { offset, width } -> emit((offset as i64 in Wrapping) + (width as i64 in Wrapping))
                Plan::Skip -> bad()
            }
            state emit(&mut self, value: i64 in Wrapping) { Output::observe(value as i64); }
            state bad(&mut self) { Output::observe(99); }
        }
    "#;
    let source = if nested {
        source
            .replace("offset: i64", "offset: Payload")
            .replace("offset: 8", "offset: Payload { value: 8 }")
            .replace("(offset as", "(offset.value as")
    } else {
        source.to_owned()
    };
    crate::front_end::checked_program(&source)
}

#[test]
fn indexed_case_payload_reads_reject_changed_root_index_case_field_or_carrier() {
    for nested in [false, true] {
        let checked = indexed_case_fixture(nested);
        let plans = &checked.facts.values.scalar_expressions;
        let (binding, retained) = plans.source_bindings.iter().find_map(|(_, binding)| {
            let (_, retained) = plans.bound_expression_at(binding.state, binding.statement_ordinal, binding.role)?;
            matches!(retained, CheckedScalarExpression::IntegerBinary { left, .. }
                if matches!(left.as_ref(), CheckedScalarExpression::StructuralParameterField { .. }))
                .then_some((binding, retained))
        }).expect("case arm payload computation");
        let validate = |candidate: &CheckedScalarExpression| {
            validate_expression(
                &checked,
                binding.state,
                binding.statement_ordinal,
                binding.expression,
                candidate,
            )
        };
        validate(retained).expect("authored payload path, including nested record fields");
        for component in 0..6 {
            let mut changed = retained.clone();
            let CheckedScalarExpression::IntegerBinary { left, .. } = &mut changed else {
                panic!("payload sum");
            };
            let CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } = left.as_mut()
            else {
                panic!("payload field");
            };
            match component {
                0 => *parameter_position = 1,
                1 => path[1] = CheckedStructuralPredicatePathSegment::FixedIndex(2),
                2 => path[2] = CheckedStructuralPredicatePathSegment::Case("Skip".into()),
                3 => path[3] = CheckedStructuralPredicatePathSegment::Field("width".into()),
                4 => {
                    path.remove(2);
                }
                _ => *primitive_type = PrimitiveType::U64,
            }
            assert!(
                matches!(
                    validate(&changed),
                    Err(LoweringError::Unsupported(
                        "scalar read differs from its authored binding or mutable place"
                    ))
                ),
                "nested={nested}, component={component}"
            );
        }
    }
}

fn fixture(body: &str, result: &str) -> CheckedTrees {
    fixture_with_declarations(body, result, "")
}

fn fixture_with_declarations(body: &str, result: &str, declarations: &str) -> CheckedTrees {
    let source = format!(
        "data Limits {{ limit: u64; spare: u64; allowed: bool; other: bool; }}
         {declarations}
         machine read(marker: u64, limits: Limits, alternate: Limits) -> {result} {{ {body} }}"
    );
    crate::front_end::checked_program(&source)
}

fn source(checked: &CheckedTrees) -> (symbols::SymbolHandle, ExpressionHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .expect("read machine");
    let state = &checked.machine_states(machine)[0];
    let StatementNode::Expression(expression) = checked
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("return")
    else {
        panic!("expression return");
    };
    (state.symbol, *expression)
}

fn ensures_left(checked: &CheckedTrees, owner: &str) -> (symbols::SymbolHandle, ExpressionHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == owner)
        .expect("contract owner");
    let contract = checked
        .machine_contracts(machine)
        .iter()
        .find(|contract| contract.kind == checked_trees::signature::SignatureContractKind::Ensures)
        .expect("normal guarantee");
    let [checked_trees::domain::ProofFact::Expression(expression)] =
        checked.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("expression guarantee");
    };
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(*expression) else {
        panic!("comparison guarantee");
    };
    (checked.machine_states(machine)[0].symbol, binary.left)
}

fn result_parameter(position: usize) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U64,
    }
}

#[test]
fn normal_result_reads_keep_reserved_slot_separate_from_parameters_and_locals() {
    let checked = crate::front_end::checked_program(
        "machine read(input: u64) -> u64 ensures result == input { input }",
    );
    let (state, expression) = ensures_left(&checked, "read");
    assert!(
        validate_normal_result_read_expression(&checked, state, expression, &result_parameter(1))
            .is_ok()
    );
    for forged in [
        result_parameter(0),
        result_parameter(2),
        CheckedScalarExpression::Local {
            position: 1,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::StorageRead {
            symbol: checked.machines()[0].symbol,
            primitive_type: PrimitiveType::U64,
        },
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I64,
        },
    ] {
        assert!(
            validate_normal_result_read_expression(&checked, state, expression, &forged).is_err(),
            "{forged:?}"
        );
    }
    let local = crate::front_end::checked_program(
        "machine read(input: u64) -> u64 { let result: u64 = input; result }",
    );
    let (local_state, local_expression) = source(&local);
    assert!(
        validate_normal_result_read_expression(
            &local,
            local_state,
            local_expression,
            &result_parameter(1)
        )
        .is_err()
    );

    let shadowed = crate::front_end::checked_program(
        "machine read(result: u64) -> u64 ensures result == result { result }",
    );
    let (state, expression) = ensures_left(&shadowed, "read");
    assert!(
        validate_normal_result_read_expression(&shadowed, state, expression, &result_parameter(0))
            .is_ok()
    );
    assert!(
        validate_normal_result_read_expression(&shadowed, state, expression, &result_parameter(1))
            .is_err()
    );
}

#[test]
fn normal_result_reads_reject_foreign_owner_and_forged_resolved_identity() {
    let checked = crate::front_end::checked_program(
        "machine read(input: u64) -> u64 ensures result == input { input }
         machine other(input: u64) -> u64 ensures result == input { input }",
    );
    let (state, expression) = ensures_left(&checked, "read");
    let (_, foreign) = ensures_left(&checked, "other");
    assert!(
        validate_normal_result_read_expression(&checked, state, foreign, &result_parameter(1))
            .is_err()
    );
    for component in 0..3 {
        let mut changed = checked.clone();
        let symbol = changed.machines()[0].symbol;
        let ExpressionNode::Name(mut path) = *changed.expression_table.expression(expression)
        else {
            panic!("reserved result");
        };
        match component {
            0 => path.symbol = symbol,
            1 => path.head_symbol = symbol,
            _ => {
                path.member_symbols = arena::HandleSpan::empty();
                changed
                    .typed
                    .expression_table
                    .push_name_path_member_symbol(&mut path.member_symbols, symbol);
            }
        }
        *changed.typed.expression_table.expression_mut(expression) = ExpressionNode::Name(path);
        assert!(
            validate_normal_result_read_expression(
                &changed,
                state,
                expression,
                &result_parameter(1)
            )
            .is_err(),
            "component {component}"
        );
    }
}

#[test]
fn normal_result_reads_do_not_reinterpret_mutable_post_state_as_entry() {
    let checked =
        crate::front_end::checked_program("machine read(mut input: u64) -> u64 { input }");
    let (state, expression) = source(&checked);
    assert!(
        validate_entry_read_expression(&checked, state, expression, &result_parameter(0)).is_ok()
    );
    assert!(
        validate_normal_result_read_expression(&checked, state, expression, &result_parameter(0))
            .is_err()
    );
}

fn validate_return(
    checked: &CheckedTrees,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (state, expression) = source(checked);
    validate_expression(checked, state, 0, expression, retained)
}

fn integer_field(position: u32, identity: &str) -> CheckedScalarExpression {
    CheckedScalarExpression::StructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.into(),
        )],
        primitive_type: PrimitiveType::U64,
    }
}

#[test]
fn projected_case_membership_replays_exact_root_field_index_and_case() {
    let checked = crate::front_end::checked_program(
        "data Color [copy] { case Red; case Blue; }
         data Palette { colors: [Color; 3]; spare: [Color; 3]; }
         machine read(palette: &Palette, alternate: &Palette) -> bool {
             palette.colors[1] in Color::Blue
         }",
    );
    let membership = |parameter_position, field: &str, element_index, case: &str| {
        CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StructuralCaseMembership {
                subject: checked_trees::CheckedStructuralParameterField {
                    parameter_position,
                    path: vec![
                        CheckedStructuralPredicatePathSegment::Field(field.into()),
                        CheckedStructuralPredicatePathSegment::FixedIndex(element_index),
                    ],
                },
                case: case.into(),
            },
        ))
    };
    assert!(validate_return(&checked, &membership(0, "colors", 1, "Blue")).is_ok());
    for forged in [
        membership(1, "colors", 1, "Blue"),
        membership(0, "spare", 1, "Blue"),
        membership(0, "colors", 0, "Blue"),
        membership(0, "colors", 3, "Blue"),
        membership(0, "colors", 1, "Red"),
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(true))),
    ] {
        assert!(validate_return(&checked, &forged).is_err(), "{forged:?}");
    }
}

#[test]
fn case_membership_receiver_rejoins_only_its_declaring_machine() {
    let checked = crate::front_end::checked_program(
        "data Choice [copy] { case Empty; case Full; }
         machine Choice::read(&self) -> bool { self in Choice::Empty }
         machine Choice::other(&self) -> bool { self in Choice::Empty }",
    );
    let state = checked.machine_states(&checked.machines()[0])[0].symbol;
    let root = super::super::locate(&checked, state, 0, CheckedScalarExpressionRole::Return)
        .expect("receiver return");
    let membership = CheckedScalarExpression::Boolean(Box::new(
        CheckedBooleanExpression::StructuralCaseMembership {
            subject: checked_trees::CheckedStructuralParameterField {
                parameter_position: 0,
                path: Vec::new(),
            },
            case: "Empty".into(),
        },
    ));
    assert!(validate_expression(&checked, state, 0, root.expression, &membership).is_ok());
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(root.expression)
    else {
        panic!("case expression");
    };
    let receiver = binary.left;
    for change_head in [false, true] {
        let mut forged = checked.clone();
        let foreign = forged.machines()[1].symbol;
        let ExpressionNode::Name(name) = forged.typed.expression_table.expression_mut(receiver)
        else {
            panic!("receiver name");
        };
        if change_head {
            name.head_symbol = foreign;
        } else {
            name.symbol = foreign;
        }
        assert!(validate_expression(&forged, state, 0, root.expression, &membership).is_err());
    }
}

#[test]
fn owned_field_requires_exact_resolved_member_and_parameter_symbols() {
    let checked = fixture("limits.limit", "u64");
    let (_, expression) = source(&checked);
    let ExpressionNode::Member(member) = checked.expression_table.expression(expression) else {
        panic!("member")
    };
    let receiver = member.receiver;
    let member_symbol = member.member_symbol;
    for symbol in [
        symbols::SymbolHandle::invalid(),
        symbols::SymbolHandle::from_parts(
            member_symbol.arena_index(),
            member_symbol.generation() + 1,
        ),
    ] {
        let mut changed = checked.clone();
        let ExpressionNode::Member(member) =
            changed.typed.expression_table.expression_mut(expression)
        else {
            panic!("member")
        };
        member.member_symbol = symbol;
        assert!(validate_return(&changed, &integer_field(1, "limit")).is_err());
    }
    let mut changed = checked.clone();
    let ExpressionNode::Name(name) = changed.typed.expression_table.expression_mut(receiver) else {
        panic!("parameter")
    };
    name.symbol = symbols::SymbolHandle::invalid();
    name.head_symbol = symbols::SymbolHandle::invalid();
    assert!(validate_return(&changed, &integer_field(1, "limit")).is_err());
}

#[test]
fn numbered_owned_field_uses_identity_instead_of_spelling() {
    let mut checked = fixture("limits.limit", "u64");
    let members = checked.data_definitions()[0].members;
    let checked_trees::data::DataMember::Field(field) =
        &mut checked.typed.tables.data_members.span_mut_or_empty(members)[0]
    else {
        panic!("field")
    };
    field.identity = Some(7);
    assert!(validate_return(&checked, &integer_field(1, "#7")).is_ok());
    assert!(validate_return(&checked, &integer_field(1, "limit")).is_err());
    assert!(validate_return(&checked, &integer_field(1, "#8")).is_err());
}

#[test]
fn entry_field_reads_rejoin_exact_borrowed_parameter_path_and_carrier() {
    let checked = crate::front_end::checked_program(
        "data Bounds { index: u64; other: u64; }
         machine read(bounds: &Bounds, alternate: &Bounds) -> u64 { bounds.index }",
    );
    let (state, expression) = source(&checked);
    let retained = integer_field(0, "index");
    assert!(validate_entry_read_expression(&checked, state, expression, &retained).is_ok());
    let mut wrong_type = retained.clone();
    if let CheckedScalarExpression::StructuralParameterField { primitive_type, .. } =
        &mut wrong_type
    {
        *primitive_type = PrimitiveType::I64;
    }
    for forged in [
        integer_field(1, "index"),
        integer_field(0, "other"),
        integer_field(2, "index"),
        wrong_type,
    ] {
        assert!(
            validate_entry_read_expression(&checked, state, expression, &forged).is_err(),
            "{forged:?}"
        );
    }
}

#[test]
fn entry_field_reads_reject_erased_and_duplicate_authored_roots() {
    for parameter_type in ["Bounds", "&Bounds"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Bounds {{ index: u64; }}
             machine read(bounds: {parameter_type}, alternate: {parameter_type}) -> u64 {{ bounds.index }}"
        ));
        let (state, expression) = source(&checked);
        let retained = integer_field(0, "index");
        assert!(validate_entry_read_expression(&checked, state, expression, &retained).is_ok());
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "read")
            .expect("read machine");
        let parameters = checked.machine_states(machine)[0].parameters;
        for duplicate in [false, true] {
            let mut changed = checked.clone();
            let parameters = changed
                .typed
                .tables
                .state_parameters
                .span_mut_or_empty(parameters);
            if duplicate {
                parameters[1].symbol = parameters[0].symbol;
            } else {
                parameters[0].relevance = language_core::BindingRelevance::Erased;
            }
            assert!(
                validate_entry_read_expression(&changed, state, expression, &retained).is_err(),
                "parameter type {parameter_type}, duplicate root {duplicate}"
            );
        }
    }
}
