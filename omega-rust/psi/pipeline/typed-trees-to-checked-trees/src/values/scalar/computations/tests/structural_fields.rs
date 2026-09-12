use super::*;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

#[test]
fn local_record_reads_retain_boolean_and_fixed_integer_carriers() {
    for (name, primitive) in [
        ("bool", PrimitiveType::Bool),
        ("u8", PrimitiveType::U8),
        ("u16", PrimitiveType::U16),
        ("u32", PrimitiveType::U32),
        ("u64", PrimitiveType::U64),
        ("i8", PrimitiveType::I8),
        ("i16", PrimitiveType::I16),
        ("i32", PrimitiveType::I32),
        ("i64", PrimitiveType::I64),
    ] {
        let checked = checked_source(
            &format!(
                "data Record {{ payload: {name}; }}
            machine observe(payload: {name}) -> {name} {{
                let record: Record = Record {{ payload: payload }};
                record.payload
            }}"
            ),
            false,
        );
        let machine = &checked.machines()[0];
        let state = &checked.machine_states(machine)[0];
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine.symbol)
                .is_some(),
            "{name} read must retain the complete structural-establishment/scalar-completion machine"
        );
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state.symbol, 1, CheckedScalarExpressionRole::Return)
            .expect("field return root");
        let node = plans.nodes.get(root.root);
        let CheckedScalarComputationKind::StructuralField {
            source_expression,
            subject,
            field,
        } = &node.kind
        else {
            panic!("{name} field is an authored read, not a synthetic scalar parameter");
        };
        let source = validation::local_scalar_record_field(
            &checked.typed,
            machine.symbol,
            state.symbol,
            1,
            *source_expression,
        )
        .expect("exact source");
        assert_eq!(node.primitive_type, primitive);
        assert_eq!(*field, source.field);
        assert_eq!(
            subject.source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: source.local
            }
        );
        assert_eq!(subject.access, CheckedStructuralAccess::SharedBorrow);
        assert!(subject.path.is_empty());
        assert_eq!(
            subject.type_identity,
            checked
                .normalized_type_identity(source.type_reference)
                .as_str()
        );
        assert_eq!(node.authored_root, *source_expression);
        assert!(normal_return::boolean_result(plans, root.root).is_none());
    }
}

#[test]
fn local_record_read_keeps_its_declaration_across_owned_selection() {
    let checked = checked_source(
        "data Record { payload: u64; }
        data Choice { case Empty; case Some(value: u32); }
        machine observe(selected: bool, payload: u64) -> u64 {
            let record: Record = Record { payload: payload };
            let left: Choice = Choice::Some { value: 37 };
            let right: Choice = Choice::Empty;
            let result: Choice = match selected { true -> left, false -> right };
            record.payload
        }",
        false,
    );
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some(),
        "post-selection read must retain its whole machine plan"
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state.symbol, 4, CheckedScalarExpressionRole::Return)
        .expect("post-selection read");
    let CheckedScalarComputationKind::StructuralField {
        source_expression,
        subject,
        ..
    } = &plans.nodes.get(root.root).kind
    else {
        panic!("field node");
    };
    let source = validation::local_scalar_record_field(
        &checked.typed,
        machine.symbol,
        state.symbol,
        4,
        *source_expression,
    )
    .expect("prior record declaration");
    assert_eq!(source.local_statement_ordinal, 0);
    assert_eq!(
        subject.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: source.local
        }
    );
}

#[test]
fn record_fields_compose_with_integer_operands_and_call_arguments_in_order() {
    let checked = checked_source(
        "data Record { payload: u64; }
        machine identity(value: u64) -> u64 { value }
        machine observe(payload: u64) -> u64 {
            let record: Record = Record { payload: payload };
            let right: Record = Record { payload: 7 };
            identity(record.payload) ^ right.payload
        }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observe");
    let state = &checked.machine_states(machine)[0];
    let plans = &checked.facts.values.scalar_computations;
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some(),
        "ordered calls and two record roots retain a whole machine plan"
    );
    let root = plans
        .root_at(state.symbol, 2, CheckedScalarExpressionRole::Return)
        .expect("integer application");
    let CheckedScalarComputationKind::Apply { operands, .. } = plans.nodes.get(root.root).kind
    else {
        panic!("ordered application");
    };
    let [left, right] = plans.operands.span(operands).expect("operand span") else {
        panic!("binary operands");
    };
    let CheckedScalarComputationKind::Call { arguments, .. } = plans.nodes.get(*left).kind else {
        panic!("left call");
    };
    let [argument] = plans.operands.span(arguments).expect("call arguments") else {
        panic!("one argument");
    };
    let CheckedScalarComputationKind::StructuralField {
        source_expression: first,
        ..
    } = plans.nodes.get(*argument).kind
    else {
        panic!("read before call");
    };
    let CheckedScalarComputationKind::StructuralField {
        source_expression: second,
        ..
    } = plans.nodes.get(*right).kind
    else {
        panic!("read after call");
    };
    assert_ne!(argument, right);
    assert_ne!(first, second);
}

#[test]
fn boolean_record_read_remains_before_its_selective_call() {
    let checked = checked_source(
        "data Record { flag: bool; }
         machine identity(value: bool) -> bool { value }
         machine observe(flag: bool, other: bool) -> bool {
             let record: Record = Record { flag: flag };
             record.flag && identity(other)
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observe");
    let state = &checked.machine_states(machine)[0];
    let plans = &checked.facts.values.scalar_computations;
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some(),
        "selective calls retain a whole machine plan"
    );
    let root = plans
        .root_at(state.symbol, 1, CheckedScalarExpressionRole::Return)
        .expect("selective return");
    let CheckedScalarComputationKind::Select {
        condition,
        when_true,
        when_false,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        panic!("selective call graph");
    };
    assert!(matches!(
        plans.nodes.get(condition).kind,
        CheckedScalarComputationKind::StructuralField { .. }
    ));
    assert!(matches!(
        plans.nodes.get(when_true).kind,
        CheckedScalarComputationKind::Call { .. }
    ));
    assert_eq!(
        normal_return::boolean_result(plans, when_false),
        Some(false)
    );
    assert_eq!(normal_return::boolean_result(plans, root.root), None);
}
