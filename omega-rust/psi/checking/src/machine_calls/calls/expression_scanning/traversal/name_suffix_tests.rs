#[test]
fn static_paths_cannot_continue_through_runtime_bindings() {
    let accepted = [
        "machine read(value: u32) -> u32 { value::Missing }",
        "machine read() -> u32 { let value: u32 = 1; value::Missing }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::Missing }",
        "data Signal { case Empty; case Full(count: u32); } machine read(value: Signal) -> u32 { value::Empty }",
        "machine read(value: [u32; 2]) -> u32 { value::Missing }",
        "machine read(value: &[u32]) -> u32 { value::Missing }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::count }",
        "machine read(value: &[u8]) -> u64 { value::len }",
        "data Pair { count: u32; } machine read(value: Pair) -> u32 { value::count::Missing }",
        "data Buffer { values: [u32; 2]; } machine read(value: &Buffer) -> u32 { value::values::Missing }",
    ].into_iter().filter(|source| crate::validate_program(&crate::front_end::typed_program(source)).is_ok()).collect::<Vec<_>>();
    assert!(
        accepted.is_empty(),
        "unresolved lexical suffixes accepted: {accepted:#?}"
    );
}

#[test]
fn missing_head_cannot_hide_a_retained_runtime_root_in_the_leaf() {
    use typed_trees::expression::ExpressionNode;

    let mut program =
        crate::front_end::typed_program("machine read(value: u32) -> u32 { value::Missing }");
    assert!(crate::validate_program(&program).is_err());
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Name(path)
                if program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("runtime-rooted static path");
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    assert!(path.head_symbol.is_valid());
    path.symbol = path.head_symbol;
    path.head_symbol = symbols::SymbolHandle::invalid();
    assert!(
        crate::validate_program(&program).is_err(),
        "missing head hid the retained runtime binding in the leaf"
    );
}

fn qualified_case_roots(
    payload: &str,
) -> (
    typed_trees::TypedTrees,
    Vec<typed_trees::expression::ExpressionHandle>,
) {
    use typed_trees::{expression::ExpressionNode, name::Identifier};

    let mut program = crate::front_end::typed_program_with_generic_data(&format!(
        "{payload} data Optional<T> {{ case None; case Some(value: T); }}
         machine read(option: Optional<Payload>) -> i32 {{
             transition option {{ Optional::Some {{ value }} -> present(value) Optional::None -> absent() }}
             state present(value: Payload) -> i32 {{ 70 }}
             state absent() -> i32 {{ 71 }}
         }}
         machine unrelated(foreign: u32) -> u32 {{ foreign }}"
    ));
    let paths = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            (program.symbols.get(path.head_symbol).kind == symbols::SymbolKind::Data
                && program
                    .symbols
                    .name(path.head_symbol)
                    .starts_with("Optional<"))
            .then_some((handle, *path))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "Some and None retain their selected carrier"
    );
    let mut handles = Vec::new();
    for (handle, path) in paths {
        let original = program
            .expression_table
            .name_path_members(path.members)
            .to_vec();
        let mut members = arena::HandleSpan::empty();
        for (index, member) in original.into_iter().enumerate() {
            let member = if index == 0 {
                Identifier::generated(format!("foreign::{}", member.as_str()))
            } else {
                member
            };
            program
                .expression_table
                .push_name_path_member(&mut members, member);
        }
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(handle) else {
            unreachable!()
        };
        path.members = members;
        handles.push(handle);
    }
    (program, handles)
}

#[test]
fn qualified_generated_case_roots_retain_nominal_declaration_identity() {
    for payload in [
        "data Payload [copy] { case First; case Second; }",
        "data Payload { count: u64; }",
    ] {
        let (program, _) = qualified_case_roots(payload);
        let result = crate::validate_program(&program);
        assert!(result.is_ok(), "{result:?}");
    }
}

#[test]
fn qualified_case_spelling_cannot_replace_missing_or_foreign_storage_identity() {
    use typed_trees::expression::ExpressionNode;

    for foreign_storage in [false, true] {
        let (mut program, handles) =
            qualified_case_roots("data Payload [copy] { case First; case Second; }");
        let unrelated = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "unrelated")
            .unwrap();
        let state = program.machine_states(unrelated).first().unwrap();
        let foreign = program.state_parameters(state)[0].symbol;
        for handle in handles {
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(handle) else {
                unreachable!()
            };
            path.head_symbol = if foreign_storage {
                foreign
            } else {
                symbols::SymbolHandle::invalid()
            };
            if !foreign_storage {
                path.symbol = symbols::SymbolHandle::invalid();
            }
        }
        let diagnostics = crate::validate_program(&program)
            .expect_err("presentation cannot supply declaration custody");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("not a declared local, parameter, field, or type")),
            "{diagnostics:?}"
        );
    }
}
