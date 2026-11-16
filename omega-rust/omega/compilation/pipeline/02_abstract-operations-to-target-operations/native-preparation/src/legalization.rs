/*
We drive mandatory target-operation legalization (`legalize_target_operations`)
and the standalone target-plan verifier (`verify_target_operation_plan_with_closed_module`),
packaging canonical `LegalizedOperationPlan` instances with their validation
receipts and obligation/runtime-index helpers.
*/

pub(crate) mod model {
    use optimization_core::OptimizationValidatorIdentity;

    pub(crate) fn legalization_validator_identity() -> OptimizationValidatorIdentity {
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.terminal-target-legalization-independent-replay.v74",
        )
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
        /// The plan stage 02 published differs from the plan the validator
        /// derived independently from the abstract program, or states a
        /// closed type the module does not retain. `machine` names the first
        /// function that differs; `None` means the plan's header or its seal.
        PublishedPlanMismatch {
            machine: Option<semantic_vocabulary::MachineId>,
            part: &'static str,
        },
        InstructionMetadata {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            reason: &'static str,
        },
        CallableValueProvenance {
            reason: &'static str,
        },
        CallableInvocationRoots {
            reason: String,
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

/// The facts of the optimized program that preparation joins to target rows.
#[derive(Debug, Clone, Copy)]
pub struct PreparationEvidence<'input> {
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub work_schedule: semantic_vocabulary::WorkScheduleIdentity,
    pub accepted_obligation_facts: &'input [optimization_unit::AcceptedObligationFact],
    pub ownership_frontier_facts: &'input [optimization_unit::OwnershipFrontierFact],
}

#[cfg(test)]
pub(crate) fn legalize_target_operations(
    target: &TargetOperationPlan,
    _abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: crate::BoundaryOriginFacts<'_>,
    callable_roots: &terminal_semantics::callable_values::CallableInvocationRoots,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    types: &crate::native_geometry::ModuleTypes,
) -> Result<legalized_operations::PreparedExecutionPlan, LegalizationError> {
    let callable_invocations = crate::PreparedCallableInvocations::new(unit, callable_roots)
        .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    prepare_execution_plan(
        target,
        PreparationEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
            ownership_frontier_facts: &unit.ownership_frontier_facts,
        },
        origins,
        &callable_invocations,
        installed_interrupts,
        types,
    )
}

/// The interrupt installations still present after lowering, in the order
/// the plan records them.
fn surviving_installations(target: &TargetOperationPlan) -> Vec<semantic_vocabulary::OperationId> {
    let mut operations: Vec<_> = target
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
        .filter_map(|operation| operation.installation_operation())
        .collect();
    operations.sort_unstable();
    operations.dedup();
    operations
}

/// Prepare external entry conversions while the accepted referent types are
/// available. Unsupported conversions stay explicit for internal-only calls.
fn entry_scalar_codecs(
    function: &target_operations::TargetFunction,
    types: &crate::native_geometry::ModuleTypes,
) -> Result<Vec<Option<u64>>, LegalizationError> {
    let fail = |reason| LegalizationError::InstructionMetadata {
        machine: Some(function.machine),
        operation: None,
        reason,
    };
    function
        .graph
        .call_plan
        .parameters
        .iter()
        .enumerate()
        .map(|(ordinal, placement)| {
            if placement.shape.class != calling_conventions::ValueClass::BorrowedScalarReference {
                return Ok(Some(0));
            }
            let structural = ordinal
                .checked_sub(function.graph.scalar_parameters.len())
                .and_then(|index| function.graph.parameters.get(index))
                .ok_or_else(|| fail("scalar entry conversion has no structural parameter"))?;
            let declaration = native_vocabulary::structural_type_catalog::unique_structural_type(
                function.graph.structural_types.iter(),
                structural.structural_type,
            )
            .ok_or_else(|| fail("scalar entry conversion has no unique referent declaration"))?;
            let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) = declaration.shape
            else {
                return Err(fail("scalar entry conversion referent is not primitive"));
            };
            let scalar = types.scalar(scalar).ok_or_else(|| {
                fail("scalar entry conversion referent is not an accepted closed type")
            })?;
            Ok(runtime_abi::ScalarStorageCodec::from_closed_scalar(scalar)
                .and_then(runtime_abi::ScalarStorageCodec::encode))
        })
        .collect()
}

/// Adopt each function's closed target calling contract in machine order.
/// Two functions with one machine refuse the program.
fn function_abis(
    target: &TargetOperationPlan,
    types: &crate::native_geometry::ModuleTypes,
) -> Result<std::sync::Arc<[legalized_operations::LegalizedFunctionAbi]>, LegalizationError> {
    let mut abis: Vec<_> = target
        .functions
        .iter()
        .map(|function| {
            let mut abi = legalized_operations::LegalizedFunctionAbi {
                machine: function.machine,
                call_plan: function.graph.call_plan.clone(),
                entry_scalar_codecs: entry_scalar_codecs(function, types)?,
                scalar_parameters: function.graph.scalar_parameters.clone(),
                structural_parameters: function.graph.parameters.clone(),
                scalar: function.scalar_abi.clone(),
                mixed_structural_scalar: function.mixed_structural_scalar_abi.clone(),
            };
            for value in abi.scalar_values_mut() {
                value.scalar_type = types
                    .scalar(value.scalar_type)
                    .ok_or(LegalizationError::InstructionMetadata {
                        machine: Some(function.machine),
                        operation: None,
                        reason: "function ABI scalar is absent from the accepted closed types",
                    })?
                    .as_checking();
            }
            Ok(abi)
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    abis.sort_unstable_by_key(|row| row.machine);
    if abis
        .windows(2)
        .any(|pair| pair[0].machine == pair[1].machine)
    {
        return Err(LegalizationError::custody());
    }
    Ok(abis.into())
}

/// Build the execution plan from the target operations stage 02 lowered.
///
/// Every target row keeps the identity and obligations of the operation it
/// came from, so this reads the target program and the optimized program's
/// facts; it never reads an earlier executable body. A fact or type the plan
/// needs and cannot find refuses the program here, before selection starts.
/// Every scalar type the plan states is closed here against the accepted
/// module's types, so selection reads closed types and asks no module.
pub fn prepare_execution_plan(
    target: &TargetOperationPlan,
    evidence: PreparationEvidence<'_>,
    origins: crate::BoundaryOriginFacts<'_>,
    callable_invocations: &crate::PreparedCallableInvocations,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    types: &crate::native_geometry::ModuleTypes,
) -> Result<legalized_operations::PreparedExecutionPlan, LegalizationError> {
    crate::installed_interrupts::authorize_target(target, origins.module, installed_interrupts)?;
    validate_boundary_origins(target, origins)?;
    let rosters = source::derive_target_function_rosters(target, evidence, types)?;
    if target.publication.functions().len() != target.functions.len()
        || target
            .publication
            .functions()
            .iter()
            .zip(&target.functions)
            .any(|(publication, function)| publication.machine != function.machine)
    {
        return Err(LegalizationError::InstructionMetadata {
            machine: None,
            operation: None,
            reason: "publication function roster differs from the target functions",
        });
    }
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: evidence.optimization_unit,
        work_schedule: evidence.work_schedule,
        target: target.target,
        profile: target.profile,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
        installation_operations: surviving_installations(target),
        function_abis: function_abis(target, types)?,
        borrowed_storage_layouts: target.borrowed_storage_layouts.clone(),
        publication: target.publication.clone(),
    };
    let (callable_invocations, entry_invocations) = callable_invocations
        .for_target(target, evidence)
        .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    Ok(legalized_operations::PreparedExecutionPlan::seal(
        plan,
        legalization_validator_identity(),
        callable_invocations,
        entry_invocations,
    ))
}

pub fn verify_target_operation_plan_with_closed_module(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: crate::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_module: &terminal_codec::ClosedTerminalModule,
) -> Result<(), LegalizationError> {
    let closed_types =
        crate::native_geometry::accepted_types(closed_module, target.psi, origins.module)
            .ok_or_else(LegalizationError::custody)?;
    verify_target_operation_plan_input(
        target,
        abstract_plan,
        unit,
        origins,
        installed_interrupts,
        &closed_types,
    )
}

fn verify_target_operation_plan_input(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: crate::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_types: &crate::native_geometry::ModuleTypes,
) -> Result<(), LegalizationError> {
    crate::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, origins)?;
    audit_target_operation_plan_with_module(
        target,
        abstract_plan,
        unit,
        closed_types,
        Some(origins.module),
    )
}

/// Compare the target operations with a plan prepared from them here and
/// now. Use this where no published plan exists: fixtures, and a retained
/// private body that was never sealed.
#[cfg(test)]
pub(crate) fn audit_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: &crate::native_geometry::ModuleTypes,
) -> Result<(), LegalizationError> {
    audit_target_operation_plan_with_module(target, abstract_plan, unit, closed_types, None)
}

fn audit_target_operation_plan_with_module(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: &crate::native_geometry::ModuleTypes,
    module: Option<&terminal_psi::TerminalModule>,
) -> Result<(), LegalizationError> {
    crate::publication::verify_source_publication(abstract_plan, &target.publication).map_err(
        |reason| LegalizationError::PublishedPlanMismatch {
            machine: None,
            part: reason,
        },
    )?;
    let expected = derive_expected_functions(target, abstract_plan, unit, closed_types, module)?;
    let retained = source::derive_target_function_rosters(
        target,
        PreparationEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
            ownership_frontier_facts: &unit.ownership_frontier_facts,
        },
        closed_types,
    )?;
    if expected.scalar_functions != retained.scalar_functions {
        return Err(LegalizationError::custody());
    }
    Ok(())
}

/// Validate the execution plan stage 02 actually published.
///
/// The validator derives every function of the plan a second way, from the
/// abstract program and the optimization unit, and requires the published
/// plan to equal that derivation row for row: concrete types, placements and
/// transports, the proof identity at each operation, edge and terminator, and
/// each function's ownership-frontier facts. It also requires the plan's
/// header to match its sources, every closed type the plan states to be one
/// the module retains, and the plan to still hash to the identity it was
/// sealed under. Ordinary compilation never calls
/// this; it runs on an explicit validator request and in tests.
pub fn verify_execution_plan(
    prepared: &legalized_operations::PreparedExecutionPlan,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: crate::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_module: &terminal_codec::ClosedTerminalModule,
) -> Result<(), LegalizationError> {
    let mismatch = |machine, part| LegalizationError::PublishedPlanMismatch { machine, part };
    let closed_types =
        crate::native_geometry::accepted_types(closed_module, target.psi, origins.module)
            .ok_or_else(LegalizationError::custody)?;
    let plan = prepared.plan();
    crate::publication::verify_source_publication(abstract_plan, &plan.publication)
        .map_err(|part| mismatch(None, part))?;
    if plan.publication != target.publication {
        return Err(mismatch(None, "publication facts differ from the target's"));
    }
    if !prepared.is_intact() {
        return Err(mismatch(None, "the plan no longer matches its seal"));
    }
    if prepared.receipt().validator() != legalization_validator_identity() {
        return Err(mismatch(None, "the plan was sealed by another preparation"));
    }
    if plan.psi != target.psi
        || plan.target != target.target
        || plan.profile != target.profile
        || plan.entry != target.entry
        || plan.optimization_unit != unit.identity
        || plan.work_schedule != unit.work_schedule
    {
        return Err(mismatch(None, "the plan header differs from its sources"));
    }
    if plan.installation_operations != surviving_installations(target) {
        return Err(mismatch(
            None,
            "the surviving installations differ from the target's",
        ));
    }
    for function in &plan.scalar_functions {
        let mut foreign = false;
        function.visit_closed_types(|scalar| foreign |= !closed_types.retains(scalar));
        if foreign {
            return Err(mismatch(
                Some(function.machine),
                "a closed type the module does not retain",
            ));
        }
    }
    if plan.borrowed_storage_layouts != target.borrowed_storage_layouts {
        return Err(mismatch(
            None,
            "borrowed storage geometry differs from the target's",
        ));
    }
    if plan.function_abis != function_abis(target, &closed_types)? {
        return Err(mismatch(None, "function ABI rows differ from the target's"));
    }
    crate::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, origins)?;
    let expected = derive_expected_functions(
        target,
        abstract_plan,
        unit,
        &closed_types,
        Some(origins.module),
    )?;
    if expected.scalar_functions.len() != plan.scalar_functions.len() {
        return Err(mismatch(None, "the plan has a different set of functions"));
    }
    for (expected, published) in expected.scalar_functions.iter().zip(&plan.scalar_functions) {
        if expected != published {
            let part = if expected.machine != published.machine {
                "the plan orders its functions differently"
            } else if expected.ownership_frontier_facts != published.ownership_frontier_facts {
                "ownership-frontier facts"
            } else if expected.parameters != published.parameters
                || expected.call_plan != published.call_plan
                || expected.structural != published.structural
            {
                "the function contract"
            } else {
                "a block, instruction or terminator"
            };
            return Err(mismatch(Some(published.machine), part));
        }
    }
    Ok(())
}

/// Check the target operations against their sources and derive, from the
/// abstract program and the optimization unit alone, the functions a plan
/// prepared from them must contain.
fn derive_expected_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: &crate::native_geometry::ModuleTypes,
    module: Option<&terminal_psi::TerminalModule>,
) -> Result<source::SourceFunctionRosters, LegalizationError> {
    scalar_graph_input::call_origin::validate_qualified_numeric_executions(
        target,
        &unit.boundary_machines,
        module,
    )?;
    let plan_geometry = crate::native_geometry::NativeGeometry::with_owned_sequence_bounds(
        &abstract_plan.structural_types,
        abstract_plan.structural_types.owned_sequence_bounds(),
    )
    .map_err(|_| LegalizationError::custody())?;
    let unit_geometry = crate::native_geometry::NativeGeometry::with_owned_sequence_bounds(
        &unit.structural_types,
        unit.structural_types.owned_sequence_bounds(),
    )
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
    derive_source_function_rosters(target, abstract_plan, unit, closed_types)
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
