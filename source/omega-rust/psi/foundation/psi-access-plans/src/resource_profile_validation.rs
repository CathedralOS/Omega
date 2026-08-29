use super::{
    AccessPlanDiagnostic, AtomicCapability, ExternalCapability, ExternalReadBehavior,
    ResourceProfile, ResourceRegion, TransferRule, ValidatedResourceProfile,
    non_authoritative_resource_profile_compatibility_fingerprint,
};

/// Normalize one provider resource profile into exact disjoint supply rows.
pub fn validate_resource_profile(
    mut profile: ResourceProfile,
    length: u64,
) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
    if length == 0 {
        return Err(AccessPlanDiagnostic(
            "resource profile must describe a nonempty range".into(),
        ));
    }
    profile
        .regions
        .sort_by_key(|region| (region.offset, region.length));
    let mut normalized: Vec<ResourceRegion> = Vec::with_capacity(profile.regions.len());
    for mut region in profile.regions {
        if region.length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile region cannot be empty".into(),
            ));
        }
        let end = region.offset.checked_add(region.length).ok_or_else(|| {
            AccessPlanDiagnostic("resource-profile region range overflows".into())
        })?;
        if end > length {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile region {}..{end} exceeds {length}-byte profile",
                region.offset
            )));
        }
        normalize_external_capability(&mut region.external)?;
        normalize_atomic_capability(&mut region.atomic)?;
        if !region.stable.any() && !region.external.any() && !region.atomic.any() {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile region {}..{end} supplies no operation",
                region.offset
            )));
        }
        if let Some(previous) = normalized.last_mut() {
            let previous_end = previous.offset + previous.length;
            if region.offset < previous_end {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile regions {}..{} and {}..{end} overlap",
                    previous.offset, previous_end, region.offset
                )));
            }
            if region.offset == previous_end
                && previous.stable == region.stable
                && previous.external == region.external
                && previous.atomic == region.atomic
                && previous.reach == region.reach
            {
                previous.length = previous.length.checked_add(region.length).ok_or_else(|| {
                    AccessPlanDiagnostic("merged resource-profile region length overflows".into())
                })?;
                continue;
            }
        }
        normalized.push(region);
    }
    let identity =
        non_authoritative_resource_profile_compatibility_fingerprint(length, &normalized);
    Ok(ValidatedResourceProfile {
        identity,
        length,
        regions: normalized,
    })
}

fn normalize_external_capability(
    capability: &mut ExternalCapability,
) -> Result<(), AccessPlanDiagnostic> {
    let ExternalCapability::Access {
        read,
        write,
        transfers,
    } = capability
    else {
        return Ok(());
    };
    if *read == ExternalReadBehavior::None && !*write {
        return Err(AccessPlanDiagnostic(
            "external capability supplies no operation; use None".into(),
        ));
    }
    normalize_transfer_rules(transfers)?;
    if transfers.is_empty() {
        return Err(AccessPlanDiagnostic(
            "external capability must list at least one transfer rule".into(),
        ));
    }
    Ok(())
}

fn normalize_atomic_capability(
    capability: &mut AtomicCapability,
) -> Result<(), AccessPlanDiagnostic> {
    let AtomicCapability::Access { transfers } = capability else {
        return Ok(());
    };
    transfers.sort_by_key(|rule| rule.transfer.width_bits);
    let mut prior_width = None;
    for rule in transfers.iter() {
        validate_transfer_rule(rule.transfer)?;
        if !rule.operations.any() {
            return Err(AccessPlanDiagnostic(format!(
                "atomic {}-bit transfer supplies no operation",
                rule.transfer.width_bits
            )));
        }
        if prior_width.replace(rule.transfer.width_bits) == Some(rule.transfer.width_bits) {
            return Err(AccessPlanDiagnostic(format!(
                "atomic capability repeats {}-bit transfer width",
                rule.transfer.width_bits
            )));
        }
    }
    if transfers.is_empty() {
        return Err(AccessPlanDiagnostic(
            "atomic capability must list at least one transfer rule".into(),
        ));
    }
    Ok(())
}

fn normalize_transfer_rules(transfers: &mut [TransferRule]) -> Result<(), AccessPlanDiagnostic> {
    transfers.sort_by_key(|rule| rule.width_bits);
    let mut prior_width = None;
    for rule in transfers.iter().copied() {
        validate_transfer_rule(rule)?;
        if prior_width.replace(rule.width_bits) == Some(rule.width_bits) {
            return Err(AccessPlanDiagnostic(format!(
                "external capability repeats {}-bit transfer width",
                rule.width_bits
            )));
        }
    }
    Ok(())
}

fn validate_transfer_rule(rule: TransferRule) -> Result<(), AccessPlanDiagnostic> {
    if rule.width_bits == 0 || rule.width_bits > 128 || !rule.width_bits.is_multiple_of(8) {
        return Err(AccessPlanDiagnostic(format!(
            "resource transfer width {} is not a supported whole-byte width in 8..=128",
            rule.width_bits
        )));
    }
    if rule.alignment_bytes == 0 || !rule.alignment_bytes.is_power_of_two() {
        return Err(AccessPlanDiagnostic(format!(
            "resource transfer alignment {} is not a positive power of two",
            rule.alignment_bytes
        )));
    }
    Ok(())
}
