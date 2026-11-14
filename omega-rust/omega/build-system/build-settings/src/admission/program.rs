/*
Bind the exact accepted invocation source and selected Build entry to the actual
host filesystem scope before authored code runs. The source-owned selection,
structural protocol rows and independently admitted host grants each retain
separate authority; an old checked result cannot replace any of them.
*/

use build_declarations::DependencyPurpose;
use build_time_evaluation::BuildEvaluationSponsor;
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority;
use symbols::SymbolHandle;
use terminal_installation::selection_questions::SelectionAuthorities;

#[must_use = "an admitted build program must prepare its execution"]
pub(crate) struct AdmittedBuildProgram {
    pub(crate) produced: std::sync::Arc<psi_compiler::ProducedTerminalProgram>,
    pub(crate) providers: super::provider_execution::BuildProviderPlans,
    pub(crate) machine: SelectedBuildMachine,
    pub(crate) filesystem_scope: BuildMachineFilesystemScope,
    pub(crate) evaluation_sponsor: Option<BuildEvaluationSponsor>,
    pub(crate) execution_profile: Option<target::TargetProfile>,
    pub(crate) artifact_only: bool, // Validated `Build::artifact_only(builder)` modifier: publishes retained outputs only, forbids executable roots/providers, and requires at least one required output.
    pub(crate) default_source: bool,
    pub(crate) default_sources:
        Option<std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
}

impl AdmittedBuildProgram {
    pub fn restricted_build_requests(&self) -> Vec<RestrictedBuildRequest> {
        self.filesystem_scope.restricted_build_requests(
            self.evaluation_sponsor.as_ref(),
            self.artifact_only,
            &self.machine.execution_mode,
        )
    }
}

pub(crate) fn admit_build_program(
    prepared: std::sync::Arc<psi_compiler::AcceptedTerminalSource>,
    providers: super::provider_execution::BuildProviderPlans,
    machine_symbol: SymbolHandle,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    execution_profile: Option<target::TargetProfile>,
    artifact_only: bool,
    source: package_compilation::BuildSourceSelection,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let typed = prepared.source();
    if !typed
        .symbols
        .symbol_source_span(machine_symbol)
        .is_some_and(|span| span.source_id == source.source_id())
    {
        return Err(vec![Diagnostic::error(
            "Build entry differs from its source-owned selection",
        )]);
    }
    let mut filesystem_scope = filesystem_scope.clone();
    let machine = admit_build_machine(&prepared, machine_symbol, &mut filesystem_scope)?;
    // Produce one invocation. Its companion and the code later passed to the
    // host retain the same execution identity; preparing again would issue a
    // different invocation rather than replay this admission.
    let produced = prepared.produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)?;
    let build_authority = package_inputs
        .map(|inputs| inputs.declaration_selection_authority(DependencyPurpose::Build));
    let product_authority = package_inputs
        .map(|inputs| inputs.declaration_selection_authority(DependencyPurpose::Product));
    let selections = produced.source_selections().ok_or_else(|| {
        vec![Diagnostic::error(
            "Build invocation has no source-owned declaration-selection companion",
        )]
    })?;
    selections
        .admit(
            produced.view(),
            &SelectionAuthorities {
                build: build_authority
                    .as_ref()
                    .map(|authority| authority as &dyn DeclarationSelectionAuthority),
                product: product_authority
                    .as_ref()
                    .map(|authority| authority as &dyn DeclarationSelectionAuthority),
            },
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "Build declaration-selection admission: {reason}"
            ))]
        })?;
    Ok(AdmittedBuildProgram {
        produced: std::sync::Arc::new(produced),
        providers,
        machine,
        filesystem_scope,
        evaluation_sponsor: evaluation_sponsor.cloned(),
        execution_profile,
        artifact_only,
        default_source: source.is_default(),
        default_sources: package_inputs
            .filter(|_| source.is_default())
            .map(package_compilation::PackageCompilationInputs::source_inputs),
    })
}
