//! Plain owned arrivals retain semantic custody without executable storage uses.
use super::shared::*;

pub(super) fn has_block_arrivals(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    function
        .block_entries
        .iter()
        .any(|block| !block.structural_parameters.is_empty())
        && accepts(function, types)
}

pub(super) fn parameter(parameter: &terminal_psi::StructuralParameterDeclaration) -> bool {
    parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

pub(super) fn accepts(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    if function.structural_parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || function.attachment.is_some()
    {
        return false;
    }
    let declarations = function
        .structural_parameters
        .iter()
        .chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .collect::<Vec<_>>();
    if declarations.iter().any(|declaration| {
        !parameter(declaration)
            || !plain_type(declaration.structural_type, types, &mut BTreeSet::new())
    }) {
        return false;
    }
    function.operations.iter().all(|operation| match operation {
        AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::Call { .. } => true,
        AbstractOperation::Return {
            cleanup_actions, ..
        }
        | AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } => cleanup(function, cleanup_actions),
        AbstractOperation::Jump {
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            trivial_affine_discards.is_empty()
                && residual_affine_discards.is_empty()
                && bindings(&declarations, structural_bindings)
        }
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false].iter().all(|edge| {
            edge.trivial_affine_discards.is_empty()
                && bindings(&declarations, &edge.structural_bindings)
        }),
        // Calls carrying structural actuals and all structural observations or
        // establishments need their own physical realization, even if unused.
        _ => false,
    })
}

fn bindings(
    declarations: &[&terminal_psi::StructuralParameterDeclaration],
    bindings: &[abstract_operations::AbstractStructuralBinding],
) -> bool {
    bindings.iter().all(|binding| {
        let source = declarations.iter().find(|parameter| parameter.place == binding.argument.place);
        let target = declarations.iter().find(|parameter| parameter.place == binding.parameter);
        matches!((source, target), (Some(source), Some(target))
            if source.structural_type == target.structural_type && source.multiplicity == target.multiplicity)
            && binding.argument.access == StructuralAccess::Owned && binding.argument.path.is_empty()
    })
}

/// Current ownership validates the live frontier and exact disposal order.
pub(super) fn cleanup(
    function: &AbstractFunction,
    actions: &[TerminalAffineCleanupAction],
) -> bool {
    let mut seen = BTreeSet::new();
    actions.iter().all(|action| {
        let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
            return false;
        };
        seen.insert(*place)
            && function
                .structural_parameters
                .iter()
                .chain(
                    function
                        .block_entries
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .any(|declaration| {
                    declaration.place == *place
                        && parameter(declaration)
                        && declaration.multiplicity == StructuralMultiplicity::Affine
                })
    })
}

fn plain_type(
    identity: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !active.insert(identity) {
        return false;
    }
    let result = types
        .get(&identity)
        .is_some_and(|declaration| match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::Record { fields } => fields.iter().all(|field| {
                field.relevance.is_erased()
                    || match field.field_type {
                        StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => true,
                        StructuralFieldType::Structural(nested) => {
                            plain_type(nested, types, active)
                        }
                        _ => false,
                    }
            }),
            StructuralTypeShape::FixedArray { element, length } => {
                *length > 0 && plain_type(*element, types, active)
            }
            _ => false,
        });
    active.remove(&identity);
    result
}
