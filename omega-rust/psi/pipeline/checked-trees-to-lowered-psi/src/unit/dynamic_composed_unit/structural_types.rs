//! Structural multiplicities and structural types of dynamic sources.

use crate::unit::{CheckedTrees, LoweringError, attached_unit, unsupported};
use checked_trees::{
    CheckedDynamicScalarCallPlan, CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape,
};
use language_semantics::Multiplicity;
use terminal_psi::StructuralMultiplicity;

pub(crate) fn terminal_structural_multiplicity(
    multiplicity: Multiplicity,
) -> StructuralMultiplicity {
    match multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
    }
}

/// A shared field projection retains its caller root's consumption bound even
/// when the projected field's own declared carrier is copyable.
pub(crate) fn terminal_projected_source_multiplicity_for(
    caller_multiplicity: Multiplicity,
) -> StructuralMultiplicity {
    match caller_multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
    }
}

pub(crate) fn lower_dynamic_structural_types(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    caller_attachment: &str,
) -> Result<
    (
        Vec<terminal_psi::StructuralTypeDeclaration>,
        Vec<(String, semantic_vocabulary::StructuralTypeId)>,
    ),
    LoweringError,
> {
    lower_dynamic_structural_types_for_source(
        checked,
        caller_attachment,
        &plan.caller_attachment_type_identity,
        &plan.source_path,
        &plan.source_type_identity,
    )
}

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
        &[
            caller_attachment.to_owned(),
            source_type_identity.to_owned(),
        ],
    )
}
