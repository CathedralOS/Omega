use super::*;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    CallingAbiDeclaration, CallingAbiStorage, CallingSignatureTelescope, LayoutMeasurement,
    ScalarApplication, TerminalModule,
};

fn fixture(with_extents: bool) -> (CanonicalTerminalArtifact, Vec<CallingAbiIdentity>) {
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
            physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::BoundaryTrait),
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
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    id: StructuralTypeId::new(ordinal as u64 + 1).unwrap(),
                    identity: identities.intern(&format!("storage-{ordinal}")),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
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
    module.identities = identities.build();
    module.calling_abi_signatures.sort_by_key(|row| row.id);
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    (
        CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap(),
        keys,
    )
}

#[test]
fn demand_retains_the_exact_declared_extent_roster_and_artifact() {
    let (artifact, keys) = fixture(true);
    let demand = CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
    assert_eq!(demand.declaration(), &keys[0]);
    assert_eq!(demand.target(), "macos_arm64");
    assert_eq!(
        demand.applications(),
        [1, 2].map(|value| ScalarApplicationId::new(value).unwrap())
    );
    assert!(demand.require_artifact(artifact.view()).is_ok());
    let (other, _) = fixture(false);
    assert!(demand.require_artifact(other.view()).is_err());
    let mut absent = keys[0].clone();
    absent.declaring_application = "Other".into();
    assert!(CallingAbiStorageDemand::new(artifact.view(), demand.target(), &absent).is_err());
}
