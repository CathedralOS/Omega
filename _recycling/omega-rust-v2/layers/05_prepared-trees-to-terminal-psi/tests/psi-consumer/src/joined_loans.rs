use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CompletionContinuation, LoweredPackage, OperationKind, Parameter, PlaceOrigin, ReferenceAccess,
    TypeShape,
};

use super::machine;

/// Resolve the retained view through the true and false edges supplying its join parameter.
pub fn check_joined_loan(
    package: &LoweredPackage,
    caller: &str,
    callee: &str,
    branch_formals: [usize; 2],
) -> Result<(), &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let callee = machine(package, callee).ok_or("missing callee")?;
    let body = &caller.body;
    let [
        Parameter::Scalar(flag),
        Parameter::Structural(left),
        Parameter::Structural(right),
    ] = body.parameters.as_slice()
    else {
        return Err("expected a flag and two view formals");
    };
    if left == right {
        return Err("the formals must have distinct descriptors");
    }
    let Some(BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    }) = body.blocks.first().map(|block| &block.terminator)
    else {
        return Err("expected a Boolean dispatch");
    };
    if condition != flag || when_true.target == when_false.target {
        return Err("expected distinct successors selected by the flag");
    }
    let jumps = [when_true, when_false].map(|edge| match &body.blocks.get(edge.target.get() - 1)?.terminator {
        BlockTerminator::Jump(jump) => Some(jump),
        _ => None,
    });
    let [Some(from_true), Some(from_false)] = jumps else {
        return Err("both branches must jump to the continuation");
    };
    if from_true.target != from_false.target {
        return Err("branches must feed the same join");
    }
    for (edge, formal) in [from_true, from_false].into_iter().zip(branch_formals) {
        let Some(Parameter::Structural(descriptor)) = body.parameters.get(formal) else {
            return Err("expected an incoming view formal");
        };
        if edge.structural_arguments.as_slice() != [CallArgument::CopyWhole(*descriptor)] {
            return Err("branch supplies the wrong incoming descriptor");
        }
    }

    let join = body
        .blocks
        .get(from_true.target.get() - 1)
        .ok_or("missing join")?;
    let [joined] = join.structural_parameters.as_slice() else {
        return Err("expected one joined view");
    };
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one suspended call");
    };
    let CallSite::Operation(call) = site.call else {
        return Err("expected an ordinary call");
    };
    let Some(OperationKind::Call { callee: target, arguments, .. }) = body.operations.get(call.index()).map(|op| &op.kind) else {
        return Err("missing call operation");
    };
    if !join.operations.contains(&call.index())
        || *target != CallTarget::Machine(callee.id)
        || *target != site.callee
        || !callee.contract.realized_pause.suspend
        || arguments.as_slice() != [CallArgument::CopyWhole(*joined)]
    {
        return Err("the suspended call must use the joined descriptor");
    }
    if !matches!(site.continuation, CompletionContinuation::Operation(next) if join.operations.contains(&next.index()) && next.index() == call.index() + 1) {
        return Err("expected the next operation in the join after resumption");
    }

    let [loan] = site.retained_loans.as_slice() else {
        return Err("expected one retained loan");
    };
    if loan.descriptor != *joined {
        return Err("the retained loan must name the joined descriptor");
    }
    let place = body.places.get(joined.index()).ok_or("missing descriptor")?;
    if place.origin != PlaceOrigin::BlockParameter(join.id)
        || place.ty != loan.ty
        || caller.parameter_types.get(1..3) != Some(&[loan.ty, loan.ty])
        || callee.parameter_types.as_slice() != [loan.ty]
        || package.types.get(loan.ty) != Some(&TypeShape::SharedByteView)
        || loan.access != ReferenceAccess::Shared
        || !loan.needed_by_callee
        || loan.needed_after_completion
    {
        return Err("wrong joined loan type, access or role");
    }
    Ok(())
}
