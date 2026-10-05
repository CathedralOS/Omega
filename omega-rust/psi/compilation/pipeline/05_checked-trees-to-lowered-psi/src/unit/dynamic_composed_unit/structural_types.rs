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

/// The caller's context parameter declaration, rejoined to the roster the
/// checked plan publishes.
///
/// Both single-call and continuation lowering used to synthesize this
/// declaration from `caller_attachment_type_identity` alone, at a fixed
/// position 0: an assertion that the caller's only runtime parameter is the
/// data the machine is qualified by. Under named implementations that data is
/// an ordinary explicit parameter, so the claim is checkable, and the checked
/// plan now publishes the roster to check it against.
///
/// The lane declares exactly one structural parameter, so a caller whose
/// roster holds more than the context place is refused by name here rather
/// than lowered with the rest of its signature dropped.
pub(crate) fn caller_context_parameter(
    view: &crate::unit::dynamic_composed_unit::dynamic_lanes::DynamicCallView<'_>,
    structural_type: semantic_vocabulary::StructuralTypeId,
    place: semantic_vocabulary::PlaceId,
) -> Result<terminal_psi::StructuralParameterDeclaration, LoweringError> {
    let [context] = view.caller_structural_parameters else {
        return unsupported(
            "direct dynamic caller must carry its context as its one structural parameter",
        );
    };
    if context.position != view.source_parameter_position
        || context.type_identity != view.caller_attachment_type_identity
        || context.access != view.caller_parameter_access
    {
        return unsupported("direct dynamic caller context parameter drifted from its roster");
    }
    Ok(terminal_psi::StructuralParameterDeclaration {
        place,
        position: context.position,
        structural_type,
        multiplicity: LOAN_MULTIPLICITY,
        access: match context.access {
            checked_trees::CheckedStructuralAccess::SharedBorrow => {
                terminal_psi::StructuralAccess::SharedBorrow
            }
            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                terminal_psi::StructuralAccess::MutableBorrow
            }
            _ => return unsupported("direct dynamic caller requires a borrowed context parameter"),
        },
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    })
}
