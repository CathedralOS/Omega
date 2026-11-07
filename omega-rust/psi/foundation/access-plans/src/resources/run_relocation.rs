//! A dense move must preserve access to every cell, not just the first one.
//! This checks resource supply and geometry only. The caller still owns the
//! occupied run, proves element mobility and transfers its retained custody.

use crate::{
    AccessPlanDiagnostic, InitializationResourceCompatibility, ValidatedAccessPlan,
    ValidatedResourceProfile, require_stable_region_supply, validate_initialization_resources,
};

/// Validate the complete source read and destination write before moving bytes.
/// Empty runs need no cell interpretation; zero-footprint nonempty runs still
/// validate their cell's placement. The returned compatibility describes the
/// first destination cell, not authority to extract any value.
pub fn validate_run_relocation_resources(
    plan: &ValidatedAccessPlan,
    count: u64,
    destination_base: u64,
    source: &ValidatedResourceProfile,
    destination: &ValidatedResourceProfile,
) -> Result<Option<InitializationResourceCompatibility>, AccessPlanDiagnostic> {
    let footprint = plan.layout().size.ok_or_else(|| {
        AccessPlanDiagnostic("run relocation requires a fixed cell footprint".into())
    })?;
    let occupied = count.checked_mul(footprint).ok_or_else(|| {
        AccessPlanDiagnostic("run relocation occupied byte length overflows".into())
    })?;
    if source.length() != occupied || destination.length() != occupied {
        return Err(AccessPlanDiagnostic(
            "run relocation requires two exact occupied-length windows".into(),
        ));
    }
    destination_base.checked_add(occupied).ok_or_else(|| {
        AccessPlanDiagnostic("run relocation destination address range overflows".into())
    })?;
    require_stable_region_supply(
        source,
        0,
        occupied,
        true,
        false,
        plan.reach(),
        "run relocation source",
    )?;
    require_stable_region_supply(
        destination,
        0,
        occupied,
        false,
        true,
        plan.reach(),
        "run relocation destination",
    )?;
    if count == 0 {
        return Ok(None);
    }

    // Within one profile region, every full cell sees identical capabilities.
    // Inspect the cells touching each region boundary and two following cells:
    // two consecutive bases also establish the repeating alignment constraint.
    // Work scales with supply regions, never with the run's runtime count.
    let mut cells = vec![0];
    if count > 1 {
        cells.push(1);
        cells.push(count - 1);
    }
    if let Some(footprint) = std::num::NonZeroU64::new(footprint) {
        for region in destination.regions() {
            for boundary in [region.offset, region.offset + region.length] {
                let cell = boundary / footprint;
                if cell > 0 && cell - 1 < count {
                    cells.push(cell - 1);
                }
                if cell < count {
                    cells.push(cell);
                }
                for distance in [1, 2] {
                    if let Some(next) = cell.checked_add(distance).filter(|next| *next < count) {
                        cells.push(next);
                    }
                }
            }
        }
    } else {
        cells.truncate(1);
    }
    cells.sort_unstable();
    cells.dedup();
    let mut first = None;
    for cell in cells {
        // The complete product and address range were checked above.
        let offset = cell * footprint;
        let profile = destination.restrict(offset, footprint, plan.reach())?;
        let compatibility = validate_initialization_resources(plan, &profile)?;
        if !compatibility
            .placement()
            .base_congruence()
            .admits(destination_base + offset)
        {
            return Err(AccessPlanDiagnostic(format!(
                "run relocation destination cell {cell} violates placement alignment",
            )));
        }
        if cell == 0 {
            first = Some(compatibility);
        }
    }
    Ok(first)
}
