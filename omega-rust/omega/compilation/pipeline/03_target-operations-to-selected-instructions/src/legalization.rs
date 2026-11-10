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
            b"omega.terminal-target-legalization-independent-replay.v56",
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ValidatedLegalizedOperations {
        pub(crate) plan: LegalizedOperationPlan,
        pub(crate) receipt: LegalizationValidationReceipt,
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
        owner: impl Into<optimization_unit::PsiProvenance>,
        obligation: ObligationId,
    ) -> impl Iterator<Item = &'unit optimization_unit::AcceptedObligationFact> {
        let owner = owner.into();
        let machine = optimized.machine;
        unit.accepted_obligation_facts.iter().filter(move |fact| {
            fact.machine == machine && fact.owner == owner && fact.obligation == obligation
        })
    }

    pub(crate) fn accepted_fact(
        unit: &PsiOptimizationUnit,
        optimized: &PsiOptimizationFunction,
        owner: impl Into<optimization_unit::PsiProvenance>,
        obligation: ObligationId,
    ) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
        facts(unit, optimized, owner, obligation)
            .next()
            .map(|fact| fact.identity)
            .ok_or_else(LegalizationError::custody)
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
    use super::scalar_graph_input::primitive_locals::read_root_type;
    use crate::LegalizationError;
    use crate::legalization::accepted_fact;
    use crate::structural_inputs::structural_reference_input::RuntimeElement;
    use crate::structural_inputs::structural_reference_input::{
        collection_projection, reference_formation_geometry, runtime_projection_elements,
    };
    use legalized_operations::LegalizedRuntimeIndexOperand;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::{OperationId, ScalarType, StructuralTypeId, ValueId};

    pub(crate) fn operands(
        optimized: &PsiOptimizationFunction,
        unit: &PsiOptimizationUnit,
        owner: impl Into<optimization_unit::PsiProvenance>,
        elements: &[RuntimeElement],
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        let owner = owner.into();
        elements
            .iter()
            .map(|element| {
                let scalar_type = defined_type(optimized, element.index)
                    .filter(
                        |scalar| matches!(scalar, ScalarType::Integer(integer) if integer.bits() <= 64),
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
        unit: &PsiOptimizationUnit,
        owner: impl Into<optimization_unit::PsiProvenance>,
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        declarations: &[terminal_psi::StructuralTypeDeclaration],
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

    pub(crate) fn reference_source(
        optimized: &PsiOptimizationFunction,
        unit: &PsiOptimizationUnit,
        operation: OperationId,
        source: &terminal_psi::StructuralArgument,
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        let root = read_root_type(optimized, source.place).ok_or(LegalizationError::custody())?;
        let path = match source.path.split_last() {
            Some((terminal_psi::StructuralPathSegment::Referent, prefix)) => prefix,
            _ => source.path.as_slice(),
        };
        let geometry = reference_formation_geometry(root, path, &unit.structural_types)
            .ok_or(LegalizationError::custody())?;
        operands(optimized, unit, operation, &geometry.elements)
    }

    pub(crate) fn collection_view(
        optimized: &PsiOptimizationFunction,
        unit: &PsiOptimizationUnit,
        operation: OperationId,
        source: &terminal_psi::StructuralArgument,
    ) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
        if !source
            .path
            .iter()
            .any(|segment| segment.runtime_index().is_some())
        {
            return Ok(Vec::new());
        }
        let root = read_root_type(optimized, source.place).ok_or(LegalizationError::custody())?;
        let path = match source.path.last() {
            Some(terminal_psi::StructuralPathSegment::FixedRange { .. }) => {
                &source.path[..source.path.len() - 1]
            }
            _ => &source.path,
        };
        let (_, _, elements) = collection_projection(root, path, &unit.structural_types)
            .ok_or(LegalizationError::custody())?;
        operands(optimized, unit, operation, &elements)
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

pub(crate) fn legalize_target_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: target_operations::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    crate::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, unit, origins)?;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        work_schedule: unit.work_schedule,
        target: target.target,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
    };
    Ok(seal(
        target,
        unit,
        plan,
        installed_interrupts,
        installed_entry_execution,
        installed_stack_requirements,
    ))
}

pub(crate) fn seal(
    target: &TargetOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
) -> ValidatedLegalizedOperations {
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        work_schedule: unit.work_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len(),
    };
    ValidatedLegalizedOperations {
        plan,
        receipt,
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
    crate::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, unit, origins)?;
    audit_target_operation_plan(target, abstract_plan, unit)
}

pub(crate) fn audit_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    source::validate_source_custody(target, abstract_plan, unit)?;
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
    Ok(())
}
