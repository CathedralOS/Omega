//! Every rejection raised while constructing a package graph or attaching
//! target inputs to it, with the diagnostic text each one renders.

use super::display_identity;
use crate::{AcceptedSemanticBindingRole, BuildDeclarationKind};
use build_declarations::DependencyPurpose;
use semantic_vocabulary::PackageKeyIdentity;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCompilationInputError {
    InvalidDependencyAcquisition {
        root: PackageKeyIdentity,
        reason: String,
    },
    InvalidRootRole {
        role: BuildDeclarationKind,
    },
    InvalidPackageName {
        identity: PackageKeyIdentity,
        name: String,
    },
    InvalidSourceRoot {
        identity: PackageKeyIdentity,
        path: PathBuf,
        reason: String,
    },
    DuplicatePackageIdentity {
        identity: PackageKeyIdentity,
    },
    DuplicateSourceRoot {
        first: PackageKeyIdentity,
        duplicate: PackageKeyIdentity,
        path: PathBuf,
    },
    OverlappingSourceRoots {
        first: PackageKeyIdentity,
        first_root: PathBuf,
        second: PackageKeyIdentity,
        second_root: PathBuf,
    },
    MissingRootPackage {
        root: PackageKeyIdentity,
    },
    InvalidAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    MissingRequester {
        requester: PackageKeyIdentity,
    },
    MissingTarget {
        requester: PackageKeyIdentity,
        alias: String,
        target: PackageKeyIdentity,
    },
    DuplicateAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    NonRootBuildDependency {
        requester: PackageKeyIdentity,
        alias: String,
    },
    UnreachablePackage {
        identity: PackageKeyIdentity,
    },
    DependencyCycle {
        cycle: Vec<PackageKeyIdentity>,
    },
    RootGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    ForeignGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    DuplicateGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    MissingGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    RootIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    ForeignIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    DuplicateIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleClosureMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundlePurposeMismatch {
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
    },
    GeneratedSourceBundleCustodyMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleTargetMismatch {
        package: PackageKeyIdentity,
        bundle_target: target::TargetProfile,
        selected_target: Option<target::TargetProfile>,
    },
    GeneratedSourceBundleExecutionProfileMismatch {
        package: PackageKeyIdentity,
        bundle_profile: Option<target::TargetProfile>,
        execution_profile: Option<target::TargetProfile>,
    },
    ForeignSemanticBindingPackage {
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
    },
    DuplicateSemanticBindingRole {
        role: AcceptedSemanticBindingRole,
    },
}

impl fmt::Display for PackageCompilationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDependencyAcquisition { root, reason } => write!(
                formatter,
                "invalid acquired dependency requests for {}: {reason}",
                display_identity(*root)
            ),
            Self::InvalidRootRole { role } => write!(
                formatter,
                "package compilation root has inadmissible declaration role {role:?}"
            ),
            Self::InvalidPackageName { identity, name } => write!(
                formatter,
                "package identity {} has invalid canonical name `{name}`; expected lowercase snake_case",
                display_identity(*identity)
            ),
            Self::InvalidSourceRoot {
                identity,
                path,
                reason,
            } => write!(
                formatter,
                "package identity {} has invalid source root {}: {reason}",
                display_identity(*identity),
                path.display()
            ),
            Self::DuplicatePackageIdentity { identity } => write!(
                formatter,
                "package identity {} has more than one source row",
                display_identity(*identity)
            ),
            Self::DuplicateSourceRoot {
                first,
                duplicate,
                path,
            } => write!(
                formatter,
                "package identities {} and {} share source root {}",
                display_identity(*first),
                display_identity(*duplicate),
                path.display()
            ),
            Self::OverlappingSourceRoots {
                first,
                first_root,
                second,
                second_root,
            } => write!(
                formatter,
                "package identities {} ({}) and {} ({}) have overlapping source roots",
                display_identity(*first),
                first_root.display(),
                display_identity(*second),
                second_root.display()
            ),
            Self::MissingRootPackage { root } => write!(
                formatter,
                "root package identity {} has no source row",
                display_identity(*root)
            ),
            Self::InvalidAlias { requester, alias } => write!(
                formatter,
                "package identity {} has invalid dependency alias `{alias}`",
                display_identity(*requester)
            ),
            Self::MissingRequester { requester } => write!(
                formatter,
                "dependency requester {} has no package source row",
                display_identity(*requester)
            ),
            Self::MissingTarget {
                requester,
                alias,
                target,
            } => write!(
                formatter,
                "dependency `{alias}` from {} targets missing package {}",
                display_identity(*requester),
                display_identity(*target)
            ),
            Self::DuplicateAlias { requester, alias } => write!(
                formatter,
                "package identity {} binds dependency alias `{alias}` more than once in one scope",
                display_identity(*requester)
            ),
            Self::NonRootBuildDependency { requester, alias } => write!(
                formatter,
                "package identity {} declares build dependency `{alias}`; only the compilation root package may declare build dependencies",
                display_identity(*requester)
            ),
            Self::UnreachablePackage { identity } => write!(
                formatter,
                "package identity {} is unreachable from the root package",
                display_identity(*identity)
            ),
            Self::DependencyCycle { cycle } => {
                write!(formatter, "package dependency cycle")?;
                for identity in cycle {
                    write!(formatter, " -> {}", display_identity(*identity))?;
                }
                Ok(())
            }
            Self::RootGeneratedSourceBundle { package } => write!(
                formatter,
                "root package {} cannot inject a generated-source bundle before its own build",
                display_identity(*package)
            ),
            Self::ForeignGeneratedSourceBundle { package } => write!(
                formatter,
                "generated-source bundle names foreign package {}",
                display_identity(*package)
            ),
            Self::DuplicateGeneratedSourceBundle { package } => write!(
                formatter,
                "package {} has more than one generated-source bundle",
                display_identity(*package)
            ),
            Self::RootIndependentComponentDescription { package } => write!(
                formatter,
                "root package {} cannot attach its own component description; only a dependency compiled as its own component publishes one",
                display_identity(*package)
            ),
            Self::ForeignIndependentComponentDescription { package } => write!(
                formatter,
                "component description names foreign package {}",
                display_identity(*package)
            ),
            Self::DuplicateIndependentComponentDescription { package } => write!(
                formatter,
                "package {} has more than one component description",
                display_identity(*package)
            ),
            Self::MissingGeneratedSourceBundle { package } => write!(
                formatter,
                "dependency package {} has no generated-source bundle",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundleClosureMismatch { package } => write!(
                formatter,
                "generated-source bundle for package {} was produced from a different dependency closure",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundlePurposeMismatch { package, purpose } => write!(
                formatter,
                "generated-source bundle for package {} has an unrequested {} activation",
                display_identity(*package),
                purpose.name()
            ),
            Self::GeneratedSourceBundleCustodyMismatch { package } => write!(
                formatter,
                "generated-source bundle for package {} does not match its retained source custody and compiler review",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundleTargetMismatch {
                package,
                bundle_target,
                selected_target,
            } => write!(
                formatter,
                "generated-source bundle for package {} targets `{}` but compilation selected `{}`",
                display_identity(*package),
                bundle_target.target_name(),
                selected_target
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<none>"),
            ),
            Self::GeneratedSourceBundleExecutionProfileMismatch {
                package,
                bundle_profile,
                execution_profile,
            } => write!(
                formatter,
                "generated-source bundle for package {} was produced under build execution profile `{}` but compilation selected `{}`",
                display_identity(*package),
                bundle_profile
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<unprofiled host>"),
                execution_profile
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<unprofiled host>"),
            ),
            Self::ForeignSemanticBindingPackage { role, package } => write!(
                formatter,
                "accepted semantic binding {role:?} names package {} outside the compilation closure",
                display_identity(*package),
            ),
            Self::DuplicateSemanticBindingRole { role } => write!(
                formatter,
                "accepted semantic binding role {role:?} appears more than once",
            ),
        }
    }
}

impl std::error::Error for PackageCompilationInputError {}
