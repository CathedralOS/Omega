use omega_psi::{BlockTerminator, CallArgument, LoweredPackage, OperationKind, Parameter, PlaceId, PlaceOrigin, PlaceProjection, ReferenceAccess, TypeShape};

use super::machine;

/// What a reader recovers from Psi alone about an exclusive descriptor that a
/// Boolean dispatch chooses and its join parameter receives.
#[derive(Debug, Eq, PartialEq)]
pub struct ExclusiveJoin {
    /// For each side of the dispatch (true, then false), the machine parameter
    /// position whose descriptor that predecessor transfers to the join.
    pub arrivals: [(bool, usize); 2],
    /// The exact descriptor type shared by every arrival and the join parameter.
    pub descriptor: TypeShape,
    /// How the first operation of the join uses the joined descriptor.
    pub consumer: JoinedUse,
    /// Machine parameters that an operation of the join block reads again after
    /// some arrival transferred them. The checker judges these against each
    /// arriving edge; the reader only reports them.
    pub reused_after_transfer: Vec<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum JoinedUse {
    /// `ElementViewLength` of the joined view.
    Length,
    /// A scalar read through the joined reference's referent at this field path.
    ReferentField(Vec<String>),
}

/// Follow the entry dispatch to the join, recover the incoming descriptor on
/// each arriving edge, and require the join's consumer to use the join parameter.
pub fn exclusive_join(
    package: &LoweredPackage,
    name: &str,
) -> Result<ExclusiveJoin, &'static str> {
    let body = &machine(package, name).ok_or("missing machine")?.body;
    let block = |id: omega_psi::BlockId| body.blocks.get(id.get() - 1).ok_or("missing block");
    let Some(BlockTerminator::Conditional { when_true, when_false, .. }) = body.blocks.first().map(|b| &b.terminator) else {
        return Err("expected a Boolean dispatch at entry");
    };
    let mut join = None;
    let mut arrivals = Vec::with_capacity(2);
    let mut transferred = Vec::with_capacity(2);
    for (side, edge) in [(true, when_true), (false, when_false)] {
        let BlockTerminator::Jump(arrival) = &block(edge.target)?.terminator else {
            return Err("each predecessor must jump to the join");
        };
        if join.is_some_and(|target| target != arrival.target) {
            return Err("predecessors must feed one join");
        }
        join = Some(arrival.target);
        let source = match arrival.structural_arguments.as_slice() {
            [CallArgument::MoveWhole(source)] => *source,
            [CallArgument::CopyWhole(_)] => return Err("an exclusive descriptor arrives by copy"),
            _ => return Err("expected one transferred descriptor on the arriving edge"),
        };
        let formal = body
            .parameters
            .iter()
            .position(|p| *p == Parameter::Structural(source))
            .ok_or("the arriving descriptor is not an incoming formal")?;
        arrivals.push((side, formal));
        transferred.push(source);
    }
    let join = block(join.ok_or("missing join")?)?;
    let [joined] = join.structural_parameters.as_slice() else {
        return Err("expected one join parameter");
    };
    let place = |p: PlaceId| body.places.get(p.index()).ok_or("missing place");
    if place(*joined)?.origin != PlaceOrigin::BlockParameter(join.id) {
        return Err("the join parameter must be a block parameter of the join");
    }
    let ty = place(*joined)?.ty;
    if transferred
        .iter()
        .any(|&source| place(source).is_ok_and(|p| p.ty != ty))
    {
        return Err("an arrival has a different descriptor type");
    }
    let descriptor = *package.types.get(ty).ok_or("missing descriptor type")?;
    if !matches!(
        descriptor,
        TypeShape::MutableElementView { .. }
            | TypeShape::NominalReference {
                access: ReferenceAccess::Mutable,
                ..
            }
            | TypeShape::ScalarReference {
                access: ReferenceAccess::Mutable,
                ..
            }
    ) {
        return Err("the joined descriptor is not exclusive");
    }
    let operations = body
        .operations
        .get(join.operations.clone())
        .ok_or("missing join operations")?;
    let consumer = match operations.first().map(|op| &op.kind) {
        Some(OperationKind::ElementViewLength { source }) if source == joined => JoinedUse::Length,
        Some(OperationKind::ProjectedScalarRead { source }) if source.root == *joined && source.path.first() == Some(&PlaceProjection::Referent) => {
            let fields = source.path[1..]
                .iter()
                .map(|step| match step {
                    PlaceProjection::Field(field) => Ok(field.path.clone()),
                    _ => Err("expected a field path below the referent"),
                })
                .collect::<Result<_, _>>()?;
            JoinedUse::ReferentField(fields)
        }
        _ => return Err("the join's consumer must use the join parameter"),
    };
    let mut reused_after_transfer: Vec<usize> = operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::ElementViewLength { source } => Some(*source),
            OperationKind::ProjectedScalarRead { source } => Some(source.root),
            _ => None,
        })
        .filter(|source| transferred.contains(source))
        .filter_map(|source| {
            body.parameters
                .iter()
                .position(|p| *p == Parameter::Structural(source))
        })
        .collect();
    reused_after_transfer.dedup();
    Ok(ExclusiveJoin {
        arrivals: [arrivals[0], arrivals[1]],
        descriptor,
        consumer,
        reused_after_transfer,
    })
}

/// A move of an entire exclusive descriptor, recovered from its defining
/// operation. Reads keep their original roots so the checker can see later
/// uses of the source as well as uses of the new binding.
pub fn local_descriptor_move(body: &omega_psi::MachineBody) -> Result<(PlaceId, PlaceId, Vec<PlaceId>), &'static str> {
    let mut moves = body.operations.iter().filter_map(|op| match &op.kind {
        OperationKind::MoveStructural { source } => Some((op, source)),
        _ => None,
    });
    let (op, source) = moves.next().ok_or("missing descriptor move")?;
    if moves.next().is_some() || !source.path.is_empty() {
        return Err("expected one whole descriptor move");
    }
    let omega_psi::OperationResult::Structural(destination) = op.result else {
        return Err("the move must produce a place");
    };
    let from = body.places.get(source.root.index()).ok_or("missing source")?;
    let to = body
        .places
        .get(destination.index())
        .ok_or("missing destination")?;
    if source.root == destination || from.ty != to.ty || to.origin != PlaceOrigin::Operation(op.id) {
        return Err("the move must define a distinct place of the same type");
    }
    let reads = body
        .operations
        .iter()
        .filter_map(|op| match op.kind {
            OperationKind::ElementViewLength { source } => Some(source),
            _ => None,
        })
        .collect();
    Ok((source.root, destination, reads))
}
