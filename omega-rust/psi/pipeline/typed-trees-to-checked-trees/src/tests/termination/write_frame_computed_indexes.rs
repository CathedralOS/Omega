use super::*;

#[test]
fn computed_indexes_preserve_origins_and_all_eager_writes() {
    let alternating = (0..16).fold("index(audit) + index(other)".to_owned(), |index, _| {
        format!("identity(({index}) % 2)")
    });
    let scalar_index = "(index(audit) + index(other)) % 2";
    let alias_body = "let alias: &mut u64 = &mut cells[$INDEX]; alias = 1; value";
    let cases = [
        (
            "assignment",
            "cells[$INDEX] = 1; value",
            scalar_index,
            true,
            true,
            false,
        ),
        ("alias", alias_body, scalar_index, true, true, false),
        (
            "signed_carrier",
            alias_body,
            "(signed_index(audit) + signed_index(other)) % 2",
            true,
            true,
            false,
        ),
        (
            "replacement",
            "let mut alias: &mut u64 = &mut backup[0]; let prior: &mut u64 = alias; alias = &mut cells[$INDEX]; prior = 1; alias = 1; value",
            scalar_index,
            true,
            true,
            true,
        ),
        (
            "terminal",
            "&mut cells[$INDEX]",
            scalar_index,
            true,
            false,
            false,
        ),
        (
            "statement_argument",
            "write_value(&mut cells[$INDEX]); value",
            scalar_index,
            true,
            true,
            false,
        ),
        (
            "helper_actual",
            "write_value(return_value(&mut cells[$INDEX])); value",
            scalar_index,
            true,
            true,
            false,
        ),
        (
            "view_terminal",
            "&mut return_cells(cells).as_mut_slice()[$INDEX]",
            scalar_index,
            true,
            false,
            false,
        ),
        (
            "alternating",
            alias_body,
            alternating.as_str(),
            true,
            true,
            false,
        ),
        (
            "record_projection",
            alias_body,
            "Pair { first: index(audit), second: index(other) }.first",
            true,
            true,
            false,
        ),
        (
            "array_projection",
            alias_body,
            "[index(audit), index(other)][0]",
            true,
            true,
            false,
        ),
        (
            "effectful_projection",
            alias_body,
            "[index(audit), index(other)][index(audit)]",
            true,
            true,
            false,
        ),
        (
            "record_argument",
            alias_body,
            "pair_index(Pair { first: index(audit), second: index(other) }) % 2",
            true,
            true,
            false,
        ),
        (
            "recursive",
            alias_body,
            "(index(audit) + recursive(other)) % 2",
            false,
            false,
            false,
        ),
        (
            "reborrow",
            alias_body,
            "(index(audit) + index(&mut other)) % 2",
            false,
            false,
            false,
        ),
        (
            "reference_computation",
            alias_body,
            "(index(audit) + return_value(other)) % 2",
            false,
            false,
            false,
        ),
        (
            "unselected_recursive",
            alias_body,
            "Pair { first: index(audit), second: recursive(other) }.first",
            false,
            false,
            false,
        ),
        (
            "unselected_reborrow",
            alias_body,
            "[index(audit), index(&mut other)][0]",
            false,
            false,
            false,
        ),
        (
            "bare_aggregate",
            alias_body,
            "Pair { first: index(audit), second: index(other) }",
            false,
            false,
            false,
        ),
        (
            "effectful_range",
            alias_body,
            "index(audit)..index(other)",
            false,
            false,
            false,
        ),
    ];
    let mut source = r#"
    data Bucket { cells: [u64; 2]; }
    data Main { bucket: Bucket; backup: [u64; 2]; value: u64; audit: u64; other: u64; }
    data Pair { first: u64; second: u64; }
    machine index(value: &mut u64) -> u64 [0..=1] { value = 1; 0 }
    machine signed_index(value: &mut u64) -> i32 [0..=1] { value = 1; 0 }
    machine recursive(value: &mut u64) -> u64 [0..=1] { recursive(value) }
    machine identity(value: u64) -> u64 { value }
    machine pair_index(value: Pair) -> u64 { value.first }
    machine write_value(value: &mut u64) { value = 1; }
    machine return_value(value: &mut u64) -> &mut u64 { value }
    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] { cells }
    machine Main::direct(&mut self) {
        let alias: &mut u64 = &mut self.bucket.cells[
            (index(&mut self.audit) + index(&mut self.other)) % 2
        ];
        alias = 2;
    }
    "#
    .to_owned();
    for (name, body, index, _, _, _) in cases {
        let body = body.replace("$INDEX", index);
        let return_lifetime = if matches!(name, "terminal" | "view_terminal") {
            "cells"
        } else {
            "value"
        };
        source.push_str(&format!(
            "machine after_{name}<'cells, 'backup, 'value, 'audit, 'other>(
                cells: &'cells mut [u64; 2], backup: &'backup mut [u64; 2],
                value: &'value mut u64, audit: &'audit mut u64, other: &'other mut u64
            ) -> &'{return_lifetime} mut u64 {{ {body} }}
            machine Main::{name}(&mut self) {{
                let alias: &mut u64 = after_{name}(&mut self.bucket.cells, &mut self.backup,
                    &mut self.value, &mut self.audit, &mut self.other);
                alias = 2;
            }}"
        ));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    // Includes malformed index roots to pin the conservative pre-validation
    // frame result. Numeric eligibility and bounds belong to separate checks.
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = validation::CallFrameResolver::new(&typed).expect("symbol cache");
    for (name, complete, writes_value, writes_backup) in cases
        .into_iter()
        .map(|(name, _, _, complete, writes_value, writes_backup)| {
            (name, complete, writes_value, writes_backup)
        })
        .chain([("direct", true, false, false)])
    {
        let qualified = format!("Main::{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        if complete {
            let mut expected = vec!["self.audit", "self.bucket.cells", "self.other"];
            if writes_value {
                expected.push("self.value");
            }
            if writes_backup {
                expected.push("self.backup");
            }
            expected.sort();
            let mut actual = frame
                .complete_paths()
                .unwrap_or_else(|| panic!("{name} must be complete"))
                .to_vec();
            actual.sort();
            assert_eq!(
                actual, expected,
                "{name} must preserve the nearest collection and every index write"
            );
        } else {
            assert!(!frame.is_complete(), "{name} must remain opaque");
        }
    }
}
