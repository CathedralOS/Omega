use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, LoweredPackage, MachineId, OperationKind, OperationResult, Parameter, PlaceOrigin, ReferenceAccess,
    ReturnEdge, TypeShape,
};

use super::machine;

/// Trace a parked call argument back through the completed call that returned its view.
pub fn returned_view_backing(
    package: &LoweredPackage,
    caller: &str,
    parking_callee: &str,
    returning_callee: &str,
) -> Result<(usize, usize), &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let parking_callee = machine(package, parking_callee).ok_or("missing parking callee")?;
    let returning_callee = machine(package, returning_callee).ok_or("missing returning callee")?;
    let body = &caller.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    let [loan] = site.retained_loans.as_slice() else {
        return Err("expected one retained loan");
    };
    if loan.access != ReferenceAccess::Shared
        || !loan.needed_by_callee
        || loan.needed_after_completion
        || !matches!(
            package.types.get(loan.ty),
            Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. })
        )
    {
        return Err("wrong retained loan access, role or type");
    }

    let call_arguments = |id: usize, target: MachineId| {
        let operation = body.operations.get(id)?;
        match &operation.kind {
            OperationKind::Call {
                callee: CallTarget::Machine(callee),
                arguments,
                ..
            } if *callee == target => Some(arguments.as_slice()),
            _ => None,
        }
    };
    let CallSite::Operation(parking_call) = site.call else {
        return Err("expected a parked operation");
    };
    if site.callee != CallTarget::Machine(parking_callee.id)
        || !parking_callee.contract.realized_pause.suspend
        || parking_callee.parameter_types.as_slice() != [loan.ty]
        || call_arguments(parking_call.index(), parking_callee.id) != Some([CallArgument::CopyWhole(loan.descriptor)].as_slice())
    {
        return Err("the parked call must use the retained result");
    }

    let place = body
        .places
        .get(loan.descriptor.index())
        .ok_or("missing retained descriptor")?;
    let PlaceOrigin::Operation(completed_call) = place.origin else {
        return Err("expected a retained descriptor returned from an operation");
    };
    if completed_call.index() >= parking_call.index()
        || place.ty != loan.ty
        || returning_callee.contract.realized_pause.suspend
        || body
            .operations
            .get(completed_call.index())
            .map(|operation| operation.result)
            != Some(OperationResult::Structural(loan.descriptor))
    {
        return Err("the returning call must complete before the parked call");
    }
    let arguments = call_arguments(completed_call.index(), returning_callee.id).ok_or("missing completed local call")?;

    let [return_block] = returning_callee.body.blocks.as_slice() else {
        return Err("expected one normal return block");
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        ..
    } = return_block.terminator
    else {
        return Err("expected a structural normal return");
    };
    if returning_callee
        .body
        .places
        .get(result.index())
        .map(|place| (place.origin, place.ty))
        != Some((PlaceOrigin::Result, loan.ty))
    {
        return Err("wrong returned descriptor type");
    }
    let formal = returning_callee
        .body
        .parameters
        .iter()
        .position(|parameter| *parameter == Parameter::Structural(source))
        .ok_or("return must name a formal descriptor")?;
    if returning_callee.parameter_types.get(formal) != Some(&loan.ty) {
        return Err("wrong formal descriptor type");
    }
    let Some(CallArgument::CopyWhole(actual)) = arguments.get(formal) else {
        return Err("the returned formal must have a shared descriptor actual");
    };
    let backing = body
        .parameters
        .iter()
        .position(|parameter| *parameter == Parameter::Structural(*actual))
        .ok_or("the selected actual must be an incoming descriptor")?;
    if caller.parameter_types.get(backing) != Some(&loan.ty) {
        return Err("wrong incoming descriptor type");
    }
    Ok((formal, backing))
}
