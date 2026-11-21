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

/// Name every incoming formal of `caller` that can back the one view retained
/// at its parked call, reading only the published Psi. The view comes from a
/// completed call to `returning`, which may call itself, so its returned
/// formals cannot be read off one return edge. Start every view-returning
/// machine at its formals of the result's type and keep the formals that a
/// normal or terminal return reaches through the current sets. This consumer
/// follows returns of a formal, of an ordinary call result and of a terminal
/// call, and rejects any other form instead of guessing.
pub fn recursive_view_backings(
    package: &LoweredPackage,
    caller: &str,
    returning: &str,
) -> Result<Vec<usize>, &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let returning = machine(package, returning).ok_or("missing returning callee")?;
    let [site] = caller.body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    let [loan] = site.retained_loans.as_slice() else {
        return Err("expected one retained loan");
    };
    if loan.access != ReferenceAccess::Shared || !loan.needed_by_callee || loan.needed_after_completion {
        return Err("wrong retained loan access or role");
    }
    let place = caller
        .body
        .places
        .get(loan.descriptor.index())
        .ok_or("missing retained descriptor")?;
    let PlaceOrigin::Operation(completed) = place.origin else {
        return Err("expected a retained descriptor returned from an operation");
    };
    let operation = caller
        .body
        .operations
        .get(completed.index())
        .ok_or("missing completed call")?;
    let OperationKind::Call {
        callee: CallTarget::Machine(target),
        arguments,
        ..
    } = &operation.kind
    else {
        return Err("expected a completed local call");
    };
    if *target != returning.id || operation.result != OperationResult::Structural(loan.descriptor) || returning.result_type != Some(loan.ty) {
        return Err("the retained view must be the result of the returning call");
    }

    let views = |m: &omega_psi::LoweredMachine| -> Vec<usize> {
        let Some(result) = m.result_type else { return Vec::new() };
        if !matches!(package.types.get(result), Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. })) {
            return Vec::new();
        }
        (m.body.parameters.iter().enumerate())
            .filter(|(_, p)| matches!(p, Parameter::Structural(f) if m.body.places[f.index()].ty == result))
            .map(|(slot, _)| slot)
            .collect()
    };
    let mut sets: Vec<Vec<usize>> = package.machines.iter().map(views).collect();
    let index = |id: MachineId| (id.get() - 1) as usize;
    // Map a call's actual arguments at the callee's current set to the caller's formals.
    let through = |sets: &[Vec<usize>], m: &omega_psi::LoweredMachine, callee: MachineId, args: &[CallArgument], out: &mut Vec<usize>| {
        for &slot in &sets[index(callee)] {
            let Some(CallArgument::CopyWhole(actual)) = args.get(slot) else {
                return Err("a returned formal needs a shared descriptor actual");
            };
            let formal = m
                .body
                .parameters
                .iter()
                .position(|p| *p == Parameter::Structural(*actual));
            out.push(formal.ok_or("the actual must be an incoming descriptor")?);
        }
        Ok(())
    };
    loop {
        let mut changed = false;
        for m in &package.machines {
            if sets[index(m.id)].is_empty() {
                continue;
            }
            let mut next = Vec::new();
            for block in &m.body.blocks {
                match &block.terminator {
                    BlockTerminator::Return {
                        result: ReturnEdge::Structural { source, .. },
                        ..
                    } => match m.body.places[source.index()].origin {
                        PlaceOrigin::Parameter => next.push(
                            m.body
                                .parameters
                                .iter()
                                .position(|p| *p == Parameter::Structural(*source))
                                .ok_or("missing formal")?,
                        ),
                        PlaceOrigin::Operation(id) => match &m.body.operations[id.index()].kind {
                            OperationKind::Call {
                                callee: CallTarget::Machine(callee),
                                arguments,
                                ..
                            } => through(&sets, m, *callee, arguments, &mut next)?,
                            _ => return Err("unsupported returned view operation"),
                        },
                        _ => return Err("unsupported returned view origin"),
                    },
                    BlockTerminator::TerminalCall {
                        callee: CallTarget::Machine(callee),
                        arguments,
                        ..
                    } => through(&sets, m, *callee, arguments, &mut next)?,
                    BlockTerminator::Conditional { .. } | BlockTerminator::Jump(_) => {}
                    _ => return Err("unsupported view-returning terminator"),
                }
            }
            next.sort_unstable();
            next.dedup();
            if next.iter().any(|slot| !sets[index(m.id)].contains(slot)) {
                return Err("a return reached a formal outside the result's view type");
            }
            if next != sets[index(m.id)] {
                sets[index(m.id)] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut backings = Vec::new();
    through(&sets, caller, returning.id, arguments, &mut backings)?;
    backings.sort_unstable();
    Ok(backings)
}
