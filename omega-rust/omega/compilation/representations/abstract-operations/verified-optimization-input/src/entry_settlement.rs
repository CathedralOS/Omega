/*
  Native `ProgramEntry` settlement and independent replay
  (`validate_native_program_entry_settlement`,
  `ValidatedNativeProgramEntrySettlement`).

  Pairs the selected source entry declaration with its evaluated calling
  contracts, replays the canonical Terminal artifact and target machine
  selections, and seals owned entry custody.
*/
mod calling_plans {
    use super::model::ValidatedNativeProgramEntryCallingPlans;
    use calling_conventions::ValidatedBoundaryEntryPlan;
    use calling_conventions::{
        BoundaryCallingApplication, BoundarySignatureGeometry, BoundaryValueClass,
    };
    use effects::provider_plan::{ServiceMethod, ServiceSchema};
    use program_entry_plan::{SelectedProgramEntrySignature, SelectedProgramStorageEntryPlan};

    pub(crate) fn validate_paired_calling_plans(
        source: &SelectedProgramEntrySignature,
        semantic: &BoundaryCallingApplication,
        physical: &BoundaryCallingApplication,
        storage: &SelectedProgramStorageEntryPlan,
    ) -> Result<ValidatedNativeProgramEntryCallingPlans, String> {
        let slot = source.target_slot();
        let (Some(expected_semantic), Some(expected_physical), Some(physical_requirement)) = (
            slot.semantic_calling_convention,
            slot.physical_calling_convention,
            slot.physical_arrival_requirement,
        ) else {
            return Err(
                "selected ProgramEntry has an incomplete two-surface calling declaration".into(),
            );
        };
        if storage.target_slot() != slot {
            return Err(
                "selected ProgramEntry storage contract drifted from its target slot".into(),
            );
        }
        let semantic_method = selected_method(storage, slot.semantic_arrival_requirement)?;
        let physical_method = selected_method(storage, physical_requirement)?;
        if Some(
            storage
                .schema()
                .identity_text(semantic_method.requirement_identity),
        ) != storage.identities().text(storage.requirement_identity())
            || semantic_method.requirement_identity == physical_method.requirement_identity
        {
            return Err(
                "selected ProgramEntry lost its distinct exact arrival requirements".into(),
            );
        }
        let semantic_plan = join_application(
            storage.schema(),
            semantic,
            semantic_method,
            slot.owner.native_target(),
        )?;
        let physical_plan = join_application(
            storage.schema(),
            physical,
            physical_method,
            slot.owner.native_target(),
        )?;
        if semantic_plan.plan().call.policy
            != calling_conventions::CallingPolicy::for_program_entry_convention(expected_semantic)
            || physical_plan.plan().call.policy
                != calling_conventions::CallingPolicy::for_program_entry_convention(
                    expected_physical,
                )
        {
            return Err(
                "selected ProgramEntry calling policies drifted from their target slot".into(),
            );
        }

        let semantic_shapes = semantic_plan
            .plan()
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect::<Vec<_>>();
        let semantic_source_pairing = match slot.schema {
            target::ProgramEntrySchema::ProgramStorageApplication => {
                semantic_method
                    .parameter_type_identities
                    .iter()
                    .map(|identity| storage.schema().declaration.identities.text(*identity))
                    .eq(source.visible_parameters().iter().map(|parameter| {
                        source
                            .identities()
                            .text(parameter.normalized_type_identity())
                            .unwrap_or_default()
                    }))
                    && semantic_shapes
                        == source
                            .visible_parameters()
                            .iter()
                            .map(|parameter| parameter.value_shape())
                            .collect::<Vec<_>>()
            }
            target::ProgramEntrySchema::HostedApplication => {
                source.visible_parameters().is_empty()
                    && semantic_method.parameter_count == 2
                    && semantic_method.parameter_type_identities.len() == 2
                    && semantic_method
                        .parameter_type_identities
                        .iter()
                        .all(|identity| identity == &semantic_method.parameter_type_identities[0])
                    && semantic_shapes == [calling_conventions::ValueShape::integer(16, 8); 2]
                    && has_internal_storage_signature(&semantic.signature)
            }
        };
        if !semantic_source_pairing
            || semantic_method.has_result
            || semantic_method.result_type_identity.is_valid()
            || semantic_plan.plan().call.result.is_some()
        {
            return Err(
                "selected ProgramEntry semantic plan is not paired with its source signature"
                    .into(),
            );
        }
        let contract = storage
            .physical_contract()
            .ok_or_else(|| "selected ProgramEntry lost its physical calling contract".to_owned())?;
        if contract.target_slot() != slot
            || contract.identities().text(contract.requirement_identity())
                != Some(
                    storage
                        .schema()
                        .identity_text(physical_method.requirement_identity),
                )
            || !contract
                .parameter_type_identities()
                .iter()
                .zip(physical_method.parameter_type_identities.iter())
                .all(|(left, right)| {
                    contract.identities().text(*left)
                        == Some(storage.schema().declaration.identities.text(*right))
                })
            || contract.parameter_type_identities().len()
                != physical_method.parameter_type_identities.len()
            || !physical_method.has_result
            || Some(
                storage
                    .schema()
                    .declaration
                    .identities
                    .text(physical_method.result_type_identity),
            ) != contract.identities().text(contract.result_type_identity())
            || contract.boundary_entry_plan() != physical_plan.plan()
            || contract.calling_plan_report_fingerprint()
                != physical_plan.contract_report_fingerprint()
        {
            return Err(
                "selected ProgramEntry physical plan drifted from its target contract".into(),
            );
        }
        Ok(ValidatedNativeProgramEntryCallingPlans {
            semantic: semantic_plan.clone(),
            physical: physical_plan.clone(),
        })
    }

    fn has_internal_storage_signature(signature: &BoundarySignatureGeometry) -> bool {
        signature.parameters.len() == 2
            && signature.result.is_none()
            && signature.parameters.iter().all(|root| {
                let Some(shape) = signature.shapes.get(usize::from(*root)) else {
                    return false;
                };
                let BoundaryValueClass::Record {
                    first_field,
                    field_count: 2,
                } = shape.class()
                else {
                    return false;
                };
                if shape.byte_size() != 16 || shape.alignment() != 8 {
                    return false;
                }
                let Some(fields) = signature
                    .fields
                    .get(usize::from(first_field)..)
                    .and_then(|fields| fields.get(..2))
                else {
                    return false;
                };
                fields.iter().zip([0, 8]).all(|(field, offset)| {
                    field.byte_offset() == offset
                        && signature
                            .shapes
                            .get(usize::from(field.shape()))
                            .is_some_and(|word| {
                                word.class() == BoundaryValueClass::Integer
                                    && word.byte_size() == 8
                                    && word.alignment() == 8
                            })
                })
            })
    }

    fn selected_method<'schema>(
        storage: &'schema SelectedProgramStorageEntryPlan,
        spelling: &str,
    ) -> Result<&'schema ServiceMethod, String> {
        let (owner, name) = spelling.split_once("::").ok_or_else(|| {
            "selected ProgramEntry has an invalid target requirement spelling".to_owned()
        })?;
        let mut methods = storage
            .schema()
            .declaration
            .methods
            .iter()
            .filter(|method| method.requirement_owner == owner && method.name == name);
        let method = methods
            .next()
            .ok_or_else(|| format!("selected ProgramEntry schema lost requirement `{spelling}`"))?;
        if methods.next().is_some() {
            return Err(format!(
                "selected ProgramEntry schema repeats requirement `{spelling}`"
            ));
        }
        Ok(method)
    }

    fn join_application<'application>(
        schema: &ServiceSchema,
        application: &'application BoundaryCallingApplication,
        method: &ServiceMethod,
        target: target::NativeTarget,
    ) -> Result<&'application ValidatedBoundaryEntryPlan, String> {
        if application.target != target
            || application.requirement_identity != schema.identity_text(method.requirement_identity)
            || schema
                .calling_application(&application.abi)
                .map(|plan| plan.report_fingerprint)
                != Some(application.report_fingerprint)
            || schema
                .calling_application(&application.abi)
                .map(|plan| plan.commitment)
                .map(|commitment| commitment.as_bytes())
                != Some(application.commitment)
        {
            return Err(
                "selected ProgramEntry lost exact calling-application identity and custody".into(),
            );
        }
        Ok(&application.plan)
    }
}
mod model {
    use super::PhysicalProgramEntry;
    use super::calling_plans::validate_paired_calling_plans;
    use calling_conventions::{
        BoundaryCallingApplication, BoundaryEntryPlan, ValidatedBoundaryEntryPlan,
    };
    use program_entry_plan::{SelectedProgramEntrySignature, SelectedProgramStorageEntryPlan};
    use terminal_installation::CheckedPrivateEntryRamRecipe;
    use terminal_installation::TerminalAccessViewEstablishment;
    use terminal_psi::CheckedProgramEntryTerminalReceipt;

    #[derive(Debug, Clone, Copy)]
    pub struct NativeProgramEntrySettlement<'entry> {
        pub checked_entry: Option<&'entry CheckedProgramEntryTerminalReceipt>,
        pub physical_adapter: Option<&'entry PhysicalProgramEntry>,
        pub source: &'entry SelectedProgramEntrySignature,
        pub semantic_calling_application: Option<&'entry BoundaryCallingApplication>,
        pub physical_calling_application: Option<&'entry BoundaryCallingApplication>,
        pub storage_entry: Option<&'entry SelectedProgramStorageEntryPlan>,
    }

    impl<'entry> NativeProgramEntrySettlement<'entry> {
        pub const fn new(
            source: &'entry SelectedProgramEntrySignature,
            calling_plans: Option<(
                &'entry BoundaryCallingApplication,
                &'entry BoundaryCallingApplication,
                &'entry SelectedProgramStorageEntryPlan,
            )>,
        ) -> Self {
            let (semantic_calling_application, physical_calling_application, storage_entry) =
                match calling_plans {
                    Some((semantic, physical, storage)) => {
                        (Some(semantic), Some(physical), Some(storage))
                    }
                    None => (None, None, None),
                };
            Self {
                checked_entry: None,
                physical_adapter: None,
                source,
                semantic_calling_application,
                physical_calling_application,
                storage_entry,
            }
        }

        pub const fn with_checked_entry(
            mut self,
            checked_entry: &'entry CheckedProgramEntryTerminalReceipt,
        ) -> Self {
            self.checked_entry = Some(checked_entry);
            self
        }

        pub const fn source(self) -> &'entry SelectedProgramEntrySignature {
            self.source
        }

        pub const fn physical_adapter(self) -> Option<&'entry PhysicalProgramEntry> {
            self.physical_adapter
        }

        pub const fn semantic_boundary_entry_plan(self) -> Option<&'entry BoundaryEntryPlan> {
            match self.semantic_calling_application {
                Some(application) => Some(application.plan.plan()),
                None => None,
            }
        }

        pub const fn semantic_calling_application(
            self,
        ) -> Option<&'entry BoundaryCallingApplication> {
            self.semantic_calling_application
        }

        pub const fn physical_calling_application(
            self,
        ) -> Option<&'entry BoundaryCallingApplication> {
            self.physical_calling_application
        }

        pub const fn storage_entry(self) -> Option<&'entry SelectedProgramStorageEntryPlan> {
            self.storage_entry
        }

        pub(crate) fn validate_for_target(
            self,
            target: target::NativeTarget,
        ) -> Result<Option<ValidatedNativeProgramEntryCallingPlans>, String> {
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
            self.semantic_calling_application,
            self.physical_calling_application,
            self.storage_entry,
        ) {
            (false, None, None, None) => Ok(None),
            (true, Some(semantic), Some(physical), Some(storage)) => {
                validate_paired_calling_plans(self.source, semantic, physical, storage)
                    .map(Some)
            }
            _ => Err(
                "selected ProgramEntry lost its exact paired semantic/physical calling-plan custody"
                    .into(),
            ),
        }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedNativeProgramEntryCallingPlans {
        pub(crate) semantic: ValidatedBoundaryEntryPlan,
        pub(crate) physical: ValidatedBoundaryEntryPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedNativeProgramEntrySettlement {
        pub(crate) checked_entry: CheckedProgramEntryTerminalReceipt,
        pub(crate) physical_adapter: Option<PhysicalProgramEntry>,
        pub(crate) target: target::NativeTarget,
        pub(crate) source: SelectedProgramEntrySignature,
        pub(crate) semantic_calling_application: Option<BoundaryCallingApplication>,
        pub(crate) admitted_calling_plans: Option<ValidatedNativeProgramEntryCallingPlans>,
        pub(crate) physical_calling_application: Option<BoundaryCallingApplication>,
        pub(crate) storage_entry: Option<SelectedProgramStorageEntryPlan>,
        pub(crate) access_view_establishments: Vec<TerminalAccessViewEstablishment>,
        pub(crate) private_entry_ram: Vec<CheckedPrivateEntryRamRecipe>,
    }

    impl ValidatedNativeProgramEntrySettlement {
        pub(crate) fn bind_physical_adapter(
            &mut self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            selected: &[provider_admission::SelectedProviderAdapter],
        ) -> Result<(), &'static str> {
            let application = self
                .physical_calling_application()
                .ok_or("physical adapter selected without a physical arrival contract")?;
            self.physical_adapter =
                Some(PhysicalProgramEntry::bind(artifact, application, selected)?);
            Ok(())
        }

        pub fn as_settlement(&self) -> NativeProgramEntrySettlement<'_> {
            NativeProgramEntrySettlement {
                checked_entry: Some(&self.checked_entry),
                physical_adapter: self.physical_adapter.as_ref(),
                source: &self.source,
                semantic_calling_application: self.semantic_calling_application.as_ref(),
                physical_calling_application: self.physical_calling_application.as_ref(),
                storage_entry: self.storage_entry.as_ref(),
            }
        }

        pub const fn checked_entry(&self) -> &CheckedProgramEntryTerminalReceipt {
            &self.checked_entry
        }

        pub const fn physical_adapter(&self) -> Option<&PhysicalProgramEntry> {
            self.physical_adapter.as_ref()
        }

        pub const fn target(&self) -> target::NativeTarget {
            self.target
        }

        pub const fn source(&self) -> &SelectedProgramEntrySignature {
            &self.source
        }

        pub const fn semantic_boundary_entry_plan(&self) -> Option<&BoundaryEntryPlan> {
            match self.semantic_calling_application.as_ref() {
                Some(application) => Some(application.plan.plan()),
                None => None,
            }
        }

        pub const fn semantic_calling_application(&self) -> Option<&BoundaryCallingApplication> {
            self.semantic_calling_application.as_ref()
        }

        pub const fn physical_calling_plan(&self) -> Option<&ValidatedBoundaryEntryPlan> {
            match self.admitted_calling_plans.as_ref() {
                Some(plans) => Some(&plans.physical),
                None => None,
            }
        }

        pub const fn physical_calling_application(&self) -> Option<&BoundaryCallingApplication> {
            self.physical_calling_application.as_ref()
        }

        pub const fn storage_entry(&self) -> Option<&SelectedProgramStorageEntryPlan> {
            self.storage_entry.as_ref()
        }

        pub fn private_entry_ram(&self) -> &[CheckedPrivateEntryRamRecipe] {
            &self.private_entry_ram
        }

        pub fn with_private_entry_ram(mut self, recipes: &[CheckedPrivateEntryRamRecipe]) -> Self {
            self.private_entry_ram = recipes.to_vec();
            self
        }

        pub fn access_view_establishments(&self) -> &[TerminalAccessViewEstablishment] {
            &self.access_view_establishments
        }

        pub fn with_access_view_establishments(
            mut self,
            establishments: &[TerminalAccessViewEstablishment],
        ) -> Self {
            self.access_view_establishments = establishments.to_vec();
            self
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum NativeProgramEntrySettlementError {
        UnsuppliedErasedInputs {
            machine: semantic_vocabulary::MachineId,
            reason: &'static str,
        },
        TargetFamily(String),
        TargetDrift,
        CallingPlanPairingDrift,
        SourceSignatureSubstitution,
        SourceMachineSubstitution,
        CanonicalArtifactReplay(String),
        TerminalPsiSubstitution,
        TerminalEntrySubstitution,
        TerminalEntryMultiplicity(usize),
    }

    diagnostics::debug_display!(NativeProgramEntrySettlementError);

    impl std::error::Error for NativeProgramEntrySettlementError {}
}
mod physical_adapter {
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PhysicalProgramEntry {
        terminal: terminal_psi::TerminalPsiIdentity,
        machine: MachineId,
        parameter_formals: Vec<u32>,
        candidate: terminal_psi::ProviderCandidateConformance,
    }

    impl PhysicalProgramEntry {
        pub const fn terminal(&self) -> terminal_psi::TerminalPsiIdentity {
            self.terminal
        }
        pub const fn machine(&self) -> MachineId {
            self.machine
        }

        pub fn parameter_formals(&self) -> &[u32] {
            &self.parameter_formals
        }

        pub(crate) fn bind(
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            application: &calling_conventions::BoundaryCallingApplication,
            selected: &[provider_admission::SelectedProviderAdapter],
        ) -> Result<Self, &'static str> {
            let module = artifact.semantic_module();
            let mut candidates = module.provider_candidates.iter().filter(|candidate| {
                module.identities.text(candidate.requirement_identity)
                    == Some(application.requirement_identity.as_str())
                    && module.calling_abi_identity(candidate.boundary).as_ref()
                        == Ok(&application.abi)
                    && selected.iter().any(|selection| {
                        selection.requirement_identity == application.requirement_identity
                            && module.identities.text(candidate.candidate_identity)
                                == Some(selection.machine_identity.as_str())
                            && module.identities.text(candidate.provider_identity)
                                == Some(selection.provider_identity.as_str())
                            && module
                                .identities
                                .text(candidate.provider_declaration.identity())
                                == Some(selection.provider_declaration.as_str())
                    })
            });
            let candidate = candidates
                .next()
                .ok_or("physical adapter has no exact selected checked conformance")?;
            if candidates.next().is_some() {
                return Err("physical adapter selection is ambiguous");
            }
            let machine = candidate.candidate;
            let body = module
                .machine(machine)
                .ok_or("physical adapter body is absent")?;
            let boundary = module
                .boundary_machine(candidate.boundary)
                .ok_or("physical adapter requirement is absent")?;
            if boundary.scalar_parameters.len() != body.parameters.len()
                || boundary.structural_parameters.len() != body.structural_parameters.len()
            {
                return Err("physical adapter formals differ from its retained body");
            }
            if !boundary.has_valid_parameter_order()
                || boundary.parameter_order.len() != application.plan.plan().call.parameters.len()
            {
                return Err("physical adapter formals are not the complete arrival signature");
            }
            terminal_semantics::validate_empty_erased_invocation_inputs(module, machine)?;
            let parameter_formals = boundary
                .parameter_positions(terminal_psi::BoundaryParameterKind::Scalar)
                .chain(
                    boundary.parameter_positions(terminal_psi::BoundaryParameterKind::Structural),
                )
                .map(|position| {
                    u32::try_from(position).map_err(|_| "physical formal index overflows")
                })
                .collect::<Result<Vec<_>, _>>()?;
            if body.runtime_parameter_formals()? != parameter_formals {
                return Err("physical adapter body changes the retained arrival parameter order");
            }
            Ok(Self {
                terminal: artifact.view().identity(),
                machine,
                parameter_formals,
                candidate: candidate.clone(),
            })
        }
    }
}
mod semantic_call {
    use super::ValidatedNativeProgramEntrySettlement;
    use crate::SelectedProgramEntryCall;
    use effects::CompilerIntrinsicExecutionIdentity;
    use effects::provider_plan::ProviderBinding;

    /// Settles the `ProgramEntry` call native admission selected for
    /// `artifact`, or `None` when no selected provider row carries the
    /// program's storage entry.
    pub fn bind_selected_program_entry_call(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        entry: &ValidatedNativeProgramEntrySettlement,
        selection: &effects::SelectedProviderPlanFacts,
        intrinsic_executions: &[Vec<Option<CompilerIntrinsicExecutionIdentity>>],
    ) -> Result<Option<SelectedProgramEntryCall>, String> {
        let plans = selection.plans();
        if plans.len() != intrinsic_executions.len() {
            return Err("semantic-entry transport has misaligned provider plan custody".into());
        }
        let module = artifact.semantic_module();
        let mut entry_rows = Vec::new();
        for (plan, executions) in plans.iter().zip(intrinsic_executions) {
            if plan.rows.len() != executions.len() {
                return Err(format!(
                    "semantic-entry transport has misaligned intrinsic rows for `{}`",
                    plan.name,
                ));
            }
            for (row, execution) in plan.rows.iter().zip(executions) {
                if *execution != Some(CompilerIntrinsicExecutionIdentity::ProgramStorageEntry) {
                    continue;
                }
                if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
                    return Err("semantic-entry transport has no selected intrinsic binding".into());
                }
                let requirement = plan.identities.text(row.requirement_identity);
                entry_rows.push((plan, requirement));
            }
        }
        if entry_rows.is_empty() {
            return Ok(None);
        }
        let mut selected = None;
        for boundary_id in module.blocks().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    terminal_psi::OperationKind::BoundaryCall { boundary, .. } => Some(*boundary),
                    _ => None,
                })
                .chain(match &block.terminator {
                    terminal_psi::Terminator::ExternalCompletion { boundary, .. } => {
                        Some(*boundary)
                    }
                    _ => None,
                })
        }) {
            let boundary = module
                .boundary_machine(boundary_id)
                .ok_or("semantic-entry occurrence names an absent Terminal requirement")?;
            let requirement = module
                .identities
                .text(boundary.identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("semantic-entry occurrence has no exact requirement identity")?;
            let mut matching_rows = entry_rows
                .iter()
                .copied()
                .filter(|(plan, row_requirement)| {
                    selection.selected_requirement_identity(requirement) == *row_requirement
                        && selection.selected_row_matches_requirement(
                            requirement,
                            row_requirement,
                            plan.report_fingerprint(),
                        )
                });
            let Some((plan, row_requirement)) = matching_rows.next() else {
                continue;
            };
            if matching_rows.next().is_some() {
                return Err("semantic-entry transport has competing selected provider rows".into());
            }
            if let Some((_, _, previous_boundary)) = selected
                && previous_boundary != boundary_id
            {
                return Err("semantic-entry transport has competing selected bindings".into());
            }
            selected = Some((plan, row_requirement, boundary_id));
        }
        let Some((plan, requirement, boundary_id)) = selected else {
            return Ok(None);
        };
        let boundary = module
            .boundary_machine(boundary_id)
            .ok_or("semantic-entry transport lost its Terminal requirement")?;
        let application = entry
            .semantic_calling_application()
            .ok_or("semantic-entry transport has no selected calling application")?;
        if entry.checked_entry().terminal_psi_identity() != artifact.manifest().semantic()
            || entry.source().target_slot().owner != target::TargetProfile::UefiX64
            || entry.target() != entry.source().target_slot().owner.native_target()
            || entry.physical_adapter().is_none()
            || entry.source().target_slot().schema
                != target::ProgramEntrySchema::ProgramStorageApplication
            || application.requirement_identity != requirement
            || module.calling_abi_identity(boundary_id)? != application.abi
            || !boundary.scalar_parameters.is_empty()
            || boundary.structural_parameters.len() != 2
            || boundary.result != terminal_psi::BoundaryMachineResult::Unit
        {
            return Err("semantic-entry transport does not join the exact selected artifact, requirement and entry shape".into());
        }
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            entry.source().target_slot().owner.target_name(),
        );
        let callee = selections
            .entry(
                artifact.view().identity(),
                module,
                terminal_psi::TargetMachineCallable::Machine(
                    entry.checked_entry().terminal_entry(),
                ),
            )
            .map_err(|reason| format!("semantic-entry target selection: {reason}"))?;
        terminal_semantics::validate_empty_erased_invocation_inputs(module, callee)?;
        Ok(Some(SelectedProgramEntryCall::settled(
            boundary.id,
            entry.checked_entry().clone(),
            callee,
            plan.identity_digest(),
        )))
    }
}

pub use crate::SelectedProgramEntryCall;
pub use physical_adapter::PhysicalProgramEntry;
pub use semantic_call::bind_selected_program_entry_call;

pub use model::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement,
};

use terminal_psi::CheckedProgramEntryTerminalReceipt;

pub fn validate_native_program_entry_settlement(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_entry: &CheckedProgramEntryTerminalReceipt,
    program_entry: NativeProgramEntrySettlement<'_>,
    target: target::NativeTarget,
) -> Result<ValidatedNativeProgramEntrySettlement, NativeProgramEntrySettlementError> {
    let slot = program_entry.source.target_slot();
    if slot.owner.native_target() != target {
        return Err(NativeProgramEntrySettlementError::TargetDrift);
    }
    let admitted_calling_plans = program_entry
        .validate_for_target(target)
        .map_err(|_| NativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    let module = artifact.semantic_module();
    if module
        .identities
        .text(checked_entry.declaration().identity())
        != Some(program_entry.source.declaration())
    {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    let psi = terminal_codec::terminal_psi_identity(module).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    if psi != checked_entry.terminal_psi_identity()
        || artifact.manifest().semantic() != checked_entry.terminal_psi_identity()
        || program_entry
            .physical_adapter
            .is_some_and(|adapter| adapter.terminal() != psi)
    {
        return Err(NativeProgramEntrySettlementError::TerminalPsiSubstitution);
    }
    let selections = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        slot.owner.target_name(),
    );
    let entry = selections
        .entry(
            artifact.view().identity(),
            module,
            terminal_psi::TargetMachineCallable::Machine(checked_entry.terminal_entry()),
        )
        .map_err(|error| NativeProgramEntrySettlementError::TargetFamily(error.to_string()))?;
    let entry_count = module
        .machines
        .iter()
        .filter(|machine| machine.id == checked_entry.terminal_entry())
        .count();
    if entry_count != 1 {
        return Err(NativeProgramEntrySettlementError::TerminalEntryMultiplicity(entry_count));
    }
    if module
        .machine(checked_entry.terminal_entry())
        .and_then(|machine| machine.declaration.as_ref())
        .is_none_or(|declaration| declaration.machine != *checked_entry.declaration())
    {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    for machine in [checked_entry.terminal_entry(), entry] {
        terminal_semantics::validate_empty_erased_invocation_inputs(module, machine).map_err(
            |reason| NativeProgramEntrySettlementError::UnsuppliedErasedInputs { machine, reason },
        )?;
    }
    Ok(ValidatedNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        physical_adapter: program_entry.physical_adapter.cloned(),
        target,
        source: program_entry.source.clone(),
        semantic_calling_application: program_entry.semantic_calling_application.cloned(),
        admitted_calling_plans,
        physical_calling_application: program_entry.physical_calling_application.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
        access_view_establishments: Vec::new(),
        private_entry_ram: Vec::new(),
    })
}
