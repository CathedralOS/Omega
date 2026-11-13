/*
Session-local completed `Pure` `Build` retention and matching
(`PureBuildCompilation`, `CompletedPureBuildInputs`, `CompletedPureBuild`).

Why `PureBuildCompilation` is encapsulated inside `build-settings` with no
public carrier constructor or global registry:
1. Only a fresh admission (`CompletedPureBuildInputs::from_admitted`) followed by
   verified output-free completion (`CompletedPureBuild::from_completed`) can
   insert a retained entry into `PureBuildCompilation`. Callers may clone the
   `PureBuildCompilation` handle within a `BuildSession`, but cannot forge or
   import carriers across sessions.
2. `CompletedPureBuildInputs::matches` compares the complete checked `Build`
   program (`CheckedBuildProgram`), `BuildProviderPlans`, measurement schedule,
   sponsor limits, selected entry symbol, initial `BuildTimeValue` (using IEEE
   bit equality for floats), source assembly inputs, dependency handoff
   commitments (preserving both consumer import scope and producer purpose),
   snapshot request, activation identity, and captured source inventory.
3. Because full `CheckedBuildProgram` comparison traverses the accepted source and
   canonical artifact, both `matching_result` and `retain_completed` clone the
   append-only `Arc<CompletedPureBuild>` slice under the mutex and run the deep
   structural comparison outside the lock.
*/

use super::CompletedBuildSource;
use crate::ComputedBuildConfig;
use build_time_evaluation::BuildTimeValue;
use evaluation_accounting::{BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity};
use package_compilation::SourceAssemblyInputs;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct CompletedPureBuildInputs {
    checked: build_time_evaluation::CheckedBuildProgram,
    providers: crate::admission::provider_execution::BuildProviderPlans,
    measurement_identity: evaluation_accounting::EvaluationMeasurementIdentity,
    sponsor_policy: Option<(EvaluationMeasurementIdentity, BuildEvaluationSponsorLimits)>, // Distinguishes sponsored ceilings/units without retaining account authority.
    selected_entry: symbols::SymbolHandle,
    initial_build: BuildTimeValue,
    source: SourceAssemblyInputs,
    dependencies: Vec<CompletedPureBuildDependency>,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
    activation: package_compilation::BuildActivation,
    captured_inventory: Option<package_compilation::BuildCapturedSourceInventory>,
    artifact_only: bool,
    default_source: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct CompletedPureBuildDependency {
    consumer_scope: build_declarations::DependencyPurpose,
    package: semantic_vocabulary::PackageKeyIdentity,
    producer_purpose: build_declarations::DependencyPurpose,
    build_execution_profile: Option<target::TargetProfile>,
    dependency_closure: package_compilation::PackageDependencyClosure,
    source_consumption_commitment: package_compilation::PackageSourceConsumptionCommitment,
}

impl CompletedPureBuildInputs {
    pub(crate) fn from_admitted(
        checked: build_time_evaluation::CheckedBuildProgram,
        providers: crate::admission::provider_execution::BuildProviderPlans,
        selected_entry: symbols::SymbolHandle,
        initial_build: BuildTimeValue,
        packages: Option<&package_compilation::PackageCompilationInputs>,
        snapshot: Option<&package_compilation::BuildSnapshotRequest>,
        scope: &package_compilation::BuildMachineFilesystemScope,
        artifact_only: bool,
        default_source: bool,
        measurement_identity: evaluation_accounting::EvaluationMeasurementIdentity,
        sponsor: Option<&build_time_evaluation::BuildEvaluationSponsor>,
    ) -> Option<Self> {
        if artifact_only
            || !scope.required_outputs().is_empty()
            || snapshot.is_some_and(|snapshot| {
                !snapshot.inputs().is_empty() || !snapshot.required_outputs().is_empty()
            })
        {
            return None;
        }
        let activation = scope.activation();
        let captured_inventory = scope.captured_source_inventory();
        if packages.is_none() && captured_inventory.is_none() {
            return None;
        }
        Some(Self {
            checked,
            providers,
            measurement_identity,
            sponsor_policy: sponsor.map(|sponsor| (sponsor.measurement(), sponsor.limits())),
            selected_entry,
            initial_build,
            source: SourceAssemblyInputs::new(packages, activation.build_execution_profile()),
            dependencies: packages
                .into_iter()
                .flat_map(|packages| packages.dependency_build_handoff_instances())
                .map(|(consumer_scope, bundle)| CompletedPureBuildDependency {
                    consumer_scope,
                    package: bundle.package(),
                    producer_purpose: bundle.purpose(),
                    build_execution_profile: bundle.build_execution_profile(),
                    dependency_closure: bundle.dependency_closure().clone(),
                    source_consumption_commitment: bundle.source_consumption_commitment(),
                })
                .collect(),
            snapshot: snapshot.cloned(),
            activation,
            captured_inventory,
            artifact_only,
            default_source,
        })
    }

    fn matches(&self, other: &Self) -> bool {
        self.checked == other.checked
            && self.providers == other.providers
            && self.measurement_identity == other.measurement_identity
            && self.sponsor_policy == other.sponsor_policy
            && self.selected_entry == other.selected_entry
            && same_initial_value(&self.initial_build, &other.initial_build)
            && self.source == other.source
            && self.dependencies == other.dependencies
            && self.snapshot == other.snapshot
            && self.activation == other.activation
            && self.captured_inventory == other.captured_inventory
            && self.artifact_only == other.artifact_only
            && self.default_source == other.default_source
    }
}

/// ```compile_fail
/// let _ = build_settings::PureBuildCompilation::retain_completed;
/// ```
#[derive(Clone, Default)]
pub struct PureBuildCompilation {
    completed: Arc<Mutex<Vec<Arc<CompletedPureBuild>>>>, // Append-only store whose length is the publication generation.
}

impl PureBuildCompilation {
    pub(crate) fn matching_result(
        &self,
        inputs: &CompletedPureBuildInputs,
    ) -> Result<Option<Arc<CompletedPureBuild>>, Vec<diagnostics::Diagnostic>> {
        let completed = self
            .completed
            .lock()
            .map_err(|_| {
                vec![diagnostics::Diagnostic::error(
                    "completed Pure Build owner is unavailable",
                )]
            })?
            .clone();
        Ok(completed
            .iter()
            .find(|completed| completed.inputs.matches(inputs))
            .cloned())
    }

    pub(crate) fn retain_completed(
        &self,
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
        source: Arc<CompletedBuildSource>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let Some(completed) = CompletedPureBuild::from_completed(inputs, configuration, source)
        else {
            return Ok(());
        };
        let completed = Arc::new(completed);
        let mut compared_count = 0;
        loop {
            let candidates = {
                let mut retained = self.completed.lock().map_err(|_| {
                    vec![diagnostics::Diagnostic::error(
                        "completed Pure Build owner is unavailable",
                    )]
                })?;
                if compared_count == retained.len() {
                    retained.push(completed);
                    return Ok(());
                }
                retained[compared_count..].to_vec()
            };
            compared_count += candidates.len();
            if candidates
                .iter()
                .any(|candidate| candidate.inputs.matches(&completed.inputs))
            {
                return Ok(());
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct CompletedPureBuild {
    inputs: CompletedPureBuildInputs,
    pub(crate) configuration: Arc<ComputedBuildConfig>,
    pub(crate) source: Arc<CompletedBuildSource>,
}

impl CompletedPureBuild {
    fn from_completed(
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
        source: Arc<CompletedBuildSource>,
    ) -> Option<Self> {
        let usage = configuration.evaluation_usage?;
        let observations = configuration.observation_summary.as_ref()?;
        if usage.measurement != inputs.measurement_identity
            || usage.validate().is_err()
            || usage.filesystem_operation_attempts != 0
            || observations.filesystem_host_observed()
            || !observations.filesystem_operation_attempts().is_empty()
            || !observations.named_input_inventories().is_empty()
            || !observations.required_output_settlements().is_empty()
            || !observations.included_source_handoffs().is_empty()
            || observations
                .staged_output_tree()
                .is_none_or(|tree| tree.entry_count() != 0)
            || !configuration.generated_sources.is_empty()
            || configuration.selected_build_machine_symbol != Some(inputs.selected_entry)
            || configuration
                .accepted_build_source
                .as_ref()
                .is_none_or(|source| !Arc::ptr_eq(source, inputs.checked.accepted_source()))
            || observations.activation() != inputs.activation
            || observations.captured_source_inventory() != inputs.captured_inventory
        {
            return None;
        }
        Some(Self {
            inputs,
            configuration,
            source,
        })
    }
}

fn same_initial_value(left: &BuildTimeValue, right: &BuildTimeValue) -> bool {
    match (left, right) {
        (BuildTimeValue::Float(left), BuildTimeValue::Float(right)) => {
            left.to_bits() == right.to_bits()
        }
        (BuildTimeValue::Array(left), BuildTimeValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_initial_value(left, right))
        }
        (
            BuildTimeValue::Struct {
                type_name: left_name,
                fields: left,
            },
            BuildTimeValue::Struct {
                type_name: right_name,
                fields: right,
            },
        )
        | (
            BuildTimeValue::Case {
                variant: left_name,
                payload: left,
            },
            BuildTimeValue::Case {
                variant: right_name,
                payload: right,
            },
        ) => {
            left_name == right_name
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|((left_name, left), (right_name, right))| {
                        left_name == right_name && same_initial_value(left, right)
                    })
        }
        _ => left == right,
    }
}
