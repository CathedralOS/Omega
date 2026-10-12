//! Source and selected-target provenance of the consuming runtime root.
//!
//! The completed Build already retained its actually executed root symbol and
//! canonical callable identity before Product pruning. Rejoin those records to
//! the immutable source seal instead of rediscovering a machine or selecting by
//! display name. A helper's registration site is not the authoritative root.
//! Package Product occurrences and original captured roots have distinct owners;
//! source custody never manufactures an anonymous package or dependency rights.
//!
//! This component identifies root-source scope, not an executable entry or call
//! occurrence. Application metadata contains neither. Complete dependency and
//! domain evidence, relationship consent and runtime installation remain separate.

use build_declarations::{BuildDeclarationKind, DependencyPurpose};
use package_compilation::{
    CheckedSourceCustody, ConsumedSourceUnitKind, ExecutedSelectedBuildRoot,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

use super::PreparedAssumedBindingRelationship;
use crate::digest_encoder::DomainDigestEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingRootOwner {
    Package(PackageKeyIdentity),
    CapturedRoot([u8; 32]),
}

/// Authentic root-source scope only. Missing original source, actual Build
/// identity or selected target stays unavailable, never an empty root grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingRootScope {
    owner: AssumedBindingRootOwner,
    build_source_revision: [u8; 32],
    build_execution_profile: Option<TargetProfile>,
    target: TargetProfile,
    digest: [u8; 32],
}

impl AssumedBindingRootScope {
    pub const fn owner(self) -> AssumedBindingRootOwner {
        self.owner
    }

    pub const fn build_source_revision(self) -> [u8; 32] {
        self.build_source_revision
    }

    pub const fn target(self) -> TargetProfile {
        self.target
    }

    pub const fn build_execution_profile(self) -> Option<TargetProfile> {
        self.build_execution_profile
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

/// Retain original root scope after ordinary source sealing. This consumes
/// existing identities; it does not perform another source or body check.
pub fn retain_assumed_binding_root_scopes(
    relationships: &mut [PreparedAssumedBindingRelationship],
    custody: &CheckedSourceCustody,
    root: Option<&ExecutedSelectedBuildRoot>,
    selected_target: Option<TargetProfile>,
) {
    for relationship in relationships {
        relationship.root_scope =
            root.and_then(|root| capture(relationship, custody, root, selected_target));
    }
}

fn capture(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
    root: &ExecutedSelectedBuildRoot,
    selected_target: Option<TargetProfile>,
) -> Option<AssumedBindingRootScope> {
    let application = root.application()?;
    let target = selected_target?;
    // Deployment identity is more precise than the native shape: Windows and
    // UEFI share a shape but cannot donate their root scope to each other.
    // This is the original selected plan, independently retained by preparation.
    if relationship.selected_supplier().plan.target != target.target_name()
        || target.native_realization()? != relationship.required_signature().native_target()
        || target.native_realization()? != relationship.supplied_signature().native_target()
    {
        return None;
    }
    let execution = root.execution();
    let selected = execution.symbol();
    let identity = execution.callable_identity();
    if identity.is_empty() || identity.contains('\0') {
        return None;
    }
    // The selected/executed role owns this exact original arena. Looking up
    // its handle in a later Product arena would substitute another declaration.
    let source = custody.declaration_source(&execution.owner().symbols, selected)?;
    let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-root-source-scope.v1\0");
    encoder.bytes(b"runtime-only");
    let owner = match (custody.package_subject(), source.unit().kind()) {
        (Some(subject), ConsumedSourceUnitKind::PackageAuthored)
            if custody.package_purpose() == Some(DependencyPurpose::Product)
                && subject.root_role() == BuildDeclarationKind::Application
                && source.unit().package() == Some(subject.root()) =>
        {
            encoder.byte(0);
            encoder.raw(&subject.root().digest());
            encoder.raw(&subject.source_consumption_commitment().digest());
            AssumedBindingRootOwner::Package(subject.root())
        }
        (None, ConsumedSourceUnitKind::CapturedRootAuthored(revision))
            if custody.package_purpose().is_none() && revision != [0; 32] =>
        {
            encoder.byte(1);
            encoder.raw(&revision);
            AssumedBindingRootOwner::CapturedRoot(revision)
        }
        _ => return None,
    };
    let build_source_revision = source.unit().revision_identity();
    encoder.raw(&build_source_revision);
    encoder.u64(source.start());
    encoder.u64(source.end());
    encoder.bytes(identity.as_bytes());
    encoder.bytes(application.name.as_str().as_bytes());
    encoder.byte(u8::from(application.artifact_only));
    let build_execution_profile = root.execution_profile();
    match build_execution_profile {
        Some(profile) => {
            encoder.byte(1);
            encoder.bytes(profile.identity().as_str().as_bytes());
        }
        None => encoder.byte(0),
    }
    encoder.bytes(target.identity().as_str().as_bytes());
    Some(AssumedBindingRootScope {
        owner,
        build_source_revision,
        build_execution_profile,
        target,
        digest: encoder.finish(),
    })
}
