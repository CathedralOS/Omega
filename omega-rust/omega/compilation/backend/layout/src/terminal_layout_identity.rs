//! Close nominal authored-layout coordinates directly from canonical data.
//! This computes identity, not callback placement or access authority. A full
//! callback path must additionally close storage, overlap and binder obligations.

use calling_conventions::{LayoutPlanId, callback_layout_plan_id, callback_plan_laid_layout_id};
use semantic_vocabulary::StructuralTypeId;
use target::TargetProfile;
use terminal_psi::{NativeLayoutRealization, TerminalModule};

pub struct TerminalLayoutIdentity<'module> {
    pub data_identity: &'module str,
    pub policy_identity: &'module str,
    pub semantic_identity: &'module str,
    /// Complete data application, distinct from the slot-owning plan itself.
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
