use super::super::{
    darwin_clock_mechanism_row, reviewed_host_operation, terminal_authority_policy_with_rows,
};
use super::{filesystem_host_permission_rows, time_host_permission_rows};
use crate::terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows;
use build_providers::plans::{BoundaryCallingPlanCommitment, ServiceMethod, ServiceSchema};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use terminal_authority_receipts::NormalizedForeignTerminalMechanismIdentity;

fn service(package: u8, name: &str, operation: &str) -> ServiceSchema {
    let mut schema = ServiceSchema::default();
    let service = Arc::make_mut(&mut schema.declaration);
    let package = PackageKeyIdentity::from_digest([package; 32]).unwrap();
    service.trait_package_identity = Some(package);
    service.trait_name = service.identities.intern(name);
    service.methods.push(ServiceMethod {
        name: operation.to_owned(),
        requirement_owner: name.to_owned(),
        requirement_owner_package_identity: Some(package),
        requirement_identity: service.identities.intern(&format!("{name}::{operation}")),
        ..Default::default()
    });
    schema
}

#[test]
fn a_filesystem_permission_is_limited_to_the_reviewed_package_and_requirement() {
    let filesystem = service(1, "FilesystemHost", "write");
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let permissions = filesystem_host_permission_rows(&filesystem, &mut identities).unwrap();
    let policy =
        terminal_authority_permission_policy_with_rows(permissions, identities.build()).unwrap();
    let owner = filesystem.identity_digest();
    assert!(
        policy
            .permission_for(owner, "FilesystemHost::write")
            .is_ok()
    );
    assert!(
        policy
            .permission_for(owner, "FilesystemHost::read")
            .is_err()
    );
    let other_package = service(2, "FilesystemHost", "write").identity_digest();
    assert!(
        policy
            .permission_for(other_package, "FilesystemHost::write")
            .is_err()
    );
}

#[test]
fn the_time_permission_names_its_requirement_and_owning_package() {
    let time = service(1, "TimeHost", "sleep");
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let permissions = time_host_permission_rows(&time, &mut identities).unwrap();
    let policy =
        terminal_authority_permission_policy_with_rows(permissions, identities.build()).unwrap();
    let owner = time.identity_digest();
    assert!(policy.permission_for(owner, "TimeHost::sleep").is_ok());
    assert!(
        policy
            .permission_for(owner, "TimeHost::wall_clock_raw")
            .is_err()
    );
    let other_owner = service(2, "TimeHost", "sleep").identity_digest();
    assert!(
        policy
            .permission_for(other_owner, "TimeHost::sleep")
            .is_err()
    );
}

#[test]
fn the_sleep_requirement_cannot_classify_an_unreviewed_import() {
    let time = service(1, "TimeHost", "sleep");
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
    let method = &time.declaration.methods[0];
    let rows = darwin_clock_mechanism_row(import.into(), &time, method)
        .or_else(|| reviewed_host_operation(import.into()));
    let policy = terminal_authority_policy_with_rows(rows.into_iter().collect()).unwrap();
    assert_eq!(
        policy.classify(import).unwrap_err().mechanism(),
        import.into()
    );
}

#[test]
fn a_service_method_named_write_cannot_classify_an_unreviewed_import() {
    let filesystem = service(1, "FilesystemHost", "write");
    let method = &filesystem.declaration.methods[0];
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"/fixture/other.dylib".to_vec(),
            symbol: b"_fixture_counter".to_vec(),
        },
        target::TargetProfile::MacosArm64,
    )
    .unwrap();
    let import = NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        BoundaryCallingPlanCommitment::from_digest([3; 32]),
    );
    assert!(darwin_clock_mechanism_row(import.into(), &filesystem, method).is_none());
    assert!(reviewed_host_operation(import.into()).is_none());
}
