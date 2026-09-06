fn check(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let source = format!("data Main {{}} machine Main::run() {{}} {source}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    crate::lower_typed_trees(typed).map(|_| ())
}

#[test]
fn temporary_projections_retain_every_lifetime_candidate() {
    for projection in [
        "forward_outer(Outer { input: input }).input",
        "forward_array([input])[0]",
    ] {
        for operation in ["write(left);", "write(right);", "write(other);"] {
            let source = format!(
                "data Input<'source> {{ first: &'source mut i32; second: &'source mut i32; }}
                 data Outer<'source> {{ input: Input<'source>; }}
                 machine forward_outer<'source>(value: Outer<'source>) -> Outer<'source> {{ value }}
                 machine forward_array<'source>(value: [Input<'source>; 1]) -> [Input<'source>; 1] {{ value }}
                 machine select<'source>(value: Input<'source>) -> &'source mut i32 {{ value.second }}
                 machine write(value: &mut i32) {{ value = 1; }}
                 machine exercise<'source>(left: &'source mut i32, right: &'source mut i32, other: &mut i32) {{
                     let input: Input<'source> = Input {{ first: left, second: right }};
                     let projected: Input<'source> = {projection};
                     let held: &'source mut i32 = select(projected);
                     {operation}
                     write(held);
                 }}"
            );
            if operation == "write(other);" {
                check(&source).expect("unrelated source remains usable");
            } else {
                let diagnostics =
                    check(&source).expect_err("all lifetime candidates remain borrowed");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("while local borrow `held` is still active")),
                    "{projection}: {diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn a_temporary_result_does_not_hide_an_input_owner_with_a_live_child() {
    let diagnostics = check(
        "data Cell { value: u64; }
         data Owned { cell: Cell; }
         machine forward(value: Owned) -> Owned { value }
         machine consume(value: Cell) {}
         machine exercise() {
             let mut cell: Cell = Cell { value: 0 };
             let child: &mut u64 = &mut cell.value;
             consume(forward(Owned { cell: cell }).cell);
             child = 1;
         }",
    )
    .expect_err("fresh result storage does not authorize moving the borrowed input");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("receives an owned value while local borrow `child`")),
        "{diagnostics:#?}"
    );
}

#[test]
fn temporary_partial_moves_keep_nominal_cleanup_entitlement() {
    let diagnostics = check(
        "data Leaf { value: i32; }
         data Wrapper { leaf: Leaf; }
         machine Wrapper::drop(&mut self) {}
         machine forward(value: Wrapper) -> Wrapper { value }
         machine exercise() {
             let wrapper: Wrapper = Wrapper { leaf: Leaf { value: 1 } };
             let extracted: Leaf = forward(wrapper).leaf;
         }",
    )
    .expect_err("drop still requires the whole temporary wrapper");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")),
        "{diagnostics:#?}"
    );
}

#[test]
fn temporary_partial_moves_cannot_discard_linear_siblings() {
    for (container, declaration, initializer, projection) in [
        (
            "Pair",
            "data Pair { left: Receipt; right: Receipt; }",
            "Pair { left: left, right: right }",
            "left",
        ),
        ("[Receipt; 2]", "", "[left, right]", "[0]"),
    ] {
        let selector = if projection.starts_with('[') {
            projection.to_owned()
        } else {
            format!(".{projection}")
        };
        let source = format!(
            "data Receipt [linear] {{ code: i32; }}
             machine Receipt::ack(self) {{}}
             {declaration}
             machine forward(value: {container}) -> {container} {{ value }}
             machine exercise() {{
                 let left: Receipt = Receipt {{ code: 1 }};
                 let right: Receipt = Receipt {{ code: 2 }};
                 let input: {container} = {initializer};
                 let extracted: Receipt = forward(input){selector};
                 Receipt::ack(extracted);
             }}"
        );
        let diagnostics = check(&source).expect_err("the unselected linear sibling is still owed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("linear")
                    || diagnostic.message.contains("claim")),
            "{container}: {diagnostics:#?}"
        );
    }
}

#[test]
fn temporary_projection_can_transfer_the_complete_linear_frontier() {
    for (container, declaration, initializer, projection) in [
        (
            "Wrapper",
            "data Wrapper { receipt: Receipt; tag: i32; }",
            "Wrapper { receipt: receipt, tag: 0 }",
            ".receipt",
        ),
        ("[Receipt; 1]", "", "[receipt]", "[0]"),
    ] {
        check(&format!(
            "data Receipt [linear] {{ code: i32; }}
             machine Receipt::ack(self) {{}}
             {declaration}
             machine forward(value: {container}) -> {container} {{ value }}
             machine exercise() {{
                 let receipt: Receipt = Receipt {{ code: 1 }};
                 let input: {container} = {initializer};
                 let extracted: Receipt = forward(input){projection};
                 Receipt::ack(extracted);
             }}"
        ))
        .expect("the projection retains all linear claims; copy siblings need no cleanup");
    }
}

#[test]
fn a_temporary_result_does_not_hide_duplicate_owned_operands() {
    let diagnostics = check(
        "data View { body: &mut i32; }
         machine forward(values: [View; 2]) -> [View; 2] { values }
         machine exercise(source: &mut i32) {
             let view: View = View { body: source };
             let projected: View = forward([view, view])[0];
         }",
    )
    .expect_err("evaluating the temporary still transfers each operand once");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("view")
                && (diagnostic.message.contains("move")
                    || diagnostic.message.contains("consum")
                    || diagnostic.message.contains("owned"))),
        "{diagnostics:#?}"
    );
}
