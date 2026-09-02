/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact. The local lowerer still has to rejoin this row to the exact
/// selected plan, Terminal demand, and its own target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    execution: omega_target_operations::CompilerBuiltinExecution,
}

/// Source-free executable body retained for one compiler-private callback
/// thunk. The canonical artifact is independently replayable and the symbol
/// is derived from the exact checked placement rather than source spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackThunkArtifact {
    private_symbol: std::sync::Arc<str>,
    artifact: std::sync::Arc<psi_terminal_codec::CanonicalTerminalArtifact>,
    lowering_receipt: psi_checked_trees_to_terminal::CallbackTerminalLoweringReceipt,
}

impl TerminalCallbackThunkArtifact {
    pub fn new(
        private_symbol: std::sync::Arc<str>,
        artifact: psi_terminal_codec::CanonicalTerminalArtifact,
        lowering_receipt: psi_checked_trees_to_terminal::CallbackTerminalLoweringReceipt,
    ) -> Result<Self, &'static str> {
        if private_symbol.is_empty() {
            return Err("Terminal callback thunk has an empty private symbol");
        }
        artifact
            .validate()
            .map_err(|_| "Terminal callback thunk contains an invalid canonical artifact")?;
        let module = psi_terminal_codec::decode_module(artifact.semantic_bytes())
            .map_err(|_| "Terminal callback thunk semantics could not be decoded")?;
        let [machine] = module.machines.as_slice() else {
            return Err("Terminal callback thunk artifact must contain exactly one machine");
        };
        if module.entry != lowering_receipt.terminal_machine
            || machine.id != lowering_receipt.terminal_machine
            || machine.entry != lowering_receipt.terminal_entry
        {
            return Err("Terminal callback thunk lowering receipt drifted from its artifact");
        }
        Ok(Self {
            private_symbol,
            artifact: std::sync::Arc::new(artifact),
            lowering_receipt,
        })
    }

    pub fn private_symbol(&self) -> &std::sync::Arc<str> {
        &self.private_symbol
    }

    pub fn artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn lowering_receipt(
        &self,
    ) -> psi_checked_trees_to_terminal::CallbackTerminalLoweringReceipt {
        self.lowering_receipt
    }
}

/// Exact join from one retained callback placement to the canonical Terminal
/// boundary-call operation produced from its authored registrar occurrence.
/// The source handles used to establish this row do not cross the Psi/Omega
/// boundary; the placement index rejoins the separately retained exact row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCallbackOccurrenceProposal {
    placement_index: usize,
    terminal_operation: psi_core::OperationId,
    direct_parameter_application: Option<omega_calling_conventions::NativeParameterApplication>,
    callback_thunk_identity: omega_function_identity::MachineFunctionIdentity,
    callback_thunk_artifact: TerminalCallbackThunkArtifact,
}

impl TerminalCallbackOccurrenceProposal {
    pub fn new(
        placement_index: usize,
        terminal_operation: psi_core::OperationId,
        direct_parameter_application: Option<omega_calling_conventions::NativeParameterApplication>,
        callback_thunk_identity: omega_function_identity::MachineFunctionIdentity,
        callback_thunk_artifact: TerminalCallbackThunkArtifact,
    ) -> Self {
        Self {
            placement_index,
            terminal_operation,
            direct_parameter_application,
            callback_thunk_identity,
            callback_thunk_artifact,
        }
    }

    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn terminal_operation(&self) -> psi_core::OperationId {
        self.terminal_operation
    }

    pub const fn direct_parameter_application(
        &self,
    ) -> Option<&omega_calling_conventions::NativeParameterApplication> {
        self.direct_parameter_application.as_ref()
    }

    pub const fn callback_thunk_identity(
        &self,
    ) -> omega_function_identity::MachineFunctionIdentity {
        self.callback_thunk_identity
    }

    pub const fn callback_thunk_artifact(&self) -> &TerminalCallbackThunkArtifact {
        &self.callback_thunk_artifact
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

/// Admitted x86 carrier for one retained target-neutral nearest-FMA
/// occurrence. The provider is immutable deployment evidence; this row does
/// not itself select or emit an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalX86ScalarFmaAdmission {
    slot: omega_target::X86ScalarFmaSlot,
    provider: omega_target::AdmittedX86ScalarFmaProvider,
}

impl TerminalX86ScalarFmaAdmission {
    pub const fn new(
        slot: omega_target::X86ScalarFmaSlot,
        provider: omega_target::AdmittedX86ScalarFmaProvider,
    ) -> Self {
        Self { slot, provider }
    }

    pub const fn slot(&self) -> omega_target::X86ScalarFmaSlot {
        self.slot
    }

    pub const fn provider(&self) -> omega_target::AdmittedX86ScalarFmaProvider {
        self.provider
    }
}

/// Source-free join from one canonical Terminal nearest-FMA operation to the
/// exact selected plan that authored it and, on x86, its admitted deployment
/// carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIeeeFloatFmaOccurrenceProposal {
    terminal_operation: psi_core::OperationId,
    provider_plan_index: usize,
    format: psi_core::IeeeFloatFormat,
    x86_admission: Option<TerminalX86ScalarFmaAdmission>,
}

impl TerminalIeeeFloatFmaOccurrenceProposal {
    pub const fn new(
        terminal_operation: psi_core::OperationId,
        provider_plan_index: usize,
        format: psi_core::IeeeFloatFormat,
        x86_admission: Option<TerminalX86ScalarFmaAdmission>,
    ) -> Self {
        Self {
            terminal_operation,
            provider_plan_index,
            format,
            x86_admission,
        }
    }

    pub const fn terminal_operation(&self) -> psi_core::OperationId {
        self.terminal_operation
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn format(&self) -> psi_core::IeeeFloatFormat {
        self.format
    }

    pub const fn x86_admission(&self) -> Option<TerminalX86ScalarFmaAdmission> {
        self.x86_admission
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
    package_terminal_authority_permissions: Vec<omega_effects::ServiceTerminalAuthorityPermission>,
    compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
    callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
    ieee_float_fma_occurrences: Vec<TerminalIeeeFloatFmaOccurrenceProposal>,
    boundary_application_coverage: omega_boundary_applications::TerminalBoundaryApplicationCoverage,
    checked_boundary_operator_scope:
        psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
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
        mut package_terminal_authority_permissions: Vec<
            omega_effects::ServiceTerminalAuthorityPermission,
        >,
        compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
        callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
        ieee_float_fma_occurrences: Vec<TerminalIeeeFloatFmaOccurrenceProposal>,
        boundary_application_demands: omega_boundary_applications::TerminalBoundaryApplicationDemands,
        boundary_application_realizations: omega_boundary_applications::TerminalBoundaryApplicationRealizations,
        checked_boundary_operator_scope: psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    ) -> Result<Self, &'static str> {
        let boundary_application_coverage =
            omega_boundary_applications::TerminalBoundaryApplicationCoverage::new(
                boundary_application_demands,
                boundary_application_realizations,
            )?;
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    left.requirement_identity()
                        .cmp(right.requirement_identity())
                })
        });
        let proposal = Self {
            terminal_artifact_identity: artifact.manifest().identity(),
            target_profile,
            native_target,
            subsystem,
            program_entry,
            selected_provider_plans,
            external_binding_rows,
            package_terminal_authority_permissions,
            compiler_builtins,
            callback_occurrences,
            ieee_float_fma_occurrences,
            boundary_application_coverage,
            checked_boundary_operator_scope,
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
        self.checked_boundary_operator_scope
            .validate_for_artifact(artifact)?;
        self.boundary_application_coverage
            .validate_for_terminal(artifact.manifest().semantic())?;
        if self.boundary_application_coverage.demands().rows().len()
            != self.checked_boundary_operator_scope.occurrences().len()
            || !self
                .boundary_application_coverage
                .demands()
                .rows()
                .iter()
                .zip(self.checked_boundary_operator_scope.occurrences())
                .all(|(demand, occurrence)| {
                    demand.terminal_operation() == occurrence.terminal_operation()
                })
        {
            return Err(
                "Terminal native proposal source-free boundary demands differ from checked occurrence custody",
            );
        }
        if self.target_profile.native_target() != self.native_target
            || self.program_entry.source_signature().target_slot().owner != self.target_profile
        {
            return Err("Terminal native proposal target, profile, and ProgramEntry disagree");
        }
        self.validate_evaluated_import_rows()?;
        self.validate_package_terminal_authority_permissions()?;
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
        self.validate_fused_program_entry_establishments(&module)?;
        for demand in self.boundary_application_coverage.demands().rows() {
            let matching_operations = module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == demand.terminal_operation())
                .count();
            if matching_operations != 1 {
                return Err(
                    "Terminal native proposal boundary demand does not name one canonical operation",
                );
            }
        }
        let mut placement_indices = std::collections::BTreeSet::new();
        let mut callback_operations = std::collections::BTreeSet::new();
        let mut callback_thunk_identities = std::collections::HashSet::new();
        for occurrence in &self.callback_occurrences {
            if !placement_indices.insert(occurrence.placement_index) {
                return Err("Terminal native proposal repeats a callback placement occurrence");
            }
            if !occurrence.callback_thunk_identity.is_valid()
                || occurrence
                    .callback_thunk_identity
                    .callback_thunk_placement_index()
                    != Some(occurrence.placement_index)
            {
                return Err(
                    "Terminal callback occurrence has an invalid callback-thunk role or placement index",
                );
            }
            if !callback_thunk_identities.insert(occurrence.callback_thunk_identity) {
                return Err("Terminal native proposal repeats a callback-thunk identity");
            }
            occurrence
                .callback_thunk_artifact
                .artifact()
                .validate()
                .map_err(|_| "Terminal callback occurrence contains an invalid thunk artifact")?;
            if !callback_operations.insert(occurrence.terminal_operation) {
                return Err("Terminal native proposal repeats a callback registrar operation");
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
        let terminal_fma_operations = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    psi_terminal::OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
                )
            })
            .collect::<Vec<_>>();
        if terminal_fma_operations.len() != self.ieee_float_fma_occurrences.len() {
            return Err(
                "Terminal native proposal does not retain every nearest-FMA occurrence exactly once",
            );
        }
        let x86_target = self.native_target.architecture == omega_target::Architecture::X86_64;
        let mut fma_operation_ids = std::collections::BTreeSet::new();
        for occurrence in &self.ieee_float_fma_occurrences {
            if !fma_operation_ids.insert(occurrence.terminal_operation) {
                return Err("Terminal native proposal repeats a nearest-FMA occurrence");
            }
            if self
                .selected_provider_plans
                .plans()
                .get(occurrence.provider_plan_index)
                .is_none()
            {
                return Err("Terminal nearest-FMA occurrence names an absent selected plan");
            }
            let matching = terminal_fma_operations
                .iter()
                .filter(|operation| operation.id == occurrence.terminal_operation)
                .collect::<Vec<_>>();
            let [operation] = matching.as_slice() else {
                return Err(
                    "Terminal nearest-FMA occurrence does not name one exact canonical operation",
                );
            };
            let Some(result) = operation.result.scalar() else {
                return Err("Terminal nearest-FMA occurrence has no scalar result");
            };
            let psi_core::ScalarType::IeeeFloat(format) = result.scalar_type else {
                return Err("Terminal nearest-FMA occurrence has a non-float result");
            };
            if format != occurrence.format {
                return Err("Terminal nearest-FMA occurrence changed its IEEE format");
            }
            match (x86_target, occurrence.x86_admission) {
                (true, Some(admission)) => {
                    let expected_slot = match occurrence.format {
                        psi_core::IeeeFloatFormat::Binary32 => {
                            omega_target::X86ScalarFmaSlot::Binary32
                        }
                        psi_core::IeeeFloatFormat::Binary64 => {
                            omega_target::X86ScalarFmaSlot::Binary64
                        }
                    };
                    let provider = admission.provider;
                    if admission.slot != expected_slot
                        || !provider.has_canonical_identity()
                        || provider.profile() != self.target_profile
                        || !provider.admits(provider.requirement(), admission.slot)
                    {
                        return Err(
                            "Terminal nearest-FMA occurrence has invalid x86 admission custody",
                        );
                    }
                }
                (true, None) => {
                    return Err(
                        "x86 Terminal nearest-FMA occurrence lacks admitted deployment custody",
                    );
                }
                (false, Some(_)) => {
                    return Err(
                        "non-x86 Terminal nearest-FMA occurrence carries an x86 deployment admission",
                    );
                }
                (false, None) => {}
            }
        }
        Ok(())
    }

    fn validate_fused_program_entry_establishments(
        &self,
        module: &psi_terminal::TerminalModule,
    ) -> Result<(), &'static str> {
        let rows = self.program_entry.fused_service_establishments();
        if rows.is_empty() {
            return Ok(());
        }
        let source = self.program_entry.source_signature();
        let Some(receiver_identity) = source.receiver().normalized_type_identity() else {
            return Err("Terminal proposal gives a free ProgramEntry a Fused root establishment");
        };
        let entry_machines = module
            .machines
            .iter()
            .filter(|machine| machine.id == module.entry)
            .collect::<Vec<_>>();
        let [entry_machine] = entry_machines.as_slice() else {
            return Err("Terminal proposal has no unique entry machine for root establishment");
        };
        let Some(attachment) = entry_machine.attachment else {
            return Err("Terminal proposal root establishment lost its entry attachment");
        };
        let attachment_types = module
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == attachment)
            .collect::<Vec<_>>();
        let [attachment_type] = attachment_types.as_slice() else {
            return Err("Terminal proposal root establishment has no unique attachment type");
        };
        let psi_terminal::StructuralTypeShape::Record { fields } = &attachment_type.shape else {
            return Err("Terminal proposal root establishment attachment is not a record");
        };
        if attachment_type.identity != rows[0].attachment_type_identity()
            || rows
                .iter()
                .any(|row| row.attachment_type_identity() != attachment_type.identity)
        {
            return Err("Terminal proposal root establishment substituted its receiver type");
        }
        let mut field_identities = std::collections::BTreeSet::new();
        for row in rows {
            if row.source_signature_identity() != source.identity()
                || row.target_slot() != source.target_slot()
                || row.receiver_type_identity() != receiver_identity
                || !field_identities.insert(row.field_identity())
            {
                return Err(
                    "Terminal proposal root establishment drifted from ProgramEntry custody",
                );
            }
            let matching_fields = fields
                .iter()
                .filter(|field| field.identity == row.field_identity())
                .collect::<Vec<_>>();
            let [field] = matching_fields.as_slice() else {
                return Err("Terminal proposal root establishment has no unique receiver field");
            };
            if !matches!(
                &field.field_type,
                psi_terminal::StructuralFieldType::Erased { type_identity }
                    if type_identity == row.carrier_type_identity()
            ) {
                return Err("Terminal proposal root establishment substituted its erased carrier");
            }
            let matching_plans = self
                .selected_provider_plans
                .plans()
                .iter()
                .filter(|plan| {
                    omega_program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema)
                        == row.requirement_identity()
                        && plan.schema.identity_digest() == row.service_schema_digest()
                        && plan.identity_digest() == row.selected_provider_plan_digest()
                })
                .count();
            if matching_plans != 1 {
                return Err(
                    "Terminal proposal root establishment has no unique selected provider plan",
                );
            }
        }
        Ok(())
    }

    fn validate_package_terminal_authority_permissions(&self) -> Result<(), &'static str> {
        let mut previous = None;
        for permission in &self.package_terminal_authority_permissions {
            if permission.requirement_identity().is_empty()
                || permission
                    .requirement_identity()
                    .chars()
                    .any(char::is_control)
            {
                return Err(
                    "Terminal native proposal contains an invalid package terminal-authority permission requirement",
                );
            }
            let coordinate = (
                permission.service_schema(),
                permission.requirement_identity(),
            );
            if previous == Some(coordinate) {
                return Err(
                    "Terminal native proposal repeats a package terminal-authority permission",
                );
            }
            previous = Some(coordinate);
        }
        Ok(())
    }

    fn validate_evaluated_import_rows(&self) -> Result<(), &'static str> {
        let target_name = self.target_profile.target_name();
        let mut matched_external_rows = std::collections::BTreeSet::new();
        for plan in self.selected_provider_plans.plans() {
            let effective_target = if plan.target.is_empty() {
                target_name
            } else {
                plan.target.as_str()
            };
            for selected_row in &plan.rows {
                let omega_effects::provider_plan::ProviderBinding::Import { evaluated } =
                    &selected_row.binding
                else {
                    continue;
                };
                if effective_target != target_name
                    || evaluated.locator().target() != self.target_profile
                {
                    return Err(
                        "Terminal native proposal selected import disagrees with its target profile",
                    );
                }
                let matching = self
                    .external_binding_rows
                    .iter()
                    .enumerate()
                    .filter(|(_, external)| {
                        external.target_name == effective_target
                            && external.trait_name == plan.schema.trait_name
                            && external.method == selected_row.method
                            && external.requirement_identity == selected_row.requirement_identity
                            && external.table_type == plan.provider_type
                    })
                    .collect::<Vec<_>>();
                let [(external_index, external)] = matching.as_slice() else {
                    return Err(
                        "Terminal native proposal selected import does not rejoin one exact external-binding row",
                    );
                };
                let omega_calling_conventions::ExternalBindingKind::Import { locator } =
                    &external.binding
                else {
                    return Err(
                        "Terminal native proposal selected import rejoined a non-evaluated external binding",
                    );
                };
                if locator != evaluated.locator() || locator.target() != self.target_profile {
                    return Err(
                        "Terminal native proposal external import substituted the selected evaluated locator",
                    );
                }
                if !matched_external_rows.insert(*external_index) {
                    return Err(
                        "Terminal native proposal external import was reused by multiple selected rows",
                    );
                }
            }
        }
        if self
            .external_binding_rows
            .iter()
            .enumerate()
            .any(|(index, external)| {
                matches!(
                    external.binding,
                    omega_calling_conventions::ExternalBindingKind::Import { .. }
                ) && !matched_external_rows.contains(&index)
            })
        {
            return Err(
                "Terminal native proposal retains an unmatched evaluated external import row",
            );
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

    /// Consumer-supplied package permission rows retained before the checked
    /// frontend was destroyed. These rows grant nothing by themselves; native
    /// re-entry must first match them to independently accepted package
    /// evidence and then rejoin that accepted set to the receiving policy.
    pub fn package_terminal_authority_permissions(
        &self,
    ) -> &[omega_effects::ServiceTerminalAuthorityPermission] {
        &self.package_terminal_authority_permissions
    }

    pub fn compiler_builtins(&self) -> &[TerminalCompilerBuiltinProposal] {
        &self.compiler_builtins
    }

    pub fn callback_occurrences(&self) -> &[TerminalCallbackOccurrenceProposal] {
        &self.callback_occurrences
    }

    pub fn ieee_float_fma_occurrences(&self) -> &[TerminalIeeeFloatFmaOccurrenceProposal] {
        &self.ieee_float_fma_occurrences
    }

    /// Non-caller-authored checked D29 scope retained before the checked
    /// frontend was destroyed.
    pub const fn checked_boundary_operator_scope(
        &self,
    ) -> &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope {
        &self.checked_boundary_operator_scope
    }

    pub const fn boundary_application_demands(
        &self,
    ) -> &omega_boundary_applications::TerminalBoundaryApplicationDemands {
        self.boundary_application_coverage.demands()
    }

    pub const fn boundary_application_realizations(
        &self,
    ) -> &omega_boundary_applications::TerminalBoundaryApplicationRealizations {
        self.boundary_application_coverage.realizations()
    }

    pub const fn boundary_application_coverage(
        &self,
    ) -> &omega_boundary_applications::TerminalBoundaryApplicationCoverage {
        &self.boundary_application_coverage
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
                let matching = proposal
                    .callback_occurrences
                    .iter()
                    .filter(|occurrence| occurrence.placement_index == placement_index)
                    .collect::<Vec<_>>();
                let [occurrence] = matching.as_slice() else {
                    return Err(
                        "Terminal native proposal does not uniquely bind a retained callback placement",
                    );
                };
                let expected_application = self.callback_placements[placement_index]
                    .private_materialization
                    .as_ref()
                    .and_then(|materialization| {
                        materialization
                            .direct_registrar_parameter_application
                            .as_ref()
                    });
                if occurrence.direct_parameter_application() != expected_application {
                    return Err(
                        "Terminal callback occurrence native parameter application drifted from its retained placement",
                    );
                }
                let expected_thunk = omega_backend_plan::canonical_callback_thunk_identity(
                    placement_index,
                    &self.callback_placements[placement_index],
                )
                .ok_or(
                    "retained callback placement cannot derive one valid callback-thunk identity",
                )?;
                if occurrence.callback_thunk_identity() != expected_thunk {
                    return Err(
                        "Terminal callback occurrence thunk continuation drifted from its retained placement",
                    );
                }
                let expected_symbol = omega_backend_plan::canonical_callback_private_symbol(
                    &self.callback_placements[placement_index],
                );
                if occurrence.callback_thunk_artifact().private_symbol() != &expected_symbol {
                    return Err(
                        "Terminal callback occurrence private symbol drifted from its retained placement",
                    );
                }
                let receipt = occurrence.callback_thunk_artifact().lowering_receipt();
                if receipt.source_machine
                    != self.callback_placements[placement_index].selected_machine
                    || receipt.source_entry
                        != self.callback_placements[placement_index].selected_entry
                {
                    return Err(
                        "Terminal callback occurrence lowering receipt drifted from its retained placement",
                    );
                }
            }
        }
        Ok(())
    }
}
