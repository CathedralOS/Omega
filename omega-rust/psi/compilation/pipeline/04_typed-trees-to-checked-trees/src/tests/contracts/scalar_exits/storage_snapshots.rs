use super::check;

#[test]
fn borrowed_parameter_exit_bounds_use_current_field_arithmetic() {
    for (parameters, receiver) in [
        ("&mut self, start: u32 in Wrapping", "self"),
        ("timer: &mut Countdown, start: u32 in Wrapping", "timer"),
        ("start: u32 in Wrapping, timer: &mut Countdown", "timer"),
    ] {
        check(
            &format!(
                "data Countdown {{ remaining: u32 in Wrapping; }}
                 machine Countdown::arm({parameters})
                 requires start < 8
                 ensures {receiver}.remaining > 0; {receiver}.remaining - 1 < 8
                 {{ {receiver}.remaining = start + 1; }}"
            ),
            true,
        );
    }
    check(
        "data Countdown { remaining: u32 in Wrapping; }
         data Clock { timer: Countdown; }
         machine arm(clock: &mut Clock, start: u32 in Wrapping)
         requires start < 8
         ensures clock.timer.remaining > 0; clock.timer.remaining - 1 < 8
         { clock.timer.remaining = start + 1; }",
        true,
    );
}

#[test]
fn borrowed_parameter_exit_bounds_reject_stale_or_wrapping_values() {
    for (requirement, body) in [
        ("start < 8", "timer.remaining = start;"),
        ("start < 9", "timer.remaining = start + 1;"),
        (
            "start < 8",
            "timer.remaining = start + 1; timer.remaining = 0;",
        ),
        ("start < 8", "timer.remaining = start + 1; clear(timer);"),
    ] {
        check(
            &format!(
                "data Countdown {{ remaining: u32 in Wrapping; }}
                 machine clear(timer: &mut Countdown) {{ timer.remaining = 0; }}
                 machine Countdown::arm(timer: &mut Countdown, start: u32 in Wrapping)
                 requires {requirement}
                 ensures timer.remaining > 0; timer.remaining - 1 < 8
                 {{ {body} }}"
            ),
            false,
        );
    }
}

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
