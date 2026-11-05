//! Private completed Pure Build ownership inside the execution issuer.
//!
//! Keeping insertion here matters: a public product constructor could pair an
//! old configuration with a new owner without executing it. Only this crate's
//! successful admission/value/output protocol may publish a completed value;
//! public callers can share its owner, but cannot extract or inject carriers.

use crate::ComputedBuildConfig;
use build_time_evaluation::BuildTimeValue;
use evaluation_accounting::{BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity};
use package_compilation::SourceAssemblyInputs;
use std::sync::{Arc, Mutex};

/// Exact content and custody of an admitted Pure Build. Product selections
/// remain fresh admission inputs; only their complete checked Build program
/// can establish that two consumers observe the same computation.
#[derive(Clone)]
pub(crate) struct CompletedPureBuildInputs {
    checked: build_time_evaluation::CheckedBuildProgram,
    // Retain the judgment that produced this exact program. It is a candidate
    // for Psi's full input/authority match, never a replacement for admission
    // or another field in the completed-value key.
    retained_check: Option<psi_compiler::RetainedCheck>,
    providers: crate::admission::provider_execution::BuildProviderPlans,
    measurement_identity: evaluation_accounting::EvaluationMeasurementIdentity,
    // Absence is the ordinary per-invocation policy, not a reason to repeat
    // pure execution. Sponsored usage carries different ceilings and units;
    // retain that distinction without retaining the account's authority.
    sponsor_policy: Option<(EvaluationMeasurementIdentity, BuildEvaluationSponsorLimits)>,
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

/// The consumer's import scope and the producer's checked purpose can differ:
/// an ordinary dependency of a Build helper is still produced for Build.
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
    /// Called only after the current program's semantic, package, operational,
    /// and authority admission. Keeping this construction separate from the
    /// completed carrier prevents an admitted-but-unexecuted value from reuse.
    pub(crate) fn from_admitted(
        checked: build_time_evaluation::CheckedBuildProgram,
        retained_check: Option<psi_compiler::RetainedCheck>,
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
            eprintln!(
                "Pure Build input declined: entry {:?}, artifact_only={}, scoped_outputs={}, snapshot_inputs={}, snapshot_outputs={}",
                selected_entry,
                artifact_only,
                scope.required_outputs().len(),
                snapshot.map_or(0, |snapshot| snapshot.inputs().len()),
                snapshot.map_or(0, |snapshot| snapshot.required_outputs().len()),
            );
            return None;
        }
        let activation = scope.activation();
        let captured_inventory = scope.captured_source_inventory();
        // A captured source can be checked before it has a package identity.
        // Its complete immutable inventory still binds the same private Pure
        // value; an uncaptured anonymous source supplies no reuse custody.
        // Fresh semantic/grant admission and completion checks remain required.
        if packages.is_none() && captured_inventory.is_none() {
            eprintln!(
                "Pure Build input declined: entry {selected_entry:?} has no package or authenticated capture"
            );
            return None;
        }
        Some(Self {
            checked,
            retained_check,
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
        let matches = self.checked == other.checked
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
            && self.default_source == other.default_source;
        if !matches {
            eprintln!(
                "Pure Build input mismatch: retained entry {:?}, current entry {:?}; checked={} providers={} measurement={} sponsor={} entry={} initial={} source={} dependencies={} snapshot={} activation={} capture={} artifact_only={} default_source={}",
                self.selected_entry,
                other.selected_entry,
                self.checked == other.checked,
                self.providers == other.providers,
                self.measurement_identity == other.measurement_identity,
                self.sponsor_policy == other.sponsor_policy,
                self.selected_entry == other.selected_entry,
                same_initial_value(&self.initial_build, &other.initial_build),
                self.source == other.source,
                self.dependencies == other.dependencies,
                self.snapshot == other.snapshot,
                self.activation == other.activation,
                self.captured_inventory == other.captured_inventory,
                self.artifact_only == other.artifact_only,
                self.default_source == other.default_source,
            );
        }
        matches
    }
}

/// One compilation's private completed-Build store. Clones are explicit
/// consumers of this owner; a fresh owner cannot import another's carriers.
/// No global registry, public carrier constructor, decoder, or carrier export
/// turns equal content into cross-compilation reuse.
/// ```compile_fail
/// let _ = build_settings::PureBuildCompilation::retain_completed;
/// ```
#[derive(Clone, Default)]
pub struct PureBuildCompilation {
    // Append-only: the length is the publication generation. Entries never
    // change or disappear while any consumer can hold this owner.
    completed: Arc<Mutex<Vec<Arc<CompletedPureBuild>>>>,
}

impl PureBuildCompilation {
    /// Completed Pure inputs retain the immutable language judgment that
    /// produced them. The checker still matches its full preparation input and
    /// admits this request before taking any earlier result.
    pub(crate) fn completed_checks(
        &self,
    ) -> Result<Vec<psi_compiler::RetainedCheck>, Vec<diagnostics::Diagnostic>> {
        let completed = self.completed.lock().map_err(|_| {
            vec![diagnostics::Diagnostic::error(
                "completed Pure Build owner is unavailable",
            )]
        })?;
        Ok(completed
            .iter()
            .filter_map(|completed| completed.inputs.retained_check.clone())
            .collect())
    }

    pub(crate) fn matching_configuration(
        &self,
        inputs: &CompletedPureBuildInputs,
    ) -> Result<Option<Arc<ComputedBuildConfig>>, Vec<diagnostics::Diagnostic>> {
        // Snapshot only shared immutable entries while locked. Exact checked
        // program equality can traverse the whole program and all CheckFacts.
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
            .find_map(|completed| completed.matching_configuration(inputs)))
    }

    /// Evaluation publishes here only after successful value/output settlement.
    /// Consumers cannot obtain or insert an already-issued carrier from a
    /// different compilation; only the completed configuration is observable.
    pub(crate) fn retain_completed(
        &self,
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let Some(completed) = CompletedPureBuild::from_completed(inputs, configuration) else {
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
                // Only append if every published entry has been compared.
                // A competing append changes the length, so compare its new
                // suffix outside the lock before attempting publication again.
                if compared_count == retained.len() {
                    retained.push(completed);
                    return Ok(());
                }
                retained[compared_count..].to_vec()
            };
            // A test may make competing writers observe the same prefix.
            // Wait only after unlocking and only before the first comparison.
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

/// One completed, output-free Pure Build retained only by its compilation.
/// No prepared entry token, host grant, checked product, or consuming account
/// receipt survives here. Each consumer must independently admit and charge.
#[derive(Clone)]
struct CompletedPureBuild {
    inputs: CompletedPureBuildInputs,
    configuration: Arc<ComputedBuildConfig>,
}

impl CompletedPureBuild {
    /// Issue only after execution, Build value validation, and output settlement.
    /// Missing usage or evidence declines retention rather than inventing facts.
    fn from_completed(
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
    ) -> Option<Self> {
        let Some(usage) = configuration.evaluation_usage else {
            eprintln!(
                "Pure Build completion declined: entry {:?} has no measured usage",
                inputs.selected_entry
            );
            return None;
        };
        let Some(observations) = configuration.observation_summary.as_ref() else {
            eprintln!(
                "Pure Build completion declined: entry {:?} has no observations",
                inputs.selected_entry
            );
            return None;
        };
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
            || observations.activation() != inputs.activation
            || observations.captured_source_inventory() != inputs.captured_inventory
        {
            eprintln!(
                "Pure Build completion declined: entry {:?}, measurement={}, usage_valid={}, usage_filesystem_ops={}, host_observed={}, filesystem_ops={}, named_inputs={}, required_outputs={}, included_sources={}, empty_staging={}, generated_sources={}, entry={}, activation={}, capture={}",
                inputs.selected_entry,
                usage.measurement == inputs.measurement_identity,
                usage.validate().is_ok(),
                usage.filesystem_operation_attempts,
                observations.filesystem_host_observed(),
                observations.filesystem_operation_attempts().len(),
                observations.named_input_inventories().len(),
                observations.required_output_settlements().len(),
                observations.included_source_handoffs().len(),
                observations
                    .staged_output_tree()
                    .is_some_and(|tree| tree.entry_count() == 0),
                configuration.generated_sources.len(),
                configuration.selected_build_machine_symbol == Some(inputs.selected_entry),
                observations.activation() == inputs.activation,
                observations.captured_source_inventory() == inputs.captured_inventory,
            );
            return None;
        }
        Some(Self {
            inputs,
            configuration,
        })
    }

    /// Content match is not admission or a policy charge. The evaluator calls
    /// this only after fresh admission and charges before returning the result.
    fn matching_configuration(
        &self,
        inputs: &CompletedPureBuildInputs,
    ) -> Option<Arc<ComputedBuildConfig>> {
        self.inputs
            .matches(inputs)
            .then(|| Arc::clone(&self.configuration))
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
