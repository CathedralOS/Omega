/*
We compute native target layout over source-free records: pointer and fat-descriptor
carrier geometry (`TypeLayout`, `pointer_layout`, `fat_descriptor_layout`),
deployment-aware layout target contexts (`LayoutTarget`), nominal authored-layout
identities (`TerminalLayoutIdentity`), selected Terminal record storage
(`TerminalRecordStorage`), boundary-value signature shapes (`boundary_values`),
and inline callback storage paths (`terminal_callback_layouts`).
*/

/*
Layout fixes where a value's bytes live without granting access or boundary-crossing
authority, and without reading source trees. Source-level `size_of` and `align_of`
queries walk typed declarations during provider planning (`data_layout_queries`);
by the time values reach backend layout, we operate solely on target-neutral value
recipes and canonical Terminal records paired with a concrete `NativeTarget` or
`TargetProfile` (needed when raw native geometry alone cannot distinguish
profiles such as Windows and UEFI).
*/

/*
`boundary_values` constructs bounded native signature graphs and opaque carrier
occurrences from Psi's closed value recipes and Terminal structural types.
`terminal_callback_layouts` closes inline callback storage paths and slot overlap
checks from canonical Terminal records, consuming the nominal layout identities
and record storage resolvers defined here.
*/

mod boundary_values;
mod terminal_callback_layouts;

pub use boundary_values::{
    BOUNDARY_VALUE_FIELD_CAPACITY, BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryOpaqueOccurrence,
    BoundaryValueLayout, TerminalBoundaryRecipes, append_boundary_value_shape,
};
pub use terminal_callback_layouts::{
    CallbackLayoutCoordinates, CallbackStorageRoot, MaterializedCallbackLayout,
    TerminalCallbackLayouts, callback_storage_has_slots,
};

use calling_conventions::{LayoutPlanId, callback_layout_plan_id, callback_plan_laid_layout_id};
use semantic_vocabulary::StructuralTypeId;
use target::{NativeTarget, TargetProfile};
use terminal_psi::{
    LayoutApplicationId, NativeLayoutRealization, TargetLayoutRealization, TerminalModule,
};
use terminal_semantics::{RecordStorageResolver, StorageGeometryError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: usize,
    pub alignment: usize,
}

pub(crate) const fn type_layout(size: usize, alignment: usize) -> TypeLayout {
    TypeLayout { size, alignment }
}

impl Default for TypeLayout {
    fn default() -> Self {
        Self {
            size: 0,
            alignment: 1,
        }
    }
}

pub fn fat_descriptor_layout(target: NativeTarget) -> TypeLayout {
    let descriptor = runtime_abi::fat_descriptor_layout(target);
    type_layout(descriptor.byte_size, descriptor.alignment)
}

pub fn pointer_layout(target: NativeTarget) -> TypeLayout {
    type_layout(target.pointer_size, target.pointer_alignment)
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutTarget {
    native: NativeTarget,
    profile: Option<TargetProfile>,
}

impl LayoutTarget {
    pub fn for_profile(profile: TargetProfile) -> Result<Self, String> {
        profile.layout_target_facts().ok_or_else(|| {
            format!(
                "target `{}` has no admitted physical layout facts",
                profile.target_name()
            )
        })?;
        let native = profile.native_realization().ok_or_else(|| {
            format!(
                "target `{}` has no native realization",
                profile.target_name()
            )
        })?;
        Ok(Self {
            native,
            profile: Some(profile),
        })
    }

    pub fn native_target(self) -> NativeTarget {
        self.native
    }

    pub fn into_parts(self) -> (NativeTarget, Option<TargetProfile>) {
        (self.native, self.profile)
    }
}

impl From<NativeTarget> for LayoutTarget {
    fn from(native: NativeTarget) -> Self {
        Self {
            native,
            profile: None,
        }
    }
}

pub struct TerminalLayoutIdentity<'module> {
    pub data_identity: &'module str,
    pub policy_identity: &'module str,
    pub semantic_identity: &'module str,
    pub data_layout: LayoutPlanId,
    pub slot_layout: LayoutPlanId,
    pub realization: &'module NativeLayoutRealization,
}

pub fn terminal_layout_identity(
    module: &TerminalModule,
    record: StructuralTypeId,
    target: TargetProfile,
) -> Result<Option<TerminalLayoutIdentity<'_>>, String> {
    let native = module
        .native_layouts
        .record(record)
        .ok_or("native layout lost its canonical record association")?;
    let Some(realization) = module
        .native_layouts
        .for_target(native, target.target_name())?
    else {
        return Ok(None);
    };
    let data_identity = module
        .identities
        .text(native.data_identity)
        .filter(|identity| !identity.is_empty())
        .ok_or("native layout has no admitted nominal data identity")?;
    let policy_identity = module
        .identities
        .text(native.policy_identity)
        .filter(|identity| !identity.is_empty())
        .ok_or("native layout has no exact selected policy identity")?;
    let geometry = target.native_target();
    let semantic_identity = module
        .identities
        .text(native.semantic_identity)
        .ok_or("native layout lost its exact semantic argument identity")?;
    let target_facts = target
        .layout_target_facts()
        .ok_or("native layout requires a concrete target geometry")?;
    if let Some(facts) = &realization.target
        && (facts.target != target.target_name()
            || facts.pointer_size != geometry.pointer_size as u64
            || facts.pointer_alignment != geometry.pointer_alignment as u64
            || facts.little_endian != target_facts.little_endian)
    {
        return Err("native layout changed its selected target geometry".into());
    }
    let report =
        layout_plans::normalized_native_layout_plan_report_fingerprint(&realization.report);
    Ok(Some(TerminalLayoutIdentity {
        data_identity,
        policy_identity,
        semantic_identity,
        data_layout: callback_plan_laid_layout_id(
            report,
            data_identity,
            policy_identity,
            geometry.pointer_size,
            geometry.pointer_alignment,
        ),
        slot_layout: callback_layout_plan_id(
            report,
            geometry.pointer_size,
            geometry.pointer_alignment,
        ),
        realization,
    }))
}

pub struct TerminalRecordStorage<'module> {
    pub module: &'module TerminalModule,
    pub target: TargetProfile,
}

impl RecordStorageResolver for TerminalRecordStorage<'_> {
    fn realization(
        &self,
        record: StructuralTypeId,
        application: LayoutApplicationId,
    ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
        let application = self
            .module
            .target_layout_applications
            .iter()
            .find(|row| row.id == application && row.record == record)
            .ok_or(StorageGeometryError::UnresolvedLayout(record))?;
        let selected = application
            .realizations
            .iter()
            .find(|row| row.target == self.target.target_name())
            .ok_or(StorageGeometryError::UnresolvedLayout(record))?;
        let facts = self
            .target
            .layout_target_facts()
            .ok_or(StorageGeometryError::MissingCarrierLayout)?;
        if selected.pointer_size != facts.pointer_size
            || selected.pointer_alignment != facts.pointer_alignment
            || selected.little_endian != facts.little_endian
        {
            return Err(StorageGeometryError::InvalidRecordStorage(record));
        }
        Ok(selected)
    }
}
