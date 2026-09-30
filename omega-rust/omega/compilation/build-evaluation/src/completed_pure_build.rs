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
    dependencies: Vec<CompletedPureBuildDependency>,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
    activation: package_compilation::BuildActivation,
    captured_inventory: Option<package_compilation::BuildCapturedSourceInventory>,
    artifact_only: bool,
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
    // Append-only: the length is the publication generation. Entries never
    // change or disappear while any consumer can hold this owner.
    completed: Arc<Mutex<Vec<Arc<CompletedPureBuild>>>>,
}

impl PureBuildCompilation {
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
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        let package =
            semantic_vocabulary::PackageKeyIdentity::from_digest([package_byte; 32]).unwrap();
        let dependency = semantic_vocabulary::PackageKeyIdentity::from_digest([42; 32]).unwrap();
        let packages = package_compilation::PackageCompilationInputs::new_package(
            package,
            vec![
                package_compilation::PackageSourceBinding::new(
                    package,
                    "fixture",
                    directory.clone(),
                ),
                package_compilation::PackageSourceBinding::new(
                    dependency,
                    "dependency",
                    directory.parent().unwrap().join("products"),
                ),
            ],
            vec![package_compilation::PackageDependencyBinding::new(
                package,
                "dependency",
                dependency,
            )],
        )
        .unwrap()
        .with_compilation_purpose(purpose)
        .unwrap();
        let bundle = package_compilation::PackageGeneratedSourceBundle::from_checked(
            dependency,
            purpose,
            target::TargetProfile::LinuxX64,
            Some(target::TargetProfile::LinuxX64),
            packages.dependency_closure_for(dependency),
            package_compilation::PackageSourceConsumptionCommitment::from_retained_digest([3; 32]),
            Vec::new(),
        );
        let packages = packages
            .with_complete_dependency_generated_sources(vec![bundle])
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
    fn completed_pure_identity_compares_checked_program_and_check_facts_directly() {
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            Default::default(),
            &typed_trees_to_checked_trees::CheckingRequest::preliminary(),
        )
        .unwrap();
        let mut baseline = inputs(build_declarations::DependencyPurpose::Product, 7);
        baseline.checked =
            build_time_evaluation::PreparedBuildMachineProgram::from_checked(checked.clone())
                .checked_program()
                .unwrap();
        let mut changed = baseline.clone();
        changed.checked =
            build_time_evaluation::PreparedBuildMachineProgram::from_checked(checked.clone())
                .checked_program()
                .unwrap();
        assert!(
            baseline.matches(&changed),
            "independent equal checked content matches"
        );

        let mut changed_program = checked.clone();
        let mut symbols = symbols::SymbolTableBuilder::new();
        symbols.insert_root(
            symbols::SymbolKind::Module,
            symbols::SymbolNameRef::Static("changed"),
        );
        changed_program.typed.symbols = symbols.finish();
        changed.checked =
            build_time_evaluation::PreparedBuildMachineProgram::from_checked(changed_program)
                .checked_program()
                .unwrap();
        assert!(
            !baseline.matches(&changed),
            "checked program content participates"
        );

        let mut changed_facts = checked.clone();
        changed_facts
            .facts
            .service_reaches
            .dependency_parameters
            .insert(symbols::SymbolHandle::from_arena_index(9));
        assert_eq!(checked.typed, changed_facts.typed);
        changed.checked =
            build_time_evaluation::PreparedBuildMachineProgram::from_checked(changed_facts)
                .checked_program()
                .unwrap();
        assert!(
            !baseline.matches(&changed),
            "CheckFacts participate with equal typed content"
        );
    }

    #[test]
    fn completed_pure_identity_compares_every_dependency_coordinate() {
        use build_declarations::DependencyPurpose::{Build, Product};
        let baseline = inputs(Build, 7);
        assert_eq!(baseline.dependencies.len(), 1);
        let dependency = &baseline.dependencies[0];
        assert_eq!(dependency.consumer_scope, Product);
        assert_eq!(dependency.producer_purpose, Build);
        assert!(baseline.matches(&inputs(Build, 7)));

        type Mutation = fn(&mut CompletedPureBuildDependency);
        let mutations: &[(&str, Mutation)] = &[
            ("consumer scope", |dependency| {
                dependency.consumer_scope = Build
            }),
            ("package", |dependency| {
                dependency.package =
                    semantic_vocabulary::PackageKeyIdentity::from_digest([43; 32]).unwrap();
            }),
            ("producer purpose", |dependency| {
                dependency.producer_purpose = Product
            }),
            ("execution profile", |dependency| {
                dependency.build_execution_profile = Some(target::TargetProfile::MacosArm64)
            }),
            ("absent execution profile", |dependency| {
                dependency.build_execution_profile = None
            }),
            ("closure", |dependency| {
                dependency.dependency_closure =
                    package_compilation::PackageDependencyClosure::from_canonical_parts(
                        dependency.package,
                        package_compilation::BuildDeclarationKind::Application,
                        vec![dependency.package],
                        Vec::new(),
                    )
                    .unwrap();
            }),
            ("source commitment", |dependency| {
                dependency.source_consumption_commitment =
                    package_compilation::PackageSourceConsumptionCommitment::from_retained_digest(
                        [4; 32],
                    );
            }),
        ];
        for (coordinate, mutate) in mutations {
            let mut changed = baseline.clone();
            mutate(&mut changed.dependencies[0]);
            assert!(
                !baseline.matches(&changed),
                "changed {coordinate} must miss"
            );
        }
        let mut changed = baseline.clone();
        changed.dependencies.clear();
        assert!(
            !baseline.matches(&changed),
            "dependency membership participates"
        );
        let mut changed = baseline.clone();
        changed.dependencies.push(changed.dependencies[0].clone());
        assert!(
            !baseline.matches(&changed),
            "dependency multiplicity participates"
        );
    }

    fn captured_inventory(
        commitment: [u8; 32],
        file_bytes: usize,
        extra_directory: bool,
    ) -> package_compilation::BuildCapturedSourceInventory {
        use checked_interpreter::{
            CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
            CanonicalFilesystemMetadataRowKind,
        };
        let mut rows = vec![
            CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                CanonicalFilesystemMetadataRowKind::Directory,
            ),
            CanonicalFilesystemMetadataRow::new(
                b"input.omg".to_vec(),
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: file_bytes as u64,
                },
            ),
        ];
        let mut entries = vec![(
            b"input.omg".to_vec(),
            build_output::CapturedSourceEntry::file(vec![b' '; file_bytes], false),
        )];
        if extra_directory {
            rows.push(CanonicalFilesystemMetadataRow::new(
                b"extra".to_vec(),
                CanonicalFilesystemMetadataRowKind::Directory,
            ));
            entries.push((
                b"extra".to_vec(),
                build_output::CapturedSourceEntry::Directory,
            ));
        }
        let capture = build_output::CapturedBuildSourceInput::from_capture_rows(
            CanonicalFilesystemMetadataIndex::version_1(commitment, rows).unwrap(),
            entries,
        )
        .unwrap();
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        package_compilation::BuildMachineFilesystemScope::for_package_root(
            directory.join("input"),
            directory.join("output"),
            None,
            None,
        )
        .with_captured_source_input(capture, directory.join("snapshot"))
        .unwrap()
        .captured_source_inventory()
        .unwrap()
    }

    #[test]
    fn completed_pure_identity_compares_captured_inventory_directly() {
        let mut baseline = inputs(build_declarations::DependencyPurpose::Product, 7);
        baseline.captured_inventory = Some(captured_inventory([5; 32], 7, false));
        assert!(baseline.matches(&baseline.clone()));
        for (coordinate, inventory) in [
            ("capture absence", None),
            ("commitment", Some(captured_inventory([6; 32], 7, false))),
            ("file bytes", Some(captured_inventory([5; 32], 8, false))),
            ("entry count", Some(captured_inventory([5; 32], 7, true))),
        ] {
            let mut changed = baseline.clone();
            changed.captured_inventory = inventory;
            assert!(
                !baseline.matches(&changed),
                "changed {coordinate} must miss"
            );
        }
    }

    fn completed_configuration(inputs: &CompletedPureBuildInputs) -> Arc<ComputedBuildConfig> {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
        let scope = package_compilation::BuildMachineFilesystemScope::for_package_root(
            directory.clone(),
            directory.join("unused-output"),
            None,
            None,
        )
        .with_package_activation(
            inputs.activation.root_package_identity().unwrap(),
            inputs.activation.root_role().unwrap(),
        )
        .with_execution_profile(inputs.activation.build_execution_profile());
        let observations = package_compilation::CapturedBuildObservations::capture(
            &Default::default(),
            &scope,
            "build",
        )
        .unwrap()
        .settle(false, "build")
        .unwrap();
        // Unit storage tests supply valid, effect-free accounting. The execution
        // and admission tests exercise production-issued telemetry separately.
        let usage = package_compilation::BuildEvaluationUsage {
            usage_schema_version: checked_interpreter::CURRENT_EVALUATION_USAGE_SCHEMA
                .schema_version(),
            step_schedule_marker: checked_interpreter::CURRENT_EVALUATION_STEP_SCHEDULE.marker(),
            invocation_fuel_ceiling: 100,
            sponsor_schema_version: None,
            session_fuel_ceiling: None,
            session_build_log_byte_ceiling: None,
            session_filesystem_attempt_ceiling: None,
            session_live_filesystem_handle_ceiling: None,
            session_live_cell_ceiling: None,
            session_live_text_byte_ceiling: None,
            session_result_cell_ceiling: None,
            session_result_text_byte_ceiling: None,
            session_peak_live_filesystem_handles: 0,
            session_peak_live_cells: 0,
            session_peak_live_text_bytes: 0,
            fuel_units: 1,
            build_log_bytes: 0,
            filesystem_operation_attempts: 0,
            peak_live_cells: 0,
            peak_live_text_bytes: 0,
            result_cells: 0,
            result_text_bytes: 0,
        };
        Arc::new(ComputedBuildConfig {
            config: Default::default(),
            optimization_report_request: optimization_core::OptimizationReportRequest::Suppressed,
            evaluation_usage: Some(usage),
            observation_summary: Some(observations),
            selected_build_machine_symbol: Some(inputs.selected_entry),
            generated_sources: Vec::new(),
        })
    }

    #[test]
    fn completed_pure_retention_deduplicates_identical_inputs() {
        let inputs = inputs(build_declarations::DependencyPurpose::Product, 7);
        let first = completed_configuration(&inputs);
        let second = completed_configuration(&inputs);
        let owner = PureBuildCompilation::default();
        owner
            .retain_completed(inputs.clone(), Arc::clone(&first))
            .unwrap();
        owner.retain_completed(inputs.clone(), second).unwrap();
        assert_eq!(owner.completed.lock().unwrap().len(), 1);
        assert!(Arc::ptr_eq(
            &first,
            &owner.matching_configuration(&inputs).unwrap().unwrap()
        ));
    }

    #[test]
    fn completed_pure_retention_preserves_distinct_inputs() {
        let first_inputs = inputs(build_declarations::DependencyPurpose::Product, 7);
        let mut second_inputs = first_inputs.clone();
        second_inputs.initial_build = build_time_evaluation::BuildTimeValue::Float(-0.0);
        let first = completed_configuration(&first_inputs);
        let second = completed_configuration(&second_inputs);
        let owner = PureBuildCompilation::default();
        owner
            .retain_completed(first_inputs.clone(), Arc::clone(&first))
            .unwrap();
        owner
            .retain_completed(second_inputs.clone(), Arc::clone(&second))
            .unwrap();
        assert_eq!(owner.completed.lock().unwrap().len(), 2);
        assert!(Arc::ptr_eq(
            &first,
            &owner
                .matching_configuration(&first_inputs)
                .unwrap()
                .unwrap()
        ));
        assert!(Arc::ptr_eq(
            &second,
            &owner
                .matching_configuration(&second_inputs)
                .unwrap()
                .unwrap()
        ));
    }

    #[test]
    fn completed_pure_retention_deduplicates_racing_identical_writers() {
        let inputs = inputs(build_declarations::DependencyPurpose::Product, 7);
        let configuration = completed_configuration(&inputs);
        let owner = PureBuildCompilation::default();
        // A distinct entry makes each contender traverse the unlocked comparison
        // path before appending. The barrier races publication without sleeps.
        let mut distinct = inputs.clone();
        distinct.initial_build = build_time_evaluation::BuildTimeValue::Float(-0.0);
        owner
            .retain_completed(distinct, Arc::clone(&configuration))
            .unwrap();
        let start = std::sync::Barrier::new(8);
        std::thread::scope(|threads| {
            for _ in 0..8 {
                threads.spawn(|| {
                    start.wait();
                    owner
                        .retain_completed(inputs.clone(), Arc::clone(&configuration))
                        .unwrap();
                });
            }
        });
        assert_eq!(owner.completed.lock().unwrap().len(), 2);
        assert!(Arc::ptr_eq(
            &configuration,
            &owner.matching_configuration(&inputs).unwrap().unwrap()
        ));
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
        first
            .completed
            .lock()
            .unwrap()
            .push(Arc::new(CompletedPureBuild {
                inputs: inputs.clone(),
                configuration: Arc::clone(&configuration),
            }));
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
