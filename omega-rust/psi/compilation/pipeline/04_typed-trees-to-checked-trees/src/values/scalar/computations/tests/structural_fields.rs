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
