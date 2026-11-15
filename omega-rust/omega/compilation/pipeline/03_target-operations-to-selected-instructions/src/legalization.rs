/*
We drive mandatory target-operation legalization (`legalize_target_operations`)
and the standalone target-plan verifier (`verify_target_operation_plan`),
packaging canonical `LegalizedOperationPlan` instances with their validation
receipts and obligation/runtime-index helpers.
*/

pub(crate) mod model {
    use legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
    use optimization_core::OptimizationValidatorIdentity;

    pub(crate) fn legalization_validator_identity() -> OptimizationValidatorIdentity {
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.terminal-target-legalization-independent-replay.v69",
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedLegalizedOperations {
        pub(crate) plan: LegalizedOperationPlan,
        pub(crate) receipt: LegalizationValidationReceipt,
        pub(crate) callable_invocations:
            std::sync::Arc<terminal_semantics::callable_values::ReachableCallableInvocations>,
        pub(crate) installed_stack_requirements:
            Vec<installation_evidence::InstalledInterruptStackRequirement>,
        pub(crate) installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
        pub(crate) installed_entry_execution:
            Option<provider_admission::AdmittedInstalledEntryExecution>,
    }

    impl ValidatedLegalizedOperations {
        pub(crate) fn installed_entry_execution(
            &self,
        ) -> Option<&provider_admission::AdmittedInstalledEntryExecution> {
            self.installed_entry_execution.as_ref()
        }

        pub(crate) fn installed_stack_requirements(
            &self,
        ) -> &[installation_evidence::InstalledInterruptStackRequirement] {
            &self.installed_stack_requirements
        }

        pub(crate) fn installed_interrupts(
            &self,
        ) -> &provider_admission::AdmittedInstalledInterruptCohort {
            &self.installed_interrupts
        }
        pub(crate) fn callable_invocations(
            &self,
        ) -> &terminal_semantics::callable_values::ReachableCallableInvocations {
            &self.callable_invocations
        }

        pub const fn plan(&self) -> &LegalizedOperationPlan {
            &self.plan
        }

        pub const fn receipt(&self) -> LegalizationValidationReceipt {
            self.receipt
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct LegalizationValidationReceipt {
        pub(crate) identity: LegalizedOperationPlanIdentity,
        pub(crate) validator: OptimizationValidatorIdentity,
        pub(crate) optimization_unit: optimization_core::OptimizationUnitIdentity,
        pub(crate) work_schedule: semantic_vocabulary::WorkScheduleIdentity,
        pub(crate) target: target::NativeTarget,
        pub(crate) function_count: usize,
    }

    impl LegalizationValidationReceipt {
        pub const fn identity(self) -> LegalizedOperationPlanIdentity {
            self.identity
        }

        pub const fn validator(self) -> OptimizationValidatorIdentity {
            self.validator
        }

        pub const fn optimization_unit(self) -> optimization_core::OptimizationUnitIdentity {
            self.optimization_unit
        }

        pub const fn work_schedule(self) -> semantic_vocabulary::WorkScheduleIdentity {
            self.work_schedule
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ReferenceObservationContext {
        pub position: Option<(semantic_vocabulary::BlockId, u32)>,
        pub place: semantic_vocabulary::PlaceId,
        pub path: Vec<terminal_psi::StructuralPathSegment>,
        pub carrier: Option<semantic_vocabulary::StructuralTypeId>,
        pub catalog_available: bool,
        pub location: Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation>,
        pub observation_available: bool,
        pub storage: Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation>,
        pub resident_path: Option<Vec<terminal_psi::StructuralPathSegment>>,
        pub resident_referent: Option<semantic_vocabulary::StructuralTypeId>,
        pub origin_count: Option<usize>,
        pub interference: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LegalizationError {
        InstructionMetadata {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            reason: &'static str,
        },
        CallableValueProvenance {
            reason: &'static str,
        },
        InstalledInterruptAssociation {
            operation: Option<semantic_vocabulary::OperationId>,
            reason: &'static str,
        },
        SourceCustodyMismatch {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            site: optimization_core::CompilerCheckSite,
        },
        ElementViewCustody {
            machine: semantic_vocabulary::MachineId,
            operation: semantic_vocabulary::OperationId,
            place: semantic_vocabulary::PlaceId,
            reason: &'static str,
        },
        DescriptorCallOrigins {
            machine: semantic_vocabulary::MachineId,
            result: semantic_vocabulary::PlaceId,
            reason: &'static str,
        },
        ReferenceObservation {
            machine: semantic_vocabulary::MachineId,
            operation: Option<semantic_vocabulary::OperationId>,
            site: optimization_core::CompilerCheckSite,
            reason: &'static str,
            context: Box<ReferenceObservationContext>,
        },
        UnsupportedScalarOperation {
            machine: semantic_vocabulary::MachineId,
            operation: abstract_operations::AbstractOperation,
        },
        InvalidScalarStore {
            machine: semantic_vocabulary::MachineId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        InvalidScalarReferenceBacking {
            machine: semantic_vocabulary::MachineId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        UnsupportedSourceShape {
            function: usize,
        },
        ReferenceCompletion {
            reason: terminal_semantics::ReferenceCompletionError,
        },
        ReferenceResult {
            reason: &'static str,
        },
        ReferenceDisposalConflict {
            machine: semantic_vocabulary::MachineId,
            place: semantic_vocabulary::PlaceId,
        },
    }

    impl std::fmt::Display for LegalizationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::ReferenceCompletion { reason } => formatter.write_str(reason.description()),
                Self::UnsupportedScalarOperation { machine, operation } => write!(
                    formatter,
                    "target legalization has no legal scalar instruction for {operation:?} in machine {machine:?}"
                ),
                _ => write!(formatter, "target legalization failed: {self:?}"),
            }
        }
    }

    impl std::error::Error for LegalizationError {}

    impl LegalizationError {
        pub(crate) fn instruction_metadata(reason: &'static str) -> Self {
            Self::InstructionMetadata {
                machine: None,
                operation: None,
                reason,
            }
        }

        #[track_caller]
        pub fn custody() -> Self {
            Self::SourceCustodyMismatch {
                machine: None,
                operation: None,
                site: optimization_core::CompilerCheckSite::caller(),
            }
        }

        pub(crate) fn attributed(
            self,
            machine: semantic_vocabulary::MachineId,
            operation: Option<semantic_vocabulary::OperationId>,
        ) -> Self {
            match self {
                Self::InstructionMetadata {
                    machine: None,
                    operation: reported,
                    reason,
                } => Self::InstructionMetadata {
                    machine: Some(machine),
                    operation: reported.or(operation),
                    reason,
                },
                Self::SourceCustodyMismatch {
                    machine: None,
                    operation: None,
                    site,
                } => Self::SourceCustodyMismatch {
                    machine: Some(machine),
                    operation,
                    site,
                },
                Self::ReferenceObservation {
                    machine: owner,
                    operation: None,
                    site,
                    reason,
                    context,
                } => Self::ReferenceObservation {
                    machine: owner,
                    operation,
                    site,
                    reason,
                    context,
                },
                other => other,
            }
        }
    }
}
pub(crate) mod obligations {
    use crate::LegalizationError;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::{ObligationId, OperationId};

    pub(crate) fn facts<'unit>(
        unit: &'unit PsiOptimizationUnit,
        optimized: &PsiOptimizationFunction,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        obligation: ObligationId,
    ) -> impl Iterator<Item = &'unit optimization_unit::AcceptedObligationFact> {
        matching_fact(
            &unit.accepted_obligation_facts,
            optimized.machine,
            owner.into(),
            obligation,
        )
        .into_iter()
    }

    pub(crate) fn accepted_fact(
        unit: &PsiOptimizationUnit,
        optimized: &PsiOptimizationFunction,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        obligation: ObligationId,
    ) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
        retained_fact(
            &unit.accepted_obligation_facts,
            optimized.machine,
            owner,
            obligation,
        )
    }

    pub(crate) fn retained_fact(
        facts: &[optimization_unit::AcceptedObligationFact],
        machine: semantic_vocabulary::MachineId,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        obligation: ObligationId,
    ) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
        matching_fact(facts, machine, owner.into(), obligation)
            .map(|fact| fact.identity)
            .ok_or_else(LegalizationError::custody)
    }

    fn matching_fact(
        facts: &[optimization_unit::AcceptedObligationFact],
        machine: semantic_vocabulary::MachineId,
        owner: optimization_unit::AcceptedObligationOwner,
        obligation: ObligationId,
    ) -> Option<&optimization_unit::AcceptedObligationFact> {
        // Attachment sorts these exact keys and admission rejects duplicates or
        // disorder. Borrow that index; don't scan or build another per-query one.
        facts
            .binary_search_by_key(&(machine, owner, obligation), |fact| {
                (fact.machine, fact.owner, fact.obligation)
            })
            .ok()
            .map(|position| &facts[position])
    }

    pub(crate) fn references_obligation(
        optimized: &PsiOptimizationFunction,
        operation: OperationId,
        obligation: ObligationId,
    ) -> bool {
        optimized.facts.iter().any(|fact| {
            matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if *referenced == obligation && *support == operation)
        })
    }
}
pub(crate) mod runtime_indices {
    use crate::LegalizationError;
    use crate::legalization::accepted_fact;
    use crate::structural_inputs::structural_reference_input::RuntimeElement;
    use crate::structural_inputs::structural_reference_input::{
        reference_formation_geometry, runtime_projection_elements,
    };
    use legalized_operations::LegalizedRuntimeIndexOperand;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{ScalarType, StructuralTypeId, ValueId};

    /// Resolve retained proof keys against the admitted catalog, without reading
    /// an earlier function body or recomputing its structural projection.
    pub(crate) fn retained_operands(
        facts: &[optimization_unit::AcceptedObligationFact],
        machine: semantic_vocabulary::MachineId,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        indices: &[target_operations::TargetStructuralRuntimeIndex],
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        let owner = owner.into();
        indices
            .iter()
            .map(|index| {
                let accepted_fact =
                    super::obligations::retained_fact(facts, machine, owner, index.obligation)?;
                Ok(LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: index.operand.source_value(),
                        scalar_type: index.operand.scalar_type(),
                    },
                    stride: index.stride,
                    extent: index.extent,
                    obligation: index.obligation,
                    accepted_fact,
                })
            })
            .collect()
    }

    pub(crate) fn operands(
        optimized: &PsiOptimizationFunction,
        unit: &crate::native_geometry::NativeUnit<'_>,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        elements: &[RuntimeElement],
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        let owner = owner.into();
        elements
            .iter()
            .map(|element| {
                let scalar_type = defined_type(optimized, element.index)
                    .filter(
                        |scalar| matches!(unit.closed_scalar(*scalar), Some(semantic_vocabulary::ClosedScalarType::Integer(integer)) if integer.bits() <= 64),
                    )
                    .ok_or(LegalizationError::custody())?;
                let accepted_fact = accepted_fact(unit, optimized, owner, element.obligation)?;
                Ok(LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: element.index,
                        scalar_type,
                    },
                    stride: element.stride,
                    extent: element.extent,
                    obligation: element.obligation,
                    accepted_fact,
                })
            })
            .collect()
    }

    pub(crate) fn defined_type(
        optimized: &PsiOptimizationFunction,
        value: ValueId,
    ) -> Option<ScalarType> {
        optimized
            .parameters
            .iter()
            .chain(
                optimized
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter().chain(block.definitions())),
            )
            .find(|definition| definition.value == value)
            .map(|definition| definition.scalar_type)
    }

    pub(crate) fn projection(
        optimized: &PsiOptimizationFunction,
        unit: &crate::native_geometry::NativeUnit<'_>,
        owner: impl Into<optimization_unit::AcceptedObligationOwner>,
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        declarations: &crate::native_geometry::NativeGeometry<'_>,
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        if !path.iter().any(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
            )
        }) {
            return Ok(Vec::new());
        }
        let elements = if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
            reference_formation_geometry(root, path, declarations)
                .ok_or(LegalizationError::custody())?
                .elements
        } else {
            runtime_projection_elements(root, path, declarations)
                .ok_or(LegalizationError::custody())?
        };
        operands(optimized, unit, owner, &elements)
    }

    #[cfg(test)]
    mod retained_tests {
        use super::retained_operands;
        use optimization_unit::{AcceptedObligationFact, PsiProvenance};
        use semantic_vocabulary::{MachineId, ObligationId, OperationId, ValueId};
        use target_operations::{TargetStructuralRuntimeIndex, TargetUnitScalarArgumentSource};

        #[test]
        fn retained_index_needs_only_target_row_and_exact_admitted_fact() {
            let machine = MachineId::new(1).unwrap();
            let operation = OperationId::new(2).unwrap();
            let obligation = ObligationId::new(3).unwrap();
            let value = ValueId::new(4).unwrap();
            let scalar_type = target_operations::selector_carrier_type();
            let index = TargetStructuralRuntimeIndex {
                operand: TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value: value,
                    scalar_type,
                },
                stride: 24,
                extent: Some(7),
                obligation,
            };
            let fact = AcceptedObligationFact::new(
                terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([5; 32]),
                },
                [6; 32],
                machine,
                PsiProvenance::Operation(operation),
                obligation,
                vec![7],
            );
            let lowered = retained_operands(
                std::slice::from_ref(&fact),
                machine,
                operation,
                std::slice::from_ref(&index),
            )
            .unwrap();
            assert_eq!(
                lowered,
                [legalized_operations::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult { value, scalar_type },
                    stride: 24,
                    extent: Some(7),
                    obligation,
                    accepted_fact: fact.identity,
                }]
            );
            assert!(
                retained_operands(&[], machine, operation, std::slice::from_ref(&index)).is_err()
            );
            for wrong in [
                AcceptedObligationFact {
                    machine: MachineId::new(9).unwrap(),
                    ..fact.clone()
                },
                AcceptedObligationFact {
                    owner: PsiProvenance::Operation(OperationId::new(9).unwrap()).into(),
                    ..fact.clone()
                },
                AcceptedObligationFact {
                    owner: optimization_unit::AcceptedObligationOwner::Terminator(
                        semantic_vocabulary::BlockId::new(2).unwrap(),
                    ),
                    ..fact.clone()
                },
                AcceptedObligationFact {
                    obligation: ObligationId::new(9).unwrap(),
                    ..fact
                },
            ] {
                assert!(
                    retained_operands(&[wrong], machine, operation, std::slice::from_ref(&index))
                        .is_err()
                );
            }
        }
    }
}
pub(crate) use crate::legalization_scalar_graph as scalar_graph_input;
pub(crate) use crate::legalization_source as source;

pub use model::LegalizationError;
pub(crate) use model::legalization_validator_identity;
pub(crate) use model::{LegalizationValidationReceipt, ValidatedLegalizedOperations};
pub(crate) use obligations::{accepted_fact, facts, references_obligation};

#[allow(unused_imports)]
pub(crate) use abstract_operations::AbstractOperationPlan;
#[allow(unused_imports)]
pub(crate) use legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
#[allow(unused_imports)]
pub(crate) use scalar_graph_input::call_origin::validate_boundary_origins;
#[allow(unused_imports)]
pub(crate) use target_operations::TargetOperationPlan;

#[allow(unused_imports)]
pub(crate) use source::derive_source_function_rosters;

#[cfg(test)]
pub(crate) fn legalize_target_operations(
    target: &TargetOperationPlan,
    _abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: target_operations::BoundaryOriginFacts<'_>,
    callable_roots: &terminal_semantics::callable_values::CallableInvocationRoots,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let callable_invocations = crate::PreparedCallableInvocations::new(unit, callable_roots)
        .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    legalize_retained_target_operations(
        target,
        crate::TargetSelectionEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
            ownership_frontier_facts: &unit.ownership_frontier_facts,
        },
        origins,
        &callable_invocations,
        installed_interrupts,
        installed_entry_execution,
        installed_stack_requirements,
    )
}

pub(crate) fn legalize_retained_target_operations(
    target: &TargetOperationPlan,
    evidence: crate::TargetSelectionEvidence<'_>,
    origins: target_operations::BoundaryOriginFacts<'_>,
    callable_invocations: &crate::PreparedCallableInvocations,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    crate::installed_interrupts::authorize_target(target, origins.module, installed_interrupts)?;
    validate_boundary_origins(target, origins)?;
    let rosters = source::derive_target_function_rosters(target, evidence)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: evidence.optimization_unit,
        work_schedule: evidence.work_schedule,
        target: target.target,
        profile: target.profile,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
    };
    let callable_invocations = callable_invocations
        .for_target(target, evidence)
        .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    Ok(seal(
        target,
        plan,
        callable_invocations,
        installed_interrupts,
        installed_entry_execution,
        installed_stack_requirements,
    ))
}

pub(crate) fn seal(
    target: &TargetOperationPlan,
    plan: LegalizedOperationPlan,
    callable_invocations: std::sync::Arc<
        terminal_semantics::callable_values::ReachableCallableInvocations,
    >,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> ValidatedLegalizedOperations {
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: plan.optimization_unit,
        work_schedule: plan.work_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len(),
    };
    ValidatedLegalizedOperations {
        plan,
        receipt,
        callable_invocations,
        installed_interrupts: installed_interrupts.clone(),
        installed_entry_execution: installed_entry_execution.cloned(),
        installed_stack_requirements: installed_stack_requirements.to_vec(),
    }
}
pub(crate) use scalar_graph_input::exact_cast_has_native_carriers;

pub fn verify_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: target_operations::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
) -> Result<(), LegalizationError> {
    verify_target_operation_plan_input(
        target,
        abstract_plan,
        unit,
        origins,
        installed_interrupts,
        None,
    )
}

pub fn verify_target_operation_plan_with_closed_module(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: target_operations::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_module: &terminal_codec::ClosedTerminalModule,
) -> Result<(), LegalizationError> {
    let closed_types =
        crate::native_geometry::retained_types(closed_module, target.psi, origins.module)
            .ok_or_else(LegalizationError::custody)?;
    verify_target_operation_plan_input(
        target,
        abstract_plan,
        unit,
        origins,
        installed_interrupts,
        Some(closed_types),
    )
}

fn verify_target_operation_plan_input(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: target_operations::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_types: Option<&terminal_codec::ClosedTypeCatalog>,
) -> Result<(), LegalizationError> {
    crate::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, origins)?;
    audit_target_operation_plan(target, abstract_plan, unit, closed_types)
}

pub(crate) fn audit_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: Option<&terminal_codec::ClosedTypeCatalog>,
) -> Result<(), LegalizationError> {
    let plan_geometry =
        crate::native_geometry::NativeGeometry::new(&abstract_plan.structural_types)
            .map_err(|_| LegalizationError::custody())?;
    let unit_geometry = crate::native_geometry::NativeGeometry::new(&unit.structural_types)
        .map_err(|_| LegalizationError::custody())?;
    let abstract_plan = &crate::native_geometry::NativeInput::with_types(
        abstract_plan,
        &plan_geometry,
        closed_types,
    );
    let unit = &crate::native_geometry::NativeInput::with_types(unit, &unit_geometry, closed_types);
    source::validate_source_custody(target, abstract_plan, unit)?;
    validate_call_descriptor_origins(target, abstract_plan, unit)?;
    scalar_graph_input::normalized_foreign::validate_native_callback_roster(target)?;
    let descriptor_inputs =
        crate::legalization_descriptor_inputs::DescriptorInputs::new(target, abstract_plan, unit);
    for function in &target.functions {
        let mut abstracted = abstract_plan
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let mut optimized = unit
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let (Some(abstracted_function), Some(optimized_function), None, None) = (
            abstracted.next(),
            optimized.next(),
            abstracted.next(),
            optimized.next(),
        ) else {
            return Err(LegalizationError::custody());
        };
        scalar_graph_input::match_input(
            function,
            abstracted_function,
            optimized_function,
            &descriptor_inputs,
        )
        .map_err(|error| error.attributed(function.machine, None))?;
    }
    let expected = derive_source_function_rosters(target, abstract_plan, unit, closed_types)?;
    let retained = source::derive_target_function_rosters(
        target,
        crate::TargetSelectionEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
            ownership_frontier_facts: &unit.ownership_frontier_facts,
        },
    )?;
    if expected.scalar_functions != retained.scalar_functions {
        return Err(LegalizationError::custody());
    }
    Ok(())
}

/// Independent source correspondence, never called by ordinary construction.
pub(crate) fn validate_call_descriptor_origins(
    target: &TargetOperationPlan,
    abstract_plan: &crate::native_geometry::NativePlan<'_>,
    unit: &crate::native_geometry::NativeUnit<'_>,
) -> Result<(), LegalizationError> {
    let call_descriptor_graph = std::cell::OnceCell::new();
    let mut call_descriptor_origins = scalar_graph_input::byte_views::CallDescriptorOrigins::new(
        unit,
        target,
        abstract_plan,
        &call_descriptor_graph,
    );
    for function in &target.functions {
        for operation in function
            .graph
            .blocks
            .iter()
            .flat_map(|block| block.operations())
        {
            if let target_operations::TargetUnitOperation::Call {
                result:
                    target_operations::TargetCallResult::Structural {
                        result,
                        descriptor_origins,
                        ..
                    },
                ..
            } = operation
            {
                let expected = call_descriptor_origins.resolve(function.machine, result)?;
                if descriptor_origins != &expected {
                    return Err(LegalizationError::DescriptorCallOrigins {
                        machine: function.machine,
                        result: result.place,
                        reason: "retained descriptor backing differs from source correspondence",
                    });
                }
            }
        }
    }
    Ok(())
}
