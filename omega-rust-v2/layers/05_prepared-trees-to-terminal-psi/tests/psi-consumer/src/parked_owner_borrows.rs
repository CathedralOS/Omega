use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, LoweredPackage, OperationKind, OwnedCustody, Parameter, PlaceProjection, ReferenceAccess, TypeShape,
};

use super::machine;

/// Recover the owned backing and the exact field loan from the published call.
pub fn check_parked_owner_borrow(
    package: &LoweredPackage,
    field_path: &str,
    terminal: bool,
) -> Result<(), &'static str> {
    let caller = machine(package, "forward").ok_or("missing caller")?;
    let callee = machine(package, "inspect").ok_or("missing callee")?;
    let body = &caller.body;
    let [Parameter::Structural(owner), Parameter::Scalar(tag)] = body.parameters.as_slice() else {
        return Err("expected an owned record and a scalar tag");
    };
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one suspended invocation");
    };
    let (target, arguments) = match site.call {
        CallSite::Operation(id) if !terminal => match body.operations.get(id.index()).map(|op| &op.kind) {
            Some(OperationKind::Call { callee, arguments, .. }) => (*callee, arguments),
            _ => return Err("missing ordinary call"),
        },
        CallSite::Edge(id) if terminal => body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall {
                    id: edge, callee, arguments, ..
                } if *edge == id => Some((*callee, arguments)),
                _ => None,
            })
            .ok_or("missing terminal call")?,
        _ => return Err("wrong call form"),
    };
    if target != CallTarget::Machine(callee.id) || site.callee != target || !callee.contract.realized_pause.suspend {
        return Err("wrong suspending callee");
    }

    let [
        CallArgument::Scalar(actual_tag),
        CallArgument::Borrow { place, access },
    ] = arguments.as_slice()
    else {
        return Err("borrow must occupy the second formal slot");
    };
    if actual_tag != tag || place.root != *owner || *access != ReferenceAccess::Shared {
        return Err("wrong tag, loan access or backing owner");
    }
    let [PlaceProjection::Field(field)] = place.path.as_slice() else {
        return Err("expected a direct field projection of owned storage");
    };
    if field.path != field_path {
        return Err("the parked borrow selects a different sibling");
    }

    let [retained] = site.retained_owners.as_slice() else {
        return Err("expected one retained backing owner");
    };
    if retained.place != *owner
        || retained.custody != OwnedCustody::Caller
        || !retained.moved.is_empty()
        || caller.parameter_types.first() != Some(&retained.ty)
        || !site.retained_loans.is_empty()
    {
        return Err("the intact root must remain in caller custody");
    }
    let backing = body.places.get(place.root.index()).ok_or("missing backing place")?;
    if backing.ty != retained.ty {
        return Err("retained owner type disagrees with the borrowed storage");
    }
    let Some(TypeShape::Nominal(pair_id)) = package.types.get(backing.ty) else {
        return Err("expected nominal owner type");
    };
    let pair = package
        .data
        .get(pair_id.get() - 1)
        .ok_or("missing owner schema")?;
    let field = pair
        .fields
        .iter()
        .find(|item| item.declaration == *field)
        .ok_or("field does not belong to the owner")?;
    let [_, formal] = callee.parameter_types.as_slice() else {
        return Err("expected the shared loan formal");
    };
    let Some(TypeShape::NominalReference { referent, access }) = package.types.get(*formal) else {
        return Err("expected nominal reference type");
    };
    if *access != ReferenceAccess::Shared || package.types.get(field.carrier) != Some(&TypeShape::Nominal(*referent)) {
        return Err("formal does not borrow the exact type of the selected field");
    }
    // In both forms inspect holds tag; only its eventual result flows onward.
    let [held] = site.activation.as_slice() else {
        return Err("expected only the scalar tag in the parked scalar frontier");
    };
    if held.value != *actual_tag
        || body.values.get(actual_tag.index()).map(|v| v.ty) != Some(held.ty)
        || !held.needed_by_callee
        || held.needed_after_completion
    {
        return Err("activation does not preserve the typed tag for the incomplete call");
    }
    Ok(())
}
