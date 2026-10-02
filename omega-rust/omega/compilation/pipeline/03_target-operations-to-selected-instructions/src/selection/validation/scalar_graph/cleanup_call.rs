//! Independently replay each cleanup pointer, exact ABI row and action owner.
use super::{SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use optimization_unit::EffectLink;
use selected_instructions::SelectedCallContract;
use target_operations::{CallSiteOwner, NativeCallOrigin};
use terminal_psi::StructuralAccess;

pub(super) fn validate(
    owner: CallSiteOwner,
    call: &LegalizedScalarCall,
    effect: EffectLink,
    replay: &mut super::Replay<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = SelectedInstructionError::custody;
    let edge = owner.edge().ok_or_else(invalid)?;
    if call.source != NativeCallOrigin::Authored
        || call.result_placement.is_some()
        || call.structural_result.is_some()
        || !call.claim_transfers.is_empty()
        || !call.requirement_obligations.is_empty()
        || !call.crash_continuations.is_empty()
        || call.arguments.len() > 1
    {
        return Err(invalid());
    }
    let mut shapes = Vec::new();
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural {
            semantic,
            target,
            indices,
        } = argument
        else {
            return Err(invalid());
        };
        if !matches!(
            semantic.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || target.access != semantic.access
            || !semantic.path.is_empty()
            || !target.path.is_empty()
            || !indices.is_empty()
            || target.source_byte_offset != 0
            || target.shape != ValueShape::borrowed_reference(8, 8)
            || target.destination.shape != target.shape
        {
            return Err(invalid());
        }
        shapes.push(target.shape);
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(environment.target()),
        &CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if expected != call.call_plan
        || call
            .arguments
            .iter()
            .zip(&expected.parameters)
            .any(|(argument, placement)| argument.placement() != placement)
    {
        return Err(invalid());
    }
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
    let provenance = SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    };
    let mut operands = Vec::new();
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural { semantic, .. } = argument else {
            return Err(invalid());
        };
        operands.push(super::structural::place_pointer_at_site(
            replay,
            provenance.clone(),
            semantic.place,
            0,
        )?);
    }
    replay.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| invalid())?,
        ),
        owner,
        call: call.clone(),
        effect,
        ownership: Vec::new(),
    });
    replay.check_instruction(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &operands,
        &provenance,
    )?;
    Ok(())
}
