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
fn a_relational_where_fact_keeps_its_window_rules() {
    let relational = messages(&program("i <= limit", "2"));
    assert!(
        relational
            .iter()
            .any(|message| message.contains("default domain")),
        "{relational:#?}"
    );
}

/// A state parameter bounded by a borrowed field (`requires y <= grid.max_y`)
/// is bounded by that field's `where` facts too: arithmetic over it proves in
/// range, a hoisted computed index proves in bounds, and a transition passing
/// a literal proves the requirement from the field's stored bounds.
fn field_bounded_grid(where_facts: &str, first_row: &str) -> String {
    format!(
        "data Grid where {where_facts} {{ cells: [u8; 12]; max_y: u32; max_x: u32; seen: u8; }}
         machine main() {{
             let mut grid: Grid;
             grid.max_y = 2;
             grid.max_x = 3;
             transition {{ _ -> row(&mut grid, {first_row}) }}
             state row(grid: &mut Grid, y: u32) requires y <= grid.max_y {{
                 transition {{ _ -> cell(grid, y, 0) }}
             }}
             state cell(grid: &mut Grid, y: u32, x: u32)
             requires 0 <= y && y <= grid.max_y && 0 <= x && x <= grid.max_x {{
                 grid.seen = grid.cells[y * 4 + x];
                 transition grid.seen > 0 {{
                     true -> hotter(grid, y, x)
                     _ -> step(grid, y, x)
                 }}
             }}
             state hotter(grid: &mut Grid, y: u32, x: u32)
             requires 0 <= y && y <= grid.max_y && 0 <= x && x <= grid.max_x {{
                 grid.seen = 0;
                 transition {{ _ -> step(grid, y, x) }}
             }}
             state step(grid: &mut Grid, y: u32, x: u32)
             requires 0 <= y && y <= grid.max_y && 0 <= x && x <= grid.max_x {{
                 transition x + 1 <= grid.max_x {{
                     true -> cell(grid, y, x + 1)
                     _ -> next_row(grid, y)
                 }}
             }}
             state next_row(grid: &mut Grid, y: u32)
             requires 0 <= y && y <= grid.max_y {{
                 transition y + 1 <= grid.max_y {{
                     true -> row(grid, y + 1)
                     _ -> done(grid)
                 }}
             }}
             state done(grid: &mut Grid) {{}}
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

#[test]
fn a_grid_bound_cannot_survive_replacement_or_change_its_coordinate() {
    let source = field_bounded_grid("max_y <= 2, max_x <= 3", "0");
    for changed in [
        source.replace(
            "transition x + 1 <= grid.max_x",
            "grid.max_y = 0; transition x + 1 <= grid.max_x",
        ),
        source.replace("cell(grid, y, x + 1)", "cell(grid, y + 1, x + 1)"),
    ] {
        let rejected = messages(&changed);
        assert!(
            rejected.iter().any(|message| {
                message.contains("cannot prove requires contract for call cell")
            }),
            "{rejected:#?}\n{changed}"
        );
    }
}
