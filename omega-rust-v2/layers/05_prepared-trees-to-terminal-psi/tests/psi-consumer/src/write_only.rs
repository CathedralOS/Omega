//! Read stores through write-only loans from published Psi alone
//! (`wiki/spec/terminal-psi/structural_access.md#write-only-authority`).
//!
//! A write-only loan may locate places in its referent and store into them. It
//! may not read, take, swap or readably reborrow their contents. This reader
//! lists each store a body makes through a write-only loan and refuses the body
//! if any operation reads through one. It is a reader of the published body,
//! not the L6 validator: it does not judge invariant windows, loan lifetimes,
//! aliasing or validity after the write.
//!
//! The loan a store uses is the one its deepest `Referent` step crosses. The
//! reader finds it from types alone: it walks each place's path from the root
//! place's declared type, and each `Referent` takes the access of the reference
//! type it crosses. A store through `x: Node` or `x: @mut Node` into
//! `x.next.v`, where `next: @write Leaf`, loads the stored descriptor through
//! a readable owner and then writes through the stored `@write`. The reverse
//! nesting, `x: @write Node` with `next: @mut Leaf`, must load the stored
//! `@mut` descriptor through the write-only loan, which is a read; the reader
//! refuses it even though the deepest loan is mutable.
//!
//! The unrestricted form requires [closed copy storage](closed_copy_storage).
//! The write-only cleanup forms additionally name the authority retained by
//! their exact nominal schema. The descriptor form leaves child closure and
//! the two live rosters to the lowered-form custody checker
//! (`wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration`).
use omega_psi::{
    CallArgument, DataId, DeclarationId, LoweredData, LoweredPackage, MachineBody, Multiplicity, OperationKind, Parameter, PlaceId, PlaceProjection,
    ProjectedPlace, ReferenceAccess, ScalarType, StructuralStoreDisposition, TypeId, TypeShape, ValueId,
};

use super::machine;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteOnlyStore {
    /// Position of the entry formal the written place is rooted at.
    pub formal: usize,
    /// The path from the formal to the record that holds `field`, or to the
    /// whole written place when `field` is `None`.
    pub path: Vec<PlaceProjection>,
    /// Index in `path` of the write-only `Referent` step whose loan authorizes
    /// the store. It is always the last `Referent` in `path`.
    pub crossing: usize,
    /// The written field, or `None` for a whole written place.
    pub field: Option<DeclarationId>,
    /// `Scalar(v)` stores the SSA scalar `v`. `Structural(p)` replaces intact
    /// content with a value from the exact structural SSA place `p`.
    pub value: Parameter<ValueId, PlaceId>,
}

/// True when displacing a value of `ty` needs no cleanup and retires no
/// custody: a scalar, a fixed array of such storage, or nominal data whose
/// declared multiplicity is `Copy`, with no authored cleanup, whose every
/// record field and case-payload field is itself such storage. A sum
/// qualifies when every payload does, since a whole replacement writes tag
/// and payload together. Reference descriptors carry loan custody that only
/// separate descriptor cleanup authority may retire, so neither they nor
/// element views, byte views or callable descriptors qualify. A declaration
/// that says `Copy` over a field that is not such storage does not qualify.
pub fn closed_copy_storage(
    package: &LoweredPackage,
    ty: TypeId,
) -> bool {
    checked_storage(package, ty, false) == Some((true, omega_psi::WriteOnlyCleanup::NoCodeAffine))
}

/// Reconstruct authority bottom-up from independent Psi declarations. Validate
/// the retained authority at each nominal boundary before accepting its parent.
/// A by-value cycle or a missing child never reaches the ready queue.
fn checked_storage(
    package: &LoweredPackage,
    ty: TypeId,
    require_retained_authority: bool,
) -> Option<(bool, omega_psi::WriteOnlyCleanup)> {
    use omega_psi::WriteOnlyCleanup::{DescriptorCleanup, NoCodeAffine};
    use std::collections::HashMap;
    let mut pending = vec![ty];
    let mut remaining = HashMap::new();
    let mut parents: HashMap<TypeId, Vec<TypeId>> = HashMap::new();
    let mut summaries = HashMap::new();
    let mut ready = Vec::new();
    while let Some(carrier) = pending.pop() {
        if remaining.contains_key(&carrier) {
            continue;
        }
        let (copy, authority, children): (_, _, Vec<_>) = match package.types.get(carrier)? {
            TypeShape::PrimitiveScalar(_) => (true, NoCodeAffine, vec![]),
            TypeShape::FixedArray { element, .. } => (true, NoCodeAffine, vec![*element]),
            TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. } =>
                (*access == ReferenceAccess::Shared, DescriptorCleanup, vec![]),
            TypeShape::Nominal(id) => {
                let data = schema(package, *id)?;
                if data.carrier != carrier || data.multiplicity == Multiplicity::Linear || data.cleanup_target.is_some() {
                    return None;
                }
                let children = data.fields.iter().chain(data.cases.iter().flat_map(|case| &case.fields)).map(|field| field.carrier).collect();
                (data.multiplicity == Multiplicity::Copy, NoCodeAffine, children)
            }
            _ => return None,
        };
        remaining.insert(carrier, children.len());
        summaries.insert(carrier, (copy, authority));
        if children.is_empty() {
            ready.push(carrier);
        }
        for field in children {
            parents.entry(field).or_default().push(carrier);
            pending.push(field);
        }
    }
    while let Some(child) = ready.pop() {
        let (copy, cleanup) = summaries[&child];
        if let TypeShape::Nominal(id) = package.types.get(child)? {
            let data = schema(package, *id)?;
            if require_retained_authority && data.write_only_cleanup != Some(cleanup) || data.multiplicity == Multiplicity::Copy && !copy {
                return None;
            }
        }
        if child == ty {
            return Some((copy, cleanup));
        }
        for parent in parents.remove(&child).unwrap_or_default() {
            let result = summaries.get_mut(&parent)?;
            result.0 &= copy;
            if cleanup == DescriptorCleanup {
                result.1 = DescriptorCleanup;
            }
            let count = remaining.get_mut(&parent)?;
            *count -= 1;
            if *count == 0 {
                ready.push(parent);
            }
        }
    }
    None
}

fn schema(
    package: &LoweredPackage,
    id: DataId,
) -> Option<&LoweredData> {
    package.data.get(id.get() - 1).filter(|data| data.id == id)
}

/// What a path has reached after some of its steps.
#[derive(Clone, Copy)]
enum Reached {
    Type(TypeId),
    /// A scalar referent has a carrier but may have no row of its own.
    Scalar(ScalarType),
    /// The active payload namespace of one case of a sum.
    Payload {
        data: DataId,
        case: usize,
    },
    /// The referent of an exclusive nominal element view; only an element follows.
    Elements(DataId),
}

const MALFORMED: &str = "a place's path does not follow its declared types";

/// Reach `place`'s end from its root's declared type and report the access of
/// each `Referent` step, by its index in the path.
fn walk(
    package: &LoweredPackage,
    body: &MachineBody,
    place: &ProjectedPlace,
) -> Result<(Reached, Vec<(usize, ReferenceAccess)>), &'static str> {
    let root = body.places.get(place.root.index()).ok_or(MALFORMED)?;
    let mut at = Reached::Type(root.ty);
    let mut crossings = Vec::new();
    for (index, step) in place.path.iter().enumerate() {
        let shape = match at {
            Reached::Type(ty) => package.types.get(ty),
            _ => None,
        };
        at = match (step, at, shape) {
            (PlaceProjection::Referent, _, Some(TypeShape::ScalarReference { referent, access })) => {
                crossings.push((index, *access));
                Reached::Scalar(*referent)
            }
            (PlaceProjection::Referent, _, Some(TypeShape::NominalReference { referent, access })) => {
                crossings.push((index, *access));
                Reached::Type(schema(package, *referent).ok_or(MALFORMED)?.carrier)
            }
            (PlaceProjection::Referent, _, Some(TypeShape::MutableElementView { element: omega_psi::ViewElement::Nominal(element) })) => {
                crossings.push((index, ReferenceAccess::Mutable));
                Reached::Elements(*element)
            }
            (PlaceProjection::ViewElement { .. }, Reached::Elements(element), _) => Reached::Type(schema(package, element).ok_or(MALFORMED)?.carrier),
            (PlaceProjection::Field(field), _, Some(TypeShape::Nominal(id))) => {
                let data = schema(package, *id).ok_or(MALFORMED)?;
                Reached::Type(
                    data.fields
                        .iter()
                        .find(|f| &f.declaration == field)
                        .ok_or(MALFORMED)?
                        .carrier,
                )
            }
            (PlaceProjection::Field(field), Reached::Payload { data, case }, _) => {
                let fields = &schema(package, data).ok_or(MALFORMED)?.cases[case].fields;
                Reached::Type(
                    fields
                        .iter()
                        .find(|f| &f.declaration == field)
                        .ok_or(MALFORMED)?
                        .carrier,
                )
            }
            (PlaceProjection::Case(selected), _, Some(TypeShape::Nominal(id))) => {
                let data = schema(package, *id).ok_or(MALFORMED)?;
                let case = data
                    .cases
                    .iter()
                    .position(|c| &c.declaration == selected)
                    .ok_or(MALFORMED)?;
                Reached::Payload { data: *id, case }
            }
            (PlaceProjection::FixedIndex(ordinal), _, Some(TypeShape::FixedArray { element, length })) if ordinal < length => Reached::Type(*element),
            _ => return Err(MALFORMED),
        };
    }
    Ok((at, crossings))
}

/// Crossing another descriptor would read storage behind the write-only loan.
/// A case step records which payload is addressed. Establishing the case at
/// this program point belongs to the checker (structural_access.md).
fn write_only_crossing(
    package: &LoweredPackage,
    body: &MachineBody,
    place: &ProjectedPlace,
) -> Result<Option<(Reached, usize)>, &'static str> {
    if !place.path.contains(&PlaceProjection::Referent) {
        return Ok(None);
    }
    let (reached, crossings) = walk(package, body, place)?;
    let Some(&(crossing, _)) = crossings
        .iter()
        .find(|(_, access)| *access == ReferenceAccess::WriteOnly)
    else {
        return Ok(None);
    };
    if place.path[crossing + 1..].contains(&PlaceProjection::Referent) {
        return Err("a descriptor stored behind a write-only loan is read to cross it");
    }
    Ok(Some((reached, crossing)))
}

pub fn write_only_stores(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<WriteOnlyStore>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let crossing = |place: &ProjectedPlace| write_only_crossing(package, body, place);
    let formal = |root: PlaceId| {
        body.parameters
            .iter()
            .position(|p| *p == Parameter::Structural(root))
            .ok_or("a write-only store is not rooted at an entry formal")
    };
    let value_type = |value: ValueId| body.values.get(value.index()).map(|v| v.ty).ok_or(MALFORMED);
    let disagrees = "a write-only store's value disagrees with its destination";
    let mut stores = Vec::new();
    for op in &body.operations {
        match &op.kind {
            OperationKind::StructuralScalarFieldStore { destination, field, value } => {
                let Some((reached, crossing)) = crossing(destination)? else { continue };
                let carrier = match reached {
                    Reached::Type(ty) => match package.types.get(ty) {
                        Some(TypeShape::Nominal(id)) => schema(package, *id)
                            .ok_or(MALFORMED)?
                            .fields
                            .iter()
                            .find(|f| &f.declaration == field),
                        _ => None,
                    },
                    Reached::Payload { data, case } => schema(package, data).ok_or(MALFORMED)?.cases[case]
                        .fields
                        .iter()
                        .find(|f| &f.declaration == field),
                    _ => None,
                }
                .ok_or("a write-only store names no field of its destination")?
                .carrier;
                if value_type(*value)? != carrier {
                    return Err(disagrees);
                }
                stores.push(WriteOnlyStore {
                    formal: formal(destination.root)?,
                    path: destination.path.clone(),
                    crossing,
                    field: Some(field.clone()),
                    value: Parameter::Scalar(*value),
                });
            }
            OperationKind::ProjectedScalarStore { destination, value } => {
                let Some((reached, crossing)) = crossing(destination)? else { continue };
                let ty = value_type(*value)?;
                let agrees = match reached {
                    Reached::Scalar(scalar) => package.types.get(ty) == Some(&TypeShape::PrimitiveScalar(scalar)),
                    Reached::Type(destination) => destination == ty,
                    _ => false,
                };
                if !agrees {
                    return Err(disagrees);
                }
                stores.push(WriteOnlyStore {
                    formal: formal(destination.root)?,
                    path: destination.path.clone(),
                    crossing,
                    field: None,
                    value: Parameter::Scalar(*value),
                });
            }
            OperationKind::StructuralStore {
                destination,
                value,
                disposition,
            } => {
                let Some((reached, crossing)) = crossing(destination)? else { continue };
                let Reached::Type(ty) = reached else { return Err(disagrees) };
                if body.places.get(value.index()).ok_or(MALFORMED)?.ty != ty {
                    return Err(disagrees);
                }
                let storage = checked_storage(package, ty, matches!(disposition, StructuralStoreDisposition::ReplaceWriteOnly(_)));
                match disposition {
                    StructuralStoreDisposition::ReplaceUnrestricted if storage == Some((true, omega_psi::WriteOnlyCleanup::NoCodeAffine)) => {}
                    StructuralStoreDisposition::ReplaceUnrestricted => return Err("a write-only replacement displaces content that is not freely discardable"),
                    StructuralStoreDisposition::ReplaceWriteOnly(authority) if storage.is_some_and(|(copy, retained)| retained == *authority && !(copy && retained == omega_psi::WriteOnlyCleanup::NoCodeAffine)) => {}
                    StructuralStoreDisposition::ReplaceWriteOnly(_) => return Err("a write-only replacement lacks its exact type's cleanup authority"),
                    _ => return Err("a write-only loan can neither take nor dispose the displaced value"),
                }
                let mut path = destination.path.clone();
                let field = match path.last() {
                    Some(PlaceProjection::Field(field)) if path.len() > crossing + 1 => {
                        let field = field.clone();
                        path.pop();
                        Some(field)
                    }
                    _ => None,
                };
                stores.push(WriteOnlyStore {
                    formal: formal(destination.root)?,
                    path,
                    crossing,
                    field,
                    value: Parameter::Structural(*value),
                });
            }
            OperationKind::ProjectedScalarRead { source }
            | OperationKind::CopyStructural { source }
            | OperationKind::MoveStructural { source }
            | OperationKind::BorrowedMove { source }
            | OperationKind::DecomposeRecord { source, .. }
            | OperationKind::StructuralCaseMembership { source, .. } => {
                if crossing(source)?.is_some() {
                    return Err("an operation reads or takes through a write-only loan");
                }
            }
            OperationKind::BorrowPlace { source, access } => reborrow(crossing(source)?, *access)?,
            OperationKind::Call { arguments, .. } => {
                for argument in arguments {
                    if let CallArgument::Borrow { place, access } = argument {
                        reborrow(crossing(place)?, *access)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(stores)
}

/// A reborrow through a write-only loan forwards it unread only with write-only access.
fn reborrow(
    crossing: Option<(Reached, usize)>,
    access: ReferenceAccess,
) -> Result<(), &'static str> {
    match crossing {
        Some(_) if access != ReferenceAccess::WriteOnly => Err("a readable reborrow crosses a write-only loan"),
        _ => Ok(()),
    }
}
