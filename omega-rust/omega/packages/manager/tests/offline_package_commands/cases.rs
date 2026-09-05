use super::fixture::*;
use package_manager::operations::{PackageCommand, PackageCommandKind, PackageCommandStatus};
use package_source::ImmutableSourceResolution;
use std::fs;

include!("resume.rs");
include!("selection.rs");

fn assert_git_pin(fixture: &Fixture, expected: &str) {
    let lock = fixture.lock();
    let mut pins = 0;
    for package in lock.targets()[0].source().packages() {
        if let ImmutableSourceResolution::Git { commit, .. } = package.resolution() {
            assert_eq!(commit.to_hex(), expected);
            pins += 1;
        }
    }
    assert!(pins > 0);
}
