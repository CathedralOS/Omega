//! Read scalar element writes from published Psi alone
//! (`wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views`).
//!
//! An `ElementViewWrite` needs an exclusive scalar element view, a value of the
//! element's exact carrier, a `u64` index, and the same view's element count
//! observed immediately before the write. This reader checks each of those
//! against the body and refuses any write that lacks one.
use omega_psi::{LoweredPackage, OperationKind, Parameter, PlaceOrigin, ScalarType, TypeShape, ValueId, ValueOrigin};

use super::machine;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementWrite {
    /// The entry formal that is the written view.
    pub formal: usize,
    pub element: ScalarType,
    pub index: ValueId,
    pub value: ValueId,
}

pub fn element_writes(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<ElementWrite>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let mut writes = Vec::new();
    for (position, op) in body.operations.iter().enumerate() {
        let OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            value,
            length,
            ..
        } = &op.kind
        else {
            continue;
        };
        if !path.is_empty() {
            return Err("this reader checks whole scalar elements only");
        }
        let place = body.places.get(destination.index()).ok_or("missing destination")?;
        let Some(TypeShape::MutableElementView { element: omega_psi::ViewElement::Scalar(element) }) = package.types.get(place.ty) else {
            return Err("an element write needs an exclusive scalar element view");
        };
        let formal = match place.origin {
            PlaceOrigin::Parameter => body
                .parameters
                .iter()
                .position(|p| *p == Parameter::Structural(*destination))
                .ok_or("missing view formal")?,
            _ => return Err("this reader expects the view formal itself"),
        };
        let value_ty = body.values.get(value.index()).ok_or("missing value")?.ty;
        if package.types.get(value_ty) != Some(&TypeShape::PrimitiveScalar(*element)) {
            return Err("the written value disagrees with the element carrier");
        }
        let index_ty = body.values.get(index.index()).ok_or("missing index")?.ty;
        if package.types.get(index_ty) != Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 })) {
            return Err("an element index must be a u64");
        }
        // The length is the same view's count, observed by the operation just before.
        let observed = position
            .checked_sub(1)
            .and_then(|before| body.operations.get(before))
            .filter(|before| matches!(before.kind, OperationKind::ElementViewLength { source } if source == *destination))
            .is_some_and(|before| matches!(body.values.get(length.index()).map(|v| &v.origin), Some(ValueOrigin::Operation(id)) if *id == before.id));
        if !observed {
            return Err("an element write needs the same view's length observed immediately before");
        }
        writes.push(ElementWrite {
            formal,
            element: *element,
            index: *index,
            value: *value,
        });
    }
    Ok(writes)
}
