use crate::tests::front_end::checked_program_result;

fn accepted(source: &str) {
    checked_program_result(source).expect("the copied value keeps its established bounds");
}

fn rejected_index(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("the index lacks a bound");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{diagnostics:?}"
    );
}

#[test]
fn field_bounds_follow_scalar_bindings_and_state_arguments() {
    accepted(
        "data Ring where head < 8 { slots: [u8; 8]; head: u64; }
        machine read(ring: &Ring) -> u8 {
            let slot: u64 = ring.head;
            let copied: u64 = slot;
            transition { _ -> finish(ring, copied) }
            state finish(ring: &Ring, position: u64) -> u8 { ring.slots[position] }
        }",
    );
    accepted(
        "data Ring where 0 <= head, head < 8 { slots: [u8; 8]; head: i32; }
        machine read(ring: &Ring) -> u8 {
            let slot: i32 = ring.head;
            ring.slots[slot]
        }",
    );
}

#[test]
fn parameter_bounds_follow_explicit_and_synthesized_bindings() {
    for index in ["copied", "position + 1"] {
        accepted(&format!(
            "machine read(values: &[u8; 8], position: u64) -> u8
            requires position < 7 {{
                let copied: u64 = position + 1;
                values[{index}]
            }}"
        ));
    }
}

#[test]
fn reassignment_does_not_keep_the_initializers_bound() {
    rejected_index(
        "data Ring where head < 8 { slots: [u8; 8]; head: u64; }
        machine read(ring: &Ring, unknown: u64) -> u8 {
            let mut slot: u64 = ring.head;
            slot = unknown;
            ring.slots[slot]
        }",
    );
}

#[test]
fn a_signed_alias_still_needs_a_nonnegative_bound() {
    rejected_index(
        "data Ring where head < 8 { slots: [u8; 8]; head: i32; }
        machine read(ring: &Ring) -> u8 {
            let slot: i32 = ring.head;
            ring.slots[slot]
        }",
    );
}
