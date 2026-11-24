/*
Bind the exact accepted invocation source and selected Build entry to the actual
host filesystem scope before authored code runs. The source-owned selection,
structural protocol rows and independently admitted host grants each retain
separate authority; an old checked result cannot replace any of them.
*/

use build_declarations::DependencyPurpose;
use declaration_scope::DeclarationSelectionAuthority;
use diagnostics::Diagnostic;
use package_compilation::BuildMachineFilesystemScope;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_compilation::build_execution::{SelectedBuildMachine, admit_build_machine};
use psi_authorization::selection_questions::SelectionAuthorities;
use evaluation_accounting::BuildEvaluationSponsor;
use symbols::SymbolHandle;

#[must_use = "an admitted build program must prepare its execution"]
pub(crate) struct AdmittedBuildProgram {
    pub(crate) prepared: std::sync::Arc<source_compiler::publication::AcceptedTerminalSource>,
    pub(crate) produced: std::sync::Arc<source_compiler::publication::ProducedTerminalProgram>,
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
    invocation: source_compiler::CheckedBuildInvocation,
    providers: super::provider_execution::BuildProviderPlans,
    machine_symbol: SymbolHandle,
    filesystem_scope: &BuildMachineFilesystemScope,
    evaluation_sponsor: Option<&BuildEvaluationSponsor>,
    execution_profile: Option<target::TargetProfile>,
    artifact_only: bool,
    source: package_compilation::BuildSourceSelection,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<AdmittedBuildProgram, Vec<Diagnostic>> {
    let prepared = invocation.accepted();
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
    let (machine, publication) =
        admit_build_machine(invocation, machine_symbol, &mut filesystem_scope)?;
    let (prepared, produced) = publication.publish()?;
    if !produced.view().pending_normal_completions().is_empty() {
        return Err(vec![Diagnostic::error(
            "Build execution requires checked normal completions for the whole program",
        )]);
    }
    let build_authority = providers.selection_scope(DependencyPurpose::Build);
    let product_authority = providers.selection_scope(DependencyPurpose::Product);
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
        prepared: std::sync::Arc::new(prepared),
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
