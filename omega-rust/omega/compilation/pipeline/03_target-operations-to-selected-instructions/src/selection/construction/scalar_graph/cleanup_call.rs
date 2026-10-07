//! Select edge-owned calls with ordinary borrowed-pointer ABI transport.
use super::{
    Builder, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use calling_conventions::{CallSignature, CallingPolicy, ValueClass, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use optimization_unit::EffectLink;
use selected_instructions::SelectedCallContract;
use target_operations::{CallSiteOwner, NativeCallOrigin};
use terminal_psi::StructuralAccess;

pub(super) fn emit(
    owner: CallSiteOwner,
    call: &LegalizedScalarCall,
    effect: EffectLink,
    builder: &mut Builder<'_>,
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
            || target.place != semantic.place
            || target.path != semantic.path
            || !indices.is_empty()
            || target.shape.class != ValueClass::BorrowedReference
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
        let LegalizedScalarArgument::Structural {
            semantic, target, ..
        } = argument
        else {
            return Err(invalid());
        };
        operands.push(super::structural::place_pointer_at_site(
            builder,
            provenance.clone(),
            semantic.place,
            target.source_byte_offset,
        )?);
    }
    builder.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        owner,
        call: call.clone(),
        effect,
        ownership: Vec::new(),
    });
    builder.emit(
        SelectedInstructionKind::CallUnit {
            target: selected_instructions::SelectedCallTarget::from_native(call.target),
        },
        key,
        &operands,
        provenance,
    )?;
    Ok(())
}
