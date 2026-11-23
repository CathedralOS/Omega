//! Follow a projected place, in a body's types and in an execution's values.
//! Both walks take the same steps, so admission and execution agree on what a
//! path selects.
use crate::values::{Value, schema};
use omega_psi::{DataId, LoweredPackage, MachineBody, Multiplicity, PlaceProjection, ProjectedPlace, ReferenceAccess, TypeId, TypeShape};

/// The type a path selects below its root, when every step is one this
/// executor follows: a shared referent, a declared field, the payload of a
/// named case, or a fixed element.
pub(crate) fn projected_type(
    unit: &LoweredPackage,
    body: &MachineBody,
    place: &ProjectedPlace,
) -> Option<TypeId> {
    let mut ty = body.places.get(place.root.index())?.ty;
    let mut case = None;
    for step in &place.path {
        let shape = unit.types.get(ty)?;
        match (step, shape) {
            (
                PlaceProjection::Referent,
                TypeShape::NominalReference {
                    referent,
                    access: ReferenceAccess::Shared,
                },
            ) if case.is_none() => ty = schema(unit, *referent)?.carrier,
            (PlaceProjection::Case(declaration), TypeShape::Nominal(id)) if case.is_none() => {
                case = Some(
                    schema(unit, *id)?
                        .cases
                        .iter()
                        .position(|candidate| &candidate.declaration == declaration)?,
                );
            }
            (PlaceProjection::Field(declaration), TypeShape::Nominal(id)) => {
                let data = schema(unit, *id)?;
                let fields = match case.take() {
                    Some(case) => &data.cases[case].fields,
                    None => &data.fields,
                };
                ty = fields
                    .iter()
                    .find(|field| &field.declaration == declaration)?
                    .carrier;
            }
            (PlaceProjection::FixedIndex(index), TypeShape::FixedArray { element, length }) if case.is_none() && index < length => ty = *element,
            _ => return None,
        }
    }
    case.is_none().then_some(ty)
}

/// The value a path selects below `root`. `None` when the value does not have
/// the shape the path was admitted against, such as another active case.
pub(crate) fn projected_value<'value>(
    unit: &LoweredPackage,
    root: &'value Value,
    path: &[PlaceProjection],
) -> Option<&'value Value> {
    let mut value = root;
    let mut selected = None;
    for step in path {
        match (step, value) {
            (PlaceProjection::Referent, Value::Shared(referent)) if selected.is_none() => value = referent,
            (PlaceProjection::Case(declaration), Value::Case { data, case, .. }) if selected.is_none() => {
                if &schema(unit, *data)?.cases.get(*case)?.declaration != declaration {
                    return None;
                }
                selected = Some(*case);
            }
            (PlaceProjection::Field(declaration), Value::Case { data, case, fields }) if selected.take() == Some(*case) => {
                let position = field_position(unit, *data, Some(*case), declaration)?;
                value = fields.get(position)?;
            }
            (PlaceProjection::Field(declaration), Value::Record { data, fields }) if selected.is_none() => {
                let position = field_position(unit, *data, None, declaration)?;
                value = fields.get(position)?;
            }
            (PlaceProjection::FixedIndex(index), Value::Array(elements)) if selected.is_none() => value = elements.get(usize::try_from(*index).ok()?)?,
            _ => return None,
        }
    }
    selected.is_none().then_some(value)
}

pub(crate) fn field_position(
    unit: &LoweredPackage,
    data: DataId,
    case: Option<usize>,
    field: &omega_psi::DeclarationId,
) -> Option<usize> {
    let data = schema(unit, data)?;
    let fields = match case {
        Some(case) => &data.cases.get(case)?.fields,
        None => &data.fields,
    };
    fields
        .iter()
        .position(|candidate| &candidate.declaration == field)
}

/// Two rows name the same type when they are the same row or the same shape.
pub(crate) fn same_type(
    unit: &LoweredPackage,
    left: TypeId,
    right: TypeId,
) -> bool {
    left == right || (unit.types.get(left).is_some() && unit.types.get(left) == unit.types.get(right))
}

/// How a type is used by an admitted body.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Use {
    /// Held, moved or returned.
    Hold,
    /// Read into independent storage, which needs every part to be `copy`.
    Copy,
}

/// Whether this executor holds values of `ty`, and copies them when asked.
/// Data with an authored cleanup is excluded: discarding it would run a
/// machine, and no admitted edge does.
pub(crate) fn supports(
    unit: &LoweredPackage,
    ty: TypeId,
    usage: Use,
) -> bool {
    let mut visited = vec![false; unit.data.len()];
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let nominal = match unit.types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::SharedByteView) => continue,
            Some(TypeShape::NominalReference {
                access: ReferenceAccess::Shared,
                referent,
            }) => {
                // Copying a shared reference copies the descriptor, not the referent.
                if usage == Use::Hold {
                    let Some(data) = schema(unit, *referent) else { return false };
                    pending.push(data.carrier);
                }
                continue;
            }
            // A by-value sequence is complete and immutable, and lowering
            // transports it by whole copy; its elements decide whether it may be.
            Some(TypeShape::FixedArray { element, .. } | TypeShape::ElementSequence { element }) => {
                pending.push(*element);
                continue;
            }
            Some(TypeShape::Nominal(id)) => *id,
            _ => return false,
        };
        let Some(data) = schema(unit, nominal) else { return false };
        if std::mem::replace(&mut visited[nominal.get() - 1], true) {
            continue;
        }
        if data.cleanup_target.is_some() || !data.value_indices.is_empty() || (usage == Use::Copy && data.multiplicity != Multiplicity::Copy) {
            return false;
        }
        pending.extend(data.fields.iter().map(|field| field.carrier));
        pending.extend(
            data.cases
                .iter()
                .flat_map(|case| &case.fields)
                .map(|field| field.carrier),
        );
    }
    true
}

/// Whether a value of `ty` can hold a reference to a record or sum, in a
/// field, a case payload or an element.
pub(crate) fn holds_reference(
    unit: &LoweredPackage,
    ty: TypeId,
) -> bool {
    let mut visited = vec![false; unit.data.len()];
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match unit.types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::SharedByteView) => {}
            Some(TypeShape::FixedArray { element, .. } | TypeShape::ElementSequence { element }) => pending.push(*element),
            Some(TypeShape::Nominal(id)) => {
                let Some(data) = schema(unit, *id) else { return true };
                if !std::mem::replace(&mut visited[id.get() - 1], true) {
                    pending.extend(data.fields.iter().map(|field| field.carrier));
                    pending.extend(
                        data.cases
                            .iter()
                            .flat_map(|case| &case.fields)
                            .map(|field| field.carrier),
                    );
                }
            }
            _ => return true,
        }
    }
    false
}
