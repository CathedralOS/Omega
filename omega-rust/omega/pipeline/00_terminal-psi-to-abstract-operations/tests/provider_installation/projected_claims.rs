use super::builders::{artifact, selected};
use super::ids::{boundary_id, claim_id, operation_id, place_id, structural_type_id};
use super::structural_provider::structural_provider_module;
use proof_admission::AdmissionProfile;
use terminal_psi::{
    CompletionReceipt, EntryClaim, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalModule,
};
use terminal_psi_to_abstract_operations::{admit_provider_installation, lower_artifact};

#[test]
fn omega_rebases_projected_provider_claims_and_preserves_sibling_sources() {
    let module = projected_structural_provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .map(|admitted| admitted.into_plan())
    .expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("projected structural provider installation");
    let [first, second] = installation.installed_calls() else {
        panic!("two installed projected provider calls")
    };
    assert_eq!(
        first.structural_arguments()[0].path,
        [StructuralPathSegment::FixedIndex(0)]
    );
    assert_eq!(
        second.structural_arguments()[0].path,
        [StructuralPathSegment::FixedIndex(1)]
    );
    assert_eq!(
        first.completion_receipts(),
        &[CompletionReceipt {
            claim: claim_id(1),
            argument_index: 0,
        }]
    );
    assert_eq!(
        second.completion_receipts(),
        &[CompletionReceipt {
            claim: claim_id(2),
            argument_index: 0,
        }]
    );
    assert_eq!(first.completion_claim_sources().len(), 2);
    assert_eq!(second.completion_claim_sources().len(), 2);
    assert_eq!(
        first.completion_claim_sources()[0]
            .entry
            .as_ref()
            .expect("first projected entry source")
            .path,
        [StructuralPathSegment::FixedIndex(0)]
    );
    assert_eq!(
        first.completion_claim_sources()[1]
            .entry
            .as_ref()
            .expect("sibling projected entry source")
            .path,
        [StructuralPathSegment::FixedIndex(1)]
    );
}

fn projected_structural_provider_module() -> TerminalModule {
    let mut module = structural_provider_module();
    let resource = structural_type_id(3);
    let resources = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: resources,
        identity: "[Resource; 2]".into(),
        shape: StructuralTypeShape::FixedArray {
            element: resource,
            length: 2,
        },
    });

    let caller = &mut module.machines[0];
    caller.structural_parameters[0].structural_type = resources;
    caller.entry_claims = vec![
        EntryClaim {
            claim: claim_id(1),
            input: place_id(1),
            path: vec![StructuralPathSegment::FixedIndex(0)],
        },
        EntryClaim {
            claim: claim_id(2),
            input: place_id(1),
            path: vec![StructuralPathSegment::FixedIndex(1)],
        },
    ];
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
                boundary: boundary_id(1),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::FixedIndex(0)],
                    access: StructuralAccess::Owned,
                }],
                completion_receipts: vec![CompletionReceipt {
                    claim: claim_id(1),
                    argument_index: 0,
                }],
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
                boundary: boundary_id(1),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                    access: StructuralAccess::Owned,
                }],
                completion_receipts: vec![CompletionReceipt {
                    claim: claim_id(2),
                    argument_index: 0,
                }],
            },
        },
    ];
    module.machines[1].blocks[0].operations[0].id = operation_id(3);
    module.machines[2].blocks[0].operations[0].id = operation_id(4);
    module
}
