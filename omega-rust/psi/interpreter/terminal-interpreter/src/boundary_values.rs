/*
    Boundary values, results, entry result requests, result custody accounting,
    error types, memory ordering, and native code layout environments.

    We define the scalar, structural, and aggregate values that cross between
    an admitted Terminal Psi execution and its embedding host, along with the
    status, crash, and error vocabulary reported at boundaries.
*/

pub(crate) mod entry_results {
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::values::TerminalScalarValue;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::{MachineId, ScalarType, ValueId};

    #[derive(Clone, Copy)]
    pub struct TerminalEntryResultRequest<'request> {
        pub machine: MachineId,
        pub results: &'request [(ValueId, ScalarType)],
    }

    pub(crate) struct EntryResults {
        machine: MachineId,
        rows: RuntimeRows<ValueId, (ScalarType, Option<TerminalScalarValue>)>,
    }

    fn failure(reason: &'static str) -> TerminalInterpretError {
        TerminalInterpretError::EntryResultObservation(reason)
    }

    impl EntryResults {
        fn captured_value(
            &self,
            result: ValueId,
            scalar_type: ScalarType,
        ) -> Result<Option<TerminalScalarValue>, String> {
            let (expected, value) = self
                .rows
                .get(&result)
                .ok_or("scalar entry check names an unrequested result occurrence")?;
            if *expected != scalar_type {
                return Err("scalar entry check changed its captured result carrier".into());
            }
            Ok(*value)
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn request_scalar_entry_checks(
            &mut self,
            checks: terminal_installation::ScalarEntryChecks,
            explicit: Option<TerminalEntryResultRequest<'_>>,
        ) -> Result<(), TerminalInterpretError> {
            if self.scalar_entry_checks.is_some() {
                return Err(failure("scalar entry checks were already installed"));
            }
            if explicit.is_some_and(|request| request.machine != self.current_machine) {
                return Err(failure(
                    "entry result observations name a different root activation",
                ));
            }
            let mut results = explicit
                .map(|request| request.results.to_vec())
                .unwrap_or_default();
            for &(result, scalar_type) in checks.results() {
                if let Some((_, expected)) = results.iter().find(|(value, _)| *value == result) {
                    if *expected != scalar_type {
                        return Err(failure(
                            "scalar entry check and explicit observation disagree on the result carrier",
                        ));
                    }
                } else {
                    results.push((result, scalar_type));
                }
            }
            self.request_entry_results(TerminalEntryResultRequest {
                machine: self.current_machine,
                results: &results,
            })?;
            self.scalar_entry_checks = Some(checks);
            Ok(())
        }

        pub(crate) fn request_entry_results(
            &mut self,
            request: TerminalEntryResultRequest<'_>,
        ) -> Result<(), TerminalInterpretError> {
            if !self.entry_storage_open || self.entry_results.is_some() {
                return Err(failure(
                    "entry result observations must be fixed before execution",
                ));
            }
            if request.machine != self.current_machine || !self.call_stack.is_empty() {
                return Err(failure(
                    "entry result observations name a different root activation",
                ));
            }
            let machine = self.program.verified_machine(request.machine)?;
            let mut rows = RuntimeRows::new(self.account.as_ref());
            for &(result, scalar_type) in request.results {
                if rows.contains_key(&result) {
                    return Err(failure("entry result observation request repeats a result"));
                }
                let declared = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| operation.result.scalar())
                    .find(|declaration| declaration.id == result);
                if declared.is_none_or(|declaration| declaration.scalar_type != scalar_type) {
                    return Err(failure(
                        "entry result observation lacks its exact operation and carrier",
                    ));
                }
                rows.insert_vacant(result, (scalar_type, None))?;
            }
            self.entry_results = Some(EntryResults {
                machine: request.machine,
                rows,
            });
            Ok(())
        }

        pub(crate) fn observe_completed_entry_result(
            &mut self,
            result: ValueId,
        ) -> Result<(), TerminalInterpretError> {
            if !self.call_stack.is_empty() {
                return Ok(());
            }
            let Some(observations) = &mut self.entry_results else {
                return Ok(());
            };
            if self.current_machine != observations.machine {
                return Ok(());
            }
            let Some((expected, observed)) = observations.rows.get_mut(&result) else {
                return Ok(());
            };
            if observed.is_some() {
                return Err(failure("entry result occurrence completed more than once"));
            }
            let value = self
                .values
                .get(&result)
                .copied()
                .ok_or_else(|| failure("completed entry result has no scalar value"))?;
            if value.scalar_type() != *expected {
                return Err(failure(
                    "completed entry result changed its declared carrier",
                ));
            }
            *observed = Some(value);
            self.discharge_scalar_entry_checks()
        }

        fn discharge_scalar_entry_checks(&mut self) -> Result<(), TerminalInterpretError> {
            let Some(checks) = &mut self.scalar_entry_checks else {
                return Ok(());
            };
            let observations = self.entry_results.as_ref().ok_or_else(|| {
                failure("scalar entry checks lost their required result observations")
            })?;
            checks
                .discharge_ready(|result, scalar_type| {
                    observations.captured_value(result, scalar_type)
                })
                .map_err(TerminalInterpretError::ScalarApplicationEvaluation)
        }

        pub(crate) fn require_scalar_entry_checks_complete(
            &self,
        ) -> Result<(), TerminalInterpretError> {
            if let Some(checks) = &self.scalar_entry_checks {
                checks
                    .require_complete()
                    .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?;
            }
            Ok(())
        }

        pub(crate) fn require_entry_results_complete(&self) -> Result<(), TerminalInterpretError> {
            if self.entry_results.as_ref().is_some_and(|observations| {
                observations.rows.values().any(|(_, value)| value.is_none())
            }) {
                return Err(failure(
                    "entry returned without completing every requested result occurrence",
                ));
            }
            Ok(())
        }

        pub(crate) fn observed_entry_result(
            &self,
            machine: MachineId,
            result: ValueId,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if self.result.is_none() || self.failure.is_some() {
                return Err(failure(
                    "entry result observations require successful normal completion",
                ));
            }
            let observations = self
                .entry_results
                .as_ref()
                .filter(|observations| observations.machine == machine)
                .ok_or_else(|| {
                    failure("entry result observation names an unrequested activation")
                })?;
            observations
                .rows
                .get(&result)
                .and_then(|(_, value)| *value)
                .ok_or_else(|| {
                    failure("entry result occurrence was not requested or not completed")
                })
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::engine::calls::frames::tests::empty_program;
        use crate::engine::execution::SuspendedCallResult;
        use crate::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
        use crate::{TerminalExecutionResult, TerminalExecutionStatus};
        use psi_work::WorkMeter;
        use semantic_vocabulary::{EdgeId, IntegerSign, IntegerType, OperationId};
        use terminal_psi::{
            Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator,
            ValueDeclaration,
        };

        struct NoEffects;
        impl TerminalEffectHandler for NoEffects {
            fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
                Err(TerminalEffectRejection::new("unexpected effect"))
            }
        }

        fn execution() -> TerminalExecution<'static> {
            let mut program = empty_program();
            let program_mut = std::sync::Arc::get_mut(&mut program).unwrap();
            program_mut.fixture_module_mut().machines[0].result =
                TerminalMachineResult::Scalar(ValueDeclaration {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                });
            program_mut.fixture_module_mut().machines[0].blocks[0].terminator = returned();
            program_mut.fixture_module_mut().machines[0].blocks[0]
                .operations
                .push(Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    OperationResult::Scalar(ValueDeclaration {
                        id: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                        qualifications: Default::default(),
                    }),
                    OperationKind::BooleanConstant { value: true },
                ));
            TerminalExecution::start_with_program(
                program,
                &[],
                crate::TerminalStructuralInputs::default(),
                None,
                None,
            )
            .unwrap()
        }

        fn request(execution: &mut TerminalExecution<'_>) {
            execution
                .request_entry_results(TerminalEntryResultRequest {
                    machine: MachineId::new(1).unwrap(),
                    results: &[(ValueId::new(1).unwrap(), ScalarType::Boolean)],
                })
                .unwrap();
        }

        #[test]
        fn entry_results_capture_successful_operation_once_across_work_pauses() {
            let mut execution = execution();
            request(&mut execution);
            let machine = execution.current_machine;
            let result = ValueId::new(1).unwrap();
            let mut meter = WorkMeter::with_budget(0);
            assert!(matches!(
                execution.resume(&mut meter, &mut NoEffects).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert!(execution.observed_entry_result(machine, result).is_err());
            meter.replenish(1).unwrap();
            assert!(matches!(
                execution.resume(&mut meter, &mut NoEffects).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert!(
                execution.observed_entry_result(machine, result).is_err(),
                "captured does not mean completed"
            );
            meter.replenish(1).unwrap();
            assert!(matches!(
                execution.resume(&mut meter, &mut NoEffects).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Boolean(true)
                ))
            ));
            assert_eq!(
                execution.observed_entry_result(machine, result).unwrap(),
                TerminalScalarValue::Boolean(true)
            );
            execution
                .values
                .insert(result, TerminalScalarValue::Boolean(false))
                .unwrap();
            assert_eq!(
                execution.observed_entry_result(machine, result).unwrap(),
                TerminalScalarValue::Boolean(true)
            );
        }

        #[test]
        fn entry_results_ignore_recursive_activation_and_capture_restored_caller() {
            let mut execution = execution();
            request(&mut execution);
            let result = ValueId::new(1).unwrap();
            execution
                .suspend_with(SuspendedCallResult::Scalar(result), Vec::new())
                .unwrap();
            execution
                .values
                .insert(result, TerminalScalarValue::Boolean(false))
                .unwrap();
            execution.observe_completed_entry_result(result).unwrap();
            assert!(execution.require_entry_results_complete().is_err());
            execution
                .values
                .insert(result, TerminalScalarValue::Boolean(true))
                .unwrap();
            assert!(
                execution
                    .settle_return(&returned(), &mut WorkMeter::unbounded())
                    .is_ok()
            );
            execution.require_entry_results_complete().unwrap();
            assert!(
                execution
                    .settle_return(&returned(), &mut WorkMeter::unbounded())
                    .is_ok()
            );
            assert_eq!(
                execution
                    .observed_entry_result(MachineId::new(1).unwrap(), result)
                    .unwrap(),
                TerminalScalarValue::Boolean(true)
            );
        }

        #[test]
        fn entry_results_repeated_completion_is_a_sticky_execution_failure() {
            let mut execution = execution();
            request(&mut execution);
            let mut meter = WorkMeter::with_budget(1);
            execution.resume(&mut meter, &mut NoEffects).unwrap();
            execution.next_operation = 0;
            meter.replenish(1).unwrap();
            let failure = execution.resume(&mut meter, &mut NoEffects).unwrap_err();
            assert!(format!("{failure:?}").contains("completed more than once"));
            assert!(execution.failure.is_some());
            assert!(
                execution
                    .resume(&mut WorkMeter::unbounded(), &mut NoEffects)
                    .is_err()
            );
            assert!(
                execution
                    .observed_entry_result(MachineId::new(1).unwrap(), ValueId::new(1).unwrap())
                    .is_err()
            );
        }

        #[test]
        fn entry_results_missing_occurrence_cannot_use_final_return() {
            let mut execution = execution();
            request(&mut execution);
            execution.next_operation = 1;
            execution
                .values
                .insert(ValueId::new(1).unwrap(), TerminalScalarValue::Boolean(true))
                .unwrap();
            let failure = execution
                .resume(&mut WorkMeter::unbounded(), &mut NoEffects)
                .unwrap_err();
            assert!(format!("{failure:?}").contains("without completing"));
            assert!(execution.result.is_none());
        }

        #[test]
        fn entry_results_require_exact_root_operation_carrier_and_unique_request() {
            for (machine, results) in [
                (
                    MachineId::new(2).unwrap(),
                    vec![(ValueId::new(1).unwrap(), ScalarType::Boolean)],
                ),
                (
                    MachineId::new(1).unwrap(),
                    vec![(ValueId::new(2).unwrap(), ScalarType::Boolean)],
                ),
                (
                    MachineId::new(1).unwrap(),
                    vec![(
                        ValueId::new(1).unwrap(),
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                    )],
                ),
                (
                    MachineId::new(1).unwrap(),
                    vec![(ValueId::new(1).unwrap(), ScalarType::Boolean); 2],
                ),
            ] {
                let mut execution = execution();
                assert!(
                    execution
                        .request_entry_results(TerminalEntryResultRequest {
                            machine,
                            results: &results
                        })
                        .is_err()
                );
                assert!(
                    execution.entry_results.is_none(),
                    "failed request leaves no partial roster"
                );
            }
        }

        fn returned() -> Terminator {
            Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            }
        }
    }
}

pub(crate) mod errors {
    use crate::boundary::effects::TerminalEffectRejection;
    use crate::boundary::results::TerminalCrash;
    use psi_work::WorkMeterError;
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType,
        StructuralDomainId, StructuralFieldId, StructuralTypeId, ValueId,
    };
    use terminal_psi::CrashCause;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalInterpretError {
        UnsuppliedErasedEntryInputs {
            machine: MachineId,
            reason: &'static str,
        },
        ZeroInitializedArgumentInvalid {
            argument_index: u32,
        },
        StructuralContentsArgumentInvalid {
            argument_index: u32,
            reason: &'static str,
        },
        UnboundExternalCompletions {
            missing: Vec<crate::engine::capabilities::TerminalExternalCompletionDemand>,
        },
        ExternalCompletion(crate::boundary::results::TerminalExternalCompletion),
        ExternalCompletionProviderReturned {
            boundary: BoundaryMachineId,
            edge: semantic_vocabulary::EdgeId,
        },
        NoNormalInvocationReturned {
            callee: MachineId,
            edge: semantic_vocabulary::EdgeId,
        },
        ScalarEntryQualificationUnsupported,
        UnsupportedCapability(crate::engine::capabilities::TerminalUnsupportedCapability),
        UnsupportedSemanticVariant(&'static str),
        ResidentStorage(TerminalEffectRejection),
        AffineFrontierMismatch,
        NominalCleanupHasLiveLoans {
            cleanup: terminal_psi::NominalAffineCleanup,
            backing_identity: u64,
            backing_path: Vec<terminal_psi::StructuralPathSegment>,
            loan_carriers: Vec<(u64, Vec<terminal_psi::StructuralPathSegment>)>,
            loan_carrier_homes: Vec<(PlaceId, crate::boundary::values::TerminalStructuralValue)>,
        },
        StructuralReturnAffineFrontierMismatch {
            source: PlaceId,
            expected: Vec<terminal_psi::StructuralAffineDiscard>,
            actual: Vec<terminal_psi::StructuralAffineDiscard>,
        },
        UnsettledReturnFrontier {
            affine: Vec<terminal_psi::StructuralAffineDiscard>,
            claims: Vec<semantic_vocabulary::ClaimId>,
        },
        StructuralReturnClaimsMismatch {
            source: PlaceId,
            expected: Vec<terminal_psi::StructuralResultClaimBinding>,
            actual: Vec<terminal_psi::StructuralResultClaimBinding>,
            unreturned: Vec<semantic_vocabulary::ClaimId>,
        },
        AffineProjectionNotRepresentable {
            context: String,
        },
        AccessViewInputsRequireCustody,
        AccessReference(String),
        AccessViewInputEstablishmentDuplicate {
            machine: MachineId,
            position: u32,
        },
        AccessViewInputEstablishmentUnexpected {
            machine: MachineId,
            position: u32,
        },
        AccessViewInputEstablishmentAliasing(u64),
        AccessViewReferent(terminal_semantics::AccessViewReferentError),
        AccessViewInput(terminal_semantics::AccessViewInputError),
        ArgumentCount {
            expected: usize,
            actual: usize,
        },
        ArgumentType {
            value: ValueId,
            expected: ScalarType,
            actual: ScalarType,
        },
        ArgumentIntegerOutsideType {
            value: ValueId,
        },
        StructuralArgumentCount {
            expected: usize,
            actual: usize,
        },
        StructuralArgumentType {
            place: PlaceId,
            expected: StructuralTypeId,
            actual: StructuralTypeId,
        },
        StructuralQualificationsNonCanonical,
        StructuralCallResultContract {
            callee: MachineId,
            place: PlaceId,
            actual: StructuralTypeId,
            expected: Option<StructuralTypeId>,
            reason: &'static str,
        },
        StructuralArgumentPreparation {
            stage: &'static str,
            detail: String,
        },
        StructuralStorePreparation {
            stage: &'static str,
            detail: String,
        },
        ReferenceFormation {
            stage: &'static str,
            detail: String,
        },
        StructuralQualificationMissing(PlaceId),
        StructuralArgumentAliasing(u64),
        StructuralPrimitiveValueCount {
            expected: usize,
            actual: usize,
        },
        StructuralPrimitiveValueInvalid {
            argument_index: u32,
        },
        StructuralPrimitiveValueType {
            argument_index: u32,
            expected: ScalarType,
            actual: ScalarType,
        },
        StructuralPrimitiveStorageMissing(PlaceId),
        StructuralIdentityExhausted,
        StructuralScalarFieldMissing {
            source: PlaceId,
            field: StructuralFieldId,
        },
        StructuralScalarFieldArgumentInvalid {
            argument_index: u32,
            field: StructuralFieldId,
        },
        StructuralBooleanFieldArgumentInvalid {
            argument_index: u32,
            field: StructuralFieldId,
        },
        StructuralBooleanFieldMissing {
            source: PlaceId,
            field: StructuralFieldId,
        },
        BoundaryQualificationMissing {
            boundary: BoundaryMachineId,
            argument_index: u32,
            domain: StructuralDomainId,
        },
        ClaimTransferMismatch,
        OwnedValueCustodyMismatch {
            source: PlaceId,
            structural_type: StructuralTypeId,
            path_length: usize,
            path_prefix: Vec<terminal_psi::StructuralPathSegment>,
            reason: &'static str,
        },
        ContentTransformationMismatch(&'static str),
        ResourceProfile {
            boundary: BoundaryMachineId,
            diagnostic: access_plans::AccessPlanDiagnostic,
        },
        ValueImage(layout_plans::MaterializationDiagnostic),
        ValueImageGeometry(terminal_semantics::StorageGeometryError),
        CompletionReceiptMismatch,
        ProviderInstallationIdentityMismatch,
        FoldedConstantClosure(&'static str),
        EntryResultObservation(&'static str),
        UnclosedScalarApplication(semantic_vocabulary::ScalarApplicationId),
        ScalarApplicationEvaluation(String),
        UnclosedArrayExtent(semantic_vocabulary::StructuralTypeId),
        TargetMachineSelectionIdentityMismatch,
        TargetLayoutSelectionIdentityMismatch,
        TargetLayoutSelectionMissing(terminal_psi::LayoutApplicationId),
        TargetLayoutSelection(terminal_installation::TargetLayoutSelectionError),
        TargetMachineSelectionMissing(terminal_psi::TargetMachineCallable),
        TargetMachineSelection(terminal_installation::TargetMachineSelectionError),
        RuntimeSupplySelectionRejected {
            boundary: BoundaryMachineId,
            reason: &'static str,
        },
        ProviderInstallationMissing(BoundaryMachineId),
        VerifiedEntryMachineMissing,
        VerifiedCallTargetMissing(MachineId),
        VerifiedBoundaryMachineMissing(BoundaryMachineId),
        VerifiedBlockMissing,
        VerifiedOperationMalformed,
        VerifiedOperationContractInvalid {
            reason: &'static str,
        },
        ExecutionSite {
            machine: MachineId,
            block: BlockId,
            operation: Option<OperationId>,
            operation_tag: Option<terminal_semantics::OperationSemanticTag>,
            cause: Box<TerminalInterpretError>,
        },
        StructuralReplacement {
            operation: OperationId,
            phase: &'static str,
            cause: Box<TerminalInterpretError>,
        },
        VerifiedStructuralPlaceMissing(PlaceId),
        VerifiedValueMissing(ValueId),
        EffectRejected {
            operation: OperationId,
            rejection: TerminalEffectRejection,
        },
        BoundaryCrashNotPermitted {
            operation: OperationId,
            boundary: BoundaryMachineId,
            cause: CrashCause,
            reason: crate::boundary::effects::crash_outcome::BoundaryCrashOutcomeError,
        },
        Crash(TerminalCrash),
        Work(WorkMeterError),
        Storage(evaluation_accounting::EvaluationAccountingError),
        StorageAllocationFailed {
            requested_bytes: usize,
            reason: String,
        },
    }

    pub use terminal_installation::{AssumedBindingRejection, ProviderInstallationError};

    #[derive(Debug)]
    pub enum TerminalArtifactInterpretError {
        ArtifactDecode(terminal_codec::CanonicalTerminalArtifactError),
        SemanticDecode(terminal_codec::CodecError),
        Execution(TerminalInterpretError),
    }

    impl std::fmt::Display for TerminalArtifactInterpretError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for TerminalArtifactInterpretError {}

    pub(crate) fn malformed() -> TerminalInterpretError {
        TerminalInterpretError::VerifiedOperationMalformed
    }

    impl TerminalInterpretError {
        pub(crate) fn in_reference_formation(self, stage: &'static str) -> Self {
            Self::ReferenceFormation {
                stage,
                detail: format!("{self:?}"),
            }
        }

        pub(crate) fn in_argument_preparation(self, stage: &'static str) -> Self {
            let detail = match self {
                Self::StructuralArgumentPreparation { stage, detail } => {
                    format!("{stage}: {detail}")
                }
                other => format!("{other:?}"),
            };
            Self::StructuralArgumentPreparation { stage, detail }
        }
    }

    impl From<WorkMeterError> for TerminalInterpretError {
        fn from(error: WorkMeterError) -> Self {
            Self::Work(error)
        }
    }

    impl std::fmt::Display for TerminalInterpretError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::ExecutionSite {
                    machine,
                    block,
                    operation,
                    operation_tag,
                    cause,
                } => {
                    write!(
                        formatter,
                        "{cause} in {machine:?}/{block:?} at {operation:?}"
                    )?;
                    if let Some(tag) = operation_tag {
                        write!(formatter, " ({tag:?})")?;
                    }
                    Ok(())
                }
                Self::StructuralReplacement {
                    operation,
                    phase,
                    cause,
                } => {
                    write!(formatter, "{cause} during {phase} at {operation:?}")
                }
                Self::EffectRejected {
                    operation,
                    rejection,
                } => {
                    write!(formatter, "{} at {operation:?}", rejection.reason)
                }
                Self::Work(reason) => reason.fmt(formatter),
                Self::Storage(reason) => reason.fmt(formatter),
                other => write!(formatter, "{other:?}"),
            }
        }
    }

    impl std::error::Error for TerminalInterpretError {}
}

pub(crate) mod memory_ordering {
    use terminal_psi::{MemoryFenceKind, RegisterTargetApplicability};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalMemoryOrderingEnvironment {
        target: RegisterTargetApplicability,
    }

    impl TerminalMemoryOrderingEnvironment {
        pub fn private_sequential(target: &str) -> Option<Self> {
            [
                RegisterTargetApplicability::X86_64,
                RegisterTargetApplicability::Aarch64,
            ]
            .into_iter()
            .find(|architecture| architecture.accepts_target_name(target))
            .map(|target| Self { target })
        }

        pub(crate) fn accepts_target(self, target: &str) -> bool {
            self.target.accepts_target_name(target)
        }

        pub(crate) fn supports(self, kind: MemoryFenceKind) -> bool {
            self.target == kind.target()
        }
    }
}

pub(crate) mod native_code_layouts {
    use crate::boundary::errors::TerminalInterpretError;
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalNativeCodeRegion {
        pub base: u64,
        pub length: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalNativeCodeLayout {
        pub machine: MachineId,
        pub entry: u64,
        pub regions: Vec<TerminalNativeCodeRegion>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalNativeCodeLayoutEnvironment {
        terminal_psi: terminal_psi::TerminalPsiIdentity,
        target: String,
        native_artifact: [u8; 32],
        layouts: Vec<TerminalNativeCodeLayout>,
    }

    impl TerminalNativeCodeLayoutEnvironment {
        pub fn linked_image(
            terminal_psi: terminal_psi::TerminalPsiIdentity,
            target: &str,
            native_artifact: [u8; 32],
            mut layouts: Vec<TerminalNativeCodeLayout>,
        ) -> Result<Self, TerminalInterpretError> {
            let invalid = || {
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "native layout supply has invalid completed-image metadata",
                )
            };
            if target.is_empty() || native_artifact == [0; 32] {
                return Err(invalid());
            }
            layouts.sort_by_key(|layout| layout.machine);
            if layouts
                .windows(2)
                .any(|pair| pair[0].machine == pair[1].machine)
            {
                return Err(invalid());
            }
            for layout in &layouts {
                let mut previous_end = None;
                let mut contains_entry = false;
                for region in &layout.regions {
                    let end = region.base.checked_add(region.length).ok_or_else(invalid)?;
                    if region.base == 0
                        || region.length == 0
                        || previous_end.is_some_and(|previous| region.base <= previous)
                    {
                        return Err(invalid());
                    }
                    contains_entry |= region.base <= layout.entry && layout.entry < end;
                    previous_end = Some(end);
                }
                if layout.entry == 0 || !contains_entry {
                    return Err(invalid());
                }
            }
            Ok(Self {
                terminal_psi,
                target: target.to_owned(),
                native_artifact,
                layouts,
            })
        }

        pub(crate) fn accepts(
            &self,
            code: terminal_codec::TerminalExecutionIdentity,
            target: &str,
        ) -> bool {
            code == terminal_codec::TerminalExecutionIdentity::portable(self.terminal_psi)
                && self.target == target
        }

        pub(crate) fn layouts(&self) -> &[TerminalNativeCodeLayout] {
            &self.layouts
        }

        pub(crate) fn layout(&self, machine: MachineId) -> Option<&TerminalNativeCodeLayout> {
            self.layouts
                .binary_search_by_key(&machine, |layout| layout.machine)
                .ok()
                .map(|position| &self.layouts[position])
        }
    }
}

pub(crate) mod result_custody {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::results::TerminalExecutionResult;
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::execution::{CompletedReturn, TerminalExecution};
    use crate::storage::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::provider_storage::{
        ProviderStorageBinding, ProviderStorageSlot, ResidentStorageSlot,
    };
    use crate::storage::structural_storage::{StorageKey, cells};
    use arena::Handle;
    use evaluation_accounting::{EvaluationAccountingError, EvaluationResource};

    pub(crate) struct ResultCustody<'storage> {
        cells: u64,
        payload_bytes: u64,
        byte_backings: Vec<&'storage ByteSequenceView>,
        allocations: Vec<Handle<ProviderStorageSlot>>,
        residents: Vec<Handle<ResidentStorageSlot>>,
    }

    fn count_overflow() -> TerminalInterpretError {
        TerminalInterpretError::Storage(EvaluationAccountingError::CounterOverflow(
            EvaluationResource::ResultCustody,
        ))
    }

    impl<'storage> ResultCustody<'storage> {
        fn new() -> Self {
            Self {
                cells: 1,
                payload_bytes: 0,
                byte_backings: Vec::new(),
                allocations: Vec::new(),
                residents: Vec::new(),
            }
        }

        pub(crate) fn add_cells(&mut self, count: usize) -> Result<(), TerminalInterpretError> {
            self.cells = self
                .cells
                .checked_add(u64::try_from(count).map_err(|_| count_overflow())?)
                .ok_or_else(count_overflow)?;
            Ok(())
        }

        pub(crate) fn rows<K: StorageKey, Value>(
            &mut self,
            rows: &impl std::borrow::Borrow<std::collections::BTreeMap<K, Value>>,
            root: &StructuralRuntimePlace,
        ) -> Result<(), TerminalInterpretError> {
            self.add_cells(cells(rows, root).count())
        }

        pub(crate) fn bytes(
            &mut self,
            view: &'storage ByteSequenceView,
        ) -> Result<(), TerminalInterpretError> {
            if !self
                .byte_backings
                .iter()
                .any(|previous| previous.shares_backing(view))
            {
                self.payload_bytes = self
                    .payload_bytes
                    .checked_add(
                        u64::try_from(view.retained_payload_bytes())
                            .map_err(|_| count_overflow())?,
                    )
                    .ok_or_else(count_overflow)?;
                self.byte_backings.push(view);
            }
            Ok(())
        }

        pub(crate) fn binding(
            &mut self,
            binding: &'storage ByteSequenceBinding,
        ) -> Result<(), TerminalInterpretError> {
            match binding {
                ByteSequenceBinding::Immutable(view) => self.bytes(view)?,
                ByteSequenceBinding::Representation(view) => self.element_view(view)?,
                _ => {}
            }
            Ok(())
        }

        pub(crate) fn element_view(
            &mut self,
            view: &'storage crate::storage::element_views::ElementView,
        ) -> Result<(), TerminalInterpretError> {
            if let Some(bytes) = view.retained_bytes() {
                self.bytes(bytes)?;
            }
            Ok(())
        }

        pub(crate) fn reference(
            &mut self,
            reference: &'storage crate::storage::reference::ReferenceReferent,
        ) -> Result<(), TerminalInterpretError> {
            if let Some(binding) = &reference.byte_owner {
                self.binding(binding)?;
            }
            if let Some(view) = &reference.collection_loan {
                self.element_view(view)?;
            }
            if let Some(loan) = &reference.primitive_loan {
                self.endpoint(&loan.storage.endpoint)?;
            }
            if let Some(region) = &reference.byte_region {
                self.endpoint(&region.endpoint)?;
            }
            Ok(())
        }

        fn endpoint(
            &mut self,
            endpoint: &'storage PrimitiveStorageEndpoint,
        ) -> Result<(), TerminalInterpretError> {
            match endpoint {
                PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                    self.element_view(&byte.view)?
                }
                PrimitiveStorageEndpoint::ViewByte { bytes, .. } => self.bytes(bytes)?,
                PrimitiveStorageEndpoint::Projection { fragments, .. } => {
                    for fragment in fragments {
                        self.endpoint(&fragment.storage.endpoint)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }

        pub(crate) fn provider(
            &mut self,
            execution: &'storage TerminalExecution<'_>,
            binding: ProviderStorageBinding,
        ) -> Result<(), TerminalInterpretError> {
            if !self.allocations.contains(&binding.allocation) {
                if !matches!(
                    execution
                        .provider_storage
                        .allocations
                        .get(binding.allocation),
                    ProviderStorageSlot::Live { .. }
                ) {
                    return Err(malformed());
                }
                self.allocations.push(binding.allocation);
                self.add_cells(1)?;
            }
            if binding.resident != Handle::default() && !self.residents.contains(&binding.resident)
            {
                let ResidentStorageSlot::Live { custody, .. } =
                    execution.provider_storage.residents.get(binding.resident)
                else {
                    return Err(malformed());
                };
                self.residents.push(binding.resident);
                self.add_cells(1)?;
                custody.storage.measure_result_custody(execution, self)?;
            }
            Ok(())
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn charge_completed_result(
            &self,
            completed: &CompletedReturn,
        ) -> Result<(), TerminalInterpretError> {
            let Some(account) = &self.account else {
                return Ok(());
            };
            let mut custody = ResultCustody::new();
            match &completed.value {
                TerminalExecutionResult::Unit | TerminalExecutionResult::Scalar(_) => {}
                TerminalExecutionResult::ScalarCase(result) => {
                    custody.add_cells(result.value().fields.len())?
                }
                TerminalExecutionResult::ScalarArray(result) => {
                    custody.add_cells(result.value.elements().len())?
                }
                TerminalExecutionResult::Structural(result) => {
                    if completed.byte_sequence.is_none() && completed.element_view.is_none() {
                        self.measure_result_storage(
                            &StructuralRuntimePlace::from(&result.value),
                            &mut custody,
                        )?;
                    }
                }
            }
            if let Some(binding) = &completed.byte_sequence {
                custody.binding(binding)?;
            }
            if let Some(view) = &completed.element_view {
                custody.element_view(view)?;
            }
            account
                .charge_result_custody(custody.cells, custody.payload_bytes)
                .map_err(TerminalInterpretError::Storage)
        }

        fn measure_result_storage<'storage>(
            &'storage self,
            root: &StructuralRuntimePlace,
            custody: &mut ResultCustody<'storage>,
        ) -> Result<(), TerminalInterpretError> {
            custody.rows(&self.structural_scalar_fields, root)?;
            custody.rows(&self.structural_primitive_storage, root)?;
            custody.rows(&self.structural_cases, root)?;
            custody.rows(&self.structural_host_atoms, root)?;
            custody.rows(&self.original_storage_roots, root)?;
            custody.rows(&self.provider_storage.completed_profiles, root)?;
            custody.rows(&self.structural_byte_storage, root)?;
            custody.rows(&self.empty_array_storage.rows, root)?;
            custody.rows(&self.structural_byte_sequence_fields, root)?;
            custody.rows(&self.structural_byte_descriptors, root)?;
            custody.rows(&self.structural_element_view_fields, root)?;
            custody.rows(&self.reference_referents, root)?;
            custody.rows(&self.provider_storage.descriptors, root)?;
            for (_, storage) in cells(&self.structural_byte_storage, root) {
                storage.measure_result_custody(custody)?;
            }
            for (_, binding) in cells(&self.structural_byte_sequence_fields, root) {
                custody.binding(binding)?;
            }
            for (_, binding) in cells(&self.structural_byte_descriptors, root) {
                custody.binding(binding)?;
            }
            for (_, view) in cells(&self.structural_element_view_fields, root) {
                custody.element_view(view)?;
            }
            for (_, reference) in cells(&self.reference_referents, root) {
                custody.reference(reference)?;
            }
            for (_, binding) in cells(&self.provider_storage.descriptors, root) {
                custody.provider(self, *binding)?;
            }
            self.activation_storage
                .measure_result_custody(root, custody)
        }
    }
}

pub(crate) mod results {
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::values::{
        TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralValue,
    };
    use crate::storage::scalar_array::TerminalScalarArrayResult;
    use psi_work::{WorkBudgetExhausted, WorkMeterError};
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, MachineId,
        OperationId, StructuralFieldId,
    };
    use terminal_psi::CrashCause;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalExecutionResult {
        Unit,
        Scalar(TerminalScalarValue),
        Structural(TerminalStructuralResult),
        ScalarCase(TerminalScalarCaseResult),
        ScalarArray(TerminalScalarArrayResult),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalStructuralResult {
        pub value: TerminalStructuralValue,
        pub claims: Vec<ClaimId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalScalarCaseResult {
        storage: std::sync::Arc<ScalarCaseStorage>,
    }

    #[derive(compiler_storage::Record)]
    struct ScalarCaseStorage {
        value: TerminalScalarAggregateValue,
        #[record(skip)]
        _lease: Option<evaluation_accounting::EvaluationStorageLease>,
    }

    impl TerminalScalarCaseResult {
        pub fn value(&self) -> &TerminalScalarAggregateValue {
            &self.storage.value
        }

        pub(crate) fn collect(
            declaration: &terminal_psi::StructuralTypeDeclaration,
            case: semantic_vocabulary::StructuralCaseId,
            count: usize,
            fields: impl IntoIterator<
                Item = Result<(StructuralFieldId, TerminalScalarValue), TerminalInterpretError>,
            >,
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            let cells = u64::try_from(count).map_err(|_| crate::boundary::errors::malformed())?;
            let lease = account
                .map(|account| account.reserve_storage(cells, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            let mut payload = Vec::new();
            let requested_bytes = count
                .checked_mul(std::mem::size_of::<(
                    Vec<CanonicalStructuralPathSegment>,
                    TerminalScalarValue,
                )>())
                .ok_or_else(crate::boundary::errors::malformed)?;
            payload.try_reserve_exact(count).map_err(|reason| {
                TerminalInterpretError::StorageAllocationFailed {
                    requested_bytes,
                    reason: reason.to_string(),
                }
            })?;
            let mut fields = fields.into_iter();
            for _ in 0..count {
                let (field, scalar) = fields
                    .next()
                    .ok_or_else(crate::boundary::errors::malformed)??;
                payload.push((vec![CanonicalStructuralPathSegment::Field(field)], scalar));
            }
            if fields.next().is_some() {
                return Err(crate::boundary::errors::malformed());
            }
            let value = TerminalScalarAggregateValue {
                structural_type: declaration.id,
                result_case: Some(case),
                fields: payload,
            };
            value.validate_against(declaration, |_| None)?;
            Ok(Self {
                storage: std::sync::Arc::new(ScalarCaseStorage {
                    value,
                    _lease: lease,
                }),
            })
        }
    }

    impl std::fmt::Debug for ScalarCaseStorage {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(formatter)
        }
    }
    impl PartialEq for ScalarCaseStorage {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    impl Eq for ScalarCaseStorage {}

    pub(crate) fn meter_status(
        error: WorkMeterError,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        match error {
            WorkMeterError::Exhausted(exhaustion) => {
                Ok(TerminalExecutionStatus::SponsorExhausted(exhaustion))
            }
            other => Err(TerminalInterpretError::Work(other)),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalExecutionStatus {
        Complete(TerminalExecutionResult),
        SponsorExhausted(WorkBudgetExhausted),
        AwaitingEffect(TerminalEffectWait),
        HostComputationPending(TerminalEffectWait),
        Crashed(TerminalCrash),
        ExternalCompletion(TerminalExternalCompletion),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalEffectWait {
        pub program: terminal_psi::TerminalPsiIdentity,
        pub machine: MachineId,
        pub block: BlockId,
        pub operation: OperationId,
        pub boundary: BoundaryMachineId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalExternalCompletion {
        pub machine: MachineId,
        pub block: BlockId,
        pub edge: semantic_vocabulary::EdgeId,
        pub boundary: BoundaryMachineId,
        pub kind: semantic_vocabulary::ExternalCompletionKind,
        pub return_code: i32,
        pub frontier_lower_bound: Vec<ClaimId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalCrash {
        pub site: TerminalCrashSite,
        pub cause: CrashCause,
        pub site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        pub frontier_lower_bound: Vec<ClaimId>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TerminalCrashSite {
        Edge(semantic_vocabulary::EdgeId),
        Operation {
            machine: MachineId,
            block: BlockId,
            operation: OperationId,
        },
        BoundaryCall {
            machine: MachineId,
            block: BlockId,
            operation: OperationId,
            boundary: BoundaryMachineId,
        },
    }
}

pub(crate) mod values {
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::StructuralPathSegment;

    pub use terminal_installation::{TerminalScalarValue, TerminalStructuralValue};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TerminalScalarAggregateValue {
        pub structural_type: StructuralTypeId,
        pub result_case: Option<StructuralCaseId>,
        pub fields: Vec<(
            Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            TerminalScalarValue,
        )>,
    }

    pub(crate) mod aggregates {
        use super::TerminalScalarAggregateValue;
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
        use terminal_psi::{
            StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
            StructuralTypeDeclaration, StructuralTypeShape,
        };

        enum Element<'declaration> {
            Scalar(&'declaration StructuralFieldType),
            Structural(usize),
        }

        enum Shape<'declaration> {
            Scalar(StructuralFieldType),
            Record(
                Vec<(
                    &'declaration StructuralFieldDeclaration,
                    Element<'declaration>,
                )>,
            ),
            Array {
                element: usize,
                length: u64,
            },
        }

        struct Node<'declaration> {
            identity: StructuralTypeId,
            shape: Shape<'declaration>,
            count: Option<u64>,
            scalar_tree: bool,
        }

        pub(crate) struct ScalarAggregatePlan<'declaration> {
            nodes: Vec<Node<'declaration>>,
            root: usize,
        }

        pub(crate) enum ScalarAggregatePart<'declaration> {
            Field(&'declaration StructuralFieldType),
            Primitive(&'declaration StructuralFieldType),
            Array {
                structural_type: StructuralTypeId,
                count: usize,
            },
        }

        enum Visit<'declaration> {
            Node(usize),
            Field(
                &'declaration StructuralFieldDeclaration,
                &'declaration Element<'declaration>,
            ),
            Element {
                node: usize,
                index: u64,
                length: u64,
            },
            Pop,
        }

        impl<'declaration> ScalarAggregatePlan<'declaration> {
            pub(crate) fn new(
                declaration: &'declaration StructuralTypeDeclaration,
                lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
            ) -> Result<Self, TerminalInterpretError> {
                let mut nodes: Vec<Node<'declaration>> = Vec::new();
                let mut active = Vec::new();
                let mut pending = vec![(declaration, false)];
                while let Some((declaration, finish)) = pending.pop() {
                    if nodes.iter().any(|node| node.identity == declaration.id) {
                        continue;
                    }
                    if !finish {
                        if declaration.quotient_carrier.is_some()
                            || active.contains(&declaration.id)
                        {
                            return Err(malformed());
                        }
                        active.push(declaration.id);
                        pending.push((declaration, true));
                        let mut child = |identity| {
                            let declaration = lookup(identity)
                                .filter(|declaration| declaration.id == identity)
                                .ok_or_else(malformed)?;
                            pending.push((declaration, false));
                            Ok::<_, TerminalInterpretError>(())
                        };
                        match &declaration.shape {
                            StructuralTypeShape::PrimitiveScalar(_) => {}
                            StructuralTypeShape::Record { fields, storage } => {
                                if matches!(
                                    storage,
                                    StructuralRecordStorage::AccessView(_)
                                        | StructuralRecordStorage::Accessor(_)
                                ) {
                                    return Err(malformed());
                                }
                                for field in fields
                                    .iter()
                                    .rev()
                                    .filter(|field| !field.relevance.is_erased())
                                {
                                    if let StructuralFieldType::Structural(identity) =
                                        field.field_type
                                    {
                                        child(identity)?;
                                    } else if field.field_type.scalar_type().is_none() {
                                        return Err(malformed());
                                    }
                                }
                            }
                            StructuralTypeShape::FixedArray { element, .. } => child(*element)?,
                            _ => return Err(malformed()),
                        }
                        continue;
                    }
                    if active.pop() != Some(declaration.id) {
                        return Err(malformed());
                    }
                    let child = |identity| {
                        nodes
                            .iter()
                            .position(|node| node.identity == identity)
                            .ok_or_else(malformed)
                    };
                    let (shape, count, scalar_tree) = match &declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar) => {
                            let field_type = declaration
                                .scalar_restriction
                                .clone()
                                .unwrap_or(StructuralFieldType::Scalar(*scalar));
                            if field_type.scalar_type() != Some(*scalar) {
                                return Err(malformed());
                            }
                            (Shape::Scalar(field_type), Some(1), true)
                        }
                        StructuralTypeShape::Record { fields, .. } => {
                            let mut members = Vec::new();
                            let mut count = Some(0_u64);
                            for field in fields.iter().filter(|field| !field.relevance.is_erased())
                            {
                                let (element, leaves) = match field.field_type {
                                    StructuralFieldType::Structural(identity) => {
                                        let index = child(identity)?;
                                        (Element::Structural(index), nodes[index].count)
                                    }
                                    _ => (Element::Scalar(&field.field_type), Some(1)),
                                };
                                count = count
                                    .zip(leaves)
                                    .and_then(|(count, leaves)| count.checked_add(leaves));
                                members.push((field, element));
                            }
                            (Shape::Record(members), count, false)
                        }
                        StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } => {
                            let element = child(*element)?;
                            let count = if *length == 0 {
                                Some(0)
                            } else {
                                nodes[element]
                                    .count
                                    .and_then(|count| count.checked_mul(*length))
                            };
                            (
                                Shape::Array {
                                    element,
                                    length: *length,
                                },
                                count,
                                nodes[element].scalar_tree,
                            )
                        }
                        _ => return Err(malformed()),
                    };
                    nodes.push(Node {
                        identity: declaration.id,
                        shape,
                        count,
                        scalar_tree,
                    });
                }
                let root = nodes.len().checked_sub(1).ok_or_else(malformed)?;
                if nodes[root].count.is_none() {
                    return Err(malformed());
                }
                Ok(Self { nodes, root })
            }

            pub(crate) fn count(&self) -> Result<usize, TerminalInterpretError> {
                usize::try_from(self.nodes[self.root].count.ok_or_else(malformed)?)
                    .map_err(|_| malformed())
            }

            pub(crate) fn scalar_fields(
                &self,
                mut scalar: impl FnMut(
                    &[Path],
                    &StructuralFieldType,
                ) -> Result<(), TerminalInterpretError>,
            ) -> Result<(), TerminalInterpretError> {
                self.walk(false, |path, part| match part {
                    ScalarAggregatePart::Field(field) | ScalarAggregatePart::Primitive(field) => {
                        scalar(path, field)
                    }
                    ScalarAggregatePart::Array { .. } => Err(malformed()),
                })
            }

            pub(crate) fn storage_parts(
                &self,
                part: impl FnMut(&[Path], ScalarAggregatePart<'_>) -> Result<(), TerminalInterpretError>,
            ) -> Result<(), TerminalInterpretError> {
                self.walk(true, part)
            }

            fn walk(
                &self,
                group_scalar_arrays: bool,
                mut part: impl FnMut(
                    &[Path],
                    ScalarAggregatePart<'_>,
                ) -> Result<(), TerminalInterpretError>,
            ) -> Result<(), TerminalInterpretError> {
                let mut pending = vec![Visit::Node(self.root)];
                let mut path = Vec::new();
                while let Some(visit) = pending.pop() {
                    match visit {
                        Visit::Node(index) => {
                            let node = &self.nodes[index];
                            if group_scalar_arrays
                                && node.scalar_tree
                                && matches!(node.shape, Shape::Array { .. })
                            {
                                part(
                                    &path,
                                    ScalarAggregatePart::Array {
                                        structural_type: node.identity,
                                        count: usize::try_from(node.count.ok_or_else(malformed)?)
                                            .map_err(|_| malformed())?,
                                    },
                                )?;
                                continue;
                            }
                            if node.count == Some(0)
                                && (!group_scalar_arrays
                                    || matches!(node.shape, Shape::Array { .. }))
                            {
                                continue;
                            }
                            match &node.shape {
                                Shape::Scalar(field) => {
                                    part(&path, ScalarAggregatePart::Primitive(field))?
                                }
                                Shape::Record(fields) => pending.extend(
                                    fields
                                        .iter()
                                        .rev()
                                        .map(|(field, element)| Visit::Field(field, element)),
                                ),
                                Shape::Array { element, length } => pending.push(Visit::Element {
                                    node: *element,
                                    index: 0,
                                    length: *length,
                                }),
                            }
                        }
                        Visit::Field(field, element) => {
                            path.push(Path::Field(field.id));
                            pending.push(Visit::Pop);
                            match element {
                                Element::Scalar(field_type) => {
                                    part(&path, ScalarAggregatePart::Field(field_type))?
                                }
                                Element::Structural(index) => pending.push(Visit::Node(*index)),
                            }
                        }
                        Visit::Element {
                            node,
                            index,
                            length,
                        } => {
                            if index < length {
                                pending.push(Visit::Element {
                                    node,
                                    index: index + 1,
                                    length,
                                });
                                path.push(Path::FixedIndex(index));
                                pending.push(Visit::Pop);
                                pending.push(Visit::Node(node));
                            }
                        }
                        Visit::Pop => {
                            path.pop().ok_or_else(malformed)?;
                        }
                    }
                }
                Ok(())
            }
        }

        impl TerminalScalarAggregateValue {
            pub(crate) fn validate_against<'declaration>(
                &self,
                declaration: &'declaration StructuralTypeDeclaration,
                lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
            ) -> Result<(), TerminalInterpretError> {
                if declaration.id != self.structural_type || declaration.quotient_carrier.is_some()
                {
                    return Err(malformed());
                }
                let mut actual = self.fields.iter();
                let mut accept = |path: &[Path], field_type: &StructuralFieldType| {
                    let (found, value) = actual.next().ok_or_else(malformed)?;
                    if found != path
                        || field_type.scalar_type() != Some(value.scalar_type())
                        || !value.satisfies_field(field_type)
                    {
                        return Err(malformed());
                    }
                    Ok(())
                };
                match (&declaration.shape, self.result_case) {
                    (
                        StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::PrimitiveScalar(_),
                        None,
                    ) => {
                        let plan = ScalarAggregatePlan::new(declaration, lookup)?;
                        if plan.count()? != self.fields.len() {
                            return Err(malformed());
                        }
                        plan.scalar_fields(&mut accept)?;
                    }
                    (StructuralTypeShape::Sum { cases }, Some(selected)) => {
                        let case = terminal_psi::find_structural_case(cases, selected)
                            .ok_or_else(malformed)?;
                        for field in &case.fields {
                            if field.relevance.is_erased()
                                || field.field_type.scalar_type().is_none()
                            {
                                return Err(malformed());
                            }
                            accept(&[Path::Field(field.id)], &field.field_type)?;
                        }
                    }
                    _ => return Err(malformed()),
                }
                if actual.next().is_some() {
                    return Err(malformed());
                }
                Ok(())
            }
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::*;
            use crate::boundary::values::TerminalScalarValue;
            use semantic_vocabulary::{
                BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType,
                StructuralFieldId,
            };
            use terminal_psi::{BindingRelevance, TerminalIdentityIndex};

            fn identity(value: u32) -> StructuralTypeId {
                StructuralTypeId::new(value.into()).unwrap()
            }

            fn member(value: u32) -> Path {
                Path::Field(StructuralFieldId::new(value.into()).unwrap())
            }

            fn integer(bits: u16, sign: IntegerSign) -> IntegerType {
                IntegerType::new(sign, bits).unwrap()
            }

            fn byte() -> ScalarType {
                ScalarType::Integer(integer(8, IntegerSign::Unsigned))
            }

            fn signed() -> ScalarType {
                ScalarType::Integer(integer(64, IntegerSign::Signed))
            }

            fn byte_value(value: u64) -> TerminalScalarValue {
                TerminalScalarValue::Integer {
                    scalar_type: integer(8, IntegerSign::Unsigned),
                    value: IntegerValue::Unsigned(value.into()),
                }
            }

            fn signed_value(value: i64) -> TerminalScalarValue {
                TerminalScalarValue::Integer {
                    scalar_type: integer(64, IntegerSign::Signed),
                    value: IntegerValue::Signed(value.into()),
                }
            }

            fn field(value: u32, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
                StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: StructuralFieldId::new(value.into()).unwrap(),
                    identity: TerminalIdentityIndex(value),
                    relevance: BindingRelevance::Relevant,
                    field_type,
                    storage: None,
                }
            }

            fn declaration(value: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id: identity(value),
                    identity: TerminalIdentityIndex(value),
                    owns_linear_claim: false,
                    copyable: true,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    shape,
                }
            }

            fn record(
                value: u32,
                fields: Vec<StructuralFieldDeclaration>,
            ) -> StructuralTypeDeclaration {
                declaration(
                    value,
                    StructuralTypeShape::Record {
                        fields,
                        storage: StructuralRecordStorage::Packed,
                    },
                )
            }

            fn array(value: u32, element: u32, length: u64) -> StructuralTypeDeclaration {
                declaration(
                    value,
                    StructuralTypeShape::FixedArray {
                        element: identity(element),
                        length: length.into(),
                    },
                )
            }

            struct Fixture {
                declarations: Vec<StructuralTypeDeclaration>,
                payload: TerminalScalarAggregateValue,
            }

            impl Fixture {
                fn mixed_arrays() -> Self {
                    let declarations = vec![
                        record(
                            1,
                            vec![field(1, StructuralFieldType::Structural(identity(2)))],
                        ),
                        array(2, 3, 2),
                        record(
                            3,
                            vec![
                                field(1, StructuralFieldType::Scalar(signed())),
                                field(2, StructuralFieldType::Structural(identity(4))),
                            ],
                        ),
                        array(4, 5, 4),
                        declaration(5, StructuralTypeShape::PrimitiveScalar(byte())),
                    ];
                    let mut fields = Vec::new();
                    for index in 0..2 {
                        fields.push((
                            vec![member(1), Path::FixedIndex(index), member(1)],
                            signed_value(if index == 0 { -23_333_333_333 } else { -1 }),
                        ));
                        for guard in 0..4 {
                            fields.push((
                                vec![
                                    member(1),
                                    Path::FixedIndex(index),
                                    member(2),
                                    Path::FixedIndex(guard),
                                ],
                                byte_value(index * 4 + guard),
                            ));
                        }
                    }
                    Self {
                        declarations,
                        payload: TerminalScalarAggregateValue {
                            structural_type: identity(1),
                            result_case: None,
                            fields,
                        },
                    }
                }

                fn lookup(&self, value: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
                    self.declarations.iter().find(|row| row.id == value)
                }

                fn plan(&self) -> Result<ScalarAggregatePlan<'_>, TerminalInterpretError> {
                    ScalarAggregatePlan::new(&self.declarations[0], |value| self.lookup(value))
                }

                fn validate(&self) -> Result<(), TerminalInterpretError> {
                    self.payload
                        .validate_against(&self.declarations[0], |value| self.lookup(value))
                }
            }

            #[test]
            fn records_arrays_and_primitives_retain_each_complete_ordered_index_path() {
                let fixture = Fixture::mixed_arrays();
                let plan = fixture.plan().unwrap();
                assert_eq!(plan.count().unwrap(), 10);
                let mut observed = Vec::new();
                plan.scalar_fields(|path, field| {
                    observed.push((path.to_vec(), field.scalar_type().unwrap()));
                    Ok(())
                })
                .unwrap();
                let expected = fixture
                    .payload
                    .fields
                    .iter()
                    .map(|(path, value)| (path.clone(), value.scalar_type()))
                    .collect::<Vec<_>>();
                assert_eq!(observed, expected);
                assert!(fixture.validate().is_ok());
            }

            #[test]
            fn missing_duplicate_reordered_extra_and_out_of_range_leaves_reject() {
                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields.pop();
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields[6] = fixture.payload.fields[1].clone();
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields.swap(1, 2);
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture
                    .payload
                    .fields
                    .push(fixture.payload.fields[0].clone());
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields[5].0[1] = Path::FixedIndex(2);
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields[4].0[3] = Path::FixedIndex(4);
                assert!(fixture.validate().is_err());

                let mut fixture = Fixture::mixed_arrays();
                fixture.payload.fields[1].1 = signed_value(0);
                assert!(fixture.validate().is_err());
            }

            #[test]
            fn primitive_declaration_restriction_is_enforced_on_every_array_element() {
                let mut fixture = Fixture::mixed_arrays();
                fixture.declarations[4].scalar_restriction =
                    Some(StructuralFieldType::BoundedInteger(
                        BoundedIntegerType::new(
                            integer(8, IntegerSign::Unsigned),
                            IntegerValue::Unsigned(0),
                            IntegerValue::Unsigned(127),
                        )
                        .unwrap(),
                    ));
                assert!(fixture.validate().is_ok());
                fixture.payload.fields[9].1 = byte_value(128);
                assert!(fixture.validate().is_err());
            }

            #[test]
            fn storage_parts_group_each_byte_array_once_but_walk_record_array_elements() {
                let fixture = Fixture::mixed_arrays();
                let plan = fixture.plan().unwrap();
                let mut observed = Vec::new();
                plan.storage_parts(|path, part| {
                    let description = match part {
                        ScalarAggregatePart::Field(field) => {
                            assert_eq!(field.scalar_type(), Some(signed()));
                            None
                        }
                        ScalarAggregatePart::Array {
                            structural_type,
                            count,
                        } => Some((structural_type, count)),
                        ScalarAggregatePart::Primitive(_) => {
                            panic!("guard bytes must be grouped for byte backing")
                        }
                    };
                    observed.push((path.to_vec(), description));
                    Ok(())
                })
                .unwrap();
                assert_eq!(
                    observed,
                    vec![
                        (vec![member(1), Path::FixedIndex(0), member(1)], None),
                        (
                            vec![member(1), Path::FixedIndex(0), member(2)],
                            Some((identity(4), 4))
                        ),
                        (vec![member(1), Path::FixedIndex(1), member(1)], None),
                        (
                            vec![member(1), Path::FixedIndex(1), member(2)],
                            Some((identity(4), 4))
                        ),
                    ]
                );
            }

            #[test]
            fn multidimensional_scalar_array_has_full_leaf_paths_and_one_storage_part() {
                let declarations = [
                    array(1, 2, 2),
                    array(2, 3, 2),
                    declaration(3, StructuralTypeShape::PrimitiveScalar(byte())),
                ];
                let lookup = |value| declarations.iter().find(|row| row.id == value);
                let plan = ScalarAggregatePlan::new(&declarations[0], lookup).unwrap();
                let mut paths = Vec::new();
                plan.scalar_fields(|path, _| {
                    paths.push(path.to_vec());
                    Ok(())
                })
                .unwrap();
                assert_eq!(
                    paths,
                    vec![
                        vec![Path::FixedIndex(0), Path::FixedIndex(0)],
                        vec![Path::FixedIndex(0), Path::FixedIndex(1)],
                        vec![Path::FixedIndex(1), Path::FixedIndex(0)],
                        vec![Path::FixedIndex(1), Path::FixedIndex(1)],
                    ]
                );
                let mut groups = Vec::new();
                plan.storage_parts(|path, part| {
                    let ScalarAggregatePart::Array {
                        structural_type,
                        count,
                    } = part
                    else {
                        panic!("pure scalar array must use the existing array storage owner")
                    };
                    groups.push((path.to_vec(), structural_type, count));
                    Ok(())
                })
                .unwrap();
                assert_eq!(groups, vec![(Vec::new(), identity(1), 4)]);
            }

            #[test]
            fn zero_array_dimensions_do_not_hide_missing_cyclic_or_unsupported_elements() {
                let missing = array(1, 99, 0);
                assert!(ScalarAggregatePlan::new(&missing, |_| None).is_err());

                let cyclic = array(1, 1, 0);
                assert!(
                    ScalarAggregatePlan::new(&cyclic, |value| (value == cyclic.id)
                        .then_some(&cyclic))
                    .is_err()
                );

                for unsupported in [
                    StructuralTypeShape::Reference {
                        referent: identity(1),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                    StructuralTypeShape::Sum { cases: Vec::new() },
                ] {
                    let declarations = [array(1, 2, 0), declaration(2, unsupported)];
                    assert!(
                        ScalarAggregatePlan::new(&declarations[0], |value| declarations
                            .iter()
                            .find(|row| row.id == value))
                        .is_err()
                    );
                }
            }

            #[test]
            fn repeated_empty_record_schema_has_no_materialized_leaves_or_storage_parts() {
                let declarations = [array(1, 2, u64::MAX), record(2, Vec::new())];
                let lookup = |value| declarations.iter().find(|row| row.id == value);
                let plan = ScalarAggregatePlan::new(&declarations[0], lookup).unwrap();
                assert_eq!(plan.count().unwrap(), 0);
                plan.scalar_fields(|_, _| panic!("empty records have no scalar payload"))
                    .unwrap();
                plan.storage_parts(|_, _| panic!("empty records need no scalar storage"))
                    .unwrap();
                let payload = TerminalScalarAggregateValue {
                    structural_type: identity(1),
                    result_case: None,
                    fields: Vec::new(),
                };
                assert!(payload.validate_against(&declarations[0], lookup).is_ok());
            }

            #[test]
            fn outer_zero_suppresses_inner_count_overflow_without_skipping_schema_validation() {
                let mut declarations = [
                    array(1, 2, 0),
                    array(2, 3, u64::MAX),
                    array(3, 4, u64::MAX),
                    declaration(4, StructuralTypeShape::PrimitiveScalar(byte())),
                ];
                let plan = ScalarAggregatePlan::new(&declarations[0], |value| {
                    declarations.iter().find(|row| row.id == value)
                })
                .unwrap();
                assert_eq!(plan.count().unwrap(), 0);
                plan.scalar_fields(|_, _| {
                    panic!("zero outer dimension creates no scalar instances")
                })
                .unwrap();
                let mut groups = Vec::new();
                plan.storage_parts(|path, part| {
                    let ScalarAggregatePart::Array {
                        structural_type,
                        count,
                    } = part
                    else {
                        panic!("scalar array grouping lost")
                    };
                    groups.push((path.to_vec(), structural_type, count));
                    Ok(())
                })
                .unwrap();
                assert_eq!(groups, vec![(Vec::new(), identity(1), 0)]);
                assert!(
                    ScalarAggregatePlan::new(&declarations[1], |value| declarations
                        .iter()
                        .find(|row| row.id == value))
                    .is_err()
                );
                declarations[3].shape = StructuralTypeShape::Reference {
                    referent: identity(4),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
                assert!(
                    ScalarAggregatePlan::new(&declarations[0], |value| declarations
                        .iter()
                        .find(|row| row.id == value))
                    .is_err()
                );
            }
        }
    }
    pub(crate) use aggregates::{ScalarAggregatePart, ScalarAggregatePlan};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalStructuralBooleanFieldValue {
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
        pub field: StructuralFieldId,
        pub value: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalStructuralPrimitiveValue {
        pub argument_index: u32,
        pub value: TerminalScalarValue,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct StructuralRuntimePlace {
        pub(crate) opaque_identity: u64,
        pub(crate) path: Vec<StructuralPathSegment>,
    }

    pub(crate) fn structural_storage_path(
        path: &[StructuralPathSegment],
    ) -> &[StructuralPathSegment] {
        match path.split_last() {
            Some((StructuralPathSegment::Case(_), parent)) => parent,
            _ => path,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum StructuralFieldNamespace {
        Member,
        CasePayload,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct StructuralScalarRuntimeField {
        pub(crate) parent: StructuralRuntimePlace,
        pub(crate) namespace: StructuralFieldNamespace,
        pub(crate) field: StructuralFieldId,
    }

    impl StructuralScalarRuntimeField {
        pub(crate) fn member(parent: StructuralRuntimePlace, field: StructuralFieldId) -> Self {
            Self {
                parent,
                namespace: StructuralFieldNamespace::Member,
                field,
            }
        }

        pub(crate) fn case_payload(
            parent: StructuralRuntimePlace,
            field: StructuralFieldId,
        ) -> Self {
            Self {
                parent,
                namespace: StructuralFieldNamespace::CasePayload,
                field,
            }
        }

        pub(crate) fn relocated(&self, parent: StructuralRuntimePlace) -> Self {
            Self {
                parent,
                namespace: self.namespace,
                field: self.field,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct StructuralByteSequenceRuntimeField {
        pub(crate) parent: StructuralRuntimePlace,
        pub(crate) namespace: StructuralFieldNamespace,
        pub(crate) field: StructuralFieldId,
    }

    impl StructuralByteSequenceRuntimeField {
        pub(crate) fn relocated(&self, parent: StructuralRuntimePlace) -> Self {
            Self {
                parent,
                namespace: self.namespace,
                field: self.field,
            }
        }
    }

    impl StructuralFieldNamespace {
        pub(crate) fn for_carrier(path: &[StructuralPathSegment]) -> Self {
            if matches!(path.last(), Some(StructuralPathSegment::Case(_))) {
                Self::CasePayload
            } else {
                Self::Member
            }
        }
    }

    impl From<&TerminalStructuralValue> for StructuralRuntimePlace {
        fn from(value: &TerminalStructuralValue) -> Self {
            Self {
                opaque_identity: value.opaque_identity,
                path: value.path.clone(),
            }
        }
    }

    pub(crate) fn scalar_view_value(
        value: &TerminalScalarValue,
    ) -> Option<terminal_semantics::ScalarViewValue> {
        Some(match value {
            TerminalScalarValue::Callable { .. } => return None,
            TerminalScalarValue::Boolean(value) => terminal_semantics::ScalarViewValue::Boolean(*value),
            TerminalScalarValue::Integer { value, .. } => terminal_semantics::ScalarViewValue::Integer(*value),
            TerminalScalarValue::IeeeFloat(value) => terminal_semantics::ScalarViewValue::IeeeFloat(*value),
        })
    }

    #[cfg(test)]
    mod scalar_aggregate_tests {
        use super::*;
        use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType};
        use terminal_psi::{
            BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
            StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
        };

        fn integer() -> IntegerType {
            IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64).unwrap()
        }
        fn declaration() -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: StructuralTypeId::new(1).unwrap(),
                identity: TerminalIdentityIndex(1),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                shape: StructuralTypeShape::Record {
                    fields: (1..=2)
                        .map(|id| StructuralFieldDeclaration {
                            inline_carrier: None,
                            id: StructuralFieldId::new(id).unwrap(),
                            identity: TerminalIdentityIndex(id as u32),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer())),
                            storage: None,
                        })
                        .collect(),
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
                scalar_restriction: None,
            }
        }
        fn payload() -> TerminalScalarAggregateValue {
            TerminalScalarAggregateValue {
                structural_type: StructuralTypeId::new(1).unwrap(),
                result_case: None,
                fields: (1..=2)
                    .map(|id| {
                        (
                            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                StructuralFieldId::new(id).unwrap(),
                            )],
                            TerminalScalarValue::Integer {
                                scalar_type: integer(),
                                value: IntegerValue::Signed(-(id as i128)),
                            },
                        )
                    })
                    .collect(),
            }
        }
        #[test]
        fn scalar_record_requires_complete_ordered_typed_fields() {
            let d = declaration();
            let p = payload();
            assert!(p.validate_against(&d, |_| None).is_ok());
            let mut bad = p.clone();
            bad.fields.swap(0, 1);
            assert!(bad.validate_against(&d, |_| None).is_err());
            let mut bad = p.clone();
            bad.fields.pop();
            assert!(bad.validate_against(&d, |_| None).is_err());
            let mut bad = p.clone();
            bad.fields.push(p.fields[0].clone());
            assert!(bad.validate_against(&d, |_| None).is_err());
            let mut bad = p.clone();
            bad.fields[0].1 = TerminalScalarValue::Boolean(true);
            assert!(bad.validate_against(&d, |_| None).is_err());
            let mut bad = p.clone();
            bad.fields[0].1 = TerminalScalarValue::Integer {
                scalar_type: integer(),
                value: IntegerValue::Signed(i128::MAX),
            };
            assert!(bad.validate_against(&d, |_| None).is_err());
            let mut bad = p.clone();
            bad.structural_type = StructuralTypeId::new(2).unwrap();
            assert!(bad.validate_against(&d, |_| None).is_err());
        }
        #[test]
        fn record_and_sum_discriminators_cannot_be_interchanged() {
            let record = declaration();
            let p = payload();
            let case = StructuralCaseId::new(1).unwrap();
            let mut tagged = p.clone();
            tagged.result_case = Some(case);
            assert!(tagged.validate_against(&record, |_| None).is_err());
            let StructuralTypeShape::Record { fields, .. } = record.shape.clone() else {
                unreachable!()
            };
            let mut sum = record;
            sum.shape = StructuralTypeShape::Sum {
                cases: vec![terminal_psi::StructuralCaseDeclaration {
                    id: case,
                    identity: TerminalIdentityIndex(3),
                    fields,
                }],
            };
            assert!(p.validate_against(&sum, |_| None).is_err());
            assert!(tagged.validate_against(&sum, |_| None).is_ok());
            tagged.result_case = Some(StructuralCaseId::new(2).unwrap());
            assert!(tagged.validate_against(&sum, |_| None).is_err());
        }
        #[test]
        fn nested_scalar_record_binds_complete_containing_paths() {
            use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
            let mut child = declaration();
            child.id = StructuralTypeId::new(2).unwrap();
            let mut root = declaration();
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut root.shape else {
                unreachable!()
            };
            fields.truncate(1);
            fields[0].field_type = terminal_psi::StructuralFieldType::Structural(child.id);
            let mut value = payload();
            for (path, _) in &mut value.fields {
                path.insert(0, Path::Field(StructuralFieldId::new(1).unwrap()));
            }
            let lookup = |identity| (identity == child.id).then_some(&child);
            assert!(value.validate_against(&root, lookup).is_ok());
            assert!(value.validate_against(&root, |_| None).is_err());
            let mut flat = value.clone();
            for (path, _) in &mut flat.fields {
                path.remove(0);
            }
            assert!(flat.validate_against(&root, lookup).is_err());
            let mut changed = value.clone();
            changed.fields[0].0[0] = Path::Field(StructuralFieldId::new(2).unwrap());
            assert!(changed.validate_against(&root, lookup).is_err());
            let mut missing = value.clone();
            missing.fields.pop();
            assert!(missing.validate_against(&root, lookup).is_err());
            let mut repeated = value.clone();
            repeated.fields[1] = repeated.fields[0].clone();
            assert!(repeated.validate_against(&root, lookup).is_err());
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut child.shape else {
                unreachable!()
            };
            fields.clear();
            value.fields.clear();
            assert!(
                value
                    .validate_against(&root, |identity| (identity == child.id).then_some(&child))
                    .is_ok()
            );
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut child.shape else {
                unreachable!()
            };
            let terminal_psi::StructuralTypeShape::Record {
                fields: root_fields,
                ..
            } = &root.shape
            else {
                unreachable!()
            };
            fields.push(root_fields[0].clone());
            assert!(
                value
                    .validate_against(&root, |identity| (identity == child.id).then_some(&child))
                    .is_err()
            );
        }
    }
}
