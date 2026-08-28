#![forbid(unsafe_code)]

//! Canonical target-legal operation custody for the clean Terminal lane.
//!
//! This representation is deliberately below instruction selection and above
//! physical-home assignment. It records which bounded target operation shapes
//! have passed target legalization, with complete semantic provenance, proof,
//! and fuel custody. It assigns no virtual or physical register, stack slot,
//! instruction encoding, or frame location.

use omega_calling_conventions::{
    CallPlan, CallbackMaterialization, CallingPolicy, EntryControl, IndirectPointerLocation,
    NativePlace, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use omega_optimization_unit::{
    EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance, ValueDefinitionSite,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{TerminalCompletionClaimSource, TerminalValueBinding};
use omega_terminal_target_operations::{MachineRegister, TerminalPsiProvenance};
use omega_terminal_target_operations::{
    TerminalClaimCompletionOnlyRealization, TerminalProviderExecutionBinding,
};
use psi_core::{
    BlockId, BoundaryMachineId, ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment,
    ContentPlaceVersion, EdgeId, FuelScheduleIdentity, IeeeFloatFormat, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, ServiceId, StructuralPlaceKind, ValueId,
};
use psi_terminal::{
    BindingRelevance, ByteSequenceCarrier, ClaimContentProjection, ClaimTransfer,
    CompletionReceipt, EntryClaim, ProviderCandidateConformance, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalLegalizedOperationPlanIdentity([u8; 32]);

impl TerminalLegalizedOperationPlanIdentity {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Dense function-local identity for a value introduced by target
/// legalization rather than Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalLegalizedTemporaryId(pub u32);

/// Closed semantic theorem that authorizes a non-identity legalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLegalizationTheorem {
    /// For unsigned exact addition with a discharged narrow overflow
    /// obligation, zero-extension commutes with addition.
    UnsignedExactAddCommutesWithWidenV1,
    /// For unsigned exact subtraction with a discharged narrow underflow
    /// obligation, zero-extension commutes with subtraction while preserving
    /// the authored operand order.
    UnsignedExactSubtractCommutesWithWidenV1,
}

/// The closed V4 legality recipe admitted for one function.
///
/// The original recipes are identity legalizations. The widened-u8 recipes
/// are closed non-identity transformations with explicit theorem, temporary,
/// source-operation, proof, and fuel custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLegalizationRecipe {
    ReturnU64ImmediateConditionalV1,
    ReturnU64EntryParameterConditionalV1,
    ReturnU64ExactAddImmediateConditionalV1,
    ReturnU64ExactSubtractImmediateConditionalV1,
    ReturnU64WidenedU8ExactAddImmediateConditionalV1,
    ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
    /// The true leaf materializes `r`, `a`, and `b`, then computes the exact
    /// chain `(r + (r + (a + b)))`; the false leaf returns one immediate.
    ReturnU64ActiveResidentExactAddChainConditionalV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalLegalizedFunction>,
    /// Exact straight-line Unit functions admitted independently from the
    /// scalar conditional recipe inventory. Keeping this roster distinct
    /// prevents a value-less return from acquiring a fabricated scalar leaf.
    pub unit_functions: Vec<TerminalLegalizedUnitFunction>,
    /// Exact structural-call Unit functions. This roster is deliberately
    /// distinct from `unit_functions`: accepting a structural signature in
    /// the value-less baseline would erase its ABI and ownership transfer.
    pub structural_unit_functions: Vec<TerminalLegalizedStructuralUnitFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub entry_block: BlockId,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
}

/// One structural-signature `ReturnUnit`, optionally preceded by one
/// claim-preserving `CallUnit`.
///
/// Semantic declarations remain paired with their target ABI projections so
/// instruction selection can consume only the legalized carrier. This record
/// assigns no virtual/physical homes, stack frame, instruction encoding,
/// symbol, relocation, object span, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedStructuralUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    /// Canonical verifier-owned structural declaration closure required by
    /// the function signature and its call arguments.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact target-native call plan for the function's structural ABI.
    pub call_plan: CallPlan,
    /// Ordered semantic parameters paired one-for-one with their target ABI
    /// shape and placement.
    pub parameters: Vec<TerminalLegalizedCallUnitParameter>,
    /// Complete declared structural-place roster used to replay the parameter
    /// roots named by claims and call arguments.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Ordered whole/projection claims present at function entry.
    pub entry_claims: Vec<EntryClaim>,
    /// Exact normalized published-service ceiling. The ProgramStorage slice
    /// requires this to be empty, but legalization must not erase it.
    pub published_service_ceiling: Vec<ServiceId>,
    pub entry_block: BlockId,
    /// Ordered metadata-only provider settlements. These consume claims and
    /// effects but select no instruction.
    pub boundary_settlements: Vec<TerminalLegalizedBoundarySettlement>,
    /// Optional sole structural call. `None` represents the terminal callee
    /// needed to close an acyclic `[CallUnit, ReturnUnit]` program while still
    /// retaining its nonempty structural ABI.
    pub call: Option<TerminalLegalizedCallUnit>,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
    pub return_effect: EffectLink,
    pub return_ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedBoundarySettlement {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: TerminalProviderExecutionBinding,
    pub realization: TerminalClaimCompletionOnlyRealization,
    pub arguments: Vec<StructuralArgument>,
    pub completion_claim_sources: Vec<TerminalCompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedCallUnitParameter {
    pub semantic: StructuralParameterDeclaration,
    pub target: omega_terminal_target_operations::TerminalTargetStructuralParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedCallUnitArgument {
    pub semantic: StructuralArgument,
    pub target: omega_terminal_target_operations::TerminalTargetStructuralArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedCallUnit {
    pub source: TerminalLegalizedCallUnitSource,
    pub operation: OperationId,
    pub callee: MachineId,
    pub arguments: Vec<TerminalLegalizedCallUnitArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

/// Exact semantic origin of one legalized structural Unit call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLegalizedCallUnitSource {
    AuthoredCallUnit,
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLegalizedCallSourceError {
    ProviderIdentityMismatch,
    ArgumentSignatureMismatch,
    CompletionEvidenceMismatch,
    OwnershipMismatch,
}

impl TerminalLegalizedCallUnit {
    /// Validate only invariants owned by this representation. Upstream source,
    /// target, optimizer, and installation replay remain independently
    /// required before construction.
    pub fn validate_source(&self) -> Result<(), TerminalLegalizedCallSourceError> {
        match &self.source {
            TerminalLegalizedCallUnitSource::AuthoredCallUnit => {
                let claims = self
                    .claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>();
                (self.ownership == [OwnershipEvent::ClaimTransfer(claims)])
                    .then_some(())
                    .ok_or(TerminalLegalizedCallSourceError::OwnershipMismatch)
            }
            TerminalLegalizedCallUnitSource::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            } => {
                if provider.boundary != *boundary || provider.candidate != self.callee {
                    return Err(TerminalLegalizedCallSourceError::ProviderIdentityMismatch);
                }
                if provider.signature.parameters.len() != self.arguments.len()
                    || self
                        .arguments
                        .iter()
                        .zip(&provider.signature.parameters)
                        .any(|(argument, parameter)| {
                            argument.semantic.path.len() != 0
                                || argument.semantic.access != parameter.access
                                || argument.target.access != parameter.access
                                || argument.target.structural_type != parameter.structural_type
                        })
                {
                    return Err(TerminalLegalizedCallSourceError::ArgumentSignatureMismatch);
                }
                let transfers = completion_receipts
                    .iter()
                    .map(|receipt| ClaimTransfer {
                        claim: receipt.claim,
                        argument_index: receipt.argument_index,
                    })
                    .collect::<Vec<_>>();
                let unique_source_claims = completion_claim_sources
                    .iter()
                    .map(|source| source.claim)
                    .collect::<std::collections::BTreeSet<_>>();
                if unique_source_claims.len() != completion_claim_sources.len()
                    || transfers != self.claim_transfers
                    || completion_receipts
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || completion_receipts.iter().any(|receipt| {
                        let Some(argument) = self.arguments.get(receipt.argument_index as usize)
                        else {
                            return true;
                        };
                        let matching = completion_claim_sources
                            .iter()
                            .filter(|source| source.claim == receipt.claim)
                            .collect::<Vec<_>>();
                        let [source] = matching.as_slice() else {
                            return true;
                        };
                        (source.entry.is_none() && source.content.is_none())
                            || source.input() != argument.semantic.place
                            || source
                                .entry
                                .as_ref()
                                .is_some_and(|entry| entry.path != argument.semantic.path)
                    })
                {
                    return Err(TerminalLegalizedCallSourceError::CompletionEvidenceMismatch);
                }
                let completed = completion_receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect::<Vec<_>>();
                (self.ownership == [OwnershipEvent::ClaimCompletion(completed)])
                    .then_some(())
                    .ok_or(TerminalLegalizedCallSourceError::OwnershipMismatch)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: TerminalLegalizationRecipe,
    pub condition_source: ValueId,
    pub condition_parameter_index: usize,
    pub condition_register: MachineRegister,
    pub condition_definition_site: ValueDefinitionSite,
    pub entry_block: BlockId,
    pub true_block: BlockId,
    pub false_block: BlockId,
    pub branch_true_edge: EdgeId,
    pub branch_false_edge: EdgeId,
    pub branch_true_fuel: Vec<FuelSettlement>,
    pub branch_false_fuel: Vec<FuelSettlement>,
    pub branch_true_bindings: Vec<TerminalValueBinding>,
    pub branch_false_bindings: Vec<TerminalValueBinding>,
    pub when_true: TerminalLegalizedLeaf,
    pub when_false: TerminalLegalizedLeaf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedLeaf {
    pub return_edge: EdgeId,
    pub source_value: ValueId,
    pub return_fuel: Vec<FuelSettlement>,
    pub value: TerminalLegalizedLeafValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLegalizedLeafValue {
    Immediate {
        value: IntegerValue,
        constant_operation: OperationId,
        definition_site: ValueDefinitionSite,
        constant_fuel: Vec<FuelSettlement>,
    },
    EntryParameter {
        parameter_index: usize,
        register: MachineRegister,
        definition_site: ValueDefinitionSite,
    },
    ExactAdd {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    ExactSubtract {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    WidenedExactAdd {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: TerminalLegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        narrow_result: ValueId,
        add_definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: TerminalLegalizedTemporaryId,
        right_temporary: TerminalLegalizedTemporaryId,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    WidenedExactSubtract {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: TerminalLegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        narrow_result: ValueId,
        subtract_definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: TerminalLegalizedTemporaryId,
        right_temporary: TerminalLegalizedTemporaryId,
        left: TerminalLegalizedImmediate,
        right: TerminalLegalizedImmediate,
    },
    ActiveResidentExactAddChain(Box<TerminalLegalizedActiveResidentExactAddChain>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedActiveResidentExactAddChain {
    pub resident: TerminalLegalizedImmediate,
    pub left: TerminalLegalizedImmediate,
    pub right: TerminalLegalizedImmediate,
    pub inner: TerminalLegalizedExactAdd,
    pub middle: TerminalLegalizedExactAdd,
    pub result: TerminalLegalizedExactAdd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedExactAdd {
    pub source_value: ValueId,
    pub obligation: ObligationId,
    pub accepted_fact: AcceptedObligationFactIdentity,
    pub operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLegalizedImmediate {
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub constant_operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

pub fn terminal_legalized_operation_plan_identity(
    plan: &TerminalLegalizedOperationPlan,
) -> TerminalLegalizedOperationPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-legalized-operations.v8\0");
    bytes.extend_from_slice(plan.terminal_psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.terminal_psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.push(match function.recipe {
            TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => 0,
            TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => 1,
            TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => 2,
            TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => 3,
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1 => 4,
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => 5,
            TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => 6,
        });
        bytes.extend_from_slice(&function.condition_source.get().to_le_bytes());
        bytes.extend_from_slice(&(function.condition_parameter_index as u64).to_le_bytes());
        encode_register(&mut bytes, function.condition_register);
        encode_definition_site(&mut bytes, function.condition_definition_site);
        bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.true_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.false_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_true_edge.get().to_le_bytes());
        bytes.extend_from_slice(&function.branch_false_edge.get().to_le_bytes());
        encode_fuel(&mut bytes, &function.branch_true_fuel);
        encode_fuel(&mut bytes, &function.branch_false_fuel);
        encode_bindings(&mut bytes, &function.branch_true_bindings);
        encode_bindings(&mut bytes, &function.branch_false_bindings);
        encode_leaf(&mut bytes, &function.when_true);
        encode_leaf(&mut bytes, &function.when_false);
    }
    encode_len(&mut bytes, plan.unit_functions.len());
    for function in &plan.unit_functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
        bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
        encode_fuel(&mut bytes, &function.return_fuel);
    }
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_structural_unit_function(&mut bytes, function);
    }
    TerminalLegalizedOperationPlanIdentity::from_canonical_bytes(&bytes)
}

fn encode_structural_unit_function(
    bytes: &mut Vec<u8>,
    function: &TerminalLegalizedStructuralUnitFunction,
) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(
        bytes,
        function.attachment.map(|attachment| attachment.get()),
    );
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_len(bytes, function.structural_types.len());
    for declaration in &function.structural_types {
        encode_structural_type(bytes, declaration);
    }
    encode_call_plan(bytes, &function.call_plan);
    encode_len(bytes, function.parameters.len());
    for parameter in &function.parameters {
        encode_structural_parameter(bytes, &parameter.semantic);
        encode_target_structural_parameter(bytes, &parameter.target);
    }
    encode_len(bytes, function.structural_places.len());
    for place in &function.structural_places {
        encode_structural_place(bytes, *place);
    }
    encode_len(bytes, function.entry_claims.len());
    for claim in &function.entry_claims {
        encode_entry_claim(bytes, claim);
    }
    encode_ids(
        bytes,
        function
            .published_service_ceiling
            .iter()
            .map(|service| service.get()),
    );
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    encode_len(bytes, function.boundary_settlements.len());
    for settlement in &function.boundary_settlements {
        encode_boundary_settlement(bytes, settlement);
    }
    match &function.call {
        Some(call) => {
            bytes.push(1);
            encode_call_source(bytes, &call.source);
            bytes.extend_from_slice(&call.operation.get().to_le_bytes());
            bytes.extend_from_slice(&call.callee.get().to_le_bytes());
            encode_len(bytes, call.arguments.len());
            for argument in &call.arguments {
                encode_structural_argument(bytes, &argument.semantic);
                encode_target_structural_argument(bytes, &argument.target);
            }
            encode_len(bytes, call.claim_transfers.len());
            for transfer in &call.claim_transfers {
                bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
                bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
            }
            encode_fuel(bytes, &call.fuel);
            encode_effect(bytes, call.effect);
            encode_ownership_roster(bytes, &call.ownership);
        }
        None => bytes.push(0),
    }
    bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
    encode_fuel(bytes, &function.return_fuel);
    encode_effect(bytes, function.return_effect);
    encode_ownership_roster(bytes, &function.return_ownership);
}

fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    settlement: &TerminalLegalizedBoundarySettlement,
) {
    bytes.extend_from_slice(&settlement.operation.get().to_le_bytes());
    bytes.extend_from_slice(&settlement.boundary.get().to_le_bytes());
    let execution = settlement.provider_execution;
    bytes.extend_from_slice(&execution.provider_plan().get().to_le_bytes());
    bytes.extend_from_slice(&execution.provider_execution_identity().to_le_bytes());
    bytes.extend_from_slice(&execution.provider_execution_fingerprint().to_le_bytes());
    bytes.extend_from_slice(&execution.normalized_root_identity().to_le_bytes());
    bytes.extend_from_slice(&execution.boundary_contract_fingerprint().to_le_bytes());
    bytes.push(1);
    encode_len(bytes, settlement.arguments.len());
    for argument in &settlement.arguments {
        encode_structural_argument(bytes, argument);
    }
    encode_len(bytes, settlement.completion_claim_sources.len());
    for source in &settlement.completion_claim_sources {
        encode_completion_claim_source(bytes, source);
    }
    encode_len(bytes, settlement.completion_receipts.len());
    for receipt in &settlement.completion_receipts {
        bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
        bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
    }
    encode_fuel(bytes, &settlement.fuel);
    encode_effect(bytes, settlement.effect);
    encode_ownership_roster(bytes, &settlement.ownership);
}

fn encode_call_source(bytes: &mut Vec<u8>, source: &TerminalLegalizedCallUnitSource) {
    match source {
        TerminalLegalizedCallUnitSource::AuthoredCallUnit => bytes.push(1),
        TerminalLegalizedCallUnitSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            encode_provider_candidate(bytes, provider);
            encode_len(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            encode_len(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
            }
        }
    }
}

fn encode_provider_candidate(bytes: &mut Vec<u8>, provider: &ProviderCandidateConformance) {
    bytes.extend_from_slice(&provider.boundary.get().to_le_bytes());
    encode_string(bytes, &provider.requirement_identity);
    encode_string(bytes, &provider.provider_identity);
    encode_string(bytes, &provider.candidate_identity);
    bytes.extend_from_slice(&provider.candidate.get().to_le_bytes());
    encode_len(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        bytes.extend_from_slice(&parameter.position.to_le_bytes());
        bytes.push(u8::from(parameter.is_self));
        bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_access(bytes, parameter.access);
        encode_ids(
            bytes,
            parameter
                .qualifications
                .iter()
                .map(|qualification| qualification.get()),
        );
    }
    encode_len(bytes, provider.refinement.positional_parameters.len());
    for parameter in &provider.refinement.positional_parameters {
        bytes.extend_from_slice(&parameter.boundary_index.to_le_bytes());
        bytes.extend_from_slice(&parameter.candidate_index.to_le_bytes());
    }
    encode_len(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        bytes.extend_from_slice(&requirement.argument_index.to_le_bytes());
        bytes.extend_from_slice(&requirement.domain.get().to_le_bytes());
    }
    encode_ids(
        bytes,
        provider
            .refinement
            .realized_service_ceiling
            .iter()
            .map(|service| service.get()),
    );
}

fn encode_completion_claim_source(bytes: &mut Vec<u8>, source: &TerminalCompletionClaimSource) {
    bytes.extend_from_slice(&source.claim.get().to_le_bytes());
    match &source.entry {
        Some(entry) => {
            bytes.push(1);
            encode_entry_claim(bytes, entry);
        }
        None => bytes.push(0),
    }
    match &source.content {
        Some(content) => {
            bytes.push(1);
            bytes.extend_from_slice(&content.claim.get().to_le_bytes());
            bytes.push(match content.input.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            bytes.extend_from_slice(&content.input.root.get().to_le_bytes());
            encode_len(bytes, content.input.segments.len());
            for segment in &content.input.segments {
                match segment {
                    ContentPlaceSegment::Field(value) => {
                        bytes.push(1);
                        encode_string(bytes, value);
                    }
                    ContentPlaceSegment::FixedIndex(value) => {
                        bytes.push(2);
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                    ContentPlaceSegment::Case(value) => {
                        bytes.push(3);
                        encode_string(bytes, value);
                    }
                }
            }
            encode_len(bytes, content.projections.len());
            for projection in &content.projections {
                encode_claim_projection(bytes, projection);
            }
        }
        None => bytes.push(0),
    }
}

fn encode_claim_projection(bytes: &mut Vec<u8>, projection: &ClaimContentProjection) {
    bytes.extend_from_slice(&projection.projection.domain.get().to_le_bytes());
    bytes.extend_from_slice(&projection.projection.projection_fingerprint.to_le_bytes());
    encode_content_algebra(bytes, &projection.algebra);
}

fn encode_content_algebra(bytes: &mut Vec<u8>, algebra: &ContentAlgebra) {
    bytes.push(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    encode_string(bytes, &algebra.parameter);
}

fn encode_effect(bytes: &mut Vec<u8>, effect: EffectLink) {
    bytes.extend_from_slice(&effect.input.to_le_bytes());
    bytes.extend_from_slice(&effect.output.to_le_bytes());
}

fn encode_ownership_roster(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::ClaimCompletion(claims) => {
                bytes.push(2);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::Cleanup(actions) => {
                bytes.push(3);
                encode_len(bytes, actions.len());
                for action in actions {
                    encode_cleanup_action(bytes, action);
                }
            }
            OwnershipEvent::StructuralReturn(claims) => {
                bytes.push(4);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::CrashFrontier(claims) => {
                bytes.push(5);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
        }
    }
}

fn encode_cleanup_action(bytes: &mut Vec<u8>, action: &psi_terminal::TerminalAffineCleanupAction) {
    match action {
        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            bytes.extend_from_slice(&discard.place.get().to_le_bytes());
            encode_structural_path(bytes, &discard.path);
            bytes.extend_from_slice(&discard.structural_type.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(3);
            bytes.extend_from_slice(&cleanup.place.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            encode_option_id(bytes, cleanup.cleanup_receiver.map(|place| place.get()));
            encode_ids(
                bytes,
                cleanup
                    .requirement_obligations
                    .iter()
                    .map(|obligation| obligation.get()),
            );
        }
    }
}

fn encode_call_plan(bytes: &mut Vec<u8>, plan: &CallPlan) {
    bytes.push(match plan.policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    });
    encode_len(bytes, plan.parameters.len());
    for placement in &plan.parameters {
        encode_placement(bytes, placement);
    }
    match &plan.result {
        Some(result) => {
            bytes.push(1);
            encode_placement(bytes, result);
        }
        None => bytes.push(0),
    }
    encode_len(bytes, plan.callback_materializations.len());
    for materialization in &plan.callback_materializations {
        encode_callback_materialization(bytes, materialization);
    }
    encode_len(bytes, plan.ordinary_clobbers.as_slice().len());
    for register in plan.ordinary_clobbers.as_slice() {
        encode_register(bytes, *register);
    }
    bytes.extend_from_slice(&plan.stack_alignment.to_le_bytes());
    bytes.extend_from_slice(&plan.shadow_bytes.to_le_bytes());
    match plan.entry_control {
        EntryControl::CallReturn => bytes.push(1),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            bytes.push(2);
            encode_register(bytes, number_register);
            bytes.extend_from_slice(&immediate.to_le_bytes());
        }
        EntryControl::InterruptReturn => bytes.push(3),
    }
}

fn encode_callback_materialization(bytes: &mut Vec<u8>, row: &CallbackMaterialization) {
    bytes.extend_from_slice(&row.binder.get().to_le_bytes());
    match &row.destination {
        NativePlace::Parameter(parameter) => {
            bytes.push(1);
            bytes.extend_from_slice(&parameter.get().to_le_bytes());
        }
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&parameter.get().to_le_bytes());
            bytes.extend_from_slice(&layout.get().to_le_bytes());
            encode_len(bytes, field_path.len());
            for slot in field_path {
                bytes.extend_from_slice(&slot.get().to_le_bytes());
            }
        }
    }
}

fn encode_placement(bytes: &mut Vec<u8>, placement: &ValuePlacement) {
    encode_shape(bytes, placement.shape);
    encode_len(bytes, placement.locations.len());
    for location in &placement.locations {
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                encode_register(bytes, register);
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                encode_indirect_pointer(bytes, pointer);
                match copy_stack_byte_offset {
                    Some(offset) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&offset.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
        }
    }
}

fn encode_indirect_pointer(bytes: &mut Vec<u8>, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            bytes.push(1);
            encode_register(bytes, register);
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
            bytes.extend_from_slice(&alignment.to_le_bytes());
        }
    }
}

fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => bytes.push(1),
        ValueClass::Float => bytes.push(2),
        ValueClass::HomogeneousFloatAggregate { members } => {
            bytes.push(3);
            bytes.push(members);
        }
        ValueClass::SystemVAggregate { first, second } => {
            bytes.push(4);
            encode_eightbyte_class(bytes, first);
            encode_eightbyte_class(bytes, second);
        }
    }
    bytes.extend_from_slice(&shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&shape.alignment.to_le_bytes());
}

fn encode_eightbyte_class(bytes: &mut Vec<u8>, class: SystemVEightbyteClass) {
    bytes.push(match class {
        SystemVEightbyteClass::Integer => 1,
        SystemVEightbyteClass::Sse => 2,
    });
}

fn encode_structural_parameter(bytes: &mut Vec<u8>, parameter: &StructuralParameterDeclaration) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.position.to_le_bytes());
    bytes.push(u8::from(parameter.is_self));
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(
        bytes,
        parameter
            .qualifications
            .iter()
            .map(|qualification| qualification.get()),
    );
}

fn encode_target_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &omega_terminal_target_operations::TerminalTargetStructuralParameter,
) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_shape(bytes, parameter.shape);
    encode_placement(bytes, &parameter.placement);
}

fn encode_structural_argument(bytes: &mut Vec<u8>, argument: &StructuralArgument) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_structural_path(bytes, &argument.path);
    encode_access(bytes, argument.access);
}

fn encode_target_structural_argument(
    bytes: &mut Vec<u8>,
    argument: &omega_terminal_target_operations::TerminalTargetStructuralArgument,
) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_access(bytes, argument.access);
    encode_structural_path(bytes, &argument.path);
    bytes.extend_from_slice(&argument.root_structural_type.get().to_le_bytes());
    bytes.extend_from_slice(&argument.structural_type.get().to_le_bytes());
    encode_shape(bytes, argument.shape);
    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
    encode_option_u64(bytes, argument.fixed_array_length);
    encode_option_u32(bytes, argument.element_stride);
    encode_placement(bytes, &argument.source);
    encode_placement(bytes, &argument.destination);
}

fn encode_entry_claim(bytes: &mut Vec<u8>, claim: &EntryClaim) {
    bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
    bytes.extend_from_slice(&claim.input.get().to_le_bytes());
    encode_structural_path(bytes, &claim.path);
}

fn encode_structural_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                bytes.push(1);
                encode_string(bytes, identity);
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}

fn encode_structural_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
    bytes.extend_from_slice(&place.id.get().to_le_bytes());
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
            bytes.push(u8::from(is_self));
        }
        StructuralPlaceKind::Result => bytes.push(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&producer.get().to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&attachment.get().to_le_bytes());
            bytes.extend_from_slice(&field.get().to_le_bytes());
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(6);
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
    }
}

fn encode_structural_type(bytes: &mut Vec<u8>, declaration: &StructuralTypeDeclaration) {
    bytes.extend_from_slice(&declaration.id.get().to_le_bytes());
    encode_string(bytes, &declaration.identity);
    match &declaration.shape {
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::Record { fields } => {
            bytes.push(2);
            encode_structural_fields(bytes, fields);
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            bytes.extend_from_slice(&element.get().to_le_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.push(4);
            encode_len(bytes, cases.len());
            for case in cases {
                bytes.extend_from_slice(&case.id.get().to_le_bytes());
                encode_string(bytes, &case.identity);
                encode_structural_fields(bytes, &case.fields);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.push(5);
            encode_structural_fields(bytes, fields);
            encode_len(bytes, cases.len());
            for case in cases {
                bytes.extend_from_slice(&case.id.get().to_le_bytes());
                encode_string(bytes, &case.identity);
                encode_structural_fields(bytes, &case.fields);
            }
        }
    }
}

fn encode_structural_fields(bytes: &mut Vec<u8>, fields: &[StructuralFieldDeclaration]) {
    encode_len(bytes, fields.len());
    for field in fields {
        bytes.extend_from_slice(&field.id.get().to_le_bytes());
        encode_string(bytes, &field.identity);
        bytes.push(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        match &field.field_type {
            StructuralFieldType::Scalar(scalar) => {
                bytes.push(1);
                encode_scalar_type(bytes, *scalar);
            }
            StructuralFieldType::IeeeFloat(format) => {
                bytes.push(2);
                bytes.push(match format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            StructuralFieldType::ByteSequence(carrier) => {
                bytes.push(3);
                encode_byte_sequence_carrier(bytes, *carrier);
            }
            StructuralFieldType::Structural(structural_type) => {
                bytes.push(4);
                bytes.extend_from_slice(&structural_type.get().to_le_bytes());
            }
            StructuralFieldType::Erased { type_identity } => {
                bytes.push(5);
                encode_string(bytes, type_identity);
            }
        }
    }
}

fn encode_byte_sequence_carrier(bytes: &mut Vec<u8>, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => bytes.push(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.push(2);
            bytes.extend_from_slice(&capacity.to_le_bytes());
        }
    }
}

fn encode_multiplicity(bytes: &mut Vec<u8>, multiplicity: StructuralMultiplicity) {
    bytes.push(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

fn encode_access(bytes: &mut Vec<u8>, access: StructuralAccess) {
    bytes.push(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_option_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_string(bytes: &mut Vec<u8>, value: &str) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn encode_leaf(bytes: &mut Vec<u8>, leaf: &TerminalLegalizedLeaf) {
    bytes.extend_from_slice(&leaf.return_edge.get().to_le_bytes());
    bytes.extend_from_slice(&leaf.source_value.get().to_le_bytes());
    encode_fuel(bytes, &leaf.return_fuel);
    match &leaf.value {
        TerminalLegalizedLeafValue::Immediate {
            value,
            constant_operation,
            definition_site,
            constant_fuel,
        } => {
            bytes.push(0);
            encode_integer(bytes, *value);
            bytes.extend_from_slice(&constant_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, constant_fuel);
        }
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index,
            register,
            definition_site,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&(*parameter_index as u64).to_le_bytes());
            encode_register(bytes, *register);
            encode_definition_site(bytes, *definition_site);
        }
        TerminalLegalizedLeafValue::ExactAdd {
            obligation,
            accepted_fact,
            add_operation,
            definition_site,
            add_fuel,
            left,
            right,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&add_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, add_fuel);
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        TerminalLegalizedLeafValue::ExactSubtract {
            obligation,
            accepted_fact,
            subtract_operation,
            definition_site,
            subtract_fuel,
            left,
            right,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&subtract_operation.get().to_le_bytes());
            encode_definition_site(bytes, *definition_site);
            encode_fuel(bytes, subtract_fuel);
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        TerminalLegalizedLeafValue::WidenedExactAdd {
            source_type,
            target_type,
            theorem,
            obligation,
            accepted_fact,
            add_operation,
            narrow_result,
            add_definition_site,
            add_fuel,
            widen_operation,
            widen_definition_site,
            widen_fuel,
            left_temporary,
            right_temporary,
            left,
            right,
        } => {
            bytes.push(4);
            encode_integer_type(bytes, *source_type);
            encode_integer_type(bytes, *target_type);
            bytes.push(match theorem {
                TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
            });
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&add_operation.get().to_le_bytes());
            bytes.extend_from_slice(&narrow_result.get().to_le_bytes());
            encode_definition_site(bytes, *add_definition_site);
            encode_fuel(bytes, add_fuel);
            bytes.extend_from_slice(&widen_operation.get().to_le_bytes());
            encode_definition_site(bytes, *widen_definition_site);
            encode_fuel(bytes, widen_fuel);
            bytes.extend_from_slice(&left_temporary.0.to_le_bytes());
            bytes.extend_from_slice(&right_temporary.0.to_le_bytes());
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        TerminalLegalizedLeafValue::WidenedExactSubtract {
            source_type,
            target_type,
            theorem,
            obligation,
            accepted_fact,
            subtract_operation,
            narrow_result,
            subtract_definition_site,
            subtract_fuel,
            widen_operation,
            widen_definition_site,
            widen_fuel,
            left_temporary,
            right_temporary,
            left,
            right,
        } => {
            bytes.push(5);
            encode_integer_type(bytes, *source_type);
            encode_integer_type(bytes, *target_type);
            bytes.push(match theorem {
                TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1 => 0,
                TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1 => 1,
            });
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
            bytes.extend_from_slice(&subtract_operation.get().to_le_bytes());
            bytes.extend_from_slice(&narrow_result.get().to_le_bytes());
            encode_definition_site(bytes, *subtract_definition_site);
            encode_fuel(bytes, subtract_fuel);
            bytes.extend_from_slice(&widen_operation.get().to_le_bytes());
            encode_definition_site(bytes, *widen_definition_site);
            encode_fuel(bytes, widen_fuel);
            bytes.extend_from_slice(&left_temporary.0.to_le_bytes());
            bytes.extend_from_slice(&right_temporary.0.to_le_bytes());
            encode_immediate(bytes, left);
            encode_immediate(bytes, right);
        }
        TerminalLegalizedLeafValue::ActiveResidentExactAddChain(chain) => {
            bytes.push(6);
            encode_immediate(bytes, &chain.resident);
            encode_immediate(bytes, &chain.left);
            encode_immediate(bytes, &chain.right);
            encode_exact_add(bytes, &chain.inner);
            encode_exact_add(bytes, &chain.middle);
            encode_exact_add(bytes, &chain.result);
        }
    }
}

fn encode_exact_add(bytes: &mut Vec<u8>, add: &TerminalLegalizedExactAdd) {
    bytes.extend_from_slice(&add.source_value.get().to_le_bytes());
    bytes.extend_from_slice(&add.obligation.get().to_le_bytes());
    bytes.extend_from_slice(&add.accepted_fact.bytes());
    bytes.extend_from_slice(&add.operation.get().to_le_bytes());
    encode_definition_site(bytes, add.definition_site);
    encode_fuel(bytes, &add.fuel);
}

fn encode_immediate(bytes: &mut Vec<u8>, immediate: &TerminalLegalizedImmediate) {
    bytes.extend_from_slice(&immediate.source_value.get().to_le_bytes());
    encode_integer(bytes, immediate.value);
    bytes.extend_from_slice(&immediate.constant_operation.get().to_le_bytes());
    encode_definition_site(bytes, immediate.definition_site);
    encode_fuel(bytes, &immediate.fuel);
}

fn encode_integer(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
}

fn encode_bindings(bytes: &mut Vec<u8>, bindings: &[TerminalValueBinding]) {
    encode_len(bytes, bindings.len());
    for binding in bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: psi_core::ScalarType) {
    match scalar_type {
        psi_core::ScalarType::Boolean => bytes.push(0),
        psi_core::ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                psi_core::IntegerCarrier::Fixed => 0,
                psi_core::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                psi_core::IntegerSign::Signed => 0,
                psi_core::IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
    }
}

fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        psi_core::IntegerCarrier::Fixed => 0,
        psi_core::IntegerCarrier::Address => 1,
    });
    bytes.push(match integer.sign() {
        psi_core::IntegerSign::Signed => 0,
        psi_core::IntegerSign::Unsigned => 1,
    });
    bytes.extend_from_slice(&integer.bits().to_le_bytes());
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    encode_len(bytes, fuel.len());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::X86_64 => 0,
        omega_target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_option_id(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = u64>) {
    let values = values.into_iter().collect::<Vec<_>>();
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use omega_target::NativeTarget;
    use psi_core::{ClaimId, IntegerSign, StructuralDomainId, StructuralFieldId, StructuralTypeId};
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    fn id<T>(raw: u64) -> T
    where
        T: psi_core::PsiSemanticId,
    {
        T::new(raw).expect("nonzero test identity")
    }

    fn call_aware_plan() -> TerminalLegalizedOperationPlan {
        let extent_type = id::<StructuralTypeId>(1);
        let granted = id::<StructuralDomainId>(1);
        let image_place = id(1);
        let storage_place = id(2);
        let shape = ValueShape::integer(16, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![shape, shape],
                result: None,
            },
        )
        .expect("two-Extent Microsoft x64 call plan");
        let parameters = [image_place, storage_place]
            .into_iter()
            .enumerate()
            .map(|(position, place)| TerminalLegalizedCallUnitParameter {
                semantic: StructuralParameterDeclaration {
                    place,
                    position: position as u32,
                    is_self: false,
                    structural_type: extent_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    access: StructuralAccess::Owned,
                    qualifications: vec![granted],
                },
                target: omega_terminal_target_operations::TerminalTargetStructuralParameter {
                    place,
                    structural_type: extent_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    access: StructuralAccess::Owned,
                    shape,
                    placement: call_plan.parameters[position].clone(),
                },
            })
            .collect::<Vec<_>>();
        let arguments = parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| TerminalLegalizedCallUnitArgument {
                semantic: StructuralArgument {
                    place: parameter.semantic.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                target: omega_terminal_target_operations::TerminalTargetStructuralArgument {
                    place: parameter.semantic.place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                    root_structural_type: extent_type,
                    structural_type: extent_type,
                    shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: call_plan.parameters[position].clone(),
                    destination: call_plan.parameters[position].clone(),
                },
            })
            .collect::<Vec<_>>();
        let call = id::<OperationId>(1);
        let return_edge = id::<EdgeId>(1);
        TerminalLegalizedOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            fuel_schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
            target: NativeTarget::from_omega_target_name(Some("uefi_x64")).expect("UEFI target"),
            entry: id(1),
            functions: Vec::new(),
            unit_functions: Vec::new(),
            structural_unit_functions: vec![TerminalLegalizedStructuralUnitFunction {
                machine: id(1),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![call],
                    edges: vec![return_edge],
                },
                structural_types: vec![StructuralTypeDeclaration {
                    id: extent_type,
                    identity: "omega::core::Extent".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            StructuralFieldDeclaration {
                                id: id::<StructuralFieldId>(1),
                                identity: "base".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(
                                    psi_core::ScalarType::Integer(
                                        IntegerType::address(64).expect("addr"),
                                    ),
                                ),
                            },
                            StructuralFieldDeclaration {
                                id: id::<StructuralFieldId>(2),
                                identity: "length".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(
                                    psi_core::ScalarType::Integer(
                                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                                    ),
                                ),
                            },
                        ],
                    },
                }],
                call_plan,
                parameters,
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: image_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: false,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: storage_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 1,
                            is_self: false,
                        },
                    },
                ],
                entry_claims: vec![
                    EntryClaim {
                        claim: id(1),
                        input: image_place,
                        path: Vec::new(),
                    },
                    EntryClaim {
                        claim: id(2),
                        input: storage_place,
                        path: Vec::new(),
                    },
                ],
                published_service_ceiling: Vec::new(),
                entry_block: id(1),
                boundary_settlements: Vec::new(),
                call: Some(TerminalLegalizedCallUnit {
                    source: TerminalLegalizedCallUnitSource::AuthoredCallUnit,
                    operation: call,
                    callee: id(2),
                    arguments,
                    claim_transfers: vec![
                        ClaimTransfer {
                            claim: id(1),
                            argument_index: 0,
                        },
                        ClaimTransfer {
                            claim: id(2),
                            argument_index: 1,
                        },
                    ],
                    fuel: vec![FuelSettlement {
                        site: PsiProvenance::Operation(call),
                        units: 2,
                    }],
                    effect: EffectLink {
                        input: 0,
                        output: 1,
                    },
                    ownership: vec![OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])],
                }),
                return_edge,
                return_fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(return_edge),
                    units: 1,
                }],
                return_effect: EffectLink {
                    input: 1,
                    output: 2,
                },
                return_ownership: vec![OwnershipEvent::Cleanup(Vec::new())],
            }],
        }
    }

    fn installed_provider_plan() -> TerminalLegalizedOperationPlan {
        let mut plan = call_aware_plan();
        let function = &mut plan.structural_unit_functions[0];
        let boundary = id::<BoundaryMachineId>(1);
        let provider = ProviderCandidateConformance {
            boundary,
            requirement_identity: "ProgramEntry::enter".into(),
            provider_identity: "UefiProgramProvider".into(),
            candidate_identity: "UefiProgramProvider::enter".into(),
            candidate: id(2),
            signature: psi_terminal::ProviderUnitSignature {
                parameters: function
                    .parameters
                    .iter()
                    .map(|parameter| psi_terminal::ProviderSignatureParameter {
                        position: parameter.semantic.position,
                        is_self: parameter.semantic.is_self,
                        structural_type: parameter.semantic.structural_type,
                        multiplicity: parameter.semantic.multiplicity,
                        access: parameter.semantic.access,
                        qualifications: parameter.semantic.qualifications.clone(),
                    })
                    .collect(),
            },
            refinement: psi_terminal::ProviderUnitRefinement {
                positional_parameters: vec![
                    psi_terminal::ProviderParameterRefinement {
                        boundary_index: 0,
                        candidate_index: 0,
                    },
                    psi_terminal::ProviderParameterRefinement {
                        boundary_index: 1,
                        candidate_index: 1,
                    },
                ],
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        };
        let completion_claim_sources = function
            .entry_claims
            .iter()
            .cloned()
            .map(|entry| TerminalCompletionClaimSource {
                claim: entry.claim,
                entry: Some(entry),
                content: None,
            })
            .collect::<Vec<_>>();
        let completion_receipts = function
            .entry_claims
            .iter()
            .enumerate()
            .map(|(argument_index, claim)| CompletionReceipt {
                claim: claim.claim,
                argument_index: argument_index as u32,
            })
            .collect::<Vec<_>>();
        let call = function.call.as_mut().expect("structural call");
        call.source = TerminalLegalizedCallUnitSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        };
        call.ownership = vec![OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])];
        plan
    }

    #[test]
    fn structural_call_source_validation_preserves_installed_completion_custody() {
        let authored = call_aware_plan();
        authored.structural_unit_functions[0]
            .call
            .as_ref()
            .expect("authored call")
            .validate_source()
            .expect("authored call source");

        let installed = installed_provider_plan();
        let call = installed.structural_unit_functions[0]
            .call
            .as_ref()
            .expect("installed call");
        call.validate_source().expect("installed provider source");
        assert_ne!(
            terminal_legalized_operation_plan_identity(&authored),
            terminal_legalized_operation_plan_identity(&installed)
        );

        let mut with_unrelated_source = installed.clone();
        let call = with_unrelated_source.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("installed call");
        let TerminalLegalizedCallUnitSource::InstalledProvider {
            completion_claim_sources,
            ..
        } = &mut call.source
        else {
            panic!("installed provider source");
        };
        completion_claim_sources.push(TerminalCompletionClaimSource {
            claim: id(3),
            entry: Some(EntryClaim {
                claim: id(3),
                input: id(1),
                path: Vec::new(),
            }),
            content: None,
        });
        call.validate_source()
            .expect("unrelated retained caller source is permitted");
        assert_ne!(
            terminal_legalized_operation_plan_identity(&installed),
            terminal_legalized_operation_plan_identity(&with_unrelated_source)
        );

        let mut duplicate_source = installed.clone();
        let call = duplicate_source.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("installed call");
        let TerminalLegalizedCallUnitSource::InstalledProvider {
            completion_claim_sources,
            ..
        } = &mut call.source
        else {
            panic!("installed provider source");
        };
        completion_claim_sources.push(completion_claim_sources[0].clone());
        assert_eq!(
            call.validate_source(),
            Err(TerminalLegalizedCallSourceError::CompletionEvidenceMismatch)
        );

        let mut wrong_ownership = installed.clone();
        let call = wrong_ownership.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("installed call");
        call.ownership = vec![OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])];
        assert_eq!(
            call.validate_source(),
            Err(TerminalLegalizedCallSourceError::OwnershipMismatch)
        );

        let mut wrong_provider = installed;
        let call = wrong_provider.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("installed call");
        let TerminalLegalizedCallUnitSource::InstalledProvider { provider, .. } = &mut call.source
        else {
            panic!("installed provider source");
        };
        provider.candidate = id(3);
        assert_eq!(
            call.validate_source(),
            Err(TerminalLegalizedCallSourceError::ProviderIdentityMismatch)
        );
    }

    fn assert_identity_drift(
        original: TerminalLegalizedOperationPlanIdentity,
        corrupted: &TerminalLegalizedOperationPlan,
    ) {
        assert_ne!(
            terminal_legalized_operation_plan_identity(corrupted),
            original
        );
    }

    #[test]
    fn call_aware_unit_identity_binds_semantic_and_target_custody() {
        let plan = call_aware_plan();
        let identity = terminal_legalized_operation_plan_identity(&plan);
        assert_eq!(identity, terminal_legalized_operation_plan_identity(&plan));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].structural_types[0]
            .identity
            .push_str("::drift");
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call_plan
            .shadow_bytes += 8;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].parameters.swap(0, 1);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].parameters[0]
            .semantic
            .qualifications
            .clear();
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].parameters[0]
            .target
            .placement
            .locations
            .clear();
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .structural_places
            .swap(0, 1);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].entry_claims[0].claim = id::<ClaimId>(3);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .published_service_ceiling
            .push(id(1));
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .callee = id(3);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .arguments
            .swap(0, 1);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .arguments[0]
            .semantic
            .path
            .push(StructuralPathSegment::Field("base".into()));
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .arguments[0]
            .target
            .source_byte_offset = 8;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .claim_transfers
            .swap(0, 1);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .fuel[0]
            .units += 1;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .effect
            .output += 1;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        let OwnershipEvent::ClaimTransfer(claims) = &mut corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .expect("call")
            .ownership[0]
        else {
            panic!("call claim-transfer ownership");
        };
        claims.swap(0, 1);
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].call = None;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].return_fuel[0].units += 1;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].return_effect.input += 1;
        assert_identity_drift(identity, &corrupted);

        let mut corrupted = plan.clone();
        let OwnershipEvent::Cleanup(actions) =
            &mut corrupted.structural_unit_functions[0].return_ownership[0]
        else {
            panic!("return cleanup ownership");
        };
        actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(id(
            1,
        )));
        assert_identity_drift(identity, &corrupted);
    }

    #[test]
    fn call_aware_unit_roster_cannot_alias_value_less_unit_roster() {
        let call_aware = call_aware_plan();
        let call_aware_identity = terminal_legalized_operation_plan_identity(&call_aware);
        let function = &call_aware.structural_unit_functions[0];
        let mut erased = call_aware.clone();
        erased.structural_unit_functions.clear();
        erased.unit_functions.push(TerminalLegalizedUnitFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: function.provenance.clone(),
            entry_block: function.entry_block,
            return_edge: function.return_edge,
            return_fuel: function.return_fuel.clone(),
        });
        assert_ne!(
            terminal_legalized_operation_plan_identity(&erased),
            call_aware_identity
        );
    }
}
