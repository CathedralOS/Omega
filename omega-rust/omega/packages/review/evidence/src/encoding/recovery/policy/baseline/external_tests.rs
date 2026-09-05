use super::tests::{fixture, recover, rejects};
use super::*;

fn external() -> PackagePolicyBaseline {
    let mut value = fixture();
    value.slack_uses.clear();
    value.dangerous_capabilities.clear();
    let callable = &mut value.callables.callables[0];
    callable.role = PackagePolicyCallableRole::PrivateExternal;
    callable.supply = PackageReviewCallableSupply::ExternalRealization;
    callable.declared_service_reach = Some(vec![]);
    callable.checked_service_reach = PackageReviewCheckedServiceReach::NoCheckedBody;
    callable.checked_termination = PackagePolicyTermination::NoGuarantee;
    callable.checked_crash.structural_runtime_requirements = None;
    callable.capability_flows.clear();
    callable.reachable_capability_flows.clear();
    callable.mutation.paths.clear();
    value
        .external_supplies
        .push(PackagePolicyExternalExecutableSupply {
            callable: callable.identity.clone(),
            signature: PackagePolicyExternalCallableSignature {
                lifetime_parameter_count: callable.lifetime_parameter_count,
                static_parameters: callable.type_parameters.clone(),
                conformance_bounds: callable.conformance_bounds.clone(),
                parameters: callable
                    .parameters
                    .iter()
                    .map(|formal| PackageReviewExternalCallableParameter {
                        type_identity: formal.type_identity.clone(),
                        is_const: formal.is_const,
                        is_mutable: formal.is_mutable,
                        is_self: formal.is_self,
                    })
                    .collect(),
                return_type: callable.return_type.clone(),
            },
            requirement: PackagePolicyExternalRequirement::Trait(callable.conformances[0].clone()),
            binding: PackagePolicyExternalBinding::CompilerIntrinsic,
        });
    value
}

#[test]
fn external_rows_rejoin_public_and_private_surfaces_without_conflicting_bindings() {
    let original = external();
    assert_eq!(
        recover(&original.canonical_bytes().unwrap()).unwrap(),
        original
    );
    let mut value = original.clone();
    value.callables.callables[0].role = PackagePolicyCallableRole::Public;
    assert_eq!(recover(&value.canonical_bytes().unwrap()).unwrap(), value);
    value = original.clone();
    let mut conflict = value.external_supplies[0].clone();
    conflict.binding = PackagePolicyExternalBinding::Syscall { number: 1 };
    value.external_supplies.push(conflict);
    value.external_supplies.sort();
    rejects(&value);
    value = original.clone();
    value.external_supplies.clear();
    rejects(&value);
    value = original.clone();
    value.callables.callables.clear();
    value.semantic_dependencies.clear();
    rejects(&value);
    value = original;
    value.callables.callables[0].role = PackagePolicyCallableRole::Build;
    rejects(&value);
}

#[test]
fn external_full_signature_and_requirement_cannot_drift_from_callable() {
    let original = external();
    for field in 0..7 {
        let mut value = original.clone();
        let supply = &mut value.external_supplies[0];
        match field {
            0 => supply.signature.lifetime_parameter_count += 1,
            1 => supply.signature.return_type = None,
            2 => supply.signature.parameters[0].is_mutable = true,
            3 => supply.signature.parameters[0].type_identity.canonical = "i64".into(),
            4 => supply
                .signature
                .static_parameters
                .push(PackagePolicyTypeParameter {
                    kind: PackagePolicyTypeParameterKind::Type,
                    bounds: PackageReviewDataProperties {
                        multiplicity: language_semantics::Multiplicity::Unrestricted,
                        carry: None,
                    },
                }),
            5 => {
                let PackagePolicyExternalRequirement::Trait(requirement) = &mut supply.requirement
                else {
                    unreachable!()
                };
                requirement.alias = Some("different".into());
            }
            _ => value.callables.callables[0].conformances.clear(),
        }
        rejects(&value);
    }
}
