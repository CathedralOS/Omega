use crate::tests::front_end::checked_program_result;

fn reject_source(source: &str, expected: &str) {
    // A synthetic Self type rejection or a missing Terminal plan is not
    // evidence that the checker recognized observation through this receiver.
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("receiver observation must fail checking: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected receiver diagnostic `{expected}`: {diagnostics:#?}\nsource:\n{source}"
    );
}

#[test]
fn write_only_self_state_transfer_cannot_restore_observation() {
    for receiver in ["&self", "&mut self", "self"] {
        reject_source(
            &format!(
                "data Record {{ value: u16; }}
                 machine Record::exercise(&write self) {{
                     transition {{ _ -> observe() }}
                     state observe({receiver}) {{ let prior: u16 = self.value; }}
                 }}"
            ),
            "widens write-only receiver",
        );
    }
}

#[test]
fn write_only_self_state_transfer_preserves_non_observing_access() {
    checked_program_result(
        "data Record { value: u16; }
         machine Record::exercise(&write self) {
             transition { _ -> replace() }
             state replace(&write self) { self.value = 17; }
         }",
    )
    .expect("a state transfer may preserve write-only receiver access");
}
