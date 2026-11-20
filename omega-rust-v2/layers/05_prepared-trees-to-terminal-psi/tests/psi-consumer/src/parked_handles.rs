use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CleanupAction, CompletionContinuation, LoweredPackage, OperationKind, OperationResult, OwnedCustody,
    Parameter, PlaceOrigin, TypeShape,
};

use super::machine;

/// Check that a parked call transfers one handle and leaves only its sibling for caller cleanup.
pub fn check_parked_handles(
    package: &LoweredPackage,
    moved_formal: usize,
    terminal: bool,
) -> Result<(), &'static str> {
    let caller = machine(package, "forward").ok_or("missing caller")?;
    let callee = machine(package, "step").ok_or("missing suspending callee")?;
    let release = machine(package, "release").ok_or("missing cleanup machine")?;
    let body = &caller.body;
    let [Parameter::Structural(left), Parameter::Structural(right)] = body.parameters.as_slice() else {
        return Err("expected two owned formals");
    };
    let roots = [*left, *right];
    let moved = roots[moved_formal];
    let kept = roots[1 - moved_formal];
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    if site.callee != CallTarget::Machine(callee.id) || !callee.contract.realized_pause.suspend {
        return Err("expected the selected suspending machine");
    }

    let (callee_target, arguments, cleanup, call_position) = match site.call {
        CallSite::Operation(id) if !terminal => {
            let call = body
                .operations
                .get(id.index())
                .ok_or("missing call operation")?;
            let OperationKind::Call { callee, arguments, .. } = &call.kind else {
                return Err("expected ordinary invocation");
            };
            let OperationResult::Scalar(result) = call.result else {
                return Err("expected scalar completion");
            };
            if site.activation.iter().any(|row| row.value == result)
                || !matches!(site.continuation, CompletionContinuation::Operation(next) if next.index() == id.index() + 1)
            {
                return Err("completion result must stay outside the parked activation");
            }
            let Some(BlockTerminator::Return { cleanup, .. }) = body.blocks.last().map(|block| &block.terminator) else {
                return Err("expected normal return with cleanup");
            };
            (*callee, arguments, cleanup, id.index())
        }
        CallSite::Edge(id) if terminal => {
            let (block, target, arguments, cleanup) = body
                .blocks
                .iter()
                .find_map(|block| match &block.terminator {
                    BlockTerminator::TerminalCall {
                        id: edge,
                        callee,
                        arguments,
                        cleanup,
                        ..
                    } if *edge == id => Some((block, *callee, arguments, cleanup)),
                    _ => None,
                })
                .ok_or("expected terminal invocation")?;
            if site.continuation != CompletionContinuation::Return {
                return Err("terminal call must resume to normal return");
            }
            (target, arguments, cleanup, block.operations.end)
        }
        _ => return Err("wrong call form"),
    };
    if callee_target != site.callee
        || body
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Call { callee:CallTarget::Machine(id),.. } if id==release.id))
        || body.blocks.iter().any(|block| match &block.terminator {
            BlockTerminator::Jump(edge) => !edge.cleanup.is_empty(),
            BlockTerminator::Conditional { when_true, when_false, .. } => !when_true.cleanup.is_empty() || !when_false.cleanup.is_empty(),
            _ => false,
        })
    {
        return Err("the fixture must leave caller cleanup on its completion edge");
    }
    let [CallArgument::MoveWhole(transferred)] = arguments.as_slice() else {
        return Err("expected a transferred argument");
    };
    let descriptor = body
        .places
        .get(transferred.index())
        .ok_or("missing transferred place")?;
    let PlaceOrigin::Operation(transfer) = descriptor.origin else {
        return Err("expected a move from an incoming owner");
    };
    if transfer.index() >= call_position
        || !matches!(
            &body.operations[transfer.index()].kind,
            OperationKind::MoveStructural { source } if source.root == moved && source.path.is_empty()
        )
    {
        return Err("wrong input moved before parking");
    }

    let [CleanupAction::Invoke { place, ty, target }] = cleanup.as_slice() else {
        return Err("expected a single cleanup hook on normal completion");
    };
    let Some(TypeShape::Nominal(nominal)) = package.types.get(*ty) else {
        return Err("cleanup must name the nominal handle type");
    };
    let handle = package
        .data
        .iter()
        .find(|data| data.id == *nominal)
        .ok_or("missing handle declaration")?;
    if place.root != kept
        || !place.path.is_empty()
        || *target != release.id
        || handle.declaration.path != "Handle"
        || handle.carrier != *ty
        || handle.carry.is_some()
        || caller.parameter_types.as_slice() != [*ty, *ty]
        || callee.parameter_types.as_slice() != [*ty]
        || descriptor.ty != *ty
    {
        return Err("wrong surviving handle, type, carry or cleanup");
    }
    if site.retained_owners.len() != 2
        || !site
            .retained_owners
            .iter()
            .any(|row| row.place == kept && row.custody == OwnedCustody::Caller)
        || !site
            .retained_owners
            .iter()
            .any(|row| row.place == *transferred && row.custody == OwnedCustody::CalleeArgument(0))
        || site
            .retained_owners
            .iter()
            .any(|row| row.ty != *ty || !row.moved.is_empty())
    {
        return Err("parked custody must name the exact two whole handles");
    }
    Ok(())
}
