use super::candidate_position;
use crate::RegisterHomeError;
use selected_instructions::VirtualRegisterId;

fn pressure(function: usize, register: u32) -> RegisterHomeError {
    RegisterHomeError::NoCompatibleHome { function, register }
}

#[test]
fn failed_original_register_precedes_lower_numbered_interfering_values() {
    let roster = [(2, VirtualRegisterId(3)), (2, VirtualRegisterId(9))];
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |_| true),
        Some(1)
    );
}

#[test]
fn rejected_failed_register_restores_original_order_and_is_not_retried() {
    let mut roster = vec![
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(5)),
        (2, VirtualRegisterId(9)),
    ];
    let failure = pressure(2, 9);
    let selected = candidate_position(&failure, &roster, |_| true).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(9)));
    // This is the existing removal-before-admission path for unsupported values.
    let selected = candidate_position(&failure, &roster, |_| true).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(3)));
    assert_eq!(candidate_position(&failure, &roster, |_| true), Some(0));
    assert_eq!(roster[0], (2, VirtualRegisterId(5)));
}

#[test]
fn failed_identity_in_another_function_does_not_gain_priority() {
    let roster = [
        (1, VirtualRegisterId(9)),
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(9)),
    ];
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |(function, _)| *function == 2),
        Some(2)
    );
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster[..2], |(function, _)| *function
            == 2),
        Some(1)
    );
}

#[test]
fn absent_or_generated_failed_value_uses_only_overlapping_original_candidates() {
    let roster = [
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(5)),
        (2, VirtualRegisterId(9)),
    ];
    // A generated reload or spill address never enlarges the original roster.
    assert_eq!(
        candidate_position(&pressure(2, 100), &roster, |(_, register)| *register
            == VirtualRegisterId(5)),
        Some(1)
    );
    assert_eq!(
        candidate_position(&pressure(2, 100), &roster, |_| false),
        None
    );
    assert_eq!(candidate_position(&pressure(2, 100), &[], |_| true), None);
}

#[test]
fn nonpressure_failures_do_not_choose_a_runtime_spill() {
    let roster = [(2, VirtualRegisterId(9))];
    assert_eq!(
        candidate_position(
            &RegisterHomeError::NoCommonCandidate {
                function: 2,
                register: 9
            },
            &roster,
            |_| true
        ),
        None
    );
}
