//! Account for the parked activation's copyable structural storage, reading
//! published Psi alone.
//!
//! A `[copy]` record or fixed array whose leaves are all scalars has
//! no ownership claim and no cleanup. When a parked call or its continuation
//! needs it, for example because `@x.key` lends one of its fields, the storage
//! stays in the activation and the site gives it a home row
//! (`wiki/spec/terminal-psi/calls_and_outcomes.md#suspension`). This reader
//! derives that need independently, then requires the
//! site's non-primitive home rows to say exactly that.
use omega_psi::{
    BlockTerminator, CallArgument, LoweredPackage, MachineBody, Multiplicity, OperationKind, PlaceId, PlaceOrigin, PlaceProjection, ProjectedPlace,
    ReturnEdge, SuspensionSite, TypeId, TypeShape, ValueOperand,
};

use super::primitive_homes::ParkedCall;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyableHome {
    pub place: PlaceId,
    /// The entry formal that is this storage, if any.
    pub formal: Option<usize>,
    pub needed_by_callee: bool,
    pub needed_after_completion: bool,
    /// Retained loan descriptors whose defining borrows select this storage.
    pub loans: Vec<PlaceId>,
}

fn is_copyable_scalar_storage(package: &LoweredPackage, ty: TypeId) -> bool {
    match package.types.get(ty) {
        Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }) => true,
        Some(TypeShape::FixedArray { element, .. }) => is_copyable_scalar_storage(package, *element),
        Some(TypeShape::Nominal(id)) => package.data.get(id.get() - 1).is_some_and(|schema| {
            schema.multiplicity == Multiplicity::Copy
                && schema.cases.is_empty()
                && schema.fields.iter().all(|field| is_copyable_scalar_storage(package, field.carrier))
        }),
        _ => false,
    }
}

fn inside(
    place: &ProjectedPlace,
    root: PlaceId,
) -> bool {
    place.root == root
        && place
            .path
            .iter()
            .all(|step| !matches!(step, PlaceProjection::Referent | PlaceProjection::ViewElement { .. }))
}

/// The storage a loan descriptor's defining borrows select, through named reborrows.
fn loan_root(
    body: &MachineBody,
    mut descriptor: PlaceId,
) -> Option<ProjectedPlace> {
    loop {
        let PlaceOrigin::Operation(id) = body.places.get(descriptor.index())?.origin else {
            return None;
        };
        let OperationKind::BorrowPlace { source, .. } = &body.operations.get(id.index())?.kind else {
            return None;
        };
        match source.path.first() {
            Some(PlaceProjection::Referent) => descriptor = source.root,
            _ => return Some(source.clone()),
        }
    }
}

fn operand_uses(
    operand: &CallArgument,
    root: PlaceId,
) -> bool {
    match operand {
        ValueOperand::Borrow { place, .. } => place.root == root,
        ValueOperand::CopyWhole(place) | ValueOperand::MoveWhole(place) => *place == root,
        ValueOperand::Scalar(_) => false,
    }
}

/// Whether an operation reads, lends or writes storage rooted at `root`.
fn mentions(
    kind: &OperationKind,
    root: PlaceId,
) -> bool {
    match kind {
        OperationKind::BorrowPlace { source, .. }
        | OperationKind::BorrowedMove { source }
        | OperationKind::CopyStructural { source }
        | OperationKind::MoveStructural { source }
        | OperationKind::StructuralCaseMembership { source, .. }
        | OperationKind::DecomposeRecord { source, .. }
        | OperationKind::ProjectedScalarRead { source } => source.root == root,
        OperationKind::StructuralScalarFieldStore { destination, .. } | OperationKind::ProjectedScalarStore { destination, .. } => destination.root == root,
        OperationKind::StructuralStore { destination, value, .. } => destination.root == root || *value == root,
        OperationKind::PrimitiveScalarRead { source, .. } | OperationKind::StructuralLeafCopy { source, .. } => *source == root,
        OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => fields.iter().any(|f| operand_uses(&f.operand, root)),
        OperationKind::EstablishStructuralArray { elements } => elements.contains(&root),
        OperationKind::Call { arguments, .. } => arguments.iter().any(|a| operand_uses(a, root)),
        _ => false,
    }
}

fn terminator_uses(
    terminator: &BlockTerminator,
    root: PlaceId,
) -> bool {
    match terminator {
        BlockTerminator::TerminalCall { arguments, .. } => arguments.iter().any(|a| operand_uses(a, root)),
        BlockTerminator::Return {
            result: ReturnEdge::Structural { source, .. },
            ..
        } => *source == root,
        BlockTerminator::Jump(edge) => edge.structural_arguments.iter().any(|a| operand_uses(a, root)),
        BlockTerminator::Conditional { when_true, when_false, .. } => [when_true, when_false]
            .iter()
            .any(|edge| edge.structural_arguments.iter().any(|a| operand_uses(a, root))),
        _ => false,
    }
}

/// Reconstruct which copyable storage the parked call and its continuation
/// need, and require the site's non-primitive home rows to match exactly, in
/// place order.
pub fn parked_copyable_homes(
    package: &LoweredPackage,
    body: &MachineBody,
    site: &SuspensionSite,
) -> Result<Vec<CopyableHome>, &'static str> {
    let call = ParkedCall::read(body, site)?;
    let primitive = |place: PlaceId| {
        matches!(body.places.get(place.index()).map(|p| p.origin), Some(PlaceOrigin::Operation(id))
            if matches!(body.operations.get(id.index()).map(|op| &op.kind), Some(OperationKind::EstablishPrimitiveLocal { .. })))
    };
    let mut rows = site.retained_homes.iter().filter(|row| !primitive(row.place)).peekable();
    let mut homes = Vec::new();
    for (index, place) in body.places.iter().enumerate() {
        let root = PlaceId(std::num::NonZeroUsize::new(index + 1).unwrap());
        if primitive(root) || !matches!(package.types.get(place.ty), Some(TypeShape::Nominal(_) | TypeShape::FixedArray { .. })) {
            continue;
        }
        if !is_copyable_scalar_storage(package, place.ty) {
            continue;
        }
        let loans: Vec<_> = site
            .retained_loans
            .iter()
            .filter(|loan| loan_root(body, loan.descriptor).is_some_and(|source| inside(&source, root)))
            .collect();
        let by_callee = call.arguments.iter().any(|a| operand_uses(a, root)) || loans.iter().any(|loan| loan.needed_by_callee);
        let after = call.later.iter().any(|kind| mentions(kind, root))
            || call.terminators.iter().any(|t| terminator_uses(t, root))
            || loans.iter().any(|loan| loan.needed_after_completion);
        if !by_callee && !after {
            continue;
        }
        let row = rows
            .next_if(|row| row.place == root)
            .ok_or("needed copyable storage has no row, or the rows are out of order")?;
        if row.ty != place.ty {
            return Err("a copyable home row disagrees with its storage type");
        }
        if (row.needed_by_callee, row.needed_after_completion) != (by_callee, after) {
            return Err("a copyable home row's needs disagree with the call and its continuation");
        }
        if site.retained_owners.iter().any(|owner| owner.place == root) || site.retained_loans.iter().any(|loan| loan.descriptor == root) {
            return Err("copyable storage is neither an owner nor a loan");
        }
        homes.push(CopyableHome {
            place: root,
            formal: body.parameters.iter().position(|p| *p == omega_psi::Parameter::Structural(root)),
            needed_by_callee: by_callee,
            needed_after_completion: after,
            loans: loans.iter().map(|loan| loan.descriptor).collect(),
        });
    }
    if rows.next().is_some() {
        return Err("a home row names storage that is neither a primitive home nor needed copyable storage");
    }
    Ok(homes)
}
