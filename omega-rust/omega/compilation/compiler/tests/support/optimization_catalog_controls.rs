//! Replay source-produced type catalogs through the public optimizer entrance.
use abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit;
use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_psi_structural_catalog,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::StructuralTypeShape;
use terminal_psi_to_abstract_operations::{ArtifactSections, lower_artifact};

pub(super) fn prepared_unit(
    artifact: &CanonicalTerminalArtifact,
) -> Result<PsiOptimizationUnit, String> {
    let bytes = terminal_codec::encode_module(artifact.semantic_module())
        .map_err(|error| format!("optimization control encoding: {error:?}"))?;
    let input = lower_artifact(ArtifactSections {
        semantic_bytes: &bytes,
    })
    .map_err(|error| format!("optimization control lowering: {error:?}"))?
    .into_optimization_artifact()
    .into_optimization_input();
    let (_, unit) =
        build_verified_psi_optimization_unit(input, psi_work::WorkSchedule::CURRENT.identity())
            .map_err(|error| format!("optimization control unit: {error:?}"))?
            .into_parts();
    // This is only the same structural catalog formation operation used by
    // whole-unit validation. No body, ownership or execution verdict is made.
    validate_psi_structural_catalog(&unit)
        .map_err(|error| format!("optimization control intact catalog: {error:?}"))?;
    reject_unknown_referent(&unit)?;
    Ok(unit)
}

fn unknown_type(
    unit: &PsiOptimizationUnit,
) -> Result<semantic_vocabulary::StructuralTypeId, String> {
    unit.structural_types
        .iter()
        .map(|row| row.id)
        .chain(unit.structural_types.referents().iter().map(|row| row.id))
        .map(|id| id.get())
        .max()
        .and_then(|id| id.checked_add(1))
        .and_then(semantic_vocabulary::StructuralTypeId::new)
        .ok_or_else(|| "optimization control cannot allocate an unknown type".into())
}

fn reject_unknown_referent(unit: &PsiOptimizationUnit) -> Result<(), String> {
    let (index, declaration) = unit
        .structural_types
        .iter()
        .enumerate()
        .find(|(_, row)| matches!(row.shape, StructuralTypeShape::Reference { .. }))
        .ok_or("optimization controls need an actual reference carrier")?;
    let mut invalid = unit.clone();
    let StructuralTypeShape::Reference { referent, .. } =
        &mut invalid.structural_types.make_mut()[index].shape
    else {
        return Err("optimization control lost its reference".into());
    };
    *referent = unknown_type(unit)?;
    invalid.identity = optimization_unit::recompute_psi_optimization_unit_identity(&invalid);
    match validate_psi_structural_catalog(&invalid) {
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(actual))
            if actual == declaration.id =>
        {
            Ok(())
        }
        result => Err(format!(
            "unknown referent expected an exact carrier-identity refusal, got {result:?}"
        )),
    }
}

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let unit = prepared_unit(artifact)?;
    let (index, _) = unit
        .structural_types
        .iter()
        .enumerate()
        .find(|(_, row)| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
        .ok_or("optimization controls need an actual element view")?;
    let mut invalid = unit.clone();
    let StructuralTypeShape::ElementView { element, .. } =
        &mut invalid.structural_types.make_mut()[index].shape
    else {
        return Err("optimization control lost its view".into());
    };
    let unknown = unknown_type(&unit)?;
    *element = unknown;
    invalid.identity = optimization_unit::recompute_psi_optimization_unit_identity(&invalid);
    match validate_psi_structural_catalog(&invalid) {
        Err(OptimizationUnitValidationError::UnknownStructuralType(actual))
            if actual == unknown =>
        {
            Ok(())
        }
        result => Err(format!(
            "unknown view element expected an exact target-identity refusal, got {result:?}"
        )),
    }
}
