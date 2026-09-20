//! Occurrence coverage derived from the canonical source closure.
//!
//! The roster is the join column between package-keyed records and the exact
//! authorized occurrences a package serves. Inside one closure the selected
//! target and build execution profile are section-level facts, so occurrence
//! identity reduces to (package, purpose): the root occurs at the product
//! purpose, a product edge propagates every purpose its requester holds, and
//! a build edge contributes the build purpose for its selected package.

use crate::declarations::PackageKey;
use crate::declarations::dependencies::DependencyPurpose;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use std::fmt;

/// One package's authorized occurrences: the exact purposes it serves for the
/// enclosing closure's target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePurposeCoverage {
    package: PackageKey,
    purposes: Vec<DependencyPurpose>,
}

impl PackagePurposeCoverage {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    /// Sorted nonempty purposes this package occurrence set covers.
    pub fn purposes(&self) -> &[DependencyPurpose] {
        &self.purposes
    }
}

/// The complete occurrence roster of one canonical source closure, in the
/// subject's canonical package order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageOccurrenceRoster {
    coverages: Vec<PackagePurposeCoverage>,
}

impl PackageOccurrenceRoster {
    /// Derive the roster by propagating purposes over the validated graph.
    /// Subjects only exist once validated as one reachable acyclic closure, so
    /// every package ends with at least one purpose; the check stays total
    /// rather than assuming it.
    pub fn derive(
        source: &CanonicalSourceClosureSubject,
    ) -> Result<Self, PackageOccurrenceRosterError> {
        let packages = source.packages();
        let mut purposes = Vec::new();
        purposes
            .try_reserve_exact(packages.len())
            .map_err(|_| PackageOccurrenceRosterError::AllocationFailed)?;
        purposes.resize(packages.len(), 0u8);
        let position = |key: &PackageKey| {
            packages
                .binary_search_by(|package| package.key().cmp(key))
                .map_err(|_| PackageOccurrenceRosterError::UnknownPackage)
        };
        let root = position(source.root().selected().key())?;
        purposes[root] = purpose_mask_bit(DependencyPurpose::Product);
        let mut pending = vec![root];
        while let Some(requester) = pending.pop() {
            let requests = source.dependency_requests();
            let start =
                requests.partition_point(|request| request.requester() < packages[requester].key());
            let end = start
                + requests[start..]
                    .partition_point(|request| request.requester() == packages[requester].key());
            for request in &requests[start..end] {
                let contribution = if request.purpose().is_product() {
                    purposes[requester]
                } else {
                    purpose_mask_bit(DependencyPurpose::Build)
                };
                let selected = position(request.selected().key())?;
                if purposes[selected] & contribution != contribution {
                    purposes[selected] |= contribution;
                    pending.push(selected);
                }
            }
        }
        let mut coverages = Vec::new();
        coverages
            .try_reserve_exact(packages.len())
            .map_err(|_| PackageOccurrenceRosterError::AllocationFailed)?;
        for (package, mask) in packages.iter().zip(purposes.iter()) {
            if *mask == 0 {
                return Err(PackageOccurrenceRosterError::UnreachablePackage);
            }
            coverages.push(PackagePurposeCoverage {
                package: package.key().clone(),
                purposes: DependencyPurpose::ALL
                    .iter()
                    .copied()
                    .filter(|purpose| mask & purpose_mask_bit(*purpose) != 0)
                    .collect(),
            });
        }
        Ok(Self { coverages })
    }

    /// Coverages in the subject's canonical package order.
    pub fn coverages(&self) -> &[PackagePurposeCoverage] {
        &self.coverages
    }

    /// The exact purposes one package occurs under, or none when the package
    /// is absent from the closure.
    pub fn purposes(&self, package: &PackageKey) -> Option<&[DependencyPurpose]> {
        self.coverages
            .binary_search_by(|coverage| coverage.package.cmp(package))
            .ok()
            .map(|index| self.coverages[index].purposes.as_slice())
    }

    /// Whether (package, purpose) is an authorized occurrence in the closure.
    pub fn is_occurrence(&self, package: &PackageKey, purpose: DependencyPurpose) -> bool {
        self.purposes(package)
            .is_some_and(|purposes| purposes.contains(&purpose))
    }

    /// Total (package, purpose) occurrences across the closure.
    pub fn occurrence_count(&self) -> usize {
        self.coverages
            .iter()
            .map(|coverage| coverage.purposes.len())
            .sum()
    }
}

/// A closed failure while deriving occurrence coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageOccurrenceRosterError {
    /// The subject names a package outside its own canonical package set, or
    /// leaves a package unreachable; validated subjects never exhibit either.
    UnknownPackage,
    UnreachablePackage,
    AllocationFailed,
}

impl fmt::Display for PackageOccurrenceRosterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownPackage | Self::UnreachablePackage => {
                "source closure does not yield a complete occurrence roster"
            }
            Self::AllocationFailed => "package occurrence roster allocation failed",
        })
    }
}

impl std::error::Error for PackageOccurrenceRosterError {}

/// Purposes encode as one bitmask in `DependencyPurpose::ALL` order.
pub(crate) fn purpose_mask_bit(purpose: DependencyPurpose) -> u8 {
    let index = DependencyPurpose::ALL
        .iter()
        .position(|candidate| *candidate == purpose)
        .expect("dependency purpose belongs to the complete purpose list");
    u8::try_from(index)
        .ok()
        .and_then(|index| 1u8.checked_shl(u32::from(index)))
        .expect("dependency purpose count fits one mask byte")
}

#[cfg(test)]
mod tests {
    use super::{DependencyPurpose, PackageOccurrenceRoster};
    use crate::lock::{
        HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
        PackageLockRecoveryLimits, PackageLockTarget,
    };
    use crate::resolution::graph::{
        CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
        PackageSourceClosureLimits, resolve_external_local_package_closure,
    };
    use crate::review::{
        PackagePolicyChangeLimits, PackagePolicyDecision, PackagePolicyDecisionSubject,
        ReviewOnlyRootPolicyDisposition, SemanticBindingReview, compare_package_policy_changes,
        compile_resolved_package_reviews, resolve_package_policy_decisions,
    };
    use package_source::{
        ExternalSourceContext, LocalSourceLimits, PrimaryGitChoices, SourceResolverStorage,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use target::TargetProfile;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "omega-occurrence-roster-{}-{stamp}",
                std::process::id()
            )))
        }

        fn package(&self, name: &str, dependencies: &str) {
            let root = self.0.join(name);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(
                root.join("build.omg"),
                format!(
                    "machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n"
                ),
            )
            .unwrap();
            std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n").unwrap();
        }

        fn subject(&self) -> CanonicalSourceClosureSubject {
            let storage = SourceResolverStorage::for_hardened_base(
                self.0.join("cache"),
                PrimaryGitChoices::default(),
            )
            .unwrap();
            let closure = resolve_external_local_package_closure(
                self.0.join("root"),
                ExternalSourceContext::derive(b"occurrence-roster"),
                &storage,
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
            )
            .unwrap();
            CanonicalSourceClosureSubject::from_resolved(
                &closure.for_exact_target(TargetProfile::CrossPlatformCli),
                CanonicalSourceClosureSubjectLimits::default(),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn purposes_of(
        roster: &PackageOccurrenceRoster,
        subject: &CanonicalSourceClosureSubject,
        name: &str,
    ) -> Vec<DependencyPurpose> {
        let package = subject
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == name)
            .unwrap_or_else(|| panic!("fixture contains package {name}"));
        roster.purposes(package.key()).unwrap().to_vec()
    }

    #[test]
    fn build_edges_and_dual_purpose_shared_custody_shape_the_roster() {
        let fixture = Fixture::new();
        fixture.package(
            "root",
            concat!(
                "builder.depend(Source::Path { location: \"../left\" });",
                "builder.depend_as(\"shared_product\", Source::Path { location: \"../shared\" });",
                "builder.build_depend_as(\"tool\", Source::Path { location: \"../tool\" });",
                "builder.build_depend_as(\"shared_build\", Source::Path { location: \"../shared\" });",
            ),
        );
        fixture.package(
            "left",
            "builder.depend(Source::Path { location: \"../shared\" });",
        );
        fixture.package(
            "tool",
            "builder.depend(Source::Path { location: \"../hostlib\" });",
        );
        fixture.package("shared", "");
        fixture.package("hostlib", "");
        let subject = fixture.subject();
        let roster = PackageOccurrenceRoster::derive(&subject).unwrap();

        assert_eq!(
            purposes_of(&roster, &subject, "root"),
            vec![DependencyPurpose::Product]
        );
        assert_eq!(
            purposes_of(&roster, &subject, "left"),
            vec![DependencyPurpose::Product]
        );
        assert_eq!(
            purposes_of(&roster, &subject, "tool"),
            vec![DependencyPurpose::Build]
        );
        // A product edge propagates every purpose its requester serves.
        assert_eq!(
            purposes_of(&roster, &subject, "hostlib"),
            vec![DependencyPurpose::Build]
        );
        // Shared custody under both purposes keeps distinct occurrences.
        assert_eq!(
            purposes_of(&roster, &subject, "shared"),
            vec![DependencyPurpose::Product, DependencyPurpose::Build]
        );
        assert_eq!(roster.occurrence_count(), 6);
        assert!(
            roster.is_occurrence(
                subject
                    .packages()
                    .iter()
                    .find(|package| package.key().name().as_str() == "shared")
                    .unwrap()
                    .key(),
                DependencyPurpose::Build
            )
        );
    }

    #[test]
    fn occurrence_ledger_roundtrips_and_rejects_tampered_or_legacy_shape() {
        let fixture = Fixture::new();
        fixture.package(
            "root",
            "builder.build_depend_as(\"tool\", Source::Path { location: \"../tool\" });",
        );
        fixture.package("tool", "");
        let Some(profile) = TargetProfile::host_if_supported() else {
            eprintln!("SKIP: package build review requires a catalogued execution host");
            return;
        };
        let storage = SourceResolverStorage::for_hardened_base(
            fixture.0.join("ledger-cache"),
            PrimaryGitChoices::default(),
        )
        .unwrap();
        let closure = resolve_external_local_package_closure(
            fixture.0.join("root"),
            ExternalSourceContext::derive(b"occurrence-ledger"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap();
        let target_closure = closure.for_exact_target(profile);
        let subject = CanonicalSourceClosureSubject::from_resolved(
            &target_closure,
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();
        // The ordinary acceptance flow is the only consent constructor.
        let reviews = compile_resolved_package_reviews(
            &target_closure,
            &fixture.0.join("build"),
            SemanticBindingReview::Discover,
        )
        .unwrap();
        let changes = compare_package_policy_changes(
            None,
            &reviews,
            &target_closure,
            PackagePolicyChangeLimits::default(),
        )
        .unwrap();
        let choices = changes
            .packages()
            .iter()
            .flat_map(|package| package.rows())
            .filter(|row| row.requires_decision())
            .map(|row| PackagePolicyDecision {
                subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
                disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
            })
            .collect::<Vec<_>>();
        let resolution =
            resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &choices)
                .unwrap();
        let baselines = subject
            .packages()
            .iter()
            .map(|package| {
                reviews
                    .review(package.key())
                    .expect("reviewed package")
                    .policy()
                    .clone()
            })
            .collect();
        let history = HistoricalPackagePolicyDecisions::capture_policy(
            &subject,
            &changes,
            &resolution,
            HistoricalPackagePolicyLimits::default(),
        )
        .unwrap();
        let target = PackageLockTarget::from_parts(subject.clone(), baselines, history).unwrap();
        assert_eq!(
            target.occurrence_purposes_for(
                &target
                    .source()
                    .packages()
                    .iter()
                    .find(|package| package.key().name().as_str() == "tool")
                    .unwrap()
                    .key()
                    .clone()
            ),
            Some(&[DependencyPurpose::Build][..])
        );
        let lock = PackageLock::from_targets(vec![target.clone()]).unwrap();
        let text = lock.canonical_text().unwrap();
        assert!(text.contains("occurrences 2\noccurrence 0 product\noccurrence 1 build\n"));
        let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default())
            .expect("recover recorded occurrence coverage");
        assert_eq!(recovered.targets()[0], target);

        // Locks written before the ledger assign the derived roster.
        let ledger_start = text.find("occurrences ").unwrap();
        let ledger_end = text[ledger_start..].find("acceptances ").unwrap() + ledger_start;
        let legacy = format!("{}{}", &text[..ledger_start], &text[ledger_end..]);
        let legacy_recovered =
            PackageLock::recover_text(&legacy, PackageLockRecoveryLimits::default())
                .expect("pre-ledger lock keeps implicit complete coverage");
        assert_eq!(legacy_recovered.targets()[0], target);

        let mut missing_row = text.replacen("occurrences 2\n", "occurrences 1\n", 1);
        missing_row = missing_row.replacen("occurrence 1 build\n", "", 1);
        for tampered in [
            text.replacen("occurrence 1 build", "occurrence 1 product", 1),
            text.replacen("occurrence 1 build", "occurrence 0 build", 1),
            text.replacen("occurrence 1 build", "occurrence 99 product", 1),
            missing_row,
        ] {
            assert!(
                PackageLock::recover_text(&tampered, PackageLockRecoveryLimits::default()).is_err(),
                "tampered occurrence ledger must reject: {tampered:?}"
            );
        }
    }
}
