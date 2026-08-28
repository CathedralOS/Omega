#![forbid(unsafe_code)]

//! Canonical Terminal-Psi to target-native realization.
//!
//! This stage owns the target-dependent lowering join. It does not own source
//! compilation, component policy, executable installation, or publication.

mod optimized_semantic_wrapper_encoding;

pub use optimized_semantic_wrapper_encoding::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};

use std::collections::BTreeSet;

use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;
pub use omega_terminal_native_artifact::{
    TerminalNativeArtifact, TerminalNativeArtifactParts, TerminalNativeProviderExecution,
    TerminalNativeSelectedProviderPlan,
};
use omega_terminal_target_operations::TerminalBoundaryRealization;
use psi_checked_trees_to_terminal::{
    CheckedProgramEntryTerminalReceipt, ProducedProgramEntryTerminalArtifact,
};
use psi_diagnostics::Diagnostic;

#[derive(Debug)]
enum StagedAbstractOperations {
    Compatibility(omega_terminal_abstract_operations::TerminalAbstractOperationPlan),
    Optimized(Box<omega_lowering_optimizer::ValidatedOptimizedAbstractPlan>),
}

impl StagedAbstractOperations {
    fn plan(&self) -> &omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
        match self {
            Self::Compatibility(plan) => plan,
            Self::Optimized(optimized) => optimized.plan(),
        }
    }
}

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct TerminalNativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn TerminalProviderExecutionEvidence,
    pub realization: TerminalBoundaryRealization,
}

/// Exact build-owned source-entry custody carried into native realization.
/// This is declaration and calling-contract evidence only: it owns no runtime
/// roots and cannot authorize a physical bootstrap, image, or publication.
#[derive(Debug, Clone, Copy)]
pub struct TerminalNativeProgramEntrySettlement<'entry> {
    source: &'entry omega_program_storage::SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: Option<&'entry omega_calling_conventions::BoundaryEntryPlan>,
    storage_entry: Option<&'entry omega_program_storage::SelectedProgramStorageEntryPlan>,
}

impl<'entry> TerminalNativeProgramEntrySettlement<'entry> {
    pub const fn new(
        source: &'entry omega_program_storage::SelectedProgramEntrySourceSignature,
        calling_plans: Option<(
            &'entry omega_calling_conventions::BoundaryEntryPlan,
            &'entry omega_program_storage::SelectedProgramStorageEntryPlan,
        )>,
    ) -> Self {
        let (semantic_boundary_entry_plan, storage_entry) = match calling_plans {
            Some((semantic, storage)) => (Some(semantic), Some(storage)),
            None => (None, None),
        };
        Self {
            source,
            semantic_boundary_entry_plan,
            storage_entry,
        }
    }

    pub const fn source(
        self,
    ) -> &'entry omega_program_storage::SelectedProgramEntrySourceSignature {
        self.source
    }

    pub const fn semantic_boundary_entry_plan(
        self,
    ) -> Option<&'entry omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan
    }

    pub const fn storage_entry(
        self,
    ) -> Option<&'entry omega_program_storage::SelectedProgramStorageEntryPlan> {
        self.storage_entry
    }

    fn validate_for_target(self, target: omega_target::NativeTarget) -> Result<(), String> {
        let slot = self.source.target_slot();
        if slot.owner.native_target() != target {
            return Err(format!(
                "selected ProgramEntry target profile `{}` does not own native target {target:?}",
                slot.owner.target_name(),
            ));
        }
        let declares_two_surfaces = slot.boundary_schema.is_some()
            || slot.physical_arrival_requirement.is_some()
            || slot.physical_contract_package.is_some()
            || slot.physical_calling_convention.is_some()
            || slot.semantic_calling_convention.is_some();
        match (
            declares_two_surfaces,
            self.semantic_boundary_entry_plan,
            self.storage_entry,
        ) {
            (false, None, None) => Ok(()),
            (true, Some(semantic), Some(storage)) => {
                validate_paired_calling_plans(self.source, semantic, storage)
            }
            _ => Err(
                "selected ProgramEntry lost its exact paired semantic/physical calling-plan custody"
                    .into(),
            ),
        }
    }
}

fn validate_paired_calling_plans(
    source: &omega_program_storage::SelectedProgramEntrySourceSignature,
    semantic: &omega_calling_conventions::BoundaryEntryPlan,
    storage: &omega_program_storage::SelectedProgramStorageEntryPlan,
) -> Result<(), String> {
    let slot = source.target_slot();
    let (Some(expected_semantic), Some(expected_physical)) = (
        slot.semantic_calling_convention,
        slot.physical_calling_convention,
    ) else {
        return Err(
            "selected ProgramEntry has an incomplete two-surface calling declaration".into(),
        );
    };
    let expected_policy = |convention| match convention {
        omega_target::ProgramEntryCallingConvention::MicrosoftX64 => {
            omega_calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if storage.target_slot() != slot || semantic.call.policy != expected_policy(expected_semantic) {
        return Err(
            "selected ProgramEntry semantic calling plan drifted from its target slot".into(),
        );
    }
    let signature = omega_calling_conventions::CallSignature {
        parameters: source
            .visible_parameters()
            .iter()
            .map(|parameter| parameter.value_shape())
            .collect(),
        result: None,
    };
    let validated_semantic =
        omega_calling_conventions::validate_boundary_entry_plan(semantic.clone(), &signature)
            .map_err(|error| format!("selected ProgramEntry semantic plan is invalid: {error}"))?;
    let matching_methods = storage
        .schema()
        .methods
        .iter()
        .filter(|method| method.requirement_identity == storage.requirement_identity())
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(
            "selected ProgramEntry storage plan lost its unique semantic requirement".into(),
        );
    };
    if method.calling_plan_fingerprint != Some(validated_semantic.contract_fingerprint())
        || method.parameter_type_identities
            != source
                .visible_parameters()
                .iter()
                .map(|parameter| parameter.normalized_type_identity().to_owned())
                .collect::<Vec<_>>()
        || method.has_result
        || method.result_type_identity.is_some()
    {
        return Err(
            "selected ProgramEntry semantic plan is not paired with its source signature".into(),
        );
    }
    let physical = storage
        .physical_contract()
        .ok_or_else(|| "selected ProgramEntry lost its physical calling contract".to_owned())?;
    let physical_plan = physical.boundary_entry_plan();
    let physical_signature = omega_calling_conventions::CallSignature {
        parameters: physical_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: physical_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated_physical = omega_calling_conventions::validate_boundary_entry_plan(
        physical_plan.clone(),
        &physical_signature,
    )
    .map_err(|error| format!("selected ProgramEntry physical plan is invalid: {error}"))?;
    if physical.target_slot() != slot
        || physical.requirement_identity() != slot.physical_arrival_requirement.unwrap_or_default()
        || physical_plan.call.policy != expected_policy(expected_physical)
        || validated_physical.contract_fingerprint() != physical.calling_plan_fingerprint()
    {
        return Err("selected ProgramEntry physical plan drifted from its target contract".into());
    }
    Ok(())
}

/// Owned, independently replayed source-entry settlement for one canonical
/// Terminal artifact. It remains declaration and calling-contract custody;
/// it grants no semantic wrapper, physical process entry, image installation,
/// or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalNativeProgramEntrySettlement {
    checked_entry: CheckedProgramEntryTerminalReceipt,
    target: omega_target::NativeTarget,
    source: omega_program_storage::SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    storage_entry: Option<omega_program_storage::SelectedProgramStorageEntryPlan>,
}

impl ValidatedTerminalNativeProgramEntrySettlement {
    pub const fn checked_entry(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.checked_entry
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn source(&self) -> &omega_program_storage::SelectedProgramEntrySourceSignature {
        &self.source
    }

    pub const fn semantic_boundary_entry_plan(
        &self,
    ) -> Option<&omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan.as_ref()
    }

    pub const fn storage_entry(
        &self,
    ) -> Option<&omega_program_storage::SelectedProgramStorageEntryPlan> {
        self.storage_entry.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalNativeProgramEntrySettlementError {
    TargetDrift,
    CallingPlanPairingDrift,
    SourceSignatureSubstitution,
    SourceMachineSubstitution,
    CanonicalArtifactReplay(String),
    TerminalPsiSubstitution,
    TerminalEntrySubstitution,
    TerminalEntryMultiplicity(usize),
}

impl std::fmt::Display for TerminalNativeProgramEntrySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalNativeProgramEntrySettlementError {}

/// Independently replay the complete source-signature, target, calling-plan,
/// Terminal-Psi, and entry-identity join without invoking the Psi receipt
/// producer.
pub fn validate_terminal_native_program_entry_settlement(
    terminal_artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    checked_entry: &CheckedProgramEntryTerminalReceipt,
    program_entry: TerminalNativeProgramEntrySettlement<'_>,
    target: omega_target::NativeTarget,
) -> Result<ValidatedTerminalNativeProgramEntrySettlement, TerminalNativeProgramEntrySettlementError>
{
    let slot = program_entry.source.target_slot();
    if slot.owner.native_target() != target {
        return Err(TerminalNativeProgramEntrySettlementError::TargetDrift);
    }
    program_entry
        .validate_for_target(target)
        .map_err(|_| TerminalNativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(TerminalNativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    if checked_entry.source_machine() != program_entry.source.machine_symbol()
        || checked_entry.source_machine_name() != program_entry.source.machine_name()
    {
        return Err(TerminalNativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    terminal_artifact.validate().map_err(|error| {
        TerminalNativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let module =
        psi_terminal_codec::decode_module(terminal_artifact.semantic_bytes()).map_err(|error| {
            TerminalNativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
        })?;
    let terminal_psi = psi_terminal_codec::terminal_psi_identity(&module).map_err(|error| {
        TerminalNativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    if terminal_psi != checked_entry.terminal_psi_identity()
        || terminal_artifact.manifest().semantic() != checked_entry.terminal_psi_identity()
    {
        return Err(TerminalNativeProgramEntrySettlementError::TerminalPsiSubstitution);
    }
    if module.entry != checked_entry.terminal_entry() {
        return Err(TerminalNativeProgramEntrySettlementError::TerminalEntrySubstitution);
    }
    let entry_count = module
        .machines
        .iter()
        .filter(|machine| machine.id == checked_entry.terminal_entry())
        .count();
    if entry_count != 1 {
        return Err(
            TerminalNativeProgramEntrySettlementError::TerminalEntryMultiplicity(entry_count),
        );
    }
    Ok(ValidatedTerminalNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        target,
        source: program_entry.source.clone(),
        semantic_boundary_entry_plan: program_entry.semantic_boundary_entry_plan.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
    })
}

/// Complete build-owned inputs for one target-native realization. Keeping
/// these coupled prevents callers from accidentally carrying entry, target,
/// optimization, and provider custody through separate positional channels.
pub struct TerminalNativeRealizationRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub subsystem: u16,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    pub program_entry: TerminalNativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub settlements: &'request [TerminalNativeProviderSettlement<'request>],
}

/// Compatibility-preserving result for the receipt-requiring native path.
#[derive(Debug)]
pub struct SettledTerminalNativeArtifact {
    artifact: TerminalNativeArtifact,
    program_entry: ValidatedTerminalNativeProgramEntrySettlement,
}

impl SettledTerminalNativeArtifact {
    pub const fn artifact(&self) -> &TerminalNativeArtifact {
        &self.artifact
    }

    pub const fn program_entry(&self) -> &ValidatedTerminalNativeProgramEntrySettlement {
        &self.program_entry
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalNativeArtifact,
        ValidatedTerminalNativeProgramEntrySettlement,
    ) {
        (self.artifact, self.program_entry)
    }
}

/// Realize a receipt-coupled checked `ProgramEntry` artifact and return its
/// independently validated, owned native settlement alongside the ordinary
/// authority-free native artifact.
pub fn realize_program_entry_terminal_native_artifact(
    produced: ProducedProgramEntryTerminalArtifact,
    request: TerminalNativeRealizationRequest<'_>,
) -> Result<SettledTerminalNativeArtifact, Vec<Diagnostic>> {
    let (terminal_artifact, checked_entry) = produced.into_parts();
    let program_entry = validate_terminal_native_program_entry_settlement(
        &terminal_artifact,
        &checked_entry,
        request.program_entry,
        request.target,
    )
    .map_err(|error| realization_error("checked ProgramEntry settlement", error))?;
    let artifact = realize_terminal_native_artifact(terminal_artifact, request)?;
    Ok(SettledTerminalNativeArtifact {
        artifact,
        program_entry,
    })
}

/// Realize one canonical Terminal-Psi artifact into an authority-free target
/// object and executable image while retaining its captured source-entry
/// settlement. Ordinary native compilation and component packaging share this
/// exact operation.
pub fn realize_terminal_native_artifact(
    terminal_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: TerminalNativeRealizationRequest<'_>,
) -> Result<TerminalNativeArtifact, Vec<Diagnostic>> {
    request
        .program_entry
        .validate_for_target(request.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    terminal_artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let semantic_bytes = terminal_artifact.semantic_bytes();
    let proof_bytes = terminal_artifact.proof_bytes();
    let abstract_operations = if request.optimization_selections.is_empty() {
        StagedAbstractOperations::Compatibility(
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections(
                semantic_bytes,
                proof_bytes,
                request.profile,
            )
            .map_err(|error| realization_error("verified artifact lowering", error))?,
        )
    } else {
        let optimization_request = omega_optimization_pipeline::compiler_baseline_request_v1(
            request.optimization_selections,
        )
        .expect("the selected realization branch is nonempty");
        StagedAbstractOperations::Optimized(Box::new(
            omega_optimization_pipeline::optimize_artifact_sections(
                semantic_bytes,
                proof_bytes,
                request.profile,
                optimization_request,
            )
            .map_err(|error| realization_error("verified optimization", error))?,
        ))
    };

    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());
    for settlement in request.settlements {
        let evidence = settlement.provider_execution;
        let requirement = evidence.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "Terminal native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = request
            .selected_provider_plans
            .plan_by_identity(evidence.provider_plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "Terminal native provider execution for `{requirement}` names unselected plan {:#018x}",
                    evidence.provider_plan()
                ))]
            })?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "Terminal native provider execution for `{requirement}` is absent from selected plan `{}`",
                selected_plan.name
            ))]);
        }
        let matching_boundaries = abstract_operations
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = matching_boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(match matching_boundaries.len() {
                0 => format!(
                    "Terminal native provider execution cites absent requirement `{requirement}`"
                ),
                count => format!(
                    "Terminal native requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedTerminalBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization: settlement.realization,
        });
        provider_executions.push(TerminalNativeProviderExecution::from_evidence(evidence));
    }
    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity(),
            left.provider_plan(),
            left.provider_execution_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan(),
                right.provider_execution_identity(),
            ))
    });

    let target_operations = match abstract_operations {
        StagedAbstractOperations::Compatibility(abstract_operations) => {
            lower_to_target_operations_with_provider_executions(
                &abstract_operations,
                request.target,
                &admitted,
            )
            .map_err(|error| realization_error("target operation lowering", error))?
        }
        StagedAbstractOperations::Optimized(optimized) => {
            let _physical = omega_optimization_pipeline::
                stage_optimized_verified_physical_pipeline_with_provider_executions(
                    *optimized, request.target, &admitted,
                )
                .map_err(|error| {
                    optimized_physical_stage_error(request.optimization_selections, error)
                })?;
            return Err(optimized_publication_unavailable(
                request.optimization_selections,
            ));
        }
    };
    let assigned =
        omega_terminal_target_operations_to_assigned_target_operations::assign_registers(
            &target_operations,
        )
        .map_err(|error| realization_error("register assignment", error))?;
    let machine_code = omega_terminal_machine_emission::emit_machine_code(&assigned)
        .map_err(|error| realization_error("machine-code emission", error))?;
    let object = omega_terminal_image_emission::build_terminal_object_artifact(&machine_code)
        .map_err(|error| realization_error("terminal object construction", error))?;
    let image =
        omega_terminal_image_emission::emit_terminal_executable_image(&object, request.subsystem)
            .map_err(|diagnostic| vec![diagnostic])?;

    let mut selected_provider_plan_projections = request
        .selected_provider_plans
        .plans()
        .iter()
        .map(|plan| {
            TerminalNativeSelectedProviderPlan::new(
                plan.identity_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    selected_provider_plan_projections.sort_by_key(TerminalNativeSelectedProviderPlan::identity);
    TerminalNativeArtifact::from_replayed_parts(TerminalNativeArtifactParts {
        target: request.target,
        terminal_artifact,
        object,
        image,
        selected_provider_closure_identity: request.selected_provider_plans.normalized_identity(),
        selected_provider_plans: selected_provider_plan_projections,
        provider_executions,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "Terminal native artifact {context} failed: {error}"
    ))]
}

fn optimized_physical_stage_error(
    selections: &omega_optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimizations `{names}` entered the optimized verified physical pipeline but failed at a named validation boundary: {error}; no output was installed"
    ))]
}

fn optimized_publication_unavailable(
    selections: &omega_optimization_core::OptimizationSelections,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` completed the verified physical pipeline through post-allocation machine validation, but frame/exit, emission, artifact, and optimized component publication validation are not available yet; no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;

    fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    fn hosted_custody() -> (
        psi_terminal_codec::CanonicalTerminalArtifact,
        CheckedProgramEntryTerminalReceipt,
        omega_program_storage::SelectedProgramEntrySourceSignature,
    ) {
        let checked = checked(
            r#"
                data Main {}
                machine Main::launch() {}
            "#,
        );
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|machine| machine.name == "Main::launch")
            .expect("terminal selection");
        let source =
            omega_program_storage::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                omega_target::TargetProfile::WindowsX64.program_entry_slot(),
                selection.machine,
                selection.machine,
                selection.name.clone(),
                "entry".into(),
                "test::Main::launch() -> Unit".into(),
                omega_program_storage::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .expect("hosted source signature");
        let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
            &checked,
            "Main::launch",
            source.identity().bytes(),
        )
        .expect("ProgramEntry Terminal artifact");
        let (artifact, receipt) = produced.into_parts();
        (artifact, receipt, source)
    }

    #[test]
    fn independently_settles_exact_hosted_source_and_terminal_entry() {
        let (artifact, receipt, source) = hosted_custody();
        let settlement = validate_terminal_native_program_entry_settlement(
            &artifact,
            &receipt,
            TerminalNativeProgramEntrySettlement::new(&source, None),
            omega_target::NativeTarget::windows_x64(),
        )
        .expect("independent ProgramEntry settlement");

        assert_eq!(settlement.source(), &source);
        assert_eq!(settlement.checked_entry(), &receipt);
        assert_eq!(
            settlement.target(),
            omega_target::NativeTarget::windows_x64()
        );
        assert!(settlement.semantic_boundary_entry_plan().is_none());
        assert!(settlement.storage_entry().is_none());
    }

    #[test]
    fn rejects_source_signature_target_and_terminal_artifact_substitution() {
        let (artifact, receipt, source) = hosted_custody();
        let substituted =
            omega_program_storage::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                source.target_slot(),
                source.machine_symbol(),
                source.state_symbol(),
                source.machine_name().into(),
                source.state_name().into(),
                "test::substituted::launch() -> Unit".into(),
                omega_program_storage::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .expect("substituted source signature");
        assert!(matches!(
            validate_terminal_native_program_entry_settlement(
                &artifact,
                &receipt,
                TerminalNativeProgramEntrySettlement::new(&substituted, None),
                omega_target::NativeTarget::windows_x64(),
            ),
            Err(TerminalNativeProgramEntrySettlementError::SourceSignatureSubstitution)
        ));
        assert!(matches!(
            validate_terminal_native_program_entry_settlement(
                &artifact,
                &receipt,
                TerminalNativeProgramEntrySettlement::new(&source, None),
                omega_target::NativeTarget::linux_x64(),
            ),
            Err(TerminalNativeProgramEntrySettlementError::TargetDrift)
        ));

        let scalar = checked(
            r#"
                data Helper {}
                machine Helper::touch() {}
                data Token { value: u64; }
                machine Token::drop(&mut self) { Helper::touch(); }
                data Main {}
                machine Main::launch(token: Token) -> u64 { 7u64 }
            "#,
        );
        let substituted_artifact =
            psi_checked_trees_to_terminal::produce_terminal_artifact(&scalar, "Main::launch")
                .expect("different canonical artifact");
        assert!(matches!(
            validate_terminal_native_program_entry_settlement(
                &substituted_artifact,
                &receipt,
                TerminalNativeProgramEntrySettlement::new(&source, None),
                omega_target::NativeTarget::windows_x64(),
            ),
            Err(TerminalNativeProgramEntrySettlementError::TerminalPsiSubstitution)
        ));
    }
}
