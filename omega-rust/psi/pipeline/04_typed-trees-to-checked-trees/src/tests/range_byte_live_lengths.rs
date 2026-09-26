use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
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
            let is_bounds_error = |diagnostic: &diagnostics::Diagnostic| {
                diagnostic.message.contains("cannot prove index")
                    || diagnostic.message.contains("cannot prove subslice range")
            };
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .any(is_bounds_error),
                "expected a bounds error: {diagnostics:#?}\n{source}"
            );
            // An element store outside the live extent is an invalid write;
            // the machine's return then also reports the field's unclosed
            // default-domain window (the consumption point of that write).
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| {
                        is_bounds_error(diagnostic)
                            || diagnostic.message.contains(
                                "cannot prove default-domain field requirement for return from",
                            )
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
