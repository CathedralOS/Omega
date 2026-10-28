//! Generated Omega source handed off by checked dependency builds: the
//! immutable bundle, the per-purpose instance roster a complete handoff
//! must cover, target and execution-profile checks, and import routing
//! into a bundle's logical source paths.

use super::{PackageCompilationInputError, PackageCompilationInputs, PackageDependencyClosure};
use crate::PackageSourceConsumptionCommitment;
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Exact generated Omega source handed off by one admitted package Build.
/// Construction remains compiler-private: carrying this value proves source
/// and output custody, not final checking of the generated program or package
/// admission. Every consumer checks the generated declarations it incorporates.
/// Clones share the immutable bundle, including paths and dependency metadata;
/// each consumer still validates its own closure, custody, purpose, effective
/// target, and build execution profile. Equal product bytes do not establish that the
/// generator ran under the consumer's admitted build context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGeneratedSourceBundle {
    contents: Arc<PackageGeneratedSourceBundleContents>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageGeneratedSourceBundleContents {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    dependency_closure: PackageDependencyClosure,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    sources: Vec<PackageGeneratedSource>,
    provider_selections:
        Arc<[build_time_evaluation::provider_declarations::RetainedProviderSelection]>,
    selection_observation: Option<[u8; 32]>,
}

impl PackageGeneratedSourceBundle {
    #[doc(hidden)]
    pub fn from_checked(
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        build_execution_profile: Option<target::TargetProfile>,
        dependency_closure: PackageDependencyClosure,
        source_consumption_commitment: PackageSourceConsumptionCommitment,
        sources: Vec<PackageGeneratedSource>,
    ) -> Self {
        Self::from_source_consumption(
            package,
            purpose,
            target,
            build_execution_profile,
            dependency_closure,
            source_consumption_commitment,
            sources,
        )
    }

    pub(crate) fn from_source_consumption(
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        build_execution_profile: Option<target::TargetProfile>,
        dependency_closure: PackageDependencyClosure,
        source_consumption_commitment: PackageSourceConsumptionCommitment,
        sources: Vec<PackageGeneratedSource>,
    ) -> Self {
        Self {
            contents: Arc::new(PackageGeneratedSourceBundleContents {
                package,
                purpose,
                target,
                build_execution_profile,
                dependency_closure,
                source_consumption_commitment,
                sources,
                provider_selections: Arc::from([]),
                selection_observation: None,
            }),
        }
    }

    pub fn package(&self) -> PackageKeyIdentity {
        self.contents.package
    }

    /// The checked activation that produced the bundle, not the import edge's
    /// relative scope. Ordinary dependencies of a build helper are build
    /// instances too, even though the helper imports them through `depend`.
    pub fn purpose(&self) -> DependencyPurpose {
        self.contents.purpose
    }

    pub fn target(&self) -> target::TargetProfile {
        self.contents.target
    }

    /// The admitted profile that checked and executed the producer's build.
    /// None denotes the unprofiled host of this in-process compiler run, not
    /// permission to substitute a catalogued execution profile.
    pub fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.contents.build_execution_profile
    }

    pub fn dependency_closure(&self) -> &PackageDependencyClosure {
        &self.contents.dependency_closure
    }

    pub fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.contents.source_consumption_commitment
    }

    pub fn sources(&self) -> &[PackageGeneratedSource] {
        &self.contents.sources
    }

    /// Own-package executed choices. Inherited rows already have their own
    /// package/purpose handoff and must not be exported a second time.
    pub fn provider_selections(
        &self,
    ) -> &[build_time_evaluation::provider_declarations::RetainedProviderSelection] {
        &self.contents.provider_selections
    }

    pub fn selection_observation(&self) -> Option<[u8; 32]> {
        self.contents.selection_observation
    }

    #[doc(hidden)]
    pub fn with_retained_provider_selections(
        self,
        selections: Vec<build_time_evaluation::provider_declarations::RetainedProviderSelection>,
        observation: Option<[u8; 32]>,
    ) -> Result<Self, &'static str> {
        if !selections.is_empty() && observation.is_none() {
            return Err("package provider choices have no original Build observation");
        }
        if selections.iter().any(|selection| {
            selection.origin.activation().source().package_identity != Some(self.package())
                || selection.origin.event_ordinal().is_none()
        }) {
            return Err(
                "package provider choices do not belong to this handoff's Build activation",
            );
        }
        let mut contents = Arc::unwrap_or_clone(self.contents);
        contents.provider_selections = selections.into();
        contents.selection_observation = observation;
        Ok(Self {
            contents: Arc::new(contents),
        })
    }

    #[doc(hidden)]
    pub fn with_checked_provider_selections(
        self,
        typed: &typed_trees::TypedTrees,
        selections: &[build_time_evaluation::provider_declarations::ProviderSelection],
        observation: Option<&crate::BuildObservationSummary>,
    ) -> Result<Self, String> {
        let retained = selections
            .iter()
            .map(|selection| {
                build_time_evaluation::provider_declarations::RetainedProviderSelection::capture(
                    typed, selection,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.with_retained_provider_selections(
            retained,
            observation.map(|summary| summary.identity().digest()),
        )
        .map_err(str::to_owned)
    }
}

impl PackageCompilationInputs {
    fn instance_purpose(&self, import_scope: DependencyPurpose) -> DependencyPurpose {
        match import_scope {
            DependencyPurpose::Product => self.compilation_purpose(),
            DependencyPurpose::Build => DependencyPurpose::Build,
        }
    }

    /// Relative import scopes are distinct from checked activation purposes.
    /// Product edges inherit their importing activation; only a build edge
    /// switches to the execution-profile activation. Construct this finite
    /// roster from the validated graph, never from supplied output bundles.
    fn dependency_source_instances(&self) -> BTreeSet<(PackageKeyIdentity, DependencyPurpose)> {
        let mut instances = BTreeSet::new();
        let mut pending = vec![(self.source.root, DependencyPurpose::Product)];
        pending.extend(
            self.build_dependencies()
                .map(|(_, _, package)| (package, DependencyPurpose::Build)),
        );
        while let Some((package, scope)) = pending.pop() {
            if !instances.insert((package, scope)) {
                continue;
            }
            if let Some(record) = self.source.packages.get(&package) {
                pending.extend(
                    record
                        .dependencies
                        .values()
                        .map(|dependency| (*dependency, scope)),
                );
            }
        }
        instances.remove(&(self.source.root, DependencyPurpose::Product));
        instances
    }

    /// Checked dependency occurrences required by this compilation. Product
    /// imports inherit the root's purpose; build imports always select Build.
    /// Producers use the same roster that validates the complete handoff.
    pub fn required_dependency_source_instances(
        &self,
    ) -> BTreeSet<(PackageKeyIdentity, DependencyPurpose)> {
        self.dependency_source_instances()
            .into_iter()
            .map(|(package, scope)| (package, self.instance_purpose(scope)))
            .collect()
    }

    /// Attach the complete set of fresh compiler-issued generated-source
    /// bundles for this root's dependencies. Empty bundles are retained so an
    /// omitted dependency build cannot be confused with a build that handed
    /// off no generated source. Coverage is per checked purpose, not package:
    /// identical target profiles or bytes never let one role satisfy another.
    pub fn with_complete_dependency_generated_sources(
        mut self,
        bundles: Vec<PackageGeneratedSourceBundle>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let required = self.required_dependency_source_instances();
        let mut generated = BTreeMap::new();
        for bundle in bundles {
            let package = bundle.package();
            let purpose = bundle.purpose();
            if package == self.source.root {
                errors.push(PackageCompilationInputError::RootGeneratedSourceBundle { package });
                continue;
            }
            if !self.source.packages.contains_key(&package) {
                errors.push(PackageCompilationInputError::ForeignGeneratedSourceBundle { package });
                continue;
            }
            if !required.contains(&(package, purpose)) {
                errors.push(
                    PackageCompilationInputError::GeneratedSourceBundlePurposeMismatch {
                        package,
                        purpose,
                    },
                );
                continue;
            }
            if bundle.dependency_closure() != &self.dependency_closure_for(package) {
                errors.push(
                    PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package },
                );
            }
            if generated.insert((package, purpose), bundle).is_some() {
                errors
                    .push(PackageCompilationInputError::DuplicateGeneratedSourceBundle { package });
            }
        }
        for (package, purpose) in required {
            if !generated.contains_key(&(package, purpose)) {
                errors.push(PackageCompilationInputError::MissingGeneratedSourceBundle { package });
            }
        }
        if errors.is_empty() {
            self.target.dependency_generated_sources = generated;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub(crate) fn dependency_generated_source_bundles(
        &self,
    ) -> impl Iterator<Item = &PackageGeneratedSourceBundle> {
        self.target.dependency_generated_sources.values()
    }

    /// Each retained handoff together with the relative source scope in which
    /// it is consumed. The same build instance can serve both relative scopes
    /// inside a build helper, but never serves a product activation.
    pub fn dependency_generated_source_instances(
        &self,
    ) -> impl Iterator<Item = (DependencyPurpose, &PackageGeneratedSourceBundle)> {
        self.dependency_source_instances()
            .into_iter()
            .filter_map(|(package, scope)| {
                self.target
                    .dependency_generated_sources
                    .get(&(package, self.instance_purpose(scope)))
                    .map(|bundle| (scope, bundle))
            })
    }

    #[doc(hidden)]
    pub fn validate_dependency_generated_source_target(
        &self,
        selected_target: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_generated_sources
            .values()
            .filter_map(|bundle| {
                let expected_target = match bundle.purpose() {
                    DependencyPurpose::Product => selected_target,
                    DependencyPurpose::Build => bundle.build_execution_profile(),
                };
                (Some(bundle.target()) != expected_target).then_some(
                    PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
                        package: bundle.package(),
                        bundle_target: bundle.target(),
                        selected_target: expected_target,
                    },
                )
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Generated bytes retain the build context that produced them even when
    /// their product target and source bytes equal another activation's output.
    pub fn validate_dependency_generated_source_execution_profile(
        &self,
        execution_profile: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_generated_sources
            .values()
            .filter(|bundle| bundle.build_execution_profile() != execution_profile)
            .map(|bundle| {
                PackageCompilationInputError::GeneratedSourceBundleExecutionProfileMismatch {
                    package: bundle.package(),
                    bundle_profile: bundle.build_execution_profile(),
                    execution_profile,
                }
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub(crate) fn generated_source_import_path(
        &self,
        package: PackageKeyIdentity,
        import_scope: DependencyPurpose,
        relative_candidates: &[PathBuf],
    ) -> Result<Option<PathBuf>, &'static str> {
        let Some(bundle) = self
            .target
            .dependency_generated_sources
            .get(&(package, self.instance_purpose(import_scope)))
        else {
            return Ok(None);
        };
        let mut matched = None;
        for source in bundle.sources() {
            let relative = generated_source_relative_path(source)?;
            if relative_candidates
                .iter()
                .any(|candidate| candidate == &relative)
            {
                if matched.is_some() {
                    return Err("generated-source import resolves to more than one handoff");
                }
                matched = Some(generated_source_logical_path(
                    self.package_root(package)
                        .expect("validated bundle package retains its source root"),
                    &relative,
                ));
            }
        }
        Ok(matched)
    }
}

fn generated_source_relative_path(
    source: &PackageGeneratedSource,
) -> Result<PathBuf, &'static str> {
    let mut path = PathBuf::new();
    for component in source.relative_path().split(|byte| *byte == b'/') {
        let component = std::str::from_utf8(component)
            .map_err(|_| "generated-source path is not canonical UTF-8")?;
        path.push(component);
    }
    Ok(path)
}

fn generated_source_logical_path(package_root: &Path, relative: &Path) -> PathBuf {
    package_root.join(".omega/generated").join(relative)
}
