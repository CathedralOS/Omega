//! Resolve a representation view against the declarations that supplied its storage.
mod scalars;
pub use scalars::{
    ScalarRepresentationViewPair, ScalarViewValue, reference_storage_byte_width,
    reference_storage_view_is_compatible, scalar_reference_view_is_total,
    scalar_reference_view_value,
};

use crate::{StructuralTypeDeclaration, StructuralTypeShape};
use semantic_vocabulary::{ScalarType, StructuralTypeId};

/// A view needs one declaration of the requested type. A stored scalar must fit
/// the declared scalar view. Aggregate views name a record or fixed array.
pub fn declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration>,
    view: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Option<&'types StructuralTypeDeclaration> {
    let mut matching = types.filter(|declaration| declaration.id == view);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match (stored, &declaration.shape) {
        (Some(stored), StructuralTypeShape::PrimitiveScalar(logical))
            if reference_storage_view_is_compatible(stored, *logical) => {}
        (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {}
        _ => return None,
    }
    Some(declaration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StructuralMultiplicity, TerminalIdentityIndex};

    #[test]
    fn a_stored_scalar_view_needs_one_compatible_declaration() {
        let view = StructuralTypeId::new(1).unwrap();
        let declared = StructuralTypeDeclaration {
            id: view,
            identity: TerminalIdentityIndex::ABSENT,
            declaration_identity: TerminalIdentityIndex::ABSENT,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            multiplicity: StructuralMultiplicity::Unrestricted,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            quotient_carrier: None,
            scalar_restriction: None,
            requires: Vec::new(),
        };
        let rows = [&declared];
        assert_eq!(
            declaration(rows.into_iter(), view, Some(ScalarType::Boolean)),
            Some(&declared)
        );
        assert_eq!(declaration(rows.into_iter(), view, None), None);
        assert_eq!(
            declaration(
                rows.into_iter(),
                view,
                Some(ScalarType::IeeeFloat(
                    semantic_vocabulary::IeeeFloatFormat::Binary32
                ))
            ),
            None
        );
        assert_eq!(
            declaration(
                [&declared, &declared].into_iter(),
                view,
                Some(ScalarType::Boolean)
            ),
            None
        );
    }
}
