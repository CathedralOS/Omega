//! Aggregate values retain their declaration across construction and storage.
//! A borrowed view owns its descriptor value, never its backing. Consumers
//! therefore use the declared multiplicity rather than inferring copy permission
//! from a descriptor layout or requiring an owned referent parameter.
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use semantic_vocabulary::{OperationId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

/// Resolve a consumed value from its live storage, independent of whether a
/// constructor or a replacement receives it. Static projections select owned
/// children; borrowed arrivals can supply only their complete view descriptor.
/// Their referents remain inaccessible through this owned-value operation.
pub(super) fn value_input(
    function: &abstract_operations::AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &super::LiveDefinitions,
    argument: &terminal_psi::StructuralArgument,
) -> Result<(StructuralTypeId, terminal_psi::StructuralMultiplicity), LoweringError> {
    let invalid = |reason| LoweringError::InvalidStructuralValueInput {
        machine: function.machine,
        place: argument.place,
        reason,
    };
    if argument.access != terminal_psi::StructuralAccess::Owned {
        return Err(invalid(
            "a value input must transfer or copy its descriptor, not borrow it",
        ));
    }
    let (root, multiplicity) = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.access() != terminal_psi::StructuralAccess::Owned {
            return Err(invalid("the live home does not own the selected value"));
        }
        (home.structural_type(), home.multiplicity())
    } else if let Some((producer, structural_type)) = live.views.get(&argument.place) {
        // Literal descriptors publish a literal declaration, while computed
        // descriptors publish a structural result. Both must rejoin the exact
        // live producer; only the literal has intrinsic copy multiplicity.
        let mut producers = function
            .operations
            .iter()
            .filter(|operation| operation.psi_operation() == Some(*producer));
        let operation = producers
            .next()
            .ok_or_else(|| invalid("the live descriptor producer is absent"))?;
        if producers.next().is_some() {
            return Err(invalid("the live descriptor names duplicate producers"));
        }
        let declared = if let Some((_, result)) = operation.structural_result() {
            if result.place != argument.place {
                return Err(invalid("the descriptor producer publishes another place"));
            }
            (result.structural_type, result.multiplicity)
        } else if let abstract_operations::AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = operation
        {
            if place.id != argument.place || !structural_type.shape.is_shared_readable_byte_view() {
                return Err(invalid(
                    "the literal does not publish this immutable byte descriptor",
                ));
            }
            (
                structural_type.id,
                terminal_psi::StructuralMultiplicity::Unrestricted,
            )
        } else {
            return Err(invalid(
                "the live descriptor producer has no value declaration",
            ));
        };
        if declared.0 != *structural_type {
            return Err(invalid(
                "the descriptor producer type differs from its live declaration",
            ));
        }
        declared
    } else {
        let parameter = function.arrival_parameter(argument.place).ok_or_else(|| {
            invalid("the value has no live home, descriptor or arrival declaration")
        })?;
        if parameter.access != terminal_psi::StructuralAccess::Owned
            && (!argument.path.is_empty()
                || !terminal_semantics::view_descriptor_parameter(
                    parameter,
                    types.values().copied(),
                )
                || live.structural_observation_type(function, argument.place)
                    != Some(parameter.structural_type))
        {
            return Err(invalid(
                "a borrowed arrival can supply only its live whole view descriptor",
            ));
        }
        (parameter.structural_type, parameter.multiplicity)
    };
    let selected = terminal_semantics::canonical_static_projection(
        types.values().copied(),
        root,
        &argument.path,
    )
    .map(|(_, declaration)| declaration.id)
    .ok_or_else(|| invalid("the value projection does not select a declared owned child"))?;
    Ok((selected, multiplicity))
}

fn sum_layout(
    structural_type: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralSum(structural_type);
    let declaration = types.get(&structural_type).ok_or_else(invalid)?;
    if !matches!(
        declaration.shape,
        terminal_psi::sum_or_mixed_shape_pattern!()
    ) {
        return Err(invalid());
    }
    crate::lowering::structural_layout::structural_sum_layout(
        structural_type,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

/// A block arrival establishes its own home under its exact declaration. It is
/// neither a call result nor an alias of whichever predecessor happens to run.
pub(super) fn block_home(
    block: semantic_vocabulary::BlockId,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    if !super::owned_arguments::is_owned_parameter(declaration, types) {
        return Err(LoweringError::UnsupportedStructuralSum(
            declaration.structural_type,
        ));
    }
    // Arrival changes the establishment identity, not the concrete value layout.
    let result = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: declaration.place,
        structural_type: declaration.structural_type,
        multiplicity: declaration.multiplicity,
        qualifications: declaration.qualifications.clone(),
        projected_qualifications: declaration.projected_qualifications.clone(),
    };
    Ok(TargetStructuralHomeRequirement {
        origin: target_operations::TargetStructuralHomeOrigin::BlockParameter {
            block,
            declaration: declaration.clone(),
        },
        layout: result_home_layout(&result, types)?,
    })
}

/// Structural result category selects its layout, not a different call graph.
pub(in crate::lowering) fn result_home_layout(
    result: &terminal_psi::StructuralResultDeclaration,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeLayout, LoweringError> {
    // Claims and qualifications remain in the exact home origin. They govern
    // ownership admission, while the complete carrier type determines layout.
    value_home_layout(result.structural_type, types)
}

/// Layout is a property of the complete value type, whether it arrives as an
/// argument, a block parameter, or a call result. Ownership admission remains
/// with those operations; a layout alone grants no transfer or borrow rights.
pub(super) fn value_home_layout(
    structural_type: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralHomeLayout, LoweringError> {
    if matches!(
        types
            .get(&structural_type)
            .map(|declaration| &declaration.shape),
        Some(
            terminal_psi::record_or_fixed_array_shape_pattern!()
                | StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::PrimitiveScalar(_)
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::ElementView { .. },
        )
    ) {
        // Whole aggregate transport needs recursive size/alignment, not scalar
        // leaves. Array construction still checks its element operations, while
        // parameter and call-result homes retain existing owned record arrays.
        return Ok(TargetStructuralHomeLayout::Aggregate(
            crate::lowering::structural_layout::structural_shape(
                structural_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
        ));
    }
    Ok(TargetStructuralHomeLayout::Sum(sum_layout(
        structural_type,
        types,
    )?))
}

pub(in crate::lowering) fn home(
    operation: OperationId,
    result: &terminal_psi::StructuralOperationResult,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    let declaration = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result.place,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: result.qualifications.clone(),
        projected_qualifications: result.projected_qualifications.clone(),
    };
    Ok(TargetStructuralHomeRequirement {
        origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout: result_home_layout(&declaration, types)?,
    })
}
