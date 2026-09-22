//! Plain owned arrivals retain semantic custody without executable storage uses.
//! The whole-function gate answers one question: whether every owned arrival
//! stays unobserved so the graph suppresses its storage home. Edge cleanup no
//! longer consults it — each action admits by its root's own home or arrival
//! declaration where the edge commits the discard.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
};
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
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerRemainder { .. }
        | AbstractOperation::WrappingIntegerSubtract { .. }
        | AbstractOperation::WrappingIntegerMultiply { .. }
        | AbstractOperation::WrappingIntegerDivide { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::Call { .. } => true,
        AbstractOperation::EstablishPrimitiveLocal { result, .. } => !declarations
            .iter()
            .any(|parameter| parameter.place == result.place),
        AbstractOperation::PrimitiveLocalStore { destination, .. }
        | AbstractOperation::PrimitiveScalarRead {
            source: destination,
            ..
        } => {
            local(function, *destination)
                && !declarations
                    .iter()
                    .any(|parameter| parameter.place == *destination)
        }
        AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } => structural_arguments.iter().all(|argument| {
            argument.access != StructuralAccess::Owned
                && argument.path.is_empty()
                && local(function, argument.place)
                && !declarations
                    .iter()
                    .any(|parameter| parameter.place == argument.place)
        }),
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
            bindings(&declarations, structural_bindings)
                && edge_discards(
                    &declarations,
                    structural_bindings,
                    trivial_affine_discards,
                    residual_affine_discards,
                    types,
                )
        }
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false].iter().all(|edge| {
            bindings(&declarations, &edge.structural_bindings)
                && edge_discards(
                    &declarations,
                    &edge.structural_bindings,
                    &edge.trivial_affine_discards,
                    &[],
                    types,
                )
        }),
        // Owned parameters remain unobserved even when unrelated local storage
        // is borrowed. All other structural operations need their own realization.
        _ => false,
    })
}

fn local(function: &AbstractFunction, place: PlaceId) -> bool {
    function.operations.iter().any(|operation| {
        matches!(operation,
        AbstractOperation::EstablishPrimitiveLocal { result, .. } if result.place == place)
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

/// One declared owned affine arrival whose home this gate may suppress — the
/// same admission `plain_home_cleanup` gives an arrival-declaration discard.
fn discardable_root(
    declarations: &[&terminal_psi::StructuralParameterDeclaration],
    place: PlaceId,
) -> bool {
    declarations.iter().any(|declaration| {
        declaration.place == place
            && parameter(declaration)
            && declaration.multiplicity == StructuralMultiplicity::Affine
    })
}

/// An edge's discards admit only what the terminator realizes per action:
/// root discards on declared owned affine arrivals, and projected residuals
/// on those same roots — each a strictly pathed plain subtree that overlaps
/// neither an earlier discard nor a transferred structural argument.
fn edge_discards(
    declarations: &[&terminal_psi::StructuralParameterDeclaration],
    structural_bindings: &[abstract_operations::AbstractStructuralBinding],
    trivial_discards: &[PlaceId],
    residual_discards: &[terminal_psi::StructuralAffineDiscard],
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    let mut roots = BTreeSet::new();
    if !trivial_discards
        .iter()
        .all(|place| roots.insert(*place) && discardable_root(declarations, *place))
    {
        return false;
    }
    let mut residuals = BTreeSet::new();
    residual_discards.iter().all(|discard| {
        let overlaps_prior =
            residuals
                .iter()
                .any(|(place, path): &(PlaceId, Vec<StructuralPathSegment>)| {
                    *place == discard.place
                        && (path.starts_with(&discard.path) || discard.path.starts_with(path))
                });
        !discard.path.is_empty()
            && discardable_root(declarations, discard.place)
            && !roots.contains(&discard.place)
            && !overlaps_prior
            && !structural_bindings.iter().any(|binding| {
                binding.argument.place == discard.place
                    && (binding.argument.path.starts_with(&discard.path)
                        || discard.path.starts_with(&binding.argument.path))
            })
            && plain_type(discard.structural_type, types, &mut BTreeSet::new())
            && residuals.insert((discard.place, discard.path.clone()))
    })
}

/// Every cleanup action an unobserved-owned function carries must be a discard
/// of a declared owned affine arrival — realizability is then decided per
/// action at the edge, not by this gate.
fn cleanup(function: &AbstractFunction, actions: &[TerminalAffineCleanupAction]) -> bool {
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
    let mut seen = BTreeSet::new();
    actions.iter().all(|action| {
        let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
            return false;
        };
        seen.insert(*place) && discardable_root(&declarations, *place)
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
                        // Bounds remain part of the declaration and source proof;
                        // they do not make an unobserved scalar payload executable.
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_) => true,
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
