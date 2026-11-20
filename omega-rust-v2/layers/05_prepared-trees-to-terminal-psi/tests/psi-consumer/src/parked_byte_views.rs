use omega_psi::{
    CallArgument, CallSite, CallTarget, CompletionContinuation, LoweredPackage, OperationKind, Parameter, PlaceOrigin, ReferenceAccess, SuspensionSite,
    TypeShape,
};

use super::machine;

/// Check the incoming descriptors against the call and the first resumed read.
pub fn check_parked_byte_views(
    package: &LoweredPackage,
    caller: &str,
    callee: &str,
    selected_formal: usize,
    resumed_formal: usize,
    site: &SuspensionSite,
) -> Result<(), &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let callee = machine(package, callee).ok_or("missing callee")?;
    if site.callee != CallTarget::Machine(callee.id) || !callee.contract.realized_pause.suspend {
        return Err("wrong suspending callee");
    }

    let body = &caller.body;
    let formal = |index| match body.parameters.get(index) {
        Some(Parameter::Structural(id)) => Ok(*id),
        _ => Err("missing structural formal"),
    };
    let selected = formal(selected_formal)?;
    let resumed = formal(resumed_formal)?;
    if selected == resumed {
        return Err("expected two distinct incoming descriptors");
    }

    let CallSite::Operation(call) = site.call else {
        return Err("expected an ordinary call");
    };
    let Some(OperationKind::Call { callee: target, arguments, .. }) = body.operations.get(call.index()).map(|op| &op.kind) else {
        return Err("missing call operation");
    };
    if *target != site.callee || arguments.as_slice() != [CallArgument::CopyWhole(selected)] {
        return Err("call selected the wrong incoming descriptor");
    }

    let CompletionContinuation::Operation(next) = site.continuation else {
        return Err("expected a resumed operation");
    };
    if next.index() != call.index() + 1
        || !matches!(
            body.operations.get(next.index()).map(|op| &op.kind),
            Some(OperationKind::ByteSequenceLength { source }) if *source == resumed
        )
    {
        return Err("continuation reads the wrong incoming descriptor");
    }

    let [first, second] = site.retained_loans.as_slice() else {
        return Err("expected two retained loans");
    };
    if first.descriptor.index() >= second.descriptor.index() {
        return Err("retained loans must be distinct and ordered");
    }
    for (formal_index, descriptor, by_callee) in [
        (selected_formal, selected, true),
        (resumed_formal, resumed, false),
    ] {
        let loan = [first, second]
            .into_iter()
            .find(|loan| loan.descriptor == descriptor)
            .ok_or("retained loan names the wrong incoming descriptor")?;
        let place = body
            .places
            .get(descriptor.index())
            .ok_or("missing descriptor")?;
        if place.origin != PlaceOrigin::Parameter
            || caller.parameter_types.get(formal_index) != Some(&loan.ty)
            || loan.ty != place.ty
            || callee.parameter_types.as_slice() != [loan.ty]
            || package.types.get(loan.ty) != Some(&TypeShape::SharedByteView)
            || loan.access != ReferenceAccess::Shared
        {
            return Err("wrong incoming loan type or access");
        }
        if loan.needed_by_callee != by_callee || loan.needed_after_completion == by_callee {
            return Err("retained loan has the wrong role");
        }
    }
    Ok(())
}
