//! Bind each retained structural call argument to its referent's exact home.
//!
//! `TargetStructuralArgument` retains the semantic referent (`place`, `path`,
//! `access`) beside the caller-side storage custody that produced the borrowed
//! pointer. A physically equivalent placement or a staged local is not the
//! referent, and shared physical shape cannot authorize access substitution:
//! this pass reconstructs the canonical home for every claimed argument place
//! from the source function — a caller parameter's checked placement, a
//! non-entry block parameter, or the exact establishing operation — and
//! rejects any argument whose `source` names different storage or a different
//! producer. Independent replay then owns the same identities downstream.

use std::collections::BTreeMap;

use abstract_operations::{AbstractBoundaryResult, AbstractFunction, AbstractOperation};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use target_operations::{
    TargetFunction, TargetStructuralArgument, TargetStructuralArgumentSource,
    TargetStructuralHomeRequirement, TargetUnitOperation,
};
use terminal_psi::StructuralOperationResult;

/// The caller-side storage one argument place is required to name. Only the
/// producing relationships the lowering pipeline can actually form are
/// accepted; every other `source` is a substitution.
enum ExpectedSource<'a> {
    /// A caller structural parameter; the required placement is read back
    /// from the already-checked target parameter roster, not re-derived.
    CallerParameter,
    /// A non-entry block structural parameter and its owning block.
    BlockParameter(BlockId),
    /// `EstablishPrimitiveLocal` activation storage. A borrowed callee
    /// parameter presents it as `EstablishedPrimitiveLocal`; owned callee
    /// transport presents the same home as `StructuralHome`.
    PrimitiveLocal(OperationId),
    /// Record, fixed-array, or sum storage established by one exact
    /// operation result home.
    Aggregate(OperationId),
    /// A byte-view descriptor established by one exact literal or subslice
    /// producer. Views are never structural homes.
    ByteView(OperationId),
    /// A structural call result home. Projected borrows may name the
    /// producing call's own result placement instead of the operation home.
    CallResult {
        operation: OperationId,
        result: &'a StructuralOperationResult,
    },
}

/// A retained structural-result `Call` producer: its semantic result, retained
/// home requirement, and published result placement.
type ResultCall<'a> = (
    &'a StructuralOperationResult,
    Option<&'a TargetStructuralHomeRequirement>,
    Option<&'a ValuePlacement>,
);

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<(), OperationId> {
    let expected = expected_sources(source);
    let mut target_calls = BTreeMap::new();
    let mut result_calls = BTreeMap::new();
    for operation in target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
    {
        match operation {
            TargetUnitOperation::Call {
                psi_operation,
                result,
                call_plan,
                arguments,
                ..
            } => {
                target_calls.insert(*psi_operation, arguments.as_slice());
                if let target_operations::TargetCallResult::Structural {
                    result,
                    result_home,
                    ..
                } = result
                {
                    result_calls.insert(
                        *psi_operation,
                        (result, result_home.as_ref(), call_plan.result.as_ref()),
                    );
                }
            }
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                psi_operation,
                structural_arguments,
                ..
            }
            | TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
                psi_operation,
                structural_arguments,
                ..
            }
            | TargetUnitOperation::NormalizedForeignCall {
                psi_operation,
                structural_arguments,
                ..
            } => {
                target_calls.insert(*psi_operation, structural_arguments.as_slice());
            }
            _ => {}
        }
    }
    let target_calls: BTreeMap<OperationId, &[TargetStructuralArgument]> = target_calls;
    let result_calls: BTreeMap<OperationId, ResultCall<'_>> = result_calls;

    for operation in &source.operations {
        let (psi_operation, structural_arguments) = match operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructuralScalar {
                psi_operation,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                structural_arguments,
                ..
            }
            | AbstractOperation::BoundaryCall {
                psi_operation,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallUnitWithDynamicArguments {
                psi_operation,
                structural_arguments,
                ..
            }
            | AbstractOperation::CallStructuralScalarWithDynamicArguments {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.as_slice()),
            _ => continue,
        };
        let Some(arguments) = target_calls.get(&psi_operation) else {
            continue;
        };
        if arguments.len() != structural_arguments.len() {
            return Err(psi_operation);
        }
        for actual in *arguments {
            if !matches_home(&expected, &result_calls, target, actual) {
                return Err(psi_operation);
            }
        }
    }
    Ok(())
}

/// Reconstruct every argument place's canonical caller-side home from the
/// source function, in the same precedence the producers consult:
/// operation-established places, then non-entry block parameters, then
/// caller parameters.
fn expected_sources(source: &AbstractFunction) -> BTreeMap<PlaceId, ExpectedSource<'_>> {
    let mut expected = BTreeMap::new();
    for operation in &source.operations {
        let (place, home) = match operation {
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                ..
            } => (result.place, ExpectedSource::PrimitiveLocal(*psi_operation)),
            AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishScalarArray {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishScalarCase {
                psi_operation,
                result,
                ..
            } => (result.place, ExpectedSource::Aggregate(*psi_operation)),
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                ..
            } => (place.id, ExpectedSource::ByteView(*psi_operation)),
            AbstractOperation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            } => (result.place, ExpectedSource::ByteView(*psi_operation)),
            AbstractOperation::CallStructural {
                psi_operation,
                result,
                ..
            } => (
                result.place,
                ExpectedSource::CallResult {
                    operation: *psi_operation,
                    result,
                },
            ),
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => (
                result.place,
                ExpectedSource::CallResult {
                    operation: *psi_operation,
                    result,
                },
            ),
            _ => continue,
        };
        expected.entry(place).or_insert(home);
    }
    for entry in &source.block_entries {
        if entry.block == source.entry {
            continue;
        }
        for parameter in &entry.structural_parameters {
            expected
                .entry(parameter.place)
                .or_insert(ExpectedSource::BlockParameter(entry.block));
        }
    }
    for parameter in &source.structural_parameters {
        expected
            .entry(parameter.place)
            .or_insert(ExpectedSource::CallerParameter);
    }
    expected
}

/// The retained source must name the claimed place's canonical storage: the
/// exact incoming placement, block parameter, or establishing producer — not
/// a staged destination, a sibling's home, or a copied referent.
fn matches_home(
    expected: &BTreeMap<PlaceId, ExpectedSource<'_>>,
    result_calls: &BTreeMap<OperationId, ResultCall<'_>>,
    target: &TargetFunction,
    actual: &TargetStructuralArgument,
) -> bool {
    let place = actual.place;
    match (expected.get(&place), &actual.source) {
        (
            Some(ExpectedSource::CallerParameter),
            TargetStructuralArgumentSource::Placement(placement),
        ) => target
            .graph
            .parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .is_some_and(|parameter| parameter.placement == *placement),
        (
            Some(ExpectedSource::BlockParameter(block)),
            TargetStructuralArgumentSource::BlockParameter {
                block: actual_block,
                place: actual_place,
            },
        ) => actual_block == block && *actual_place == place,
        (
            Some(ExpectedSource::PrimitiveLocal(producer)),
            TargetStructuralArgumentSource::EstablishedPrimitiveLocal { psi_operation }
            | TargetStructuralArgumentSource::StructuralHome { psi_operation },
        ) => psi_operation == producer,
        (
            Some(ExpectedSource::Aggregate(producer)),
            TargetStructuralArgumentSource::StructuralHome { psi_operation },
        ) => psi_operation == producer,
        (
            Some(ExpectedSource::ByteView(producer)),
            TargetStructuralArgumentSource::EstablishedByteView { psi_operation },
        ) => psi_operation == producer,
        (
            Some(ExpectedSource::CallResult { operation, .. }),
            TargetStructuralArgumentSource::StructuralHome { psi_operation },
        ) => psi_operation == operation,
        (
            Some(ExpectedSource::CallResult { operation, result }),
            TargetStructuralArgumentSource::Placement(placement),
        ) => result_calls
            .get(operation)
            .is_some_and(|(actual_result, home, result_placement)| {
                *actual_result == *result
                    && home.and_then(TargetStructuralHomeRequirement::operation_result)
                        == Some((*operation, *result))
                    && *result_placement == Some(placement)
            }),
        _ => false,
    }
}
