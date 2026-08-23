use super::{
    AccessPlanDiagnostic, AtomicCapability, BaseCongruence, EffectiveFieldSupply,
    EffectiveSupplyKind, ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess,
    PlacementResourceCompatibility, ResourceRegion, ValidatedPlacementPlan,
    ValidatedResourceProfile,
};

/// Join one normalized placement demand with one normalized provider profile.
///
/// This is the pure resource-compatibility judgment. Concrete loan admission
/// separately discharges its returned base congruence and retains the selected
/// field rows as evidence for later specialization replay.
pub fn validate_placement_resources(
    plan: &ValidatedPlacementPlan,
    profile: &ValidatedResourceProfile,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
    if plan.access.layout_size_bytes > profile.length {
        return Err(AccessPlanDiagnostic(format!(
            "{}-byte placed layout exceeds {}-byte resource profile",
            plan.access.layout_size_bytes, profile.length
        )));
    }
    let mut congruence = CongruenceAccumulator {
        value: BaseCongruence {
            modulus: 1,
            residue: 0,
        },
        source: "unconstrained base".into(),
    };
    require_base_congruence(&mut congruence, "layout base", 0, plan.layout.align)?;

    let mut fields = Vec::with_capacity(plan.access.fields.len());
    for descriptor in &plan.access.fields {
        let width_bytes = u64::from(descriptor.transfer_width_bits / 8);
        let end = descriptor
            .container_byte_offset
            .checked_add(width_bytes)
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` resource interval overflows",
                    descriptor.field
                ))
            })?;
        let region = profile
            .regions
            .iter()
            .find(|region| {
                descriptor.container_byte_offset >= region.offset
                    && end <= region.offset + region.length
            })
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` transfer at {}..{end} is not covered by one resource region",
                    descriptor.field, descriptor.container_byte_offset
                ))
            })?;
        if !region.reach.contains_all(&plan.reach) {
            return Err(AccessPlanDiagnostic(format!(
                "resource region covering field `{}` does not supply the placement's complete boundary reach",
                descriptor.field
            )));
        }
        let access = plan
            .access
            .field(descriptor.key)
            .expect("validated descriptor must retain its source access decision")
            .access();
        let (kind, alignment_bytes) = select_effective_supply(&descriptor.field, access, region)?;
        require_base_congruence(
            &mut congruence,
            descriptor.field.as_str(),
            descriptor.container_byte_offset,
            alignment_bytes,
        )?;
        fields.push(EffectiveFieldSupply {
            key: descriptor.key,
            field: descriptor.field.clone(),
            offset: descriptor.container_byte_offset,
            width_bits: descriptor.transfer_width_bits,
            alignment_bytes,
            kind,
        });
    }
    Ok(PlacementResourceCompatibility {
        placement: plan.identity,
        profile: profile.identity,
        base: congruence.value,
        fields,
    })
}

fn select_effective_supply(
    field: &str,
    access: &FieldAccess,
    region: &ResourceRegion,
) -> Result<(EffectiveSupplyKind, u64), AccessPlanDiagnostic> {
    match access {
        FieldAccess::Inaccessible => {
            unreachable!("inaccessible fields do not have validated descriptors")
        }
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            if !region.stable.permits(*read, *write) {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{field}` requests Stable read={read} write={write}, but its resource region does not supply them"
                )));
            }
            Ok((
                EffectiveSupplyKind::Stable,
                stable_transfer_alignment(*transfer_width_bits),
            ))
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            if let ExternalCapability::Access {
                read: supplied_read,
                write: supplied_write,
                transfers,
            } = &region.external
                && external_read_compatible(*read, *supplied_read)
                && (!*write || *supplied_write)
                && let Some(rule) = transfers
                    .iter()
                    .find(|rule| rule.width_bits == *transfer_width_bits)
            {
                return Ok((EffectiveSupplyKind::External, rule.alignment_bytes));
            }
            let stable_read = *read == ExternalRead::Read;
            if *read != ExternalRead::Take && region.stable.permits(stable_read, *write) {
                return Ok((
                    EffectiveSupplyKind::Stable,
                    stable_transfer_alignment(*transfer_width_bits),
                ));
            }
            Err(AccessPlanDiagnostic(format!(
                "field `{field}` requests incompatible External {transfer_width_bits}-bit read={read:?} write={write}"
            )))
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            ..
        } => {
            let AtomicCapability::Access { transfers } = &region.atomic else {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{field}` requests Atomic access, but its resource region supplies none"
                )));
            };
            let rule = transfers
                .iter()
                .find(|rule| {
                    rule.transfer.width_bits == *transfer_width_bits
                        && rule.operations.contains(*operations)
                })
                .ok_or_else(|| {
                    AccessPlanDiagnostic(format!(
                        "field `{field}` requests unsupported Atomic {transfer_width_bits}-bit operation families"
                    ))
                })?;
            Ok((EffectiveSupplyKind::Atomic, rule.transfer.alignment_bytes))
        }
    }
}

const fn external_read_compatible(demand: ExternalRead, supply: ExternalReadBehavior) -> bool {
    match demand {
        ExternalRead::None => true,
        ExternalRead::Read => matches!(supply, ExternalReadBehavior::Repeatable),
        ExternalRead::Take => matches!(supply, ExternalReadBehavior::Destructive),
    }
}

const fn stable_transfer_alignment(width_bits: u16) -> u64 {
    let width_bytes = width_bits / 8;
    if width_bytes.is_power_of_two() {
        width_bytes as u64
    } else {
        1
    }
}

struct CongruenceAccumulator {
    value: BaseCongruence,
    source: String,
}

fn require_base_congruence(
    accumulated: &mut CongruenceAccumulator,
    source: &str,
    offset: u64,
    alignment: u64,
) -> Result<(), AccessPlanDiagnostic> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(AccessPlanDiagnostic(format!(
            "field `{source}` requires invalid transfer alignment {alignment}"
        )));
    }
    let required = BaseCongruence {
        modulus: alignment,
        residue: (alignment - offset % alignment) % alignment,
    };
    let shared_modulus = accumulated.value.modulus.min(required.modulus);
    if accumulated.value.residue % shared_modulus != required.residue % shared_modulus {
        return Err(AccessPlanDiagnostic(format!(
            "field `{source}` at offset {offset} with {alignment}-byte transfer alignment conflicts with {} (base mod {} = {}, required base mod {alignment} = {})",
            accumulated.source,
            accumulated.value.modulus,
            accumulated.value.residue,
            required.residue
        )));
    }
    if required.modulus > accumulated.value.modulus {
        accumulated.value = required;
        accumulated.source =
            format!("field `{source}` at offset {offset} with {alignment}-byte transfer alignment");
    }
    Ok(())
}
