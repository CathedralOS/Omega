use typed_trees::TypedTrees;
use typed_trees::statement::{StatementNode, TableCall};

fn qualified_program(body: &str) -> TypedTrees {
    let helpers = "module helpers;
        pub data Counter {}
        pub data Other {}
        pub machine Counter::write(value: &mut i32) { value = 70; }
        pub machine Counter::read(value: &i32) -> i32 { value }
        pub machine Other::write(value: &mut i32) { value = 9; }";
    let caller = format!(
        "use helpers;
         machine Counter() {{}}
         machine run() -> i32 {{ let mut value: i32 = 0; {body} }}"
    );
    let mut sources = source::SourceMap::default();
    let helper_source = sources
        .add("helpers.omg".into(), helpers.to_owned())
        .source_id;
    let caller_source = sources.add("main.omg".into(), caller.clone()).source_id;
    crate::front_end::typed_program_from_source_map(
        sources,
        &[(helper_source, helpers), (caller_source, &caller)],
    )
}

fn qualified_call(program: &mut TypedTrees) -> &mut TableCall {
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("caller");
    let statements = program.machine_states(caller)[0].statement_nodes;
    program
        .statement_table
        .statements_mut(statements)
        .iter_mut()
        .find_map(|statement| match statement {
            StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("qualified statement call")
}

fn rejects(program: &TypedTrees, fragment: &str) {
    let diagnostics = validation::validate_program(program).expect_err("invalid call rejects");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

#[test]
fn qualified_statement_selects_its_declaration_before_namesake_machine() {
    // The unrelated root machine Counter cannot replace helpers::Counter.
    let program = qualified_program("helpers::Counter::write(&mut value); value");
    validation::validate_program(&program).expect("static statement namespace is not storage");
}

#[test]
fn named_operator_statement_keeps_its_signature_validation_route() {
    let program = crate::front_end::typed_program(
        "pub data F64 {}
         pub boundary operator F64::square_root(value: f64) -> f64;
         machine run() -> i32 { _ = F64::square_root(4.0f64); 0 }",
    );
    validation::validate_program(&program).expect("operator namespace is not an ordinary machine");
}

#[test]
fn qualified_statement_keeps_argument_and_result_obligations() {
    for (body, fragment) in [
        (
            "helpers::Counter::write(); value",
            "expects 1 argument(s), got 0",
        ),
        (
            "helpers::Counter::read(&value); value",
            "discards its non-unit",
        ),
        ("helpers::Counter::write(true); value", "i32"),
        (
            "helpers; value",
            "not a declared local, parameter, field, or type",
        ),
    ] {
        rejects(&qualified_program(body), fragment);
    }
}

#[test]
fn qualified_statement_rejects_absent_selected_target() {
    let mut program = qualified_program("helpers::Counter::write(&mut value); value");
    qualified_call(&mut program).target_symbol = symbols::SymbolHandle::invalid();
    rejects(&program, "has no resolved callable declaration");
}

#[test]
fn qualified_statement_rejects_another_carriers_same_named_target() {
    let mut program = qualified_program("helpers::Counter::write(&mut value); value");
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("Other::write"))
        .expect("other carrier's method");
    let target = program.machine_states(other)[0].symbol;
    qualified_call(&mut program).target_symbol = target;
    rejects(
        &program,
        "does not belong to the receiver's exact data type",
    );
}
