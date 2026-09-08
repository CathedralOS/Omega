use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize byte extent fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse byte extent fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve byte extent fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type byte extent fixture");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(
            accepted,
            "unused byte capacity admitted as live extent:\n{source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
                "expected bounds error: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| {
                        diagnostic.message.contains("cannot prove index")
                            || diagnostic.message.contains("cannot prove subslice range")
                    }),
                "expected bounds errors, not unrelated rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

fn field_source(body: &str) -> String {
    format!(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8;3] in Utf8; other: [u8;3] in Utf8; }}
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {{
            {body}
        }}
    "#
    )
}

#[test]
fn bounded_byte_indexes_use_live_prefix_not_capacity() {
    for (literal, position, accepted) in [
        ("XXX", 2, true),
        ("X", 0, true),
        ("X", 1, false),
        ("X", 2, false),
        ("", 0, false),
    ] {
        for access in [
            format!("self.out[{position}] = 65;"),
            format!("let observed: u8 = self.out[{position}];"),
        ] {
            check(
                &field_source(&format!("self.out = \"{literal}\"; {access}")),
                accepted,
            );
        }
    }
}

#[test]
fn bounded_byte_runtime_indexes_and_repeated_element_writes_keep_live_length() {
    check(
        &field_source("self.out = \"XXX\"; self.out[position] = byte; self.out[position] = 65;"),
        true,
    );
    check(
        &field_source("self.out = \"X\"; self.out[position] = byte;"),
        false,
    );
}

#[test]
fn bounded_byte_whole_replacement_updates_extent_and_siblings_do_not() {
    for (body, accepted) in [
        (
            "self.out = \"XXX\"; self.out = \"X\"; self.out[2] = 65;",
            false,
        ),
        (
            "self.out = \"XXX\"; self.out = \"\"; self.out[0] = 65;",
            false,
        ),
        (
            "self.out = \"X\"; self.out = \"XXX\"; self.out[2] = 65;",
            true,
        ),
        (
            "self.out = \"XXX\"; self.other = \"\"; self.out[2] = 65;",
            true,
        ),
        (
            "self.other = \"XXX\"; self.out = \"X\"; self.out[2] = 65;",
            false,
        ),
    ] {
        check(&field_source(body), accepted);
    }
}

#[test]
fn bounded_byte_ranges_end_at_live_length() {
    for (literal, range, accepted) in [
        ("X", "0..1", true),
        ("X", "0..2", false),
        ("X", "0..=0", true),
        ("X", "0..=1", false),
        ("", "0..0", true),
        ("", "0..1", false),
    ] {
        check(
            &field_source(&format!(
                "self.out = \"{literal}\"; let observed: &[u8] = self.out[{range}];"
            )),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_literal_extent_counts_encoded_bytes_not_characters() {
    for (position, accepted) in [(0, true), (1, true), (2, false)] {
        check(
            &field_source(&format!(
                "self.out = \"é\"; let observed: u8 = self.out[{position}];"
            )),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_unknown_live_length_needs_a_guard() {
    check(&field_source("let observed: u8 = self.out[0];"), false);
    check(
        &field_source(
            "transition 0 < self.out.len { true -> read() _ -> done() } state read(&mut self) { let observed: u8 = self.out[0]; } state done(&mut self) {}",
        ),
        true,
    );
}

#[test]
fn raw_fixed_byte_array_keeps_its_exact_declared_extent() {
    check(
        "machine replace(output: &mut [u8;3], position: u64 [0..=2]) { output[position] = 65; }",
        true,
    );
}

#[test]
fn bounded_byte_live_length_survives_state_transitions_and_indexed_loop_writes() {
    for (literal, accepted) in [("XXX", true), ("X", false), ("", false)] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                data Record {{ out: [u8;3] in Utf8; position: u64; }}
                machine Record::replace(&mut self) {{
                    self.out = "{literal}";
                    self.position = 0;
                    transition {{ _ -> head() }}
                    state head(&mut self) {{
                        transition self.position < 3 {{ true -> write() _ -> done() }}
                    }}
                    state write(&mut self) {{
                        self.out[self.position] = 65;
                        self.position = self.position + 1;
                        transition {{ _ -> head() }}
                    }}
                    state done(&mut self) {{}}
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_extent_is_specific_to_the_receiver_path() {
    for (left, right, accepted) in [("XXX", "X", true), ("X", "XXX", false)] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                data Record {{ out: [u8;3] in Utf8; }}
                data Pair {{ left: Record; right: Record; }}
                machine Pair::replace(&mut self) {{
                    self.left.out = "{left}";
                    self.right.out = "{right}";
                    self.left.out[2] = 65;
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_replacement_calls_retire_only_overlapping_extents() {
    for (call, accepted) in [
        ("self.inspect();", true),
        ("self.clear_other();", true),
        ("self.clear();", false),
    ] {
        check(
            &format!(
                "{} machine Record::inspect(&self) {{}}
                 machine Record::clear_other(&mut self) {{ self.other = \"\"; }}
                 machine Record::clear(&mut self) {{ self.out = \"\"; }}",
                field_source(&format!("self.out = \"XXX\"; {call} self.out[2] = 65;")),
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_state_joins_require_extent_on_every_incoming_path() {
    for (right, accepted) in [
        ("self.out = \"YYY\";", true),
        ("self.out = \"X\";", false),
        ("", false),
    ] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                data Record {{ out: [u8;3] in Utf8; }}
                machine Record::replace(&mut self, choose: bool) {{
                    transition choose {{ true -> left() _ -> right() }}
                    state left(&mut self) {{
                        self.out = "XXX";
                        transition {{ _ -> join() }}
                    }}
                    state right(&mut self) {{
                        {right}
                        transition {{ _ -> join() }}
                    }}
                    state join(&mut self) {{ self.out[2] = 65; }}
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_locals_do_not_inherit_unused_capacity() {
    for (literal, accepted) in [("XXX", true), ("X", false), ("", false)] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                machine replace() {{
                    let mut output: [u8;3] in Utf8 = "{literal}";
                    output[2] = 65;
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_borrowed_receivers_track_alias_mutation() {
    for (call, accepted) in [("alias.inspect();", true), ("alias.clear();", false)] {
        check(
            &format!(
                "{} machine Record::inspect(&self) {{}}
                 machine Record::clear(&mut self) {{ self.out = \"\"; }}",
                field_source(&format!(
                    "self.out = \"XXX\"; let alias: &mut Record = &mut self; {call} let observed: u8 = alias.out[2];"
                )),
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_direct_alias_replacement_retires_the_original_extent() {
    for (replacement, accepted) in [("XXX", true), ("", false)] {
        check(
            &field_source(&format!(
                "self.out = \"XXX\"; let alias: &mut Record = &mut self; alias.out = \"{replacement}\"; let observed: u8 = self.out[2];"
            )),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_earlier_call_operands_invalidate_live_length() {
    for (write, accepted) in [("", true), ("self.out = \"\";", false)] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                data Record {{ out: [u8;3] in Utf8; }}
                machine Record::clear_value(&mut self) -> u8 {{ {write} 0 }}
                machine combine(left: u8, right: u8) {{}}
                machine Record::read(&mut self) {{
                    self.out = "XXX";
                    combine(self.clear_value(), self.out[2]);
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_assignment_value_cannot_retire_the_destination_extent() {
    for (write, accepted) in [
        ("", true),
        ("self.other = \"\";", true),
        ("self.out = \"\";", false),
    ] {
        check(
            &format!(
                "{} machine Record::clear_value(&mut self) -> u8 {{ {write} 65 }}",
                field_source("self.out = \"XXX\"; self.out[2] = self.clear_value();"),
            ),
            accepted,
        );
    }
}

#[test]
fn bounded_byte_parameter_receivers_do_not_share_field_extents() {
    for (left, right, accepted) in [("XXX", "X", true), ("X", "XXX", false)] {
        check(
            &format!(
                r#"
                domain [u8;3]::Utf8 requires valid_utf8(self);
                data Record {{ out: [u8;3] in Utf8; }}
                machine read(left: &mut Record, right: &mut Record) -> u8 {{
                    left.out = "{left}";
                    right.out = "{right}";
                    left.out[2]
                }}
            "#
            ),
            accepted,
        );
    }
}
