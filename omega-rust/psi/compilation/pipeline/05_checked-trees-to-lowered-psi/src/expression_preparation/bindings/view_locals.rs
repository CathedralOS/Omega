//! Established byte owners and view locals, as scalar observations see them.
//!
//! A view local (an `as_slice` loan or a subslice of another view) publishes
//! one shared view place when its establishment completes. Lengths and
//! element reads rooted at the local observe that exact place with the
//! Terminal operation its view family selects, so the binding records the
//! family its produced type declared rather than re-deriving it from source.
//! Bounded byte owners use the same length/read observations on their own
//! home; this registry grants no borrow or mutation authority.
use super::{LoweringError, PlaceId, ScalarType, StructuralTypeId, unsupported};
use terminal_psi::{
    ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
};

/// The Terminal view family of an established view place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewCarrier {
    /// A byte owner or view: lengths count live bytes, reads yield `u8`.
    Bytes,
    /// A borrowed element view; `element` is the scalar a read yields, absent
    /// for record elements, which have no scalar read.
    Elements { element: Option<ScalarType> },
}

impl ViewCarrier {
    /// The observation family `structural_type` declares, if it has live extent.
    pub(crate) fn of(
        structural_type: StructuralTypeId,
        types: &[StructuralTypeDeclaration],
    ) -> Option<Self> {
        let declaration = find_structural_type(types, structural_type)?;
        match declaration.shape {
            StructuralTypeShape::ByteSequence(
                terminal_psi::byte_sequence_view_carrier_pattern!()
                | ByteSequenceCarrier::BoundedOwned { .. },
            ) => Some(Self::Bytes),
            StructuralTypeShape::ElementView { element, .. } => Some(Self::Elements {
                element: find_structural_type(types, element).and_then(|declaration| {
                    match declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
                        _ => None,
                    }
                }),
            }),
            _ => None,
        }
    }
}

/// One view local's `let` symbol and the place its establishment published.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewLocalBinding {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) carrier: ViewCarrier,
    pub(crate) access: terminal_psi::StructuralAccess,
}

/// The unique established view local `symbol` names.
pub(crate) fn resolve(
    view_locals: &[ViewLocalBinding],
    symbol: symbols::SymbolHandle,
) -> Result<ViewLocalBinding, LoweringError> {
    let mut matching = view_locals.iter().filter(|local| local.symbol == symbol);
    let Some(local) = matching.next() else {
        return unsupported("view observation names no established view local");
    };
    if !symbol.is_valid() || matching.next().is_some() {
        return unsupported("view local identity is missing or duplicated");
    }
    Ok(*local)
}
