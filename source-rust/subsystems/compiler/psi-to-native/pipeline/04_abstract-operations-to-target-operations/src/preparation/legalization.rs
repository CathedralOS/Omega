/*
We drive mandatory target-operation legalization (`prepare_execution_plan`)
and the standalone target-plan validators (`verify_execution_plan` and
`verify_target_operation_plan_with_closed_module`), packaging canonical
`LegalizedOperationPlan` instances with their validation receipts and
obligation/runtime-index helpers.
*/

pub(crate) mod model {
    use optimization_core::OptimizationValidatorIdentity;

    pub(crate) fn legalization_validator_identity() -> OptimizationValidatorIdentity {
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.terminal-target-legalization-independent-replay.v75",
        )
    }

    /// Errors raised while preparing or validating a target execution plan.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LegalizationError {
        InstructionMetadata {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            reason: &'static str,
        },
        /// The plan stage 04 published differs from the plan the validator
        /// derived from the target operations, or states a closed type the
        /// module does not retain. `machine` names the first function that
        /// differs; `None` means the plan's header or its seal.
        PublishedPlanMismatch {
            machine: Option<semantic_vocabulary::MachineId>,
            part: &'static str,
        },
        CallableValueProvenance {
            reason: &'static str,
        },
        CallablePublication {
            cause: target_vocabulary::NativeExecutionFactsError,
        },
        CallableBindings {
            reason: String,
        },
        InstalledInterruptAssociation {
            operation: Option<semantic_vocabulary::OperationId>,
            reason: &'static str,
        },
        SourceCustodyMismatch {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            site: compiler_check_site::CompilerCheckSite,
            cause: Option<String>,
        },
    }

    impl std::fmt::Display for LegalizationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "target legalization failed: {self:?}")
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
            Self::custody_reason(None)
        }

        /// Keep the failed query's reason as well as this conversion's site.
        /// Some input queries expose only Debug, so retain that description
        /// without imposing another error vocabulary on their owners.
        #[track_caller]
        pub(crate) fn custody_with_cause(cause: impl std::fmt::Debug) -> Self {
            Self::custody_reason(Some(format!("{cause:?}")))
        }

        #[track_caller]
        fn custody_reason(cause: Option<String>) -> Self {
            Self::SourceCustodyMismatch {
                machine: None,
                operation: None,
                site: compiler_check_site::CompilerCheckSite::caller(),
                cause,
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
                    cause,
                } => Self::SourceCustodyMismatch {
                    machine: Some(machine),
                    operation,
                    site,
                    cause,
                },
                other => other,
            }
        }
    }
}

pub(crate) mod obligations {
    use crate::preparation::LegalizationError;
    use semantic_vocabulary::ObligationId;

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
}

pub(crate) mod runtime_indices {
    use crate::preparation::LegalizationError;
    use legalized_operations::LegalizedRuntimeIndexOperand;

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

    #[cfg(test)]
    mod retained_tests {
        use super::retained_operands;
        use optimization_unit::{AcceptedObligationFact, PsiProvenance};
        use semantic_vocabulary::{MachineId, ObligationId, OperationId, ValueId};
        use target_operations::{TargetStructuralRuntimeIndex, TargetUnitScalarArgumentSource};

        #[test]
        fn retained_index_needs_only_target_row_and_admitted_fact() {
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

pub(in crate::preparation) use crate::preparation::legalization_scalar_graph as scalar_graph_input;
pub(in crate::preparation) use crate::preparation::legalization_source as source;

pub use model::LegalizationError;
pub(crate) use model::legalization_validator_identity;

#[allow(unused_imports)]
pub(crate) use abstract_operations::AbstractOperationPlan;
#[allow(unused_imports)]
pub(crate) use legalized_operations::LegalizedOperationPlan;
#[allow(unused_imports)]
pub(crate) use scalar_graph_input::call_origin::validate_boundary_origins;
#[allow(unused_imports)]
pub(crate) use target_operations::TargetOperationPlan;

/// The facts of the optimized program that preparation joins to target rows.
#[derive(Debug, Clone, Copy)]
pub struct PreparationEvidence<'input> {
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub work_schedule: semantic_vocabulary::WorkScheduleIdentity,
    pub accepted_obligation_facts: &'input [optimization_unit::AcceptedObligationFact],
}

/// The interrupt installations still present after lowering, in the order
/// the plan records them.
fn surviving_installations(
    target: &TargetOperationPlan,
) -> Vec<(
    semantic_vocabulary::MachineId,
    semantic_vocabulary::OperationId,
)> {
    let mut operations: Vec<_> = target
        .functions
        .iter()
        .flat_map(|function| {
            function.graph.operations().filter_map(move |operation| {
                operation
                    .installation_operation()
                    .map(|operation| (function.machine, operation))
            })
        })
        .collect();
    operations.sort_unstable();
    operations.dedup();
    operations
}

/// Prepare external entry conversions while the accepted referent types are
/// available. Unsupported conversions stay explicit for internal-only calls.
fn entry_scalar_codecs(
    function: &target_operations::TargetFunction,
    types: &crate::preparation::native_geometry::ModuleTypes,
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
            let declaration = structural_storage::structural_type_catalog::unique_structural_type(
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
    types: &crate::preparation::native_geometry::ModuleTypes,
) -> Result<std::sync::Arc<[legalized_operations::LegalizedFunctionAbi]>, LegalizationError> {
    let mut abis: Vec<_> = target
        .functions
        .iter()
        .map(|function| {
            let mut abi = legalized_operations::LegalizedFunctionAbi {
                assembly_return: function.assembly_return,
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

/// Build the execution plan from the target operations stage 04 lowered.
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
    origins: crate::preparation::BoundaryOriginFacts<'_>,
    callable_invocations: &crate::preparation::PreparedCallableInvocations,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    types: &crate::preparation::native_geometry::ModuleTypes,
    pre_physical: optimization_core::PrePhysicalOptimizationManifestIdentity,
) -> Result<legalized_operations::PreparedExecutionPlan, LegalizationError> {
    crate::preparation::installed_interrupts::authorize_target(
        target,
        origins.module,
        installed_interrupts,
    )?;
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
        execution: Some(
            callable_invocations
                .for_target(target, evidence, pre_physical)
                .map_err(|cause| LegalizationError::CallablePublication { cause })?,
        ),
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
    legalized_operations::PreparedExecutionPlan::seal(plan, legalization_validator_identity())
        .map_err(|reason| LegalizationError::CallableValueProvenance { reason })
}

pub fn verify_target_operation_plan_with_closed_module(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: crate::preparation::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_module: &terminal_codec::ClosedTerminalModule,
) -> Result<(), LegalizationError> {
    let closed_types = crate::preparation::native_geometry::accepted_types(
        closed_module,
        target.psi,
        origins.module,
    )
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
    origins: crate::preparation::BoundaryOriginFacts<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    closed_types: &crate::preparation::native_geometry::ModuleTypes,
) -> Result<(), LegalizationError> {
    crate::preparation::installed_interrupts::validate_target(
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

/// Compare the target operations with their source rosters, obligations, and
/// closed scalar types when no sealed execution plan exists.
#[cfg(test)]
pub(crate) fn audit_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: &crate::preparation::native_geometry::ModuleTypes,
) -> Result<(), LegalizationError> {
    audit_target_operation_plan_with_module(target, abstract_plan, unit, closed_types, None)
}

fn audit_target_operation_plan_with_module(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    closed_types: &crate::preparation::native_geometry::ModuleTypes,
    module: Option<&terminal_psi::TerminalModule>,
) -> Result<(), LegalizationError> {
    crate::preparation::publication::verify_source_publication(abstract_plan, &target.publication)
        .map_err(|reason| LegalizationError::PublishedPlanMismatch {
            machine: None,
            part: reason,
        })?;
    verify_target_source_correspondence(target, abstract_plan, unit, module)?;
    let _ = source::derive_target_function_rosters(
        target,
        PreparationEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
        },
        closed_types,
    )?;
    Ok(())
}

/// Validate the execution plan stage 04 actually published.
///
/// The validator checks the plan's seal, header, callable facts, source
/// publication, closed types, ABIs, surviving installations, boundary origins,
/// and target-to-source operation correspondence, then re-derives every
/// legalized function from the target operations and requires the published
/// plan to equal that derivation row for row. Ordinary compilation never calls
/// this; it runs on an explicit validator request and in tests.
pub fn verify_execution_plan(
    prepared: &legalized_operations::PreparedExecutionPlan,
    target: &TargetOperationPlan,
    optimized: &optimization_unit::OptimizedAbstractProgram,
    lowering: &verified_optimization_input::AdmittedLoweringFacts,
) -> Result<(), LegalizationError> {
    let abstract_plan = optimized.plan();
    let unit = optimized.unit();
    let pre_physical = optimized.pre_physical_manifest();
    let origins = crate::preparation::BoundaryOriginFacts::from_lowering(lowering);
    let installed_interrupts = lowering.installed_interrupts();
    let closed_module = lowering.closed_module();
    let callable_bindings = lowering
        .callable_bindings(target)
        .map_err(|reason| LegalizationError::CallableBindings { reason })?;
    let mismatch = |machine, part| LegalizationError::PublishedPlanMismatch { machine, part };
    let closed_types = crate::preparation::native_geometry::accepted_types(
        closed_module,
        target.psi,
        origins.module,
    )
    .ok_or_else(LegalizationError::custody)?;
    let plan = prepared.plan();
    let callable_program =
        super::callable_invocations::callable_program_with_providers(unit, &callable_bindings)
            .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    let main = terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
        &callable_program,
        [unit.entry],
        callable_bindings.private_entries(),
    )
    .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
    let mut entries = Vec::new();
    for &entry in &main.roots {
        if entry == unit.entry {
            continue;
        }
        let summary =
            terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
                &callable_program,
                [entry],
                callable_bindings.private_entries(),
            )
            .map_err(|reason| LegalizationError::CallableValueProvenance { reason })?;
        entries.push((entry, std::sync::Arc::new(summary)));
    }
    let expected_execution = target_vocabulary::NativeExecutionFacts::seal(
        target_vocabulary::NativeExecutionRoots {
            psi: unit.psi,
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            pre_physical,
            entry: unit.entry,
        },
        std::sync::Arc::new(main),
        entries,
    )
    .map_err(|cause| LegalizationError::CallablePublication { cause })?;
    if plan.execution.as_ref() != Some(&expected_execution) {
        return Err(mismatch(
            None,
            "prepared callable invocation facts differ from their source",
        ));
    }
    crate::preparation::publication::verify_source_publication(abstract_plan, &plan.publication)
        .map_err(|part| mismatch(None, part))?;
    if plan.publication != target.publication {
        return Err(mismatch(None, "publication facts differ from the target's"));
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
    crate::preparation::installed_interrupts::validate_target(
        target,
        origins.module,
        installed_interrupts,
        abstract_plan,
    )?;
    validate_boundary_origins(target, origins)?;
    verify_target_source_correspondence(target, abstract_plan, unit, Some(origins.module))?;
    let expected = source::derive_target_function_rosters(
        target,
        PreparationEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
        },
        &closed_types,
    )?;
    if expected.scalar_functions.len() != plan.scalar_functions.len() {
        return Err(mismatch(None, "the plan has a different set of functions"));
    }
    for (expected, published) in expected.scalar_functions.iter().zip(&plan.scalar_functions) {
        if expected != published {
            let part = if expected.machine != published.machine {
                "the plan orders its functions differently"
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

/// Verify target-to-source roster, metadata, terminator, and constant
/// correspondence without re-auditing `PsiOptimizationUnit` input custody.
fn verify_target_source_correspondence(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    module: Option<&terminal_psi::TerminalModule>,
) -> Result<(), LegalizationError> {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ScalarType;
    use target_operations::{TargetControlTerminator, TargetUnitOperation};

    scalar_graph_input::call_origin::validate_qualified_numeric_executions(
        target,
        &unit.boundary_machines,
        module,
    )?;
    source::validate_source_custody(target, abstract_plan, unit)?;
    scalar_graph_input::normalized_foreign::validate_native_callback_roster(target)?;

    for ((target_fn, abstract_fn), unit_fn) in target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
    {
        let machine = target_fn.machine;
        if machine != abstract_fn.machine
            || machine != unit_fn.machine
            || target_fn.graph.entry != abstract_fn.entry
            || target_fn.graph.entry != unit_fn.entry
            || target_fn.graph.blocks.len() != abstract_fn.blocks.len()
            || target_fn.graph.blocks.len() != unit_fn.blocks.len()
        {
            return Err(LegalizationError::custody().attributed(machine, None));
        }
        let has_storage = target_fn.mixed_structural_scalar_abi.is_some()
            || !target_fn.graph.parameters.is_empty()
            || !unit_fn.declared_places.is_empty();
        match &target_fn.structural_contract {
            Some(contract) => {
                if !has_storage
                    || contract.parameters != abstract_fn.structural_parameters
                    || contract.places != unit_fn.structural_places
                    || contract.entry_claims != abstract_fn.entry_claims
                    || contract.published_service_ceiling != abstract_fn.published_service_ceiling
                    || contract.result.as_ref() != abstract_fn.result.structural()
                {
                    return Err(LegalizationError::custody().attributed(machine, None));
                }
            }
            None if has_storage => {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            None => {}
        }
        for ((target_block, abstract_block), unit_block) in target_fn
            .graph
            .blocks
            .iter()
            .zip(&abstract_fn.blocks)
            .zip(&unit_fn.blocks)
        {
            if target_block.block != abstract_block.block || target_block.block != unit_block.id {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            let Some((source_terminal, body)) = unit_block.nodes.split_last() else {
                return Err(LegalizationError::custody().attributed(machine, None));
            };
            if target_block.terminator_metadata.work != source_terminal.work
                || target_block.terminator_metadata.effect != source_terminal.effect
                || target_block.terminator_metadata.ownership != source_terminal.ownership
            {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            let edge_matches =
                |edge,
                 destination,
                 work: &[optimization_unit::WorkSettlement],
                 source: &optimization_unit::OptimizationEdge| {
                    edge == source.psi_edge
                        && destination == source.target
                        && work == source.work.as_slice()
                };
            let terminator_ok = match &target_block.terminator {
                TargetControlTerminator::Jump { successor } => matches!(
                    (
                        &source_terminal.operation,
                        source_terminal.successors.as_slice(),
                    ),
                    (
                        AbstractOperation::Jump {
                            psi_edge,
                            target: dest,
                            ..
                        },
                        [source_edge],
                    ) if *psi_edge == successor.psi_edge
                        && *dest == successor.target
                        && edge_matches(
                            successor.psi_edge,
                            successor.target,
                            &successor.work,
                            source_edge,
                        )
                ),
                TargetControlTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => matches!(
                    (
                        &source_terminal.operation,
                        source_terminal.successors.as_slice(),
                    ),
                    (AbstractOperation::Conditional { .. }, [source_true, source_false])
                        if edge_matches(
                            when_true.psi_edge,
                            when_true.target,
                            &when_true.work,
                            source_true,
                        ) && edge_matches(
                            when_false.psi_edge,
                            when_false.target,
                            &when_false.work,
                            source_false,
                        )
                ),
                TargetControlTerminator::StructuralCase {
                    psi_block, cases, ..
                } => {
                    matches!(
                        &source_terminal.operation,
                        AbstractOperation::StructuralCase { psi_block: source_block, .. }
                            if source_block == psi_block
                    ) && cases.len() == source_terminal.successors.len()
                        && cases.iter().zip(&source_terminal.successors).all(
                            |(case, source_edge)| {
                                edge_matches(case.psi_edge, case.target, &case.work, source_edge)
                            },
                        )
                }
                TargetControlTerminator::Return { psi_edge, .. } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::ReturnUnit { psi_edge: source_edge, .. }
                        if source_edge == psi_edge
                ),
                TargetControlTerminator::ReturnScalar { psi_edge, .. } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::Return { psi_edge: source_edge, .. }
                        if source_edge == psi_edge
                ),
                TargetControlTerminator::ReturnStructural { psi_edge, .. } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::ReturnStructural { psi_edge: source_edge, .. }
                        if source_edge == psi_edge
                ),
                TargetControlTerminator::Crash { psi_edge, .. } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::Crash { psi_edge: source_edge, .. }
                        if source_edge == psi_edge
                ),
                TargetControlTerminator::EliminateEmpty { psi_edge, .. } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::EliminateEmpty { psi_edge: source_edge, .. }
                        if source_edge == psi_edge
                ),
                TargetControlTerminator::InvokeNoNormal {
                    psi_edge, callee, ..
                } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::InvokeNoNormal {
                        psi_edge: source_edge,
                        callee: source_callee,
                        ..
                    } if source_edge == psi_edge && source_callee == callee
                ),
                TargetControlTerminator::ExternalCompletion {
                    psi_edge, boundary, ..
                } => matches!(
                    &source_terminal.operation,
                    AbstractOperation::ExternalCompletion {
                        psi_edge: source_edge,
                        boundary: source_boundary,
                        ..
                    } if source_edge == psi_edge && source_boundary == boundary
                ),
            };
            if !terminator_ok {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            let ordinary_count = target_block
                .operations
                .iter()
                .position(|row| matches!(row.kind, TargetUnitOperation::CleanupCall { .. }))
                .unwrap_or(target_block.operations.len());
            if ordinary_count != body.len() {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            if target_block.operations[ordinary_count..].iter().any(|row| {
                row.source.is_some() || !matches!(row.kind, TargetUnitOperation::CleanupCall { .. })
            }) {
                return Err(LegalizationError::custody().attributed(machine, None));
            }
            for (row, node) in target_block.operations[..ordinary_count].iter().zip(body) {
                let Some(source) = &row.source else {
                    return Err(LegalizationError::custody().attributed(machine, None));
                };
                let Some(expected_op) = node.operation.psi_operation() else {
                    return Err(LegalizationError::custody().attributed(machine, None));
                };
                let expected_result = match node.definitions.as_slice() {
                    [] => None,
                    [definition] => Some(*definition),
                    _ => {
                        return Err(
                            LegalizationError::custody().attributed(machine, Some(expected_op))
                        );
                    }
                };
                if source.operation != expected_op
                    || source.result != expected_result
                    || source.metadata.work != node.work
                    || source.metadata.effect != node.effect
                    || source.metadata.ownership != node.ownership
                {
                    return Err(LegalizationError::custody().attributed(machine, Some(expected_op)));
                }
                let literal_ok = match &row.kind {
                    TargetUnitOperation::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type,
                        value,
                    } => matches!(
                        &node.operation,
                        AbstractOperation::IntegerConstant {
                            psi_operation: source_op,
                            result: source_result,
                            scalar_type: source_type,
                            value: source_value,
                        } if source_op == psi_operation
                            && source_result == result
                            && *source_type == ScalarType::Integer(*scalar_type)
                            && source_value == value
                    ),
                    TargetUnitOperation::BooleanConstant {
                        psi_operation,
                        result,
                        value,
                    } => matches!(
                        &node.operation,
                        AbstractOperation::BooleanConstant {
                            psi_operation: source_op,
                            result: source_result,
                            value: source_value,
                        } if source_op == psi_operation
                            && source_result == result
                            && source_value == value
                    ),
                    TargetUnitOperation::IeeeFloatConstant {
                        psi_operation,
                        result,
                        value,
                    } => matches!(
                        &node.operation,
                        AbstractOperation::IeeeFloatConstant {
                            psi_operation: source_op,
                            result: source_result,
                            value: source_value,
                        } if source_op == psi_operation
                            && source_result == result
                            && source_value == value
                    ),
                    _ => true,
                };
                if !literal_ok {
                    return Err(LegalizationError::custody().attributed(machine, Some(expected_op)));
                }
            }
        }
    }
    Ok(())
}
