//! Final selected-comparison admission must count sealed boundary occurrences, not
//! builtin integer operations. Mutation checks retain the artifact and its
//! sealed checked scope while changing the native proposal's selected rows.

use compilation_report::{
    RetainedTerminalArtifact, TerminalIntegerComparisonOccurrenceProposal,
    TerminalNativeRealizationInputs, TerminalNativeRealizationProposal,
};
use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use std::{fs, path::PathBuf};

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-integer-comparison-publication-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create comparison project");
        fs::write(
            directory.join("main.omg"),
            r#"
boundary machine == Comparison::equal(left: i32, right: i32) -> bool;
data ComparisonProvider {}
machine ComparisonProvider::equal(left: i32, right: i32) -> bool
    satisfies Comparison::equal via Binding::CompilerIntrinsic;

data Main { left: i32; right: i32; first: bool; second: bool; builtin: bool; }
machine Main::main(&mut self) {
    self.first = !(self.left == self.right);
    self.second = self.right == self.left;
    self.builtin = 1u64 != 2u64;
}
"#,
        )
        .expect("write mixed selected and builtin comparisons");
        fs::write(
            directory.join("build.omg"),
            r#"
machine build(builder: &mut Build) {
    builder.application("integer-comparison-publication");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write comparison build");
        Self(directory)
    }

    fn compile(&self) -> RetainedTerminalArtifact {
        let identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32])
            .expect("fixture package identity");
        let packages = package_compilation::PackageCompilationInputs::new(
            identity,
            package_compilation::BuildDeclarationKind::Application,
            vec![package_compilation::PackageSourceBinding::new(
                identity,
                "integer-comparison-publication",
                self.0.clone(),
            )],
            Vec::new(),
        )
        .expect("exact source package owner");
        compile(
            CompileRequest::new(CompileOptions {
                root_path: self.0.join("main.omg"),
                build_dir: Some(self.0.join("build")),
                target_name: Some("windows_x86_64".into()),
            })
            .with_requested_product(RequestedCompileProduct::TerminalArtifact)
            .with_package_inputs(packages),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("mixed comparisons must publish: {diagnostics:#?}"))
        .into_retained_terminal_artifact()
        .expect("retained Terminal artifact")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rebuild(
    retained: &RetainedTerminalArtifact,
    occurrences: Vec<TerminalIntegerComparisonOccurrenceProposal>,
    mutate: impl FnOnce(&mut TerminalNativeRealizationInputs),
) -> Result<TerminalNativeRealizationProposal, &'static str> {
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let mut inputs = TerminalNativeRealizationInputs {
        target_profile: proposal.target_profile(),
        native_target: proposal.native_target(),
        subsystem: proposal.subsystem(),
        application_intent: proposal.application_intent(),
        application_identifier: proposal.application_identifier().cloned(),
        application_name: proposal.application_name().map(str::to_owned),
        post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
        program_entry: proposal.program_entry().clone(),
        checked_program_entry: proposal.checked_program_entry().clone(),
        selected_provider_plans: proposal.selected_provider_plans().clone(),
        external_binding_rows: proposal.external_binding_rows().to_vec(),
        package_terminal_authority_permissions: proposal
            .package_terminal_authority_permissions()
            .to_vec(),
        compiler_builtins: proposal.compiler_builtins().to_vec(),
        callback_occurrences: proposal.callback_occurrences().to_vec(),
        ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
        ieee_float_comparison_occurrences: proposal.ieee_float_comparison_occurrences().to_vec(),
        integer_comparison_occurrences: occurrences,
        boundary_application_demands: proposal.boundary_application_demands().clone(),
        boundary_application_realizations: proposal.boundary_application_realizations().clone(),
        checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
        behavior_exclusions: proposal.behavior_exclusions().clone(),
    };
    mutate(&mut inputs);
    TerminalNativeRealizationProposal::new(retained.artifact(), inputs)
}

#[test]
fn selected_comparison_publication_preserves_complete_custody_among_builtins() {
    let project = Project::new();
    let retained = project.compile();
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let occurrences = proposal.integer_comparison_occurrences();
    assert_eq!(
        occurrences.len(),
        2,
        "both i32 equalities select the same provider"
    );
    assert!(
        !occurrences[0].negated,
        "the explicit ! is an ordinary consumer, not selected !="
    );
    assert_eq!(
        occurrences[0].provider_plan_index,
        occurrences[1].provider_plan_index
    );
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode semantics");
    let comparisons = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block.operations.iter().filter_map(move |operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::IntegerEqual { .. }
                            | terminal_psi::OperationKind::IntegerLessThan { .. }
                            | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
                    )
                    .then_some((machine.id, operation.id))
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(
        comparisons.len() > occurrences.len(),
        "the product also contains builtin comparisons"
    );
    rebuild(&retained, occurrences.to_vec(), |_| {}).expect("unchanged proposal");
    assert_eq!(
        rebuild(&retained, Vec::new(), |_| {}).unwrap_err(),
        "Terminal proposal must retain every selected integer comparison occurrence exactly once"
    );
    assert_eq!(
        rebuild(
            &retained,
            vec![occurrences[0], occurrences[0]],
            |_| {}
        )
        .unwrap_err(),
        "Terminal proposal repeats an integer comparison occurrence"
    );

    let builtin = comparisons
        .iter()
        .find(|(_, operation)| {
            !occurrences
                .iter()
                .any(|row| row.terminal_operation == *operation)
        })
        .expect("builtin operation");
    let mut substituted = occurrences.to_vec();
    substituted[0].terminal_machine = builtin.0;
    substituted[0].terminal_operation = builtin.1;
    // Match the builtin's carrier so rejection cannot rely on a type mismatch:
    // it must fail the selected plan/application join.
    substituted[0].integer_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64");
    assert_eq!(
        rebuild(&retained, substituted, |_| {}).unwrap_err(),
        "Terminal integer comparison requires one exact boundary realization companion",
        "builtin cannot replace selected comparison even when its operand type matches"
    );
    assert_eq!(
        rebuild(&retained, vec![occurrences[0]], |_| {}).unwrap_err(),
        "Terminal proposal must retain every selected integer comparison occurrence exactly once"
    );

    // Caller-authored companion labels cannot hide a selected comparison from
    // the census retained in the sealed source-to-Terminal scope.
    assert_eq!(
        rebuild(&retained, Vec::new(), |inputs| {
            use boundary_applications::{
                BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
                BoundaryNominalIdentity, TerminalBoundaryApplicationRealizations,
            };
            let rows = inputs
                .boundary_application_realizations
                .rows()
                .iter()
                .map(|row| {
                    BoundaryApplicationRealizationCompanion::new(
                        row.terminal_operation(),
                        *row.selected_plan_digest(),
                        BoundaryApplicationRealization::NongenericCheckedBody {
                            realization_machine: BoundaryNominalIdentity::new("other::body".into())
                                .expect("nominal identity"),
                            realization_state: BoundaryNominalIdentity::new("other::entry".into())
                                .expect("nominal identity"),
                            realization_contract_commitment: [1; 32],
                        },
                    )
                    .expect("well-formed but substituted companion")
                })
                .collect();
            inputs.boundary_application_realizations =
                TerminalBoundaryApplicationRealizations::new(
                    &inputs.boundary_application_demands,
                    rows,
                )
                .expect("same demand coverage");
        })
        .unwrap_err(),
        "Terminal proposal must retain every selected integer comparison occurrence exactly once"
    );

    assert!(
        boundary_applications::TerminalBoundaryApplicationRealizations::new(
            proposal.boundary_application_demands(),
            Vec::new()
        )
        .is_err(),
        "dropping only the companions is incomplete"
    );
    assert_eq!(
        rebuild(&retained, Vec::new(), |inputs| {
            inputs.boundary_application_demands =
                boundary_applications::TerminalBoundaryApplicationDemands::new(
                    retained.artifact().manifest().semantic(),
                    Vec::new(),
                )
                .expect("empty demands");
            inputs.boundary_application_realizations =
                boundary_applications::TerminalBoundaryApplicationRealizations::new(
                    &inputs.boundary_application_demands,
                    Vec::new(),
                )
                .expect("empty companions");
        })
        .unwrap_err(),
        "Terminal native proposal source-free boundary demands differ from checked occurrence custody"
    );
}
