// Field note (review 547b665d4b11..bc6c5788e062): this glob is the one file
// over the extents ceiling (0) in tests/architecture/glob_self_imports.rs —
// the gate is red on main since d5b124cd40; import the used names instead.
use super::*;
use crate::identities::AddressSpaceId;
use crate::mapping::MappingId;

fn authority() -> OrderingAuthority {
    OrderingAuthority {
        address_space: AddressSpaceId::from_normalized_identity(1).unwrap(),
        provenance: ExtentProvenanceId::from_normalized_identity(2).unwrap(),
        era: MappingEraId::from_normalized_identity(3).unwrap(),
        lineage: ExtentLineageId::from_normalized_identity(4).unwrap(),
    }
}

fn event(role: DeviceOrderingRole, identity: u64) -> OrderingEvent {
    OrderingEvent::from_admitted_provider(
        OrderingEventId::from_normalized_identity(identity).unwrap(),
        role,
        DeviceInstanceId::from_normalized_identity(10).unwrap(),
        RuntimeScopeId::from_normalized_identity(20).unwrap(),
        ExternalLoanId::from_normalized_identity(30).unwrap(),
        authority(),
        [(
            OrderingCoordinateKind::Data,
            OrderingRange {
                mapping: MappingId::from_normalized_identity(40).unwrap(),
                base: 0x1000,
                length: 0x200,
            },
        )],
    )
    .unwrap()
}

fn requirement(roles: &[DeviceOrderingRole]) -> OrderingCoverageRequirement {
    OrderingCoverageRequirement {
        device: DeviceInstanceId::from_normalized_identity(10).unwrap(),
        scope: RuntimeScopeId::from_normalized_identity(20).unwrap(),
        request: ExternalLoanId::from_normalized_identity(30).unwrap(),
        authority: authority(),
        roles: roles.iter().copied().collect(),
    }
}

#[test]
fn role_discriminant_participates_in_identity() {
    let publication = event(DeviceOrderingRole::Publication, 100);
    let mut notification = event(DeviceOrderingRole::Notification, 100);
    notification.identity = publication.identity;
    assert!(publication.same_payload_different_role(&notification));
    assert_ne!(publication, notification);
}

#[test]
fn complete_roster_admits() {
    let roles = [
        DeviceOrderingRole::Publication,
        DeviceOrderingRole::Notification,
        DeviceOrderingRole::Completion,
        DeviceOrderingRole::Acquisition,
    ];
    let events: Vec<_> = roles
        .iter()
        .enumerate()
        .map(|(index, role)| event(*role, index as u64 + 1))
        .collect();
    let admission = admit_role_coverage(&requirement(&roles), events).unwrap();
    assert_eq!(admission.events().len(), 4);
}

#[test]
fn missing_role_rejects_and_returns_events() {
    let roles = [
        DeviceOrderingRole::Publication,
        DeviceOrderingRole::Completion,
    ];
    let events = vec![event(DeviceOrderingRole::Publication, 1)];
    let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
    assert!(rejection.diagnostic().to_string().contains("missing"));
    assert_eq!(rejection.into_events().len(), 1);
}

#[test]
fn extra_role_rejects_and_returns_events() {
    let roles = [DeviceOrderingRole::Publication];
    let events = vec![
        event(DeviceOrderingRole::Publication, 1),
        event(DeviceOrderingRole::Completion, 2),
    ];
    let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
    assert!(rejection.diagnostic().to_string().contains("extra"));
    assert_eq!(rejection.into_events().len(), 2);
}

#[test]
fn duplicate_role_rejects() {
    let roles = [DeviceOrderingRole::Publication];
    let events = vec![
        event(DeviceOrderingRole::Publication, 1),
        event(DeviceOrderingRole::Publication, 2),
    ];
    let rejection = admit_role_coverage(&requirement(&roles), events).unwrap_err();
    assert!(rejection.diagnostic().to_string().contains("duplicates"));
}

#[test]
fn drifted_scope_rejects() {
    let roles = [DeviceOrderingRole::Publication];
    let mut drifted = event(DeviceOrderingRole::Publication, 1);
    drifted.scope = RuntimeScopeId::from_normalized_identity(99).unwrap();
    let rejection = admit_role_coverage(&requirement(&roles), vec![drifted]).unwrap_err();
    assert!(rejection.diagnostic().to_string().contains("scope"));
}

#[test]
fn acquisition_requires_completion_on_same_request() {
    let completion = event(DeviceOrderingRole::Completion, 1);
    let acquisition = event(DeviceOrderingRole::Acquisition, 2);
    admit_acquisition_after_completion(&acquisition, &completion).unwrap();

    let mut wrong_scope = event(DeviceOrderingRole::Acquisition, 3);
    wrong_scope.scope = RuntimeScopeId::from_normalized_identity(77).unwrap();
    let error = admit_acquisition_after_completion(&wrong_scope, &completion).unwrap_err();
    assert!(error.to_string().contains("scope"));

    let notification = event(DeviceOrderingRole::Notification, 4);
    let error = admit_acquisition_after_completion(&acquisition, &notification).unwrap_err();
    assert!(error.to_string().contains("completion"));
}

#[test]
fn duplicate_coordinate_kind_rejects() {
    let error = OrderingEvent::from_admitted_provider(
        OrderingEventId::from_normalized_identity(1).unwrap(),
        DeviceOrderingRole::Publication,
        DeviceInstanceId::from_normalized_identity(10).unwrap(),
        RuntimeScopeId::from_normalized_identity(20).unwrap(),
        ExternalLoanId::from_normalized_identity(30).unwrap(),
        authority(),
        [
            (
                OrderingCoordinateKind::Data,
                OrderingRange {
                    mapping: MappingId::from_normalized_identity(40).unwrap(),
                    base: 0x1000,
                    length: 0x200,
                },
            ),
            (
                OrderingCoordinateKind::Data,
                OrderingRange {
                    mapping: MappingId::from_normalized_identity(40).unwrap(),
                    base: 0x3000,
                    length: 0x100,
                },
            ),
        ],
    )
    .unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn empty_coordinates_reject() {
    let error = OrderingEvent::from_admitted_provider(
        OrderingEventId::from_normalized_identity(1).unwrap(),
        DeviceOrderingRole::Publication,
        DeviceInstanceId::from_normalized_identity(10).unwrap(),
        RuntimeScopeId::from_normalized_identity(20).unwrap(),
        ExternalLoanId::from_normalized_identity(30).unwrap(),
        authority(),
        [],
    )
    .unwrap_err();
    assert!(error.to_string().contains("at least one coordinate"));
}
