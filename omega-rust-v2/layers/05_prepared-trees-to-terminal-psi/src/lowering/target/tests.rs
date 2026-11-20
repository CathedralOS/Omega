use super::fold_observation;
use omega_psi::{
    DeclarationKind, FloatFormat, FloatOperation, FloatRealization, FloatSemantics, FoldedSubject, FoldedTargetClosure, ReceivingTarget, Site, SourceSpan,
    TargetIdentity,
};
use omega_syntax::NodeKind;
use omega_target_contract::{AddressSpaceIdentity, RequirementSlot, SelectedRealization, SelectedTargetProperty, TargetObservation, TargetProperty};

fn declaration<K: Ord>(
    kind: K,
    path: &str,
) -> omega_program_contracts::DeclarationId<K> {
    omega_program_contracts::DeclarationId {
        kind,
        name: std::sync::Arc::new(omega_program_contracts::DeclarationName {
            package: "provider".into(),
            path: path.into(),
            operands: vec!["u8".into()],
            result_domains: vec!["Tag".into()],
            signature_owners: vec![("provider".into(), "Tag".into())],
            resolved_signature: None,
            enclosing_machine: None,
        }),
    }
}
#[test]
fn target_replay_distinguishes_selected_float_operation_provider_contract_and_property_version() {
    let identity = TargetIdentity {
        profile: "target".into(),
        semantics_version: 2,
        realization: vec![9].into(),
    };
    let site = Site {
        package: "provider".into(),
        file: "impl.omg".into(),
        span: SourceSpan::new(12, 16),
    };
    let float = FloatRealization {
        format: FloatFormat::Binary32,
        operation: FloatOperation::Add,
        semantics: FloatSemantics::Ieee754NearestEvenV1,
    };
    let property = |projection_version| SelectedTargetProperty {
        projection_version,
        value: TargetProperty::ExclusiveAddressBound {
            address_space: AddressSpaceIdentity(vec![1].into()),
            exclusive_bound: 256u32.into(),
        },
    };
    let source_selection = SelectedRealization {
        requirement: RequirementSlot {
            declaration: declaration(NodeKind::Machine, "Ops::run"),
            trait_arguments: vec![omega_typed::ValueType::Bool],
        },
        arguments: vec![],
        implementation: declaration(NodeKind::Machine, "run"),
        conformance: Some(declaration(NodeKind::Conformance, "Provider")),
        binding: site.clone(),
        checked_contract: vec![3].into(),
        selected_plan: vec![4].into(),
    };
    let saved = [
        TargetObservation::FloatRealization {
            target: identity.clone(),
            realization: float,
        },
        TargetObservation::SelectedRealization {
            target: identity.clone(),
            realization: source_selection,
        },
        TargetObservation::Property {
            target: identity.clone(),
            property: property(5),
        },
    ];
    let closure = FoldedTargetClosure::new(
        identity.clone(),
        saved
            .iter()
            .map(|o| fold_observation(o, &identity).unwrap())
            .collect(),
        vec![],
    );
    let receiver = |operation, contract, version| {
        ReceivingTarget::new(identity.clone(), [], [FloatRealization { operation, ..float }])
            .unwrap()
            .with_selected_realizations([SelectedRealization {
                requirement: RequirementSlot {
                    declaration: declaration(DeclarationKind::Machine, "Ops::run"),
                    trait_arguments: vec![FoldedSubject::Bool],
                },
                arguments: vec![],
                implementation: declaration(DeclarationKind::Machine, "run"),
                conformance: Some(declaration(DeclarationKind::Conformance, "Provider")),
                binding: site.clone(),
                checked_contract: vec![contract].into(),
                selected_plan: vec![4].into(),
            }])
            .unwrap()
            .with_target_properties([SelectedTargetProperty {
                projection_version: version,
                value: TargetProperty::ExclusiveAddressBound {
                    address_space: AddressSpaceIdentity(vec![1].into()),
                    exclusive_bound: 256u32.into(),
                },
            }])
            .unwrap()
    };
    assert_eq!(closure.replay(&receiver(FloatOperation::Add, 3, 5)), Ok(()));
    for (operation, contract, version, missing) in [
        (FloatOperation::Subtract, 3, 5, 0),
        (FloatOperation::Add, 7, 5, 1),
        (FloatOperation::Add, 3, 6, 2),
    ] {
        assert_eq!(closure.replay(&receiver(operation, contract, version)), Err(&closure.observations()[missing]));
    }
}
