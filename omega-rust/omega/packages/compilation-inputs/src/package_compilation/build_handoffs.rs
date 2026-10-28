//! Complete per-purpose output handoffs from admitted dependency Builds.
//!
//! The existing package/target/source-custody roster carries generated Omega
//! source and executed Product choices together. Each activation exports only
//! its own outputs. Receiving admission validates their source, declaration
//! roles and precedence; this carrier grants no private nameability, host
//! backing or execution authority.

use super::{PackageCompilationInputError, PackageCompilationInputs, PackageDependencyClosure};
use crate::PackageSourceConsumptionCommitment;
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable generated source and Product intent from one admitted package Build.
/// Construction remains compiler-private: carrying this value proves source
/// and output custody, not final checking of the generated program or package
/// admission. Every consumer checks the generated declarations it incorporates.
/// Clones share the immutable bundle, including paths and dependency metadata;
/// each consumer still validates its own closure, custody, purpose, effective
/// target, and build execution profile. Equal product bytes do not establish that the
/// generator ran under the consumer's admitted build context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageBuildHandoff {
    contents: Arc<PackageBuildHandoffContents>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageBuildHandoffContents {
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
    native_bindings: Arc<[super::retained_native_bindings::RetainedNativeBinding]>,
    native_evaluation: Option<Arc<super::NativeBuildEvaluation>>,
}

impl PackageBuildHandoff {
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
            contents: Arc::new(PackageBuildHandoffContents {
                package,
                purpose,
                target,
                build_execution_profile,
                dependency_closure,
                source_consumption_commitment,
                sources,
                provider_selections: Arc::from([]),
                selection_observation: None,
                native_bindings: Arc::from([]),
                native_evaluation: None,
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

    /// The one generated unit selected by exact relative paths. Source imports
    /// and retained declaration admission share path decoding and ambiguity.
    pub(crate) fn generated_source(
        &self,
        relative_candidates: &[PathBuf],
    ) -> Result<Option<&PackageGeneratedSource>, &'static str> {
        let mut matched = None;
        for source in self.sources() {
            let relative = generated_source_relative_path(source)?;
            if relative_candidates
                .iter()
                .any(|candidate| candidate == &relative)
            {
                if matched.is_some() {
                    return Err("generated-source import resolves to more than one handoff");
                }
                matched = Some(source);
            }
        }
        Ok(matched)
    }

    /// Own-package executed choices. Inherited rows already have their own
    /// package/purpose handoff and must not be exported a second time.
    pub fn provider_selections(
        &self,
    ) -> &[build_time_evaluation::provider_declarations::RetainedProviderSelection] {
        &self.contents.provider_selections
    }

    pub fn native_bindings(&self) -> &[super::retained_native_bindings::RetainedNativeBinding] {
        &self.contents.native_bindings
    }

    pub fn native_evaluation(&self) -> Option<&Arc<super::NativeBuildEvaluation>> {
        self.contents.native_evaluation.as_ref()
    }

    /// Only completed immutable package checking exports native application
    /// judgments. Recovery retains them inertly; receiving source admission
    /// independently checks every supplier and declaration anchor.
    #[doc(hidden)]
    pub fn with_checked_native_bindings(
        self,
        bindings: Vec<super::retained_native_bindings::RetainedNativeBinding>,
        evaluation: Option<Arc<super::NativeBuildEvaluation>>,
    ) -> Result<Self, &'static str> {
        if !bindings.is_empty() && evaluation.is_none() {
            return Err("native package intent lost its completed Terminal Build code");
        }
        if let Some(evaluation) = &evaluation {
            evaluation.usage.validate().map_err(|_| "native package evaluation has inconsistent accounting")?;
            if !matches!(evaluation.usage.measurement, evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi { .. }) {
                return Err("native package intent requires its actual Terminal evaluation");
            }
            if self.selection_observation().is_some_and(|provider| provider != evaluation.observation_digest) {
                return Err("one package handoff mixes different original Build observations");
            }
            if bindings.iter().any(|row| row.observation != evaluation.observation_digest || row.usage != evaluation.usage) {
                return Err("native package code and retained rows disagree on their original Build invocation");
            }
        }
        for (position, binding) in bindings.iter().enumerate() {
            binding.validate()?;
            if bindings[..position]
                .iter()
                .any(|prior| prior.origin == binding.origin)
            {
                return Err("native package intent repeats an executed Build occurrence");
            }
            if binding.origin.activation().source().package_identity != Some(self.package())
                || binding.origin.event_ordinal().is_none()
                || binding.observation == [0; 32]
                || binding.receipt.producer_package()
                    != binding.origin.author().source().package_identity
                || binding.usage.validate().is_err()
            {
                return Err(
                    "native package intent lost its checked activation, supplier or Build observation",
                );
            }
            if let Some(first) = bindings.first()
                && (first.origin.activation() != binding.origin.activation()
                    || first.observation != binding.observation
                    || first.usage != binding.usage)
            {
                return Err("native package intents mix sponsoring Build invocations");
            }
            if let Some(demand) = &binding.same_stack_demand
                && (demand.bytes == 0
                    || demand.origin.activation() != binding.origin.activation()
                    || demand.origin.author().source().package_identity
                        != binding.origin.author().source().package_identity)
            {
                return Err(
                    "native package footprint differs from its original binding activation or supplier",
                );
            }
        }
        let mut contents = Arc::unwrap_or_clone(self.contents);
        contents.native_bindings = bindings.into();
        contents.native_evaluation = evaluation;
        Ok(Self {
            contents: Arc::new(contents),
        })
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
        if let (Some(provider), Some(native)) = (observation, self.native_evaluation()) {
            if provider != native.observation_digest {
                return Err("one package handoff mixes different original Build observations");
            }
        }
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
        if let Some(first) = selections.first()
            && selections
                .iter()
                .any(|selection| selection.origin.activation() != first.origin.activation())
        {
            return Err("package provider choices mix different original Build activations");
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
    pub fn with_complete_dependency_build_handoffs(
        mut self,
        bundles: Vec<PackageBuildHandoff>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let required = self.required_dependency_source_instances();
        let mut generated = BTreeMap::new();
        for bundle in bundles {
            let package = bundle.package();
            let purpose = bundle.purpose();
            if package == self.source.root {
                errors.push(PackageCompilationInputError::RootBuildHandoff { package });
                continue;
            }
            if !self.source.packages.contains_key(&package) {
                errors.push(PackageCompilationInputError::ForeignBuildHandoff { package });
                continue;
            }
            if !required.contains(&(package, purpose)) {
                errors.push(PackageCompilationInputError::BuildHandoffPurposeMismatch {
                    package,
                    purpose,
                });
                continue;
            }
            if bundle.dependency_closure() != &self.dependency_closure_for(package) {
                errors.push(PackageCompilationInputError::BuildHandoffClosureMismatch { package });
            }
            if generated.insert((package, purpose), bundle).is_some() {
                errors.push(PackageCompilationInputError::DuplicateBuildHandoff { package });
            }
        }
        for (package, purpose) in required {
            if !generated.contains_key(&(package, purpose)) {
                errors.push(PackageCompilationInputError::MissingBuildHandoff { package });
            }
        }
        if errors.is_empty() {
            self.target.dependency_build_handoffs = generated;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub(crate) fn dependency_build_handoffs(&self) -> impl Iterator<Item = &PackageBuildHandoff> {
        self.target.dependency_build_handoffs.values()
    }

    /// Each retained handoff together with the relative source scope in which
    /// it is consumed. The same build instance can serve both relative scopes
    /// inside a build helper, but never serves a product activation.
    pub fn dependency_build_handoff_instances(
        &self,
    ) -> impl Iterator<Item = (DependencyPurpose, &PackageBuildHandoff)> {
        self.dependency_source_instances()
            .into_iter()
            .filter_map(|(package, scope)| {
                self.target
                    .dependency_build_handoffs
                    .get(&(package, self.instance_purpose(scope)))
                    .map(|bundle| (scope, bundle))
            })
    }

    #[doc(hidden)]
    pub fn validate_dependency_build_handoff_target(
        &self,
        selected_target: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_build_handoffs
            .values()
            .filter_map(|bundle| {
                let expected_target = match bundle.purpose() {
                    DependencyPurpose::Product => selected_target,
                    DependencyPurpose::Build => bundle.build_execution_profile(),
                };
                (Some(bundle.target()) != expected_target).then_some(
                    PackageCompilationInputError::BuildHandoffTargetMismatch {
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
    pub fn validate_dependency_build_handoff_execution_profile(
        &self,
        execution_profile: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_build_handoffs
            .values()
            .filter(|bundle| bundle.build_execution_profile() != execution_profile)
            .map(
                |bundle| PackageCompilationInputError::BuildHandoffExecutionProfileMismatch {
                    package: bundle.package(),
                    bundle_profile: bundle.build_execution_profile(),
                    execution_profile,
                },
            )
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
            .dependency_build_handoffs
            .get(&(package, self.instance_purpose(import_scope)))
        else {
            return Ok(None);
        };
        let Some(source) = bundle.generated_source(relative_candidates)? else {
            return Ok(None);
        };
        Ok(Some(generated_source_logical_path(
            self.package_root(package)
                .ok_or("generated-source handoff lost its source supplier")?,
            &generated_source_relative_path(source)?,
        )))
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
