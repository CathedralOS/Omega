/*
  Executable work-domain closure selection (`NativeExecutionScope`) and complete
  Terminal-to-native admission (`admit_native_realization`,
  `AdmittedNativeRealization`).

  Selects the reachable machine, boundary, cleanup-hook, callback, and
  installed-interrupt closure from canonical Terminal without mutating the
  canonical module, then sequences input preparation, provider settlement,
  installation, and authority review into one admitted native realization input.
*/
use crate::SelectedProviderAdapter;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{OperationKind, TerminalModule, Terminator};
mod installed_interrupts {
    pub(super) use terminal_semantics::callable_values::installed_interrupt_helper_roots as helper_roots;
}

mod storage {
    use super::NativeExecutionScope;
    use semantic_vocabulary::ScalarApplicationId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::TerminalModule;

    impl NativeExecutionScope {
        pub(super) fn storage_extent_demand(
            &self,
            module: &TerminalModule,
            layouts: &terminal_installation::AdmittedTargetLayouts,
        ) -> Result<BTreeSet<ScalarApplicationId>, String> {
            let mut declarations = module
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect::<BTreeMap<_, _>>();
            for resolved in layouts.resolved_records() {
                declarations.insert(resolved.id, resolved);
            }
            let mut roots = BTreeSet::new();
            for machine in module
                .machines
                .iter()
                .filter(|machine| self.contains_machine(machine.id))
            {
                roots.extend(
                    machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .map(|parameter| parameter.structural_type),
                );
                roots.extend(
                    machine
                        .result
                        .structural()
                        .map(|result| result.structural_type),
                );
                for place in &machine.structural_places {
                    roots.extend(place.kind.produced_structural_type());
                }
                for block in &machine.blocks {
                    for operation in &block.operations {
                        roots.extend(
                            operation
                                .result
                                .structural()
                                .map(|result| result.structural_type),
                        );
                        for storage in operation.kind.storage_uses() {
                            terminal_semantics::storage_operand_dependencies(
                                machine,
                                &declarations,
                                storage,
                                &mut roots,
                            )
                            .map_err(|reason| {
                                format!("native storage demand for {:?}: {reason}", operation.id)
                            })?;
                        }
                    }
                    for projection in block.terminator.storage_uses() {
                        terminal_semantics::storage_operand_dependencies(
                            machine,
                            &declarations,
                            projection.into(),
                            &mut roots,
                        )
                        .map_err(|reason| {
                            format!(
                                "native terminator storage demand for {:?}: {reason}",
                                machine.id
                            )
                        })?;
                    }
                }
            }
            for boundary in module
                .boundary_machines
                .iter()
                .filter(|boundary| self.contains_boundary(boundary.id))
            {
                roots.extend(
                    boundary
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.structural_type),
                );
                if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
                    roots.insert(result.structural_type);
                }
            }
            calling_conventions::terminal_layout::StructuralLayouts::new(declarations.into_values())
                .map_err(|error| format!("native storage declarations: {error:?}"))?
                .required_scalar_extents(roots)
                .map_err(|error| format!("native storage extent demand: {error:?}"))
        }
    }
}

#[derive(Debug)]
pub(crate) enum NativeExecutionScopeError {
    TargetFamily(String),
    InstalledInterrupt(String),
    MissingMachine(MachineId),
    InvalidMachineDescriptor {
        descriptor: semantic_vocabulary::MachineDescriptorId,
        reason: &'static str,
    },
    MissingBoundary(BoundaryMachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetFamily(error) | Self::InstalledInterrupt(error) => {
                formatter.write_str(error)
            }
            Self::InvalidMachineDescriptor { descriptor, reason } => write!(
                formatter,
                "native machine selection {descriptor:?}: {reason}"
            ),
            Self::MissingMachine(machine) => write!(
                formatter,
                "native execution scope reaches absent machine {machine:?}"
            ),
            Self::MissingBoundary(boundary) => write!(
                formatter,
                "native execution scope reaches absent boundary {boundary:?}"
            ),
            Self::MissingSelectedCheckedProvider(boundary) => write!(
                formatter,
                "native execution scope has no selected checked provider for {boundary:?}"
            ),
            Self::CompetingSelectedCheckedProviders(boundary) => write!(
                formatter,
                "native execution scope has competing selected checked providers for {boundary:?}"
            ),
        }
    }
}

impl std::error::Error for NativeExecutionScopeError {}

pub(crate) struct NativeExecutionScope {
    machines: Vec<MachineId>,
    boundaries: Vec<BoundaryMachineId>,
    boundaries_set: std::collections::BTreeSet<BoundaryMachineId>,
    selected_candidate_positions: Vec<usize>,
}

struct ScopeBoundaryIndex<'a> {
    boundary_set: std::collections::BTreeSet<BoundaryMachineId>,
    candidates_by_boundary: std::collections::BTreeMap<
        BoundaryMachineId,
        Vec<(usize, &'a terminal_psi::ProviderCandidateConformance)>,
    >,
    selected_by_req: std::collections::BTreeMap<&'a str, Vec<&'a SelectedProviderAdapter>>,
}

impl NativeExecutionScope {
    pub(crate) fn derive(
        module: &TerminalModule,
        entry: MachineId,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        selected: &[SelectedProviderAdapter],
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        callbacks: &[backend_plan::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, NativeExecutionScopeError> {
        let mut function_positions = module
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| {
                module.target_machine_families.iter().all(|family| {
                    let callable = terminal_psi::TargetMachineCallable::Machine(machine.id);
                    !family
                        .members
                        .iter()
                        .any(|member| member.callable == callable)
                        || targets
                            .member(family.canonical)
                            .is_ok_and(|selected| selected.callable() == callable)
                })
            })
            .map(|(position, machine)| (machine.id, position))
            .collect::<Vec<_>>();
        function_positions.sort_unstable_by_key(|(machine, _)| *machine);
        let boundary_set = module
            .boundary_machines
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut candidates_by_boundary = std::collections::BTreeMap::<
            BoundaryMachineId,
            Vec<(usize, &terminal_psi::ProviderCandidateConformance)>,
        >::new();
        for (pos, candidate) in module.provider_candidates.iter().enumerate() {
            candidates_by_boundary
                .entry(candidate.boundary)
                .or_default()
                .push((pos, candidate));
        }
        let mut selected_by_req =
            std::collections::BTreeMap::<&str, Vec<&SelectedProviderAdapter>>::new();
        for row in selected {
            selected_by_req
                .entry(row.requirement_identity.as_str())
                .or_default()
                .push(row);
        }
        let index = ScopeBoundaryIndex {
            boundary_set,
            candidates_by_boundary,
            selected_by_req,
        };
        let mut visited = vec![false; module.machines.len()];
        let mut pending = vec![entry];
        pending.extend(
            module
                .dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().flatten())
                .copied(),
        );
        let mut scope = Self {
            machines: Vec::new(),
            boundaries: Vec::new(),
            boundaries_set: std::collections::BTreeSet::new(),
            selected_candidate_positions: Vec::new(),
        };
        while let Some(machine) = pending.pop() {
            let position = function_positions
                .binary_search_by_key(&machine, |(identity, _)| *identity)
                .map_err(|_| NativeExecutionScopeError::MissingMachine(machine))?;
            let function_position = function_positions[position].1;
            if std::mem::replace(&mut visited[function_position], true) {
                continue;
            }
            scope.machines.push(machine);
            let body = &module.machines[function_position];
            for block in &body.blocks {
                for operation in &block.operations {
                    if let OperationKind::BoundaryCall { .. } = operation.kind {
                        pending.extend(
                            installed_interrupts::helper_roots(module, operation.id)
                                .map_err(NativeExecutionScopeError::InstalledInterrupt)?,
                        );
                        pending.extend(
                            callbacks
                                .iter()
                                .filter(|callback| callback.terminal_operation() == operation.id)
                                .map(|callback| callback.callback_thunk_artifact().entry()),
                        );
                    }
                    if let Some(invocation) = operation.kind.module_invocation() {
                        pending.push(invocation.callee);
                    }
                    match &operation.kind {
                        OperationKind::SelectMachine { descriptor } => {
                            let invalid =
                                |reason| NativeExecutionScopeError::InvalidMachineDescriptor {
                                    descriptor: *descriptor,
                                    reason,
                                };
                            let selected = module
                                .callables
                                .descriptor(*descriptor)
                                .ok_or_else(|| invalid("selected descriptor is absent"))?;
                            if operation.result.scalar().map(|value| value.scalar_type)
                                != Some(semantic_vocabulary::ScalarType::Callable(
                                    selected.contract,
                                ))
                                || module.callables.contract(selected.contract).is_none()
                            {
                                return Err(invalid(
                                    "selected descriptor changes its callable contract",
                                ));
                            }
                            pending.push(selected.machine);
                        }
                        OperationKind::EstablishNativeCodeLayout { machine } => {
                            pending.push(*machine)
                        }
                        OperationKind::BoundaryCall { boundary, .. } => {
                            scope.retain_boundary(
                                module,
                                targets,
                                entry_call,
                                &index,
                                assumptions,
                                &function_positions,
                                *boundary,
                                &mut pending,
                            )?;
                        }
                        _ => {}
                    }
                }
                for (_, cleanup) in block.terminator.cleanup_rosters() {
                    for action in cleanup {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        {
                            pending.push(cleanup.cleanup_machine);
                        }
                    }
                }
                if let Some(invocation) = block.terminator.module_invocation() {
                    pending.push(invocation.callee);
                }
                if let Terminator::ExternalCompletion { boundary, .. } = block.terminator {
                    scope.retain_boundary(
                        module,
                        targets,
                        entry_call,
                        &index,
                        assumptions,
                        &function_positions,
                        boundary,
                        &mut pending,
                    )?;
                }
            }
        }
        scope.machines.sort_unstable();
        scope.boundaries.sort_unstable();
        scope.selected_candidate_positions.sort_unstable();
        Ok(scope)
    }

    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        index: &ScopeBoundaryIndex<'_>,
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        function_positions: &[(MachineId, usize)],
        boundary: BoundaryMachineId,
        pending: &mut Vec<MachineId>,
    ) -> Result<(), NativeExecutionScopeError> {
        if self.boundaries_set.contains(&boundary) {
            return Ok(());
        }
        if !index.boundary_set.contains(&boundary) {
            return Err(NativeExecutionScopeError::MissingBoundary(boundary));
        }
        self.boundaries_set.insert(boundary);
        self.boundaries.push(boundary);
        if let Some(entry) = entry_call.filter(|entry| entry.boundary() == boundary) {
            pending.push(entry.entry());
            return Ok(());
        }
        if let Some((_, supplied)) = assumptions
            .iter()
            .find(|(required, _)| *required == boundary)
        {
            return self.retain_boundary(
                module,
                targets,
                entry_call,
                index,
                assumptions,
                function_positions,
                *supplied,
                pending,
            );
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) =
            terminal_installation::admitted_boundary_member(module, Some(targets), boundary)
                .map_err(NativeExecutionScopeError::TargetFamily)?
        {
            pending.push(member);
            return Ok(());
        }
        let Some(candidates) = index.candidates_by_boundary.get(&boundary) else {
            return Ok(());
        };
        if !candidates.iter().any(|(_, candidate)| {
            module
                .identities
                .text(candidate.requirement_identity)
                .is_some_and(|req| index.selected_by_req.contains_key(req))
        }) {
            return Ok(());
        }
        let mut matches = candidates.iter().filter(|(_, candidate)| {
            function_positions
                .binary_search_by_key(&candidate.candidate, |(machine, _)| *machine)
                .is_ok()
                && module
                    .identities
                    .text(candidate.requirement_identity)
                    .and_then(|req| index.selected_by_req.get(req))
                    .is_some_and(|rows| {
                        rows.iter().any(|row| {
                            module.identities.text(candidate.provider_identity)
                                == Some(row.provider_identity.as_str())
                                && module
                                    .identities
                                    .text(candidate.provider_declaration.identity())
                                    == Some(row.provider_declaration.as_str())
                                && module.identities.text(candidate.candidate_identity)
                                    == Some(row.machine_identity.as_str())
                        })
                    })
        });
        let candidate =
            matches
                .next()
                .ok_or(NativeExecutionScopeError::MissingSelectedCheckedProvider(
                    boundary,
                ))?;
        if matches.next().is_some() {
            return Err(NativeExecutionScopeError::CompetingSelectedCheckedProviders(boundary));
        }
        self.selected_candidate_positions.push(candidate.0);
        pending.push(candidate.1.candidate);
        Ok(())
    }

    pub(crate) fn machines(&self) -> &[MachineId] {
        &self.machines
    }

    pub(crate) fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
    }

    pub(crate) fn scalar_value_demand(
        &self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut demanded = std::collections::BTreeSet::new();
        for machine in module
            .machines
            .iter()
            .filter(|machine| self.contains_machine(machine.id))
        {
            for operation in machine.operations() {
                if let OperationKind::ReadScalarApplication { application } = operation.kind {
                    demanded.insert(application);
                }
            }
        }
        let catalog = &module.scalar_applications;
        for (position, declaration) in catalog.declarations().iter().enumerate() {
            let terminal_psi::ScalarApplication::Evaluate { entry, .. } = declaration else {
                continue;
            };
            let machine = match entry {
                terminal_psi::TargetMachineCallable::Machine(machine) => *machine,
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    let Ok(member) = targets.member(*entry) else {
                        continue;
                    };
                    let terminal_psi::TargetMachineCallable::Machine(machine) = member.callable()
                    else {
                        continue;
                    };
                    machine
                }
            };
            if !self.contains_machine(machine) {
                continue;
            }
            let application = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(semantic_vocabulary::ScalarApplicationId::new)
                .ok_or("native scalar application coordinate exceeds its catalog")?;
            if catalog.conditions.contains_subject(
                terminal_psi::ScalarConditionSubject::Application(application),
            ) {
                demanded.insert(application);
            }
        }
        Ok(demanded.into_iter().collect())
    }

    pub(crate) fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        layouts: &terminal_installation::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut applications = self.storage_extent_demand(module, layouts)?;
        applications.extend(self.scalar_value_demand(module, targets)?);
        Ok(applications.into_iter().collect())
    }

    pub(crate) fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub(crate) fn contains_candidate_position(&self, position: usize) -> bool {
        self.selected_candidate_positions
            .binary_search(&position)
            .is_ok()
    }
}

pub(crate) mod native_admission {
    use crate::providers::provider_admission::{
        AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers,
        realization_error,
    };
    use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
    use boundary_applications::TerminalBoundaryApplicationCoverage;
    use diagnostics::Diagnostic;
    use semantic_vocabulary::BoundaryMachineId;
    use std::collections::BTreeSet;
    use terminal_codec::CanonicalTerminalArtifact;

    #[derive(Clone, Copy)]
    pub struct NativeAdmissionRequest<'request, 'execution> {
        pub entry: terminal_psi::TargetMachineCallable,
        pub private_entry_ram: &'request [terminal_installation::CheckedPrivateEntryRamRecipe],
        pub providers: ProviderAdmissionRequest<'request, 'execution>,
        pub boundary_application_coverage: Option<&'request TerminalBoundaryApplicationCoverage>,
        pub behavior_exclusions: &'request effects::BehaviorExclusions,
    }

    pub struct AdmittedNativeRealization<'execution> {
        pub input: VerifiedNativeArtifactInput,
        pub providers: AdmittedNativeProviders<'execution>,
        pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
    }

    pub fn admit_native_realization<'execution>(
        artifact: &CanonicalTerminalArtifact,
        request: &NativeAdmissionRequest<'_, 'execution>,
    ) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
        let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
            artifact,
            request.providers.profile,
            request.entry,
            request.private_entry_ram,
            &crate::NativeExecutionSelection {
                scalar_applications: None,
                providers: request.providers.checked_adapters,
                plans: request.providers.selected_provider_plans,
                callbacks: &[],
                entry_call: None,
            },
        )
        .map_err(artifact_lowering_error)?;
        let input = input
            .bind_selected_provider_coordinates(artifact, request.providers.selected_provider_plans)
            .map_err(|error| realization_error("selected provider coordinates", error))?;
        admit_prepared_native_realization(input, artifact, request)
    }

    pub fn admit_prepared_native_realization<'execution>(
        input: VerifiedNativeArtifactInput,
        artifact: &CanonicalTerminalArtifact,
        request: &NativeAdmissionRequest<'_, 'execution>,
    ) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
        let executed_boundaries: BTreeSet<BoundaryMachineId> = input
            .plan()
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect();
        let module_boundaries: BTreeSet<BoundaryMachineId> = input
            .context()
            .module()
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect();
        let settlements = request
            .providers
            .settlements
            .iter()
            .copied()
            .filter(|settlement| {
                let mut matching = input
                    .context()
                    .module()
                    .boundary_machines
                    .iter()
                    .filter(|boundary| {
                        request
                            .providers
                            .selected_provider_plans
                            .selected_row_matches_requirement(
                                input
                                    .context()
                                    .module()
                                    .identities
                                    .text(boundary.identity)
                                    .unwrap_or_default(),
                                settlement.provider_execution.requirement_identity(),
                                settlement
                                    .provider_execution
                                    .provider_plan_report_identity(),
                            )
                    })
                    .peekable();
                matching.peek().is_none()
                    || matching.any(|boundary| executed_boundaries.contains(&boundary.id))
            })
            .collect::<Vec<_>>();
        let compiler_builtins = request
            .providers
            .compiler_builtins
            .iter()
            .copied()
            .filter(|proposal| {
                executed_boundaries.contains(&proposal.boundary)
                    || !module_boundaries.contains(&proposal.boundary)
            })
            .collect::<Vec<_>>();
        let (native_callbacks, installed_interrupts) = if request
            .providers
            .native_callbacks
            .is_empty()
            && request.providers.installed_interrupts.is_empty()
        {
            (Vec::new(), Vec::new())
        } else {
            let plan_boundary_call_ops: BTreeSet<semantic_vocabulary::OperationId> = input
                .plan()
                .operations()
                .filter_map(|operation| match operation {
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation, ..
                    } => Some(*psi_operation),
                    _ => None,
                })
                .collect();
            let module_boundary_call_ops: BTreeSet<semantic_vocabulary::OperationId> = input
                .context()
                .module()
                .machines
                .iter()
                .flat_map(|machine| machine.operations())
                .filter_map(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::BoundaryCall { .. }
                    )
                    .then_some(operation.id)
                })
                .collect();
            let in_scope = |operation| {
                plan_boundary_call_ops.contains(&operation)
                    || !module_boundary_call_ops.contains(&operation)
            };
            let native_callbacks = request
                .providers
                .native_callbacks
                .iter()
                .filter(|callback| in_scope(callback.terminal_operation))
                .cloned()
                .collect::<Vec<_>>();
            let mut callback_occurrences = vec![None; request.providers.callback_occurrences.len()];
            for occurrence in request.providers.callback_occurrences {
                let Some(slot) = callback_occurrences.get_mut(occurrence.placement_index()) else {
                    return Err(realization_error(
                        "installed interrupt callback scope",
                        "callback placement falls outside the original complete roster",
                    ));
                };
                if slot.replace(occurrence).is_some() {
                    return Err(realization_error(
                        "installed interrupt callback scope",
                        "original callback placement identity is duplicated",
                    ));
                }
            }
            let installed_interrupts = request
                .providers
                .installed_interrupts
                .iter()
                .filter(|entry| {
                    in_scope(entry.operation)
                        || callback_occurrences
                            .get(entry.placement_index)
                            .copied()
                            .flatten()
                            .is_some_and(|occurrence| {
                                occurrence.callback_thunk_artifact().entry() == input.plan().entry
                                    && occurrence.terminal_operation() == entry.operation
                                    && occurrence.application_index()
                                        == entry.placement.application_index
                                    && occurrence.callback_thunk_identity()
                                        == entry.callback_function
                            })
                })
                .cloned()
                .collect::<Vec<_>>();
            (native_callbacks, installed_interrupts)
        };
        let scoped_providers = ProviderAdmissionRequest {
            profile: request.providers.profile,
            bootstrap_entry: request.providers.bootstrap_entry,
            program_provider_context: request.providers.program_provider_context,
            installation_assumptions: request.providers.installation_assumptions,
            selected_provider_plans: request.providers.selected_provider_plans,
            checked_adapters: request.providers.checked_adapters,
            external_binding_rows: request.providers.external_binding_rows,
            settlements: &settlements,
            compiler_builtins: &compiler_builtins,
            native_callbacks: &native_callbacks,
            installed_interrupts: &installed_interrupts,
            installed_calling: request.providers.installed_calling,
            callback_occurrences: request.providers.callback_occurrences,
            terminal_authority_policy: request.providers.terminal_authority_policy,
            terminal_authority_permission_policy: request
                .providers
                .terminal_authority_permission_policy,
            x86_scalar_fma_provider: request.providers.x86_scalar_fma_provider,
        };
        let mut providers =
            admit_native_providers(&input.admission_source(), artifact, &scoped_providers)?;
        request
            .behavior_exclusions
            .validate_physical_authority_closure(providers.terminal_authority_closure_review())?;
        let mut input = input;
        input.retain_admitted_assumed_bindings(
            providers
                .installation
                .as_ref()
                .map(|installation| {
                    installation
                        .terminal_selection()
                        .assumed_bindings()
                        .iter()
                        .map(
                            |row| installation_evidence::InstalledAssumedBindingEvidence {
                                required: row.required,
                                supplied_requirement: row.supplied_requirement,
                                implementation: row.implementation,
                                selected_requirement: row.selected_requirement,
                                supply_path: row.supply_path.clone(),
                            },
                        )
                        .collect()
                })
                .unwrap_or_default(),
        );
        input.retain_admitted_native_callbacks(providers.take_native_callbacks());
        let installed_interrupts = providers.take_installed_interrupts();
        if installed_interrupts.psi() != input.plan().psi {
            return Err(vec![Diagnostic::error(
                "installed interrupt entries belong to another Terminal program",
            )]);
        }
        input.retain_admitted_installed_interrupts(installed_interrupts);
        Ok(AdmittedNativeRealization {
            input,
            providers,
            boundary_application_coverage: request.boundary_application_coverage.cloned(),
        })
    }

    pub fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
        match error {
            ArtifactLoweringError::SemanticDecode(error) => {
                realization_error("semantic section", error)
            }
            error => realization_error("native artifact lowering", error),
        }
    }
}
