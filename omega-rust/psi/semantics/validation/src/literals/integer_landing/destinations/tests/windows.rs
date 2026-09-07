use super::*;

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
fn projected_windows_admit_large_leaves_at_the_selected_element_type() {
    for (element_type, expected_count) in [("u16", 2), ("u8", 0), ("f64", 0)] {
        for selection in ["1..3", "1..=2", "..2"] {
            let source = format!(
                "data Packet {{ values: [{element_type}; 4]; }}
                 machine fill(packet: &write Packet) {{
                     packet.values[{selection}] = [({LARGE_ARGUMENT}) * 513, 8];
                 }}"
            );
            assert_eq!(
                width_grants(&typed(&source)).len(),
                expected_count,
                "{source}"
            );
        }
    }
    let source = format!(
        "machine fill(values: &write [u16; 4]) {{
             let start: usize = 1; let copy: usize = start;
             let end: usize = 2; let last: usize = end;
             values[copy..=last] = [{LARGE_ARGUMENT}, 8];
         }}"
    );
    assert_eq!(width_grants(&typed(&source)).len(), 2);
}

#[test]
fn window_landing_requires_directed_known_in_range_bounds_and_matching_width() {
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
        ("let mut start: usize = 1;", "start..3"),
        ("let start: usize = 0 + 1;", "start..3"),
        ("let start: usize = bound;", "start..3"),
        ("", "bound..3"),
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
fn windows_require_fixed_collections_and_literal_replacements() {
    for source in [
        format!("machine fill(values: &mut [u16]) {{ values[1..3] = [{LARGE_ARGUMENT}, 8]; }}"),
        format!(
            "machine fill<const N: u64>(values: &mut [u16; N]) {{ values[1..3] = [{LARGE_ARGUMENT}, 8]; }}"
        ),
        format!("machine fill(values: &write [u16; 4]) {{ values[1..3] = {LARGE_ARGUMENT}; }}"),
        "machine fill(values: &mut [u16]) { values[1..3] = [0.1 * 70, 8]; }".to_owned(),
        "machine fill<const N: u64>(values: &mut [u16; N]) { values[1..3] = [0.1 * 70, 8]; }"
            .to_owned(),
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
fn window_elements_preserve_typed_arithmetic_and_exact_landing_failures() {
    for (element_type, element) in [
        ("u16", "7u16 / 2 * 2"),
        ("u16", "7 / 2"),
        ("u8", "513 / 2 * 2"),
        ("f64", "0.1 * 70"),
        ("u16", "18446744073709551617 / 2"),
        ("u16", "18446744073709551616"),
        ("u16", "18446744073709551616 / 0"),
        ("u16", "18446744073709551616 / 18446744073709551616u64"),
    ] {
        let source = format!(
            "machine fill(values: &write [{element_type}; 4]) {{ values[1..3] = [{element}, 8]; }}"
        );
        let program = typed(&source);
        assert!(
            anonymous_integer_landing_warnings(&program).is_empty(),
            "{source}"
        );
        assert!(width_grants(&program).is_empty(), "{source}");
    }
}

#[test]
fn window_width_grants_do_not_escape_to_shared_unsupported_windows() {
    for (other_type, other_selection) in [("f64", "1..3"), ("u16", "0..4"), ("u16", "3..5")] {
        let source = format!(
            "machine fill(values: &write [u16; 4], other: &mut [{other_type}; 4]) {{
                 values[1..3] = [{LARGE_ARGUMENT}, 8];
                 other[{other_selection}] = [0, 0];
             }}"
        );
        let mut program = typed(&source);
        assert_eq!(width_grants(&program).len(), 2, "{source}");
        let arrays: Vec<_> = program
            .expression_table
            .expression_entries()
            .filter_map(|(handle, node)| {
                if let ExpressionNode::ArrayLiteral(elements) = node {
                    Some((handle, *elements))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(arrays.len(), 2);
        *program.expression_table.expression_mut(arrays[1].0) =
            ExpressionNode::ArrayLiteral(arrays[0].1);
        assert!(width_grants(&program).is_empty(), "{source}");
    }
}

#[test]
fn unsupported_array_parents_retain_window_element_width_obligations() {
    let mut program = typed(&format!(
        "machine fill(values: &write [u16; 4]) {{ values[1..3] = [{LARGE_ARGUMENT}, 8]; }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let elements = program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| {
            if let ExpressionNode::ArrayLiteral(elements) = node {
                Some(*elements)
            } else {
                None
            }
        })
        .expect("replacement array");
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements));
    assert!(width_grants(&program).is_empty());
}

#[test]
fn a_typed_window_sibling_retains_shared_subtree_width_obligations() {
    let mut program = typed(&format!(
        "machine fill(values: &write [u16; 4]) {{
             values[1..3] = [{LARGE_ARGUMENT}, 1u16 * 0];
         }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let elements = program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| {
            if let ExpressionNode::ArrayLiteral(elements) = node {
                Some(*elements)
            } else {
                None
            }
        })
        .expect("replacement array");
    let [anonymous, typed] = *program.expression_table.expression_handles(elements) else {
        panic!("two window elements");
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression_mut(typed) else {
        panic!("typed multiplication");
    };
    binary.right = anonymous;
    assert!(width_grants(&program).is_empty());
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
