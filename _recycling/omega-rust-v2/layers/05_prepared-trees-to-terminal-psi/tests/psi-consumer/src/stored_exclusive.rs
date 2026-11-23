use omega_psi::{
    BlockTerminator, CleanupAction, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection,
    ProjectedPlace, ReferenceAccess, ReturnEdge, TypeShape,
};

/// How the returned exclusive reference was formed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExclusiveOrigin {
    /// `MoveStructural` out of owned storage: the caller's stored loan itself
    /// is the result.
    Transfer,
    /// `BorrowedMove` through a `@mutable` parent. The source is a hole that must be
    /// restored before the parent loan is released.
    BorrowedMove,
    /// A `Mutable` `BorrowPlace` below a stored descriptor: a child of that loan.
    Reborrow,
}

/// What a reader recovers from Psi alone about a returned exclusive reference.
#[derive(Debug, Eq, PartialEq)]
pub struct ReturnedExclusive {
    pub origin: ExclusiveOrigin,
    /// The incoming formal the source path starts at, after following
    /// whole-record moves into locals.
    pub formal: usize,
    /// The source path: field declarations, with `*` for each referent crossing.
    pub path: Vec<String>,
    /// Operations in the body that move this exact source again.
    pub moves_of_source: usize,
    /// What the return edge disposes: `root`, or the field path of a residual.
    pub cleanup: Vec<String>,
}

fn spell(path: &[PlaceProjection]) -> Result<Vec<String>, &'static str> {
    path.iter()
        .map(|step| match step {
            PlaceProjection::Field(field) => Ok(field.path.clone()),
            PlaceProjection::Referent => Ok("*".to_string()),
            _ => Err("only fields and referent crossings may lead to a stored reference"),
        })
        .collect()
}

/// Follow whole-value moves (`let y = x`) back to the place they started from.
fn moved_root(
    body: &MachineBody,
    mut root: PlaceId,
) -> PlaceId {
    while let Some(PlaceOrigin::Operation(id)) = body.places.get(root.index()).map(|p| p.origin)
        && let Some(OperationKind::MoveStructural { source }) = body.operations.get(id.index()).map(|op| &op.kind)
        && source.path.is_empty()
    {
        root = source.root;
    }
    root
}

/// `@mutable` and `@write` loans are both exclusive, so neither may be copied.
fn exclusive(shape: Option<&TypeShape>) -> bool {
    matches!(
        shape,
        Some(
            TypeShape::NominalReference {
                access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                ..
            } | TypeShape::ScalarReference {
                access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                ..
            }
        )
    )
}

fn access_of(shape: Option<&TypeShape>) -> Option<ReferenceAccess> {
    match shape? {
        TypeShape::NominalReference { access, .. } | TypeShape::ScalarReference { access, .. } => Some(*access),
        _ => None,
    }
}

/// Trace `name`'s one structural return to the operation that defines it.
pub fn returned_exclusive(
    package: &LoweredPackage,
    name: &str,
) -> Result<ReturnedExclusive, &'static str> {
    let machine = super::machine(package, name).ok_or("missing machine")?;
    let result = machine.result_type.ok_or("the machine returns no reference")?;
    if !exclusive(package.types.get(result)) {
        return Err("the result type must be an exclusive reference");
    }
    let body = &machine.body;
    // An exclusive reference is affine: no operation may duplicate one.
    for op in &body.operations {
        if let (OperationKind::CopyStructural { .. } | OperationKind::StructuralLeafCopy { .. }, OperationResult::Structural(p)) = (&op.kind, op.result)
            && exclusive(
                body.places
                    .get(p.index())
                    .and_then(|place| package.types.get(place.ty)),
            )
        {
            return Err("an exclusive reference is copied");
        }
    }
    let mut returns = body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::Return { result, cleanup, .. } => Some((result, cleanup)),
        _ => None,
    });
    let (Some((ReturnEdge::Structural { source, result: slot }, cleanup)), None) = (returns.next(), returns.next()) else {
        return Err("expected one structural return");
    };
    let place_ty = |p: PlaceId| body.places.get(p.index()).map(|place| place.ty);
    if place_ty(*slot) != Some(result) || place_ty(*source) != Some(result) {
        return Err("the return must pass a reference of the public result type");
    }
    let Some(PlaceOrigin::Operation(definition)) = body.places.get(source.index()).map(|p| p.origin) else {
        return Err("the returned reference must be formed in the body");
    };
    let op = body
        .operations
        .get(definition.index())
        .ok_or("missing defining operation")?;
    if op.result != OperationResult::Structural(*source) {
        return Err("the operation must define the returned reference");
    }
    let (origin, from): (ExclusiveOrigin, &ProjectedPlace) = match &op.kind {
        OperationKind::MoveStructural { source } => (ExclusiveOrigin::Transfer, source),
        OperationKind::BorrowedMove { source } => (ExclusiveOrigin::BorrowedMove, source),
        // A reborrow keeps the result's own exclusive access: `@mutable` from
        // `@mutable $(..)`, `@write` from `@write $(..)`.
        OperationKind::BorrowPlace { source, access } if Some(*access) == access_of(package.types.get(result)) => (ExclusiveOrigin::Reborrow, source),
        OperationKind::BorrowPlace {
            access: ReferenceAccess::Shared,
            ..
        } => return Err("a mutable reborrow became shared"),
        OperationKind::BorrowPlace { .. } => return Err("a reborrow changed its exclusive access"),
        _ => return Err("the returned reference has no transfer or reborrow"),
    };
    let crosses = from.path.contains(&PlaceProjection::Referent);
    match origin {
        ExclusiveOrigin::Transfer if crosses => return Err("a transfer cannot cross a loan; that is a borrowed move"),
        ExclusiveOrigin::BorrowedMove if !crosses => return Err("a borrowed move must cross its parent loan"),
        ExclusiveOrigin::Reborrow if !crosses => return Err("a reborrow must reach below a stored reference"),
        _ => {}
    }
    let root = moved_root(body, from.root);
    let formal = body
        .parameters
        .iter()
        .position(|p| *p == Parameter::Structural(root))
        .ok_or("the source must start at an incoming formal")?;
    let moves_of_source = body
        .operations
        .iter()
        .filter(|op| matches!(&op.kind, OperationKind::MoveStructural { source } | OperationKind::BorrowedMove { source } if source == from))
        .count();
    let mut disposed = Vec::with_capacity(cleanup.len());
    for action in cleanup {
        let (place, path) = match action {
            CleanupAction::DiscardRoot(p) | CleanupAction::RetireHome(p) => (*p, &[][..]),
            CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => (place.root, place.path.as_slice()),
        };
        // A transferred loan left the record; disposing it there would end the
        // caller's loan that the result still carries.
        if origin == ExclusiveOrigin::Transfer && place == from.root && from.path.starts_with(path) {
            return Err("cleanup discards the transferred loan");
        }
        disposed.push(if path.is_empty() { "root".to_string() } else { spell(path)?.join(".") });
    }
    Ok(ReturnedExclusive {
        origin,
        formal,
        path: spell(&from.path)?,
        moves_of_source,
        cleanup: disposed,
    })
}
