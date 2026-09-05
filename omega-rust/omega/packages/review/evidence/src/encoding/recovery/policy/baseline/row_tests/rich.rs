use super::*;

fn rich() -> PackagePolicyBaseline {
    let mut value = fixture();
    value.selected_providers = super::super::super::selected_providers::row_fixture();
    value.package = value.selected_providers.package;
    value.callables.package = value.package;
    value.callables.callables.clear();
    value.dangerous_capabilities.clear();
    value.slack_uses.clear();
    value.semantic_dependencies.clear();
    value.terminal_permissions = super::super::super::terminal_permissions::row_fixture();
    value.representation = super::super::super::representation::row_fixture();
    value.public_api = super::super::super::public_api::row_fixture();
    let owner = PackageReviewNominalOwner::Package(value.package);
    value.public_api.traits[0].identity.owner = owner;
    value.public_api.traits[0].requirements[0].identity.owner = owner;
    value.public_api.traits[0].requirements[0].parameters.push(
        PackageReviewTraitRequirementParameter {
            name: "input".into(),
            type_identity: PackageReviewTypeIdentity {
                canonical: "Unit".into(),
            },
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );
    value.public_api.conformances[0].identity.owner = owner;
    value.public_api.domains[0].identity.owner = owner;
    value.public_api.propositions[0].identity.owner = owner;
    value.public_api.consts[0].identity.owner = owner;
    value.public_api.operators[0].coordinate.identity.owner = owner;
    value.public_api.data[0].identity.owner = owner;
    let plan = &mut value.selected_providers.plans[0];
    let coordinate = PackageReviewOperatorCoordinate {
        identity: plan.schema_declaration.clone(),
        parameter_dispatch: "dispatch".into(),
        result_dispatch: "result".into(),
    };
    let requirement = coordinate.policy_requirement_identity();
    plan.methods[0].requirement = requirement.clone();
    let row = &mut plan.rows[0];
    row.requirement = requirement.clone();
    row.binding = PackagePolicyProviderBinding::CheckedAdapter {
        machine_identity: row.realization.path.clone(),
        machine_package_identity: Some(value.package),
    };
    value
        .boundary_applications
        .realizations
        .push(PackagePolicyBoundaryApplicationRealization {
            operator_coordinate: coordinate,
            requirement_identity: row.requirement.path.clone(),
            application: PackageReviewBoundaryApplication::Empty,
            selected_plan_index: 0,
            realization: PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration: row.realization.clone(),
                realization: row.realization.clone(),
            },
        });
    value.selected_providers.families[0].coordinates[0].requirement_identity = requirement.path;
    value
        .validate_canonical_structure()
        .expect("reused complete component fixtures join one valid baseline");
    value
}

#[test]
fn all_public_families_and_nonempty_provider_representation_permissions_remain_readable() {
    let value = rich();
    let original_bytes = value.canonical_bytes().unwrap();
    let original_text = value.canonical_text().unwrap();
    let projected = rows(&value).0;
    for kind in [
        PackagePolicyRowKind::PublicTrait,
        PackagePolicyRowKind::PublicConformance,
        PackagePolicyRowKind::PublicDomain,
        PackagePolicyRowKind::PublicProposition,
        PackagePolicyRowKind::PublicConst,
        PackagePolicyRowKind::PublicOperator,
        PackagePolicyRowKind::PublicData,
        PackagePolicyRowKind::RepresentationAvailability,
        PackagePolicyRowKind::RepresentationSelection,
        PackagePolicyRowKind::RepresentationDemand,
        PackagePolicyRowKind::TerminalService,
        PackagePolicyRowKind::TerminalPermission,
    ] {
        assert!(projected.iter().any(|row| row.kind() == kind), "{kind:?}");
    }
    let association = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::SelectedProviderAssociation)
        .unwrap();
    for field in [
        "field families",
        "field plans",
        "field closed_applications",
        "field selected_plan_index",
        "field grants",
    ] {
        assert!(association.canonical_text().contains(field), "{field}");
    }
    let trait_row = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicTrait)
        .unwrap();
    assert!(
        trait_row
            .canonical_text()
            .contains("field establishment_routes")
    );
    assert!(trait_row.canonical_text().contains("field published_crash"));
    assert_eq!(value.canonical_bytes().unwrap(), original_bytes);
    assert_eq!(value.canonical_text().unwrap(), original_text);
}

#[test]
fn each_public_family_and_atomic_index_association_mutation_changes_rows() {
    let original = rich();
    let baseline = rows(&original).0;
    for axis in 0..14 {
        let mut changed = original.clone();
        let kind = match axis {
            0 => {
                changed.public_api.traits[0].requirements[0].has_default_realization = false;
                PackagePolicyRowKind::PublicTrait
            }
            1 => {
                changed.public_api.conformances[0].interface.arguments[0].canonical = "u64".into();
                PackagePolicyRowKind::PublicConformance
            }
            2 => {
                changed.public_api.domains[0].predicate_facts.clear();
                PackagePolicyRowKind::PublicDomain
            }
            3 => {
                changed.public_api.propositions[0].parameter_types.push(
                    PackageReviewTypeIdentity {
                        canonical: "u64".into(),
                    },
                );
                PackagePolicyRowKind::PublicProposition
            }
            4 => {
                changed.public_api.consts[0].canonical_value_encoding = "other".into();
                PackagePolicyRowKind::PublicConst
            }
            5 => {
                changed.public_api.operators[0].return_type = Some(PackageReviewTypeIdentity {
                    canonical: "Unit".into(),
                });
                PackagePolicyRowKind::PublicOperator
            }
            6 => {
                changed.public_api.data[0].zero_gated = true;
                PackagePolicyRowKind::PublicData
            }
            7 => {
                changed.selected_providers.plans[0].grants.pop();
                PackagePolicyRowKind::SelectedProviderAssociation
            }
            8 => {
                changed.selected_providers.families.clear();
                PackagePolicyRowKind::SelectedProviderAssociation
            }
            9 => {
                let PackagePolicyBoundaryRealization::NongenericCheckedBody { realization, .. } =
                    &mut changed.boundary_applications.realizations[0].realization
                else {
                    unreachable!()
                };
                realization.path = "other_checked_body".into();
                PackagePolicyRowKind::SelectedProviderAssociation
            }
            10 => {
                changed.representation.demands[0]
                    .calling
                    .semantic_parameters[0]
                    .name = "renamed_formal".into();
                PackagePolicyRowKind::RepresentationDemand
            }
            11 => {
                changed.representation.producer_availability[0]
                    .conformance
                    .lifetime_parameter_count += 1;
                PackagePolicyRowKind::RepresentationAvailability
            }
            12 => {
                changed.terminal_permissions.services[0].methods[0].name =
                    "unpermitted_sibling_changed".into();
                PackagePolicyRowKind::TerminalService
            }
            _ => {
                changed.terminal_permissions.services[0].permissions[0].permitted =
                    omega_effects::TerminalAuthorityDisposition::from_classes([]);
                PackagePolicyRowKind::TerminalPermission
            }
        };
        // Mutations exercise serialization meaning, including independently
        // associated nested values; they are not claims of new checked custody.
        let candidate = rows(&changed).0;
        assert_ne!(baseline, candidate, "axis {axis}");
        let differences = baseline
            .iter()
            .zip(&candidate)
            .filter(|(left, right)| left != right)
            .collect::<Vec<_>>();
        assert_eq!(differences.len(), 1, "axis {axis}");
        assert_eq!(differences[0].0.kind(), kind, "axis {axis}");
        assert_eq!(
            differences[0].0.key_bytes(),
            differences[0].1.key_bytes(),
            "axis {axis}"
        );
    }
}
