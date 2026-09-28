use super::{OperationKind, SOURCE, publish_original};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn independent_verifier_rejects_local_place_tampering_after_valid_publication() {
    let original = publish_original(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let foreign_place = module
        .machines
        .iter()
        .find_map(|machine| machine.structural_parameters.first())
        .expect("stamp owns a same-typed borrowed parameter")
        .place;
    for mutation in ["read source", "store destination", "missing establishment"] {
        let mut changed = module.clone();
        let root = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let mut mutations = 0;
        for block in &mut root.blocks {
            if mutation == "missing establishment" {
                let previous = block.operations.len();
                block.operations.retain(|operation| {
                    !matches!(
                        operation.kind,
                        OperationKind::EstablishPrimitiveLocal { .. }
                    )
                });
                mutations += previous - block.operations.len();
                continue;
            }
            for operation in &mut block.operations {
                match &mut operation.kind {
                    OperationKind::PrimitiveScalarRead { source, path }
                        if mutation == "read source" =>
                    {
                        assert!(path.is_empty());
                        assert_ne!(*source, foreign_place);
                        *source = foreign_place;
                        mutations += 1;
                    }
                    OperationKind::WriteOnlyPrimitiveStore { destination, .. }
                        if mutation == "store destination" =>
                    {
                        assert_ne!(*destination, foreign_place);
                        *destination = foreign_place;
                        mutations += 1;
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            mutations, 1,
            "one retained local operation tampered: {mutation}"
        );
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "verifier accepted {mutation}"
        );
    }
}
