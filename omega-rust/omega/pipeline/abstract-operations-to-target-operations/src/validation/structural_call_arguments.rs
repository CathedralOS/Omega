//! Reconcile retained embedded calls against source operations and callee identity.
//!
//! Every retained row in the in-module and installed-provider call families is
//! collected and replayed against the callee's independently reconstructed
//! signature: the embedded plan, scalar and structural argument rows,
//! descriptor and dispatch custody, result identity, and result home are all
//! re-derived from the abstract declarations. The retained target plan, its
//! self-reported placements, and any producer-side digest are never admission
//! authority. A retained row bound to no source call operation, or a second
//! row under an operation key that already has one, is forged by construction.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractBoundaryResult, AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter,
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch, AbstractResult,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch, CompletionClaimSource,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, EntryControl, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, CompilerBuiltinExecution, NativeCallOrigin,
    NormalizedForeignCallBinding, ProviderExecutionBinding, ScalarAbiValue, ScalarFunctionAbi,
    TargetBoundaryResult, TargetControlTerminator, TargetDynamicDescriptorArgument,
    TargetDynamicDescriptorInstanceSource, TargetDynamicDescriptorParameterAbi, TargetFunction,
    TargetNativeCallbackArgument, TargetReferenceResult, TargetStructuralArgument,
    TargetStructuralArgumentSource, TargetStructuralHomeRequirement, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, ClosedConformanceCallableResult, CrashRouteBucket,
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalDynamicRequirement,
};

use super::reference_results;
use super::structural_shapes;
use super::structural_signatures;

/// The result row one retained `Call`-family operation carries.
enum EmbeddedResult<'a> {
    /// The callee returns Unit and no result row exists.
    Unit,
    /// An ordinary scalar call retains its exact semantic result and home.
    Scalar(&'a TargetUnitScalarHomeRequirement),
    /// structural-result `Call`: the call's result row, the retained declared
    /// callee result, the required durable home, and the retained reference
    /// leaf roster.
    Structural {
        result: &'a StructuralOperationResult,
        callee_result: &'a StructuralResultDeclaration,
        result_home: Option<&'a TargetStructuralHomeRequirement>,
        reference_results: &'a [TargetReferenceResult],
    },
}

/// One retained call row keyed by its source operation identity. Dispatch kind
/// selects the custody to check; result and argument shapes remain data. Source
/// operations retain their own semantic admission checks after collection.
enum EmbeddedCall<'a> {
    /// Ordinary calls share a scalar/structural argument roster and origin.
    Direct {
        origin: &'a NativeCallOrigin,
        callee: MachineId,
        call_plan: &'a CallPlan,
        scalar_arguments: &'a [TargetUnitScalarCallArgument],
        arguments: &'a [TargetStructuralArgument],
        result: EmbeddedResult<'a>,
        claim_transfers: &'a [ClaimTransfer],
        returned_claim_transfers: &'a [StructuralResultClaimTransfer],
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
    },
    /// `StructuralScalarCallWithDynamicArguments` and
    /// `StructuralUnitCallWithDynamicArguments`: ordered `{data, table}`
    /// descriptor pairs instead of scalar or structural rows.
    DynamicArguments {
        callee: MachineId,
        call_plan: &'a CallPlan,
        result: Option<(&'a AbstractResult, &'a TargetUnitScalarHomeRequirement)>,
        structural_arguments: &'a [TargetStructuralArgument],
        dynamic_arguments: &'a [TargetDynamicDescriptorArgument],
        claim_transfers: &'a [ClaimTransfer],
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
    },
    /// `StoredDynamicScalarCall`: reload and invoke one stored descriptor.
    StoredDynamic {
        dynamic_dispatch: &'a AbstractStoredDynamicDispatch,
        call_plan: &'a CallPlan,
        result: &'a AbstractResult,
        result_home: &'a TargetUnitScalarHomeRequirement,
        source_argument: &'a TargetStructuralArgument,
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
    },
    /// `DynamicParameterScalarCall` and `DynamicParameterUnitCall`: indirect
    /// requirement invocations through one of the function's own descriptor
    /// parameters — the descriptor ABI, requirement row, erased dispatch plan,
    /// and slot offset all replay against the declared interface.
    ParameterDynamic {
        dynamic_dispatch: &'a AbstractParameterDynamicDispatch,
        parameter_abi: &'a TargetDynamicDescriptorParameterAbi,
        requirement: &'a TerminalDynamicRequirement,
        dispatch_call_plan: &'a CallPlan,
        table_slot_byte_offset: u32,
        result: Option<(&'a AbstractResult, &'a TargetUnitScalarHomeRequirement)>,
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
    },
    /// `DynamicScalarCall` and `DynamicUnitCall`: rebound descriptor calls
    /// through a private table.
    ReboundDynamic {
        dynamic_dispatch: &'a AbstractReboundDynamicDispatch,
        call_plan: &'a CallPlan,
        result: Option<(&'a AbstractResult, &'a TargetUnitScalarHomeRequirement)>,
        initial_argument: &'a TargetStructuralArgument,
        rebound_argument: &'a TargetStructuralArgument,
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
    },
    /// `StoreDynamicDescriptor`: not a call, but its retained argument row
    /// embeds the unique stored call's plan placement and must replay it.
    StoredDescriptor {
        stored: &'a AbstractStoredDynamicDescriptor,
        source_argument: &'a TargetStructuralArgument,
    },
    /// `NormalizedForeignCall`: an evaluated foreign-boundary call retains
    /// its embedded boundary entry plan, admitted provider custody, and
    /// projected borrowed argument rows beside the declaration identity.
    NormalizedForeign {
        boundary: BoundaryMachineId,
        provider_execution: &'a ProviderExecutionBinding,
        binding: &'a NormalizedForeignCallBinding,
        scalar_arguments: &'a [TargetUnitScalarCallArgument],
        structural_arguments: &'a [TargetStructuralArgument],
        result_home: &'a Option<TargetUnitScalarHomeRequirement>,
    },
    /// `BoundarySettlement`: a compiler-builtin settlement retains its closed
    /// realization, admitted execution custody, verbatim semantic rosters,
    /// and the runtime scalar lane its hosted realization consumes.
    /// `nonreturning_tail` records whether the row is its block's last
    /// operation followed by a plain `Return` — the shape the producer's
    /// exit settlement requires.
    Settlement {
        boundary: BoundaryMachineId,
        result: &'a TargetBoundaryResult,
        execution: &'a BoundaryExecutionBinding,
        realization: &'a BoundaryRealization,
        scalar_arguments: &'a [BoundaryScalarArgument],
        runtime_scalar_arguments: &'a [TargetUnitScalarCallArgument],
        arguments: &'a [StructuralArgument],
        byte_sequence_arguments: &'a [BoundaryByteSequenceArgument],
        completion_claim_sources: &'a [CompletionClaimSource],
        completion_receipts: &'a [terminal_psi::CompletionReceipt],
        nonreturning_tail: bool,
    },
}

/// The result contract one source operation binds the embedded call to.
enum BoundResult<'a> {
    Unit,
    Scalar(&'a AbstractResult),
    Structural(&'a StructuralOperationResult),
}

/// Everything one replay needs that stays constant across the plan.
struct Replay<'a> {
    source: &'a AbstractFunction,
    source_functions: &'a [AbstractFunction],
    target: &'a TargetFunction,
    target_functions: &'a [TargetFunction],
    declarations: &'a [StructuralTypeDeclaration],
    boundary_machines: &'a [BoundaryMachineDeclaration],
    native_target: NativeTarget,
    /// The plan's retained native-callback roster: the sole custody carrier
    /// of binder and demand context for a registrar's materialized private
    /// parameter slot.
    native_callbacks: &'a [TargetNativeCallbackArgument],
    roots: &'a BTreeMap<PlaceId, RootDeclaration>,
    /// The reference leaf roster each structural-result call establishes,
    /// independently recomputed from the caller's own custody stream by
    /// `reference_results::expected`. A source whose custody cannot be
    /// replayed yields an empty map, so every retained structural-result `Call`
    /// row under it rejects.
    expected_reference_results: BTreeMap<OperationId, Vec<TargetReferenceResult>>,
}

/// The semantic referent declaration bound to one argument place: the root
/// structural type and the access that declaration grants. Operation
/// establishments own their storage; caller and block parameters keep their
/// declared access.
struct RootDeclaration {
    structural_type: StructuralTypeId,
    access: StructuralAccess,
}

pub(super) fn validate(
    source: &AbstractFunction,
    source_functions: &[AbstractFunction],
    target: &TargetFunction,
    target_functions: &[TargetFunction],
    declarations: &[StructuralTypeDeclaration],
    boundary_machines: &[BoundaryMachineDeclaration],
    native_target: NativeTarget,
    native_callbacks: &[TargetNativeCallbackArgument],
) -> Result<(), OperationId> {
    // One source operation yields at most one retained call row; a second row
    // under the same operation key can only shadow the honest replay.
    let mut target_calls = BTreeMap::new();
    for call in target
        .graph
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .map(move |operation| (block, operation))
        })
        .filter_map(|(block, operation)| {
            let call = match operation {
                TargetUnitOperation::Call {
                    origin,
                    psi_operation,
                    result,
                    callee,
                    call_plan,
                    scalar_arguments,
                    arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    let (result, returned_claim_transfers) = match result {
                        target_operations::TargetCallResult::Unit => {
                            (EmbeddedResult::Unit, &[][..])
                        }
                        target_operations::TargetCallResult::Scalar(home) => {
                            (EmbeddedResult::Scalar(home), &[][..])
                        }
                        target_operations::TargetCallResult::Structural {
                            result,
                            callee_result,
                            result_home,
                            reference_results,
                            returned_claim_transfers,
                        } => (
                            EmbeddedResult::Structural {
                                result,
                                callee_result,
                                result_home: result_home.as_ref(),
                                reference_results,
                            },
                            returned_claim_transfers.as_slice(),
                        ),
                    };
                    (
                        *psi_operation,
                        EmbeddedCall::Direct {
                            origin,
                            callee: *callee,
                            call_plan,
                            scalar_arguments,
                            arguments,
                            result,
                            claim_transfers,
                            returned_claim_transfers,
                            requirement_obligations,
                            crash_continuations,
                        },
                    )
                }
                TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                    psi_operation,
                    result,
                    callee,
                    call_plan,
                    result_home,
                    structural_arguments,
                    dynamic_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::DynamicArguments {
                        callee: *callee,
                        call_plan,
                        result: Some((result, result_home)),
                        structural_arguments,
                        dynamic_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
                    psi_operation,
                    callee,
                    call_plan,
                    structural_arguments,
                    dynamic_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::DynamicArguments {
                        callee: *callee,
                        call_plan,
                        result: None,
                        structural_arguments,
                        dynamic_arguments,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::StoredDynamicScalarCall {
                    psi_operation,
                    result,
                    dynamic_dispatch,
                    call_plan,
                    result_home,
                    source_argument,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::StoredDynamic {
                        dynamic_dispatch,
                        call_plan,
                        result,
                        result_home,
                        source_argument,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::DynamicScalarCall {
                    psi_operation,
                    result,
                    dynamic_dispatch,
                    call_plan,
                    result_home,
                    initial_argument,
                    rebound_argument,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::ReboundDynamic {
                        dynamic_dispatch,
                        call_plan,
                        result: Some((result, result_home)),
                        initial_argument,
                        rebound_argument,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::DynamicUnitCall {
                    psi_operation,
                    dynamic_dispatch,
                    call_plan,
                    initial_argument,
                    rebound_argument,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::ReboundDynamic {
                        dynamic_dispatch,
                        call_plan,
                        result: None,
                        initial_argument,
                        rebound_argument,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::DynamicParameterScalarCall {
                    psi_operation,
                    result,
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    table_slot_byte_offset,
                    result_home,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::ParameterDynamic {
                        dynamic_dispatch,
                        parameter_abi,
                        requirement,
                        dispatch_call_plan,
                        table_slot_byte_offset: *table_slot_byte_offset,
                        result: Some((result, result_home)),
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::DynamicParameterUnitCall {
                    psi_operation,
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    table_slot_byte_offset,
                    requirement_obligations,
                    crash_continuations,
                } => (
                    *psi_operation,
                    EmbeddedCall::ParameterDynamic {
                        dynamic_dispatch,
                        parameter_abi,
                        requirement,
                        dispatch_call_plan,
                        table_slot_byte_offset: *table_slot_byte_offset,
                        result: None,
                        requirement_obligations,
                        crash_continuations,
                    },
                ),
                TargetUnitOperation::StoreDynamicDescriptor {
                    psi_operation,
                    stored,
                    source_argument,
                } => (
                    *psi_operation,
                    EmbeddedCall::StoredDescriptor {
                        stored,
                        source_argument,
                    },
                ),
                TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    boundary,
                    provider_execution,
                    binding,
                    scalar_arguments,
                    structural_arguments,
                    result_home,
                } => (
                    *psi_operation,
                    EmbeddedCall::NormalizedForeign {
                        boundary: *boundary,
                        provider_execution,
                        binding,
                        scalar_arguments,
                        structural_arguments,
                        result_home,
                    },
                ),
                TargetUnitOperation::BoundarySettlement {
                    psi_operation,
                    boundary,
                    result,
                    execution,
                    realization,
                    scalar_arguments,
                    runtime_scalar_arguments,
                    arguments,
                    byte_sequence_arguments,
                    completion_claim_sources,
                    completion_receipts,
                } => (
                    *psi_operation,
                    EmbeddedCall::Settlement {
                        boundary: *boundary,
                        result,
                        execution,
                        realization,
                        scalar_arguments,
                        runtime_scalar_arguments,
                        arguments,
                        byte_sequence_arguments,
                        completion_claim_sources,
                        completion_receipts,
                        // An exit settlement ends its block: the producer
                        // rejects any later operation and admits only a plain
                        // `Return` terminator after a `HostedExitProcessI32`
                        // row.
                        nonreturning_tail: block
                            .operations
                            .last()
                            .is_some_and(|last| std::ptr::eq(last, operation))
                            && matches!(
                                &block.terminator,
                                TargetControlTerminator::Return {
                                    cleanup_actions,
                                    ..
                                } if cleanup_actions.is_empty()
                            ),
                    },
                ),
                _ => return None,
            };
            Some(call)
        })
    {
        if target_calls.insert(call.0, call.1).is_some() {
            return Err(call.0);
        }
    }

    // Every retained call row must bind to one source call-family operation.
    // A row keyed to anything else has no declaration to replay against, so it
    // is forged whether or not its fields happen to look consistent.
    let source_call_operations = source
        .operations
        .iter()
        .filter_map(|operation| {
            Some(*match operation {
                AbstractOperation::CallUnit { psi_operation, .. }
                | AbstractOperation::CallStructuralScalar { psi_operation, .. }
                | AbstractOperation::CallStructural { psi_operation, .. }
                | AbstractOperation::BoundaryCall { psi_operation, .. }
                | AbstractOperation::Call { psi_operation, .. }
                | AbstractOperation::CallUnitWithDynamicArguments { psi_operation, .. }
                | AbstractOperation::CallStructuralScalarWithDynamicArguments {
                    psi_operation,
                    ..
                }
                | AbstractOperation::CallStoredDynamicScalar { psi_operation, .. }
                | AbstractOperation::CallDynamicScalar { psi_operation, .. }
                | AbstractOperation::CallDynamicUnit { psi_operation, .. }
                | AbstractOperation::CallDynamicParameterScalar { psi_operation, .. }
                | AbstractOperation::CallDynamicParameterUnit { psi_operation, .. }
                | AbstractOperation::StoreDynamicDescriptor { psi_operation, .. } => psi_operation,
                _ => return None,
            })
        })
        .collect::<BTreeSet<_>>();

    let roots = canonical_roots(source);
    // The retained row identity selects which custody effect a source
    // `BoundaryCall` replays: an installed-provider call moves arguments and
    // establishes declared result leaves like an authored call, a settlement
    // establishes only its structural result home, and any other retained row
    // family has no custody effect.
    let mut boundary_call_rows = BTreeMap::new();
    for operation in &source.operations {
        let AbstractOperation::BoundaryCall { psi_operation, .. } = operation else {
            continue;
        };
        let row = match target_calls.get(psi_operation) {
            Some(EmbeddedCall::Direct {
                origin: NativeCallOrigin::InstalledProvider { provider, .. },
                ..
            }) => reference_results::BoundaryCallRow::Installed(provider.candidate),
            Some(EmbeddedCall::Settlement { .. }) => reference_results::BoundaryCallRow::Settlement,
            _ => reference_results::BoundaryCallRow::Other,
        };
        boundary_call_rows.insert(*psi_operation, row);
    }
    let replay = Replay {
        source,
        source_functions,
        target,
        target_functions,
        declarations,
        boundary_machines,
        native_target,
        native_callbacks,
        roots: &roots,
        expected_reference_results: reference_results::expected(
            source,
            source_functions,
            declarations,
            &boundary_call_rows,
        )
        .unwrap_or_default(),
    };

    for (position, operation) in source.operations.iter().enumerate() {
        match operation {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => replay.structural_call(
                *psi_operation,
                *callee,
                BoundResult::Unit,
                arguments,
                structural_arguments,
                claim_transfers,
                &[],
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => replay.structural_call(
                *psi_operation,
                *callee,
                BoundResult::Scalar(result),
                arguments,
                structural_arguments,
                claim_transfers,
                &[],
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallStructural {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            } => {
                if !selected_evidence.is_empty() {
                    return Err(*psi_operation);
                }
                replay.structural_call(
                    *psi_operation,
                    *callee,
                    BoundResult::Structural(result),
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    returned_claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    target_calls.get(psi_operation),
                )?;
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } => replay.boundary_call(
                *psi_operation,
                position,
                result,
                *boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
            } => replay.scalar_call(
                *psi_operation,
                *result,
                *scalar_type,
                *callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallUnitWithDynamicArguments {
                psi_operation,
                callee,
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => replay.dynamic_arguments_call(
                *psi_operation,
                *callee,
                None,
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallStructuralScalarWithDynamicArguments {
                psi_operation,
                result,
                callee,
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => replay.dynamic_arguments_call(
                *psi_operation,
                *callee,
                Some(result),
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallStoredDynamicScalar {
                psi_operation,
                result,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
            } => replay.stored_dynamic_call(
                *psi_operation,
                result,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallDynamicScalar {
                psi_operation,
                result,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
            } => replay.rebound_dynamic_call(
                *psi_operation,
                Some(result),
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallDynamicUnit {
                psi_operation,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
            } => replay.rebound_dynamic_call(
                *psi_operation,
                None,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallDynamicParameterScalar {
                psi_operation,
                result,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
            } => replay.parameter_dynamic_call(
                *psi_operation,
                Some(result),
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::CallDynamicParameterUnit {
                psi_operation,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
            } => replay.parameter_dynamic_call(
                *psi_operation,
                None,
                dynamic_dispatch,
                requirement_obligations,
                crash_continuations,
                target_calls.get(psi_operation),
            )?,
            AbstractOperation::StoreDynamicDescriptor {
                psi_operation,
                stored,
            } => {
                replay.stored_descriptor(*psi_operation, stored, target_calls.get(psi_operation))?
            }
            _ => continue,
        }
    }
    if let Some(&forged) = target_calls
        .keys()
        .find(|key| !source_call_operations.contains(key))
    {
        return Err(forged);
    }
    Ok(())
}

impl Replay<'_> {
    fn callee(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> Result<&AbstractFunction, OperationId> {
        self.source_functions
            .iter()
            .find(|function| function.machine == machine)
            .ok_or(operation)
    }

    /// The independently derived signature and evaluated plan for one callee.
    /// The embedded plan is not authority: a substituted scalar row, result
    /// placement, or plan detail cannot carry matching destinations.
    fn expected_plan(
        &self,
        callee: &AbstractFunction,
        operation: OperationId,
    ) -> Result<CallPlan, OperationId> {
        let signature =
            structural_signatures::signature(callee, self.declarations, self.native_target)
                .ok_or(operation)?;
        evaluate_call_plan(
            CallingPolicy::native_for_target(self.native_target),
            &signature,
        )
        .map_err(|_| operation)
    }

    /// Authored and installed `Call`-family rows share one roster shape; the
    /// expected origin and result contract differ per source operation.
    #[allow(clippy::too_many_arguments)]
    fn structural_call(
        &self,
        psi_operation: OperationId,
        source_callee: MachineId,
        expected_result: BoundResult<'_>,
        scalar_arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        claim_transfers: &[ClaimTransfer],
        returned_claim_transfers: &[StructuralResultClaimTransfer],
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(call) = call else {
            return Ok(());
        };
        let EmbeddedCall::Direct {
            origin,
            callee,
            call_plan,
            scalar_arguments: actual_scalar,
            arguments: actual_structural,
            result: actual_result,
            claim_transfers: actual_claims,
            returned_claim_transfers: actual_returned,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        } = call
        else {
            return Err(psi_operation);
        };
        // The producer admits only empty claim rosters on every lane that can
        // emit this row, and an authored source never retains an
        // installed-provider origin. Requirement obligations are discharged
        // proof metadata: the retained row must replay the source roster
        // exactly rather than drop it. Crash continuations are the call's
        // verified surviving-route roster: they ride the same correspondence,
        // so the retained row must replay them exactly as the scalar and
        // dynamic call lanes already do. A retained call carrying extra rows
        // or the wrong origin is forged even when plausible.
        if *origin != &NativeCallOrigin::Authored
            || !claim_transfers.is_empty()
            || !returned_claim_transfers.is_empty()
            || !actual_claims.is_empty()
            || !actual_returned.is_empty()
            || *actual_crashes != crash_continuations
            || requirement_obligations != *actual_obligations
            || *callee != source_callee
            || actual_structural.len() != structural_arguments.len()
            || actual_scalar.len() != scalar_arguments.len()
        {
            return Err(psi_operation);
        }
        let callee_function = self.callee(source_callee, psi_operation)?;
        // Every producer lane admits only fixed-native scalar parameters on
        // the callee; a wider roster cannot produce this row.
        if callee_function.structural_parameters.len() != actual_structural.len()
            || callee_function.parameters.len() != scalar_arguments.len()
            || callee_function.parameters.iter().any(|parameter| {
                structural_signatures::fixed_native_scalar_shape(parameter.scalar_type).is_none()
            })
        {
            return Err(psi_operation);
        }
        let expected_plan = self.expected_plan(callee_function, psi_operation)?;
        if *call_plan != &expected_plan {
            return Err(psi_operation);
        }
        self.scalar_arguments(
            psi_operation,
            actual_scalar,
            scalar_arguments,
            &callee_function.parameters,
            &expected_plan,
        )?;
        for (index, ((actual, semantic), declared)) in actual_structural
            .iter()
            .zip(structural_arguments)
            .zip(&callee_function.structural_parameters)
            .enumerate()
        {
            self.structural_argument(
                psi_operation,
                index + scalar_arguments.len(),
                actual,
                semantic,
                declared,
                &expected_plan,
            )?;
        }
        self.bind_result(
            psi_operation,
            expected_result,
            actual_result,
            callee_function,
        )
    }

    /// An installed provider call projects the boundary call's scalar and
    /// structural rosters through the selected candidate's signature; the
    /// origin row carries the boundary, provider, and receipt identity so a
    /// substituted provider or receipt set cannot satisfy the replay.
    #[allow(clippy::too_many_arguments)]
    fn boundary_call(
        &self,
        psi_operation: OperationId,
        position: usize,
        result: &AbstractBoundaryResult,
        boundary: semantic_vocabulary::BoundaryMachineId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        completion_claim_sources: &[CompletionClaimSource],
        completion_receipts: &[terminal_psi::CompletionReceipt],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(call) = call else {
            return Ok(());
        };
        if let EmbeddedCall::NormalizedForeign { .. } = call {
            return self.normalized_foreign_call(
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                call,
            );
        }
        if let EmbeddedCall::Settlement { .. } = call {
            return self.boundary_settlement(
                psi_operation,
                position,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                call,
            );
        }
        let EmbeddedCall::Direct {
            origin:
                NativeCallOrigin::InstalledProvider {
                    boundary: actual_boundary,
                    provider,
                    completion_claim_sources: actual_sources,
                    completion_receipts: actual_receipts,
                },
            callee,
            call_plan,
            scalar_arguments: actual_scalar,
            arguments: actual_structural,
            result: actual_result,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = call
        else {
            return Err(psi_operation);
        };
        // The resolved call carries claim transfers derived from the
        // completion receipts; every receiving lane admits only empty rosters,
        // so a call with receipts or any nonempty roster row is forged.
        if *actual_boundary != boundary
            || provider.boundary != boundary
            || provider.candidate != *callee
            || *actual_sources != *completion_claim_sources
            || *actual_receipts != *completion_receipts
            || !completion_receipts.is_empty()
            || !claim_transfers.is_empty()
            || !returned_claim_transfers.is_empty()
            || !requirement_obligations.is_empty()
            || !crash_continuations.is_empty()
        {
            return Err(psi_operation);
        }
        let expected_result = match result {
            AbstractBoundaryResult::Unit => BoundResult::Unit,
            // A scalar boundary result has no honest embedded call row: the
            // resolved projection admits only Unit and Structural results.
            AbstractBoundaryResult::Scalar(_) => return Err(psi_operation),
            AbstractBoundaryResult::Structural(result) => BoundResult::Structural(result),
        };
        let callee_function = self.callee(provider.candidate, psi_operation)?;
        if callee_function.structural_parameters.len() != actual_structural.len()
            || callee_function.parameters.len() != arguments.len()
            || actual_structural.len() != structural_arguments.len()
            || actual_scalar.len() != arguments.len()
            || callee_function.parameters.iter().any(|parameter| {
                structural_signatures::fixed_native_scalar_shape(parameter.scalar_type).is_none()
            })
        {
            return Err(psi_operation);
        }
        let expected_plan = self.expected_plan(callee_function, psi_operation)?;
        if *call_plan != &expected_plan {
            return Err(psi_operation);
        }
        self.scalar_arguments(
            psi_operation,
            actual_scalar,
            arguments,
            &callee_function.parameters,
            &expected_plan,
        )?;
        for (index, ((actual, semantic), declared)) in actual_structural
            .iter()
            .zip(structural_arguments)
            .zip(&callee_function.structural_parameters)
            .enumerate()
        {
            self.structural_argument(
                psi_operation,
                index + arguments.len(),
                actual,
                semantic,
                declared,
                &expected_plan,
            )?;
        }
        self.bind_result(
            psi_operation,
            expected_result,
            actual_result,
            callee_function,
        )
    }

    /// An evaluated normalized foreign call embeds the boundary's own entry
    /// plan and projects borrowed flat-record arguments through it. The
    /// retained plan, locator, provider binding, and argument rows are never
    /// authority: every coordinate is re-derived from the unique boundary
    /// declaration, the caller's checked structural parameter roster, and the
    /// target's own ABI evaluation. A registrar callback lane replays against
    /// the plan's retained `native_callback_arguments`: the admitted argument
    /// must join this row by Terminal operation and exact registrar entry
    /// plan, and its context re-validates the materialized signature in place
    /// of the ordinary plan check.
    #[allow(clippy::too_many_arguments)]
    fn normalized_foreign_call(
        &self,
        psi_operation: OperationId,
        result: &AbstractBoundaryResult,
        boundary: BoundaryMachineId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        completion_claim_sources: &[CompletionClaimSource],
        completion_receipts: &[terminal_psi::CompletionReceipt],
        call: &EmbeddedCall<'_>,
    ) -> Result<(), OperationId> {
        let EmbeddedCall::NormalizedForeign {
            boundary: actual_boundary,
            provider_execution,
            binding,
            scalar_arguments: actual_scalar,
            structural_arguments: actual_structural,
            result_home,
        } = call
        else {
            return Err(psi_operation);
        };
        if *actual_boundary != boundary
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
        {
            return Err(psi_operation);
        }
        // Exactly one declaration may carry the retained boundary identity,
        // and the admitted same-stack contribution must name that
        // declaration's requirement and the exact provider plan behind the
        // retained execution binding.
        let mut declarations = self
            .boundary_machines
            .iter()
            .filter(|row| row.id == boundary);
        let declaration = declarations.next().ok_or(psi_operation)?;
        if declarations.next().is_some()
            || binding.locator.target().native_target() != self.native_target
            || binding.same_stack_contribution.requirement_identity() != declaration.identity
            || binding
                .same_stack_contribution
                .provider_plan_report_identity()
                != provider_execution.provider_plan_report_identity().get()
            || binding.boundary_entry_plan.call.policy
                != CallingPolicy::native_for_target(self.native_target)
            || binding.boundary_entry_plan.call.entry_control != EntryControl::CallReturn
        {
            return Err(psi_operation);
        }
        // Each borrowed structural parameter transports one referent pointer:
        // the signature is rebuilt from the target's pointer word, never from
        // the retained rows. A retained callback occupies one such slot too.
        let pointer_size =
            u16::try_from(self.native_target.pointer_size).map_err(|_| psi_operation)?;
        let pointer_alignment =
            u16::try_from(self.native_target.pointer_alignment).map_err(|_| psi_operation)?;
        let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
        // A retained callback occupies one native-only parameter slot in the
        // registrar plan; the roster entry joins by Terminal operation and
        // exact registrar plan. A materialized plan without one, a duplicated
        // operation key, a substituted registrar plan, and a claimed
        // application placement the plan does not carry all fail closed.
        let mut matching_callbacks = self
            .native_callbacks
            .iter()
            .filter(|callback| callback.terminal_operation == psi_operation);
        let callback = match matching_callbacks.next() {
            None => {
                if !binding
                    .boundary_entry_plan
                    .call
                    .callback_materializations
                    .is_empty()
                {
                    return Err(psi_operation);
                }
                None
            }
            Some(callback) => {
                let ordinal = usize::try_from(callback.application.native_ordinal)
                    .map_err(|_| psi_operation)?;
                if binding
                    .boundary_entry_plan
                    .call
                    .callback_materializations
                    .is_empty()
                    || matching_callbacks.next().is_some()
                    || callback.registrar_boundary_entry_plan != binding.boundary_entry_plan
                    || !callback.callback_function.is_valid()
                    || callback.callback_function.callback_thunk_placement_index()
                        != Some(callback.placement_index)
                    || callback.application.shape != callback.application.placement.shape
                    || callback.application.shape != pointer_shape
                    || binding.boundary_entry_plan.call.parameters.get(ordinal)
                        != Some(&callback.application.placement)
                {
                    return Err(psi_operation);
                }
                Some((callback, ordinal))
            }
        };
        // Reconstruct both ABI lanes from the portable declaration's authored
        // runtime order, independently of the target argument rows.
        if structural_arguments.len() != declaration.structural_parameters.len()
            || actual_structural.len() != structural_arguments.len()
            || arguments.len() != declaration.scalar_parameters.len()
            || actual_scalar.len() != arguments.len()
            || !declaration.has_valid_parameter_order()
        {
            return Err(psi_operation);
        }
        let scalar_shapes = declaration
            .scalar_parameters
            .iter()
            .map(|parameter| {
                let ScalarType::Integer(integer) = parameter else {
                    return Err(psi_operation);
                };
                structural_signatures::fixed_native_integer_shape(*integer).ok_or(psi_operation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected_result = match (result, &declaration.result) {
            (AbstractBoundaryResult::Unit, terminal_psi::BoundaryMachineResult::Unit) => None,
            (
                AbstractBoundaryResult::Scalar(result),
                terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(declared)),
            ) => {
                // A scalar result needs the attached Unit frame to retain the
                // home, and the declared result must be the same fixed-width
                // integer the abstract result names.
                let ScalarType::Integer(result_type) = result.scalar_type else {
                    return Err(psi_operation);
                };
                if *declared != result_type || self.source.attachment.is_none() {
                    return Err(psi_operation);
                }
                let shape = structural_signatures::fixed_native_integer_shape(result_type)
                    .ok_or(psi_operation)?;
                Some((
                    TargetUnitScalarHomeRequirement {
                        defining_operation: psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape,
                    },
                    shape,
                ))
            }
            _ => return Err(psi_operation),
        };
        // The materialized signature spells every authored and private
        // placement, while the declaration still counts only semantic
        // formals — semantic argument indices shift around the callback's
        // native ordinal.
        let mut next_scalar_shape = scalar_shapes.iter();
        let mut parameter_shapes = declaration
            .parameter_order
            .iter()
            .map(|kind| match kind {
                terminal_psi::BoundaryParameterKind::Scalar => {
                    next_scalar_shape.next().copied().ok_or(psi_operation)
                }
                terminal_psi::BoundaryParameterKind::Structural => Ok(pointer_shape),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some((_, ordinal)) = callback {
            if ordinal > parameter_shapes.len() {
                return Err(psi_operation);
            }
            parameter_shapes.insert(ordinal, pointer_shape);
        }
        let signature = CallSignature {
            parameters: parameter_shapes,
            result: expected_result.as_ref().map(|(_, shape)| *shape),
        };
        let validated = match callback {
            Some((callback, _)) => {
                calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                    binding.boundary_entry_plan.clone(),
                    &signature,
                    &callback.registrar_context,
                )
            }
            None => calling_conventions::validate_boundary_entry_plan(
                binding.boundary_entry_plan.clone(),
                &signature,
            ),
        }
        .map_err(|_| psi_operation)?;
        if validated.plan() != &binding.boundary_entry_plan
            || binding.boundary_entry_plan.call.parameters.len()
                != scalar_shapes.len()
                    + structural_arguments.len()
                    + usize::from(callback.is_some())
            || expected_result.as_ref().map(|(home, _)| home) != result_home.as_ref()
        {
            return Err(psi_operation);
        }
        match (
            &binding.boundary_entry_plan.call.result,
            expected_result.as_ref().map(|(_, shape)| *shape),
        ) {
            (None, None) => {}
            (Some(placement), Some(shape)) => {
                let placed = matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    }] if *byte_size == shape.byte_size
                );
                if placement.shape != shape || !placed {
                    return Err(psi_operation);
                }
            }
            _ => return Err(psi_operation),
        }
        // `Home` sources cite the exact retained result home of the producing
        // operation — replayed on that row's own source operation — while an
        // `IntegerImmediate` must equal the source `IntegerConstant` op it
        // names. Parameters, block values, and boolean or float immediates
        // are inadmissible on this lane, so every other `source` kind is a
        // substitution.
        let mut retained_scalar_homes = BTreeMap::new();
        for operation in self
            .target
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
        {
            let home = match operation {
                TargetUnitOperation::IeeeFloatCompare { result_home, .. }
                | TargetUnitOperation::StoredDynamicScalarCall { result_home, .. }
                | TargetUnitOperation::DynamicScalarCall { result_home, .. }
                | TargetUnitOperation::DynamicParameterScalarCall { result_home, .. }
                | TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                    result_home,
                    ..
                }
                | TargetUnitOperation::ScalarDefinition { result_home, .. } => Some(result_home),
                TargetUnitOperation::Call { result, .. } => result.scalar_home(),
                TargetUnitOperation::NormalizedForeignCall { result_home, .. } => {
                    result_home.as_ref()
                }
                _ => None,
            };
            if let Some(home) = home {
                retained_scalar_homes.insert(home.defining_operation, home);
            }
        }
        for ((index, ((actual, value), shape)), formal_position) in actual_scalar
            .iter()
            .zip(arguments)
            .zip(&scalar_shapes)
            .enumerate()
            .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
        {
            let placed_byte_size = match actual.placement.locations.as_slice() {
                [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ]
                | [
                    ValueLocation::Stack {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] => *byte_size,
                _ => return Err(psi_operation),
            };
            let source_invalid = match &actual.source {
                TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value: literal,
                } => {
                    *source_value != *value
                        || semantic_vocabulary::ScalarTerm::integer(*scalar_type, *literal).is_err()
                        || !self.source.operations.iter().any(|operation| {
                            matches!(
                                operation,
                                AbstractOperation::IntegerConstant {
                                    psi_operation,
                                    result,
                                    scalar_type: declared,
                                    value
                                } if *psi_operation == *defining_operation
                                    && *result == *source_value
                                    && *declared == ScalarType::Integer(*scalar_type)
                                    && *value == *literal
                            )
                        })
                }
                TargetUnitScalarArgumentSource::Home(home) => {
                    home.source_value != *value
                        || home.shape != *shape
                        || home.defining_operation == psi_operation
                        || retained_scalar_homes.get(&home.defining_operation) != Some(&home)
                }
                _ => true,
            };
            // When a retained callback claims a native-only slot, every
            // semantic scalar at or after its ordinal shifts one placement
            // position — the materialized signature is positional in the
            // authored order, not the semantic order.
            let expected_index = formal_position
                + usize::from(callback.is_some_and(|(_, ordinal)| formal_position >= ordinal));
            let destination = binding
                .boundary_entry_plan
                .call
                .parameters
                .get(expected_index)
                .ok_or(psi_operation)?;
            if usize::try_from(actual.parameter_index).ok() != Some(expected_index)
                || actual.placement != *destination
                || actual.placement.shape != *shape
                || shape.byte_size != placed_byte_size
                || actual.source.source_value() != *value
                || actual.source.scalar_type() != declaration.scalar_parameters[index]
                || source_invalid
            {
                return Err(psi_operation);
            }
        }
        for ((index, ((actual, semantic), declaration_parameter)), formal_position) in
            actual_structural
                .iter()
                .zip(structural_arguments)
                .zip(&declaration.structural_parameters)
                .enumerate()
                .zip(
                    declaration
                        .parameter_positions(terminal_psi::BoundaryParameterKind::Structural),
                )
        {
            // The referent root must be one of the caller's own checked
            // structural parameters; the projected type, offset, and home are
            // re-derived from its declaration rather than the retained row.
            let root = self
                .target
                .graph
                .parameters
                .iter()
                .find(|parameter| parameter.place == semantic.place)
                .ok_or(psi_operation)?;
            if semantic.path.is_empty()
                || semantic
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(psi_operation);
            }
            let (projected_type, projected_shape, byte_offset) =
                structural_shapes::projected_field(
                    root.structural_type,
                    &semantic.path,
                    self.declarations,
                )
                .map_err(|_| psi_operation)?;
            let destination = binding
                .boundary_entry_plan
                .call
                .parameters
                .get(
                    formal_position
                        + usize::from(
                            callback.is_some_and(|(_, ordinal)| formal_position >= ordinal),
                        ),
                )
                .ok_or(psi_operation)?;
            let placed_pointer_word = match destination.locations.as_slice() {
                [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ]
                | [
                    ValueLocation::Stack {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] => *byte_size,
                _ => return Err(psi_operation),
            };
            if usize::try_from(declaration_parameter.position).ok() != Some(index)
                || semantic.access != declaration_parameter.access
                || !matches!(
                    semantic.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || projected_type != declaration_parameter.structural_type
                || declaration_parameter.multiplicity
                    != terminal_psi::StructuralMultiplicity::Unrestricted
                || !declaration_parameter.qualifications.is_empty()
                || !declaration_parameter.projected_qualifications.is_empty()
                || u32::from(projected_shape.byte_size)
                    .checked_add(byte_offset)
                    .is_none_or(|end| end > u32::from(root.shape.byte_size))
                || destination.shape != pointer_shape
                || placed_pointer_word != pointer_size
            {
                return Err(psi_operation);
            }
            let expected_shape =
                structural_shapes::parameter_shape(projected_shape, declaration_parameter.access);
            if actual.place != semantic.place
                || actual.access != semantic.access
                || actual.path != semantic.path
                || actual.root_structural_type != root.structural_type
                || actual.structural_type != projected_type
                || actual.shape != expected_shape
                || actual.source_byte_offset != byte_offset
                || actual.fixed_array_length.is_some()
                || actual.element_stride.is_some()
                || actual.source
                    != TargetStructuralArgumentSource::Placement(root.placement.clone())
                || actual.destination != *destination
            {
                return Err(psi_operation);
            }
        }
        Ok(())
    }

    /// A compiler-builtin `BoundarySettlement` is not a call through a callee
    /// signature: it retains the closed realization, the admitted execution
    /// custody, the verbatim semantic rosters, and the hosted runtime scalar
    /// lane its realization consumes. Every retained coordinate replays
    /// against the unique boundary declaration and the caller's own scalar
    /// provenance — the retained rows are never admission authority. Only the
    /// three hosted realizations produce this row; every other lane fails
    /// closed at the routing guard before the settlement exists.
    #[allow(clippy::too_many_arguments)]
    fn boundary_settlement(
        &self,
        psi_operation: OperationId,
        position: usize,
        result: &AbstractBoundaryResult,
        boundary: BoundaryMachineId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        completion_claim_sources: &[CompletionClaimSource],
        completion_receipts: &[terminal_psi::CompletionReceipt],
        call: &EmbeddedCall<'_>,
    ) -> Result<(), OperationId> {
        let EmbeddedCall::Settlement {
            boundary: actual_boundary,
            result: actual_result,
            execution,
            realization,
            scalar_arguments,
            runtime_scalar_arguments,
            arguments: actual_arguments,
            byte_sequence_arguments,
            completion_claim_sources: actual_sources,
            completion_receipts: actual_receipts,
            nonreturning_tail,
        } = call
        else {
            return Err(psi_operation);
        };
        // The semantic rosters are verbatim clones of the source operation;
        // the compile-time scalar roster and the byte-sequence roster are
        // always empty on every lane this producer can emit.
        if *actual_boundary != boundary
            || *actual_arguments != structural_arguments
            || *actual_sources != completion_claim_sources
            || *actual_receipts != completion_receipts
            || !scalar_arguments.is_empty()
            || !byte_sequence_arguments.is_empty()
        {
            return Err(psi_operation);
        }
        // Execution custody is admission evidence the artifact cannot
        // reconstruct, but a compiler builtin always pairs with its own
        // realization — a mismatched builtin pair is forged by construction.
        let execution_matches = match *execution {
            BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::HostedExitProcessI32,
            ) => matches!(realization, BoundaryRealization::HostedExitProcessI32(_)),
            BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::HostedWriteByteI32,
            ) => matches!(realization, BoundaryRealization::HostedWriteByteI32(_)),
            BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedReadByte) => {
                matches!(realization, BoundaryRealization::HostedReadByte(_))
            }
            BoundaryExecutionBinding::AdmittedProvider(_) => true,
        };
        if !execution_matches {
            return Err(psi_operation);
        }
        // Exactly one declaration may carry the retained boundary identity.
        let mut declarations = self
            .boundary_machines
            .iter()
            .filter(|row| row.id == boundary);
        let declaration = declarations.next().ok_or(psi_operation)?;
        if declarations.next().is_some() {
            return Err(psi_operation);
        }
        // A builtin settlement admits a Unit result only when the declaration
        // returns Unit and the realization is not the hosted byte read. A
        // Structural result requires the declared carrier identity and the
        // independently reconstructed conventional-sum home, and only the
        // hosted byte read carries one. A scalar boundary result has no
        // honest builtin settlement row at all.
        let result_home = match (result, *actual_result) {
            (AbstractBoundaryResult::Unit, TargetBoundaryResult::Unit) => {
                if !declaration.result.is_unit()
                    || matches!(realization, BoundaryRealization::HostedReadByte(_))
                {
                    return Err(psi_operation);
                }
                None
            }
            (
                AbstractBoundaryResult::Structural(result),
                TargetBoundaryResult::Structural(home),
            ) => {
                if !matches!(realization, BoundaryRealization::HostedReadByte(_)) {
                    return Err(psi_operation);
                }
                let terminal_psi::BoundaryMachineResult::Structural(expected) = &declaration.result
                else {
                    return Err(psi_operation);
                };
                if result.structural_type != expected.structural_type
                    || result.multiplicity != expected.multiplicity
                    || result.qualifications != expected.qualifications
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return Err(psi_operation);
                }
                let expected_home = structural_shapes::boundary_result_home(
                    psi_operation,
                    result,
                    self.declarations,
                )
                .map_err(|_| psi_operation)?;
                if *home != expected_home {
                    return Err(psi_operation);
                }
                Some((result, home))
            }
            _ => return Err(psi_operation),
        };
        match realization {
            BoundaryRealization::HostedWriteByteI32(_)
            | BoundaryRealization::HostedExitProcessI32(_) => {
                // The hosted scalar lane forwards exactly one signed i32
                // argument through the evaluated one-parameter native plan;
                // the retained source's provenance replays independently.
                let i32_type =
                    IntegerType::new(IntegerSign::Signed, 32).expect("i32 is a valid type");
                let shape = structural_signatures::fixed_native_integer_shape(i32_type)
                    .ok_or(psi_operation)?;
                let call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(self.native_target),
                    &CallSignature {
                        parameters: vec![shape],
                        result: None,
                    },
                )
                .map_err(|_| psi_operation)?;
                let [placement] = call_plan.parameters.as_slice() else {
                    return Err(psi_operation);
                };
                let exits = matches!(realization, BoundaryRealization::HostedExitProcessI32(_));
                let supports_target = if exits {
                    target_operations::HostedExitProcessI32Realization::supports_target(
                        self.native_target,
                    )
                } else {
                    target_operations::HostedWriteByteI32Realization::supports_target(
                        self.native_target,
                    )
                };
                let [row] = *runtime_scalar_arguments else {
                    return Err(psi_operation);
                };
                let [source_value] = *arguments else {
                    return Err(psi_operation);
                };
                if !supports_target
                    || declaration.scalar_parameters.as_slice() != [ScalarType::Integer(i32_type)]
                    || !declaration.structural_parameters.is_empty()
                    || !structural_arguments.is_empty()
                    || row.parameter_index != 0
                    || row.placement != *placement
                {
                    return Err(psi_operation);
                }
                // `HostedExitProcessI32` never returns: the settlement is the
                // last operation of its block and the block closes on a plain
                // `Return`, and the source call must end its own block on a
                // cleanup-free `ReturnUnit`.
                if exits && (!*nonreturning_tail || !hosted_exit_source_tail(self.source, position))
                {
                    return Err(psi_operation);
                }
                self.hosted_scalar_source(psi_operation, source_value, i32_type, &row.source)
            }
            BoundaryRealization::HostedReadByte(_) => {
                // The hosted byte read writes one `[empty, byte]` sum result:
                // an empty first case and a single signed 32-bit payload
                // field whose declared or bounded carrier covers a byte.
                let Some((result, home)) = result_home else {
                    return Err(psi_operation);
                };
                let Some(result_declaration) = self
                    .declarations
                    .iter()
                    .find(|declaration| declaration.id == result.structural_type)
                else {
                    return Err(psi_operation);
                };
                let StructuralTypeShape::Sum { cases } = &result_declaration.shape else {
                    return Err(psi_operation);
                };
                let [empty, byte] = cases.as_slice() else {
                    return Err(psi_operation);
                };
                let [field] = byte.fields.as_slice() else {
                    return Err(psi_operation);
                };
                let valid_payload = empty.fields.is_empty()
                    && !field.relevance.is_erased()
                    && match field.field_type {
                        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                            !integer.is_address()
                                && integer.sign() == IntegerSign::Signed
                                && integer.bits() == 32
                        }
                        StructuralFieldType::BoundedInteger(bounds) => {
                            let integer = bounds.integer_type();
                            !integer.is_address()
                                && integer.sign() == IntegerSign::Signed
                                && integer.bits() == 32
                                && bounds.contains(IntegerValue::Signed(0))
                                && bounds.contains(IntegerValue::Signed(255))
                        }
                        _ => false,
                    };
                if !target_operations::HostedReadByteRealization::supports_target(
                    self.native_target,
                ) || !valid_payload
                    || !arguments.is_empty()
                    || !structural_arguments.is_empty()
                    || !declaration.scalar_parameters.is_empty()
                    || !declaration.structural_parameters.is_empty()
                    || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty()
                    || !runtime_scalar_arguments.is_empty()
                    || home.layout.sum().is_none_or(|layout| {
                        layout.tag_byte_offset != 0 || layout.tag_shape != ValueShape::integer(4, 4)
                    })
                {
                    return Err(psi_operation);
                }
                Ok(())
            }
            _ => Err(psi_operation),
        }
    }

    /// One hosted scalar lane argument replays the producer's known-integer
    /// source: an incoming parameter, a non-entry block parameter, an
    /// `IntegerConstant` the source body already defined, or the durable home
    /// one earlier integer result left behind. Every other `source` kind is
    /// a substitution.
    fn hosted_scalar_source(
        &self,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        source: &TargetUnitScalarArgumentSource,
    ) -> Result<(), OperationId> {
        let expected_scalar = ScalarType::Integer(scalar_type);
        let valid = match source {
            TargetUnitScalarArgumentSource::Parameter {
                parameter_index,
                source_value: actual,
                scalar_type: actual_type,
            } => {
                *actual == source_value
                    && *actual_type == expected_scalar
                    && usize::try_from(*parameter_index)
                        .ok()
                        .and_then(|index| self.source.parameters.get(index))
                        .is_some_and(|parameter| {
                            parameter.value == source_value
                                && parameter.scalar_type == expected_scalar
                        })
            }
            TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
                parameter.value == source_value
                    && parameter.scalar_type == expected_scalar
                    && self.source.block_entries.iter().any(|entry| {
                        entry.block == parameter.block
                            && entry.block != self.source.entry
                            && entry.parameters.iter().any(|block_parameter| {
                                block_parameter.value == source_value
                                    && block_parameter.scalar_type == parameter.scalar_type
                            })
                    })
            }
            TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value: actual,
                scalar_type: actual_type,
                value,
            } => {
                *actual == source_value
                    && *actual_type == scalar_type
                    && actual_type.admits(*value)
                    && self.source.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            AbstractOperation::IntegerConstant {
                                psi_operation: produced,
                                result,
                                scalar_type: declared,
                                value: literal,
                            } if *produced == *defining_operation
                                && *result == *actual
                                && *declared == expected_scalar
                                && *literal == *value
                        )
                    })
            }
            TargetUnitScalarArgumentSource::Home(home) => {
                home.source_value == source_value
                    && home.scalar_type == expected_scalar
                    && home.defining_operation != psi_operation
                    && Some(home.shape)
                        == structural_signatures::fixed_native_integer_shape(scalar_type)
                    && self.source.operations.iter().any(|operation| {
                        integer_home_result(operation).is_some_and(|(produced, result)| {
                            produced == home.defining_operation
                                && result.value == home.source_value
                                && result.scalar_type == home.scalar_type
                        })
                    })
            }
            _ => false,
        };
        if valid { Ok(()) } else { Err(psi_operation) }
    }

    /// A scalar-result direct call replays the published fixed-native scalar
    /// ABI: the callee must be the same service-free family the standalone
    /// entrance publishes, and the published ABI rows must equal this plan.
    #[allow(clippy::too_many_arguments)]
    fn scalar_call(
        &self,
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        source_callee: MachineId,
        arguments: &[ValueId],
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::Direct {
            origin,
            callee,
            call_plan,
            result: EmbeddedResult::Scalar(result_home),
            scalar_arguments: actual,
            arguments: structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        if origin != &NativeCallOrigin::Authored
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
            || !returned_claim_transfers.is_empty()
            || callee != source_callee
            || actual.len() != arguments.len()
            || *actual_obligations != *requirement_obligations
            || *actual_crashes != *crash_continuations
        {
            return Err(psi_operation);
        }
        let callee_function = self.callee(source_callee, psi_operation)?;
        let Some(callee_result) = callee_function.result.scalar() else {
            return Err(psi_operation);
        };
        if !callee_function.structural_parameters.is_empty()
            || !callee_function.entry_claims.is_empty()
            || !callee_function.published_service_ceiling.is_empty()
            || callee_function.operations.iter().any(|operation| {
                matches!(
                    operation,
                    AbstractOperation::DynamicDescriptorParameter { .. }
                )
            })
            || callee_result.scalar_type != scalar_type
            || structural_signatures::fixed_native_scalar_shape(scalar_type).is_none()
            || callee_function.parameters.len() != arguments.len()
            || callee_function.parameters.iter().any(|parameter| {
                structural_signatures::fixed_native_scalar_shape(parameter.scalar_type).is_none()
            })
        {
            return Err(psi_operation);
        }
        let expected_plan = self.expected_plan(callee_function, psi_operation)?;
        if call_plan != &expected_plan {
            return Err(psi_operation);
        }
        let Some(result_placement) = expected_plan.result.as_ref() else {
            return Err(psi_operation);
        };
        // The producer admits only a single-register result at offset zero.
        if !matches!(
            result_placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            }] if *byte_size == result_placement.shape.byte_size
        ) {
            return Err(psi_operation);
        }
        self.scalar_arguments(
            psi_operation,
            actual,
            arguments,
            &callee_function.parameters,
            &expected_plan,
        )?;
        if result_home.defining_operation != psi_operation
            || result_home.source_value != result
            || result_home.scalar_type != scalar_type
            || result_home.shape != result_placement.shape
        {
            return Err(psi_operation);
        }
        // The callee's published standalone entrance must carry this same
        // plan and identity rows; a call to a callee that never published one
        // is forged no matter how consistent the embedded plan looks.
        let expected_abi = ScalarFunctionAbi {
            call_plan: expected_plan.clone(),
            parameters: callee_function
                .parameters
                .iter()
                .zip(&expected_plan.parameters)
                .map(|(parameter, placement)| ScalarAbiValue {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    placement: placement.clone(),
                })
                .collect(),
            result: ScalarAbiValue {
                value: callee_result.value,
                scalar_type: callee_result.scalar_type,
                placement: result_placement.clone(),
            },
        };
        if self
            .target_functions
            .iter()
            .find(|function| function.machine == source_callee)
            .and_then(|function| function.scalar_abi.as_ref())
            != Some(&expected_abi)
        {
            return Err(psi_operation);
        }
        Ok(())
    }

    /// A dynamic-argument call expands each existential descriptor into an
    /// ordered `{data, table}` pointer pair. The callee's leading descriptor
    /// parameters, the source custody joins, and the concrete instance
    /// projections all replay independently.
    #[allow(clippy::too_many_arguments)]
    fn dynamic_arguments_call(
        &self,
        psi_operation: OperationId,
        source_callee: MachineId,
        expected_result: Option<&AbstractResult>,
        source_structural: &[StructuralArgument],
        source_dynamic: &[AbstractDynamicDescriptorArgument],
        claim_transfers: &[ClaimTransfer],
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::DynamicArguments {
            callee,
            call_plan,
            result: actual_result,
            structural_arguments,
            dynamic_arguments,
            claim_transfers: actual_claims,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        // A non-entry helper may forward its own borrowed descriptor
        // parameters without an attachment; projected or rebound sources still
        // require the attached structural-parameter lane they read from.
        if (self.source.attachment.is_none()
            && source_dynamic.iter().any(|argument| {
                !matches!(
                    argument.source,
                    AbstractDynamicDescriptorSource::Parameter(_)
                )
            }))
            || !source_structural.is_empty()
            || source_dynamic.is_empty()
            || callee != source_callee
            || !structural_arguments.is_empty()
            || dynamic_arguments.len() != source_dynamic.len()
            || *actual_claims != *claim_transfers
            || *actual_obligations != *requirement_obligations
            || *actual_crashes != *crash_continuations
        {
            return Err(psi_operation);
        }
        let callee_function = self.callee(source_callee, psi_operation)?;
        let callee_dynamic_parameters = callee_function
            .operations
            .iter()
            .take_while(|operation| {
                matches!(
                    operation,
                    AbstractOperation::DynamicDescriptorParameter { .. }
                )
            })
            .filter_map(|operation| match operation {
                AbstractOperation::DynamicDescriptorParameter { parameter } => Some(parameter),
                _ => None,
            })
            .collect::<Vec<_>>();
        let result_matches = match (expected_result, &callee_function.result) {
            (None, AbstractFunctionResult::Unit) => true,
            (Some(result), AbstractFunctionResult::Scalar(declared)) => {
                declared.scalar_type == result.scalar_type
                    && matches!(
                        result.scalar_type,
                        ScalarType::Boolean | ScalarType::Integer(_)
                    )
            }
            _ => false,
        };
        if !result_matches
            || !callee_function.parameters.is_empty()
            || !callee_function.structural_parameters.is_empty()
            || !callee_function.published_service_ceiling.is_empty()
            || callee_dynamic_parameters.len() != source_dynamic.len()
            || source_dynamic
                .iter()
                .enumerate()
                .any(|(ordinal, argument)| {
                    argument.target != *callee_dynamic_parameters[ordinal]
                        || argument.target.ordinal != u32::try_from(ordinal).unwrap_or(u32::MAX)
                        || argument.target.source_position
                            != u32::try_from(ordinal).unwrap_or(u32::MAX)
                        || !argument.has_complete_custody(
                            self.source.machine,
                            psi_operation,
                            source_callee,
                        )
                })
        {
            return Err(psi_operation);
        }
        let pointer_size =
            u16::try_from(self.native_target.pointer_size).map_err(|_| psi_operation)?;
        let pointer_alignment =
            u16::try_from(self.native_target.pointer_alignment).map_err(|_| psi_operation)?;
        let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
        let result_shape =
            expected_result.map(|result| structural_shapes::scalar_shape(result.scalar_type));
        let descriptor_parameter_count =
            source_dynamic.len().checked_mul(2).ok_or(psi_operation)?;
        let expected_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(self.native_target),
            &CallSignature {
                parameters: vec![pointer_shape; descriptor_parameter_count],
                result: result_shape,
            },
        )
        .map_err(|_| psi_operation)?;
        if call_plan != &expected_plan
            || expected_plan.parameters.len() != descriptor_parameter_count
        {
            return Err(psi_operation);
        }
        match (expected_result, actual_result) {
            (None, None) => {}
            (Some(expected), Some((actual, home))) => {
                if *actual != *expected
                    || home.defining_operation != psi_operation
                    || home.source_value != expected.value
                    || home.scalar_type != expected.scalar_type
                    || Some(home.shape) != result_shape
                {
                    return Err(psi_operation);
                }
            }
            _ => return Err(psi_operation),
        }
        for (ordinal, (actual, custody)) in dynamic_arguments.iter().zip(source_dynamic).enumerate()
        {
            self.descriptor_argument(psi_operation, ordinal, actual, custody, &expected_plan)?;
        }
        Ok(())
    }

    /// One `{data, table}` descriptor pair replays its selection's concrete
    /// instance projection through the caller's own parameter placement.
    fn descriptor_argument(
        &self,
        psi_operation: OperationId,
        ordinal: usize,
        actual: &TargetDynamicDescriptorArgument,
        custody: &AbstractDynamicDescriptorArgument,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        if actual.custody != *custody {
            return Err(psi_operation);
        }
        let instance_index = ordinal.checked_mul(2).ok_or(psi_operation)?;
        let (Some(destination), Some(table)) = (
            expected_plan.parameters.get(instance_index),
            expected_plan.parameters.get(instance_index + 1),
        ) else {
            return Err(psi_operation);
        };
        // A parameter pass-through carries no concrete projection: the
        // caller's signature-bound descriptor ABI row is the instance source
        // and both incoming words land on the callee's pair verbatim. The
        // roster row itself was already rebound to this function's entrance
        // plan by signature validation.
        if let AbstractDynamicDescriptorSource::Parameter(parameter) = &custody.source {
            let Some(expected_abi) = self
                .target
                .graph
                .dynamic_parameters
                .iter()
                .find(|abi| abi.parameter == *parameter)
            else {
                return Err(psi_operation);
            };
            return if matches!(
                &actual.instance,
                TargetDynamicDescriptorInstanceSource::Parameter {
                    parameter: actual_parameter,
                    destination: actual_destination,
                } if actual_parameter == expected_abi && actual_destination == destination
            ) && actual.table_destination == *table
            {
                Ok(())
            } else {
                Err(psi_operation)
            };
        }
        let TargetDynamicDescriptorInstanceSource::Projection(instance) = &actual.instance else {
            return Err(psi_operation);
        };
        let selection = match &custody.source {
            AbstractDynamicDescriptorSource::Selection { selection, .. } => selection,
            AbstractDynamicDescriptorSource::Rebound { rebound, .. } => rebound,
            AbstractDynamicDescriptorSource::Parameter(_) => {
                unreachable!("parameter sources return before projection replay")
            }
        };
        let source = &selection.source;
        if custody.target.access != source.access
            || source.path.is_empty()
            || source
                .path
                .iter()
                .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
        {
            return Err(psi_operation);
        }
        let root = self
            .target
            .graph
            .parameters
            .iter()
            .find(|parameter| parameter.place == source.place)
            .ok_or(psi_operation)?;
        let (projected_type, projected_shape, byte_offset) = structural_shapes::projected_field(
            root.structural_type,
            &source.path,
            self.declarations,
        )
        .map_err(|_| psi_operation)?;
        if byte_offset
            .checked_add(u32::from(projected_shape.byte_size))
            .is_none_or(|end| end > u32::from(root.shape.byte_size))
        {
            return Err(psi_operation);
        }
        if instance.place != source.place
            || instance.access != source.access
            || instance.path != source.path
            || instance.root_structural_type != root.structural_type
            || instance.structural_type != projected_type
            || instance.shape != projected_shape
            || instance.source_byte_offset != byte_offset
            || instance.source != root.placement
            || instance.destination != *destination
            || actual.table_destination != *table
        {
            return Err(psi_operation);
        }
        Ok(())
    }

    /// A stored descriptor's retained source argument embeds the unique stored
    /// call's plan placement; the descriptor custody, the unique call, and the
    /// callee's single structural parameter all replay.
    fn stored_descriptor(
        &self,
        psi_operation: OperationId,
        stored: &AbstractStoredDynamicDescriptor,
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::StoredDescriptor {
            stored: actual_stored,
            source_argument,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        if actual_stored != stored
            || !stored.has_complete_custody(self.source.machine, psi_operation)
        {
            return Err(psi_operation);
        }
        let calls = self
            .source
            .operations
            .iter()
            .filter_map(|candidate| match candidate {
                AbstractOperation::CallStoredDynamicScalar {
                    psi_operation,
                    dynamic_dispatch,
                    result,
                    ..
                } if &dynamic_dispatch.stored == stored => {
                    Some((*psi_operation, dynamic_dispatch, result))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(call_operation, dispatch, result)] = calls.as_slice() else {
            return Err(psi_operation);
        };
        let (expected_plan, callee_parameter) = self.stored_call_layout(
            *call_operation,
            psi_operation,
            dispatch,
            result.scalar_type,
            structural_shapes::scalar_shape(result.scalar_type),
        )?;
        let [destination] = expected_plan.parameters.as_slice() else {
            return Err(psi_operation);
        };
        self.projected_call_argument(
            psi_operation,
            source_argument,
            &stored.selection.source,
            callee_parameter,
            destination,
        )
    }

    /// A stored dynamic call replays the descriptor's unique establishment
    /// and the callee realization's single-argument signature.
    fn stored_dynamic_call(
        &self,
        psi_operation: OperationId,
        result: &AbstractResult,
        dynamic_dispatch: &AbstractStoredDynamicDispatch,
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::StoredDynamic {
            dynamic_dispatch: actual_dispatch,
            call_plan,
            result: actual_result,
            result_home,
            source_argument,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        if !matches!(
            result.scalar_type,
            ScalarType::Boolean | ScalarType::Integer(_)
        ) || self.source.attachment.is_none()
            || actual_dispatch != dynamic_dispatch
            || !dynamic_dispatch.has_complete_custody(self.source.machine, psi_operation)
            || *actual_result != *result
            || *actual_obligations != *requirement_obligations
            || *actual_crashes != *crash_continuations
            || self
                .source
                .operations
                .iter()
                .filter(|candidate| {
                    matches!(candidate,
                        AbstractOperation::StoreDynamicDescriptor { stored, .. }
                            if stored == &dynamic_dispatch.stored)
                })
                .count()
                != 1
        {
            return Err(psi_operation);
        }
        let result_shape = structural_shapes::scalar_shape(result.scalar_type);
        let (expected_plan, callee_parameter) = self.stored_call_layout(
            psi_operation,
            psi_operation,
            dynamic_dispatch,
            result.scalar_type,
            result_shape,
        )?;
        if call_plan != &expected_plan {
            return Err(psi_operation);
        }
        let [destination] = expected_plan.parameters.as_slice() else {
            return Err(psi_operation);
        };
        self.projected_call_argument(
            psi_operation,
            source_argument,
            &dynamic_dispatch.stored.selection.source,
            callee_parameter,
            destination,
        )?;
        if result_home.defining_operation != psi_operation
            || result_home.source_value != result.value
            || result_home.scalar_type != result.scalar_type
            || result_home.shape != result_shape
        {
            return Err(psi_operation);
        }
        Ok(())
    }

    /// The shared stored-call projection: dispatch custody joins at the call
    /// operation, the realization callee admits exactly one structural
    /// parameter and the matching scalar result, and the independently derived
    /// plan leaves exactly one destination. Returns the evaluated plan and the
    /// callee's sole structural parameter.
    fn stored_call_layout(
        &self,
        custody_operation: OperationId,
        error_operation: OperationId,
        dynamic_dispatch: &AbstractStoredDynamicDispatch,
        expected_result: ScalarType,
        result_shape: ValueShape,
    ) -> Result<(CallPlan, &StructuralParameterDeclaration), OperationId> {
        if self.source.attachment.is_none()
            || !dynamic_dispatch.has_complete_custody(self.source.machine, custody_operation)
        {
            return Err(error_operation);
        }
        let callee = dynamic_dispatch.dispatch.realization;
        let callee_function = self.callee(callee, error_operation)?;
        let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
            return Err(error_operation);
        };
        if !callee_function.parameters.is_empty()
            || !matches!(
                &callee_function.result,
                AbstractFunctionResult::Scalar(result)
                    if result.scalar_type == expected_result
            )
            || !callee_function.published_service_ceiling.is_empty()
        {
            return Err(error_operation);
        }
        let expected_plan = self.expected_plan(callee_function, error_operation)?;
        let [_] = expected_plan.parameters.as_slice() else {
            return Err(error_operation);
        };
        if expected_plan
            .result
            .as_ref()
            .map(|placement| placement.shape)
            != Some(result_shape)
        {
            return Err(error_operation);
        }
        Ok((expected_plan, callee_parameter))
    }

    /// A rebound dynamic call lowers both the initializer and the latest
    /// source against the selected realization's single-argument signature.
    fn rebound_dynamic_call(
        &self,
        psi_operation: OperationId,
        expected_result: Option<&AbstractResult>,
        dynamic_dispatch: &AbstractReboundDynamicDispatch,
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::ReboundDynamic {
            dynamic_dispatch: actual_dispatch,
            call_plan,
            result: actual_result,
            initial_argument,
            rebound_argument,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        if self.source.attachment.is_none()
            || actual_dispatch != dynamic_dispatch
            || !dynamic_dispatch
                .has_complete_application_custody(self.source.machine, psi_operation)
            || *actual_obligations != *requirement_obligations
            || *actual_crashes != *crash_continuations
        {
            return Err(psi_operation);
        }
        let callee = dynamic_dispatch.dispatch.realization;
        let callee_function = self.callee(callee, psi_operation)?;
        let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
            return Err(psi_operation);
        };
        let result_matches = match (&callee_function.result, expected_result) {
            (AbstractFunctionResult::Unit, None) => true,
            (AbstractFunctionResult::Scalar(declared), Some(result)) => {
                declared.scalar_type == result.scalar_type
                    && matches!(
                        result.scalar_type,
                        ScalarType::Boolean | ScalarType::Integer(_)
                    )
            }
            _ => false,
        };
        if !result_matches
            || !callee_function.parameters.is_empty()
            || !callee_function.published_service_ceiling.is_empty()
        {
            return Err(psi_operation);
        }
        let result_shape =
            expected_result.map(|result| structural_shapes::scalar_shape(result.scalar_type));
        let expected_plan = self.expected_plan(callee_function, psi_operation)?;
        let [destination] = expected_plan.parameters.as_slice() else {
            return Err(psi_operation);
        };
        if call_plan != &expected_plan
            || expected_plan
                .result
                .as_ref()
                .map(|placement| placement.shape)
                != result_shape
        {
            return Err(psi_operation);
        }
        self.projected_call_argument(
            psi_operation,
            initial_argument,
            &dynamic_dispatch.initial.source,
            callee_parameter,
            destination,
        )?;
        self.projected_call_argument(
            psi_operation,
            rebound_argument,
            &dynamic_dispatch.rebound.source,
            callee_parameter,
            destination,
        )?;
        match (expected_result, actual_result) {
            (None, None) => Ok(()),
            (Some(expected), Some((actual, home))) => {
                if *actual == *expected
                    && home.defining_operation == psi_operation
                    && home.source_value == expected.value
                    && home.scalar_type == expected.scalar_type
                    && Some(home.shape) == result_shape
                {
                    Ok(())
                } else {
                    Err(psi_operation)
                }
            }
            _ => Err(psi_operation),
        }
    }

    /// A parameter-dispatch call invokes one requirement slot of the caller's
    /// own descriptor parameter. Every retained row replays independently:
    /// the dispatch custody join, the signature-bound descriptor ABI roster
    /// row, the closed-interface requirement selected by the slot, the erased
    /// one-pointer dispatch plan, the slot's byte offset in the incoming
    /// table, and the result row and home when the requirement returns one.
    /// The retained plan never names a concrete realization.
    fn parameter_dynamic_call(
        &self,
        psi_operation: OperationId,
        expected_result: Option<&AbstractResult>,
        dynamic_dispatch: &AbstractParameterDynamicDispatch,
        requirement_obligations: &[ObligationId],
        crash_continuations: &[CrashRouteBucket],
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(&EmbeddedCall::ParameterDynamic {
            dynamic_dispatch: actual_dispatch,
            parameter_abi,
            requirement: actual_requirement,
            dispatch_call_plan,
            table_slot_byte_offset,
            result: actual_result,
            requirement_obligations: actual_obligations,
            crash_continuations: actual_crashes,
        }) = call
        else {
            return if call.is_none() {
                Ok(())
            } else {
                Err(psi_operation)
            };
        };
        if actual_dispatch != dynamic_dispatch
            || !dynamic_dispatch.has_complete_custody(self.source.machine, psi_operation)
            || *actual_obligations != *requirement_obligations
            || *actual_crashes != *crash_continuations
        {
            return Err(psi_operation);
        }
        // The consumed parameter must be the roster row this function's
        // entrance signature already bound; a substituted descriptor pair
        // cannot satisfy the join.
        let Some(expected_abi) = self
            .target
            .graph
            .dynamic_parameters
            .iter()
            .find(|abi| abi.parameter == dynamic_dispatch.parameter)
        else {
            return Err(psi_operation);
        };
        if parameter_abi != expected_abi {
            return Err(psi_operation);
        }
        let Some(requirement) = dynamic_dispatch
            .parameter
            .requirements
            .iter()
            .find(|requirement| requirement.slot == dynamic_dispatch.dispatch.requirement_slot)
        else {
            return Err(psi_operation);
        };
        if actual_requirement != requirement {
            return Err(psi_operation);
        }
        let expected_scalar = match expected_result {
            Some(result)
                if matches!(
                    result.scalar_type,
                    ScalarType::Boolean | ScalarType::Integer(_)
                ) =>
            {
                Some(result.scalar_type)
            }
            Some(_) => return Err(psi_operation),
            None => None,
        };
        let closed_scalar = match requirement.result {
            ClosedConformanceCallableResult::Unit => None,
            ClosedConformanceCallableResult::I32 => Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).expect("closed i32 result is valid"),
            )),
            ClosedConformanceCallableResult::Bool => Some(ScalarType::Boolean),
        };
        if closed_scalar != expected_scalar {
            return Err(psi_operation);
        }
        let result_shape =
            expected_result.map(|result| structural_shapes::scalar_shape(result.scalar_type));
        let pointer_size =
            u16::try_from(self.native_target.pointer_size).map_err(|_| psi_operation)?;
        let pointer_alignment =
            u16::try_from(self.native_target.pointer_alignment).map_err(|_| psi_operation)?;
        let expected_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(self.native_target),
            &CallSignature {
                parameters: vec![ValueShape::integer(pointer_size, pointer_alignment)],
                result: result_shape,
            },
        )
        .map_err(|_| psi_operation)?;
        if dispatch_call_plan != &expected_plan
            || expected_plan.parameters.len() != 1
            || table_slot_byte_offset
                != dynamic_dispatch
                    .dispatch
                    .requirement_slot
                    .checked_mul(u32::from(pointer_size))
                    .ok_or(psi_operation)?
        {
            return Err(psi_operation);
        }
        match (expected_result, actual_result) {
            (None, None) => Ok(()),
            (Some(expected), Some((actual, home))) => {
                if *actual == *expected
                    && home.defining_operation == psi_operation
                    && home.source_value == expected.value
                    && home.scalar_type == expected.scalar_type
                    && Some(home.shape) == result_shape
                {
                    Ok(())
                } else {
                    Err(psi_operation)
                }
            }
            _ => Err(psi_operation),
        }
    }

    /// The producer's `projected_argument::lower` replay: the argument must be
    /// a nonempty field-only path into one caller structural parameter whose
    /// projected carrier type, access-adjusted shape, and byte offset land on
    /// the plan's canonical destination.
    fn projected_call_argument(
        &self,
        psi_operation: OperationId,
        actual: &TargetStructuralArgument,
        source: &StructuralArgument,
        callee_parameter: &StructuralParameterDeclaration,
        destination: &ValuePlacement,
    ) -> Result<(), OperationId> {
        let root = self
            .target
            .graph
            .parameters
            .iter()
            .find(|parameter| parameter.place == source.place)
            .ok_or(psi_operation)?;
        if source.path.is_empty()
            || source
                .path
                .iter()
                .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
        {
            return Err(psi_operation);
        }
        let (projected_type, projected_shape, byte_offset) = structural_shapes::projected_field(
            root.structural_type,
            &source.path,
            self.declarations,
        )
        .map_err(|_| psi_operation)?;
        let shape = structural_shapes::parameter_shape(projected_shape, callee_parameter.access);
        if projected_type != callee_parameter.structural_type
            || source.access != callee_parameter.access
            || shape != destination.shape
            || u32::from(projected_shape.byte_size)
                .checked_add(byte_offset)
                .is_none_or(|end| end > u32::from(root.shape.byte_size))
        {
            return Err(psi_operation);
        }
        if actual.place != source.place
            || actual.access != source.access
            || actual.path != source.path
            || actual.root_structural_type != root.structural_type
            || actual.structural_type != projected_type
            || actual.shape != shape
            || actual.source_byte_offset != byte_offset
            || actual.fixed_array_length.is_some()
            || actual.element_stride.is_some()
            || actual.source != TargetStructuralArgumentSource::Placement(root.placement.clone())
            || actual.destination != *destination
        {
            return Err(psi_operation);
        }
        Ok(())
    }

    /// Ordered scalar argument rows bind the plan's anonymous parameter
    /// placements back to the declared value and type identities.
    fn scalar_arguments(
        &self,
        psi_operation: OperationId,
        actual: &[TargetUnitScalarCallArgument],
        values: &[ValueId],
        declared: &[AbstractParameter],
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        for (position, ((actual, value), declared)) in
            actual.iter().zip(values).zip(declared).enumerate()
        {
            let Ok(index) = u32::try_from(position) else {
                return Err(psi_operation);
            };
            if actual.parameter_index != index
                || actual.source.source_value() != *value
                || actual.scalar_type() != declared.scalar_type
                || actual.placement != expected_plan.parameters[position]
            {
                return Err(psi_operation);
            }
        }
        Ok(())
    }

    /// One retained structural call argument replays the declared parameter
    /// identity, the independently reconstructed referent shape, and the
    /// canonical destination placement.
    fn structural_argument(
        &self,
        psi_operation: OperationId,
        plan_index: usize,
        actual: &TargetStructuralArgument,
        semantic: &StructuralArgument,
        declared: &StructuralParameterDeclaration,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        // A `.., Referent` argument transports the referent root's
        // pointer: `place` names the root whose canonical storage
        // `source` must identify (replayed by argument-source custody),
        // while `path` retains the carrier-relative custody projection
        // verbatim. The carrier place itself is loan custody, not a
        // pointer source.
        if matches!(semantic.path.last(), Some(StructuralPathSegment::Referent)) {
            if !matches_referent_argument(actual, semantic, declared, self.declarations) {
                return Err(psi_operation);
            }
        } else if !matches_argument_identity(actual, semantic, declared) {
            return Err(psi_operation);
        }
        let Some(referent) =
            structural_shapes::reconstruct(actual.structural_type, self.declarations).ok()
        else {
            return Err(psi_operation);
        };
        if actual.shape != structural_shapes::parameter_shape(referent, actual.access) {
            return Err(psi_operation);
        }
        let Some(destination) = expected_plan.parameters.get(plan_index) else {
            return Err(psi_operation);
        };
        if actual.destination != *destination {
            return Err(psi_operation);
        }
        let Some(root) = self.roots.get(&semantic.place) else {
            return Ok(());
        };
        // Static subloans carry a pointer to the reconstructed leaf, not
        // an array-view descriptor. Indexed paths need the same carrier
        // and offset replay as fields; owned indexed copies retain their
        // separate array transport metadata.
        let static_borrow = root.access != StructuralAccess::Owned
            && semantic.access != StructuralAccess::Owned
            && semantic.path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            });
        if static_borrow
            || semantic
                .path
                .iter()
                .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        {
            if actual.root_structural_type != root.structural_type {
                return Err(psi_operation);
            }
            match structural_shapes::project_static_path(
                root.structural_type,
                &semantic.path,
                self.declarations,
            ) {
                Ok((projected_type, byte_offset)) => {
                    if !matches_projected_carrier(actual, projected_type, self.declarations)
                        || actual.source_byte_offset != byte_offset
                    {
                        return Err(psi_operation);
                    }
                }
                Err(_) => {
                    if !matches_bounded_byte_field(actual, root, self.declarations) {
                        return Err(psi_operation);
                    }
                }
            }
        } else if semantic.path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            // An indexed projection outside the borrowed-subloan arm —
            // an owned root or owned transport — still replays its root
            // type, projected carrier, and byte offset from the referent's
            // own declaration, and the retained array extent and element
            // stride must equal the root's reconstructed transport
            // metadata rather than any substituted pair.
            let (expected_length, expected_stride) =
                structural_shapes::root_array_transport(root.structural_type, self.declarations)
                    .map_err(|_| psi_operation)?;
            let Ok((projected_type, byte_offset)) = structural_shapes::project_static_path(
                root.structural_type,
                &semantic.path,
                self.declarations,
            ) else {
                return Err(psi_operation);
            };
            if actual.root_structural_type != root.structural_type
                || actual.structural_type != projected_type
                || actual.source_byte_offset != byte_offset
                || actual.fixed_array_length != expected_length
                || actual.element_stride != expected_stride
            {
                return Err(psi_operation);
            }
        }
        Ok(())
    }

    /// The retained result row must be the source-declared result verbatim,
    /// the declared callee result kind, and the independently reconstructed
    /// durable home. A physically similar row is never authority.
    fn bind_result(
        &self,
        psi_operation: OperationId,
        expected: BoundResult<'_>,
        actual: &EmbeddedResult<'_>,
        callee: &AbstractFunction,
    ) -> Result<(), OperationId> {
        match (expected, actual) {
            (BoundResult::Unit, EmbeddedResult::Unit) => {
                if callee.result != AbstractFunctionResult::Unit {
                    return Err(psi_operation);
                }
                Ok(())
            }
            (BoundResult::Scalar(expected), EmbeddedResult::Scalar(actual)) => {
                let declared = callee.result.scalar().ok_or(psi_operation)?;
                if actual.defining_operation != psi_operation
                    || actual.source_value != expected.value
                    || actual.scalar_type != expected.scalar_type
                    || structural_signatures::fixed_native_scalar_shape(expected.scalar_type)
                        != Some(actual.shape)
                    || actual.scalar_type != declared.scalar_type
                    || !callee.entry_claims.is_empty()
                    || !callee.published_service_ceiling.is_empty()
                {
                    return Err(psi_operation);
                }
                Ok(())
            }
            (
                BoundResult::Structural(expected),
                EmbeddedResult::Structural {
                    result,
                    callee_result,
                    result_home,
                    reference_results,
                },
            ) => {
                let declared = callee.result.structural().ok_or(psi_operation)?;
                if **result != *expected
                    || *callee_result != declared
                    || declared.structural_type != expected.structural_type
                    || declared.multiplicity != expected.multiplicity
                    || !declared.qualifications.is_empty()
                    || !declared.projected_qualifications.is_empty()
                    || !callee.entry_claims.is_empty()
                    // The retained leaf roster must equal the roster the
                    // caller's own custody replay derives; a forged path,
                    // root, or count cannot survive the comparison.
                    || self
                        .expected_reference_results
                        .get(&psi_operation)
                        .is_none_or(|rows| *reference_results != rows.as_slice())
                {
                    return Err(psi_operation);
                }
                // A bare reference result is custody only: the producer
                // establishes no physical result home for a `Reference`-shaped
                // carrier, so the honest row retains `None` and the call's
                // `reference_results` rows carry the leaf custody. Aggregate
                // and sum results still replay the exact durable home.
                let reference_only = self.declarations.iter().any(|declaration| {
                    declaration.id == expected.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::Reference { .. }
                        )
                });
                if reference_only {
                    if result_home.is_some() {
                        return Err(psi_operation);
                    }
                } else {
                    let expected_home = structural_shapes::structural_result_home(
                        psi_operation,
                        expected,
                        self.declarations,
                    )
                    .map_err(|_| psi_operation)?;
                    if *result_home != Some(&expected_home) {
                        return Err(psi_operation);
                    }
                }
                Ok(())
            }
            _ => Err(psi_operation),
        }
    }
}

/// A fixed byte array lends its original backing through a view descriptor.
/// The projected storage type therefore differs from the callee's view type;
/// reconstruct the adaptation instead of requiring identity or trusting the
/// producer's length and stride. Ordinary pointer arguments have no adaptation.
fn matches_projected_carrier(
    actual: &TargetStructuralArgument,
    projected: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};
    if actual.structural_type == projected {
        return actual.fixed_array_length.is_none() && actual.element_stride.is_none();
    }
    let find_shape = |identity| {
        declarations
            .iter()
            .find(|declaration| declaration.id == identity)
            .map(|declaration| &declaration.shape)
    };
    let Some(StructuralTypeShape::FixedArray { element, length }) = find_shape(projected) else {
        return false;
    };
    actual.access == StructuralAccess::MutableBorrow
        && *length > 0
        && matches!(
            find_shape(*element),
            Some(StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(integer)
            )) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                && integer.bits() == 8 && !integer.is_address()
        )
        && matches!(
            find_shape(actual.structural_type),
            Some(StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView
            ))
        )
        && actual.fixed_array_length == Some(*length)
        && actual.element_stride == Some(1)
}

/// A bounded inline byte field has no projected carrier identity of its own.
/// The argument still names the field's storage: its live length word and bytes
/// stay in place and the callee sees only the borrowed view. Reconstruct the
/// field's offset and capacity rather than trusting a substituted type or a
/// static-length descriptor's metadata.
fn matches_bounded_byte_field(
    actual: &TargetStructuralArgument,
    root: &RootDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Ok((field_offset, _capacity)) = structural_shapes::bounded_byte_field_geometry(
        root.structural_type,
        &actual.path,
        declarations,
    ) else {
        return false;
    };
    matches!(
        actual.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && matches!(
        (root.access, actual.access),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
        ) | (
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow
        )
    ) && declarations.iter().any(|declaration| {
        declaration.id == actual.structural_type
            && declaration.shape
                == terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                )
    }) && actual.source_byte_offset == field_offset
        && actual.fixed_array_length.is_none()
        && actual.element_stride.is_none()
}

fn matches_argument_identity(
    actual: &TargetStructuralArgument,
    semantic: &StructuralArgument,
    declared: &StructuralParameterDeclaration,
) -> bool {
    actual.place == semantic.place
        && actual.path == semantic.path
        && actual.access == semantic.access
        && actual.access == declared.access
        && actual.structural_type == declared.structural_type
}

/// A `.., Referent` argument carries the referent root's pointer, so the
/// compositional `place` + `path` reading does not apply: `place` names the
/// root place whose canonical storage `source` must identify (replayed by
/// `structural_argument_sources`), `path` retains the carrier-relative custody
/// projection verbatim, and the declared parameter must be a primitive
/// borrowed reference. No projection offset or array descriptor is
/// transported.
fn matches_referent_argument(
    actual: &TargetStructuralArgument,
    semantic: &StructuralArgument,
    declared: &StructuralParameterDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Some((StructuralPathSegment::Referent, carrier_path)) = semantic.path.split_last() else {
        return false;
    };
    carrier_path
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        && semantic.access != StructuralAccess::Owned
        && !declared.is_self
        && declared.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && declared.access != StructuralAccess::Owned
        && declared.qualifications.is_empty()
        && declared.projected_qualifications.is_empty()
        && matches!(
            declarations
                .iter()
                .find(|declaration| declaration.id == declared.structural_type)
                .map(|declaration| &declaration.shape),
            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
        )
        && actual.access == semantic.access
        && actual.access == declared.access
        && actual.path == semantic.path
        && actual.structural_type == declared.structural_type
        && actual.root_structural_type == declared.structural_type
        && actual.source_byte_offset == 0
        && actual.fixed_array_length.is_none()
        && actual.element_stride.is_none()
}

/// A `HostedExitProcessI32` settlement never returns: the source boundary
/// call is the last non-terminator operation of its block and the block
/// closes on a `ReturnUnit` with no cleanup — the shape the producer's
/// nonreturning guard requires.
fn hosted_exit_source_tail(source: &AbstractFunction, position: usize) -> bool {
    let mut block_end = None;
    for (index, entry) in source.block_entries.iter().enumerate() {
        let end = source
            .block_entries
            .get(index + 1)
            .map_or(source.operations.len(), |next| next.operation_offset);
        if entry.operation_offset <= position && position < end {
            block_end = Some(end);
            break;
        }
    }
    let Some(end) = block_end else {
        return false;
    };
    position + 2 == end
        && matches!(
            source.operations.get(end - 1),
            Some(AbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            }) if cleanup_actions.is_empty()
        )
}

/// The integer-result operations whose durable scalar home enters the
/// producer's known-integer map: scalar definitions, structural scalar
/// reads, and the scalar-result call family. Constants bind `Immediate`
/// sources and parameters bind parameter sources, so a `Home` citation can
/// only name one of these producing operations.
fn integer_home_result(operation: &AbstractOperation) -> Option<(OperationId, AbstractResult)> {
    let integer = |operation: OperationId, result: ValueId, scalar_type: IntegerType| {
        (
            operation,
            AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(scalar_type),
            },
        )
    };
    Some(match operation {
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralCaseMembership {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructuralScalar {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructuralScalarWithDynamicArguments {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStoredDynamicScalar {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallDynamicScalar {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallDynamicParameterScalar {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
        ),
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: AbstractBoundaryResult::Scalar(result),
            ..
        } => (*psi_operation, *result),
        AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        }
        | AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            target_type,
            ..
        } => integer(*psi_operation, *result, *target_type),
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        }
        | AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        } => integer(*psi_operation, *result, *value_type),
        AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            ..
        } => integer(*psi_operation, *result, *scalar_type),
        _ => return None,
    })
}

/// Reconstruct each argument place's referent declaration in the same
/// precedence `structural_argument_sources::expected_sources` binds homes:
/// operation-established places, then non-entry block parameters, then
/// caller parameters. Every projected argument must replay its root type and
/// byte offset against the referent's own declaration, not only when that
/// referent happens to arrive as a machine parameter.
fn canonical_roots(source: &AbstractFunction) -> BTreeMap<PlaceId, RootDeclaration> {
    let mut roots = BTreeMap::new();
    for operation in &source.operations {
        let (place, structural_type) = match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishScalarArray { result, .. }
            | AbstractOperation::EstablishScalarCase { result, .. }
            | AbstractOperation::ByteSequenceSubslice { result, .. }
            | AbstractOperation::CallStructural { result, .. } => {
                (result.place, result.structural_type)
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type,
                ..
            } => (place.id, structural_type.id),
            AbstractOperation::BoundaryCall {
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => (result.place, result.structural_type),
            _ => continue,
        };
        roots.entry(place).or_insert(RootDeclaration {
            structural_type,
            access: StructuralAccess::Owned,
        });
    }
    for entry in &source.block_entries {
        if entry.block == source.entry {
            continue;
        }
        for parameter in &entry.structural_parameters {
            roots.entry(parameter.place).or_insert(RootDeclaration {
                structural_type: parameter.structural_type,
                access: parameter.access,
            });
        }
    }
    for parameter in &source.structural_parameters {
        roots.entry(parameter.place).or_insert(RootDeclaration {
            structural_type: parameter.structural_type,
            access: parameter.access,
        });
    }
    roots
}
