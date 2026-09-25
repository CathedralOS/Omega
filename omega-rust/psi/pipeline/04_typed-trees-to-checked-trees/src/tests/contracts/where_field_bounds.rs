//! A data whose `where` facts each compare one field with a literal bounds
//! that field as a bracket did: a store owes the field's interval under the
//! statement's flow facts, and every read, an index included, may assume it.
//! Relational facts keep the default-domain window rules.
use crate::tests::front_end::checked_program_result;

fn messages(source: &str) -> Vec<String> {
    match checked_program_result(source) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
    }
}

fn program(facts: &str, stored: &str) -> String {
    format!(
        "data Main where {facts} {{ values: [u8; 4]; i: u64; limit: u64; }}
         machine Main::bump(&mut self) {{
             let next: u64 = {stored};
             self.i = next;
         }}
         machine Main::read(&self) -> u8 {{ self.values[self.i] }}"
    )
}

#[test]
fn an_interval_where_field_proves_computed_stores_and_indexes() {
    let accepted = messages(&program("i <= 3", "2"));
    assert!(accepted.is_empty(), "{accepted:#?}");
    let stored_too_high = messages(&program("i <= 3", "5"));
    assert!(
        stored_too_high
            .iter()
            .any(|message| message.contains("not provably within its data's `where` facts")),
        "{stored_too_high:#?}"
    );
    let index_too_wide = messages(&program("i <= 4", "2"));
    assert!(
        index_too_wide
            .iter()
            .any(|message| message.contains("cannot prove index `self.i` is within length 4")),
        "{index_too_wide:#?}"
    );
}

#[test]
fn a_relational_where_fact_keeps_its_window_rules() {
    let relational = messages(&program("i <= limit", "2"));
    assert!(
        relational
            .iter()
            .any(|message| message.contains("default domain")),
        "{relational:#?}"
    );
}
