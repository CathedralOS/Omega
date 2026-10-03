#[test]
fn observed_integer_ceiling_survives_the_comparison_temporary() {
    for (carrier, ceiling_carrier, comparison, increment, accepted) in [
        (
            "u64",
            "u64",
            "position < Values::count(selected)",
            "1",
            true,
        ),
        (
            "u32",
            "u32",
            "position < Values::count(selected)",
            "1",
            true,
        ),
        (
            "u64",
            "u64",
            "Values::count(selected) > position",
            "1",
            true,
        ),
        (
            "u64",
            "u64",
            "!(position >= Values::count(selected))",
            "1",
            true,
        ),
        (
            "u64",
            "u64",
            "position <= Values::count(selected)",
            "1",
            false,
        ),
        (
            "u64",
            "u64",
            "position < Values::count(selected)",
            "2",
            false,
        ),
        (
            "u8",
            "u64",
            "position < Values::count(selected)",
            "1",
            false,
        ),
        ("u64", "u8", "position < Values::count(selected)", "1", true),
    ] {
        let source = format!(
            "data Values {{ length: {ceiling_carrier}; }}
             machine Values::count(values: &Values) -> {ceiling_carrier} {{ values.length }}
             machine visit(selected: &Values, position: {carrier}) -> {carrier} {{
                 transition {comparison} {{
                     true -> advance(position)
                     false -> 0
                 }}
                 state advance(position: {carrier}) -> {carrier} {{ position + {increment} }}
             }}"
        );
        let result = crate::validate_program(&crate::front_end::typed_program(&source));
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
}

#[test]
fn observed_ceiling_tracks_the_subject_and_branch() {
    for (comparison, argument, preparation) in [
        ("position >= Values::count(selected)", "position", ""),
        ("position < Values::count(selected)", "other", ""),
        (
            "position < Values::count(selected)",
            "position",
            "position = other;",
        ),
    ] {
        let source = format!(
            "data Values {{ length: u64; }}
             machine Values::count(values: &Values) -> u64 {{ values.length }}
             machine visit(selected: &Values, mut position: u64, other: u64) -> u64 {{
                 transition {comparison} {{
                     true -> advance({argument}, other)
                     false -> 0
                 }}
                 state advance(mut position: u64, other: u64) -> u64 {{
                     {preparation}
                     position + 1
                 }}
             }}"
        );
        let diagnostics = crate::validate_program(&crate::front_end::typed_program(&source))
            .expect_err("an unrelated or overwritten scalar has no observed strict bound");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("may overflow `u64`")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn observed_ceiling_does_not_outlive_writes_inside_the_guard() {
    for guard in [
        "position < overwrite(&mut position)",
        "position < Values::count(selected) && overwrite(&mut position) > 0",
    ] {
        let source = format!(
            "data Values {{ length: u64; }}
             machine Values::count(values: &Values) -> u64 {{ values.length }}
             machine overwrite(position: &mut u64) -> u64 {{
                 position = 18446744073709551615;
                 18446744073709551615
             }}
             machine visit(selected: &Values, mut position: u64) -> u64 {{
                 transition {guard} {{
                     true -> advance(position)
                     false -> 0
                 }}
                 state advance(position: u64) -> u64 {{ position + 1 }}
             }}"
        );
        let diagnostics = crate::validate_program(&crate::front_end::typed_program(&source))
            .expect_err("the comparison observed storage before it was overwritten");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("may overflow `u64`")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn receiver_results_require_established_and_valid_bounds() {
    for (return_type, body, diagnostic) in [
        ("u64", "1", None),
        (
            "u64",
            "alignment.width",
            Some("local `width` stores a value not provably within its declared range"),
        ),
        (
            "u64 [1..=8]",
            "9",
            Some("returns a value not provably within its declared range"),
        ),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "data Alignment [copy] {{ width: u64; }}
             machine Alignment::size(alignment: &Alignment) -> {return_type} {{ {body} }}
             machine consume(alignment: &Alignment) -> u64 {{
                 let width: u64 [1..=8] = Alignment::size(alignment);
                 width
             }}"
        ));
        let result = crate::validate_program(&program);
        if let Some(diagnostic) = diagnostic {
            let diagnostics = result.expect_err("unproved range");
            assert!(
                format!("{diagnostics:?}").contains(diagnostic),
                "{diagnostics:?}"
            );
        } else {
            assert!(
                result.is_ok(),
                "a body-established bound is usable: {result:?}"
            );
        }
    }
}
