//! A multi-state scalar graph on an attached machine keeps the ambient
//! `&self` receiver on the entry roster only, while non-entry structural
//! formals forward across the edges that reach them. This is the
//! `SnapshotRegionFilter::get_element_count` shape: an Alignment formal rides
//! the jump into the divide state, which also reads the ambient receiver.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    data Alignment [copy] { bytes: u64; }
    data Filter [copy] { width: u64; }
    machine Filter::region_size(&self) -> u64 {
        self.width
    }
    // Interning `Filter` as an authored nominal type is what the real filter
    // gets from its enclosing scan types; without it the shared-receiver
    // argument cannot resolve the attachment's declared type.
    machine noop(other: Filter) -> u64 {
        42
    }
    machine Filter::count(&self, width: u64, alignment: Alignment) -> u64
    crashes Abort
    {
        transition alignment.bytes > 0 {
            true -> divide(alignment)
            false -> violated()
        }
        state violated(&self) -> u64 {
            crash Abort;
        }
        state divide(&self, alignment: Alignment) -> u64 {
            let size: u64 = self.region_size();
            transition {
                _ -> (size / alignment.bytes)
            }
        }
    }
"#;

#[test]
fn multi_state_scalar_graph_forwards_structural_formals_on_edges() {
    let checked = crate::front_end::checked_program(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Filter::count"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed multi-state scalar graph lowers its forwarded structural formals")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("forwarded multi-state scalar graph independently verifies");

    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("count entry machine");
    let [receiver, alignment] = entry.structural_parameters.as_slice() else {
        panic!("entry retains the ambient receiver and the forwarded formal")
    };
    assert!(receiver.is_self);
    assert!(!alignment.is_self);
    let forwarded = alignment.place;
    let reads_alignment = entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| match &operation.kind {
                OperationKind::IntegerStructuralField { source, .. } => *source == forwarded,
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments
                    .iter()
                    .any(|argument| argument.place == forwarded),
                _ => false,
            })
    });
    assert!(
        reads_alignment,
        "a block reads the forwarded Alignment formal"
    );
}
