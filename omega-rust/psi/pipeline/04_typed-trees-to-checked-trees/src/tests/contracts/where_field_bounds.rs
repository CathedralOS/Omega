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

/// A state parameter bounded by a receiver field (`requires y <= self.max_y`)
/// is bounded by that field's `where` facts too: arithmetic over it proves in
/// range, a hoisted computed index proves in bounds, and a transition passing
/// a literal proves the requirement from the field's stored bounds.
fn field_bounded_grid(where_facts: &str, first_row: &str) -> String {
    format!(
        "data Main where {where_facts} {{ cells: [u8; 12]; max_y: u32; max_x: u32; seen: u8; }}
         machine Main::main(&mut self) {{
             self.max_y = 2;
             self.max_x = 3;
             transition {{ _ -> row({first_row}) }}
             state row(&mut self, y: u32) requires y <= self.max_y {{
                 transition {{ _ -> cell(y, 0) }}
             }}
             state cell(&mut self, y: u32, x: u32)
             requires y <= self.max_y
             requires x <= self.max_x {{
                 self.seen = self.cells[y * 4 + x];
                 transition x + 1 <= self.max_x {{
                     true -> cell(y, x + 1)
                     _ -> done()
                 }}
             }}
             state done(&mut self) {{}}
         }}"
    )
}

#[test]
fn a_parameter_bounded_by_a_where_field_carries_that_fields_bound() {
    let accepted = messages(&field_bounded_grid("max_y <= 2, max_x <= 3", "0"));
    assert!(accepted.is_empty(), "{accepted:#?}");
    let too_wide = messages(&field_bounded_grid("max_y <= 3, max_x <= 3", "0"));
    assert!(
        too_wide
            .iter()
            .any(|message| message.contains("cannot prove index `y * 4 + x` is within length 12")),
        "{too_wide:#?}"
    );
    let beyond_field = messages(&field_bounded_grid("max_y <= 2, max_x <= 3", "5"));
    assert!(
        beyond_field
            .iter()
            .any(|message| message.contains("cannot prove requires contract for call row")),
        "{beyond_field:#?}"
    );
}
