use super::{LARGE_ARGUMENT, anonymous_integer_landing_warnings, typed, width_grants};

#[test]
fn projected_window_destinations_report_exact_fractional_origins() {
    for source in [
        "machine fill(values: &write [u16; 4]) { values[1..3] = [0.1 * 70, 7u16 / 2 * 2]; }",
        "machine fill(values: &mut [u16; 4]) { values[1..=2] = [0.1 * 70, 8]; }",
        "machine fill(values: &write [u16; 4]) { values[..2] = [0.1 * 70, 8]; }",
        "machine fill(values: &write [u16; 4]) { let start: usize = 1; let copy: usize = start; let end: usize = 3; values[copy..end] = [0.1 * 70, 8]; }",
        "data Packet { values: [u16; 4]; } machine fill(packet: &write Packet) { packet.values[1..3] = [0.1 * 70, 8]; }",
        "data Packet { values: [u16; 4]; } machine Packet::fill(&write self) { self.values[1..3] = [0.1 * 70, 8]; }",
        "data Packet { values: [u16; 4]; } machine fill(packets: &mut [Packet; 2]) { packets[1].values[1..3] = [0.1 * 70, 8]; }",
        "machine fill(values: &mut [[u16; 4]; 2]) { values[1][1..3] = [0.1 * 70, 8]; }",
    ] {
        let warnings = anonymous_integer_landing_warnings(&typed(source));
        let [warning] = warnings.as_slice() else {
            panic!("{source}: {warnings:?}");
        };
        assert!(warning.message.contains("fractional intermediate `1/10`"));
        assert!(warning.message.contains("integer `7`"));
        let offset = source.find("0.1").expect("fractional window element");
        assert_eq!(
            warning.source_span.expect("authored origin").span,
            source::Span::new(offset, offset + 3)
        );
    }
}

#[test]
fn statically_impossible_window_bounds_and_sizes_do_not_grant_integer_widths() {
    for (locals, selection) in [
        ("", "3..1"),
        ("", "3..=1"),
        ("", "3..5"),
        ("", "3..=4"),
        ("", "5..5"),
        ("", "1..2"),
        ("", "0..4"),
        ("", "2..2"),
        ("", "1.."),
        ("", ".."),
        ("", "-1..1"),
        ("", "1..18446744073709551615"),
        ("", "1..=18446744073709551615"),
    ] {
        for element in ["0.1 * 70", LARGE_ARGUMENT] {
            let source = format!(
                "machine fill(values: &write [u16; 4], bound: usize) {{
                     {locals} values[{selection}] = [{element}, 8];
                 }}"
            );
            let program = typed(&source);
            assert!(
                anonymous_integer_landing_warnings(&program).is_empty(),
                "{source}"
            );
            assert!(width_grants(&program).is_empty(), "{source}");
        }
    }
}

#[test]
fn windows_require_literal_replacements() {
    for source in [
        format!("machine fill(values: &write [u16; 4]) {{ values[1..3] = {LARGE_ARGUMENT}; }}"),
        "machine fill(values: &write [u16; 4]) { values[1..3] = 0.1 * 70; }".to_owned(),
    ] {
        let program = typed(&source);
        assert!(
            anonymous_integer_landing_warnings(&program).is_empty(),
            "{source}"
        );
        assert!(width_grants(&program).is_empty(), "{source}");
    }
}

#[test]
fn runtime_and_generic_lengths_preserve_element_widths_and_warnings() {
    for (generics, collection, locals, selection) in [
        ("", "[u16]", "", "1..3"),
        ("<const N: u64>", "[u16; N]", "", "1..3"),
        ("<const N: u64>", "[u16; N]", "", "2.."),
        ("", "[u16; 4]", "let mut start: usize = 1;", "start..3"),
        ("", "[u16; 4]", "let start: usize = 0 + 1;", "start..3"),
        ("", "[u16; 4]", "let start: usize = bound;", "start..3"),
        ("", "[u16; 4]", "", "bound..3"),
        ("", "[u16]", "", "bound.."),
    ] {
        let source = |element| {
            format!(
                "machine fill{generics}(values: &mut {collection}, bound: usize) {{ {locals} values[{selection}] = [{element}, 8]; }}"
            )
        };
        assert_eq!(width_grants(&typed(&source(LARGE_ARGUMENT))).len(), 2);
        let warnings = anonymous_integer_landing_warnings(&typed(&source("0.1 * 70")));
        assert_eq!(warnings.len(), 1, "{}: {warnings:?}", source("0.1 * 70"));
        assert!(
            warnings[0]
                .message
                .contains("fractional intermediate `1/10`")
        );
    }
}

#[test]
fn non_scalar_windows_preserve_explicit_constructor_field_width_grants() {
    for (definition, constructor) in [
        ("data Record [copy] { value: u16; }", "Record"),
        (
            "data Record [copy] { case Value(value: u16); }",
            "Record::Value",
        ),
    ] {
        for (element_type, element) in [
            (
                "Record",
                format!("{constructor} {{ value: {LARGE_ARGUMENT} }}"),
            ),
            (
                "[Record; 1]",
                format!("[{constructor} {{ value: {LARGE_ARGUMENT} }}]"),
            ),
            (
                "[[Record; 1]; 1]",
                format!("[[{constructor} {{ value: {LARGE_ARGUMENT} }}]]"),
            ),
        ] {
            let source = format!(
                "{definition}
                 machine fill(values: &write [{element_type}; 4]) {{
                     values[1..2] = [{element}];
                 }}"
            );
            let program = typed(&source);
            assert_eq!(width_grants(&program).len(), 2, "{source}");
        }
    }
}
