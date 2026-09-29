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

#[test]
fn runtime_record_boolean_read_replays_its_field_and_index_bound() {
    let source = "data Cell { tag: u8; ready: bool; }
        data Buffer { cells: [Cell; 4]; }
        machine Buffer::read(&self, index: u64) -> bool
        requires index < 4 {
            self.cells[index].ready
        }";
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Buffer::read"))
        .expect("Boolean fields use the shared runtime-indexed scalar read");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let decoded = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(decoded, lowered.semantic_module);
    terminal_verifier::verify_module(
        &decoded,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    assert!(
        decoded
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::PrimitiveScalarRead { path, .. }
                    if matches!(path.as_slice(), [
                        terminal_psi::StructuralPathSegment::Field(_),
                        terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                        terminal_psi::StructuralPathSegment::Field(_)
                    ])
            ))
    );
    rejected(&source.replace("index < 4", "index < 5"), "Buffer::read");
    let mut changed = decoded;
    let path = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::PrimitiveScalarRead { path, .. } => Some(path),
            _ => None,
        })
        .unwrap();
    *path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::Field("tag".into());
    assert!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a byte field cannot supply the Boolean read"
    );
}

// Terminal element reads take a u64 position. Any integer carrier may index
// once its bounds are proven, so a signed or narrower index lands through an
// exact cast whose obligation the dominating guard discharges.
