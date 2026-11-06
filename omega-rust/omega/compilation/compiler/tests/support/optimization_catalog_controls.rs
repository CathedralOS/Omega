//! Replay source-produced type catalogs through the public optimizer entrance.
use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::{
    OptimizationUnitValidationError,
    reconstruct_ownership_frontier_facts_with_admitted_cycle_machines,
    validate_psi_optimization_unit_with_admitted_cycle_machines,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::StructuralTypeShape;
use terminal_psi_to_abstract_operations::{
    ArtifactSections, build_verified_psi_optimization_unit, lower_artifact,
};

pub(super) fn intact_unit(
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
    let (_, unit) = build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(|error| format!("optimization control unit: {error:?}"))?
    .into_parts();
    // These acyclic fixtures need no external cycle authority. The intact
    // unit must pass before a malformed catalog is allowed to count.
    let frontiers = reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .map_err(|error| format!("optimization control intact ownership: {error:?}"))?;
    let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers)
        .map_err(|error| format!("optimization control ownership attachment: {error:?}"))?;
    validate_psi_optimization_unit_with_admitted_cycle_machines(&unit, &[])
        .map_err(|error| format!("optimization control intact replay: {error:?}"))?;
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
    match validate_psi_optimization_unit_with_admitted_cycle_machines(&invalid, &[]) {
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
    let unit = intact_unit(artifact)?;
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
    match validate_psi_optimization_unit_with_admitted_cycle_machines(&invalid, &[]) {
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
