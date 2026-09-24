//! Indexed reads `self.bytes[position]` on fixed-array record fields: the
//! index is a runtime operand, not a path segment, so the read lowers to an
//! `IndexedPrimitiveRead` whose obligation `index < declared extent` the
//! verifier proves from the array shape at the path's leaf. This is the
//! census family's `indexed reads require a whole view/byte-view parameter`
//! wall: the leaf is `Structural(FixedArray{PrimitiveScalar})`, not a
//! byte-sequence carrier, so neither the view-observation arm nor the
//! byte-sequence field arm applied.
//!
//! The runtime bound is discharged by the ordinary fact machinery — a guard
//! conjunct `position < 16` transports to the read site. An unguarded read
//! still declines at verification, and drift between the element type and
//! the result declines at the read's validation.
use crate::{TerminalMachineSelection, lower_machine};

fn verify(source: &str, machine: &'static str) {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
        .expect("indexed fixed-array field read composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("indexed fixed-array field read verifies");
}

fn rejected(source: &str, machine: &'static str) {
    let Ok(checked) = crate::front_end::checked_program_result(source) else {
        return;
    };
    let Ok(lowered) = lower_machine(&checked, TerminalMachineSelection::Name(machine)) else {
        return;
    };
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "this indexed fixed-array field read must not verify"
    );
}

const STORAGE_SIZE_TEXT: &str = r#"
    pub data StorageSizeText {
        bytes: [u8; 16];
        len: u64;
    }
"#;

#[test]
fn storage_size_text_get_byte_lowers_and_verifies() {
    verify(
        &format!(
            "{STORAGE_SIZE_TEXT}
            pub machine StorageSizeText::get_byte(&self, position: u64) -> u8 {{
                transition position < self.len && position < 16 {{
                    true -> (self.bytes[position])
                    false -> (0)
                }}
            }}"
        ),
        "StorageSizeText::get_byte",
    );
}

#[test]
fn indexed_array_field_read_result_value_lowers_and_verifies() {
    verify(
        &format!(
            "{STORAGE_SIZE_TEXT}
            machine StorageSizeText::byte_at(&self, position: u64) -> u8 {{
                transition position < 16 {{
                    true -> (self.bytes[position])
                    false -> (0)
                }}
            }}"
        ),
        "StorageSizeText::byte_at",
    );
}

#[test]
fn indexed_non_byte_array_field_read_lowers_and_verifies() {
    verify(
        r#"
        data Table { values: [u64; 8]; }
        machine Table::entry(&self, index: u64) -> u64 {
            transition index < 8 {
                true -> (self.values[index])
                false -> (0)
            }
        }
        "#,
        "Table::entry",
    );
}

#[test]
fn indexed_array_field_read_unguarded_rejects() {
    rejected(
        &format!(
            "{STORAGE_SIZE_TEXT}
            machine StorageSizeText::byte_at(&self, position: u64) -> u8 {{
                self.bytes[position]
            }}"
        ),
        "StorageSizeText::byte_at",
    );
}

#[test]
fn indexed_array_field_read_wrong_bound_rejects() {
    rejected(
        &format!(
            "{STORAGE_SIZE_TEXT}
            machine StorageSizeText::byte_at(&self, position: u64) -> u8 {{
                transition position < 17 {{
                    true -> (self.bytes[position])
                    false -> (0)
                }}
            }}"
        ),
        "StorageSizeText::byte_at",
    );
}

// Terminal element reads take a u64 position. Any integer carrier may index
// once its bounds are proven, so a signed or narrower index lands through an
// exact cast whose obligation the dominating guard discharges.

#[test]
fn indexed_array_field_read_with_signed_index_lowers_and_verifies() {
    verify(
        r#"
        data Table { values: [u64; 8]; }
        machine Table::entry(&self, index: i32) -> u64 {
            transition index >= 0 && index < 8 {
                true -> (self.values[index])
                false -> (0)
            }
        }
        "#,
        "Table::entry",
    );
}

#[test]
fn indexed_array_field_read_with_narrow_unsigned_index_lowers_and_verifies() {
    verify(
        r#"
        data Table { values: [u64; 8]; }
        machine Table::entry(&self, index: u32) -> u64 {
            transition index < 8 {
                true -> (self.values[index])
                false -> (0)
            }
        }
        "#,
        "Table::entry",
    );
}

#[test]
fn indexed_array_field_read_with_unbounded_signed_index_rejects() {
    rejected(
        r#"
        data Table { values: [u64; 8]; }
        machine Table::entry(&self, index: i32) -> u64 {
            transition index < 8 {
                true -> (self.values[index])
                false -> (0)
            }
        }
        "#,
        "Table::entry",
    );
}
