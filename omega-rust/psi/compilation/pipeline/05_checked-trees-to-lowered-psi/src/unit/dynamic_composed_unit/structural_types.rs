//! Structural multiplicities and structural types of dynamic sources.

use crate::unit::{CheckedTrees, LoweringError, attached_unit, unsupported};
use checked_trees::{
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape,
};
use terminal_psi::StructuralMultiplicity;

/// Terminal multiplicity of a borrowed caller `self` and of the field subloan
/// it lends to a realization's borrowed `self`. A loan never owns its
/// referent; the attachment's own multiplicity stays with the place that owns
/// the attachment. Ordinary signatures declare every `&T` and `&mut T`
/// parameter this way (`TypedTrees::type_multiplicity`), and the verifier
/// requires it of any loan whose type carries a `Binding`: an `Affine` loan
/// would claim custody of service authority that the loan does not hold.
/// The checked plan still carries the attachment multiplicity, and plan
/// validation replays it; only the loan's own declaration is fixed here.
pub(crate) const LOAN_MULTIPLICITY: StructuralMultiplicity = StructuralMultiplicity::Unrestricted;

pub(crate) fn lower_dynamic_structural_types_for_source(
    checked: &CheckedTrees,
    caller_attachment: &str,
    checked_caller_attachment: &str,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
) -> Result<
    (
        Vec<terminal_psi::StructuralTypeDeclaration>,
        Vec<(String, semantic_vocabulary::StructuralTypeId)>,
    ),
    LoweringError,
> {
    if caller_attachment != checked_caller_attachment {
        return unsupported("direct dynamic caller attachment identity drifted");
    }
    let roots = &checked.facts.flow.terminal_unit_effects.structural_types;
    let caller_roots = roots
        .iter()
        .filter(|candidate| candidate.identity == caller_attachment)
        .collect::<Vec<_>>();
    let [caller] = caller_roots.as_slice() else {
        return unsupported("direct dynamic caller attachment shape is absent or ambiguous");
    };
    let [CheckedUnitStructuralPathSegment::Field(source_field)] = source_path else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    let CheckedUnitStructuralTypeShape::Record { fields } = &caller.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *source_field
                && field.field_type
                    == CheckedUnitStructuralFieldType::Structural {
                        type_identity: source_type_identity.to_owned(),
                    }
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic checked source field no longer matches its carrier");
    }
    attached_unit::lower_unit_structural_type_roots(
        checked,
        attached_unit::bodies::UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        &[
            caller_attachment.to_owned(),
            source_type_identity.to_owned(),
        ],
    )
}
