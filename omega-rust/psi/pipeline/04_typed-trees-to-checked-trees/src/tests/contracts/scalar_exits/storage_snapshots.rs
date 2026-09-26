use super::check;

#[test]
fn storage_initializers_and_assignments_preserve_selected_results() {
    for (value_type, computation, expected) in [
        ("u8", "3 + 4", "7"),
        ("u8", "(255 + 1) - 1", "255"),
        (
            "u64",
            "(18446744073709551615 + 1) - 1",
            "18446744073709551615u64",
        ),
        ("i8", "(0 - 129) + 1", "-128"),
        ("u8", "((255u8 as u8 in Wrapping) + 2) as u8", "1"),
        ("u8", "((255u8 as u8 in Saturating) + 2) as u8", "255"),
        ("bool", "false || true", "true"),
    ] {
        let zero = if value_type == "bool" { "false" } else { "0" };
        for body in [
            format!("let mut stored: {value_type} = {computation}; stored"),
            format!("let mut stored: {value_type} = {zero}; stored = {computation}; stored"),
        ] {
            for accepted in [true, false] {
                let comparison = if accepted { "==" } else { "!=" };
                check(
                    &format!(
                        "machine produce() -> {value_type} ensures result {comparison} {expected} {{ {body} }}"
                    ),
                    accepted,
                );
            }
        }
    }
}
