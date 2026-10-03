//! Aggregate results and block arrivals establish durable structural homes.
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use semantic_vocabulary::{OperationId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

fn sum_layout(
    structural_type: StructuralTypeId,
    types: &StructuralTypeLookup<'_>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralSum(structural_type);
    let declaration = types.get(&structural_type).ok_or_else(invalid)?;
    if !matches!(
        declaration.shape,
        StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. }
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
    if !super::super::unobserved_owned::parameter(declaration)
        && !super::super::scalar::byte_views::is_owned_byte_parameter(declaration, types)
    {
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
    // Qualification rosters are caller-visible custody evidence replayed
    // through the requirement's origin; they do not change the carrier's
    // physical layout, so only linear multiplicity still rejects here.
    if result.multiplicity == StructuralMultiplicity::Linear {
        return Err(LoweringError::UnsupportedStructuralSum(
            result.structural_type,
        ));
    }
    if matches!(
        types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape),
        Some(
            StructuralTypeShape::Record { .. }
                | StructuralTypeShape::FixedArray { .. }
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
                result.structural_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
        ));
    }
    Ok(TargetStructuralHomeLayout::Sum(sum_layout(
        result.structural_type,
        types,
    )?))
}

pub(in crate::lowering) fn home(
    operation: OperationId,
    result: &terminal_psi::StructuralOperationResult,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    if !result.claims.is_empty() {
        return Err(LoweringError::UnsupportedStructuralSum(
            result.structural_type,
        ));
    }
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
