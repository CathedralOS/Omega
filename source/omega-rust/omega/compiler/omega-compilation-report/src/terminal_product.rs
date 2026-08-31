/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact. The local lowerer still has to rejoin this row to the exact
/// selected plan, Terminal demand, and its own target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    execution: omega_target_operations::CompilerBuiltinExecution,
}

/// Exact join from one retained callback placement to the canonical Terminal
/// boundary-call operation produced from its authored registrar occurrence.
/// The source handles used to establish this row do not cross the Psi/Omega
/// boundary; the placement index rejoins the separately retained exact row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCallbackOccurrenceProposal {
    placement_index: usize,
    terminal_operation: psi_core::OperationId,
}

impl TerminalCallbackOccurrenceProposal {
    pub const fn new(placement_index: usize, terminal_operation: psi_core::OperationId) -> Self {
        Self {
            placement_index,
            terminal_operation,
        }
    }

    pub const fn placement_index(self) -> usize {
        self.placement_index
    }

    pub const fn terminal_operation(self) -> psi_core::OperationId {
        self.terminal_operation
    }
}

impl TerminalCompilerBuiltinProposal {
    pub fn new(
        requirement_identity: String,
        provider_plan_index: usize,
        execution: omega_target_operations::CompilerBuiltinExecution,
    ) -> Result<Self, &'static str> {
        if requirement_identity.is_empty() {
            return Err("Terminal compiler-builtin proposal has an empty requirement identity");
        }
        Ok(Self {
            requirement_identity,
            provider_plan_index,
            execution,
        })
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn execution(&self) -> omega_target_operations::CompilerBuiltinExecution {
        self.execution
    }
}

/// Exact target-constrained proposal retained beside a target-neutral
/// Terminal artifact.
///
/// This owns full selected plans and external-binding rows rather than a
/// compact report fingerprint. It grants no provider execution, installation,
/// proof admission, optimization, or publication authority; a later consumer
/// must supply and replay those independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
    terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
    target_profile: omega_target::TargetProfile,
    native_target: omega_target::NativeTarget,
    subsystem: u16,
    program_entry: omega_build_evaluation::SelectedCompilerProgramEntry,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    external_binding_rows: Vec<omega_calling_conventions::ExternalBindingRow>,
    compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
    callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
}

impl TerminalNativeRealizationProposal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
        target_profile: omega_target::TargetProfile,
        native_target: omega_target::NativeTarget,
        subsystem: u16,
        program_entry: omega_build_evaluation::SelectedCompilerProgramEntry,
        selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
        external_binding_rows: Vec<omega_calling_conventions::ExternalBindingRow>,
        compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
        callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
    ) -> Result<Self, &'static str> {
        let proposal = Self {
            terminal_artifact_identity: artifact.manifest().identity(),
            target_profile,
            native_target,
            subsystem,
            program_entry,
            selected_provider_plans,
            external_binding_rows,
            compiler_builtins,
            callback_occurrences,
        };
        proposal.validate_for_artifact(artifact)?;
        Ok(proposal)
    }

    pub fn validate_for_artifact(
        &self,
        artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        artifact
            .validate()
            .map_err(|_| "Terminal native proposal is paired with an invalid canonical artifact")?;
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("Terminal native proposal belongs to a different canonical artifact");
        }
        if self.target_profile.native_target() != self.native_target
            || self.program_entry.source_signature().target_slot().owner != self.target_profile
        {
            return Err("Terminal native proposal target, profile, and ProgramEntry disagree");
        }
        let mut requirements = std::collections::BTreeSet::new();
        for builtin in &self.compiler_builtins {
            if !requirements.insert(builtin.requirement_identity()) {
                return Err("Terminal native proposal repeats a compiler-builtin requirement");
            }
            let Some(plan) = self
                .selected_provider_plans
                .plans()
                .get(builtin.provider_plan_index())
            else {
                return Err("Terminal native proposal names an absent selected provider plan");
            };
            let matching_rows = plan
                .rows
                .iter()
                .filter(|row| {
                    row.requirement_identity == builtin.requirement_identity
                        && matches!(
                            row.binding,
                            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                        )
                })
                .count();
            if matching_rows != 1 {
                return Err(
                    "Terminal native proposal compiler builtin does not rejoin one exact selected row",
                );
            }
        }
        let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
            |_| "Terminal callback occurrence replay could not decode canonical semantics",
        )?;
        let mut placement_indices = std::collections::BTreeSet::new();
        for occurrence in &self.callback_occurrences {
            if !placement_indices.insert(occurrence.placement_index) {
                return Err("Terminal native proposal repeats a callback placement occurrence");
            }
            let matching = module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == occurrence.terminal_operation)
                .collect::<Vec<_>>();
            let [operation] = matching.as_slice() else {
                return Err(
                    "Terminal callback occurrence does not name one exact canonical operation",
                );
            };
            if !matches!(
                operation.kind,
                psi_terminal::OperationKind::BoundaryCall { .. }
            ) {
                return Err("Terminal callback occurrence does not name a boundary call");
            }
        }
        Ok(())
    }

    pub const fn terminal_artifact_identity(&self) -> psi_terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub const fn target_profile(&self) -> omega_target::TargetProfile {
        self.target_profile
    }

    pub const fn native_target(&self) -> omega_target::NativeTarget {
        self.native_target
    }

    pub const fn subsystem(&self) -> u16 {
        self.subsystem
    }

    pub const fn program_entry(&self) -> &omega_build_evaluation::SelectedCompilerProgramEntry {
        &self.program_entry
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn external_binding_rows(&self) -> &[omega_calling_conventions::ExternalBindingRow] {
        &self.external_binding_rows
    }

    pub fn compiler_builtins(&self) -> &[TerminalCompilerBuiltinProposal] {
        &self.compiler_builtins
    }

    pub fn callback_occurrences(&self) -> &[TerminalCallbackOccurrenceProposal] {
        &self.callback_occurrences
    }
}

/// Canonical Terminal artifact coupled to every exact checked callback
/// placement and target-constrained native proposal that crossed its
/// production boundary.
///
/// Callback rows remain target-owned compiler evidence rather than Terminal-
/// Psi vocabulary. Keeping them in the same retained product prevents an
/// artifact-only consuming escape from silently discarding the sidecar. This
/// carrier grants no registration, invocation, address, or lifetime authority.
#[derive(Debug)]
pub struct RetainedTerminalArtifact {
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
}

impl RetainedTerminalArtifact {
    /// Couples structurally valid rows to an artifact without reconstructing
    /// their checked-compilation provenance. The compiler's private product
    /// route supplies that provenance and preserves its checked row order.
    pub fn new(
        artifact: psi_terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: None,
        };
        retained.validate()?;
        Ok(retained)
    }

    pub fn new_with_native_realization_proposal(
        artifact: psi_terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: Some(native_realization_proposal),
        };
        retained.validate()?;
        Ok(retained)
    }

    pub const fn artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn callback_placements(&self) -> &[omega_backend_plan::BoundNominalCallbackPlacement] {
        &self.callback_placements
    }

    pub const fn native_realization_proposal(&self) -> Option<&TerminalNativeRealizationProposal> {
        self.native_realization_proposal.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        psi_terminal_codec::CanonicalTerminalArtifact,
        Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
        Option<TerminalNativeRealizationProposal>,
    ) {
        (
            self.artifact,
            self.callback_placements,
            self.native_realization_proposal,
        )
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.artifact
            .validate()
            .map_err(|_| "retained Terminal product contains an invalid canonical artifact")?;
        for placement in &self.callback_placements {
            omega_backend_plan::validate_bound_nominal_callback_placement(placement)
                .map_err(|_| "retained Terminal product contains an invalid callback placement")?;
        }
        if let Some(proposal) = &self.native_realization_proposal {
            proposal.validate_for_artifact(&self.artifact)?;
            if proposal.callback_occurrences.len() != self.callback_placements.len() {
                return Err(
                    "Terminal native proposal does not cover every retained callback placement",
                );
            }
            for placement_index in 0..self.callback_placements.len() {
                if proposal
                    .callback_occurrences
                    .iter()
                    .filter(|occurrence| occurrence.placement_index == placement_index)
                    .count()
                    != 1
                {
                    return Err(
                        "Terminal native proposal does not uniquely bind a retained callback placement",
                    );
                }
            }
        }
        Ok(())
    }
}
