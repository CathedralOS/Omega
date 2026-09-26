use crate::tests::front_end::checked_program_result;

fn assert_range_rejection(diagnostics: &[diagnostics::Diagnostic], source: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("cannot prove subslice range") }),
        "expected subslice range rejection, not an earlier proof failure: {diagnostics:#?}\n{source}"
    );
}

fn check_range(source: &str, accepted: bool) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "stale computed bounds accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "check: {diagnostics:#?}\n{source}");
            assert_range_rejection(&diagnostics, source);
        }
    }
}

#[test]
fn builtin_unary_and_cast_operands_keep_their_storage_dependencies() {
    for (scalar, expression) in [
        ("i64 [0..=5]", "-(original - 1)"),
        ("i32 [0..=5]", "(original as i64) - 1"),
    ] {
        for (mutation, accepted) in [
            ("", true),
            ("unrelated = 1;", true),
            ("original = replacement;", false),
        ] {
            check_range(
                &format!(
                    "machine window(items: &[i32; 4], mut original: {scalar},
                         replacement: {scalar}) -> u64
                     requires 0 <= {expression} && {expression} <= 4; {{
                         let mut unrelated: i64 = 0;
                         {mutation}
                         let cut: i64 = {expression};
                         let view: &[i32] = items[0..cut]; view.len
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn receiver_field_dependencies_match_explicit_and_bare_write_paths() {
    for original in ["self.original", "original"] {
        for (mutation, accepted) in [
            ("", true),
            ("self.unrelated = replacement;", true),
            ("unrelated = replacement;", true),
            ("set(&mut self.unrelated, replacement);", true),
            ("self.original = replacement;", false),
            ("original = replacement;", false),
            ("set(&mut self.original, replacement);", false),
        ] {
            check_range(
                &format!(
                    "data Main {{ original: i64 [0..=5]; unrelated: i64 [0..=5]; }}
                     machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) {{
                         output = replacement;
                     }}
                     machine Main::window(&mut self, items: &[i32; 4],
                         replacement: i64 [0..=4]) -> u64
                     requires 0 <= {original} - 1 && {original} - 1 <= 4; {{
                         {mutation}
                         let cut: i64 = {original} - 1;
                         let view: &[i32] = items[0..cut]; view.len
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn guarded_true_and_false_routes_preserve_only_unwritten_computed_bounds() {
    for (guard, first, selected) in [
        ("original - 1 >= 0", "false", "true"),
        ("original - 1 < 0", "true", "false"),
    ] {
        for (prefix, accepted) in [
            ("0", true),
            ("set(&mut unrelated, replacement)", true),
            ("set(&mut original, replacement)", false),
        ] {
            // Chapter 5 schedules arguments left to right. Both the write and
            // the subslice access run under this arm's same-state guard facts.
            check_range(
                &format!(
                    "machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) -> u64 {{
                         output = replacement; 0
                     }}
                     machine combine(ignored: u64, result: u64) -> u64 {{ result }}
                     machine window(items: &[i32; 4], mut original: i64 [0..=5],
                         replacement: i64 [0..=4]) -> u64
                     requires original - 1 <= 4; {{
                         let mut unrelated: i64 [0..=5] = 0;
                         transition {guard} {{
                             {first} -> 0
                             {selected} -> (combine({prefix}, items[0..original - 1].len))
                         }}
                     }}"
                ),
                accepted,
            );
        }
    }
}

/// An explicit `&`/`&mut` inside a premise's place chain addresses exactly its
/// target's storage: `(&pair).a` reads `pair.a`, so the premise survives
/// writes the projected place cannot observe and is retired by a write the
/// place can — while a known-value write answers the bound from the fresh
/// fact it installs instead of the retired premise.
#[test]
fn borrowed_member_premise_reads_exactly_the_projected_place() {
    fn source(mutation: &str, boundary: &str) -> String {
        format!(
            "data Pair {{ a: i64; b: i64; }}
             machine window(items: &[i32; 4], mut pair: Pair, mut unrelated: i64,
                 replacement: i64 [0..=5]) -> u64
             requires 0 <= (&pair).a && (&pair).a <= 4; {{
                 {mutation}
                 let view: &[i32] = items[0..{boundary}];
                 view.len
             }}"
        )
    }
    for (mutation, boundary, accepted) in [
        ("", "(&pair).a", true),
        ("unrelated = 1;", "(&pair).a", true),
        ("pair.b = 2;", "(&pair).a", true),
        // The write retires the premise but installs `pair.a == 2`, which
        // proves the same bound — evidence may discharge the range, never
        // carry a stale premise past an overlapping write.
        ("pair.a = 2;", "(&pair).a", true),
        ("pair.a = replacement;", "(&pair).a", false),
        ("pair = Pair { a: replacement, b: 2 };", "(&pair).a", false),
        // A copy taken before the write names the frozen value, not `pair.a`'s
        // current storage, so it keeps its bounds across the mutation.
        (
            "let cut: i64 = (&pair).a; pair.a = replacement;",
            "cut",
            true,
        ),
    ] {
        check_range(&source(mutation, boundary), accepted);
    }
}
