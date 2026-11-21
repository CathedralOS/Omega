use super::super::{cohort_mechanism_row, terminal_authority_policy_with_rows};
use super::time_host_permission_rows;
use crate::terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows;
use provider_plans::{BoundaryCallingPlanCommitment, ServiceMethod, ServiceSchema};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use terminal_authority_receipts::NormalizedForeignTerminalMechanismIdentity;

fn time_service(package: u8) -> ServiceSchema {
    let mut schema = ServiceSchema::default();
    let service = Arc::make_mut(&mut schema.declaration);
    let package = PackageKeyIdentity::from_digest([package; 32]).unwrap();
    service.trait_package_identity = Some(package);
    service.trait_name = service.identities.intern("TimeHost");
    service.methods.push(ServiceMethod {
        name: "sleep".to_owned(),
        requirement_owner: "TimeHost".to_owned(),
        requirement_owner_package_identity: Some(package),
        requirement_identity: service.identities.intern("TimeHost::sleep"),
        ..Default::default()
    });
    schema
}

#[test]
fn the_time_permission_names_its_requirement_and_owning_package() {
    let service = time_service(1);
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let permissions = time_host_permission_rows(&service, &mut identities).unwrap();
    let policy =
        terminal_authority_permission_policy_with_rows(permissions, identities.build()).unwrap();
    let owner = service.identity_digest();
    assert!(policy.permission_for(owner, "TimeHost::sleep").is_ok());
    assert!(
        policy
            .permission_for(owner, "TimeHost::wall_clock_raw")
            .is_err()
    );
    let other_owner = time_service(2).identity_digest();
    assert!(
        policy
            .permission_for(other_owner, "TimeHost::sleep")
            .is_err()
    );
}

#[test]
fn the_sleep_requirement_cannot_classify_an_unreviewed_import() {
    let service = time_service(1);
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"/fixture/unreviewed-clock.dylib".to_vec(),
            symbol: b"_fixture_sleep".to_vec(),
        },
        target::TargetProfile::MacosArm64,
    )
    .unwrap();
    let import = NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        BoundaryCallingPlanCommitment::from_digest([3; 32]),
    );
    let method = &service.declaration.methods[0];
    let rows = cohort_mechanism_row(import.into(), &service, method);
    let policy = terminal_authority_policy_with_rows(rows.into_iter().collect()).unwrap();
    assert_eq!(
        policy.classify(import).unwrap_err().mechanism(),
        import.into()
    );
}
