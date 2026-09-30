//! Private completed Pure Build ownership inside the execution issuer.
//!
//! Keeping insertion here matters: a public product constructor could pair an
//! old configuration with a new owner without executing it. Only this crate's
//! successful admission/value/output protocol may publish a completed value;
//! public callers can share its owner, but cannot extract or inject carriers.

use crate::ComputedBuildConfig;
use std::sync::{Arc, Mutex};

/// Exact content and custody of an admitted Pure Build. Product selections
/// remain fresh admission inputs; only their complete checked Build program
/// can establish that two consumers observe the same computation.
#[derive(Clone)]
pub(crate) struct CompletedPureBuildInputs {
    checked: build_time_evaluation::CheckedBuildProgram,
    selected_entry: symbols::SymbolHandle,
    initial_build: build_time_evaluation::BuildTimeValue,
    source: package_compilation::SourceAssemblyInputs,
    dependencies: Vec<(
        build_declarations::DependencyPurpose,
        semantic_vocabulary::PackageKeyIdentity,
        build_declarations::DependencyPurpose,
        Option<target::TargetProfile>,
        package_compilation::PackageDependencyClosure,
        package_compilation::PackageSourceConsumptionCommitment,
    )>,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
    activation: package_compilation::BuildActivation,
    captured_inventory: Option<package_compilation::BuildCapturedSourceInventory>,
    artifact_only: bool,
}

impl CompletedPureBuildInputs {
    /// Called only after the current program's semantic, package, operational,
    /// and authority admission. Keeping this construction separate from the
    /// completed carrier prevents an admitted-but-unexecuted value from reuse.
    #[doc(hidden)]
    pub(crate) fn from_admitted(
        checked: build_time_evaluation::CheckedBuildProgram,
        selected_entry: symbols::SymbolHandle,
        initial_build: build_time_evaluation::BuildTimeValue,
        packages: &package_compilation::PackageCompilationInputs,
        snapshot: Option<&package_compilation::BuildSnapshotRequest>,
        scope: &package_compilation::BuildMachineFilesystemScope,
        artifact_only: bool,
    ) -> Option<Self> {
        if artifact_only
            || !scope.required_outputs().is_empty()
            || snapshot.is_some_and(|snapshot| {
                !snapshot.inputs().is_empty()
                    || !snapshot.required_outputs().is_empty()
                    || snapshot.dependency_inputs().next().is_some()
            })
        {
            return None;
        }
        let activation = scope.activation();
        Some(Self {
            checked,
            selected_entry,
            initial_build,
            source: package_compilation::SourceAssemblyInputs::new(
                Some(packages),
                activation.build_execution_profile(),
            ),
            dependencies: packages
                .dependency_generated_source_instances()
                .map(|(scope, bundle)| {
                    (
                        scope,
                        bundle.package(),
                        bundle.purpose(),
                        bundle.build_execution_profile(),
                        bundle.dependency_closure().clone(),
                        bundle.source_consumption_commitment(),
                    )
                })
                .collect(),
            snapshot: snapshot.cloned(),
            activation,
            captured_inventory: scope.captured_source_inventory(),
            artifact_only,
        })
    }

    fn matches(&self, other: &Self) -> bool {
        self.checked == other.checked
            && self.selected_entry == other.selected_entry
            && same_initial_value(&self.initial_build, &other.initial_build)
            && self.source == other.source
            && self.dependencies == other.dependencies
            && self.snapshot == other.snapshot
            && self.activation == other.activation
            && self.captured_inventory == other.captured_inventory
            && self.artifact_only == other.artifact_only
    }
}

/// One compilation's private completed-Build store. Clones are explicit
/// consumers of this owner; a fresh owner cannot import another's carriers.
/// No global registry, public carrier constructor, decoder, or carrier export
/// turns equal content into cross-compilation reuse.
/// ```compile_fail
/// let _ = build_evaluation::PureBuildCompilation::retain_completed;
/// ```
#[derive(Clone, Default)]
pub struct PureBuildCompilation {
    completed: Arc<Mutex<Vec<CompletedPureBuild>>>,
}

impl PureBuildCompilation {
    #[doc(hidden)]
    pub(crate) fn matching_configuration(
        &self,
        inputs: &CompletedPureBuildInputs,
    ) -> Result<Option<Arc<ComputedBuildConfig>>, Vec<diagnostics::Diagnostic>> {
        let completed = self.completed.lock().map_err(|_| {
            vec![diagnostics::Diagnostic::error(
                "completed Pure Build owner is unavailable",
            )]
        })?;
        Ok(completed
            .iter()
            .find_map(|completed| completed.matching_configuration(inputs)))
    }

    /// Evaluation publishes here only after successful value/output settlement.
    /// Consumers cannot obtain or insert an already-issued carrier from a
    /// different compilation; only the completed configuration is observable.
    #[doc(hidden)]
    pub(crate) fn retain_completed(
        &self,
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if let Some(completed) = CompletedPureBuild::from_completed(inputs, configuration) {
            self.completed
                .lock()
                .map_err(|_| {
                    vec![diagnostics::Diagnostic::error(
                        "completed Pure Build owner is unavailable",
                    )]
                })?
                .push(completed);
        }
        Ok(())
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
    #[doc(hidden)]
    fn from_completed(
        inputs: CompletedPureBuildInputs,
        configuration: Arc<ComputedBuildConfig>,
    ) -> Option<Self> {
        let usage = configuration.evaluation_usage?;
        let observations = configuration.observation_summary.as_ref()?;
        if usage.validate().is_err()
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
            return None;
        }
        Some(Self {
            inputs,
            configuration,
        })
    }

    /// Content match is not admission or a policy charge. The evaluator calls
    /// this only after fresh admission and charges before returning the result.
    #[doc(hidden)]
    fn matching_configuration(
        &self,
        inputs: &CompletedPureBuildInputs,
    ) -> Option<Arc<ComputedBuildConfig>> {
        self.inputs
            .matches(inputs)
            .then(|| Arc::clone(&self.configuration))
    }
}

fn same_initial_value(
    left: &build_time_evaluation::BuildTimeValue,
    right: &build_time_evaluation::BuildTimeValue,
) -> bool {
    use build_time_evaluation::BuildTimeValue;
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

#[cfg(test)]
mod pure_tests {
    use super::*;

    fn inputs(
        purpose: build_declarations::DependencyPurpose,
        package_byte: u8,
    ) -> CompletedPureBuildInputs {
        let directory = std::env::current_dir().unwrap().canonicalize().unwrap();
        let package =
            semantic_vocabulary::PackageKeyIdentity::from_digest([package_byte; 32]).unwrap();
        let packages = package_compilation::PackageCompilationInputs::new_package(
            package,
            vec![package_compilation::PackageSourceBinding::new(
                package,
                "fixture",
                directory.clone(),
            )],
            Vec::new(),
        )
        .unwrap()
        .with_compilation_purpose(purpose)
        .unwrap();
        let scope = package_compilation::BuildMachineFilesystemScope::for_package_root(
            directory.clone(),
            directory.join("unused-output"),
            None,
            None,
        )
        .with_package_activation(package, package_compilation::BuildDeclarationKind::Package)
        .with_execution_profile(Some(target::TargetProfile::LinuxX64));
        CompletedPureBuildInputs::from_admitted(
            build_time_evaluation::PreparedBuildMachineProgram::from_checked(Default::default())
                .checked_program()
                .unwrap(),
            symbols::SymbolHandle::from_arena_index(1),
            build_time_evaluation::BuildTimeValue::Float(0.0),
            &packages,
            None,
            &scope,
            false,
        )
        .unwrap()
    }

    #[test]
    fn completed_pure_identity_compares_facts_entry_arguments_profile_and_purpose() {
        let baseline = inputs(build_declarations::DependencyPurpose::Product, 7);
        assert!(baseline.matches(&baseline.clone()));
        let mut changed = baseline.clone();
        changed.selected_entry = symbols::SymbolHandle::from_arena_index(2);
        assert!(!baseline.matches(&changed));
        let mut changed = baseline.clone();
        changed.initial_build = build_time_evaluation::BuildTimeValue::Float(-0.0);
        assert!(
            !baseline.matches(&changed),
            "arguments compare exact float bits"
        );
        assert!(!baseline.matches(&inputs(build_declarations::DependencyPurpose::Build, 7)));
        assert!(!baseline.matches(&inputs(build_declarations::DependencyPurpose::Product, 8)));
        let mut changed = baseline.clone();
        changed.source = package_compilation::SourceAssemblyInputs::new(
            None,
            Some(target::TargetProfile::MacosArm64),
        );
        assert!(!baseline.matches(&changed));
        let mut changed = baseline.clone();
        changed.snapshot = Some(package_compilation::BuildSnapshotRequest::new([]));
        assert!(!baseline.matches(&changed), "capture mode is exact");
        let mut changed = baseline.clone();
        changed.artifact_only = true;
        assert!(!baseline.matches(&changed));
    }

    #[test]
    fn completed_pure_owner_cannot_import_another_compilations_carrier() {
        let inputs = inputs(build_declarations::DependencyPurpose::Product, 7);
        // This owner test builds private stored state directly. Public callers
        // cannot move this carrier into another owner or supply it in a request.
        let configuration = Arc::new(ComputedBuildConfig {
            config: Default::default(),
            optimization_report_request: optimization_core::OptimizationReportRequest::Suppressed,
            evaluation_usage: None,
            observation_summary: None,
            selected_build_machine_symbol: Some(inputs.selected_entry),
            generated_sources: Vec::new(),
        });
        let first = PureBuildCompilation::default();
        first.completed.lock().unwrap().push(CompletedPureBuild {
            inputs: inputs.clone(),
            configuration: Arc::clone(&configuration),
        });
        let same = first.clone();
        let foreign = PureBuildCompilation::default();
        assert!(Arc::ptr_eq(
            &configuration,
            &same.matching_configuration(&inputs).unwrap().unwrap()
        ));
        assert!(foreign.matching_configuration(&inputs).unwrap().is_none());
        assert!(
            CompletedPureBuild::from_completed(inputs, configuration).is_none(),
            "missing telemetry never mints a carrier"
        );
    }
}
