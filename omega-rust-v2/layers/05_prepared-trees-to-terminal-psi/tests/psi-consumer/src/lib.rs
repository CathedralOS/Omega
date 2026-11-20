//! A downstream client of the owned Psi package. Its manifest is the dependency boundary.
use omega_psi::{
    DeclarationKind, FoldedLayout, FoldedSubject, FoldedTargetClosure, FoldedTargetObservation, LayoutExtent, LayoutGeometry, LoweredPackage, ReceivingTarget,
    TargetIdentity, TypeShape,
};

mod assembly;
mod trapping;
pub use assembly::assembly_branch;
pub use trapping::{DivisionOutcome, observe_byte_division};

/// Every terminal external exit of a body, read from published terminators alone.
pub fn external_exits(body: &omega_psi::MachineBody) -> Vec<&omega_psi::ExternalExit> {
    body.blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            omega_psi::BlockTerminator::ExternalExit(exit) => Some(exit),
            _ => None,
        })
        .collect()
}

/// Replay a package on a separately selected receiver, including the plan that supplied its geometry.
pub fn replay_layout<'a>(
    closure: &'a FoldedTargetClosure,
    profile: &str,
    plan: u8,
    size: u64,
    subject: FoldedSubject,
) -> Result<(), &'a FoldedTargetObservation> {
    let identity = TargetIdentity {
        profile: profile.to_string(),
        semantics_version: 1,
        realization: vec![7].into(),
    };
    let layout = FoldedLayout {
        target: identity.clone(),
        subject,
        schema: vec![1].into(),
        plan: vec![plan].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(size),
            alignment: Some(8),
        },
    };
    let receiver = ReceivingTarget::new(identity, [layout], []).expect("valid independently selected layout");
    closure.replay(&receiver)
}

pub fn array_parameter_length(
    package: &LoweredPackage,
    machine_name: &str,
) -> Option<u64> {
    let machine = machine(package, machine_name)?;
    match package.types.get(*machine.parameter_types.first()?)? {
        TypeShape::FixedArray { length, .. } => Some(*length),
        _ => None,
    }
}

pub fn machine_contract<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> Option<&'a omega_psi::MachineContract> {
    Some(&machine(package, name)?.contract)
}
fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> Option<&'a omega_psi::LoweredMachine> {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.kind == DeclarationKind::Machine && machine.declaration.path == name)
}

/// Read the published target and operand sequence of an executable member call.
pub fn member_call<'a>(
    package: &'a LoweredPackage,
    operation: &'a omega_psi::OperationKind,
) -> Option<(&'a omega_psi::MemberCallSelection, &'a [omega_psi::CallArgument])> {
    let omega_psi::OperationKind::Call {
        callee: omega_psi::CallTarget::Member(id),
        arguments,
        ..
    } = operation
    else {
        return None;
    };
    Some((package.member_calls.get(*id)?, arguments))
}

/// Read the parent place of a single-block machine's explicitly returned reborrow.
pub fn returned_reborrow(body: &omega_psi::MachineBody) -> Option<&omega_psi::ProjectedPlace> {
    let [block] = body.blocks.as_slice() else { return None };
    let omega_psi::BlockTerminator::Return {
        result: omega_psi::ReturnEdge::Structural { source, .. },
        ..
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::PlaceOrigin::Operation(id) = body.places.get(source.index())?.origin else {
        return None;
    };
    let op = body.operations.get(id.index())?;
    if op.result != omega_psi::OperationResult::Structural(*source) {
        return None;
    }
    match &op.kind {
        omega_psi::OperationKind::BorrowPlace { source, .. } => Some(source),
        _ => None,
    }
}

/// Follow the returned place to its defining transfer and read precisely what survives on that edge.
pub fn returned_move<'a>(
    body: &'a omega_psi::MachineBody,
    block: &'a omega_psi::BranchBlock,
) -> Option<(&'a omega_psi::ProjectedPlace, &'a [omega_psi::CleanupAction])> {
    let omega_psi::BlockTerminator::Return {
        result: omega_psi::ReturnEdge::Structural { source, .. },
        cleanup,
        ..
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::PlaceOrigin::Operation(id) = body.places.get(source.index())?.origin else {
        return None;
    };
    let operation = body.operations.get(id.index())?;
    if operation.result != omega_psi::OperationResult::Structural(*source) {
        return None;
    }
    match &operation.kind {
        omega_psi::OperationKind::MoveStructural { source } => Some((source, cleanup)),
        _ => None,
    }
}

/// Reconstruct the local crossing roster from selected machine envelopes after dropping all source trees.
pub fn suspending_local_calls(
    package: &LoweredPackage,
    body: &omega_psi::MachineBody,
) -> Vec<omega_psi::CallSite> {
    let may_park = |target: omega_psi::CallTarget| matches!(target,omega_psi::CallTarget::Machine(id) if package.machines[(id.get()-1) as usize].contract.realized_pause.suspend);
    body.blocks
        .iter()
        .flat_map(|block| {
            body.operations[block.operations.clone()]
                .iter()
                .filter_map(|op| match op.kind {
                    omega_psi::OperationKind::Call { callee, .. } if may_park(callee) => Some(omega_psi::CallSite::Operation(op.id)),
                    _ => None,
                })
                .chain(match block.terminator {
                    omega_psi::BlockTerminator::TerminalCall { id, callee, .. } if may_park(callee) => Some(omega_psi::CallSite::Edge(id)),
                    _ => None,
                })
        })
        .collect()
}

mod parked_byte_views;
pub use parked_byte_views::check_parked_byte_views;

mod joined_loans;
pub use joined_loans::check_joined_loan;

mod returned_view;
pub use returned_view::returned_view_backing;

mod parked_handles;
pub use parked_handles::check_parked_handles;

mod embedded_fields;
pub use embedded_fields::{check_incoming_fields, check_constructed_field};

mod parked_owner_borrows;
pub use parked_owner_borrows::check_parked_owner_borrow;

pub mod product_witness;
