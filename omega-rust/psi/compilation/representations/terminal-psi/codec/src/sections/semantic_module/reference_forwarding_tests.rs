//! Transport and identity of requested forwarding, independently of fulfillment.

use super::normal_qualification_tests;
use crate::{CodecError, encode_module, semantic_fingerprint};
use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{
    BoundaryReferenceForwardingDemand, BoundaryReferenceSource, StructuralPathSegment,
    TerminalIdentityIndex, TerminalModule,
};

fn module() -> TerminalModule {
    let mut module = normal_qualification_tests::module();
    module.boundary_reference_forwarding = vec![BoundaryReferenceForwardingDemand {
        machine: MachineId::new(1).unwrap(),
        operation: OperationId::new(1).unwrap(),
        output: 0,
        sources: vec![
            BoundaryReferenceSource::Reference { argument_index: 0 },
            BoundaryReferenceSource::Stored {
                argument_index: 1,
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(8))],
            },
        ],
    }];
    module
}

#[test]
fn reference_forwarding_wire_and_fingerprint_retain_source_kind_and_coordinates() {
    let module = module();
    let original = semantic_fingerprint(&module).unwrap();
    // Raw body decoding isolates wire transport. Public admission separately
    // validates call/output/source shape and independently proves fulfillment.
    let bytes = encode_module(&module).unwrap();
    let mut reader = super::wire::Reader::new(&bytes);
    assert_eq!(reader.take(super::MAGIC.len()).unwrap(), super::MAGIC);
    assert_eq!(reader.u16().unwrap(), super::FORMAT_MARKER);
    assert_eq!(
        super::module_wire::decode_module_body(&mut reader).unwrap(),
        module
    );
    assert_eq!(reader.remaining(), 0);

    let mut variants = vec![module.clone(); 7];
    variants[0].boundary_reference_forwarding.clear();
    variants[1].boundary_reference_forwarding[0].machine = MachineId::new(2).unwrap();
    variants[2].boundary_reference_forwarding[0].operation = OperationId::new(2).unwrap();
    variants[3].boundary_reference_forwarding[0].output = 1;
    variants[4].boundary_reference_forwarding[0].sources[0] =
        BoundaryReferenceSource::Reference { argument_index: 2 };
    variants[5].boundary_reference_forwarding[0].sources[0] = BoundaryReferenceSource::Stored {
        argument_index: 0,
        path: Vec::new(),
    };
    variants[6].boundary_reference_forwarding[0].sources[1] = BoundaryReferenceSource::Stored {
        argument_index: 1,
        path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(9))],
    };
    for changed in variants {
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    }
}

#[test]
fn reference_forwarding_roster_refuses_duplicate_output_keys_and_source_sets() {
    let original = module();
    let mut changed = original.clone();
    let mut second = changed.boundary_reference_forwarding[0].clone();
    second.output = 1;
    changed.boundary_reference_forwarding.push(second);
    assert!(encode_module(&changed).is_ok());
    changed.boundary_reference_forwarding.reverse();
    let demands_error = Err(CodecError::NonCanonicalOrder(
        "boundary reference forwarding demands",
    ));
    assert_eq!(encode_module(&changed), demands_error);
    let mut changed = original.clone();
    let mut duplicate = changed.boundary_reference_forwarding[0].clone();
    duplicate.sources.pop();
    changed.boundary_reference_forwarding.push(duplicate);
    assert_eq!(encode_module(&changed), demands_error);

    let mut changed = original.clone();
    changed.boundary_reference_forwarding[0].sources.reverse();
    let sources_error = Err(CodecError::NonCanonicalOrder(
        "boundary reference forwarding sources",
    ));
    assert_eq!(encode_module(&changed), sources_error);
    let mut changed = original;
    let duplicate = changed.boundary_reference_forwarding[0].sources[0].clone();
    changed.boundary_reference_forwarding[0].sources = vec![duplicate.clone(), duplicate];
    assert_eq!(encode_module(&changed), sources_error);
}

#[test]
#[ignore = "explicit qualification supplies OMEGA_REFERENCE_STORE_CANONICAL_ARTIFACT"]
fn qualified_reference_store_artifact_is_accepted() {
    let path = std::env::var_os("OMEGA_REFERENCE_STORE_CANONICAL_ARTIFACT")
        .expect("supply a saved program from the qualified compiler invocation");
    let bytes = std::fs::read(path).expect("read the unchanged saved program");
    crate::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the saved program must pass ordinary canonical admission");
}

fn raw_saved_module(bytes: &[u8]) -> TerminalModule {
    let mut cursor = crate::cursor::Cursor::new(bytes);
    assert_eq!(cursor.take(8).unwrap(), b"PSIART\0\0");
    assert_eq!(cursor.u16().unwrap(), 3);
    let semantic_len = usize::try_from(cursor.u64().unwrap()).unwrap();
    cursor.u64().unwrap();
    match cursor.byte().unwrap() {
        0 => {}
        1 => {
            cursor.u64().unwrap();
        }
        _ => panic!("invalid debug section"),
    }
    let mut reader = super::wire::Reader::new(cursor.take(semantic_len).unwrap());
    assert_eq!(reader.take(super::MAGIC.len()).unwrap(), super::MAGIC);
    assert_eq!(reader.u16().unwrap(), super::FORMAT_MARKER);
    let module = super::module_wire::decode_module_body(&mut reader).unwrap();
    assert_eq!(reader.remaining(), 0);
    module
}

#[test]
#[ignore = "explicit qualification supplies OMEGA_FORMATION_FORWARDING_CANONICAL_ARTIFACT"]
fn qualified_saved_pointer_use_retains_its_forwarding_condition() {
    let path = std::env::var_os("OMEGA_FORMATION_FORWARDING_CANONICAL_ARTIFACT")
        .expect("supply the saved program that omitted the later pointer-use condition");
    let bytes = std::fs::read(path).expect("read the unchanged qualification artifact");
    let Err(crate::CanonicalTerminalArtifactError::Semantic(error)) =
        crate::CanonicalTerminalArtifact::from_bytes(&bytes)
    else {
        panic!("the old bytes must still refuse their missing pointer-use condition");
    };
    assert_eq!(error, CodecError::MalformedOperationFoundation {
        machine: "nominal(package-owner(32:163048cf22b555a93ce9a828d4cb2c784c0404aa3f07c1e0416853da6fce0136),path(main))".to_owned(),
        state: "nominal(package-owner(32:163048cf22b555a93ce9a828d4cb2c784c0404aa3f07c1e0416853da6fce0136),path(main::entry))".to_owned(),
        block: semantic_vocabulary::BlockId::new(6).unwrap(),
        operation: OperationId::new(16).unwrap(),
        reason: "reference establishment cannot retain source membership obligations",
    });
    // This is a producer counterfactual, not admission that repairs old bytes.
    // Change only the conditional request roster; every body remains identical.
    let mut module = raw_saved_module(&bytes);
    let old = module.boundary_reference_forwarding.clone();
    let mut stable = false;
    for _ in 0..=module
        .machines
        .iter()
        .map(|body| body.operations().count())
        .sum::<usize>()
    {
        let required =
            terminal_semantics::descriptor_results::formation_reference_forwarding_requirements(
                &module,
            )
            .expect("discover actual saved-pointer use dependencies");
        if required == module.boundary_reference_forwarding {
            stable = true;
            break;
        }
        module.boundary_reference_forwarding = required;
    }
    assert!(
        stable,
        "the finite use dependencies must reach a fixed point"
    );
    assert!(
        module.boundary_reference_forwarding.len() > old.len(),
        "the missing request must actually be retained"
    );
    eprintln!(
        "FORMATION_FORWARDING_RETAINED {:?}",
        module.boundary_reference_forwarding
    );
    let wire =
        super::module_wire::encode_raw(&module).expect("encode only the completed request roster");
    crate::decode_module(&wire)
        .expect("the receiving checker must validate the completed program independently");
}

#[test]
#[ignore = "explicit qualification supplies OMEGA_QUALIFIED_REFERENCE_FORWARDING_CANONICAL_ARTIFACT"]
fn qualified_reference_forwarding_rejects_omitted_original_value_guarantee() {
    let path = std::env::var_os("OMEGA_QUALIFIED_REFERENCE_FORWARDING_CANONICAL_ARTIFACT")
        .expect("supply the unchanged original program's qualified canonical output");
    let bytes = std::fs::read(path).expect("read the qualified original saved program");
    let artifact = crate::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the intact original must first pass ordinary canonical admission");
    assert!(
        !artifact
            .semantic_module()
            .boundary_reference_forwarding
            .is_empty(),
        "the original must carry the forwarding premise being removed",
    );
    let mut omitted = artifact.semantic_module().clone();
    omitted.boundary_reference_forwarding.clear();
    let wire =
        super::module_wire::encode_raw(&omitted).expect("raw-encode the forwarding-only omission");
    let error = crate::decode_module(&wire)
        .expect_err("removing required forwarding must refuse the old-value guarantee");
    eprintln!("REFERENCE_FORWARDING_OMISSION {error:?}");
    assert_eq!(error, expected_original_value_membership_rejection());
}

fn expected_original_value_membership_rejection() -> CodecError {
    CodecError::MalformedOperationFoundation {
        machine: "nominal(package-owner(32:2af6aadc16d305d166d93be065da8c8a6ed557a7ff5ec47c5b397c3309ff07aa),path(exercise))".to_owned(),
        state: "nominal(package-owner(32:2af6aadc16d305d166d93be065da8c8a6ed557a7ff5ec47c5b397c3309ff07aa),path(exercise::entry))".to_owned(),
        block: semantic_vocabulary::BlockId::new(88).unwrap(),
        operation: semantic_vocabulary::OperationId::new(124).unwrap(),
        reason: "reference establishment cannot retain source membership obligations",
    }
}

#[test]
#[ignore = "explicit qualification supplies OMEGA_INSTALLED_EMPTY_FORWARDING_CANONICAL_ARTIFACT"]
fn qualified_installed_empty_rejects_missing_forwarding_condition() {
    let path = std::env::var_os("OMEGA_INSTALLED_EMPTY_FORWARDING_CANONICAL_ARTIFACT")
        .expect("supply the fresh qualified installed-empty program");
    let bytes = std::fs::read(path).expect("read the unchanged fresh program");
    let artifact = crate::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the intact fresh program must first pass public canonical admission");
    let mut omitted = artifact.semantic_module().clone();
    assert!(
        !omitted.boundary_reference_forwarding.is_empty(),
        "the fresh producer must retain the condition this use needs",
    );
    // Preserve all bodies, declarations and other requests. This negative
    // component removes only the forwarding roster from the admitted module.
    omitted.boundary_reference_forwarding.clear();
    let wire = super::module_wire::encode_raw(&omitted)
        .expect("raw-encode only the request-roster omission");
    let error = crate::decode_module(&wire)
        .expect_err("the omitted condition must fail reference formation");
    eprintln!("INSTALLED_EMPTY_FORWARDING_OMISSION {error:?}");
    assert!(matches!(
        error,
        CodecError::MalformedOperationFoundation {
            reason: "reference establishment cannot retain source membership obligations",
            ..
        }
    ));
}
