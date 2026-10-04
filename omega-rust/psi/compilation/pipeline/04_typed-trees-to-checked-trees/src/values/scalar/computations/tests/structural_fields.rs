use super::{CheckedScalarComputationKind, CheckedScalarExpressionRole, PrimitiveType};
use crate::values::scalar::computations::normal_return;
use crate::values::scalar::computations::tests::checked_source;
use checked_trees::CheckedStructuralAccess;

#[test]
fn local_record_reads_retain_primitive_carriers() {
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
        ("f32", PrimitiveType::F32),
        ("f64", PrimitiveType::F64),
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
            receiver_value,
            subject,
            field,
        } = &node.kind
        else {
            panic!("{name} field is an authored read, not a synthetic scalar parameter");
        };
        let source = validation::structural_scalar_field(
            &checked.typed,
            machine.symbol,
            state.symbol,
            1,
            *source_expression,
        )
        .expect("exact source");
        assert_eq!(node.primitive_type, primitive);
        assert!(!receiver_value.is_valid());
        assert_eq!(*field, source.field);
        assert_eq!(subject.structural_storage_root(), Some(source.root));
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
fn fresh_record_field_keeps_the_complete_receiver_and_exact_sibling_computation() {
    use checked_trees::{
        CheckedIntegerBinaryKind, CheckedScalarExpression, CheckedStructuralRecordFieldValue,
        CheckedStructuralValueKind,
    };
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    use typed_trees::statement::StatementNode;

    let checked = checked_source(
        "data Pair { first: u32; second: u32; }
         machine selected(divisor: u32) -> u32 requires divisor != 0 {
             Pair { first: 7u32, second: 42u32 / divisor }.first
         }",
        false,
    );
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    let [StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("the observation has no synthesized source local");
    };
    let ExpressionNode::Member(member) = checked.expression_table.expression(*expression) else {
        panic!("authored scalar observation");
    };
    let receiver = member.receiver;
    let plans = &checked.facts.values.scalar_computations;
    let returned = plans
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .expect("a field observation must not fold to its selected initializer");
    let CheckedScalarComputationKind::StructuralField {
        source_expression,
        receiver_value,
        subject,
        field,
    } = &plans.nodes.get(returned.root).kind
    else {
        panic!("return observes the completed structural value");
    };
    assert_eq!(*source_expression, *expression);
    assert!(receiver_value.is_valid());
    assert_eq!(
        subject.structural_storage_root(),
        Some(facts::PlaceRoot::Expression(receiver))
    );
    assert_eq!(subject.access, CheckedStructuralAccess::SharedBorrow);
    assert!(subject.path.is_empty());
    let values = &checked.facts.values.structural_values;
    let root = values
        .root_for_expression(state.symbol, 0, receiver)
        .expect("exact receiver has one construction root");
    assert_eq!(root.root, *receiver_value);
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .any(|shape| {
                shape.identity
                    == checked
                        .normalized_type_identity(root.type_reference)
                        .as_str()
            }),
        "a receiver without a local or formal still owns its structural type namespace"
    );
    let value = values.nodes.get(*receiver_value);
    assert_eq!(value.expression, receiver);
    let CheckedStructuralValueKind::Record { fields, .. } = value.kind else {
        panic!("complete receiver construction");
    };
    let fields = values
        .record_fields
        .span(fields)
        .expect("complete field roster");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].field, *field);
    for (ordinal, field) in fields.iter().enumerate() {
        let CheckedStructuralRecordFieldValue::Scalar(computation) = field.value else {
            panic!("both scalar initializers stay in the structural value");
        };
        let operand = plans
            .root_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::RecordField {
                    expression: receiver,
                    field_ordinal: u32::try_from(ordinal).unwrap(),
                },
            )
            .expect("each authored initializer keeps its computation root");
        assert_eq!(operand.root, computation);
        assert_eq!(plans.nodes.get(computation).authored_root, field.expression);
        assert_ne!(returned.root, computation);
    }
    let CheckedStructuralRecordFieldValue::Scalar(sibling) = fields[1].value else {
        panic!("exact sibling operand");
    };
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(fields[1].expression)
    else {
        panic!("authored exact division");
    };
    assert_eq!(binary.operator, BinaryOperator::Divide);
    let sibling_expression = match &plans.nodes.get(sibling).kind {
        CheckedScalarComputationKind::Value(expression)
        | CheckedScalarComputationKind::Apply { expression, .. } => expression,
        _ => panic!("sibling retains its arithmetic computation"),
    };
    assert!(matches!(
        sibling_expression,
        CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactDivide,
            primitive_type: PrimitiveType::U32,
            ..
        }
    ));
}

#[test]
fn fresh_nested_receiver_collects_its_complete_shape_without_unrelated_data() {
    let checked = checked_source(
        "data Child { value: u32; }
         data Envelope { child: Child; sibling: u32; }
         data Unused { other: u64; }
         machine observe(value: u32) -> u32 {
             Envelope { child: Child { value: value }, sibling: 9u32 }.child.value
         }",
        false,
    );
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    let root = checked
        .facts
        .values
        .scalar_computations
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .expect("nested receiver observation");
    let CheckedScalarComputationKind::StructuralField { receiver_value, .. } = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(root.root)
        .kind
    else {
        panic!("nested receiver remains an evaluated dependency");
    };
    let values = &checked.facts.values.structural_values;
    let value = values.nodes.get(receiver_value);
    let receiver = values
        .root_for_expression(state.symbol, 0, value.expression)
        .expect("complete outer receiver root");
    let checked_trees::CheckedStructuralValueKind::Record { fields, .. } = value.kind else {
        panic!("outer record construction");
    };
    let fields = values.record_fields.span(fields).expect("receiver fields");
    let shapes = &checked.facts.flow.terminal_scalar_graphs.structural_types;
    assert_eq!(shapes.len(), 2, "only Envelope and its Child are reachable");
    for reference in [receiver.type_reference, fields[0].type_reference] {
        assert!(
            shapes.iter().any(|shape| {
                shape.identity == checked.normalized_type_identity(reference).as_str()
            }),
            "the whole receiver and its nested field each retain their type"
        );
    }
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

#[test]
fn named_interval_local_fields_retain_ordered_guarded_tail() {
    let checked = checked_source(
        "domain u32::Limited(value) requires value <= 100;
         data Record { payload: u32 in Limited; }
         machine observe() -> u32 {
             let record: Record = Record { payload: 42 };
             transition {
                 record.payload < 10 -> low()
                 record.payload < 50 -> middle()
                 record.payload <= 100 -> high()
                 _ -> other()
             }
             state low() -> u32 { 1 }
             state middle() -> u32 { 2 }
             state high() -> u32 { 3 }
             state other() -> u32 { 4 }
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    for ordinal in 1..=3 {
        let root = checked
            .facts
            .values
            .scalar_computations
            .root_at(state.symbol, ordinal, CheckedScalarExpressionRole::Guard)
            .expect("qualified local guard computation");
        assert_eq!(
            checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root)
                .primitive_type,
            PrimitiveType::Bool
        );
    }
    let graphs = &checked.facts.flow.terminal_scalar_graphs;
    let tails = graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol)
        .collect::<Vec<_>>();
    assert_eq!(tails.len(), 1);
    let exits = graphs.guarded_exits.span(tails[0].arms).unwrap();
    assert_eq!(
        exits
            .iter()
            .map(|exit| exit.guard_statement_ordinal)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert!(tails[0].fallback.is_some());
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .is_some(),
        "record establishment and qualified reads compose with guarded successors"
    );
}
