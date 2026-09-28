use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{StructuralTypeShape, TerminalModule};

pub fn check(source: &str) -> CheckedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    crate::front_end::checked_program_from_source_map(sources, &[(source_id, source)])
}

pub fn publish(source: &str, entry: &str) -> (CheckedTrees, TerminalModule, Vec<u8>, Vec<u8>) {
    let checked = check(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("lower owned scalar graph {entry}: {error:?}\n{source}"));
    let debug = lowered
        .debug_map
        .expect("source-backed graph has a debug map");
    assert!(!debug.sites.is_empty());
    let debug_bytes = terminal_codec::encode_debug_map(&lowered.semantic_module, &debug).unwrap();
    assert_eq!(
        terminal_codec::decode_debug_map(&lowered.semantic_module, &debug_bytes).unwrap(),
        debug
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish owned scalar graph closure")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent canonical verification");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    (
        checked,
        module,
        artifact.semantic_bytes().to_vec(),
        artifact.proof_bytes().to_vec(),
    )
}

pub fn field(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    identity: &str,
) -> StructuralFieldId {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .expect("retained input type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("record input");
    };
    fields
        .iter()
        .find(|field| field.identity == identity)
        .expect("exact authored field")
        .id
}

pub fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::TerminalProductionRequest::new(
            checked,
            TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "accepted owned scalar custody mutation: {mutation}"
    );
}
