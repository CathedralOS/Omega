use super::*;
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
fn local_field_rejects_unrepresented_sibling_storage() {
    for (declarations, field, value) in [
        (
            "data Evidence { case Only; }",
            "extra [erased]: Evidence;",
            "Evidence::Only",
        ),
        ("", "extra: u64 in Wrapping;", "1 as u64 in Wrapping"),
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "{declarations}
             data Record {{ payload: u64; {field} }}
             machine observe(payload: u64) -> u64 {{
                 let record: Record = Record {{ payload: payload, extra: {value} }};
                 record.payload
             }}"
        ));
        assert!(
            local_scalar_record_field(&program, machine, state, 1, expression).is_none(),
            "the selected scalar field cannot admit unsupported sibling storage: {field}"
        );
    }
}

#[test]
fn local_field_retains_each_nested_declaration_and_allows_nested_siblings() {
    for (expression, path) in [
        ("record.payload", Vec::new()),
        ("record.extra.value", vec!["extra"]),
    ] {
        let (program, machine, state, expression) = typed_source(&format!(
            "data Nested {{ value: u64; }}
             data Record {{ payload: u64; extra: Nested; }}
             machine observe(payload: u64) -> u64 {{
                 let record: Record = Record {{ payload: payload, extra: Nested {{ value: 1 }} }};
                 {expression}
             }}"
        ));
        let source = local_scalar_record_field(&program, machine, state, 1, expression)
            .expect("nested storage is retained by complete record construction");
        assert_eq!(source.path, path);
    }
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
fn local_field_allows_closed_integer_ranges_without_losing_declaration_identity() {
    for field_type in ["u64[0..=100]", "i64[-10..11]", "u8[0..256]"] {
        let (program, machine, state, expression) = typed_source(&format!(
            "data Record {{ payload: {field_type}; extra: u64[0..=100]; }}
             machine observe(payload: {field_type}) -> {field_type} {{
                 let record: Record = Record {{ payload: payload, extra: 1 }};
                 record.payload
             }}"
        ));
        let read = local_scalar_record_field(&program, machine, state, 1, expression)
            .expect("closed bounded field");
        let DataMember::Field(field) = &program.data_members(&program.data_definitions()[0])[0]
        else {
            panic!("field")
        };
        assert_eq!(read.field, field.symbol);
        assert_eq!(
            Some(read.primitive_type),
            program.primitive_type_reference(field.type_reference)
        );
        assert!(matches!(
            program
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::Constrained { .. }
        ));
    }
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

#[test]
fn local_field_allows_plain_ieee_sibling_storage() {
    let (program, machine, state, expression) = typed_source(
        "data Record { payload: u64; floating: f64; }
         machine observe(payload: u64, floating: f64) -> u64 {
             let record: Record = Record { payload: payload, floating: floating };
             record.payload
         }",
    );
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_some());
}

fn typed_source(source: &str) -> (TypedTrees, SymbolHandle, SymbolHandle, ExpressionHandle) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("field tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("field syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("field resolution");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("field typing");
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
fn local_field_rejoins_its_exact_prior_declaration() {
    let (program, machine, state, expression) = fixture();
    let read =
        local_scalar_record_field(&program, machine, state, 1, expression).expect("local field");
    assert_eq!(read.local_statement_ordinal, 0);
    assert_eq!(read.primitive_type, PrimitiveType::U64);
    let owner = &program.machines()[0];
    let state = &program.machine_states(owner)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("local declaration");
    };
    assert_eq!(read.local, local.symbol);
    assert_eq!(read.type_reference, local.type_reference);
    let record = &program.data_definitions()[0];
    let DataMember::Field(field) = &program.data_members(record)[0] else {
        panic!("field");
    };
    assert_eq!(read.field, field.symbol);
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
fn local_field_rejects_a_foreign_field_laundered_through_the_record_roster() {
    let (mut program, machine, state, expression) = fixture();
    let read =
        local_scalar_record_field(&program, machine, state, 1, expression).expect("original field");
    let other = program
        .data_definitions()
        .iter()
        .find(|record| record.name.as_str() == "Other")
        .expect("other record");
    let DataMember::Field(other_field) = &program.data_members(other)[0] else {
        panic!("other field");
    };
    let foreign = other_field.symbol;
    let record = program
        .data_definitions()
        .iter()
        .find(|record| program.type_reference_symbol(read.type_reference) == record.symbol)
        .expect("record");
    let member_handle = record.members.start();
    let DataMember::Field(field) = program.data_members.get_mut(member_handle) else {
        panic!("record field");
    };
    field.symbol = foreign;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        panic!("member");
    };
    member.member_symbol = foreign;
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
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
fn local_field_rejects_mutable_and_conflicting_receiver_identity() {
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
    assert!(local_scalar_record_field(&program, machine, state, 1, expression).is_none());
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
    for field_type in ["u64 in Wrapping", "f64", "addr"] {
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
