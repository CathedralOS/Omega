use omega_psi::{
    BlockTerminator, CallArgument, CallSite, CallTarget, LoweredPackage, OperationKind, OperationResult, OwnedCustody, Parameter, PlaceOrigin,
    PlaceProjection, ReferenceAccess, TypeShape,
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
    let backing = body
        .places
        .get(place.root.index())
        .ok_or("missing backing place")?;
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
    if held.value != *actual_tag || body.values.get(actual_tag.index()).map(|v| v.ty) != Some(held.ty) || !held.needed_by_callee || held.needed_after_completion
    {
        return Err("activation does not preserve the typed tag for the incomplete call");
    }
    Ok(())
}

/// How the owner row at a crossing covers the storage that one parked field
/// loan selects. Lowering publishes every case; the checker decides which
/// ones are admissible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParkedLoanBacking {
    /// The caller keeps the root and no moved path overlaps the loan.
    Intact,
    /// The caller keeps the root, but this moved path contains the loan or
    /// lies inside it.
    Moved(Vec<String>),
    /// The same call moves the whole root into this callee formal.
    Callee(usize),
}

/// One loan of owned storage that a suspended call or its continuation holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParkedFieldLoan {
    /// The call argument that carries the loan, if the callee receives it.
    pub slot: Option<usize>,
    /// Set for a named loan: whether the callee and the continuation need it.
    pub retained: Option<(bool, bool)>,
    pub access: ReferenceAccess,
    /// Field declarations from the owner's root to the selected place.
    pub path: Vec<String>,
    pub backing: ParkedLoanBacking,
}

/// Read every loan of caller-owned fields at the single crossing of `caller`
/// from the published call, retained loans and owner rows alone.
pub fn parked_field_loans(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<ParkedFieldLoan>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one suspended invocation");
    };
    let arguments = match site.call {
        CallSite::Operation(id) => match body.operations.get(id.index()).map(|op| &op.kind) {
            Some(OperationKind::Call { callee, arguments, .. }) if *callee == site.callee => arguments,
            _ => return Err("missing ordinary call"),
        },
        CallSite::Edge(id) => body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall {
                    id: edge, callee, arguments, ..
                } if *edge == id && *callee == site.callee => Some(arguments),
                _ => None,
            })
            .ok_or("missing terminal call")?,
        CallSite::Cleanup(_) => return Err("a cleanup invocation does not suspend"),
    };
    // A named loan leads through its defining borrow to the owned storage.
    let named_source = |descriptor: omega_psi::PlaceId| {
        let PlaceOrigin::Operation(id) = body.places.get(descriptor.index())?.origin else {
            return None;
        };
        let op = body.operations.get(id.index())?;
        match &op.kind {
            OperationKind::BorrowPlace { source, access } if op.result == OperationResult::Structural(descriptor) => Some((source, *access)),
            _ => None,
        }
    };
    let row = |place: omega_psi::PlaceId| site.retained_owners.iter().find(|owner| owner.place == place);
    let backing = |place: &omega_psi::ProjectedPlace, access: ReferenceAccess, slot, retained| {
        // A root moved out whole has no row of its own. Follow its move to the
        // place whose row names the callee formal that received it.
        let owner = match row(place.root) {
            Some(owner) => owner,
            None => {
                let destination = body
                    .operations
                    .iter()
                    .find_map(|op| match (&op.kind, op.result) {
                        (OperationKind::MoveStructural { source }, OperationResult::Structural(destination))
                            if source.root == place.root && source.path.is_empty() =>
                        {
                            Some(destination)
                        }
                        _ => None,
                    })
                    .ok_or("the loan's root has no owner row at the crossing")?;
                row(destination)
                    .filter(|owner| matches!(owner.custody, OwnedCustody::CalleeArgument(_)))
                    .ok_or("the moved root reaches no callee custody at the crossing")?
            }
        };
        if body.places.get(place.root.index()).map(|root| root.ty) != Some(owner.ty) {
            return Err("owner row type disagrees with the borrowed storage");
        }
        let path = names(&place.path)?;
        let backing = match owner.custody {
            OwnedCustody::CalleeArgument(formal) => {
                if arguments.get(formal) != Some(&CallArgument::MoveWhole(owner.place)) {
                    return Err("callee custody names an argument that does not move the owner");
                }
                ParkedLoanBacking::Callee(formal)
            }
            OwnedCustody::Caller => match owner
                .moved
                .iter()
                .find(|hole| hole.starts_with(&place.path) || place.path.starts_with(hole))
            {
                Some(hole) => ParkedLoanBacking::Moved(names(hole)?),
                None => ParkedLoanBacking::Intact,
            },
        };
        Ok(ParkedFieldLoan {
            slot,
            retained,
            access,
            path,
            backing,
        })
    };
    let mut loans = Vec::new();
    for (slot, argument) in arguments.iter().enumerate() {
        let CallArgument::Borrow { place, access } = argument else { continue };
        // A direct loan of storage, not a reborrow through a descriptor.
        let storage = body
            .places
            .get(place.root.index())
            .and_then(|root| package.types.get(root.ty));
        if named_source(place.root).is_none() && matches!(storage, Some(TypeShape::Nominal(_) | TypeShape::FixedArray { .. })) {
            loans.push(backing(place, *access, Some(slot), None)?);
        }
    }
    for loan in &site.retained_loans {
        let Some((source, access)) = named_source(loan.descriptor) else { continue };
        if access != loan.access || body.places.get(loan.descriptor.index()).map(|d| d.ty) != Some(loan.ty) {
            return Err("retained loan disagrees with its defining borrow");
        }
        let slot = arguments.iter().position(
            |argument| matches!(argument, CallArgument::Borrow { place, .. } if place.root == loan.descriptor && place.path == [PlaceProjection::Referent]),
        );
        if slot.is_some() != loan.needed_by_callee {
            return Err("retained loan disagrees with the call operand");
        }
        loans.push(backing(source, access, slot, Some((loan.needed_by_callee, loan.needed_after_completion)))?);
    }
    Ok(loans)
}

fn names(path: &[PlaceProjection]) -> Result<Vec<String>, &'static str> {
    path.iter()
        .map(|step| match step {
            PlaceProjection::Field(field) => Ok(field.path.clone()),
            _ => Err("expected only field projections of owned storage"),
        })
        .collect()
}
