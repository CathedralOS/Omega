//! Descriptive ABI completion cannot donate execution or Product authority.

use super::*;
use semantic_vocabulary::{CallingAbiIdentity, StructuralTypeId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    CallingAbiDeclaration, CallingAbiStorage, CallingSignatureTelescope, TerminalModule,
};

fn fixture(
    with_extents: bool,
    false_helper: bool,
) -> (CanonicalTerminalArtifact, Vec<CallingAbiIdentity>) {
    let mut module = TerminalModule::declarations();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let integer = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );
    let mut keys = Vec::new();
    for receiver in ["Receiver<u8>", "Receiver<u16>"] {
        let key = CallingAbiIdentity {
            requirement_identity: "Receiver::inspect".into(),
            declaring_application: receiver.into(),
            operands: Vec::new(),
        };
        module.calling_abi_signatures.push(CallingAbiDeclaration {
            id: key.id(),
            requirement_identity: identities.intern(&key.requirement_identity),
            declaring_application: identities.intern(&key.declaring_application),
            operands: Vec::new(),
            formals: Vec::new(),
            default_eligible: true,
            storage: Ok(CallingAbiStorage {
                parameters: Vec::new(),
                result: with_extents.then(|| StructuralTypeId::new(3).unwrap()),
                telescope: CallingSignatureTelescope {
                    parameters: Vec::new(),
                    binders: Vec::new(),
                    direct_callbacks: Vec::new(),
                },
            }),
        });
        keys.push(key);
    }
    if with_extents {
        for (ordinal, shape) in [
            terminal_psi::StructuralTypeShape::PrimitiveScalar(integer),
            terminal_psi::StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: terminal_psi::ArrayExtent::Application(
                    ScalarApplicationId::new(1).unwrap(),
                ),
            },
            terminal_psi::StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(2).unwrap(),
                length: terminal_psi::ArrayExtent::Application(
                    ScalarApplicationId::new(2).unwrap(),
                ),
            },
        ]
        .into_iter()
        .enumerate()
        {
            module
                .structural_types
                .push(terminal_psi::StructuralTypeDeclaration {
                    id: StructuralTypeId::new(ordinal as u64 + 1).unwrap(),
                    identity: identities.intern(&format!("storage-{ordinal}")),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    copyable: true,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                    scalar_restriction: None,
                    shape,
                });
        }
        for measurement in [LayoutMeasurement::Size, LayoutMeasurement::Alignment] {
            module
                .scalar_applications
                .insert(ScalarApplication::LayoutObservation {
                    subject: StructuralTypeId::new(1).unwrap(),
                    measurement,
                    semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                    scalar_type: integer,
                })
                .unwrap();
        }
    }
    if false_helper {
        let helper = MachineId::new(1).unwrap();
        let declaration = terminal_psi::ProofDeclarationId(1);
        let identity = identities.intern("extent-helper");
        module.scalar_applications.proof_scope.declarations.insert(
            terminal_psi::ProofScopeDeclaration::Callable(
                terminal_psi::CheckedMachineDeclaration {
                    machine: terminal_psi::NominalDeclarationIdentity::new(identity),
                    state: terminal_psi::NominalDeclarationIdentity::new(identity),
                    semantic_declaration: identity,
                    parameter_types: Vec::new(),
                    callable: identity,
                    result_type: terminal_psi::TerminalIdentityIndex::ABSENT,
                    contract: [1; 32],
                    specialization: None,
                },
            ),
        );
        module.scalar_applications.proof_scope.callables.insert(
            terminal_psi::ProofCallableAssociation::Callable {
                declaration,
                callable: terminal_psi::TargetMachineCallable::Machine(helper),
            },
        );
        module.scalar_applications.proof_scope.plan.machines.insert(
            semantic_vocabulary::proof_applications::ProofApplicationMachine {
                machine: declaration,
                product_selected: true,
                ..Default::default()
            },
        );
        let mut nodes = Arena::new();
        let root = nodes.insert(terminal_psi::ScalarPredicateNode(
            numerics::scalar_predicates::PredicateNode::Value(ScalarValue::Boolean(false)),
        ));
        module
            .scalar_applications
            .conditions
            .retain_predicate(
                terminal_psi::ScalarConditionSubject::Declaration(declaration),
                "actual ABI extent helper debt".into(),
                nodes,
                root,
            )
            .unwrap();
    }
    module.identities = identities.build();
    module.calling_abi_signatures.sort_by_key(|row| row.id);
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    (
        CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap(),
        keys,
    )
}

fn facts(target: &str) -> crate::LayoutTargetFacts {
    crate::LayoutTargetFacts {
        profile: target.into(),
        pointer_size: 8,
        pointer_alignment: 8,
        little_endian: true,
    }
}

#[test]
fn abi_storage_request_requires_exact_canonical_roster() {
    let (artifact, keys) = fixture(true, false);
    let demand = CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
    let expected = [
        ScalarApplicationId::new(1).unwrap(),
        ScalarApplicationId::new(2).unwrap(),
    ];
    assert_eq!(demand.applications(), expected);
    let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
    let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let observations = ScalarApplicationObservations::new(
        artifact.view(),
        &targets,
        &providers,
        &facts("macos_arm64"),
    )
    .unwrap()
    .with_abi_storage(&demand)
    .unwrap();
    assert!(observations.conditions("macos_arm64", &expected).is_ok());
    for changed in [
        vec![],
        vec![expected[0]],
        vec![expected[1], expected[0]],
        vec![expected[0], expected[1], expected[0]],
    ] {
        let error = observations
            .conditions("macos_arm64", &changed)
            .err()
            .unwrap();
        assert!(error.contains("canonical extent roster"), "{error}");
    }
}

#[test]
fn abi_storage_receipt_binds_full_key_and_grants_no_entry_or_product_authority() {
    let (artifact, keys) = fixture(false, false);
    let demands = keys
        .iter()
        .map(|key| CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", key).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(demands[0].applications(), demands[1].applications());
    let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
    let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let observations = || {
        ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &facts("macos_arm64"),
        )
        .unwrap()
        .with_abi_storage(&demands[0])
        .unwrap()
    };
    let observed = observations();
    let conditions = observed.conditions("macos_arm64", &[]).unwrap();
    let mut wrong =
        ScalarApplicationClosure::for_abi_storage(artifact.view(), &demands[1]).unwrap();
    let error = wrong
        .close_scope(observed, conditions, Ok(None))
        .unwrap_err();
    assert!(error.contains("roots differ"), "{error}");
    let observed = observations();
    let conditions = observed.conditions("macos_arm64", &[]).unwrap();
    let mut closure =
        ScalarApplicationClosure::for_abi_storage(artifact.view(), &demands[0]).unwrap();
    closure.close_scope(observed, conditions, Ok(None)).unwrap();
    let completed = closure.finish().unwrap();
    assert!(completed.require_product_formation().is_err());
    assert!(
        !completed
            .primary()
            .admits_machine(artifact.semantic_module(), MachineId::new(1).unwrap())
    );
}

#[test]
fn abi_storage_request_refuses_other_artifact_and_target() {
    let (artifact, keys) = fixture(false, false);
    let (other, _) = fixture(true, false);
    let demand = CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
    assert!(
        ScalarApplicationClosure::for_abi_storage(other.view(), &demand)
            .err()
            .unwrap()
            .contains("another Terminal artifact")
    );
    let targets = crate::admit_target_machine_selections(artifact.view(), "linux_arm64");
    let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let error = ScalarApplicationObservations::new(
        artifact.view(),
        &targets,
        &providers,
        &facts("linux_arm64"),
    )
    .unwrap()
    .with_abi_storage(&demand)
    .err()
    .unwrap();
    assert!(error.contains("another selected target"), "{error}");
}

#[test]
fn abi_storage_completion_refuses_actual_false_helper_debt() {
    let (artifact, keys) = fixture(false, true);
    let demand = CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
    let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
    let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
    let observations = ScalarApplicationObservations::new(
        artifact.view(),
        &targets,
        &providers,
        &facts("macos_arm64"),
    )
    .unwrap()
    .with_abi_storage(&demand)
    .unwrap();
    let mut conditions = observations.conditions("macos_arm64", &[]).unwrap();
    conditions.discharge_ready(&observations).unwrap();
    conditions
        .include_evaluation_entry(MachineId::new(1).unwrap())
        .unwrap();
    let error = conditions.discharge_ready(&observations).unwrap_err();
    assert!(error.contains("actual ABI extent helper debt"), "{error}");
    let mut closure = ScalarApplicationClosure::for_abi_storage(artifact.view(), &demand).unwrap();
    let error = closure
        .close_scope(observations, conditions, Ok(None))
        .unwrap_err();
    assert!(error.contains("actual ABI extent helper debt"), "{error}");
}
