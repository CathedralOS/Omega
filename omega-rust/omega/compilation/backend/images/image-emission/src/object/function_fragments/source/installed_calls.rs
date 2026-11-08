//! Check the two ordinary calls made by an installation before image publication.
//! This does not establish the CPU guards or root/table writes. The separate
//! installation admission must establish those before accepting the Scope.

use super::Error;
use abstract_operations::{AbstractBoundaryResult, AbstractFunction, AbstractOperation};
use legalized_operations::{LegalizedScalarArgument, NativeCallOrigin};
use selected_instructions::{
    SelectedCallContract, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use semantic_vocabulary::OperationId;
use target_operations::{
    CallSiteOwner, InstallationCallRole, NativeCallTarget, TargetFunction, TargetUnitOperation,
};

/// Prepare each owning roster once. Opaque operation IDs are sorted keys,
/// never allocation lengths or unchecked indices into another owner's arena.
struct PreparedCalls<'a> {
    targets: Vec<(OperationId, &'a TargetUnitOperation)>,
    sources: Vec<(OperationId, &'a AbstractOperation)>,
    calls: Vec<(
        (OperationId, InstallationCallRole),
        &'a SelectedCallContract,
    )>,
    instructions: Vec<(SelectedInstructionId, &'a SelectedInstruction)>,
}

fn unique_index<Key: Copy + Ord, Row>(
    rows: impl Iterator<Item = (Key, Row)>,
    duplicate: &'static str,
) -> Result<Vec<(Key, Row)>, Error> {
    let mut rows = rows.collect::<Vec<_>>();
    rows.sort_unstable_by_key(|(key, _)| *key);
    if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Mismatch(duplicate));
    }
    Ok(rows)
}

fn find<Key: Ord, Row>(rows: &[(Key, Row)], key: Key) -> Option<&Row> {
    rows.binary_search_by(|(actual, _)| actual.cmp(&key))
        .ok()
        .map(|position| &rows[position].1)
}

impl<'a> PreparedCalls<'a> {
    fn new(
        abstracted: &'a AbstractFunction,
        targeted: &'a TargetFunction,
        selected: &'a SelectedFunction,
    ) -> Result<Self, Error> {
        let targets = unique_index(
            targeted.graph.operations().filter_map(|row| match row {
                TargetUnitOperation::InstallStaticInterrupt { psi_operation, .. } => {
                    Some((*psi_operation, row))
                }
                _ => None,
            }),
            "installation target operation is duplicated",
        )?;
        let calls = unique_index(
            selected.calls.iter().filter_map(|row| match row.owner {
                CallSiteOwner::Installation { operation, role } => Some(((operation, role), row)),
                _ => None,
            }),
            "installation helper role is duplicated",
        )?;
        for ((operation, _), _) in &calls {
            if find(&targets, *operation).is_none() {
                return Err(Error::Mismatch("installation call has no owning operation"));
            }
        }
        // Most functions contain no installations. Avoid preparing unused
        // source/instruction tables for their unrelated ordinary calls.
        if targets.is_empty() {
            return Ok(Self {
                targets,
                calls,
                sources: Vec::new(),
                instructions: Vec::new(),
            });
        }
        let sources = unique_index(
            abstracted.operations.iter().filter_map(|row| match row {
                AbstractOperation::BoundaryCall { psi_operation, .. } => {
                    Some((*psi_operation, row))
                }
                _ => None,
            }),
            "source boundary operation is duplicated",
        )?;
        let instructions = unique_index(
            selected
                .blocks
                .iter()
                .flat_map(|block| {
                    // Nonreturning calls live in the terminator. Its instruction
                    // has the same exact contract identity as an ordinary call.
                    block.instructions.iter().chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                })
                .map(|row| (row.id, row)),
            "selected instruction identity is duplicated",
        )?;
        Ok(Self {
            targets,
            sources,
            calls,
            instructions,
        })
    }
}

pub(super) fn validate(
    abstracted: &AbstractFunction,
    targeted: &TargetFunction,
    selected: &SelectedFunction,
) -> Result<(), Error> {
    let prepared = PreparedCalls::new(abstracted, targeted, selected)?;
    for (_, row) in &prepared.targets {
        let TargetUnitOperation::InstallStaticInterrupt {
            psi_operation,
            boundary,
            arguments,
            calls,
            result_home,
        } = row
        else {
            continue;
        };
        let Some(AbstractOperation::BoundaryCall {
            boundary: source_boundary,
            arguments: scalars,
            structural_arguments,
            result: AbstractBoundaryResult::Scalar(result),
            ..
        }) = find(&prepared.sources, *psi_operation).copied()
        else {
            return Err(Error::Mismatch(
                "installation calls have no unique source boundary operation",
            ));
        };
        if boundary != source_boundary
            || !scalars.is_empty()
            || structural_arguments.as_slice() != arguments
            || result.scalar_type != semantic_vocabulary::ScalarType::Boolean
            || result_home.defining_operation != *psi_operation
            || result_home.source_value != result.value
            || result_home.scalar_type != result.scalar_type
        {
            return Err(Error::Mismatch(
                "installation calls changed their source operands or refusal result",
            ));
        }
        for (position, target) in calls.iter().enumerate() {
            let (role, argument_position, result_shape) = if position == 0 {
                (
                    InstallationCallRole::Validate,
                    0,
                    calling_conventions::ValueShape::for_scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                )
            } else {
                (InstallationCallRole::Work, 5, None)
            };
            let contract = find(&prepared.calls, (*psi_operation, role))
                .copied()
                .ok_or(Error::Mismatch(
                    "installation must retain exactly its validation and work calls",
                ))?;
            let mut semantic = arguments[argument_position].clone();
            semantic.access = terminal_psi::StructuralAccess::SharedBorrow;
            let call = &contract.call;
            let [
                LegalizedScalarArgument::Structural {
                    semantic: actual,
                    target: actual_target,
                    ..
                },
            ] = call.arguments.as_slice()
            else {
                return Err(Error::Mismatch(
                    "installation helper lost its single borrowed operand",
                ));
            };
            if target.role != role
                || contract.owner
                    != (CallSiteOwner::Installation {
                        operation: *psi_operation,
                        role,
                    })
                || call.target != NativeCallTarget::Machine(target.callee)
                || call.source != NativeCallOrigin::Authored
                || call.call_plan != target.call_plan
                || call.result_placement != target.call_plan.result
                || call
                    .result_placement
                    .as_ref()
                    .map(|placement| placement.shape)
                    != result_shape
                || actual != &semantic
                || actual_target != &target.argument
                || call.structural_result.is_some()
                || !call.descriptor_origins.is_empty()
                || !call.access_bindings.is_empty()
                || !call.claim_transfers.is_empty()
                || !call.requirement_obligations.is_empty()
                || !call.crash_continuations.is_empty()
            {
                return Err(Error::Mismatch(
                    "installation helper changed its role, target, borrow or ordinary ABI",
                ));
            }
            let instruction = find(&prepared.instructions, contract.instruction)
                .copied()
                .ok_or(Error::Mismatch(match role {
                    InstallationCallRole::Validate => {
                        "installation validation call has no unique selected instruction"
                    }
                    InstallationCallRole::Work => {
                        "installation work terminator has no unique selected instruction"
                    }
                }))?;
            let callee = selected_instructions::SelectedCallTarget::Machine(target.callee);
            let expected = match role {
                InstallationCallRole::Validate => {
                    SelectedInstructionKind::CallScalar { target: callee }
                }
                InstallationCallRole::Work => {
                    SelectedInstructionKind::InvokeNoNormal { target: callee }
                }
            };
            if instruction.kind != expected
                || instruction.provenance.operations.as_slice() != [*psi_operation]
                || !instruction.provenance.edges.is_empty()
            {
                return Err(Error::Mismatch(
                    "installation helper changed its source attribution or return behavior",
                ));
            }
        }
    }
    Ok(())
}
