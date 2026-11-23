//! Follow a completed reference result from its use after a parked call back
//! through its producing callee's actual return, including a copy into a field.
use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, EmbeddedLoanSource, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceOrigin,
    PlaceProjection, ReferenceAccess, ReturnEdge, SuspensionSite, TypeShape, ValueOperand,
};

pub fn parked_reference_inputs(
    package: &LoweredPackage,
    caller_name: &str,
    site: &SuspensionSite,
) -> Option<(Vec<usize>, ReferenceAccess)> {
    let caller = super::machine(package, caller_name)?;
    let body = &caller.body;
    let CallSite::Operation(wait) = site.call else { return None };
    let wait_op = body.operations.get(wait.index())?;
    if !matches!(&wait_op.kind, OperationKind::Call {callee, arguments, ..} if *callee == site.callee && arguments.is_empty()) {
        return None;
    }
    let (descriptor, ty, access) = match (site.retained_loans.as_slice(), site.retained_owners.as_slice()) {
        ([loan], []) if !loan.needed_by_callee && loan.needed_after_completion => {
            // The continuation actually dereferences the retained result.
            if !body.operations.get(wait.index() + 1..)?.iter().any(|op| {
                matches!(&op.kind,
                OperationKind::ProjectedScalarRead {source} if source.root == loan.descriptor && source.path.first() == Some(&PlaceProjection::Referent))
            }) {
                return None;
            }
            (loan.descriptor, loan.ty, loan.access)
        }
        ([], [owner]) if owner.moved.is_empty() && owner.custody == omega_psi::OwnedCustody::Caller => {
            let [loan] = owner.embedded_loans.as_slice() else { return None };
            let [PlaceProjection::Field(field)] = loan.path.as_slice() else { return None };
            let [origin] = loan.origins.as_slice() else { return None };
            let EmbeddedLoanSource::CallResult { operation, result, path } = &origin.source else {
                return None;
            };
            if !path.is_empty() || !origin.reborrows.is_empty() || !origin.arrival.is_empty() {
                return None;
            }
            let PlaceOrigin::Operation(setup) = body.places.get(owner.place.index())?.origin else {
                return None;
            };
            let establish = body.operations.get(setup.index())?;
            let OperationKind::EstablishRecord { fields } = &establish.kind else {
                return None;
            };
            if establish.result != OperationResult::Structural(owner.place)
                || !fields
                    .iter()
                    .any(|f| &f.field == field && f.operand == ValueOperand::CopyWhole(*result))
                || body.places.get(result.index())?.origin != PlaceOrigin::Operation(*operation)
                || !body.operations.get(wait.index() + 1..)?.iter().any(|op| {
                    matches!(&op.kind, OperationKind::MoveStructural {source} if source.root == owner.place && source.path.is_empty())
                        && body.blocks.iter().any(|b| {
                            matches!(b.terminator,
                        BlockTerminator::Return {result: ReturnEdge::Structural { source,.. },..} if OperationResult::Structural(source) == op.result)
                        })
                })
            {
                return None;
            }
            (*result, loan.ty, loan.access)
        }
        _ => return None,
    };
    let place = body.places.get(descriptor.index())?;
    if ty != place.ty || !matches!(package.types.get(ty)?, TypeShape::ScalarReference{access:a,..} | TypeShape::NominalReference{access:a,..} if *a == access) {
        return None;
    }
    let PlaceOrigin::Operation(producer) = place.origin else { return None };
    let call = body.operations.get(producer.index())?;
    let OperationKind::Call {
        callee: CallTarget::Machine(id),
        arguments,
        ..
    } = &call.kind
    else {
        return None;
    };
    if call.result != OperationResult::Structural(descriptor) || producer.index() >= wait.index() {
        return None;
    }
    let callee = package.machines.get(id.get() as usize - 1)?;
    if callee.result_type != Some(ty) {
        return None;
    }
    let mut inputs = Vec::new();
    for block in &callee.body.blocks {
        let source = match block.terminator {
            BlockTerminator::Return {
                result: ReturnEdge::Structural { source, .. },
                ..
            } => source,
            BlockTerminator::Conditional { .. } | BlockTerminator::Jump(_) => continue,
            _ => return None,
        };
        if callee.body.places.get(source.index())?.ty != ty {
            return None;
        }
        let slot = callee
            .body
            .parameters
            .iter()
            .position(|param| *param == Parameter::Structural(source))?;
        let actual = match arguments.get(slot)? {
            CallArgument::Borrow { place, access: supplied } if *supplied == access && place.path.as_slice() == [PlaceProjection::Referent] => place.root,
            CallArgument::CopyWhole(place) if access == ReferenceAccess::Shared => *place,
            CallArgument::MoveWhole(place) if access != ReferenceAccess::Shared => *place,
            _ => return None,
        };
        inputs.push(
            body.parameters
                .iter()
                .position(|param| *param == Parameter::Structural(actual))?,
        );
    }
    inputs.sort_unstable();
    inputs.dedup();
    (!inputs.is_empty()).then_some((inputs, access))
}
