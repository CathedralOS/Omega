//! Follow the parked record's return through a local identity call to the
//! caller's original borrow, and check both storage rosters.
use omega_psi::{
    BlockTerminator, CallRegion, CallSite, CallTarget, EmbeddedLoanSource, LoweredPackage, MachineBody, OperationKind, OperationResult, OwnedCustody,
    Parameter, PlaceId, PlaceOrigin, PlaceProjection, ReferenceAccess, ReturnEdge, SuspensionSite, ValueOperand,
};

pub fn returned_aggregate_backing(
    package: &LoweredPackage,
    caller: &str,
    relay: &str,
    wait: &str,
) -> Option<usize> {
    let caller = super::machine(package, caller)?;
    let relay = super::machine(package, relay)?;
    let wait = super::machine(package, wait)?;
    let body = &caller.body;
    let [parameter] = relay.body.parameters.as_slice() else { return None };
    let Parameter::Structural(formal) = *parameter else { return None };
    let [block] = relay.body.blocks.as_slice() else { return None };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, .. },
        ..
    } = block.terminator
    else {
        return None;
    };
    if !matches!(&definition(&relay.body, source)?.kind,
        OperationKind::MoveStructural { source } if source.root == formal && source.path.is_empty())
    {
        return None;
    }
    let [first, second] = body.suspension_sites.as_slice() else { return None };
    if first.callee != CallTarget::Machine(relay.id) || second.callee != CallTarget::Machine(wait.id) {
        return None;
    }
    let (CallSite::Operation(relay_call), CallSite::Operation(wait_call)) = (first.call, second.call) else {
        return None;
    };
    let call = body.operations.get(relay_call.index())?;
    let (OperationKind::Call { callee, arguments, .. }, OperationResult::Structural(result)) = (&call.kind, call.result) else {
        return None;
    };
    if *callee != first.callee
        || !matches!(
            body.call_regions
                .iter()
                .find(|row| row.call == first.call)?
                .arguments
                .as_slice(),
            [CallRegion::Operands]
        )
    {
        return None;
    }
    let [ValueOperand::MoveWhole(actual)] = arguments.as_slice() else {
        return None;
    };
    let moved = definition(body, *actual)?;
    let OperationKind::MoveStructural { source } = &moved.kind else { return None };
    if !source.path.is_empty() {
        return None;
    }
    let OperationKind::EstablishRecord { fields } = &definition(body, source.root)?.kind else {
        return None;
    };
    let [field] = fields.as_slice() else { return None };
    let ValueOperand::CopyWhole(loan) = field.operand else { return None };
    let origin = definition(body, loan)?;
    let OperationKind::BorrowPlace {
        source,
        access: ReferenceAccess::Shared,
    } = &origin.kind
    else {
        return None;
    };
    let [PlaceProjection::Field(backing_field)] = source.path.as_slice() else {
        return None;
    };
    if backing_field.path != "Home::v" || field.field.path != "Box::r" {
        return None;
    }
    let backing = body
        .parameters
        .iter()
        .position(|formal| *formal == Parameter::Structural(source.root))?;

    let [incoming] = first.retained_owners.as_slice() else { return None };
    let [outgoing] = second.retained_owners.as_slice() else { return None };
    if incoming.place != *actual || incoming.custody != OwnedCustody::CalleeArgument(0) || outgoing.place != result || outgoing.custody != OwnedCustody::Caller
    {
        return None;
    }
    let (path, input_origin) = only_loan(incoming)?;
    let (result_path, output_origin) = only_loan(outgoing)?;
    if path != [PlaceProjection::Field(field.field.clone())]
        || path != result_path
        || *input_origin != (EmbeddedLoanSource::Borrow { operation: origin.id })
        || *output_origin
            != (EmbeddedLoanSource::CallResult {
                operation: relay_call,
                result,
                path: path.to_vec(),
            })
    {
        return None;
    }
    let read = body.operations.get(wait_call.index() + 1)?;
    let OperationKind::ProjectedScalarRead { source: read_source, .. } = &read.kind else {
        return None;
    };
    if read_source.root != result || read_source.path != [path[0].clone(), PlaceProjection::Referent] {
        return None;
    }
    (roster(body, first, source.root) == Some((true, false)) && roster(body, second, source.root) == Some((false, true))).then_some(backing)
}

fn definition(
    body: &MachineBody,
    place: PlaceId,
) -> Option<&omega_psi::PsiOperation> {
    let PlaceOrigin::Operation(id) = body.places.get(place.index())?.origin else {
        return None;
    };
    let operation = body.operations.get(id.index())?;
    (operation.result == OperationResult::Structural(place)).then_some(operation)
}

fn only_loan(owner: &omega_psi::RetainedOwner) -> Option<(&[PlaceProjection], &EmbeddedLoanSource)> {
    let [loan] = owner.embedded_loans.as_slice() else { return None };
    let [origin] = loan.origins.as_slice() else { return None };
    (loan.access == ReferenceAccess::Shared && origin.arrival.is_empty() && origin.reborrows.is_empty()).then_some((&loan.path, &origin.source))
}

fn roster(
    body: &MachineBody,
    site: &SuspensionSite,
    root: PlaceId,
) -> Option<(bool, bool)> {
    let [home] = site.retained_homes.as_slice() else { return None };
    (home.place == root && home.ty == body.places.get(root.index())?.ty && home.embedded_loans.is_empty())
        .then_some((home.needed_by_callee, home.needed_after_completion))
}
