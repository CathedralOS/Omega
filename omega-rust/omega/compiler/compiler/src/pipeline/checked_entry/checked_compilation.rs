//! Checked compilation and the source custody sealed against it.

use super::build_continuation::BuildSourceCustody;
use super::execution_settlement::CheckedExecution;
use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::timing::CompileTimings;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::sync::Arc;

#[cfg(test)]
mod custody_tests;

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
///
/// Clones share program storage until mutation. Review instantiation currently
/// uses mutable clones as scratch; this does not recheck their evidence.
/// Consumers must still perform their independent reconstruction.
#[derive(Debug, Clone)]
pub struct CheckedCompilation {
    execution: CheckedExecution,
    sources: CheckedSourceCustody,
    timings: CompileTimings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedSourceCustody {
    source_file_count: usize,
    package_subject: Option<package_compilation::PackageCompilationSubject>,
    base_source_consumption_commitment: Option<crate::pipeline::PackageSourceConsumptionCommitment>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
    generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    own_generated_sources: Vec<build_output::PackageGeneratedSource>,
}

// Measurements are observations, not checked semantic identity. Each retained
// semantic owner derives equality so newly retained fields participate too.
impl PartialEq for CheckedCompilation {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.sources == other.sources
    }
}

impl Eq for CheckedCompilation {}

impl CheckedCompilation {
    pub(super) fn seal(
        execution: CheckedExecution,
        sources: BuildSourceCustody,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let package_subject = package_inputs
            .map(|inputs| {
                package_compilation::derive_package_compilation_subject(
                    &execution.settled.program,
                    inputs,
                    &sources.generated_source_custody,
                )
            })
            .transpose()?;
        let exact_toolchain_sources =
            package_compilation::toolchain_source_identities(&execution.settled.program)?;
        if package_subject.is_some() {
            package_compilation::verify_current_files(
                &execution.settled.program,
                &sources.generated_source_custody,
            )?;
        }
        if let Some(package_inputs) = package_inputs {
            package_inputs.validate_canonical_source_metadata()?;
        }
        Ok(Self {
            execution,
            sources: CheckedSourceCustody {
                source_file_count: sources.source_file_count,
                package_subject,
                base_source_consumption_commitment: sources.base_source_consumption_commitment,
                exact_toolchain_sources,
                generated_source_custody: sources.generated_source_custody,
                own_generated_sources: sources.own_generated_sources,
            },
            timings,
        })
    }
}

impl CheckedCompilation {
    /// Restore only exact selected-dispatch edits after checking their settled
    /// operand/type graphs. This is not a pre-specialization or source-text view.
    pub fn pre_selected_dispatch_source_trees(
        &self,
    ) -> Result<std::borrow::Cow<'_, typed_trees::TypedTrees>, Vec<Diagnostic>> {
        self.execution
            .settled
            .dispatch_source_edits
            .source_trees(&self.execution.settled.program.typed)
    }

    /// Canonical boundary calls already live in this checked program.
    pub(crate) fn terminal_production_trees(&self) -> &CheckedTrees {
        &self.execution.settled.program
    }

    /// Exact physical/generated source count consumed by this checked run.
    pub const fn source_file_count(&self) -> usize {
        self.sources.source_file_count
    }

    /// Authored hosted presentation intent; never inferred from a PE word.
    pub const fn application_intent(&self) -> Option<build_evaluation::HostedApplicationIntent> {
        self.execution.application_intent
    }

    /// Exact image subsystem selected by the owning build configuration.
    pub const fn subsystem(&self) -> u16 {
        self.execution.subsystem
    }

    /// The two independent optional proof-product requests retained from the
    /// authoritative build machine. Off by default; they never change checking.
    pub const fn pcc_requests(&self) -> build_evaluation::PccRequests {
        self.execution.pcc_requests
    }

    /// Reconciled root package identity for package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match &self.sources.package_subject {
            Some(subject) => Some(subject.root()),
            None => None,
        }
    }

    /// Exact source-path-free dependency closure consumed by package-aware
    /// compilation. Standalone compilation has no package closure.
    pub const fn dependency_closure(&self) -> Option<&crate::pipeline::PackageDependencyClosure> {
        match &self.sources.package_subject {
            Some(subject) => Some(subject.dependency_closure()),
            None => None,
        }
    }

    /// Canonical commitment to the exact source bytes consumed by this
    /// package-aware frontend run. Standalone compilation has no package-
    /// custody commitment.
    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<crate::pipeline::PackageSourceConsumptionCommitment> {
        match &self.sources.package_subject {
            Some(subject) => Some(subject.source_consumption_commitment()),
            None => None,
        }
    }

    /// Canonical commitment to the exact package-aware source closure admitted
    /// before this activation executed its selected build machine. Own
    /// generated source is absent; imported dependency-generated bundles are
    /// already part of this base.
    pub const fn base_source_consumption_commitment(
        &self,
    ) -> Option<crate::pipeline::PackageSourceConsumptionCommitment> {
        self.sources.base_source_consumption_commitment
    }

    /// Canonical package/source subject derived from the final checked source
    /// closure. Standalone compilation has no package subject.
    pub const fn package_compilation_subject(
        &self,
    ) -> Option<&package_compilation::PackageCompilationSubject> {
        self.sources.package_subject.as_ref()
    }

    #[doc(hidden)]
    pub fn resolved_semantic_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
    ) -> Option<&selected_dispatch::ResolvedAcceptedSemanticBinding> {
        self.execution
            .settled
            .resolved_semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }

    /// Exact consumer-policy semantic bindings that were resolved and
    /// consumed by this checked package compilation. Unconsumed or stale input
    /// bindings reject before a compilation can expose this set.
    #[doc(hidden)]
    pub fn resolved_semantic_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &package_compilation::AcceptedSemanticBinding> {
        self.execution
            .settled
            .resolved_semantic_bindings
            .iter()
            .map(selected_dispatch::ResolvedAcceptedSemanticBinding::accepted)
    }

    /// Construct a non-authoritative review candidate for one exact package-
    /// owned requirement-only boundary. Readable role nomination happens
    /// outside the compiler; this contributes only checked owner, nominal, and
    /// normalized-schema coordinates for a required bound replay.
    #[doc(hidden)]
    pub fn candidate_service_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<package_compilation::AcceptedSemanticBinding, Diagnostic> {
        let matches = self
            .execution
            .settled
            .program
            .typed
            .traits()
            .iter()
            .filter(|definition| {
                definition.is_boundary
                    && self
                        .execution
                        .settled
                        .program
                        .typed
                        .symbols
                        .symbol_package_identity(definition.symbol)
                        == Some(package)
                    && self
                        .execution
                        .settled
                        .program
                        .typed
                        .symbols
                        .display_path(definition.symbol, "::")
                        == declaration_path
            })
            .filter_map(|definition| {
                provider_planning::service_schema::from_typed(
                    &self.execution.settled.program.typed,
                    definition,
                )
            })
            .collect::<Vec<_>>();
        let [schema] = matches.as_slice() else {
            return Err(Diagnostic::error(format!(
                "semantic-binding candidate {:?} resolved to {} exact package-owned boundary schemas instead of one",
                role,
                matches.len(),
            )));
        };
        package_compilation::AcceptedSemanticBinding::new_service(
            role,
            package,
            declaration_path,
            package_compilation::accepted_service_schema_digest(role, schema),
        )
        .map_err(Diagnostic::error)
    }

    /// Compiler-validated exact source owners used while projecting
    /// source-free structural and nominal identity. Source IDs are private
    /// join coordinates and never enter canonical product or review bytes.
    #[doc(hidden)]
    pub fn exact_toolchain_sources(&self) -> &[(source::SourceId, [u8; 32])] {
        &self.sources.exact_toolchain_sources
    }

    /// Re-read every ordinary physical source path and require it to equal the
    /// bytes retained by the frontend. Generated sources are instead checked
    /// against their compiler-retained staged-output custody. Resolver
    /// orchestration calls this around its own whole-snapshot verification;
    /// hostile same-user races still require an OS isolation boundary.
    pub fn verify_current_source_consumption(&self) -> Result<(), Vec<Diagnostic>> {
        package_compilation::verify_current_files(
            &self.execution.settled.program,
            &self.sources.generated_source_custody,
        )
    }

    /// Retain this package's own explicit generated-source handoffs as one
    /// compiler-issued bundle suitable for a later dependency compilation.
    /// The bundle is not admission and carries no filesystem authority.
    pub fn package_generated_source_bundle(
        &self,
    ) -> Result<crate::pipeline::PackageGeneratedSourceBundle, &'static str> {
        let package = self
            .package_identity()
            .ok_or("generated-source bundles require package-aware compilation")?;
        let target = self
            .execution
            .selected_target_profile
            .ok_or("generated-source bundles require one selected target")?;
        let dependency_closure = self
            .dependency_closure()
            .cloned()
            .ok_or("generated-source bundles require one dependency closure")?;
        let source_consumption_commitment = self
            .source_consumption_commitment()
            .ok_or("generated-source bundles require source-consumption custody")?;
        Ok(crate::pipeline::PackageGeneratedSourceBundle::from_checked(
            package,
            target,
            dependency_closure,
            source_consumption_commitment,
            self.sources.own_generated_sources.clone(),
        ))
    }

    /// Exact native target selected for this checked compilation. Semantic-only
    /// checking has no selected target and therefore cannot be staged.
    pub const fn selected_native_target(&self) -> Option<target::NativeTarget> {
        self.execution.selected_native_target
    }

    /// Exact deployment-policy target selected for this checked compilation.
    /// This remains distinct when profiles share a native ABI, notably
    /// `windows_x64` and `uefi_x64`.
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.execution.selected_target_profile
    }

    /// Exact build-selected x86 scalar FMA admission for this compilation.
    /// Absence preserves the generic SSE2 baseline. The retained provider
    /// carries the canonical semantic cancellation-vector admission but does
    /// not claim native differential execution.
    pub const fn x86_scalar_fma_provider(&self) -> Option<target::AdmittedX86ScalarFmaProvider> {
        self.execution.x86_scalar_fma_provider
    }

    /// Exact source-selected nearest-FMA plans joined to this compilation's
    /// admitted x86 provider. Empty means the source demanded no x86 FMA;
    /// admission alone never fabricates source demand or execution evidence.
    pub fn x86_scalar_fma_plan_associations(
        &self,
    ) -> &[crate::pipeline::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation] {
        &self.execution.x86_scalar_fma_plan_associations
    }

    /// Exact target-owned `ProgramEntry` choice retained by Omega, if this
    /// checked-only compilation had one. Pure semantic checking is entry-
    /// agnostic; an execution caller must not infer a machine from its name.
    pub fn selected_program_entry_machine(&self) -> Option<&str> {
        self.execution
            .selected_program_entry
            .as_ref()
            .map(build_evaluation::SelectedCompilerProgramEntry::machine_name)
    }

    /// Complete build-owned `ProgramEntry` settlement captured while typed
    /// declarations and evaluated calling plans were still available. The
    /// source signature and optional target calling plans remain one custody
    /// object; downstream stages must not reconstruct either from the retained
    /// machine name.
    pub const fn selected_program_entry(
        &self,
    ) -> Option<&build_evaluation::SelectedCompilerProgramEntry> {
        self.execution.selected_program_entry.as_ref()
    }

    /// Exact symbol of the uniquely selected build machine. No build machine
    /// is represented by `None`; callers must not rediscover one by name.
    pub const fn selected_build_machine_symbol(&self) -> Option<symbols::SymbolHandle> {
        self.execution.selected_build_machine_symbol
    }

    /// Canonical semantic identity of the build machine actually evaluated.
    /// This is derived while the final typed declarations remain available;
    /// downstream custody must not rediscover it by short name.
    pub fn selected_build_machine_identity(&self) -> Option<&str> {
        self.execution.selected_build_machine_identity.as_deref()
    }

    /// Complete compiler-validated opaque-representation selections harvested
    /// from the authoritative build machine. Unused selections remain here as
    /// activation policy; this custody does not imply a by-value demand or a
    /// physical ABI commitment.
    pub fn opaque_representation_selections(
        &self,
    ) -> &[representation_planning::OpaqueRepresentationSelection] {
        &self.execution.opaque_representation_selections
    }

    /// Exact validated boundary calling-plan realizations retained while the
    /// typed declaration graph and selected opaque representations still
    /// coexisted. This is compiler custody for downstream reconstruction; its
    /// presence does not itself publish a package ABI or admission row.
    pub fn boundary_calling_plan_realizations(
        &self,
    ) -> &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization] {
        &self.execution.boundary_calling_plan_realizations
    }

    /// Exact named optimizations selected by the authoritative root build.
    /// Empty executes each applicable optimization phase as a validated
    /// identity transformation; it does not select an optimizer-free path.
    pub const fn optimization_selections(&self) -> &optimization_core::OptimizationSelections {
        self.execution.optimization.selections()
    }

    /// Domain-separated identity of the exact canonical selected set. This is
    /// retained independently so later cache, replay, and artifact boundaries
    /// never have to rediscover an optimization input from build syntax.
    pub const fn optimization_selection_identity(
        &self,
    ) -> optimization_core::OptimizationSelectionIdentity {
        self.execution.optimization.selection_identity()
    }

    /// Auxiliary report projection requested by the authoritative root build.
    /// This remains independent of the exact transformation selection.
    pub const fn optimization_report_request(
        &self,
    ) -> optimization_core::OptimizationReportRequest {
        self.execution.optimization.report()
    }

    pub const fn selected_provider_plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.execution.settled.selected_provider_plan_facts
    }

    /// Exact `build.omg` grants resolved to retained selected provider plans.
    pub fn selected_provider_grants(
        &self,
    ) -> &[trust_model::ResolvedAuthoredSelectedProviderGrant] {
        &self.execution.settled.selected_provider_grants
    }

    #[doc(hidden)]
    pub fn provider_plans(&self) -> &[effects::provider_plan::ProviderPlan] {
        &self.execution.provider_plans
    }

    /// Complete exact ordinary-`via` evaluation population, including leaves
    /// not selected for this executable. Package review consumes this table;
    /// backend lowering consumes only the evaluated imports installed in
    /// provider plans.
    #[doc(hidden)]
    pub const fn evaluated_via_bindings(
        &self,
    ) -> &provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable {
        &self.execution.evaluated_via_bindings
    }

    /// Exact selected normalized-import bindings and their evaluated calling
    /// plans. These rows are derived before typed trees are consumed and remain
    /// the only source of normalized foreign-locator custody in native
    /// realization; other provider mechanisms keep their specialized lanes.
    pub fn external_binding_rows(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.execution.external_binding_rows
    }

    #[doc(hidden)]
    pub fn root_grants(&self) -> &[String] {
        &self.execution.root_grants
    }

    #[doc(hidden)]
    pub const fn accepted_template_classifications(
        &self,
    ) -> &trust_model::AcceptedTemplateClassifications {
        &self.execution.settled.accepted_template_classifications
    }

    #[doc(hidden)]
    pub fn selected_provider_provenance(
        &self,
    ) -> &[crate::pipeline::provider_plans::SelectedProviderReviewProvenance] {
        &self.execution.settled.selected_provider_provenance
    }

    pub const fn component_progress(&self) -> Option<&effects::ComponentProgressManifest> {
        self.execution.settled.component_progress.as_ref()
    }

    pub const fn task_activations(&self) -> &task_plans::TaskActivationPlanSet {
        &self.execution.settled.task_activations
    }

    /// Exact target-owned callback recipes joined to their checked nominal
    /// use sites. An execution engine must consume these plans rather than
    /// derive placement from the semantic tree.
    pub fn callback_placements(&self) -> &[backend_plan::BoundNominalCallbackPlacement] {
        &self.execution.settled.callback_placements
    }

    pub const fn build_evaluation_usage(
        &self,
    ) -> Option<crate::pipeline::build_config::BuildEvaluationUsage> {
        self.execution.build_evaluation_usage
    }

    /// Exact selected build-machine observation ceiling and realized class.
    /// This execution evidence remains separate from package capability/API
    /// comparison bytes.
    pub const fn build_observation_summary(
        &self,
    ) -> Option<&crate::pipeline::build_config::BuildObservationSummary> {
        self.execution.build_observation_summary.as_ref()
    }

    /// Exact compiler-owned coordinates of checked implementation claims that
    /// ordinary validation deliberately left unjudged. Package review rejects
    /// any row until a later-discharge ledger exists.
    pub fn contract_entailment_stand_downs(&self) -> &[validation::ContractEntailmentStandDown] {
        &self.execution.settled.contract_entailment_stand_downs
    }

    pub(in crate::pipeline) const fn timings(&self) -> &CompileTimings {
        &self.timings
    }

    pub fn into_program(self) -> CheckedTrees {
        Arc::try_unwrap(self.execution.settled.program).unwrap_or_else(|shared| (*shared).clone())
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        &self.execution.settled.program
    }
}

// Review signature instantiation still uses compilation clones
// as temporary type storage. Retire this when those projectors own their scratch
// trees independently of compilation evidence. Mutation never reseals evidence.
impl std::ops::DerefMut for CheckedCompilation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.execution.settled.program)
    }
}
