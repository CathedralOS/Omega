//! Reconcile retained embedded calls against source operations and callee identity.
//!
//! Every retained row in the in-module and installed-provider call families is
//! collected and replayed against the callee's independently reconstructed
//! signature: the embedded plan, scalar and structural argument rows, result
//! identity, and result home are all re-derived from the abstract
//! declarations. The retained target plan, its
//! self-reported placements, and any producer-side digest are never admission
//! authority. A retained row bound to no source call operation, or a second
//! row under an operation key that already has one, is forged by construction.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget, AbstractFunction,
    AbstractFunctionResult, AbstractOperation, AbstractResult, AbstractStructuralCallResult,
    CompletionClaimSource,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, NativePlace, ValueClass, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    BoundaryMachineId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, CompilerBuiltinExecution, NativeCallOrigin, NativeCallTarget,
    NormalizedForeignCallBinding, ProviderExecutionBinding, ScalarAbiValue, ScalarFunctionAbi,
    TargetBoundaryResult, TargetFunction, TargetNativeCallbackArgument, TargetReferenceResult,
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
    TargetUnitOperation, TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument,
    TargetUnitScalarHomeRequirement,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CrashRouteBucket, StructuralAccess,
    StructuralArgument, StructuralFieldType, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalDynamicDispatchCatalog,
};

mod canonical_roots;
mod result_projections;
mod retained_matches;

use super::reference_results;
use super::structural_shapes;
use super::structural_signatures;
use canonical_roots::{RootDeclaration, canonical_roots};
use result_projections::*;
use retained_matches::*;

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
        target: NativeCallTarget,
        call_plan: &'a CallPlan,
        scalar_arguments: &'a [TargetUnitScalarCallArgument],
        arguments: &'a [TargetStructuralArgument],
        result: EmbeddedResult<'a>,
        claim_transfers: &'a [ClaimTransfer],
        returned_claim_transfers: &'a [StructuralResultClaimTransfer],
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
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
        result: &'a TargetBoundaryResult,
    },
    /// `BoundarySettlement`: a compiler-builtin settlement retains its closed
    /// realization, admitted execution custody, verbatim semantic rosters,
    /// and the runtime scalar lane its hosted realization consumes.
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
    },
}

/// The result contract one source operation binds the embedded call to.
enum BoundResult<'a> {
    Unit,
    Scalar(&'a AbstractResult),
    /// The scalar-only `Call`: a scalar result whose callee must also publish
    /// a standalone scalar ABI equal to the call's plan.
    PublishedScalar(AbstractResult),
    Structural(&'a StructuralOperationResult),
}

/// The source side of one authored direct call: a `CallStructural`, or the
/// scalar-only `Call` with empty structural and claim rosters.
struct SourceDirectCall<'a> {
    target: AbstractCallTarget,
    scalar_arguments: &'a [ValueId],
    structural_arguments: &'a [StructuralArgument],
    claim_transfers: &'a [ClaimTransfer],
    returned_claim_transfers: &'a [StructuralResultClaimTransfer],
    requirement_obligations: &'a [ObligationId],
    crash_continuations: &'a [CrashRouteBucket],
    result: BoundResult<'a>,
}

/// Everything one replay needs that stays constant across the plan.
struct Replay<'a> {
    source: &'a AbstractFunction,
    source_functions: &'a [AbstractFunction],
    dynamic_dispatch: &'a TerminalDynamicDispatchCatalog,
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
pub(super) fn validate(
    source: &AbstractFunction,
    source_functions: &[AbstractFunction],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
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
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let call = match operation {
                TargetUnitOperation::Call {
                    origin,
                    psi_operation,
                    result,
                    target,
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
                            target: *target,
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
                TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    boundary,
                    provider_execution,
                    binding,
                    scalar_arguments,
                    structural_arguments,
                    result,
                } => (
                    *psi_operation,
                    EmbeddedCall::NormalizedForeign {
                        boundary: *boundary,
                        provider_execution,
                        binding,
                        scalar_arguments,
                        structural_arguments,
                        result,
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
                AbstractOperation::CallStructural { psi_operation, .. }
                | AbstractOperation::BoundaryCall { psi_operation, .. }
                | AbstractOperation::Call { psi_operation, .. } => psi_operation,
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
        dynamic_dispatch,
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
            boundary_machines,
            &boundary_call_rows,
        )
        .unwrap_or_default(),
    };

    for operation in &source.operations {
        match operation {
            AbstractOperation::CallStructural {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                result,
            } => {
                let (result, returned_claim_transfers) = match result {
                    AbstractStructuralCallResult::Unit => (BoundResult::Unit, &[][..]),
                    AbstractStructuralCallResult::Scalar(result) => {
                        (BoundResult::Scalar(result), &[][..])
                    }
                    AbstractStructuralCallResult::Structural {
                        result,
                        returned_claim_transfers,
                        selected_evidence,
                    } => {
                        if !selected_evidence.is_empty() {
                            return Err(*psi_operation);
                        }
                        (
                            BoundResult::Structural(result),
                            returned_claim_transfers.as_slice(),
                        )
                    }
                };
                replay.direct_call(
                    *psi_operation,
                    SourceDirectCall {
                        target: *callee,
                        scalar_arguments: arguments,
                        structural_arguments,
                        claim_transfers,
                        returned_claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                        result,
                    },
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
            } => replay.direct_call(
                *psi_operation,
                SourceDirectCall {
                    target: AbstractCallTarget::Machine(*callee),
                    scalar_arguments: arguments,
                    structural_arguments: &[],
                    claim_transfers: &[],
                    returned_claim_transfers: &[],
                    requirement_obligations,
                    crash_continuations,
                    result: BoundResult::PublishedScalar(AbstractResult {
                        value: *result,
                        scalar_type: *scalar_type,
                    }),
                },
                target_calls.get(psi_operation),
            )?,
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
        callee: AbstractCallSignature<'_>,
        operation: OperationId,
    ) -> Result<CallPlan, OperationId> {
        let signature =
            structural_signatures::call_signature(callee, self.declarations).ok_or(operation)?;
        evaluate_call_plan(
            CallingPolicy::native_for_target(self.native_target),
            &signature,
        )
        .map_err(|_| operation)
    }

    /// One authored direct call replays against its source operation: the
    /// retained row must carry the authored origin and the source rosters,
    /// then shares the row replay with installed-provider calls.
    fn direct_call(
        &self,
        psi_operation: OperationId,
        source: SourceDirectCall<'_>,
        call: Option<&EmbeddedCall<'_>>,
    ) -> Result<(), OperationId> {
        let Some(call) = call else {
            return Ok(());
        };
        let EmbeddedCall::Direct {
            origin,
            target,
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
        // The source and target retain identical claim transfers; physical
        // transport cannot erase or substitute semantic ownership. An authored
        // source never retains an installed-provider origin. Requirement obligations
        // are discharged proof metadata: the retained row must replay the source roster
        // exactly rather than drop it. Crash continuations are the call's
        // verified surviving-route roster: they ride the same correspondence,
        // so the retained row must replay them exactly. A retained call carrying extra rows or the wrong
        // origin is forged even when plausible.
        if *origin != &NativeCallOrigin::Authored
            || source.claim_transfers != *actual_claims
            || source.returned_claim_transfers != *actual_returned
            || *actual_crashes != source.crash_continuations
            || source.requirement_obligations != *actual_obligations
            || !call_target_matches(source.target, *target)
        {
            return Err(psi_operation);
        }
        self.direct_row(
            psi_operation,
            source.target,
            source.scalar_arguments,
            source.structural_arguments,
            source.result,
            call_plan,
            actual_scalar,
            actual_structural,
            actual_result,
        )
    }

    /// Authored and installed direct rows share one roster shape once their
    /// origin is bound: the callee's independently derived plan, the ordered
    /// scalar and structural argument rows, and the result tail of the source
    /// result kind. Every producer lane admits only fixed-native scalar
    /// parameters on the callee; a wider roster cannot produce this row.
    #[allow(clippy::too_many_arguments)]
    fn direct_row(
        &self,
        psi_operation: OperationId,
        target: AbstractCallTarget,
        scalar_arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
        expected_result: BoundResult<'_>,
        call_plan: &CallPlan,
        actual_scalar: &[TargetUnitScalarCallArgument],
        actual_structural: &[TargetStructuralArgument],
        actual_result: &EmbeddedResult<'_>,
    ) -> Result<(), OperationId> {
        let declaration = match target {
            AbstractCallTarget::Machine(machine) => {
                AbstractCallSignature::Machine(self.callee(machine, psi_operation)?)
            }
            AbstractCallTarget::Dynamic {
                selector,
                interface,
                slot,
            } => {
                let requirement = self
                    .dynamic_dispatch
                    .interface(interface)
                    .and_then(|interface| interface.slots.get(slot as usize))
                    .filter(|requirement| requirement.slot == slot)
                    .ok_or(psi_operation)?;
                // The selector is an ordinary scalar value, but not an ABI
                // argument. Exact source identity and type bind it here; the
                // executable graph replay owns CFG dominance as for arguments.
                let scalar_type = target_operations::selector_carrier_type();
                let declared = self.source.parameters.iter().any(|parameter| {
                    parameter.value == selector && parameter.scalar_type == scalar_type
                }) || self.source.block_entries.iter().any(|entry| {
                    entry.parameters.iter().any(|parameter| {
                        parameter.value == selector && parameter.scalar_type == scalar_type
                    })
                }) || self.source.operations.iter().any(|operation| {
                    scalar_home_result(operation).is_some_and(|(_, result)| {
                        result.value == selector && result.scalar_type == scalar_type
                    })
                });
                if !declared {
                    return Err(psi_operation);
                }
                AbstractCallSignature::Requirement(requirement)
            }
        };
        if declaration.structural_parameter_count() != actual_structural.len()
            || declaration.scalar_parameter_count() != scalar_arguments.len()
            || actual_structural.len() != structural_arguments.len()
            || actual_scalar.len() != scalar_arguments.len()
            || (0..declaration.scalar_parameter_count()).any(|position| {
                declaration
                    .scalar_parameter(position)
                    .and_then(structural_signatures::fixed_native_scalar_shape)
                    .is_none()
            })
        {
            return Err(psi_operation);
        }
        let expected_plan = self.expected_plan(declaration, psi_operation)?;
        if *call_plan != expected_plan {
            return Err(psi_operation);
        }
        self.scalar_arguments(
            psi_operation,
            actual_scalar,
            scalar_arguments,
            declaration,
            &expected_plan,
        )?;
        for (index, (actual, semantic)) in actual_structural
            .iter()
            .zip(structural_arguments)
            .enumerate()
        {
            let formal = declaration
                .structural_parameter(index)
                .ok_or(psi_operation)?;
            // This projection carries permissions and types only. Its place is
            // the authored argument's place; no callee storage is invented.
            let declared = StructuralParameterDeclaration {
                place: semantic.place,
                position: formal.position,
                structural_type: formal.structural_type,
                multiplicity: formal.multiplicity,
                access: formal.access,
                qualifications: formal.qualifications.to_vec(),
                projected_qualifications: formal.projected_qualifications.to_vec(),
            };
            self.structural_argument(
                psi_operation,
                index + scalar_arguments.len(),
                actual,
                semantic,
                &declared,
                &expected_plan,
            )?;
        }
        self.bind_result(
            psi_operation,
            expected_result,
            actual_result,
            declaration,
            &expected_plan,
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
            target,
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
            || *target != NativeCallTarget::Machine(provider.candidate)
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
            // An installed scalar provider is an ordinary scalar call; its
            // result replays exactly as any scalar call's does.
            AbstractBoundaryResult::Scalar(result) => BoundResult::Scalar(result),
            AbstractBoundaryResult::Structural(result) => BoundResult::Structural(result),
        };
        self.direct_row(
            psi_operation,
            AbstractCallTarget::Machine(provider.candidate),
            arguments,
            structural_arguments,
            expected_result,
            call_plan,
            actual_scalar,
            actual_structural,
            actual_result,
        )
    }

    /// An evaluated normalized foreign call embeds the boundary's own entry
    /// plan and projects borrowed arguments through it: record fields, fixed
    /// array elements, or a caller parameter's whole forwarded loan. The
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
            result: actual_result,
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
        // and the mechanism's retained custody must name that declaration's
        // requirement and the exact provider plan behind the execution binding.
        // Imports carry that join in their admitted stack contribution; kernel
        // entries carry it without inventing a user-space stack contribution.
        let mut declarations = self
            .boundary_machines
            .iter()
            .filter(|row| row.id == boundary);
        let declaration = declarations.next().ok_or(psi_operation)?;
        if declarations.next().is_some()
            || binding.native_target() != Some(self.native_target)
            || binding.requirement_identity() != declaration.identity
            || binding.provider_plan_report_identity()
                != provider_execution.provider_plan_report_identity().get()
            || !binding.has_valid_entry_mechanism()
        {
            return Err(psi_operation);
        }
        // A borrowed formal transports its referent's pointer word — except a
        // `ByteSequence(BorrowedView)` formal, whose stored two-word view
        // crosses by value — and an owned formal carries the aggregate's
        // policy-classified by-value shape. The signature is rebuilt from the
        // declarations, never from the retained rows. A retained callback
        // occupies one pointer slot.
        let pointer_size =
            u16::try_from(self.native_target.pointer_size).map_err(|_| psi_operation)?;
        let pointer_alignment =
            u16::try_from(self.native_target.pointer_alignment).map_err(|_| psi_operation)?;
        let pointer_shape = ValueShape::integer(u32::from(pointer_size), pointer_alignment);
        // A retained callback joins the plan's `callback_materializations` row
        // it names by `placement_index`; the roster entry joins by Terminal
        // operation and exact registrar plan. A parameter destination occupies
        // one native-only parameter slot while a field destination reuses its
        // root record parameter's slot. A materialized plan without roster
        // rows, a count mismatch, a duplicated or out-of-range placement
        // index, a substituted registrar plan, a swapped destination row, and
        // a claimed application placement the plan does not carry all fail
        // closed.
        let mut callbacks: Vec<&TargetNativeCallbackArgument> = self
            .native_callbacks
            .iter()
            .filter(|callback| callback.terminal_operation == psi_operation)
            .collect();
        callbacks.sort_by_key(|callback| callback.placement_index);
        let callback_materializations = &binding.boundary_entry_plan.call.callback_materializations;
        if callbacks.is_empty() {
            if !callback_materializations.is_empty() {
                return Err(psi_operation);
            }
        } else if callbacks.len() != callback_materializations.len() {
            return Err(psi_operation);
        }
        for (index, callback) in callbacks.iter().enumerate() {
            let ordinal =
                usize::try_from(callback.application.native_ordinal).map_err(|_| psi_operation)?;
            let Some(materialization) = callback_materializations.get(callback.placement_index)
            else {
                return Err(psi_operation);
            };
            let expects_slot = matches!(callback.destination, NativePlace::Parameter(_));
            if index != callback.placement_index
                || materialization.destination != callback.destination
                || expects_slot != callback.field_byte_offset.is_none()
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
        }
        let parameter_slot_callbacks = callbacks
            .iter()
            .filter(|callback| matches!(callback.destination, NativePlace::Parameter(_)))
            .count();
        let registrar_context = callbacks
            .first()
            .map(|callback| callback.registrar_context.clone());
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
                structural_signatures::fixed_native_scalar_shape(*parameter).ok_or(psi_operation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (expected_result, result_shape) = match (result, &declaration.result) {
            (AbstractBoundaryResult::Unit, terminal_psi::BoundaryMachineResult::Unit) => {
                (TargetBoundaryResult::Unit, None)
            }
            (
                AbstractBoundaryResult::Scalar(result),
                terminal_psi::BoundaryMachineResult::Scalar(declared),
            ) => {
                // The declared result must be the same fixed-native scalar the
                // abstract result names. Its home is the ordinary SSA value the
                // caller's body defines, so a receiverless static body retains
                // it exactly as an attached one does.
                if *declared != result.scalar_type {
                    return Err(psi_operation);
                }
                let shape = structural_signatures::fixed_native_scalar_shape(*declared)
                    .ok_or(psi_operation)?;
                (
                    TargetBoundaryResult::Scalar(TargetUnitScalarHomeRequirement {
                        defining_operation: psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape,
                    }),
                    Some(shape),
                )
            }
            (
                AbstractBoundaryResult::Structural(result),
                terminal_psi::BoundaryMachineResult::Structural(declared),
            ) => {
                if result.structural_type != declared.structural_type
                    || result.multiplicity != declared.multiplicity
                    || result.qualifications != declared.qualifications
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                {
                    return Err(psi_operation);
                }
                let home = structural_shapes::structural_result_home(
                    psi_operation,
                    result,
                    self.declarations,
                )
                .map_err(|_| psi_operation)?;
                let shape = structural_shapes::classified_boundary_shape(
                    declared.structural_type,
                    self.declarations,
                    binding.boundary_entry_plan.call.policy,
                )
                .map_err(|_| psi_operation)?;
                (TargetBoundaryResult::Structural(home), Some(shape))
            }
            _ => return Err(psi_operation),
        };
        // The materialized signature spells every authored and private
        // placement, while the declaration still counts only semantic
        // formals — semantic argument indices shift around the callback's
        // native ordinal.
        let mut next_scalar_shape = scalar_shapes.iter();
        let mut next_structural_formal = declaration.structural_parameters.iter();
        let mut parameter_shapes = declaration
            .parameter_order
            .iter()
            .map(|kind| match kind {
                terminal_psi::BoundaryParameterKind::Scalar => {
                    next_scalar_shape.next().copied().ok_or(psi_operation)
                }
                terminal_psi::BoundaryParameterKind::Structural => {
                    let formal = next_structural_formal.next().ok_or(psi_operation)?;
                    structural_shapes::boundary_formal_shape(
                        formal.structural_type,
                        formal.access,
                        self.declarations,
                        binding.boundary_entry_plan.call.policy,
                        pointer_shape,
                    )
                    .map_err(|_| psi_operation)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Only a parameter destination contributes a slot: a field
        // destination's retained application names the root record
        // parameter's own placement and rides its location.
        let mut parameter_ordinals: Vec<usize> = Vec::with_capacity(parameter_slot_callbacks);
        for callback in &callbacks {
            if matches!(callback.destination, NativePlace::Parameter(_)) {
                parameter_ordinals.push(
                    usize::try_from(callback.application.native_ordinal)
                        .map_err(|_| psi_operation)?,
                );
            }
        }
        parameter_ordinals.sort_unstable();
        for &ordinal in &parameter_ordinals {
            if ordinal > parameter_shapes.len() {
                return Err(psi_operation);
            }
            parameter_shapes.insert(ordinal, pointer_shape);
        }
        let signature = CallSignature {
            parameters: parameter_shapes,
            result: result_shape,
        };
        let validated = match registrar_context {
            Some(registrar_context) => {
                calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                    binding.boundary_entry_plan.clone(),
                    &signature,
                    &registrar_context,
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
                != scalar_shapes.len() + structural_arguments.len() + parameter_slot_callbacks
            || &expected_result != *actual_result
        {
            return Err(psi_operation);
        }
        match (&binding.boundary_entry_plan.call.result, result_shape) {
            (None, None) => {}
            (Some(placement), Some(shape)) => {
                let placed = single_register_sized(&placement.locations, shape.byte_size);
                if placement.shape != shape
                    || (matches!(expected_result, TargetBoundaryResult::Scalar(_)) && !placed)
                {
                    return Err(psi_operation);
                }
            }
            _ => return Err(psi_operation),
        }
        // `Home` sources cite the exact retained result home of the producing
        // operation — replayed on that row's own source operation — while an
        // `IntegerImmediate` must equal the source `IntegerConstant` op it
        // names. Incoming and block parameters use the same independently
        // reconstructed source coordinates as hosted integer boundaries;
        // neither the argument row nor its matching width establishes identity.
        // Reads and membership checks retain a bare `AbstractResult` instead of
        // a home field; their lowering derives the requirement deterministically
        // from that result, so the roster rebuilds it the same way here.
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
                | TargetUnitOperation::ScalarRepresentationView { result_home, .. }
                | TargetUnitOperation::FloatResultPolicy { result_home, .. }
                | TargetUnitOperation::NearestIeeeFloatArithmetic { result_home, .. }
                | TargetUnitOperation::ScalarDefinition { result_home, .. } => Some(*result_home),
                TargetUnitOperation::Call { result, .. } => result.scalar_home().copied(),
                TargetUnitOperation::AtomicAccess { result_home, .. } => *result_home,
                TargetUnitOperation::NormalizedForeignCall { result, .. } => {
                    result.scalar_home().copied()
                }
                TargetUnitOperation::StructuralScalarFieldRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralCaseMembership {
                    psi_operation,
                    result,
                    ..
                } => structural_signatures::fixed_native_scalar_shape(result.scalar_type).map(
                    |shape| TargetUnitScalarHomeRequirement {
                        defining_operation: *psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape,
                    },
                ),
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
            let placed_byte_size =
                placed_word_byte_size(&actual.placement.locations).ok_or(psi_operation)?;
            let source_invalid = match &actual.source {
                TargetUnitScalarArgumentSource::Parameter { .. }
                | TargetUnitScalarArgumentSource::BlockParameter(_) => self
                    .scalar_argument_source(
                        psi_operation,
                        *value,
                        declaration.scalar_parameters[index],
                        &actual.source,
                    )
                    .is_err(),
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
                TargetUnitScalarArgumentSource::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value: literal,
                } => {
                    *source_value != *value
                        || !self.source.operations.iter().any(|operation| {
                            matches!(
                                operation,
                                AbstractOperation::BooleanConstant {
                                    psi_operation,
                                    result,
                                    value
                                } if *psi_operation == *defining_operation
                                    && *result == *source_value
                                    && *value == *literal
                            )
                        })
                }
                TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value: literal,
                } => {
                    *source_value != *value
                        || !self.source.operations.iter().any(|operation| {
                            matches!(
                                operation,
                                AbstractOperation::IeeeFloatConstant {
                                    psi_operation,
                                    result,
                                    value
                                } if *psi_operation == *defining_operation
                                    && *result == *source_value
                                    && *value == *literal
                            )
                        })
                }
                TargetUnitScalarArgumentSource::Home(home) => {
                    home.source_value != *value
                        || home.shape != *shape
                        || home.defining_operation == psi_operation
                        || retained_scalar_homes.get(&home.defining_operation) != Some(home)
                }
            };
            // Each retained parameter-destination callback claims a
            // native-only slot: every semantic scalar at or after its
            // ordinal shifts one placement position — the materialized
            // signature is positional in the authored order, not the
            // semantic order.
            let expected_index = formal_position
                + parameter_ordinals.partition_point(|&ordinal| formal_position >= ordinal);
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
            // The referent root resolves through the caller's own checked
            // structural places — parameters, block entries, and
            // operation-established results — and the projected type,
            // offset, and home are re-derived from those declarations
            // rather than the retained row.
            let root = self.roots.get(&semantic.place).ok_or(psi_operation)?;
            let destination = binding
                .boundary_entry_plan
                .call
                .parameters
                .get(
                    formal_position
                        + parameter_ordinals.partition_point(|&ordinal| formal_position >= ordinal),
                )
                .ok_or(psi_operation)?;
            if usize::try_from(declaration_parameter.position).ok() != Some(index)
                || semantic.access != declaration_parameter.access
                || !declaration_parameter.qualifications.is_empty()
                || !declaration_parameter.projected_qualifications.is_empty()
            {
                return Err(psi_operation);
            }
            // Reconstruct geometry and multiplicity from source declarations,
            // not the producer's retained home. The common
            // structural_argument_sources replay separately binds the exact
            // parameter, block arrival or establishing operation. Neither
            // check needs to rediscover a call-result-only source pattern.
            // Ordinary projected calls may borrow a result ABI placement;
            // foreign calls transport the durable home instead. A placement
            // here must therefore be an actual incoming parameter, never a
            // transient return register with the same bytes.
            if let TargetStructuralArgumentSource::Placement(placement) = &actual.source {
                let parameter = self
                    .target
                    .graph
                    .parameters
                    .iter()
                    .find(|parameter| parameter.place == semantic.place)
                    .ok_or(psi_operation)?;
                if parameter.structural_type != root.structural_type
                    || parameter.placement != *placement
                {
                    return Err(psi_operation);
                }
            }
            let source_byte_size =
                structural_shapes::reconstruct(root.structural_type, self.declarations)
                    .map_err(|_| psi_operation)?
                    .byte_size;
            let expected_multiplicity = if semantic.access != StructuralAccess::Owned {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else if semantic.path.is_empty() {
                root.multiplicity
            } else {
                // A projected owned member move surrenders the leaf as
                // affine custody: the moved occurrence is used at most
                // once while the ownership frontier tracks the hole it
                // leaves. The leaf binds the parameter's type below and
                // stores no multiplicity of its own.
                terminal_psi::StructuralMultiplicity::Affine
            };
            if declaration_parameter.multiplicity != expected_multiplicity {
                return Err(psi_operation);
            }
            let descriptor_formal = self.declarations.iter().any(|entry| {
                entry.id == declaration_parameter.structural_type
                    && matches!(
                        entry.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
            });
            let mut runtime_elements = Vec::new();
            let (projected_type, projected_shape, byte_offset) = match declaration_parameter.access
            {
                StructuralAccess::Owned => {
                    // Whole-place owned transport names the root itself; a
                    // projected member moves its subtree by value, resolving
                    // leaf type, shape, and byte offset exactly like a
                    // borrowed projection.
                    if semantic.path.is_empty() {
                        (
                            root.structural_type,
                            structural_shapes::reconstruct(root.structural_type, self.declarations)
                                .map_err(|_| psi_operation)?,
                            0,
                        )
                    } else {
                        if semantic.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        }) {
                            return Err(psi_operation);
                        }
                        if semantic
                            .path
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                        {
                            static_element_projection(
                                root.structural_type,
                                &semantic.path,
                                self.declarations,
                            )
                            .ok_or(psi_operation)?
                        } else {
                            structural_shapes::projected_field(
                                root.structural_type,
                                &semantic.path,
                                self.declarations,
                            )
                            .map_err(|_| psi_operation)?
                        }
                    }
                }
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => {
                    if semantic.path.is_empty() {
                        // A whole loan names the exact root at offset zero.
                        (
                            root.structural_type,
                            structural_shapes::reconstruct(root.structural_type, self.declarations)
                                .map_err(|_| psi_operation)?,
                            0,
                        )
                    } else if semantic.path.iter().any(|segment| {
                        matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
                    }) {
                        let (ty, offset, elements) = structural_shapes::project_runtime_path(
                            root.structural_type,
                            &semantic.path,
                            self.declarations,
                        )
                        .map_err(|_| psi_operation)?;
                        runtime_elements = elements;
                        (
                            ty,
                            structural_shapes::reconstruct(ty, self.declarations)
                                .map_err(|_| psi_operation)?,
                            offset,
                        )
                    } else if semantic
                        .path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                    {
                        // A fixed element (`&text[0]`) is one static offset
                        // inside the referent, like a field; the element's
                        // own declaration supplies its storage shape.
                        static_element_projection(
                            root.structural_type,
                            &semantic.path,
                            self.declarations,
                        )
                        .ok_or(psi_operation)?
                    } else {
                        if semantic
                            .path
                            .iter()
                            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                        {
                            return Err(psi_operation);
                        }
                        match structural_shapes::projected_field(
                            root.structural_type,
                            &semantic.path,
                            self.declarations,
                        ) {
                            Ok(projection) => projection,
                            Err(_) => {
                                // A leaf carrying BorrowedView storage
                                // has no projected carrier identity; its
                                // field offset inside the root replays
                                // instead, and only a descriptor formal
                                // admits it.
                                let field_offset = structural_shapes::borrowed_view_field_offset(
                                    root.structural_type,
                                    &semantic.path,
                                    self.declarations,
                                )
                                .map_err(|_| psi_operation)?;
                                if !descriptor_formal {
                                    return Err(psi_operation);
                                }
                                (
                                    declaration_parameter.structural_type,
                                    structural_shapes::reconstruct(
                                        declaration_parameter.structural_type,
                                        self.declarations,
                                    )
                                    .map_err(|_| psi_operation)?,
                                    field_offset,
                                )
                            }
                        }
                    }
                }
            };
            let projects_view_backing = semantic.access != StructuralAccess::Owned
                && !semantic.path.is_empty()
                && self.declarations.iter().any(|row| {
                    row.id == root.structural_type
                        && (row.shape.is_borrowed_byte_carrier()
                            || matches!(row.shape, StructuralTypeShape::ElementView { .. }))
                });
            if projected_type != declaration_parameter.structural_type
                || (!projects_view_backing
                    && projected_shape
                        .byte_size
                        .checked_add(byte_offset)
                        .is_none_or(|end| end > source_byte_size))
            {
                return Err(psi_operation);
            }
            let destination_ok = match declaration_parameter.access {
                StructuralAccess::Owned => {
                    // By-value transport lands the aggregate's classified
                    // shape — one integer register pair, an HFA row, or a
                    // SysV split — never a borrowed-reference destination.
                    !destination.locations.is_empty()
                        && destination.shape.byte_size == projected_shape.byte_size
                        && destination.shape.alignment == projected_shape.alignment
                        && destination.shape.class != ValueClass::BorrowedReference
                }
                _ if descriptor_formal => {
                    // The stored view crosses by value under the aggregate
                    // class: two pointer words in the plan's own placement.
                    !destination.locations.is_empty()
                        && destination.shape.byte_size == projected_shape.byte_size
                        && destination.shape.alignment == projected_shape.alignment
                        && destination.shape.class != ValueClass::BorrowedReference
                }
                _ => {
                    let placed_pointer_word =
                        placed_word_byte_size(&destination.locations).ok_or(psi_operation)?;
                    destination.shape == pointer_shape
                        && placed_pointer_word == u32::from(pointer_size)
                }
            };
            if !destination_ok {
                return Err(psi_operation);
            }
            for index in &actual.indices {
                self.scalar_argument_source(
                    psi_operation,
                    index.operand.source_value(),
                    index.operand.scalar_type(),
                    &index.operand,
                )?;
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
                || actual.indices.len() != runtime_elements.len()
                || actual.indices.iter().zip(&runtime_elements).any(
                    |(actual, (selector, _, stride))| {
                        actual.operand.source_value() != *selector || actual.stride != *stride
                    },
                )
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
            // Inline float arithmetic never forms a settlement row.
            BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::NearestIeeeFloatArithmetic { .. },
            ) => false,
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
            BoundaryRealization::HostedWriteByteI32(_) => {
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
                let supports_target =
                    target_operations::HostedWriteByteI32Realization::supports_target(
                        self.native_target,
                    );
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
                self.scalar_argument_source(
                    psi_operation,
                    source_value,
                    ScalarType::Integer(i32_type),
                    &row.source,
                )
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

    /// Rejoin a scalar operand to its exact declaration or producing operation,
    /// for ordinary, hosted and foreign calls alike. Shape equality alone does
    /// not identify a computed value. Executable CFG dominance remains the
    /// downstream graph replay's responsibility, not this ABI custody receipt.
    fn scalar_argument_source(
        &self,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        source: &TargetUnitScalarArgumentSource,
    ) -> Result<(), OperationId> {
        let expected_scalar = scalar_type;
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
                    && ScalarType::Integer(*actual_type) == expected_scalar
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
            TargetUnitScalarArgumentSource::BooleanImmediate {
                defining_operation,
                source_value: actual,
                value,
            } => {
                *actual == source_value
                    && expected_scalar == ScalarType::Boolean
                    && self.source.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            AbstractOperation::BooleanConstant {
                                psi_operation: produced,
                                result,
                                value: literal,
                            } if *produced == *defining_operation
                                && *result == *actual
                                && *literal == *value
                        )
                    })
            }
            TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                defining_operation,
                source_value: actual,
                value,
            } => {
                *actual == source_value
                    && expected_scalar == ScalarType::IeeeFloat(value.format())
                    && self.source.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            AbstractOperation::IeeeFloatConstant {
                                psi_operation: produced,
                                result,
                                value: literal,
                            } if *produced == *defining_operation
                                && *result == *actual
                                && *literal == *value
                        )
                    })
            }
            TargetUnitScalarArgumentSource::Home(home) => {
                home.source_value == source_value
                    && home.scalar_type == expected_scalar
                    && home.defining_operation != psi_operation
                    && Some(home.shape)
                        == structural_signatures::fixed_native_scalar_shape(scalar_type)
                    && self.source.operations.iter().any(|operation| {
                        scalar_home_result(operation).is_some_and(|(produced, result)| {
                            produced == home.defining_operation
                                && result.value == home.source_value
                                && result.scalar_type == home.scalar_type
                        })
                    })
            }
        };
        if valid { Ok(()) } else { Err(psi_operation) }
    }

    /// Ordered scalar argument rows bind the plan's anonymous parameter
    /// placements back to the declared value and type identities.
    fn scalar_arguments(
        &self,
        psi_operation: OperationId,
        actual: &[TargetUnitScalarCallArgument],
        values: &[ValueId],
        declared: AbstractCallSignature<'_>,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        if actual.len() != values.len() || values.len() != declared.scalar_parameter_count() {
            return Err(psi_operation);
        }
        for (position, (actual, value)) in actual.iter().zip(values).enumerate() {
            let declared = declared.scalar_parameter(position).ok_or(psi_operation)?;
            let Ok(index) = u32::try_from(position) else {
                return Err(psi_operation);
            };
            if actual.parameter_index != index
                || actual.source.source_value() != *value
                || actual.scalar_type() != declared
                || actual.placement != expected_plan.parameters[position]
            {
                return Err(psi_operation);
            }
            self.scalar_argument_source(psi_operation, *value, declared, &actual.source)?;
        }
        Ok(())
    }

    /// One retained structural call argument replays the declared parameter
    /// identity, the independently reconstructed referent shape, and the
    /// canonical destination placement.
    #[allow(clippy::too_many_arguments)]
    fn structural_argument(
        &self,
        psi_operation: OperationId,
        plan_index: usize,
        actual: &TargetStructuralArgument,
        semantic: &StructuralArgument,
        declared: &StructuralParameterDeclaration,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        if semantic.path.contains(&StructuralPathSegment::Referent) {
            if !self.reference_projection(actual, semantic, declared) {
                return Err(psi_operation);
            }
            return expected_plan
                .parameters
                .get(plan_index)
                .is_some_and(|destination| actual.destination == *destination)
                .then_some(())
                .ok_or(psi_operation);
        }
        if !matches_argument_identity(actual, semantic, declared) {
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
        if matches_bounded_byte_storage(actual, root, self.declarations) {
            return Ok(());
        }
        // A fixed array presents its backing from an incoming parameter, from
        // a block parameter that rebinds such a loan at a state edge or,
        // whole, from a local home lent through this argument's own borrow.
        let local_loan = root.loan_presentation(semantic, declared);
        let block_root = self
            .source
            .block_entries
            .iter()
            .filter(|entry| entry.block != self.source.entry)
            .flat_map(|entry| &entry.structural_parameters)
            .find(|parameter| parameter.place == semantic.place);
        let window_roots = || {
            self.source
                .structural_parameters
                .iter()
                .chain(&local_loan)
                .chain(block_root)
        };
        if let Some(window) = window_roots().find_map(|source| {
            terminal_semantics::fixed_byte_array_window(
                self.declarations.iter(),
                source,
                semantic,
                declared,
            )
        }) {
            let (_, backing_offset) = structural_shapes::project_static_path(
                root.structural_type,
                window.backing_path,
                self.declarations,
            )
            .map_err(|_| psi_operation)?;
            let offset = u64::from(backing_offset)
                .checked_add(window.offset)
                .and_then(|offset| u32::try_from(offset).ok())
                .ok_or(psi_operation)?;
            if actual.root_structural_type != root.structural_type
                || actual.source_byte_offset != offset
                || actual.fixed_array_length != Some(window.length)
                || actual.element_stride != Some(1)
            {
                return Err(psi_operation);
            }
            return Ok(());
        }
        // The element-view counterpart of the byte window: a whole-field
        // fixed array loans its backing to a `slice`-shaped parameter, so
        // the retained transport carries the array's extent and element
        // stride rather than stride 1.
        if let Some(length) = window_roots().find_map(|source| {
            terminal_semantics::fixed_element_array_extent(
                self.declarations.iter(),
                source,
                semantic,
                declared,
            )
        }) {
            let (array_type, byte_offset) = structural_shapes::project_static_path(
                root.structural_type,
                &semantic.path,
                self.declarations,
            )
            .map_err(|_| psi_operation)?;
            let (_transport_length, expected_stride) =
                structural_shapes::root_array_transport(array_type, self.declarations)
                    .map_err(|_| psi_operation)?;
            if actual.root_structural_type != root.structural_type
                || actual.source_byte_offset != byte_offset
                || actual.fixed_array_length != Some(length)
                || actual.element_stride != expected_stride
            {
                return Err(psi_operation);
            }
            return Ok(());
        }
        // A range cannot fall through the ordinary owned projection checks.
        if semantic
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::FixedByteRange { .. }))
        {
            return Err(psi_operation);
        }
        // Static subloans from borrowed roots or addressable homes carry the
        // leaf address, as do record/sum reference formals. Reconstruct that
        // presentation from this operand's declaration and storage, never from
        // a sibling operand or the call's result kind.
        let aggregate_reference = matches!(
            declared.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && declared.multiplicity
            == terminal_psi::StructuralMultiplicity::Unrestricted
            && declared.qualifications.is_empty()
            && declared.projected_qualifications.is_empty()
            && self.declarations.iter().any(|declaration| {
                declaration.id == declared.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Record { .. } | StructuralTypeShape::Sum { .. }
                    )
            });
        let static_borrow =
            (root.access != StructuralAccess::Owned || root.operation_home || aggregate_reference)
                && semantic.access != StructuralAccess::Owned
                && semantic.path.iter().all(|segment| {
                    matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                            | StructuralPathSegment::FixedIndex(_)
                            | StructuralPathSegment::Case(_)
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
                    if !matches_bounded_byte_storage(actual, root, self.declarations)
                        && !matches_stored_view_field(actual, root, self.declarations)
                    {
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
        } else if semantic
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        {
            // A runtime-indexed borrow carries no static descriptor: the
            // retained byte offset is the projection's static prefix and each
            // runtime segment's resolved operand and stride ride `indices`
            // with the verified selector verbatim on `path`.
            let (projected_type, byte_offset, elements) = structural_shapes::project_runtime_path(
                root.structural_type,
                &semantic.path,
                self.declarations,
            )
            .map_err(|_| psi_operation)?;
            if actual.root_structural_type != root.structural_type
                || actual.structural_type != projected_type
                || actual.source_byte_offset != byte_offset
                || actual.indices.len() != elements.len()
                || actual.indices.iter().zip(elements.iter()).any(
                    |(retained, (selector, _, stride))| {
                        retained.operand.source_value() != *selector || retained.stride != *stride
                    },
                )
                || actual.fixed_array_length.is_some()
                || actual.element_stride.is_some()
            {
                return Err(psi_operation);
            }
        }
        Ok(())
    }

    /// Reconstruct the carrier descriptor and its referent projection from
    /// source types. Canonical storage is checked separately against the same
    /// carrier place; provenance never substitutes for the stored pointer.
    fn reference_projection(
        &self,
        actual: &TargetStructuralArgument,
        semantic: &StructuralArgument,
        declared: &StructuralParameterDeclaration,
    ) -> bool {
        if !semantic.path.contains(&StructuralPathSegment::Referent) {
            return false;
        }
        if !matches_argument_identity(actual, semantic, declared)
            || semantic.access == StructuralAccess::Owned
            || declared.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !declared.qualifications.is_empty()
            || !declared.projected_qualifications.is_empty()
            || actual.fixed_array_length.is_some()
            || actual.element_stride.is_some()
            || !actual.indices.is_empty()
        {
            return false;
        }
        let Some(root) = self.roots.get(&semantic.place) else {
            return false;
        };
        let mut current = root.structural_type;
        let mut path = semantic.path.as_slice();
        while let Some(crossing) = path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)
        {
            let Ok((carrier, _)) = structural_shapes::project_static_path(
                current,
                &path[..crossing],
                self.declarations,
            ) else {
                return false;
            };
            let Some(StructuralTypeShape::Reference { referent, access }) = self
                .declarations
                .iter()
                .find(|declaration| declaration.id == carrier)
                .map(|declaration| &declaration.shape)
            else {
                return false;
            };
            if !(*access == semantic.access
                || *access == StructuralAccess::MutableBorrow
                    && matches!(
                        semantic.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                    ))
            {
                return false;
            }
            current = *referent;
            path = &path[crossing + 1..];
            if *access == StructuralAccess::WriteOnlyBorrow
                && path.contains(&StructuralPathSegment::Referent)
            {
                return false;
            }
        }
        let Ok((selected, offset)) =
            structural_shapes::project_static_path(current, path, self.declarations)
        else {
            return false;
        };
        actual.root_structural_type == root.structural_type
            && actual.structural_type == selected
            && actual.source_byte_offset == offset
            && structural_shapes::reconstruct(selected, self.declarations).is_ok_and(|shape| {
                actual.shape == structural_shapes::parameter_shape(shape, actual.access)
            })
    }

    /// The retained result row must be the source-declared result verbatim,
    /// the declared callee result kind, and the independently reconstructed
    /// durable home. A physically similar row is never authority.
    fn bind_result(
        &self,
        psi_operation: OperationId,
        expected: BoundResult<'_>,
        actual: &EmbeddedResult<'_>,
        callee: AbstractCallSignature<'_>,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        match (expected, actual) {
            (BoundResult::Unit, EmbeddedResult::Unit) => {
                if !matches!(callee, AbstractCallSignature::Machine(machine) if machine.result == AbstractFunctionResult::Unit)
                    && !matches!(callee, AbstractCallSignature::Requirement(requirement) if requirement.result.is_none())
                {
                    return Err(psi_operation);
                }
                Ok(())
            }
            (BoundResult::Scalar(expected), EmbeddedResult::Scalar(actual)) => {
                scalar_result(psi_operation, expected, actual, callee)
            }
            (BoundResult::PublishedScalar(expected), EmbeddedResult::Scalar(actual)) => {
                scalar_result(psi_operation, &expected, actual, callee)?;
                let AbstractCallSignature::Machine(callee) = callee else {
                    return Err(psi_operation);
                };
                self.published_scalar_abi(psi_operation, actual, callee, expected_plan)
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
                // The public requirement schema has no structural result contract.
                let AbstractCallSignature::Machine(callee) = callee else {
                    return Err(psi_operation);
                };
                let declared = callee.result.structural().ok_or(psi_operation)?;
                if **result != *expected
                    || *callee_result != declared
                    || declared.structural_type != expected.structural_type
                    || declared.multiplicity != expected.multiplicity
                    || !declared_or_minted_qualifications(declared, expected)
                    || declared.projected_qualifications != expected.projected_qualifications
                    || (!declared.qualifications.is_empty()
                        && !self.declarations.iter().any(|declaration| {
                            declaration.id == declared.structural_type
                                && declaration.shape.supports_result_qualifications()
                        }))
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
                // Reference results retain an address payload home just like
                // other values. Its geometry grants no loan authority: the
                // independent candidate roster comparison above still applies.
                let expected_home = structural_shapes::structural_result_home(
                    psi_operation,
                    expected,
                    self.declarations,
                )
                .map_err(|_| psi_operation)?;
                if *result_home != Some(&expected_home) {
                    return Err(psi_operation);
                }
                Ok(())
            }
            _ => Err(psi_operation),
        }
    }

    /// The scalar-only `Call` additionally replays the callee's published
    /// standalone scalar entrance: the callee declares no descriptor
    /// parameters, the result sits in one register at offset zero with the
    /// retained home's shape, and the callee's published ABI rows equal this
    /// plan. A call to a callee that never published one is forged no matter
    /// how consistent the embedded plan looks.
    fn published_scalar_abi(
        &self,
        psi_operation: OperationId,
        home: &TargetUnitScalarHomeRequirement,
        callee: &AbstractFunction,
        expected_plan: &CallPlan,
    ) -> Result<(), OperationId> {
        let Some(callee_result) = callee.result.scalar() else {
            return Err(psi_operation);
        };
        let Some(result_placement) = expected_plan.result.as_ref() else {
            return Err(psi_operation);
        };
        if !single_register_sized(
            &result_placement.locations,
            result_placement.shape.byte_size,
        ) || home.shape != result_placement.shape
        {
            return Err(psi_operation);
        }
        let expected_abi = ScalarFunctionAbi {
            call_plan: expected_plan.clone(),
            parameters: callee
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
            .find(|function| function.machine == callee.machine)
            .and_then(|function| function.scalar_abi.as_ref())
            != Some(&expected_abi)
        {
            return Err(psi_operation);
        }
        Ok(())
    }
}
