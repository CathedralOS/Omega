use crate::CheckingRequest;
use crate::flow::check_against_whole_pass as lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn indexed_byte_ranges_preserve_only_live_ascii_replacements() {
    for (text, before_read, after_read, output, accepted) in [
        ("HELLO", "", "", ".....", true),
        ("HELLO", "", "self.text = \"world\";", ".....", true),
        ("HELLO", "self.text[0] = 255;", "", ".....", false),
        ("HELLO", "", "self.byte = 255;", ".....", false),
        (
            "HELLO",
            "",
            "let alias: &mut u8 in Wrapping = &mut self.byte; alias = 255;",
            ".....",
            false,
        ),
        ("HELLO", "", "", "é...", false),
        ("~~~~~", "", "", ".....", false),
        (r"\xFD\xFE\xFF\xFD\xFE", "", "", ".....", false),
    ] {
        let source = format!(
            r#"
            domain [u8; 5]::Utf8 requires valid_utf8(self);
            data Buffer {{ text: [u8; 5]; output: [u8; 5] in Utf8; position: i32 [0..=4]; byte: u8 in Wrapping; }}
            machine Buffer::shift(&mut self) {{
                self.text = "{text}";
                self.output = "{output}";
                {before_read}
                self.byte = self.text[self.position] as u8 in Wrapping + 3;
                {after_read}
                self.output[self.position] = self.byte as u8;
                transition {{ _ -> done() }}
                state done(&mut self) {{}}
            }}
        "#
        );
        check(&source, accepted);
    }
}

fn loop_source(initialization: &str, replacement: &str) -> String {
    format!(
        r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        data Buffer {{ output: [u8; 2] in Utf8; position: i32 [0..=2]; }}
        machine Buffer::fill(&mut self) {{
            {initialization}
            self.position = 0;
            transition {{ _ -> step() }}
            state step(&mut self) {{
                transition self.position < 2 {{ true -> write() false -> done() }}
            }}
            state write(&mut self) {{
                {replacement}
                self.position = self.position + 1;
                transition {{ _ -> step() }}
            }}
            state done(&mut self) {{}}
        }}
        "#
    )
}

fn check(source: &str, accepted: bool) {
    let result = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled());
    if accepted {
        result.unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    } else {
        let diagnostics = result.expect_err("every state edge must prove the field domain");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove default-domain field requirement")),
            "{source}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn field_class_meets_all_predecessors_independent_of_state_order() {
    for (right_initialization, accepted) in [
        ("self.output = \"CD\";", true),
        ("", false),
        ("self.output = \"é\";", false),
    ] {
        for reverse_order in [false, true] {
            let left = r#"state left(&mut self) {
                self.output = "AB";
                transition { _ -> join() }
            }"#;
            let right = format!(
                "state right(&mut self) {{ {right_initialization} transition {{ _ -> join() }} }}"
            );
            let join = r#"state join(&mut self) {
                self.output[0] = 65;
                transition { _ -> done() }
            }"#;
            let states = if reverse_order {
                format!("{join} {right} {left}")
            } else {
                format!("{left} {right} {join}")
            };
            check(
                &format!(
                    r#"
                    domain [u8; 2]::Utf8 requires valid_utf8(self);
                    data Buffer {{ output: [u8; 2] in Utf8; choose: bool; }}
                    machine Buffer::fill(&mut self) {{
                        transition self.choose {{ true -> left() false -> right() }}
                        {states}
                        state done(&mut self) {{}}
                    }}
                    "#
                ),
                accepted,
            );
        }
    }
}

#[test]
fn ordinary_state_invocation_is_not_a_field_evidence_route() {
    // A state is a transition destination, never a callee
    // (state_contracts.md), so an ordinary `step();` cannot reach the loop
    // with or without the transition's field evidence: it is refused as a
    // call before any field domain is considered.
    let source = loop_source(
        "self.output = \"AB\"; step();",
        "self.output[self.position] = 65;",
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect_err("an internal state is not callable");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("calls its internal state `step` as an ordinary call")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn nested_loops_preserve_bounded_ascii_fields_without_assuming_utf8_replacement() {
    for (initial, replacement, accepted) in [
        ("  ", "((self.row + 48) as u8 in Wrapping) as u8", true),
        ("  ", "255", false),
        ("é", "32", false),
    ] {
        check(
            &format!(
                r#"
                domain [u8; 2]::Utf8 requires valid_utf8(self);
                data Buffer {{ output: [u8; 2] in Utf8; row: i32 in Wrapping; column: i32 in Wrapping; }}
                machine Buffer::fill(&mut self) {{
                    self.output = "{initial}";
                    self.row = 1;
                    transition {{ _ -> rows() }}
                    state rows(&mut self) {{
                        transition self.row >= 1 && self.row <= 9 {{ true -> start_columns() _ -> done() }}
                    }}
                    state start_columns(&mut self) {{
                        self.column = 0;
                        transition {{ _ -> columns() }}
                    }}
                    state columns(&mut self) {{
                        transition self.column >= 0 && self.column < 2 {{ true -> write() _ -> next_row() }}
                    }}
                    state write(&mut self) {{
                        self.output[self.column] = {replacement};
                        self.column = self.column + 1;
                        transition {{ _ -> columns() }}
                    }}
                    state next_row(&mut self) {{
                        self.row = self.row + 1;
                        transition {{ _ -> rows() }}
                    }}
                    state done(&mut self) {{}}
                }}
                "#,
            ),
            accepted,
        );
    }
}
