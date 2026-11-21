//! Read only the public package: no prepared tree, source sites or provider body.
use omega_lowering::*;

pub fn machine<'a>(pkg: &'a LoweredPackage, name: &str) -> &'a LoweredMachine {
    pkg.machines.iter().find(|m| m.declaration.path == name).unwrap()
}

pub fn parked_view(pkg: &LoweredPackage, body: &MachineBody) -> PlaceId {
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    let [loan] = site.retained_loans.as_slice() else { panic!("one live descriptor") };
    let CallSite::Operation(call) = site.call else { panic!("ordinary crossing") };
    let OperationKind::Call { callee, arguments, .. } = &body.operations[call.index()].kind else { panic!("parked call") };
    assert_eq!(site.callee, CallTarget::Machine(machine(pkg, "inspect").id));
    assert_eq!(*callee, site.callee);
    assert_eq!(arguments.as_slice(), [CallArgument::CopyWhole(loan.descriptor)]);
    assert_eq!(loan.ty, body.places[loan.descriptor.index()].ty);
    assert_eq!(pkg.types.get(loan.ty), Some(&TypeShape::SharedByteView));
    assert_eq!(loan.access, ReferenceAccess::Shared);
    assert!(loan.needed_by_callee && !loan.needed_after_completion);
    loan.descriptor
}

pub fn producing_call(body: &MachineBody, result: PlaceId) -> (CallTarget, &[CallArgument]) {
    let PlaceOrigin::Operation(id) = body.places[result.index()].origin else { panic!("expected a completed operation") };
    let op = &body.operations[id.index()];
    assert_eq!((op.id, op.result), (id, OperationResult::Structural(result)));
    let OperationKind::Call { callee, arguments, .. } = &op.kind else { panic!("expected call result") };
    (*callee, arguments)
}

/// The reference is the completed call's result. Its lifetime is related to
/// the selected operand; the provider need not return bytes inside that input.
pub fn lifetime_actual(pkg: &LoweredPackage, body: &MachineBody, result: PlaceId) -> (usize, PlaceId) {
    let (CallTarget::Member(slot), actuals) = producing_call(body, result) else { panic!("boundary requirement") };
    let selection = &pkg.member_calls[slot];
    let device = pkg.traits.iter().find(|t| t.boundary && t.declaration.path == "Device").unwrap();
    let view = device.requirements.iter().find(|m| m.declaration.path == "Device::view").unwrap();
    assert_eq!(selection.owner.declaration, device.declaration);
    assert_eq!(selection.requirement.declaration, view.declaration);
    assert_eq!(selection.target, MemberRealization::Boundary { direction: None });
    assert_eq!(selection.result, Some(body.places[result.index()].ty));
    assert_eq!(selection.parameters, view.parameter_types);
    let anchor = selection.boundary_result_lifetime_input.expect("input-related result lifetime");
    let CallArgument::CopyWhole(actual) = actuals[anchor] else { panic!("expected lifetime anchor") };
    assert_ne!(actual, result);
    (anchor, actual)
}
