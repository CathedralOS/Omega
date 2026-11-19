use omega_resolver::DeclarationId;
use omega_typer::TypedConstant;

use crate::{MaterializationError, PreparedTrees, PreparedValue, SelectedLayout, TargetObservation, TargetSemantics};

/// Inputs for the consumer that realizes an admitted constant under a selected
/// layout. The initializer identifies its source and exact applied carrier.
#[derive(Debug)]
pub struct ConstantMaterialization<'a> {
    pub definition: &'a TypedConstant,
    pub value: &'a PreparedValue,
    pub layout: &'a SelectedLayout,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConstantMaterializationError<'a> {
    UnknownConstant,
    OpenConstant,
    ChangedTargetDependency(&'a TargetObservation),
    MissingLayout(&'a TypedConstant),
    IndeterminateValue(MaterializationError),
}

impl PreparedTrees<'_> {
    /// A lowering consumer must use a compatible target and the plan for the
    /// constant's exact carrier. The plan owner supplies the field placements.
    pub fn constant_for_materialization<'a>(
        &'a self,
        id: &DeclarationId,
        target: &'a TargetSemantics,
    ) -> Result<ConstantMaterialization<'a>, ConstantMaterializationError<'a>> {
        let definition = self
            .typed()
            .constants
            .iter()
            .find(|constant| &constant.declaration == id)
            .ok_or(ConstantMaterializationError::UnknownConstant)?;
        let value = self
            .constant(id)
            .ok_or(ConstantMaterializationError::OpenConstant)?;
        if let Some(changed) = self
            .target_observations()
            .iter()
            .find(|used| !target.matches_observation(used))
        {
            return Err(ConstantMaterializationError::ChangedTargetDependency(changed));
        }
        let layout = target
            .layout_for(&definition.initializer.carrier)
            .ok_or(ConstantMaterializationError::MissingLayout(definition))?;
        value
            .check_materializable()
            .map_err(ConstantMaterializationError::IndeterminateValue)?;
        Ok(ConstantMaterialization { definition, value, layout })
    }
}
