//! Read by-value immutable sequences from published Psi alone
//! (`wiki/spec/terminal-psi/structural_access.md#complete-structural-array-construction`).
//!
//! A `[T]` held by value is established by one `EstablishStructuralArray`
//! whose result type is an `ElementSequence`. That row names the exact row of
//! one element. The operation's ordered operands are the whole sequence: their
//! count is its stored length, and each is a place of exactly the element row,
//! complete before the sequence is published. A scalar element is a value, so
//! each one arrives in its own primitive home. This reader checks those facts
//! against the body and the type table. It refuses the same operation under a
//! fixed array, and any borrowed view, so a test cannot mistake either for a
//! sequence.
use omega_psi::{
    BlockTerminator, LoweredData, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceOrigin, PsiOperation, ReturnEdge,
    TypeId, TypeShape, ValueId,
};

use super::machine;

/// Where the place of one element comes from.
#[derive(Debug)]
pub enum ElementSource<'a> {
    /// An earlier operation of the same body published the place.
    Produced(&'a PsiOperation),
    /// The scalar value this element's primitive home was established from.
    Homed(ValueId),
    /// The place is this entry formal, supplied by copy.
    Formal(usize),
}

#[derive(Debug)]
pub struct EstablishedSequence<'a> {
    /// The place the establishing operation publishes.
    pub place: PlaceId,
    /// The sequence's own type row.
    pub ty: TypeId,
    /// The exact row of every element.
    pub element: TypeId,
    /// The operand places, in element order. Their count is the stored length.
    pub elements: &'a [PlaceId],
    /// Where each operand comes from, in element order.
    pub sources: Vec<ElementSource<'a>>,
}

/// The element row of a by-value sequence type, or why `ty` is not one.
pub fn sequence_element(
    package: &LoweredPackage,
    ty: TypeId,
) -> Result<TypeId, &'static str> {
    match package.types.get(ty).ok_or("missing type row")? {
        TypeShape::ElementSequence { element } => {
            // A row follows its element's row, so no sequence contains itself.
            if element.get() >= ty.get() || package.types.get(*element).is_none() {
                return Err("the sequence does not follow its element's row");
            }
            Ok(*element)
        }
        TypeShape::SharedElementView { .. } | TypeShape::MutableElementView { .. } | TypeShape::SharedByteView => {
            Err("a borrowed view is not a by-value sequence")
        }
        TypeShape::FixedArray { .. } => Err("a fixed array has a declared extent, not a stored length"),
        _ => Err("not a sequence type"),
    }
}

/// The record or sum declaration a type row names, with its own row checked.
pub fn nominal_row(
    package: &LoweredPackage,
    ty: TypeId,
) -> Option<&LoweredData> {
    let TypeShape::Nominal(data) = package.types.get(ty)? else {
        return None;
    };
    package
        .data
        .get(data.get() - 1)
        .filter(|row| row.id == *data && row.carrier == ty)
}

fn established<'a>(
    package: &'a LoweredPackage,
    body: &'a MachineBody,
    operation: &'a PsiOperation,
) -> Result<EstablishedSequence<'a>, &'static str> {
    let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
        return Err("the place is not established by a structural array construction");
    };
    let OperationResult::Structural(place) = operation.result else {
        return Err("the construction publishes no place");
    };
    let published = body.places.get(place.index()).ok_or("missing result place")?;
    if published.origin != PlaceOrigin::Operation(operation.id) {
        return Err("the result place names another origin");
    }
    let element = sequence_element(package, published.ty)?;
    let scalar = matches!(package.types.get(element), Some(TypeShape::PrimitiveScalar(_)));
    let mut sources = Vec::with_capacity(elements.len());
    for operand in elements {
        let supplied = body
            .places
            .get(operand.index())
            .ok_or("missing element place")?;
        if supplied.ty != element {
            return Err("an operand is not exactly the element's row");
        }
        let id = match supplied.origin {
            PlaceOrigin::Operation(id) => id,
            PlaceOrigin::Parameter if !scalar => {
                let formal = body
                    .parameters
                    .iter()
                    .position(|parameter| *parameter == Parameter::Structural(*operand))
                    .ok_or("an element names a place that is no formal")?;
                sources.push(ElementSource::Formal(formal));
                continue;
            }
            _ => return Err("an element is neither a completed operation result nor a formal"),
        };
        let producer = body
            .operations
            .get(id.index())
            .ok_or("missing element producer")?;
        // Every operand is complete before the sequence is published.
        if producer.id != id || id.get() >= operation.id.get() {
            return Err("an element is produced after its sequence");
        }
        if !scalar {
            sources.push(ElementSource::Produced(producer));
            continue;
        }
        // A scalar element is a fresh, immutable home of exactly that scalar,
        // held by this sequence alone.
        let OperationKind::EstablishPrimitiveLocal {
            value,
            receiving_domains: None,
        } = producer.kind
        else {
            return Err("a scalar element is not an unqualified primitive home");
        };
        if body.values.get(value.index()).map(|value| value.ty) != Some(element) {
            return Err("a scalar element's home holds another type");
        }
        if body.mutable_homes.contains(operand) || elements.iter().filter(|other| *other == operand).count() != 1 {
            return Err("a scalar element's home is writable or shared between elements");
        }
        sources.push(ElementSource::Homed(value));
    }
    Ok(EstablishedSequence {
        place,
        ty: published.ty,
        element,
        elements,
        sources,
    })
}

/// Every by-value sequence the machine establishes, in operation order.
pub fn established_sequences<'a>(
    package: &'a LoweredPackage,
    owner: &str,
) -> Result<Vec<EstablishedSequence<'a>>, &'static str> {
    let body = &machine(package, owner).ok_or("missing machine")?.body;
    body.operations
        .iter()
        .filter(|operation| match (&operation.kind, operation.result) {
            (OperationKind::EstablishStructuralArray { .. }, OperationResult::Structural(place)) => body
                .places
                .get(place.index())
                .is_some_and(|place| matches!(package.types.get(place.ty), Some(TypeShape::ElementSequence { .. }))),
            _ => false,
        })
        .map(|operation| established(package, body, operation))
        .collect()
}

/// The sequence a machine establishes and returns from its only block. The
/// signature, the returned place and the result place must be one sequence
/// type, and the return owes no cleanup: owned elements moved into the
/// sequence, and the sequence itself owes none.
pub fn returned_sequence<'a>(
    package: &'a LoweredPackage,
    owner: &str,
) -> Result<EstablishedSequence<'a>, &'static str> {
    let machine = machine(package, owner).ok_or("missing machine")?;
    let body = &machine.body;
    let [block] = body.blocks.as_slice() else {
        return Err("this reader expects one block");
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        cleanup,
        ..
    } = &block.terminator
    else {
        return Err("the machine returns no structural value");
    };
    if !cleanup.is_empty() {
        return Err("returning a sequence owes no cleanup");
    }
    let returned = body
        .places
        .get(source.index())
        .ok_or("missing returned place")?;
    let published = body.places.get(result.index()).ok_or("missing result place")?;
    if published.origin != PlaceOrigin::Result || machine.result_type != Some(returned.ty) || published.ty != returned.ty {
        return Err("the signature, returned place and result place disagree");
    }
    sequence_element(package, returned.ty)?;
    let PlaceOrigin::Operation(id) = returned.origin else {
        return Err("the returned sequence is not established in this body");
    };
    let sequence = established(
        package,
        body,
        body.operations
            .get(id.index())
            .ok_or("missing establishing operation")?,
    )?;
    if sequence.place != *source {
        return Err("the establishing operation publishes another place");
    }
    Ok(sequence)
}

/// The sequence type a machine takes as its only formal and returns as that
/// same place, with no operation and no cleanup: the whole value travels by
/// copy of its descriptor.
pub fn forwarded_sequence(
    package: &LoweredPackage,
    owner: &str,
) -> Result<TypeId, &'static str> {
    let machine = machine(package, owner).ok_or("missing machine")?;
    let body = &machine.body;
    let ([Parameter::Structural(formal)], [ty]) = (body.parameters.as_slice(), machine.parameter_types.as_slice()) else {
        return Err("this reader expects one structural formal");
    };
    let ([block], true) = (body.blocks.as_slice(), body.operations.is_empty()) else {
        return Err("forwarding a sequence needs no operation and one block");
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        cleanup,
        ..
    } = &block.terminator
    else {
        return Err("the machine returns no structural value");
    };
    let published = body.places.get(result.index()).ok_or("missing result place")?;
    let arrived = body.places.get(formal.index()).ok_or("missing formal place")?;
    if source != formal || !cleanup.is_empty() {
        return Err("the formal is not returned whole and free of cleanup");
    }
    if arrived.origin != PlaceOrigin::Parameter
        || arrived.ty != *ty
        || published.origin != PlaceOrigin::Result
        || published.ty != *ty
        || machine.result_type != Some(*ty)
    {
        return Err("the formal, the result place and the signature are not one type");
    }
    sequence_element(package, *ty)?;
    Ok(*ty)
}
