use super::{ExpressionNode, SymbolHandle, computed_receiver_method_target, resolved};

#[test]
fn nested_receiver_guard_call_retains_declared_return_type() {
    let source = r#"
        data Items { count: u64; }
        data Request { items: Items; }
        machine Items::get_count(&self) -> u64 { self.count }
        machine has_items(request: &Request) -> bool {
            transition request.items.get_count() > 0 {
                true -> (true)
                false -> (false)
            }
        }
    "#;
    crate::front_end::typed_program_result(source)
        .expect("a projected receiver retains its method's explicit result type");
}

#[test]
fn nested_receiver_candidates_follow_declared_places() {
    for (receiver, admitted) in [
        ("request.items", true),
        ("request.inner.items", true),
        ("request.groups[0].items", true),
        ("local.items", true),
        ("self.request.items", true),
        ("request.missing", false),
        ("request.scalar", false),
    ] {
        let source = format!(
            r#"
            data Items {{ count: u64; }}
            data OtherItems {{ count: bool; }}
            data Group {{ items: Items; }}
            data Request {{ items: Items; inner: Group; groups: [Group; 2]; scalar: u64; }}
            data Owner {{ request: Request; }}
            machine Items::get_count(&self) -> u64 {{ self.count }}
            machine OtherItems::get_count(&self) -> bool {{ self.count }}
            machine get_count() -> u64 {{ 9 }}
            machine Owner::run(&self, request: &Request) {{
                let local: &Request = request;
                let result: u64 = {receiver}.get_count();
            }}
        "#
        );
        let typed = crate::front_end::typed_program(&source);
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Items::get_count")
            .expect("exact receiver method");
        let expected = if admitted {
            typed.machine_states(machine)[0].symbol
        } else {
            SymbolHandle::invalid()
        };
        let call = typed
            .expression_table
            .expression_entries()
            .find_map(|(_, node)| match node {
                ExpressionNode::Call(call) if call.target.as_str() == "get_count" => Some(call),
                _ => None,
            })
            .expect("projected receiver call");
        assert_eq!(call.target_symbol, expected, "{receiver}");
    }
}

#[test]
fn nested_receiver_guard_call_with_arguments_retains_boolean_return() {
    let source = r#"
        data Identifier { count: u64; }
        data Item { identifier: Identifier; }
        machine Identifier::matches(&self, name: &[u8]) -> bool { name.len == self.count }
        machine accepts(item: &mut Item) -> bool {
            transition item.identifier.matches("u64") == true {
                true -> (true)
                false -> (false)
            }
        }
    "#;
    crate::front_end::typed_program_result(source)
        .expect("borrowed nested receivers retain bool results and ordinary arguments");
}

#[test]
fn nested_receiver_method_rejects_conflicting_field_identity() {
    let mut program = crate::front_end::resolved_program(
        r#"
        data Items { count: u64; }
        data Request { items: Items; }
        data Other { items: Items; }
        machine Items::get_count(&self) -> u64 { self.count }
        machine read(request: &Request) -> u64 { request.items.get_count() }
        "#,
    );
    let foreign_owner = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .expect("foreign receiver");
    let foreign_field = program
        .data_members(foreign_owner.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .expect("foreign same-named field");
    let call = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, node)| match node {
            resolved::expression::ExpressionNode::Call(call) => Some(call.clone()),
            _ => None,
        })
        .expect("receiver call");
    let resolved::expression::ExpressionNode::Member(member) = program
        .tables
        .bodies
        .expressions
        .expression_mut(call.receiver)
    else {
        panic!("projected receiver");
    };
    member.member_symbol = foreign_field;
    let target = crate::expressions::expression::declared_receiver_method_target(
        &program,
        &program.tables.bodies.expressions,
        call.receiver,
        &call.target,
    );
    assert!(
        !target.is_valid(),
        "a foreign field cannot supply receiver ownership"
    );
}

#[test]
fn nested_receiver_owner_precedes_reference_source_preference() {
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;

    let text = r#"
        data Items { count: u64; }
        data Request { items: Items; }
        machine Items::get_count(&self) -> u64 { self.count }
        machine read(request: &Request) -> u64 { request.items.get_count() }
    "#;
    let mut sources = SourceMap::default();
    let base = sources
        .add(PathBuf::from("base.omg"), text.to_owned())
        .source_id;
    let extension = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("extension.omg"),
            text.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let program = crate::front_end::resolved_program_from_merged_source_map(
        sources,
        &[(extension, text), (base, text)],
    );
    let expressions = &program.tables.bodies.expressions;
    let call_from = |source_id| {
        expressions
            .iter_expressions()
            .find_map(|(_, node)| match node {
                resolved::expression::ExpressionNode::Call(call)
                    if call.target.source_span().source_id == source_id =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("source's projected call")
    };
    let base_call = call_from(base);
    let extension_call = call_from(extension);
    let expected_machine = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.as_str() == "Items::get_count"
                && machine.name.source_span().source_id == base
        })
        .expect("base nominal receiver method");
    let expected = program
        .machine_state(program.machine_state_handles(expected_machine.states)[0])
        .symbol;
    for target in [&base_call.target, &extension_call.target] {
        let actual = crate::expressions::expression::declared_receiver_method_target(
            &program,
            expressions,
            base_call.receiver,
            target,
        );
        assert_eq!(
            actual, expected,
            "receiver ownership precedes reference-source preference"
        );
    }
}

#[test]
fn nested_receiver_signature_free_lookup_preserves_ambiguity() {
    let program = crate::front_end::resolved_program(
        r#"
        data Items { count: u64; }
        data Request { items: Items; }
        machine Items::get_count(&self) -> u64 { self.count }
        machine Items::get_count(&self, extra: u64) -> u64 { extra }
        machine read(request: &Request) -> u64 { request.items.get_count() }
    "#,
    );
    let expressions = &program.tables.bodies.expressions;
    let call = expressions
        .iter_expressions()
        .find_map(|(_, node)| match node {
            resolved::expression::ExpressionNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("projected call");
    let target = resolved::name::DiagnosticName::generated("get_count");
    let actual = crate::expressions::expression::declared_receiver_method_target(
        &program,
        expressions,
        call.receiver,
        &target,
    );
    assert!(
        !actual.is_valid(),
        "an unselected overload cannot be guessed from its name"
    );
}

#[test]
fn computed_receiver_candidates_follow_exact_declared_result_types() {
    let cases = [
        ("identity(&mut self.cell)", true),
        ("identity(identity(&mut self.cell))", true),
        ("array(&mut self.cells)[0]", true),
        ("array(&mut self.cells)[index]", true),
        ("slice(values)[index]", true),
        ("bucket(&mut self.bucket).cell", true),
        ("identity(&mut self.cell).again()", true),
        ("array(&mut self.cells)", false),
        ("scalar()", false),
        ("unknown(&mut self.cell)", false),
        ("bucket(&mut self.bucket).missing", false),
    ];
    for (receiver, admitted) in cases {
        let source = format!(
            r#"
            data Cell {{}} data Bucket {{ cell: Cell; }}
            data Owner {{ cell: Cell; cells: [Cell; 2]; bucket: Bucket; }}
            machine Cell::read(&self) -> u64 {{ 1 }}
            machine Cell::again(&mut self) -> &mut Cell {{ self }}
            machine identity(value: &mut Cell) -> &mut Cell {{ value }}
            machine array(value: &mut [Cell; 2]) -> &mut [Cell; 2] {{ value }}
            machine slice(value: &mut [Cell]) -> &mut [Cell] {{ value }}
            machine bucket(value: &mut Bucket) -> &mut Bucket {{ value }}
            machine scalar() -> u64 {{ 1 }}
            machine read() -> u64 {{ 2 }}
            machine Owner::run(&mut self, values: &mut [Cell], index: u64) {{
                let result: u64 = {receiver}.read();
            }}
        "#
        );
        let typed = crate::front_end::typed_program(&source);
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Owner::run")
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let typed_trees::statement::StatementNode::LocalData(result) =
            &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("result");
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression(result.initial_value)
        else {
            panic!("call");
        };
        let callee = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Cell::read")
            .expect("callee");
        let expected = if admitted {
            typed.machine_states(callee)[0].symbol
        } else {
            SymbolHandle::invalid()
        };
        assert_eq!(
            call.target_symbol, expected,
            "{receiver}: no scalar/array/free-name target fallback"
        );
    }
}

#[test]
fn computed_receiver_rejects_stale_and_foreign_producer_targets() {
    let source = r#"
        data Cell {} data Owner { cell: Cell; }
        machine Cell::read(&self) -> u64 { 1 }
        machine Cell::identity(&mut self) -> &mut Cell { self }
        machine identity(value: &mut Cell) -> &mut Cell { value }
        machine Owner::run(&mut self) { let result: u64 = identity(&mut self.cell).read(); }
    "#;
    let (resolved, typed) = crate::front_end::typed_program_with_resolution(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::run")
        .expect("caller");
    let state = &typed.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(result) =
        &typed.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("result");
    };
    let ExpressionNode::Call(outer) = typed.expression_table.expression(result.initial_value)
    else {
        panic!("outer");
    };
    let foreign = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Cell::identity")
        .expect("foreign");
    let foreign_state = typed.machine_states(foreign)[0].symbol;
    let target = resolved::name::DiagnosticName::generated(outer.target.as_str());
    for variant in ["exact", "stale", "foreign", "non_state", "missing"] {
        let mut expressions = typed.expression_table.clone();
        let ExpressionNode::Call(producer) = expressions.expression_mut(outer.receiver) else {
            panic!("producer");
        };
        producer.target_symbol = match variant {
            "stale" => SymbolHandle::from_parts(
                producer.target_symbol.arena_index(),
                producer.target_symbol.generation() + 1,
            ),
            "foreign" => foreign_state,
            "non_state" => machine.symbol,
            "missing" => SymbolHandle::invalid(),
            _ => producer.target_symbol,
        };
        let actual =
            computed_receiver_method_target(&resolved, &expressions, outer.receiver, &target);
        if variant == "exact" {
            assert!(actual.is_valid());
            assert_eq!(actual, outer.target_symbol);
        } else {
            assert!(
                !actual.is_valid(),
                "{variant} cannot supply a declared producer result"
            );
        }
    }
}
