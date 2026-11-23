use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection,
    ReferenceAccess, SuccessorEdge, TypeShape,
};

use super::machine;

/// What a parked field reborrow leads back to, read from published Psi alone.
#[derive(Debug, Eq, PartialEq)]
pub struct ParkedFieldReborrow {
    /// The incoming formal positions that supply the borrowed record: one per
    /// arriving edge in block order at a join, otherwise the single formal.
    pub incoming: Vec<usize>,
    /// Each descriptor stored in a reached record and crossed by `Referent`,
    /// named by its fields below the previous referent. The last one is the
    /// reborrow's immediate parent; with none, the incoming loan is the parent.
    pub crossings: Vec<Vec<String>>,
    /// The fields the reborrow selects below its parent's referent.
    pub projection: Vec<String>,
}

/// Follow the one loan held by `caller`'s suspended call to `callee` through
/// its defining borrow, a join's arriving edges and every stored crossing.
pub fn parked_field_reborrow(
    package: &LoweredPackage,
    caller: &str,
    callee: &str,
) -> Result<ParkedFieldReborrow, &'static str> {
    let caller = machine(package, caller).ok_or("missing caller")?;
    let callee = machine(package, callee).ok_or("missing callee")?;
    let body = &caller.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one suspended call");
    };
    let CallSite::Operation(call) = site.call else {
        return Err("expected an ordinary call");
    };
    let Some(OperationKind::Call { callee: target, arguments, .. }) = body.operations.get(call.index()).map(|op| &op.kind) else {
        return Err("missing call operation");
    };
    if *target != CallTarget::Machine(callee.id) || site.callee != *target || !callee.contract.realized_pause.suspend {
        return Err("wrong suspending callee");
    }
    let [
        CallArgument::Borrow {
            place: actual,
            access: ReferenceAccess::Shared,
        },
    ] = arguments.as_slice()
    else {
        return Err("expected one shared loan actual");
    };
    let [loan] = site.retained_loans.as_slice() else {
        return Err("expected one retained loan");
    };
    let place_ty = |p: PlaceId| body.places.get(p.index()).map(|place| place.ty);
    if loan.descriptor != actual.root
        || place_ty(loan.descriptor) != Some(loan.ty)
        || loan.access != ReferenceAccess::Shared
        || !loan.needed_by_callee
        || loan.needed_after_completion
    {
        return Err("the retained loan must be the borrowed actual's descriptor");
    }
    if !site.retained_owners.is_empty() || !site.activation.is_empty() {
        return Err("an incoming loan needs no owner or scalar in the activation");
    }

    // A named loan's actual crosses its own descriptor; its defining borrow captured the place.
    let (root, path) = match body.places[actual.root.index()].origin {
        PlaceOrigin::Operation(definition) => {
            let op = body
                .operations
                .get(definition.index())
                .ok_or("missing loan definition")?;
            let OperationKind::BorrowPlace {
                source,
                access: ReferenceAccess::Shared,
            } = &op.kind
            else {
                return Err("the named loan must come from a shared borrow");
            };
            if actual.path != [PlaceProjection::Referent] || op.result != OperationResult::Structural(actual.root) || callee.parameter_types != [loan.ty] {
                return Err("the named loan must pass its own descriptor");
            }
            (source.root, source.path.as_slice())
        }
        _ => (actual.root, actual.path.as_slice()),
    };

    let formal = |p: PlaceId| {
        body.parameters
            .iter()
            .position(|parameter| *parameter == Parameter::Structural(p))
            .ok_or("the supplied loan must be an incoming formal")
    };
    let incoming = match body.places[root.index()].origin {
        PlaceOrigin::Parameter => vec![formal(root)?],
        PlaceOrigin::BlockParameter(join) => {
            let slot = body
                .blocks
                .get(join.get() - 1)
                .and_then(|block| block.structural_parameters.iter().position(|p| *p == root))
                .ok_or("missing join parameter")?;
            let mut incoming = Vec::new();
            for block in &body.blocks {
                let edges: Vec<&SuccessorEdge> = match &block.terminator {
                    BlockTerminator::Jump(edge) => vec![edge],
                    BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true, when_false],
                    _ => Vec::new(),
                };
                for edge in edges.into_iter().filter(|edge| edge.target == join) {
                    match edge.structural_arguments.get(slot) {
                        Some(CallArgument::CopyWhole(p)) if place_ty(*p) == place_ty(root) => incoming.push(formal(*p)?),
                        _ => return Err("an arriving edge must copy an incoming loan of the joined type"),
                    }
                }
            }
            if incoming.len() < 2 {
                return Err("a join needs every arriving loan");
            }
            incoming
        }
        _ => return Err("the reborrow must start at an incoming loan"),
    };

    let Some(TypeShape::NominalReference { referent, .. }) = place_ty(root).and_then(|ty| package.types.get(ty)) else {
        return Err("the incoming loan must reference a record");
    };
    let [PlaceProjection::Referent, rest @ ..] = path else {
        return Err("the reborrow must cross the incoming loan first");
    };
    let mut subject = package
        .data
        .get(referent.get() - 1)
        .ok_or("missing referent schema")?
        .carrier;
    let (mut crossings, mut projection) = (Vec::new(), Vec::new());
    for step in rest {
        match (package.types.get(subject), step) {
            (Some(TypeShape::Nominal(record)), PlaceProjection::Field(field)) => {
                let schema = package
                    .data
                    .get(record.get() - 1)
                    .ok_or("missing record schema")?;
                subject = schema
                    .fields
                    .iter()
                    .find(|item| item.declaration == *field)
                    .ok_or("the field does not belong to the reached record")?
                    .carrier;
                projection.push(field.path.clone());
            }
            (Some(TypeShape::NominalReference { referent, .. }), PlaceProjection::Referent) if !projection.is_empty() => {
                subject = package
                    .data
                    .get(referent.get() - 1)
                    .ok_or("missing referent schema")?
                    .carrier;
                crossings.push(std::mem::take(&mut projection));
            }
            _ => return Err("each step must select a field of the reached record or cross a stored descriptor"),
        }
    }
    let Some(&formal_ty) = callee.parameter_types.first() else {
        return Err("expected the loan formal");
    };
    let Some(TypeShape::NominalReference {
        referent: token,
        access: ReferenceAccess::Shared,
    }) = package.types.get(formal_ty)
    else {
        return Err("expected a shared record formal");
    };
    if projection.is_empty() || package.types.get(subject) != Some(&TypeShape::Nominal(*token)) {
        return Err("the reborrow must select a field of the formal's record type");
    }
    Ok(ParkedFieldReborrow {
        incoming,
        crossings,
        projection,
    })
}
