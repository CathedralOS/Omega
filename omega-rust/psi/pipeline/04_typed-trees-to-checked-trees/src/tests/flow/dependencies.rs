use crate::tests::front_end::checked_program_result;

#[test]
fn domain_conjunction_write_checks_every_predicate_facet() {
    let source = r#"
        domain [u8]::Utf8
        requires
            valid_utf8(self);
        domain [u8]::AsciiOnly
        requires
            ascii_only(self);

        data Main {
            text: &[u8] in Utf8 & AsciiOnly;
        }

        machine Main::main(&mut self) {
            self.text = "é";
        }
    "#;

    let diagnostics = checked_program_result(source)
        .expect_err("a UTF-8 but non-ASCII literal must fail the second predicate facet");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("domain `[u8]::AsciiOnly`") })
    );
}
