use super::*;

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
    let result = lower_typed_trees(parse_typed_trees(source));
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
fn ascii_field_class_survives_indexed_writes_and_loop_edges() {
    check(
        &loop_source("self.output = \"AB\";", "self.output[self.position] = 65;"),
        true,
    );
}

#[test]
fn loop_field_class_requires_initial_evidence_and_every_replacement() {
    for (initialization, replacement) in [
        ("", "self.output[self.position] = 65;"),
        ("self.output = \"é\";", "self.output[self.position] = 65;"),
        ("self.output = \"AB\";", "self.output[self.position] = 255;"),
        (
            "self.output = \"AB\";",
            "self.output[self.position] = 65; self.output[0] = 255;",
        ),
    ] {
        check(&loop_source(initialization, replacement), false);
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
fn aliased_writes_cannot_restore_a_loop_field_class() {
    for replacement in [
        "let alias: &mut [u8; 2] = &mut self.output; alias[0] = 255;",
        "corrupt(&mut self.output);",
    ] {
        let source = format!(
            "machine corrupt(bytes: &mut [u8; 2]) {{ bytes[0] = 255; }}\n{}",
            loop_source("self.output = \"AB\";", replacement)
        );
        check(&source, false);
    }
}

#[test]
fn nested_field_paths_keep_their_exact_owner() {
    for (destination, accepted) in [("first", true), ("second", false)] {
        let source = loop_source(
            "self.first.output = \"AB\";",
            &format!("self.{destination}.output[self.position] = 65;"),
        )
        .replace(
            "data Buffer { output: [u8; 2] in Utf8; position: i32 [0..=2]; }",
            "data Text { output: [u8; 2] in Utf8; }
             data Buffer { first: Text; second: Text; position: i32 [0..=2]; }",
        );
        check(&source, accepted);
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
