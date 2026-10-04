use super::{
    DataMember, ExpressionHandle, ExpressionNode, SymbolHandle, TypedTrees, structural_scalar_field,
};
use checked_trees::CheckedUnitStructuralPathSegment;
use typed_trees::statement::StatementNode;

fn fixture() -> (TypedTrees, SymbolHandle, SymbolHandle, ExpressionHandle) {
    let source = "data Record { payload: u64; flag: bool; }
        data Other { payload: u64; }
        machine observe(payload: u64, flag: bool) -> u64 {
            let record: Record = Record { payload: payload, flag: flag };
            record.payload
        }";
    typed_source(source)
}

#[test]
fn local_scalar_observation_keeps_neighboring_exclusive_reference_custody() {
    let (program, machine, state, expression) = typed_source(
        "data Record { destination: &mut u64; count: u64; }
         machine observe(destination: &mut u64) -> u64 {
             let record: Record = Record { destination: destination, count: 7 };
             record.count
         }",
    );
    let field = structural_scalar_field(&program, machine, state, 1, expression)
        .expect("observing metadata does not transfer the neighboring reference");
    assert!(field.path.is_empty());
    assert_eq!(field.primitive_type, super::PrimitiveType::U64);
}

#[test]
fn local_field_observes_nested_boolean_beside_array_and_sum_storage() {
    let (program, machine, state, expression) = typed_source(
        "data Token [copy] { position: u64; case Empty; case Full(value: u64); }
         data Output { tokens: [Token; 2]; accepted: bool; }
         data Parser { output: Output; bytes: [u8; 16]; }
         machine observe() -> bool {
             let mut parser: Parser;
             parser.output.accepted
         }",
    );
    let field = structural_scalar_field(&program, machine, state, 1, expression)
        .expect("scalar observation retains the established aggregate owner");
    assert_eq!(
        field.path,
        [CheckedUnitStructuralPathSegment::Field("output".into())]
    );
    assert_eq!(field.primitive_type, super::PrimitiveType::Bool);
}

#[test]
fn local_field_selects_a_literal_element_of_a_record_array() {
    let (program, machine, state, expression) = typed_source(
        "data Tally { count: u64; open: bool; }
         data Board { tallies: [Tally; 3]; total: u64; }
         machine observe() -> u64 {
             let mut board: Board;
             board.tallies[3 - 1].count
         }",
    );
    let field = structural_scalar_field(&program, machine, state, 1, expression)
        .expect("a literal element of a local record array is a field path");
    assert_eq!(
        field.path,
        [
            CheckedUnitStructuralPathSegment::Field("tallies".into()),
            CheckedUnitStructuralPathSegment::FixedIndex(2),
        ]
    );
    assert_eq!(field.primitive_type, super::PrimitiveType::U64);
}

#[test]
fn local_field_declines_an_element_outside_its_array_or_selected_at_runtime() {
    for source in [
        "data Tally { count: u64; }
         data Board { tallies: [Tally; 3]; }
         machine observe() -> u64 {
             let mut board: Board;
             board.tallies[3].count
         }",
        "data Tally { count: u64; }
         data Board { tallies: [Tally; 3]; }
         machine observe(index: u64) -> u64 {
             let mut board: Board;
             board.tallies[index].count
         }",
    ] {
        let (program, machine, state, expression) = typed_source(source);
        assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
    }
}

#[test]
fn local_field_does_not_relax_linear_owner_custody_for_a_boolean_observation() {
    let (program, machine, state, expression) = typed_source(
        "data Claim [linear] { value: u64; }
         data Record { claim: Claim; accepted: bool; }
         machine observe(incoming: Record) -> bool {
             let record: Record = incoming;
             record.accepted
         }",
    );
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_rejects_a_foreign_intermediate_declaration() {
    let (mut program, machine, state, expression) = typed_source(
        "data Child { value: u64; } data Other { child: Child; }
         data Record { child: Child; }
         machine observe() -> u64 {
             let record: Record = Record { child: Child { value: 1 } };
             record.child.value
         }",
    );
    let read =
        structural_scalar_field(&program, machine, state, 1, expression).expect("nested read");
    assert_eq!(
        read.path,
        [CheckedUnitStructuralPathSegment::Field("child".into())]
    );
    let other = program
        .data_definitions()
        .iter()
        .find(|record| record.name.as_str() == "Other")
        .unwrap();
    let DataMember::Field(field) = &program.data_members(other)[0] else {
        panic!("other child")
    };
    let foreign = field.symbol;
    let ExpressionNode::Member(leaf) = program.expression_table.expression(expression) else {
        panic!("leaf")
    };
    let parent = leaf.receiver;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(parent) else {
        panic!("carrier")
    };
    member.member_symbol = foreign;
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_does_not_use_open_or_invalid_ranges_as_plain_carriers() {
    for field_type in [
        "u64[0..=unknown()]",
        "u8[0..=256u8]",
        "f64[0..=100]",
        "u64[0..=100] in Wrapping",
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "data Record {{ payload: {field_type}; }}
             machine observe(payload: {field_type}) -> {field_type} {{
                 let record: Record = Record {{ payload: payload }};
                 record.payload
             }}"
        ));
        assert!(
            structural_scalar_field(&program, machine, state, 1, expression).is_none(),
            "{field_type}"
        );
    }
}

fn typed_source(source: &str) -> (TypedTrees, SymbolHandle, SymbolHandle, ExpressionHandle) {
    let program = crate::front_end::typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("authored observation machine");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[1]
    else {
        panic!("authored field return");
    };
    let (machine, state) = (machine.symbol, state.symbol);
    (program, machine, state, expression)
}

#[test]
fn local_field_rejects_foreign_same_spelled_field_identity() {
    let (mut program, machine, state, expression) = fixture();
    let other = program
        .data_definitions()
        .iter()
        .find(|record| record.name.as_str() == "Other")
        .expect("other record");
    let DataMember::Field(field) = &program.data_members(other)[0] else {
        panic!("other payload");
    };
    let foreign = field.symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        panic!("member");
    };
    member.member_symbol = foreign;
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_rejects_an_unattributed_member_expression() {
    let (mut program, machine, state, expression) = fixture();
    let detached_node = program.expression_table.expression(expression).clone();
    let detached = program.expression_table.insert(detached_node);
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_some());
    assert!(structural_scalar_field(&program, machine, state, 1, detached).is_none());
}

#[test]
fn local_field_rejects_early_and_reordered_declarations() {
    let (mut program, machine, state, expression) = fixture();
    assert!(structural_scalar_field(&program, machine, state, 0, expression).is_none());
    assert!(structural_scalar_field(&program, machine, state, 2, expression).is_none());
    let owner = program.machines()[0].clone();
    let statements = program.machine_states(&owner)[0].statement_nodes;
    program
        .statement_table
        .statements_mut(statements)
        .swap(0, 1);
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_observes_mutable_storage_and_rejects_conflicting_receiver_identity() {
    let (original, machine, state, expression) = fixture();
    let mut program = original.clone();
    let owner = program.machines()[0].clone();
    let statements = program.machine_states(&owner)[0].statement_nodes;
    let StatementNode::LocalData(local) =
        &mut program.statement_table.statements_mut(statements)[0]
    else {
        panic!("local");
    };
    local.is_mutable = true;
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_some());
    let mut program = original;
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        panic!("member");
    };
    let receiver = member.receiver;
    let ExpressionNode::Name(name) = program.expression_table.expression_mut(receiver) else {
        panic!("receiver");
    };
    name.head_symbol = machine;
    assert!(structural_scalar_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_preserves_float_carriers_and_rejects_addresses() {
    for (field_type, expected) in [
        ("f32", Some(super::PrimitiveType::F32)),
        ("f64", Some(super::PrimitiveType::F64)),
        ("addr", None),
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "data Record {{ payload: {field_type}; }}
             machine observe(payload: {field_type}) -> {field_type} {{
                 let record: Record = Record {{ payload: payload }};
                 record.payload
             }}"
        ));
        assert_eq!(
            structural_scalar_field(&program, machine, state, 1, expression)
                .map(|field| field.primitive_type),
            expected,
            "{field_type} retains its exact supported scalar carrier"
        );
    }
}

#[test]
fn local_field_retains_policy_without_excluding_plain_siblings() {
    use numerics::arithmetic::ArithmeticDomain;
    for (policy, domain) in [
        ("Wrapping", ArithmeticDomain::Wrapping),
        ("Saturating", ArithmeticDomain::Saturating),
        ("Trapping", ArithmeticDomain::Trapping),
    ] {
        for (member, expected) in [("payload", domain), ("plain", ArithmeticDomain::Exact)] {
            let (program, machine, state, expression) = typed_source(&format!(
                "data Child {{ payload: u64 in {policy}; plain: u64; }}
                 data Record {{ child: Child; }}
                 machine observe(payload: u64 in {policy}) -> u64 in {policy} {{
                     let record: Record = Record {{ child: Child {{ payload: payload, plain: 1 }} }};
                     record.child.{member}
                 }}"
            ));
            let field = structural_scalar_field(&program, machine, state, 1, expression)
                .unwrap_or_else(|| {
                    panic!("{policy} {member}: policy-only fields keep scalar storage")
                });
            assert_eq!(field.domain, expected);
            assert_eq!(
                field.path,
                [CheckedUnitStructuralPathSegment::Field("child".into())]
            );
        }
    }
}

#[test]
fn scalar_predicate_local_field_reads_preserve_declared_identity() {
    for (primitive, predicate, expected) in [
        ("u32", "value <= 100", super::PrimitiveType::U32),
        (
            "i32",
            "0 - 10 <= value && value <= 10",
            super::PrimitiveType::I32,
        ),
        ("u32", "value != 50", super::PrimitiveType::U32),
        ("bool", "value", super::PrimitiveType::Bool),
        ("i32", "value < 0 || value >= 10", super::PrimitiveType::I32),
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "domain {primitive}::Limited(value) requires {predicate};
             data Child {{ payload: {primitive} in Limited; }}
             data Record {{ child: Child; }}
             machine observe(payload: {primitive} in Limited) -> {primitive} {{
                 let record: Record = Record {{ child: Child {{ payload: payload }} }};
                 record.child.payload
             }}"
        ));
        let field = structural_scalar_field(&program, machine, state, 1, expression)
            .expect("an established predicate-domain field has scalar storage");
        assert_eq!(field.primitive_type, expected);
        assert_eq!(
            field.path,
            [CheckedUnitStructuralPathSegment::Field("child".into())]
        );
        assert_eq!(field.domain, numerics::arithmetic::ArithmeticDomain::Exact);
        let record = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Record")
            .expect("declared record");
        assert_eq!(
            program.type_reference_symbol(field.type_reference),
            record.symbol
        );
        assert!(structural_scalar_field(&program, machine, state, 0, expression).is_none());
    }
}

#[test]
fn scalar_predicate_local_field_rejects_unrepresented_domains() {
    for (definition, field_type) in [
        (
            "domain u32::Limited(value) requires value % 2 == 0;",
            "u32 in Limited",
        ),
        (
            "domain u32::Limited established by Authority::limited;
             trait Authority { machine limited() -> u32 ensures result in u32::Limited; }",
            "u32 in Limited",
        ),
        (
            "domain u32::Limited(value) requires value <= 100;",
            "u32 in Limited & Wrapping",
        ),
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "{definition}
             data Record {{ payload: {field_type}; }}
             machine observe(payload: {field_type}) -> u32 {{
                 let record: Record = Record {{ payload: payload }};
                 record.payload
             }}"
        ));
        assert!(
            structural_scalar_field(&program, machine, state, 1, expression).is_none(),
            "{field_type}: qualifications must have an exact supported storage interpretation"
        );
    }
}

#[test]
fn local_payload_observation_preserves_case_and_statement_identity() {
    let mut program = crate::front_end::typed_program(
        "data Choice [copy] { case Left(value: u64); case Right(value: u64); }
         machine finish(value: u64) -> u64 { value }
         machine observe(incoming: Choice) -> u64 {
             let saved: Choice = incoming;
             transition saved {
                 Choice::Left { value } -> finish(value)
                 Choice::Right { value } -> finish(value)
             }
         }",
    );
    let (machine, state, left_statement, left) = payload_argument(&program, 0);
    let (_, _, right_statement, right) = payload_argument(&program, 1);
    assert_ne!(left_statement, right_statement);
    let left_field = structural_scalar_field(&program, machine, state, left_statement, left)
        .expect("selected left payload");
    let right_field = structural_scalar_field(&program, machine, state, right_statement, right)
        .expect("selected right payload");
    assert_eq!(
        left_field.path,
        [CheckedUnitStructuralPathSegment::Case("Left".into())]
    );
    assert_eq!(
        right_field.path,
        [CheckedUnitStructuralPathSegment::Case("Right".into())]
    );
    assert_ne!(left_field.field, right_field.field);
    assert_eq!(left_field.root, right_field.root);
    assert_eq!(left_field.primitive_type, super::PrimitiveType::U64);
    assert!(structural_scalar_field(&program, machine, state, right_statement, left).is_none());
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(left) else {
        panic!("case-qualified payload projection");
    };
    member.member_symbol = right_field.field;
    assert!(structural_scalar_field(&program, machine, state, left_statement, left).is_none());
}

#[test]
fn local_payload_observation_keeps_nested_record_projection() {
    let program = crate::front_end::typed_program(
        "data Item [copy] { amount: u64; }
         data Choice [copy] { case Left(value: Item); case Right(value: Item); }
         machine finish(value: u64) -> u64 { value }
         machine observe(incoming: Choice) -> u64 {
             let saved: Choice = incoming;
             transition saved {
                 Choice::Left { value } -> finish(value.amount)
                 Choice::Right { value } -> finish(value.amount)
             }
         }",
    );
    let (machine, state, statement, expression) = payload_argument(&program, 0);
    let field = structural_scalar_field(&program, machine, state, statement, expression)
        .expect("record field beneath the selected payload");
    assert_eq!(
        field.path,
        [
            CheckedUnitStructuralPathSegment::Case("Left".into()),
            CheckedUnitStructuralPathSegment::Field("value".into()),
        ]
    );
    let item = program
        .data_definitions()
        .iter()
        .find(|declaration| declaration.name.as_str() == "Item")
        .expect("nested payload declaration");
    assert_eq!(
        program.type_reference_symbol(field.carrier_type_reference),
        item.symbol
    );
    assert_eq!(field.primitive_type, super::PrimitiveType::U64);
}

#[test]
fn saved_subject_fields_share_the_producer_without_copying_the_owner() {
    for properties in ["", "[copy]"] {
        let program = crate::front_end::typed_program(&format!(
            "data Choice {properties} {{ case Left(value: u64); case Right(value: u64); }}
             machine forward(incoming: Choice) -> Choice {{ incoming }}
             machine finish(value: u64) -> u64 {{ value }}
             machine observe(incoming: Choice) -> u64 {{
                 transition forward(incoming) {{
                     Choice::Left {{ value }} -> finish(value)
                     Choice::Right {{ value }} -> finish(value)
                 }}
             }}"
        ));
        let (machine, state, left_statement, left) = payload_argument(&program, 0);
        let (_, _, right_statement, right) = payload_argument(&program, 1);
        let left_field = structural_scalar_field(&program, machine, state, left_statement, left)
            .expect("saved left payload");
        let right_field = structural_scalar_field(&program, machine, state, right_statement, right)
            .expect("saved right payload");
        let facts::PlaceRoot::Expression(subject) = left_field.root else {
            panic!("an unnamed subject keeps expression identity");
        };
        assert_eq!(left_field.root, right_field.root);
        assert_eq!(left_field.statement_ordinal, left_statement);
        assert_eq!(right_field.statement_ordinal, left_statement);
        assert_eq!(
            right_field.path,
            [CheckedUnitStructuralPathSegment::Case("Right".into())]
        );
        assert_ne!(left_field.field, right_field.field);
        let owner = program
            .machine_by_symbol(machine)
            .expect("observation owner");
        let authored_state = &program.machine_states(owner)[0];
        assert_eq!(
            super::saved_dispatch_subject(
                &program,
                authored_state,
                left_statement as usize,
                subject
            ),
            Some((left_statement as usize, subject))
        );
        assert!(structural_scalar_field(&program, machine, state, right_statement, left).is_none());
    }
}

#[test]
fn saved_subject_field_rejects_an_identically_spelled_independent_call() {
    let mut program = crate::front_end::typed_program(
        "data Choice [copy] { case Left(value: u64); case Right(value: u64); }
         machine forward(incoming: Choice) -> Choice { incoming }
         machine finish(value: u64) -> u64 { value }
         machine observe(incoming: Choice) -> u64 {
             let independent: Choice = forward(incoming);
             transition forward(incoming) {
                 Choice::Left { value } -> finish(value)
                 Choice::Right { value } -> finish(value)
             }
         }",
    );
    let (machine, state, statement, expression) = payload_argument(&program, 0);
    let field = structural_scalar_field(&program, machine, state, statement, expression)
        .expect("original saved payload");
    let facts::PlaceRoot::Expression(subject) = field.root else {
        panic!("saved subject expression");
    };
    let owner = program
        .machine_by_symbol(machine)
        .expect("observation owner");
    let authored_state = &program.machine_states(owner)[0];
    let StatementNode::LocalData(independent) = &program
        .statement_table
        .statements(authored_state.statement_nodes)[0]
    else {
        panic!("independent call declaration");
    };
    let independent = independent.initial_value;
    assert!(
        program
            .expression_table
            .expressions_structurally_equal(subject, independent)
    );
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        panic!("selected payload member");
    };
    member.receiver = independent;
    assert!(structural_scalar_field(&program, machine, state, statement, expression).is_none());
}

fn payload_argument(
    program: &TypedTrees,
    arm_index: usize,
) -> (SymbolHandle, SymbolHandle, u32, ExpressionHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("authored observation machine");
    let state = &program.machine_states(machine)[0];
    let (statement_index, transition) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            StatementNode::Transition(transition) => Some((index, transition)),
            _ => None,
        })
        .nth(arm_index)
        .expect("authored payload transition arm");
    let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("terminal payload call");
    };
    (
        machine.symbol,
        state.symbol,
        u32::try_from(statement_index).expect("fixture statement index"),
        program.statement_table.expression_handles(*arguments)[0],
    )
}
