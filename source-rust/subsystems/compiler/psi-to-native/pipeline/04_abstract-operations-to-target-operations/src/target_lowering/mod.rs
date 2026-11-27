#![forbid(unsafe_code)]

/*
  Native stage 04 (`04_abstract-operations-to-target-operations`): target-aware
  lowering of `AbstractOperationPlan` into `TargetOperationPlan`
  (`lower_native_body`, `NativeBodyPlan`,
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
  single block-structured target graph here, native stage 05
  (`05_target-operations-to-selected-instructions`) can legalize operands and
  select machine instructions without re-evaluating source signatures or
  provider settlements.
*/

/*
  Consumes the `OptimizedAbstractBody` published by native stage 03
  (`03_abstract-operations-to-abstract-operations`) alongside admitted boundary
  settlements, provider installation evidence and the selected target.
  Produces a prepared instruction-selection plan for native stage 05
  (`05_target-operations-to-selected-instructions`).
*/

mod call_result_origins;
pub(crate) mod control_flow;
mod coordination;
mod function_contract;
mod function_signature;
pub(crate) mod model;
mod scalar;
pub(crate) mod structural_layout;
mod unit;

pub(crate) use function_signature::{scalar_abi, structural_signature, structural_type_lookup};

pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError,
};
pub use optimized::{OptimizedTargetLoweringRequest, lower_native_body, require_lowering_context};

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
    use crate::target_lowering::LoweringError;
    use crate::target_lowering::structural_type_lookup::StructuralTypeLookup;
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
            selected_calls: &crate::target_lowering::selected_boundary_calls::SelectedCallsByCall,
        ) -> Option<(OperationId, BoundaryMachineId)> {
            function.operations().find_map(|operation| {
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
        optimized: &optimization_unit::PsiOptimizationFunction,
        target: NativeTarget,
        profile: Option<target::TargetProfile>,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        callables: &terminal_psi::TerminalCallableCatalog,
        borrowed_reference_sources: super::control_flow::references::BorrowedSources,
        stored_reference_effects: super::control_flow::references::StoredEffectCatalog,
        scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
        structural_types: &StructuralTypeLookup<'_>,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        selected_calls: &crate::target_lowering::selected_boundary_calls::SelectedCallsByCall,
        native_callbacks: &BTreeMap<
            (semantic_vocabulary::MachineId, OperationId),
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
            optimized,
            target,
            profile,
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

    use crate::target_lowering::{AdmittedBoundarySettlement, LoweringError};

    /// The two ways a target body can name a private function are a retained
    /// foreign call that materializes a callback, and an installed gate that
    /// takes its address. External completion transfers carry no callbacks.
    fn retained_private_references(
        prepared: &legalized_operations::PreparedExecutionPlan,
        facts: &AdmittedLoweringFacts,
    ) -> Vec<function_identity::MachineFunctionIdentity> {
        let plan = prepared.plan();
        let mut references: Vec<_> = plan
            .scalar_functions
            .iter()
            .flat_map(|function| function.instructions())
            .filter_map(|instruction| match &instruction.kind {
                legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(
                    call,
                ) => Some(&call.callbacks),
                _ => None,
            })
            .flatten()
            .map(|callback| callback.callback_function)
            .chain(
                facts
                    .installed_interrupts()
                    .entries()
                    .iter()
                    .filter(|entry| {
                        plan.installation_operations
                            .binary_search(&(entry.machine, entry.operation))
                            .is_ok()
                    })
                    .map(|entry| entry.callback_function),
            )
            .collect();
        references.sort_unstable_by_key(|identity| identity.to_bytes());
        references.dedup();
        references
    }
    fn boundary_origins(
        input: &AdmittedLoweringFacts,
    ) -> crate::preparation::BoundaryOriginFacts<'_> {
        crate::preparation::BoundaryOriginFacts::from_lowering(input)
    }

    pub struct OptimizedTargetLoweringRequest<'a> {
        pub target: NativeTarget,
        pub settlements: &'a [AdmittedBoundarySettlement<'a>],
        pub installation: Option<AdmittedProviderInstallation>,
    }

    pub fn lower_native_body(
        input: verified_optimization_input::OptimizedAbstractBody,
        request: OptimizedTargetLoweringRequest<'_>,
        retain_original: bool,
    ) -> Result<crate::NativeBodyPlan, LoweringError> {
        let (optimized, lowering_facts) = input.into_parts();
        let OptimizedTargetLoweringRequest {
            target,
            settlements,
            installation,
        } = request;
        let target_rules = require_lowering_context(&optimized, &lowering_facts, target)?;
        let bindings =
            super::provider_evidence::bind_provider_executions(optimized.plan(), settlements)
                .map_err(|error| error.with_boundary_context(optimized.plan()))?;
        let program =
            super::coordination::lower_to_target_operations_with_settlements_and_installation(
                optimized.plan(),
                optimized.unit(),
                target,
                &bindings,
                installation.as_ref(),
                lowering_facts.native_callbacks().callbacks(),
                Some(&lowering_facts),
            )
            .map_err(|error| error.with_boundary_context(optimized.plan()))?;
        let prepared = Arc::new(prepare(&optimized, &lowering_facts, &program)?);
        let private_references = retained_private_references(&prepared, &lowering_facts);
        let selection = legalized_operations::PreparedInstructionSelection {
            plan: Arc::clone(&prepared),
            target_rules,
            selections: optimized.selections().clone(),
            budget_per_pass: optimized.budget_per_pass(),
        };
        let mut facts = crate::emission_facts::project(&optimized, &lowering_facts);
        let original_for_audit = if retain_original {
            Some(
                facts
                    .retain_target_operations(
                        optimized,
                        lowering_facts,
                        Arc::new(program),
                        prepared,
                    )
                    .map_err(|reason| LoweringError::Publication {
                        machine: facts.program.entry,
                        reason: reason.into(),
                    })?,
            )
        } else {
            None
        };
        let machine = facts.program.entry;
        let facts = crate::BodyPublication::new(facts, original_for_audit).map_err(|reason| {
            LoweringError::Publication {
                machine,
                reason: reason.into(),
            }
        })?;
        Ok(crate::NativeBodyPlan {
            selection,
            facts,
            private_references,
        })
    }

    /// Finish the stage: join the lowered target operations to the optimized
    /// program's facts, the admitted origins and installations, and the sealed
    /// module's retained types. Everything read here is this stage's own input
    /// or output, so nothing has to reach instruction selection separately.
    fn prepare(
        optimized: &OptimizedAbstractProgram,
        lowering_facts: &AdmittedLoweringFacts,
        program: &TargetOperationPlan,
    ) -> Result<legalized_operations::PreparedExecutionPlan, LoweringError> {
        let custody = || {
            LoweringError::Preparation(Box::new(
                crate::preparation::TargetOperationAuditError::custody(),
            ))
        };
        let unit = optimized.unit();
        let types = crate::preparation::accepted_types(
            lowering_facts.closed_module(),
            program.psi,
            lowering_facts.module(),
        )
        .ok_or_else(custody)?;
        let callable_bindings = lowering_facts
            .callable_bindings(program)
            .map_err(|reason| {
                LoweringError::Preparation(Box::new(
                    crate::preparation::TargetOperationAuditError::CallableBindings { reason },
                ))
            })?;
        let callable_invocations =
            crate::preparation::PreparedCallableInvocations::new(unit, &callable_bindings)
                .map_err(|reason| {
                    LoweringError::Preparation(Box::new(
                        crate::preparation::TargetOperationAuditError::CallableValueProvenance {
                            reason,
                        },
                    ))
                })?;
        crate::preparation::prepare_execution_plan(
            program,
            crate::preparation::PreparationEvidence {
                optimization_unit: unit.identity,
                work_schedule: unit.work_schedule,
                accepted_obligation_facts: &unit.accepted_obligation_facts,
            },
            boundary_origins(lowering_facts),
            &callable_invocations,
            lowering_facts.installed_interrupts(),
            &types,
            optimized.selections().identity(),
            optimized.budget_per_pass(),
        )
        .map_err(|error| LoweringError::Preparation(Box::new(error)))
    }

    pub fn require_lowering_context(
        optimized: &OptimizedAbstractProgram,
        lowering_facts: &AdmittedLoweringFacts,
        target: NativeTarget,
    ) -> Result<optimization_core::OptimizationPhaseSelections, LoweringError> {
        use optimization_core::Optimization;
        let phase = OptimizationExecutionPhase::TargetOperations;
        let rules = optimized.selections().project_phase(phase);
        for rule in rules.selections().as_slice() {
            match *rule {
                Optimization::UnsignedPowerOfTwoRemainderMask => {}
                Optimization::X86ScaledIndexedLoad
                    if target.architecture == target::Architecture::X86_64 => {}
                Optimization::X86ScaledIndexedLoad => {
                    return Err(LoweringError::TargetOptimizationArchitecture {
                        rule: *rule,
                        required: target::Architecture::X86_64,
                        actual: target.architecture,
                    });
                }
                _ => return Err(LoweringError::UnconsumedOptimizationPhase(phase)),
            }
        }
        let plan = optimized.plan();
        lowering_facts
            .require_lowering_context(plan, target)
            .map_err(LoweringError::ForeignLoweringFacts)?;
        optimization_unit::validate_runtime_evidence_context(
            optimized.unit(),
            plan.psi,
            lowering_facts.module(),
        )
        .map_err(LoweringError::ForeignLoweringFacts)?;
        if lowering_facts.native_callbacks().psi() != plan.psi {
            return Err(LoweringError::ForeignNativeCallbackCohort);
        }
        if lowering_facts.installed_interrupts().psi() != plan.psi {
            return Err(LoweringError::ForeignInstalledInterruptCohort);
        }
        Ok(rules)
    }
}

pub(crate) mod provider_evidence {
    use crate::target_lowering::AdmittedBoundaryExecution;
    use crate::target_lowering::{AdmittedBoundarySettlement, LoweringError};
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
                    AdmittedBoundaryExecution::NativeBinding { execution, .. } => {
                        target_operations::BoundaryExecutionBinding::NativeBinding(execution)
                    }
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
    use crate::target_lowering::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
    use operation_evidence::provider_calls::InstalledProviderCallEvidence;
    use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId};
    use std::collections::BTreeMap;
    use target_operations::NativeCallOrigin;

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
            for operation in function.operations() {
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
                let entry_call = input
                    .and_then(|input| input.program_entry())
                    .filter(|entry| entry.boundary() == *boundary);
                let (callee, origin) = if let Some(entry) = entry_call {
                    if installed_call.is_some() || plan.function(entry.entry()).is_none() {
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
                    match installed_call {
                        Some(call) => (
                            call.provider.candidate,
                            NativeCallOrigin::InstalledProvider {
                                boundary: *boundary,
                                provider: call.provider.clone(),
                                completion_claim_sources: completion_claim_sources.to_vec(),
                                completion_receipts: completion_receipts.to_vec(),
                            },
                        ),
                        None => continue,
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
