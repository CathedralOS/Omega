//! Fuel-bounded reference execution for verified terminal-Psi artifacts.
//!
//! The public entry accepts only canonical semantic/proof bytes and an
//! admission profile. It decodes and verifies those bytes before constructing
//! execution state; no source or checked-tree representation crosses this
//! boundary.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimSettlement, ClaimTransfer, CrashCause, EntryClaim,
    OperationKind, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalMachineResult, Terminator,
};
use psi_terminal_fuel::{FuelExhaustion, FuelMeterError, TerminalFuelMeter, TerminalFuelUsage};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Decode, verify, and execute the canonical semantic and proof sections of one
/// terminal-Psi artifact. This is the reference-interpreter trust boundary for
/// executable artifact content: no source, checked tree, producer-owned module,
/// or prevalidated Rust object crosses it. Installation and debug sections are
/// separately bound by the artifact manifest and do not affect interpretation.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut handler = AcceptTerminalEffects;
    interpret_terminal_artifact_with_effect_handler_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        arguments,
        &[],
        &mut handler,
    )
}

/// Execute one verified artifact with opaque structural runtime arguments and
/// an injected deterministic effect handler. The interpreter records every
/// accepted effect in semantic execution order; the handler cannot inspect or
/// mutate fuel, values, claims, or control state.
pub fn interpret_terminal_artifact_with_effect_handler_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
    )?;
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume_with_effect_handler(&mut meter, handler)
        .map_err(TerminalArtifactInterpretError::Execution)?
    {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Fuel(FuelMeterError::Exhausted(exhaustion)),
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(crash),
            ));
        }
    };
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
        effects: execution.effects,
    })
}

/// Decode, verify, and execute canonical terminal-Psi semantic/proof artifact
/// sections, returning only their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_kernel::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(semantic_bytes, proof_bytes, profile, arguments)
        .map(MeasuredTerminalExecution::into_value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

/// Opaque target-neutral runtime carrier for one structural argument.
///
/// `opaque_identity` is chosen by the embedding host and is only preserved for
/// argument forwarding and deterministic effect observation. Psi never treats
/// it as an address or layout. Qualification IDs are semantic runtime facts
/// supplied by the root installation and must be strictly increasing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
}

/// One externally observable terminal-Psi effect in semantic execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffect {
    BoundaryCallUnit {
        operation: OperationId,
        boundary: BoundaryMachineId,
        structural_arguments: Vec<TerminalStructuralValue>,
        claim_settlements: Vec<ClaimSettlement>,
    },
    PortWrite {
        operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
}

/// Injected semantic effect sink used by the oracle and tests. Native provider
/// selection and hardware realization remain outside the Psi interpreter.
pub trait TerminalEffectHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEffectRejection {
    pub reason: String,
}

impl TerminalEffectRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct AcceptTerminalEffects;

impl TerminalEffectHandler for AcceptTerminalEffects {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }
}

/// The normal result of terminal-Psi execution.
///
/// Unit is a successful absence of a value, not a distinguished scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecutionResult {
    Unit,
    Scalar(TerminalScalarValue),
}

impl TerminalScalarValue {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

/// Resumable execution state created from canonical terminal-Psi artifact
/// sections.
///
/// Fuel exhaustion never advances `next_operation` or the current terminator,
/// so a sponsor can replenish the same meter and resume without replaying
/// semantic work or charging it twice.
pub struct TerminalExecution {
    machines: BTreeMap<MachineId, ExecutableMachine>,
    boundary_machines: BTreeMap<BoundaryMachineId, BoundaryMachineDeclaration>,
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    live_claims: BTreeMap<ClaimId, LiveClaim>,
    current_machine: MachineId,
    current: BlockId,
    next_operation: usize,
    call_stack: Vec<SuspendedCall>,
    result: Option<TerminalExecutionResult>,
    crash: Option<TerminalCrash>,
    effects: Vec<TerminalEffect>,
}

#[derive(Clone)]
struct ExecutableMachine {
    parameters: Vec<psi_terminal::ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    entry_claims: Vec<EntryClaim>,
    content_entry_claims: Vec<psi_terminal::ContentEntryClaim>,
    result: TerminalMachineResult,
    entry: BlockId,
    blocks: BTreeMap<BlockId, Block>,
}

struct SuspendedCall {
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    live_claims: BTreeMap<ClaimId, LiveClaim>,
    current_machine: MachineId,
    current: BlockId,
    next_operation: usize,
    result: SuspendedCallResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveClaim {
    place: Option<PlaceId>,
    multiplicity: Option<StructuralMultiplicity>,
}

enum SuspendedCallResult {
    Scalar(ValueId),
    Unit,
}

impl TerminalExecution {
    /// Canonical-decode, verify, and begin one resumable artifact execution.
    /// The resulting state owns its verified entry block graph, so no decoded
    /// producer object or self-referential verifier borrow escapes this entry.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &psi_proof_kernel::AdmissionProfile,
        arguments: &[TerminalScalarValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_arguments(
            semantic_bytes,
            proof_bytes,
            profile,
            arguments,
            &[],
        )
    }

    pub fn start_artifact_with_structural_arguments(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &psi_proof_kernel::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = psi_terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        Self::start(&verified, scalar_arguments, structural_arguments)
            .map_err(TerminalArtifactInterpretError::Execution)
    }

    fn start(
        verified: &VerifiedTerminalModule<'_>,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
    ) -> Result<Self, TerminalInterpretError> {
        let module = verified.module();
        let machines = module
            .machines
            .iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        parameters: machine.parameters.clone(),
                        structural_parameters: machine.structural_parameters.clone(),
                        entry_claims: machine.entry_claims.clone(),
                        content_entry_claims: machine.content_entry_claims.clone(),
                        result: machine.result,
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .iter()
                            .map(|block| (block.id, block.clone()))
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let boundary_machines = module
            .boundary_machines
            .iter()
            .cloned()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let machine = machines
            .get(&module.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        let values = bind_arguments(&machine.parameters, scalar_arguments)?;
        let structural_values =
            bind_structural_arguments(&machine.structural_parameters, structural_arguments)?;
        let live_claims = bind_entry_claims(
            &machine.entry_claims,
            &machine.content_entry_claims,
            &machine.structural_parameters,
            &structural_values,
        )?;
        let blocks = machine.blocks.clone();
        let current = machine.entry;
        Ok(Self {
            machines,
            boundary_machines,
            blocks,
            values,
            structural_values,
            live_claims,
            current_machine: module.entry,
            current,
            next_operation: 0,
            call_stack: Vec::new(),
            result: None,
            crash: None,
            effects: Vec::new(),
        })
    }

    pub fn resume(
        &mut self,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        let mut handler = AcceptTerminalEffects;
        self.resume_with_effect_handler(meter, &mut handler)
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    pub fn live_claim_frontier(&self) -> impl Iterator<Item = ClaimId> + '_ {
        self.live_claims.keys().copied()
    }

    pub fn resume_with_effect_handler(
        &mut self,
        meter: &mut TerminalFuelMeter,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        if let Some(result) = self.result {
            return Ok(TerminalExecutionStatus::Complete(result));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        loop {
            while let Some(operation) = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .operations
                .get(self.next_operation)
                .cloned()
            {
                if let Err(error) = meter.charge_operation(&operation) {
                    return meter_status(error);
                }
                match operation.kind.clone() {
                    OperationKind::CallUnit {
                        callee,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        if !matches!(operation.result, psi_terminal::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let callee_id = callee;
                        let callee =
                            self.machines.get(&callee_id).cloned().ok_or(
                                TerminalInterpretError::VerifiedCallTargetMissing(callee_id),
                            )?;
                        if callee.result != TerminalMachineResult::Unit
                            || !callee.parameters.is_empty()
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let arguments = resolve_structural_arguments(
                            &self.structural_values,
                            &structural_arguments,
                        )?;
                        let structural_values =
                            bind_structural_arguments(&callee.structural_parameters, &arguments)?;
                        let (remaining_claims, live_claims) = transfer_claims(
                            &self.live_claims,
                            &self.structural_values,
                            &structural_arguments,
                            &claim_transfers,
                            &callee.structural_parameters,
                            &callee.entry_claims,
                            &callee.content_entry_claims,
                            &structural_values,
                        )?;
                        self.next_operation += 1;
                        self.live_claims = remaining_claims;
                        self.call_stack.push(SuspendedCall {
                            blocks: std::mem::take(&mut self.blocks),
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            live_claims: std::mem::take(&mut self.live_claims),
                            current_machine: self.current_machine,
                            current: self.current,
                            next_operation: self.next_operation,
                            result: SuspendedCallResult::Unit,
                        });
                        self.blocks = callee.blocks;
                        self.values = BTreeMap::new();
                        self.structural_values = structural_values;
                        self.live_claims = live_claims;
                        self.current_machine = callee_id;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        continue;
                    }
                    OperationKind::BoundaryCallUnit {
                        boundary,
                        structural_arguments,
                        claim_settlements,
                        ..
                    } => {
                        if !matches!(operation.result, psi_terminal::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let boundary_declaration = self.boundary_machines.get(&boundary).ok_or(
                            TerminalInterpretError::VerifiedBoundaryMachineMissing(boundary),
                        )?;
                        let arguments = resolve_structural_arguments(
                            &self.structural_values,
                            &structural_arguments,
                        )?;
                        bind_structural_arguments(
                            &boundary_declaration.structural_parameters,
                            &arguments,
                        )?;
                        validate_boundary_requirements(boundary_declaration, &arguments)?;
                        let remaining_claims = settle_claims(
                            &self.live_claims,
                            &structural_arguments,
                            &claim_settlements,
                            &boundary_declaration.structural_parameters,
                        )?;
                        let effect = TerminalEffect::BoundaryCallUnit {
                            operation: operation.id,
                            boundary,
                            structural_arguments: arguments,
                            claim_settlements,
                        };
                        handler.handle_effect(&effect).map_err(|rejection| {
                            TerminalInterpretError::EffectRejected {
                                operation: operation.id,
                                rejection,
                            }
                        })?;
                        self.live_claims = remaining_claims;
                        self.effects.push(effect);
                    }
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    } => {
                        if !matches!(operation.result, psi_terminal::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let effect = TerminalEffect::PortWrite {
                            operation: operation.id,
                            service,
                            port,
                            value,
                        };
                        handler.handle_effect(&effect).map_err(|rejection| {
                            TerminalInterpretError::EffectRejected {
                                operation: operation.id,
                                rejection,
                            }
                        })?;
                        self.effects.push(effect);
                    }
                    OperationKind::Call {
                        callee, arguments, ..
                    } => {
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let callee_id = callee;
                        let callee =
                            self.machines.get(&callee_id).cloned().ok_or(
                                TerminalInterpretError::VerifiedCallTargetMissing(callee_id),
                            )?;
                        if !callee.structural_parameters.is_empty()
                            || !callee.entry_claims.is_empty()
                            || !callee.content_entry_claims.is_empty()
                            || !matches!(callee.result, TerminalMachineResult::Scalar(_))
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let values = bind_arguments(&callee.parameters, &arguments)?;
                        self.next_operation += 1;
                        self.call_stack.push(SuspendedCall {
                            blocks: std::mem::take(&mut self.blocks),
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            live_claims: std::mem::take(&mut self.live_claims),
                            current_machine: self.current_machine,
                            current: self.current,
                            next_operation: self.next_operation,
                            result: SuspendedCallResult::Scalar(
                                operation.result.expect_scalar().id,
                            ),
                        });
                        self.blocks = callee.blocks;
                        self.values = values;
                        self.structural_values = BTreeMap::new();
                        self.live_claims = BTreeMap::new();
                        self.current_machine = callee_id;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        continue;
                    }
                    OperationKind::IntegerConstant { value } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::BooleanConstant { value } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(value),
                        );
                    }
                    OperationKind::BooleanNot { operand } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(value) = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(!value),
                        );
                    }
                    OperationKind::BooleanEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(left) = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Boolean(right) = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerLessThan { left, right }
                    | OperationKind::IntegerLessOrEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let ordering = left_type
                            .compare(left_value, right_value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let result = match operation.kind.clone() {
                            OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
                            OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                            _ => unreachable!(),
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(result),
                        );
                    }
                    OperationKind::IntegerBitwiseNot { operand } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: operand_type,
                            value: operand,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if operand_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .bitwise_not(operand)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::IntegerWiden { operand } => {
                        let ScalarType::Integer(target_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .widen_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
                        );
                    }
                    OperationKind::IntegerExactCast { operand, .. } => {
                        let ScalarType::Integer(target_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .exact_cast_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
                        );
                    }
                    OperationKind::IntegerBitwiseAnd { left, right }
                    | OperationKind::IntegerBitwiseOr { left, right }
                    | OperationKind::IntegerBitwiseXor { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind.clone() {
                            OperationKind::IntegerBitwiseAnd { .. } => {
                                scalar_type.bitwise_and(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseOr { .. } => {
                                scalar_type.bitwise_or(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseXor { .. } => {
                                scalar_type.bitwise_xor(left_value, right_value)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::WrappingIntegerShiftLeft { value, count }
                    | OperationKind::WrappingIntegerShiftRight { value, count }
                    | OperationKind::ExactIntegerShiftLeft { value, count, .. }
                    | OperationKind::ExactIntegerShiftRight { value, count, .. } => {
                        let ScalarType::Integer(value_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: actual_value_type,
                            value,
                        } = self
                            .values
                            .get(&value)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: count_type,
                            value: count,
                        } = self
                            .values
                            .get(&count)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(count))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if actual_value_type != value_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind.clone() {
                            OperationKind::WrappingIntegerShiftLeft { .. } => {
                                value_type.wrapping_shift_left(value, count_type, count)
                            }
                            OperationKind::WrappingIntegerShiftRight { .. } => {
                                value_type.wrapping_shift_right(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftLeft { .. } => {
                                value_type.exact_shift_left(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftRight { .. } => {
                                value_type.exact_shift_right(value, count_type, count)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: value_type,
                                value,
                            },
                        );
                    }
                    OperationKind::ExactIntegerAdd { left, right, .. }
                    | OperationKind::WrappingIntegerAdd { left, right }
                    | OperationKind::ExactIntegerSubtract { left, right, .. }
                    | OperationKind::WrappingIntegerSubtract { left, right }
                    | OperationKind::ExactIntegerMultiply { left, right, .. }
                    | OperationKind::ExactIntegerDivide { left, right, .. }
                    | OperationKind::ExactIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerDivide { left, right, .. }
                    | OperationKind::WrappingIntegerRemainder { left, right, .. }
                    | OperationKind::SaturatingIntegerDivide { left, right, .. }
                    | OperationKind::SaturatingIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind.clone() {
                            OperationKind::ExactIntegerAdd { .. } => {
                                scalar_type.exact_add(left, right)
                            }
                            OperationKind::WrappingIntegerAdd { .. } => {
                                scalar_type.wrapping_add(left, right)
                            }
                            OperationKind::ExactIntegerSubtract { .. } => {
                                scalar_type.exact_sub(left, right)
                            }
                            OperationKind::WrappingIntegerSubtract { .. } => {
                                scalar_type.wrapping_sub(left, right)
                            }
                            OperationKind::ExactIntegerMultiply { .. } => {
                                scalar_type.exact_mul(left, right)
                            }
                            OperationKind::ExactIntegerDivide { .. } => {
                                scalar_type.exact_div(left, right)
                            }
                            OperationKind::ExactIntegerRemainder { .. } => {
                                scalar_type.exact_rem(left, right)
                            }
                            OperationKind::WrappingIntegerDivide { .. } => {
                                scalar_type.wrapping_div(left, right)
                            }
                            OperationKind::WrappingIntegerRemainder { .. } => {
                                scalar_type.wrapping_rem(left, right)
                            }
                            OperationKind::SaturatingIntegerDivide { .. } => {
                                scalar_type.saturating_div(left, right)
                            }
                            OperationKind::SaturatingIntegerRemainder { .. } => {
                                scalar_type.saturating_rem(left, right)
                            }
                            OperationKind::WrappingIntegerMultiply { .. } => {
                                scalar_type.wrapping_mul(left, right)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerAdd { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_add(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerSubtract { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_sub(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_mul(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                }
                self.next_operation += 1;
            }
            let terminator = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator
                .clone();
            match &terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => {
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(target)
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
                    self.current = *target;
                    self.next_operation = 0;
                }
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let condition = self
                        .values
                        .get(condition)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*condition))?;
                    let TerminalScalarValue::Boolean(condition) = condition else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    let successor = if condition { when_true } else { when_false };
                    if let Err(error) = meter.charge_edge(successor.edge, &terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(&successor.target)
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = successor
                        .arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
                    self.current = successor.target;
                    self.next_operation = 0;
                }
                Terminator::Return {
                    value,
                    trivial_affine_discards,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if !matches!(machine.result, TerminalMachineResult::Scalar(_))
                        || has_live_linear_claims(&self.live_claims)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let result = self
                        .values
                        .get(value)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
                    for place in trivial_affine_discards {
                        if self.structural_values.remove(place).is_none() {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                    }
                    if let Some(caller) = self.call_stack.pop() {
                        let SuspendedCallResult::Scalar(result_value) = caller.result else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.values.insert(result_value, result);
                        self.structural_values = caller.structural_values;
                        self.live_claims = caller.live_claims;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Scalar(result);
                    self.result = Some(result);
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if machine.result != TerminalMachineResult::Unit
                        || has_live_linear_claims(&self.live_claims)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    for place in trivial_affine_discards {
                        if self.structural_values.remove(place).is_none() {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                    }
                    if let Some(caller) = self.call_stack.pop() {
                        if !matches!(caller.result, SuspendedCallResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.structural_values = caller.structural_values;
                        self.live_claims = caller.live_claims;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Unit;
                    self.result = Some(result);
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Crash {
                    edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } => {
                    if frontier_lower_bound != &self.live_claims.keys().copied().collect::<Vec<_>>()
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let crash = TerminalCrash {
                        edge: *edge,
                        cause: *cause,
                        site_guard: site_guard.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                    };
                    self.crash = Some(crash.clone());
                    return Ok(TerminalExecutionStatus::Crashed(crash));
                }
            }
        }
    }
}

fn bind_arguments(
    parameters: &[psi_terminal::ValueDeclaration],
    arguments: &[TerminalScalarValue],
) -> Result<BTreeMap<ValueId, TerminalScalarValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.scalar_type != argument.scalar_type() {
            return Err(TerminalInterpretError::ArgumentType {
                value: parameter.id,
                expected: parameter.scalar_type,
                actual: argument.scalar_type(),
            });
        }
        if let TerminalScalarValue::Integer { scalar_type, value } = argument
            && !scalar_type.admits(*value)
        {
            return Err(TerminalInterpretError::ArgumentIntegerOutsideType {
                value: parameter.id,
            });
        }
        values.insert(parameter.id, *argument);
    }
    Ok(values)
}

fn bind_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut bound_identities = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if argument
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(TerminalInterpretError::StructuralQualificationsNonCanonical);
        }
        if parameter.structural_type != argument.structural_type {
            return Err(TerminalInterpretError::StructuralArgumentType {
                place: parameter.place,
                expected: parameter.structural_type,
                actual: argument.structural_type,
            });
        }
        if parameter
            .qualifications
            .iter()
            .any(|domain| !argument.qualifications.contains(domain))
        {
            return Err(TerminalInterpretError::StructuralQualificationMissing(
                parameter.place,
            ));
        }
        if let Some(previous) =
            bound_identities.insert(argument.opaque_identity, parameter.multiplicity)
            && (previous != StructuralMultiplicity::Unrestricted
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted)
        {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(
                argument.opaque_identity,
            ));
        }
        if values.insert(parameter.place, argument.clone()).is_some() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(values)
}

fn bind_entry_claims(
    entry_claims: &[EntryClaim],
    content_entry_claims: &[psi_terminal::ContentEntryClaim],
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut claims = BTreeMap::new();
    for entry_claim in entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !values.contains_key(&parameter.place)
            || claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        multiplicity: Some(if entry_claim.field_path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    for entry_claim in content_entry_claims {
        claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: None,
            multiplicity: None,
        });
    }
    Ok(claims)
}

fn resolve_structural_arguments(
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[StructuralArgument],
) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
    arguments
        .iter()
        .map(|argument| {
            values.get(&argument.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn transfer_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    _caller_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    caller_arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    callee_parameters: &[StructuralParameterDeclaration],
    callee_entry_claims: &[EntryClaim],
    callee_content_entry_claims: &[psi_terminal::ContentEntryClaim],
    callee_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    if transfers.len() != callee_entry_claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut expected_by_argument = BTreeMap::<u32, Vec<&EntryClaim>>::new();
    for entry_claim in callee_entry_claims {
        let (index, parameter) = callee_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !callee_values.contains_key(&parameter.place)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        expected_by_argument
            .entry(index as u32)
            .or_default()
            .push(entry_claim);
    }
    let mut actual_by_argument = BTreeMap::<u32, Vec<&ClaimTransfer>>::new();
    for transfer in transfers {
        if transfer.argument_index as usize >= caller_arguments.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        actual_by_argument
            .entry(transfer.argument_index)
            .or_default()
            .push(transfer);
    }
    if expected_by_argument.keys().collect::<Vec<_>>()
        != actual_by_argument.keys().collect::<Vec<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let mut remaining = caller_claims.clone();
    let mut callee_claims = BTreeMap::new();
    for (argument_index, expected) in expected_by_argument {
        let actual = &actual_by_argument[&argument_index];
        if actual.len() != expected.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let caller_place = caller_arguments[argument_index as usize].place;
        for (transfer, entry_claim) in actual.iter().zip(expected) {
            let caller_claim = remaining
                .remove(&transfer.claim)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if caller_claim.place != Some(caller_place) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let parameter = callee_parameters
                .get(argument_index as usize)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if callee_claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        multiplicity: Some(if entry_claim.field_path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
    }
    for entry_claim in callee_content_entry_claims {
        callee_claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: None,
            multiplicity: None,
        });
    }
    Ok((remaining, callee_claims))
}

fn validate_boundary_requirements(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[TerminalStructuralValue],
) -> Result<(), TerminalInterpretError> {
    for requirement in &boundary.requires {
        let argument = arguments
            .get(requirement.argument_index as usize)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if !argument.qualifications.contains(&requirement.domain) {
            return Err(TerminalInterpretError::BoundaryQualificationMissing {
                boundary: boundary.id,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn settle_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    settlements: &[ClaimSettlement],
    _boundary_parameters: &[StructuralParameterDeclaration],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let expected = caller_arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller_claims.iter().filter_map(move |(claim, live)| {
                (live.place == Some(argument.place)).then_some((index as u32, *claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let mut remaining = caller_claims.clone();
    let mut actual = BTreeSet::new();
    for settlement in settlements {
        if !actual.insert((settlement.argument_index, settlement.claim))
            || !expected.contains(&(settlement.argument_index, settlement.claim))
        {
            return Err(TerminalInterpretError::ClaimSettlementMismatch);
        }
        let argument = caller_arguments
            .get(settlement.argument_index as usize)
            .ok_or(TerminalInterpretError::ClaimSettlementMismatch)?;
        let claim = remaining
            .remove(&settlement.claim)
            .ok_or(TerminalInterpretError::ClaimSettlementMismatch)?;
        if claim.place != Some(argument.place) {
            return Err(TerminalInterpretError::ClaimSettlementMismatch);
        }
    }
    if actual != expected {
        return Err(TerminalInterpretError::ClaimSettlementMismatch);
    }
    Ok(remaining)
}

fn has_live_linear_claims(claims: &BTreeMap<ClaimId, LiveClaim>) -> bool {
    claims
        .values()
        .any(|claim| claim.multiplicity == Some(StructuralMultiplicity::Linear))
}

fn meter_status(error: FuelMeterError) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
    match error {
        FuelMeterError::Exhausted(exhaustion) => {
            Ok(TerminalExecutionStatus::SponsorExhausted(exhaustion))
        }
        other => Err(TerminalInterpretError::Fuel(other)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionStatus {
    Complete(TerminalExecutionResult),
    SponsorExhausted(FuelExhaustion),
    Crashed(TerminalCrash),
}

/// The explicit terminal-Psi crash outcome reached by an execution.
///
/// `frontier_lower_bound` is the machine-local claim frontier recorded by the
/// artifact. It is not an assertion that no wider runtime state was abandoned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCrash {
    pub edge: psi_core::EdgeId,
    pub cause: CrashCause,
    pub site_guard: Vec<psi_terminal::CrashPredicateTerm>,
    pub frontier_lower_bound: Vec<ClaimId>,
}

/// A successful semantic result paired with deterministic terminal-Psi fuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredTerminalExecution {
    value: TerminalExecutionResult,
    usage: TerminalFuelUsage,
    effects: Vec<TerminalEffect>,
}

impl MeasuredTerminalExecution {
    pub const fn value(&self) -> TerminalExecutionResult {
        self.value
    }

    pub const fn usage(&self) -> &TerminalFuelUsage {
        &self.usage
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    pub fn into_value(self) -> TerminalExecutionResult {
        self.value
    }

    pub fn into_parts(self) -> (TerminalExecutionResult, TerminalFuelUsage) {
        (self.value, self.usage)
    }

    pub fn into_parts_with_effects(
        self,
    ) -> (
        TerminalExecutionResult,
        TerminalFuelUsage,
        Vec<TerminalEffect>,
    ) {
        (self.value, self.usage, self.effects)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
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
    StructuralQualificationMissing(PlaceId),
    StructuralArgumentAliasing(u64),
    BoundaryQualificationMissing {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    ClaimTransferMismatch,
    ClaimSettlementMismatch,
    VerifiedEntryMachineMissing,
    VerifiedCallTargetMissing(MachineId),
    VerifiedBoundaryMachineMissing(BoundaryMachineId),
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
    VerifiedStructuralPlaceMissing(PlaceId),
    VerifiedValueMissing(ValueId),
    EffectRejected {
        operation: OperationId,
        rejection: TerminalEffectRejection,
    },
    Crash(TerminalCrash),
    Fuel(FuelMeterError),
}

#[derive(Debug)]
pub enum TerminalArtifactInterpretError {
    SemanticDecode(psi_terminal_codec::CodecError),
    ProofDecode(psi_terminal_codec::ProofCodecError),
    Verification(psi_terminal_verifier::VerificationError),
    Execution(TerminalInterpretError),
}

impl std::fmt::Display for TerminalArtifactInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactInterpretError {}

impl From<FuelMeterError> for TerminalInterpretError {
    fn from(error: FuelMeterError) -> Self {
        Self::Fuel(error)
    }
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInterpretError {}
