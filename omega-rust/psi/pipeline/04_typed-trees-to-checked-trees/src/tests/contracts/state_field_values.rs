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
fn ordinary_state_invocation_does_not_inherit_transition_field_evidence() {
    check(
        &loop_source(
            "self.output = \"AB\"; step();",
            "self.output[self.position] = 65;",
        ),
        false,
    );
}
