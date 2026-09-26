//! A composed machine returns a record carrying a shared byte-view member:
//! `pick` returns `Handle<'e> { name: &'e [u8]; tag: u64 }` built from its
//! borrowed parameter on each of two state-graph arms. The result signature
//! admits the record as an owned carrier of scalars plus shared view
//! descriptors — the `&` members keep their own statically named loans while
//! the record itself copies whole.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;

const SOURCE: &str = r#"
    data Handle<'e> { name: &'e [u8]; tag: i32; }
    machine pick<'e>(s: &'e [u8], flag: bool) -> Handle<'e> {
        transition flag {
            true -> left(s)
            _ -> right(s)
        }
        state left(s: &'e [u8]) -> Handle<'e> { Handle { name: s, tag: 1 } }
        state right(s: &'e [u8]) -> Handle<'e> { Handle { name: s, tag: 2 } }
    }
"#;

fn lower() -> checked_trees_to_lowered_psi::lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("pick"))
        .expect("a borrowed byte-view member result lowers")
}

#[test]
fn a_shared_byte_view_member_result_lowers() {
    let lowered = lower();
    // Each arm's literal mints a whole view descriptor via a leaf copy of the
    // borrowed parameter, then establishes the record with that descriptor as
    // an owned member — the view's shared authority stays declared on the
    // record's field, never on the field's custody.
    let establishes = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishRecord { .. }
            )
        })
        .count();
    assert_eq!(establishes, 2);
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    assert_eq!(module, lowered.semantic_module);
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
}
