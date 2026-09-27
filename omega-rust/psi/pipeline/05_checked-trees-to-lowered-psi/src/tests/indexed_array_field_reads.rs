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
