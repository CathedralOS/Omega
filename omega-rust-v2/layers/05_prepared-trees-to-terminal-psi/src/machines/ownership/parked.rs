use super::{CallArgument, CleanupContext, Frontier, LiveRoot, LoweringError, PlaceProjection, TypeId, TypeShape};
use crate::{EmbeddedLoanOrigin, OperationKind, OperationResult, PlaceId, PlaceOrigin, ReferenceAccess, ValueOperand};

const UNTRACED_DESCRIPTOR: &str = "parking this owner requires the backing or carry evidence of its embedded descriptors";

/// Ownership reaches a call in forward execution order; liveness visits it
/// backward. Keep only nonempty snapshots, keyed by the published call.
#[derive(Default)]
pub(crate) struct OwnedCrossings {
    sites: Vec<(crate::CallSite, Vec<crate::RetainedOwner>)>,
    /// Descriptor classification by type, shared by every crossing of the body.
    embeds: Embeds,
}

impl OwnedCrossings {
    pub(super) fn sort_by_call(&mut self) {
        self.sites.sort_unstable_by_key(|(call, _)| Self::key(*call));
    }

    /// A total order over every call coordinate; only calls and terminal
    /// edges park, so cleanup sites never match a recorded crossing.
    fn key(call: crate::CallSite) -> (u8, usize, usize) {
        match call {
            crate::CallSite::Operation(id) => (0, id.index(), 0),
            crate::CallSite::Edge(id) => (1, id.get() - 1, 0),
            crate::CallSite::Cleanup(crate::CleanupSite {
                list: crate::CleanupList::Edge(id),
                action,
            }) => (2, id.get() - 1, action),
            crate::CallSite::Cleanup(crate::CleanupSite {
                list: crate::CleanupList::Store(id),
                action,
            }) => (3, id.index(), action),
        }
    }

    pub(in crate::machines) fn take(
        &mut self,
        call: crate::CallSite,
    ) -> Vec<crate::RetainedOwner> {
        match self
            .sites
            .binary_search_by_key(&Self::key(call), |(site, _)| Self::key(*site))
        {
            Ok(index) => std::mem::take(&mut self.sites[index].1),
            Err(_) => Vec::new(),
        }
    }

    pub(super) fn record(
        &mut self,
        call: crate::CallSite,
        state: &Frontier,
        arguments: &[CallArgument],
        context: &CleanupContext<'_>,
        lineage: &Lineage<'_>,
    ) -> Result<(), LoweringError> {
        let mut owners = Vec::new();
        for root in &state.0 {
            if root.exhausted(context) {
                continue;
            }
            let ty = context.places[root.place.index()].ty;
            let embedded_loans = embedded_loans(root, ty, context, lineage, &mut self.embeds)?;
            owners.push(crate::RetainedOwner {
                place: root.place,
                ty,
                moved: root.moved.clone(),
                custody: crate::OwnedCustody::Caller,
                embedded_loans,
            });
        }
        // A direct borrow stays on the call operand. Only a whole-value
        // transfer changes custody of the backing owner.
        for (slot, arg) in arguments.iter().enumerate() {
            let (place, transfer) = match arg {
                CallArgument::MoveWhole(place) => (*place, true),
                CallArgument::CopyWhole(place) => (*place, false),
                CallArgument::Borrow { .. } | CallArgument::Scalar(_) => continue,
            };
            if let Ok(index) = owners.binary_search_by_key(&place.index(), |owner| owner.place.index()) {
                if !transfer {
                    return Err(LoweringError::new(context.site, "parked call cannot copy an owned argument"));
                }
                owners[index].custody = crate::OwnedCustody::CalleeArgument(slot);
            }
        }
        if !owners.is_empty() {
            self.sites.push((call, owners));
        }
        Ok(())
    }
}

/// Reaching values for freshly constructed writable records in one basic block.
/// Keys identify storage, not the value moved into it. Each home keeps the
/// whole value it last received and, after it, each store into one of its
/// fields, oldest first. A writable alias loses the ability to track stores at
/// this root at all. Never carry one predecessor's witness through a join.
#[derive(Default)]
pub(super) struct LocalContents(std::collections::BTreeMap<usize, Option<Contents>>);

#[derive(Clone)]
pub(super) struct Contents {
    whole: PlaceId,
    stores: Vec<(Vec<PlaceProjection>, PlaceId)>,
}

impl Contents {
    /// The value that supplies `path` of the home, with the rest of the path
    /// inside it: the newest store at a prefix of `path`, otherwise the whole value.
    fn reaching(
        &self,
        path: &[PlaceProjection],
    ) -> (PlaceId, Vec<PlaceProjection>) {
        for (stored, value) in self.stores.iter().rev() {
            if path.starts_with(stored) {
                return (*value, path[stored.len()..].to_vec());
            }
        }
        (self.whole, path.to_vec())
    }
}

impl LocalContents {
    pub(super) fn operand(&mut self, operand: &ValueOperand) {
        if let ValueOperand::Borrow { place, access } = operand && *access != ReferenceAccess::Shared {
            self.0.remove(&place.root.index());
        }
    }

    pub(super) fn observe(&mut self, op: &crate::PsiOperation, mutable_homes: &[PlaceId]) {
        if mutable_homes.is_empty() {
            return;
        }
        match &op.kind {
            OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
                for field in fields {
                    self.operand(&field.operand);
                }
                if matches!(op.kind, OperationKind::EstablishRecord { .. })
                    && let OperationResult::Structural(place) = op.result
                    && mutable_homes.contains(&place)
                {
                    self.0.insert(place.index(), Some(Contents { whole: place, stores: Vec::new() }));
                }
            }
            OperationKind::BorrowPlace { source, access } if *access != ReferenceAccess::Shared => {
                self.0.remove(&source.root.index());
            }
            OperationKind::Call { arguments, .. } => {
                for arg in arguments {
                    self.operand(arg);
                }
            }
            OperationKind::StructuralStore { destination, value, .. } => {
                if let Some(current) = self.0.get_mut(&destination.root.index()) {
                    if mutable_homes.contains(value) || destination.path.contains(&PlaceProjection::Referent) {
                        *current = None;
                    } else if destination.path.is_empty() {
                        *current = Some(Contents { whole: *value, stores: Vec::new() });
                    } else if let Some(contents) = current {
                        contents.stores.push((destination.path.clone(), *value));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Only a writable home can have its own fields replaced. A write through a
/// reference changes its referent while preserving the descriptor itself.
pub(super) struct Lineage<'a> {
    pub(super) operations: &'a [crate::PsiOperation],
    pub(super) mutable_homes: &'a [PlaceId],
    pub(super) contents: &'a LocalContents,
}

/// An owner row describes custody of its storage. Each live descriptor in that
/// storage also needs its backing, traced to the incoming formal it came from,
/// and a transfer out of the owner is a recorded hole. An intact sum or
/// nonempty fixed array holding descriptors, callable descriptors and
/// descriptors without a single traced origin keep the crossing unsupported.
fn embedded_loans(
    root: &LiveRoot,
    ty: TypeId,
    context: &CleanupContext<'_>,
    lineage: &Lineage<'_>,
    embeds: &mut Embeds,
) -> Result<Vec<crate::EmbeddedLoan>, LoweringError> {
    let refuse = || LoweringError::new(context.site, UNTRACED_DESCRIPTOR);
    let formal = matches!(context.places[root.place.index()].origin, PlaceOrigin::Parameter);
    let mut loans = Vec::new();
    let mut pending = vec![(Vec::new(), ty)];
    while let Some((path, ty)) = pending.pop() {
        if root.moved.binary_search(&path).is_ok() || !embeds.contains(ty, context).ok_or_else(refuse)? {
            continue;
        }
        let access = match context.types.get(ty) {
            Some(TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. }) => *access,
            Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. }) => ReferenceAccess::Shared,
            Some(TypeShape::MutableElementView { .. }) => ReferenceAccess::Mutable,
            Some(TypeShape::Nominal(id)) => {
                let schema = &context.data[id.get() - 1];
                // The descriptors of an intact sum depend on its active case.
                if !schema.cases.is_empty() {
                    return Err(refuse());
                }
                // Reverse pushes keep the rows in declaration pre-order.
                for field in schema.fields.iter().rev() {
                    let mut child = path.clone();
                    child.push(PlaceProjection::Field(field.declaration.clone()));
                    pending.push((child, field.carrier));
                }
                continue;
            }
            // One row per literal ordinal could be enormous. A callable
            // descriptor needs contract evidence rather than a loan origin.
            _ => return Err(refuse()),
        };
        // Only a shared descriptor is copied into a local owner unchanged.
        if !formal && access != ReferenceAccess::Shared {
            return Err(refuse());
        }
        let origin = origin(root.place, &path, ty, context, lineage).ok_or_else(refuse)?;
        loans.push(crate::EmbeddedLoan { path, ty, access, origin });
    }
    Ok(loans)
}

/// Follows the descriptor at `path` inside `place` back through the earlier
/// operations that established it until it reaches an incoming formal. A
/// record field initializer, a field-only transfer or copy, or a decomposed
/// component each name exactly one source. A shared reborrow stays in the
/// lineage and continues from its parent descriptor. Joins, call results,
/// borrows of local storage and intermediate writable sources have no traced origin.
fn origin(
    mut place: PlaceId,
    path: &[PlaceProjection],
    mut expected: TypeId,
    context: &CleanupContext<'_>,
    lineage: &Lineage<'_>,
) -> Option<EmbeddedLoanOrigin> {
    let mut path = path.to_vec();
    let mut reborrows = Vec::new();
    // Only the initial lookup describes storage at *this crossing*. An
    // initializer read an earlier value; never consult the current home there.
    if lineage.mutable_homes.contains(&place) {
        (place, path) = lineage.contents.0.get(&place.index())?.as_ref()?.reaching(&path);
    }
    for depth in 0..=context.places.len() {
        if depth != 0 && lineage.mutable_homes.contains(&place) {
            return None;
        }
        match context.places.get(place.index())?.origin {
            PlaceOrigin::Parameter => {
                if field_type(context.places[place.index()].ty, &path, context)? != expected {
                    return None;
                }
                reborrows.reverse();
                return Some(EmbeddedLoanOrigin {
                    formal: place,
                    path,
                    reborrows,
                });
            }
            PlaceOrigin::Operation(id) => {
                let op = lineage.operations.get(id.index())?;
                if op.result != OperationResult::Structural(place) {
                    return None;
                }
                match &op.kind {
                    OperationKind::EstablishRecord { fields } => {
                        let Some(PlaceProjection::Field(name)) = path.first() else {
                            return None;
                        };
                        let field = fields.iter().find(|field| &field.field == name)?;
                        place = match field.operand {
                            ValueOperand::CopyWhole(source) => source,
                            // A descriptor itself is copied; an aggregate holding it may move.
                            ValueOperand::MoveWhole(source) if path.len() > 1 => source,
                            _ => return None,
                        };
                        path.remove(0);
                    }
                    OperationKind::MoveStructural { source } | OperationKind::CopyStructural { source } => place = rebase(source, &mut path)?,
                    OperationKind::BorrowPlace {
                        source,
                        access: ReferenceAccess::Shared,
                    } if path.is_empty() && context.places[place.index()].ty == expected => {
                        let referent = source
                            .path
                            .iter()
                            .position(|step| *step == PlaceProjection::Referent)?;
                        let parent = &source.path[..referent];
                        if source.path[referent + 1..].contains(&PlaceProjection::Referent) {
                            return None;
                        }
                        expected = field_type(context.places.get(source.root.index())?.ty, parent, context)?;
                        if !matches!(
                            context.types.get(expected),
                            Some(
                                TypeShape::ScalarReference {
                                    access: ReferenceAccess::Shared,
                                    ..
                                } | TypeShape::NominalReference {
                                    access: ReferenceAccess::Shared,
                                    ..
                                }
                            )
                        ) {
                            return None;
                        }
                        reborrows.push(id);
                        path = parent.to_vec();
                        place = source.root;
                    }
                    _ => return None,
                }
            }
            PlaceOrigin::Component { operation, position } => {
                let OperationKind::DecomposeRecord { source, fields, .. } = &lineage.operations.get(operation.index())?.kind else {
                    return None;
                };
                let component = fields.get(position)?;
                if component.result != OperationResult::Structural(place) {
                    return None;
                }
                path.insert(0, PlaceProjection::Field(component.field.clone()));
                place = rebase(source, &mut path)?;
            }
            PlaceOrigin::BlockParameter(_) | PlaceOrigin::Result => return None,
        }
    }
    None
}

/// Prefixes a field-only source path and returns its root.
fn rebase(
    source: &crate::ProjectedPlace,
    path: &mut Vec<PlaceProjection>,
) -> Option<PlaceId> {
    if !source
        .path
        .iter()
        .all(|step| matches!(step, PlaceProjection::Field(_)))
    {
        return None;
    }
    path.splice(0..0, source.path.iter().cloned());
    Some(source.root)
}

/// The carrier reached by field projections from `ty`. Sums and other steps have none.
fn field_type(
    mut ty: TypeId,
    path: &[PlaceProjection],
    context: &CleanupContext<'_>,
) -> Option<TypeId> {
    for step in path {
        let (PlaceProjection::Field(name), Some(TypeShape::Nominal(id))) = (step, context.types.get(ty)) else {
            return None;
        };
        let schema = context.data.get(id.get() - 1)?;
        if !schema.cases.is_empty() {
            return None;
        }
        ty = schema
            .fields
            .iter()
            .find(|field| &field.declaration == name)?
            .carrier;
    }
    Some(ty)
}

/// Classification of one type: not yet visited, on the explicit stack, or settled.
#[derive(Clone, Copy, Default)]
enum Embedding {
    #[default]
    Unknown,
    Visiting,
    Known(bool),
}

/// Whether a carrier stores a descriptor by value, classified once per type
/// with an explicit stack and indexed by the dense type id. Meeting a type
/// still being classified exposes a value cycle, which is refused.
#[derive(Default)]
struct Embeds(Vec<Embedding>);

impl Embeds {
    /// An id outside the type table has no classification and is refused.
    fn index(
        ty: TypeId,
        context: &CleanupContext<'_>,
    ) -> Option<usize> {
        context.types.get(ty)?;
        usize::try_from(ty.get() - 1).ok()
    }

    fn state(
        &self,
        index: usize,
    ) -> Embedding {
        self.0.get(index).copied().unwrap_or_default()
    }

    fn set(
        &mut self,
        index: usize,
        state: Embedding,
    ) {
        if index >= self.0.len() {
            self.0.resize(index + 1, Embedding::Unknown);
        }
        self.0[index] = state;
    }

    fn contains(
        &mut self,
        ty: TypeId,
        context: &CleanupContext<'_>,
    ) -> Option<bool> {
        let mut pending = vec![(ty, false)];
        let mut children = Vec::new();
        while let Some((ty, finished)) = pending.pop() {
            let index = Self::index(ty, context)?;
            children.clear();
            if !finished {
                if !matches!(self.state(index), Embedding::Unknown) {
                    continue;
                }
                Self::children(ty, context, &mut children);
                self.set(index, Embedding::Visiting);
                pending.push((ty, true));
                pending.extend(children.iter().map(|&child| (child, false)));
                continue;
            }
            let mut found = Self::children(ty, context, &mut children);
            for &child in &children {
                let Embedding::Known(child) = self.state(Self::index(child, context)?) else {
                    return None;
                };
                found |= child;
            }
            self.set(index, Embedding::Known(found));
        }
        match self.state(Self::index(ty, context)?) {
            Embedding::Known(found) => Some(found),
            Embedding::Unknown | Embedding::Visiting => None,
        }
    }

    /// Collects the carriers stored by value and reports whether the type is
    /// itself a descriptor. An unknown shape counts as one so that it is refused.
    fn children(
        ty: TypeId,
        context: &CleanupContext<'_>,
        children: &mut Vec<TypeId>,
    ) -> bool {
        match context.types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }) => false,
            Some(TypeShape::FixedArray { element, length }) => {
                if *length != 0 {
                    children.push(*element);
                }
                false
            }
            Some(TypeShape::Nominal(id)) => {
                let schema = &context.data[id.get() - 1];
                children.extend(
                    schema
                        .fields
                        .iter()
                        .chain(schema.cases.iter().flat_map(|case| &case.fields))
                        .map(|field| field.carrier),
                );
                false
            }
            _ => true,
        }
    }
}
