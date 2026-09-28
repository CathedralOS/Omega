use super::{
    DataMember, ExpressionHandle, ExpressionNode, SymbolHandle, TypedTrees,
    local_scalar_record_field,
};
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
        local_scalar_record_field(&program, machine, state, 1, expression).expect("nested read");
    assert_eq!(read.path, ["child"]);
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
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
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
            local_scalar_record_field(&program, machine, state, 1, expression).is_none(),
            "{field_type}"
        );
    }
}

fn typed_source(source: &str) -> (TypedTrees, SymbolHandle, SymbolHandle, ExpressionHandle) {
    let program = crate::front_end::typed_program(source);
    let machine = &program.machines()[0];
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
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_rejects_an_unattributed_member_expression() {
    let (mut program, machine, state, expression) = fixture();
    let detached_node = program.expression_table.expression(expression).clone();
    let detached = program.expression_table.insert(detached_node);
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_some());
    assert!(local_scalar_record_field(&program, machine, state, 1, detached).is_none());
}

#[test]
fn local_field_rejects_early_and_reordered_declarations() {
    let (mut program, machine, state, expression) = fixture();
    assert!(local_scalar_record_field(&program, machine, state, 0, expression).is_none());
    assert!(local_scalar_record_field(&program, machine, state, 2, expression).is_none());
    let owner = program.machines()[0].clone();
    let statements = program.machine_states(&owner)[0].statement_nodes;
    program
        .statement_table
        .statements_mut(statements)
        .swap(0, 1);
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
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
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_some());
    let mut program = original;
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        panic!("member");
    };
    let receiver = member.receiver;
    let ExpressionNode::Name(name) = program.expression_table.expression_mut(receiver) else {
        panic!("receiver");
    };
    name.head_symbol = machine;
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
}

#[test]
fn local_field_does_not_erase_scalar_constraints_or_noninteger_carriers() {
    for field_type in ["f64", "addr"] {
        let (program, machine, state, expression) = typed_source(&format!(
            "data Record {{ payload: {field_type}; }}
             machine observe(payload: {field_type}) -> {field_type} {{
                 let record: Record = Record {{ payload: payload }};
                 record.payload
             }}"
        ));
        assert!(
            local_scalar_record_field(&program, machine, state, 1, expression).is_none(),
            "{field_type} needs its own retained scalar contract"
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
            let field = local_scalar_record_field(&program, machine, state, 1, expression)
                .unwrap_or_else(|| {
                    panic!("{policy} {member}: policy-only fields keep scalar storage")
                });
            assert_eq!(field.domain, expected);
            assert_eq!(field.path, ["child"]);
        }
    }
}
