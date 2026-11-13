#![forbid(unsafe_code)]

/*
  Omega Stage 02 (`02_abstract-operations-to-target-operations`): target-aware
  lowering of `AbstractOperationPlan` into `TargetOperationPlan`
  (`lower_optimized_to_target_operations`, `ValidatedOptimizedTargetOperations`,
  `OptimizedTargetLoweringRequest`).

  We lower validated abstract operations into target control-flow graphs with
  evaluated ABI calling plans, concrete structural byte layouts, and settled
  boundary realizations before instruction selection. `coordination` binds
  admitted boundary settlements, provider installation evidence, and native
  callback arguments once across the plan; `function_signature` and
  `structural_layout` derive target calling conventions, value shapes, and byte
  offsets; `control_flow` (`control_flow_operations`, `control_flow_calls`,
  `control_flow_storage`, `references`) lowers every function through a single
  dominating-definition control graph; and `scalar` and `unit` lower scalar
  expressions, byte/element view descriptors, and boundary/foreign call
  transports.
*/

/*
  Target operation lowering separates target-wide ABI and byte-layout decisions
  from ISA-specific instruction selection. By resolving structural sizes, field
  offsets, calling-convention placements, and reference descriptor storage on a
  single block-structured target graph here, Omega Stage 03
  (`03_target-operations-to-selected-instructions`) can legalize operands and
  select machine instructions without re-evaluating source signatures or
  provider settlements.
*/

/*
  Consumes `StagedOptimizedAbstractOperationPlan` from Omega Stage 01
  (`01_abstract-operations-to-abstract-operations`) alongside admitted boundary
  settlements, provider installation receipts, and target selection from Omega
  Stage 00 (`00_terminal-psi-to-abstract-operations`). Produces
  `ValidatedOptimizedTargetOperations` (`TargetOperationPlan`) for Omega Stage
  03 (`03_target-operations-to-selected-instructions`).
*/

pub mod access_view_inputs {}
pub(crate) mod control_flow;
mod coordination;
mod function_signature;
pub(crate) mod model;
mod scalar;
pub(crate) mod structural_layout;
mod unit;

pub(crate) use function_signature::{
    borrowed_storage, scalar_abi, structural_signature, structural_type_lookup,
};

pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError,
};
pub use optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub(crate) mod boundary_settlements {
    use abstract_operations::{AbstractFunction, CompletionClaimSource};
    use semantic_vocabulary::{PlaceId, ValueId};
    use std::collections::{BTreeMap, BTreeSet};
    use target_operations::TargetStructuralParameter;

    pub(super) fn claim_completion_only_boundary_is_exact(
        function: &AbstractFunction,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        scalar_arguments: &[ValueId],
        structural_arguments: &[terminal_psi::StructuralArgument],
        completion_claim_sources: &[CompletionClaimSource],
        completion_receipts: &[terminal_psi::CompletionReceipt],
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    ) -> bool {
        if !scalar_arguments.is_empty()
            || !declaration.scalar_parameters.is_empty()
            || !declaration.result.is_unit()
            || !declaration.program_local_root_introductions.is_empty()
            || !declaration.content_guarantees.is_empty()
            || !declaration.published_service_ceiling.is_empty()
            || structural_arguments.is_empty()
            || structural_arguments.len() != declaration.structural_parameters.len()
            || declaration.requires.iter().any(|requirement| {
                requirement.argument_index as usize >= declaration.structural_parameters.len()
            })
            || completion_receipts.is_empty()
            || completion_claim_sources
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || completion_receipts
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return false;
        }

        for (index, (argument, boundary_parameter)) in structural_arguments
            .iter()
            .zip(&declaration.structural_parameters)
            .enumerate()
        {
            let Some(source) = parameters_by_place.get(&argument.place).copied() else {
                return false;
            };
            let Some(caller_parameter) = function.structural_parameter(argument.place) else {
                return false;
            };
            let mut expected_qualifications = boundary_parameter.qualifications.clone();
            let mut expected_projected = boundary_parameter.projected_qualifications.clone();
            for requirement in declaration
                .requires
                .iter()
                .filter(|requirement| requirement.argument_index as usize == index)
            {
                if requirement.path.is_empty() {
                    expected_qualifications.push(requirement.domain);
                } else {
                    expected_projected.push(terminal_psi::StructuralPathQualification {
                        path: requirement.path.clone(),
                        domain: requirement.domain,
                    });
                }
            }
            expected_projected.sort();
            expected_projected.dedup();
            expected_qualifications.sort_unstable();
            expected_qualifications.dedup();
            if !argument.path.is_empty()
                || argument.access != terminal_psi::StructuralAccess::Owned
                || source.access != terminal_psi::StructuralAccess::Owned
                || boundary_parameter.access != terminal_psi::StructuralAccess::Owned
                || source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                || boundary_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                || boundary_parameter.position != index as u32
                || source.structural_type != boundary_parameter.structural_type
                || caller_parameter.qualifications != expected_qualifications
                || caller_parameter.projected_qualifications != expected_projected
            {
                return false;
            }
        }

        let canonical_sources = function
            .entry_claims
            .iter()
            .cloned()
            .map(|entry| CompletionClaimSource {
                claim: entry.claim,
                entry: Some(entry),
                content: None,
            })
            .collect::<Vec<_>>();
        if completion_claim_sources != canonical_sources {
            return false;
        }

        let expected = structural_arguments
            .iter()
            .enumerate()
            .flat_map(|(argument_index, argument)| {
                completion_claim_sources.iter().filter_map(move |source| {
                    (source.input() == argument.place)
                        .then_some((argument_index as u32, source.claim))
                })
            })
            .collect::<BTreeSet<_>>();
        let actual = completion_receipts
            .iter()
            .map(|receipt| (receipt.argument_index, receipt.claim))
            .collect::<BTreeSet<_>>();
        expected == actual && actual.len() == completion_receipts.len()
    }
}

pub(crate) mod function {
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
    use std::collections::BTreeMap;
    use target::NativeTarget;
    use target_operations::{BoundarySettlementBinding, ScalarFunctionAbi, TargetFunction};

    mod native_boundaries {
        use super::{
            AbstractFunction, BTreeMap, BoundaryMachineId, BoundarySettlementBinding, OperationId,
        };
        use abstract_operations::AbstractOperation;
        use target_operations::BoundaryRealization;
        pub(super) fn unsupported_scalar_call(
            function: &AbstractFunction,
            settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
            selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
        ) -> Option<(OperationId, BoundaryMachineId)> {
            function.operations.iter().find_map(|operation| {
                let AbstractOperation::BoundaryCall {
                    psi_operation,
                    boundary,
                    arguments,
                    ..
                } = operation
                else {
                    return None;
                };
                (!arguments.is_empty()
                    && !selected_calls.contains_key(&(
                        function.machine,
                        (*psi_operation).into(),
                        *boundary,
                    ))
                    && !matches!(
                        settlements
                            .get(boundary)
                            .map(|binding| &binding.realization),
                        Some(target_operations::BoundarySettlementRealization::Builtin(
                            BoundaryRealization::HostedExitProcessI32(_)
                                | BoundaryRealization::HostedWriteByteI32(_)
                                | BoundaryRealization::NearestIeeeFloatArithmetic(_)
                                | BoundaryRealization::AccessorScalarAccess(_)
                        )) | Some(
                            target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                                _
                            )
                        )
                    ))
                .then_some((*psi_operation, *boundary))
            })
        }
    }

    pub(super) fn lower_function(
        function: &AbstractFunction,
        target: NativeTarget,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        callables: &terminal_psi::TerminalCallableCatalog,
        borrowed_reference_sources: super::control_flow::references::BorrowedSources,
        stored_reference_effects: super::control_flow::references::StoredEffectCatalog,
        scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
        structural_types: &StructuralTypeLookup<'_>,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
        native_callbacks: &BTreeMap<
            OperationId,
            Vec<target_operations::TargetNativeCallbackArgument>,
        >,
        installed_interrupts: Option<&provider_admission::AdmittedInstalledInterruptCohort>,
    ) -> Result<TargetFunction, LoweringError> {
        if let Some((operation, boundary)) =
            native_boundaries::unsupported_scalar_call(function, settlements, selected_calls)
        {
            return Err(
                LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                    machine: function.machine,
                    operation,
                    boundary,
                },
            );
        }
        super::control_flow::lower(
            function,
            target,
            functions,
            dynamic_dispatch,
            callables,
            borrowed_reference_sources,
            stored_reference_effects,
            structural_types,
            boundary_machines,
            settlements,
            selected_calls,
            scalar_abis,
            native_callbacks,
            installed_interrupts,
        )
    }
}

pub(crate) mod optimized {
    use optimization_core::OptimizationExecutionPhase;
    use optimization_unit::OptimizedAbstractProgram;
    use provider_admission::AdmittedProviderInstallation;
    use std::sync::Arc;
    use target::NativeTarget;
    use target_operations::TargetOperationPlan;
    use verified_optimization_input::AdmittedLoweringFacts;

    use crate::{AdmittedBoundarySettlement, LoweringError};

    #[derive(Debug)]
    pub struct ValidatedOptimizedTargetOperations {
        pub(super) optimized: OptimizedAbstractProgram,
        lowering_facts: AdmittedLoweringFacts,
        current_program: Arc<TargetOperationPlan>,
        installed_stack_requirements:
            Vec<installation_evidence::InstalledInterruptStackRequirement>,
        installed_entry_execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
        pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
    }

    impl ValidatedOptimizedTargetOperations {
        pub fn with_installed_entry_execution(
            mut self,
            execution: provider_admission::AdmittedInstalledEntryExecution,
        ) -> Result<Self, &'static str> {
            let entry = execution.entry().entry();
            if self.installed_entry_execution.is_some()
                || entry.psi() != self.current_program.psi
                || entry.target() != self.current_program.target
                || entry.machine() != self.current_program.entry
            {
                return Err("installed execution differs from the current private object");
            }
            self.installed_entry_execution = Some(execution);
            Ok(self)
        }

        pub fn installed_entry_execution(
            &self,
        ) -> Option<&provider_admission::AdmittedInstalledEntryExecution> {
            self.installed_entry_execution.as_ref()
        }

        pub fn with_installed_stack_requirements(
            mut self,
            mut requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
        ) -> Result<Self, &'static str> {
            let cohort = self.lowering_facts.installed_interrupts();
            let mut entries: Vec<_> = cohort
                .entries()
                .iter()
                .map(|entry| (entry.operation, entry.placement_index))
                .collect();
            entries.sort_unstable();
            requirements.sort_unstable_by_key(|row| (row.operation, row.placement_index));
            if entries.windows(2).any(|pair| pair[0] == pair[1])
                || requirements.windows(2).any(|pair| {
                    (pair[0].operation, pair[0].placement_index)
                        == (pair[1].operation, pair[1].placement_index)
                })
            {
                return Err("installed stack requirement or admitted placement is duplicated");
            }
            for requirement in &requirements {
                if requirement.psi != self.current_program.psi
                    || requirement.target != self.current_program.target
                    || requirement.required_bytes().is_none()
                    || requirement.alignment == 0
                    || !requirement.alignment.is_power_of_two()
                    || entries
                        .binary_search(&(requirement.operation, requirement.placement_index))
                        .is_err()
                {
                    return Err("installed stack requirement has no matching admitted entry");
                }
            }
            self.installed_stack_requirements = requirements;
            Ok(self)
        }

        pub fn installed_stack_requirements(
            &self,
        ) -> &[installation_evidence::InstalledInterruptStackRequirement] {
            &self.installed_stack_requirements
        }

        pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
            &self.current_program.borrowed_storage_layouts
        }
        pub const fn optimized(&self) -> &OptimizedAbstractProgram {
            &self.optimized
        }

        pub const fn lowering_facts(&self) -> &AdmittedLoweringFacts {
            &self.lowering_facts
        }

        pub fn boundary_origins(&self) -> target_operations::BoundaryOriginFacts<'_> {
            let input = &self.lowering_facts;
            target_operations::BoundaryOriginFacts {
                program_entry: input.program_entry().map(|call| {
                    target_operations::ProgramEntryOrigin {
                        boundary: call.boundary(),
                        entry: call.entry(),
                        source_signature: call.source_signature(),
                    }
                }),
                assumed_bindings: input.assumed_bindings(),
                target_machine_selections: input.target_machine_selections(),
                module: input.module(),
            }
        }

        pub fn target(&self) -> NativeTarget {
            self.current_program.target
        }

        pub fn target_operations(&self) -> &TargetOperationPlan {
            &self.current_program
        }

        pub fn shared_program(&self) -> Arc<TargetOperationPlan> {
            Arc::clone(&self.current_program)
        }

        pub fn provider_installation(&self) -> Option<&AdmittedProviderInstallation> {
            self.provider_installation.as_deref()
        }
    }

    pub struct OptimizedTargetLoweringRequest<'a> {
        pub target: NativeTarget,
        pub settlements: &'a [AdmittedBoundarySettlement<'a>],
        pub installation: Option<AdmittedProviderInstallation>,
    }

    pub fn lower_optimized_to_target_operations(
        optimized: OptimizedAbstractProgram,
        lowering_facts: AdmittedLoweringFacts,
        request: OptimizedTargetLoweringRequest<'_>,
    ) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
        let OptimizedTargetLoweringRequest {
            target,
            settlements,
            installation,
        } = request;
        let phase = OptimizationExecutionPhase::TargetOperations;
        if !optimized.selections().for_phase(phase).is_empty() {
            return Err(LoweringError::UnconsumedOptimizationPhase(phase));
        }
        lowering_facts
            .require_lowering_context(optimized.plan(), target)
            .map_err(LoweringError::ForeignLoweringFacts)?;
        if lowering_facts.native_callbacks().psi() != optimized.plan().psi {
            return Err(LoweringError::ForeignNativeCallbackCohort);
        }
        if lowering_facts.installed_interrupts().psi() != optimized.plan().psi {
            return Err(LoweringError::ForeignInstalledInterruptCohort);
        }
        let installed = installation
            .as_ref()
            .map(|value| value as &dyn installation_evidence::ProviderInstallationEvidence);
        let bindings =
            super::provider_evidence::bind_provider_executions(optimized.plan(), settlements)?;
        let program =
            super::coordination::lower_to_target_operations_with_settlements_and_installation(
                optimized.plan(),
                target,
                &bindings,
                installed,
                lowering_facts.native_callbacks().callbacks(),
                Some(&lowering_facts),
            )?;
        Ok(ValidatedOptimizedTargetOperations {
            optimized,
            lowering_facts,
            current_program: Arc::new(program),
            installed_stack_requirements: Vec::new(),
            installed_entry_execution: None,
            provider_installation: installation.map(Box::new),
        })
    }
}

pub(crate) mod provider_evidence {
    use crate::AdmittedBoundaryExecution;
    use crate::{AdmittedBoundarySettlement, LoweringError};
    use abstract_operations::AbstractOperationPlan;
    use target_operations::BoundarySettlementBinding;

    pub(super) fn bind_provider_executions(
        plan: &AbstractOperationPlan,
        settlements: &[AdmittedBoundarySettlement<'_>],
    ) -> Result<Vec<BoundarySettlementBinding>, LoweringError> {
        settlements
            .iter()
            .map(|settlement| {
                terminal_psi::find_boundary_machine(&plan.boundary_machines, settlement.boundary)
                    .ok_or(LoweringError::UnknownBoundarySettlement(
                    settlement.boundary,
                ))?;
                let execution = match settlement.execution {
                    AdmittedBoundaryExecution::Provider(provider_execution) => {
                        let provider_plan = target_operations::ProviderPlanReportIdentity::new(
                            provider_execution.provider_plan_report_identity(),
                        )
                        .ok_or_else(|| {
                            LoweringError::ProviderExecutionBinding("zero provider plan".into())
                        })?;
                        target_operations::ProviderExecutionBinding::from_execution_record(
                            provider_plan,
                            provider_execution.provider_execution_report_identity(),
                            provider_execution.provider_execution_report_fingerprint(),
                            provider_execution.normalized_root_report_identity(),
                            provider_execution.boundary_contract_report_fingerprint(),
                        )
                        .ok_or_else(|| {
                            LoweringError::ProviderExecutionBinding(
                                "admitted provider execution contains a zero identity".into(),
                            )
                        })?
                        .into()
                    }
                    AdmittedBoundaryExecution::CompilerBuiltin(settled) => {
                        target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                            settled.execution(),
                        )
                    }
                };
                Ok(BoundarySettlementBinding {
                    boundary: settlement.boundary,
                    execution,
                    realization: settlement.realization.clone(),
                })
            })
            .collect()
    }
}

pub(crate) mod selected_boundary_calls {
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
    use installation_evidence::InstalledProviderCallEvidence;
    use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId};
    use std::collections::BTreeMap;
    use target_operations::NativeCallOrigin;
    use terminal_psi::{TargetMachineCallable, TargetMachineSupply};

    pub(crate) struct SelectedBoundaryCall {
        pub caller: MachineId,
        pub invocation_site: InvocationSite,
        pub boundary: BoundaryMachineId,
        pub callee: MachineId,
        pub result: AbstractBoundaryResult,
        pub origin: NativeCallOrigin,
    }

    pub(crate) type SelectedCallsByCall =
        BTreeMap<(MachineId, InvocationSite, BoundaryMachineId), SelectedBoundaryCall>;

    pub(crate) fn index(
        plan: &AbstractOperationPlan,
        installed: BTreeMap<
            (MachineId, InvocationSite, BoundaryMachineId),
            InstalledProviderCallEvidence,
        >,
        input: Option<&verified_optimization_input::AdmittedLoweringFacts>,
    ) -> Result<SelectedCallsByCall, LoweringError> {
        let mut selected = SelectedCallsByCall::new();
        for function in &plan.functions {
            for operation in &function.operations {
                let (
                    invocation_site,
                    boundary,
                    result,
                    completion_claim_sources,
                    completion_receipts,
                ) = match operation {
                    AbstractOperation::BoundaryCall {
                        psi_operation,
                        boundary,
                        result,
                        completion_claim_sources,
                        completion_receipts,
                        ..
                    } => (
                        (*psi_operation).into(),
                        boundary,
                        result,
                        completion_claim_sources.as_slice(),
                        completion_receipts.as_slice(),
                    ),
                    AbstractOperation::ExternalCompletion {
                        psi_edge, boundary, ..
                    } => (
                        InvocationSite::ExternalCompletion(*psi_edge),
                        boundary,
                        &AbstractBoundaryResult::Unit,
                        &[][..],
                        &[][..],
                    ),
                    _ => continue,
                };
                let key = (function.machine, invocation_site, *boundary);
                let invalid = || LoweringError::SelectedBoundaryCallEvidenceMismatch {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                };
                let installed_call = installed.get(&key);
                let family = input.and_then(|input| {
                    input
                        .module()
                        .target_machine_families
                        .iter()
                        .find(|family| {
                            family.canonical == TargetMachineCallable::Boundary(*boundary)
                                && !family
                                    .members
                                    .iter()
                                    .any(|member| member.callable == family.canonical)
                        })
                });
                let entry_call = input
                    .and_then(|input| input.program_entry())
                    .filter(|entry| entry.boundary() == *boundary);
                let (callee, origin) = if let Some(entry) = entry_call {
                    if installed_call.is_some()
                        || family.is_some()
                        || plan.function(entry.entry()).is_none()
                    {
                        return Err(invalid());
                    }
                    (
                        entry.entry(),
                        NativeCallOrigin::ProgramEntry {
                            boundary: *boundary,
                            entry: entry.entry(),
                            source_signature: entry.source_signature(),
                            completion_claim_sources: completion_claim_sources.to_vec(),
                            completion_receipts: completion_receipts.to_vec(),
                        },
                    )
                } else {
                    match (installed_call, family) {
                        (Some(_), Some(_)) => return Err(invalid()),
                        (Some(call), None) => (
                            call.provider.candidate,
                            NativeCallOrigin::InstalledProvider {
                                boundary: *boundary,
                                provider: call.provider.clone(),
                                assumption: call.assumption.clone(),
                                completion_claim_sources: completion_claim_sources.to_vec(),
                                completion_receipts: completion_receipts.to_vec(),
                            },
                        ),
                        (None, Some(family)) => {
                            let input = input.ok_or_else(invalid)?;
                            let selections =
                                input.target_machine_selections().ok_or_else(invalid)?;
                            let member =
                                selections.member(family.canonical).map_err(|_| invalid())?;
                            let TargetMachineCallable::Machine(callee) = member.callable() else {
                                continue;
                            };
                            if selections.terminal_psi() != plan.psi
                                || !matches!(
                                    member.supply(),
                                    TargetMachineSupply::CheckedBody
                                        | TargetMachineSupply::Boundary
                                )
                                || family
                                    .members
                                    .iter()
                                    .filter(|row| {
                                        row.target == selections.target()
                                            && row.callable == member.callable()
                                            && row.supply == member.supply()
                                    })
                                    .count()
                                    != 1
                                || plan.function(callee).is_none()
                            {
                                return Err(invalid());
                            }
                            (
                                callee,
                                NativeCallOrigin::TargetFamily {
                                    boundary: *boundary,
                                    member: callee,
                                    target: selections.target().to_owned(),
                                    completion_claim_sources: completion_claim_sources.to_vec(),
                                    completion_receipts: completion_receipts.to_vec(),
                                },
                            )
                        }
                        (None, None) => continue,
                    }
                };
                if selected
                    .insert(
                        key,
                        SelectedBoundaryCall {
                            caller: key.0,
                            invocation_site: key.1,
                            boundary: key.2,
                            callee,
                            result: result.clone(),
                            origin,
                        },
                    )
                    .is_some()
                {
                    return Err(LoweringError::DuplicateSelectedBoundaryCall {
                        machine: key.0,
                        operation: key.1,
                        boundary: key.2,
                    });
                }
            }
        }
        Ok(selected)
    }
}
