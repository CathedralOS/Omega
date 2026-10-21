use crate::{AccessFieldKey, PlacementPlanId, ResourceProfileId};
use crate::{
    AccessPlanDiagnostic, AtomicCapability, ExternalCapability, ExternalRead, ExternalReadBehavior,
    FieldAccess, ResourceRegion, StableFieldRegion, ValidatedPlacementPlan,
    ValidatedResourceProfile,
};

/// Normalized power-of-two constraint on the concrete loan base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaseCongruence {
    pub(crate) modulus: u64,
    pub(crate) residue: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectiveSupplyKind {
    Stable,
    External,
    Atomic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveFieldSupply {
    pub(crate) key: AccessFieldKey,
    pub(crate) field: String,
    pub(crate) offset: u64,
    pub(crate) width_bits: u16,
    pub(crate) alignment_bytes: u64,
    pub(crate) kind: EffectiveSupplyKind,
}

/// Sealed result of joining one normalized placement demand with one
/// normalized provider profile before a concrete loan is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementResourceCompatibility {
    pub(crate) placement: PlacementPlanId,
    profile: ResourceProfileId,
    pub(crate) base: BaseCongruence,
    pub(crate) fields: Vec<EffectiveFieldSupply>,
    pub(crate) stable_regions: Vec<StableFieldRegion>,
}

/// Supply compatibility for writing a complete initial image and then using
/// its declared placement. This describes demand, not vacancy, a live loan,
/// owned input custody, or a completed write. Those belong to establishment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializationResourceCompatibility {
    placement: PlacementResourceCompatibility,
    byte_length: u64,
}

impl InitializationResourceCompatibility {
    pub const fn placement(&self) -> &PlacementResourceCompatibility {
        &self.placement
    }

    /// Complete encoded image, including padding initialized by the writer.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

/// Join initialization's complete-image write with the subsequent view demand.
///
/// Accessor permissions alone cannot answer this question: an inaccessible or
/// read-only field still has to be initialized. The complete-image writer also
/// touches padding. Require uninterrupted Stable write supply over that image,
/// independently of the operations the resulting view exposes. Adjacent rows
/// may supply different additional capabilities; no single-region assumption
/// is justified for a sequence of ordinary Stable writes.
pub fn validate_initialization_resources(
    plan: &ValidatedPlacementPlan,
    profile: &ValidatedResourceProfile,
) -> Result<InitializationResourceCompatibility, AccessPlanDiagnostic> {
    let placement = validate_placement_resources(plan, profile)?;
    let byte_length = plan.access.layout_size_bytes;
    require_stable_region_supply(
        profile,
        0,
        byte_length,
        false,
        true,
        &plan.reach,
        "initialization image",
    )?;
    Ok(InitializationResourceCompatibility {
        placement,
        byte_length,
    })
}

/// Stable storage is not a single device transfer. Adjacent compatible supply
/// regions can cover one structural field, but a gap, missing operation or
/// insufficient reach rejects the complete interpretation before any access.
#[allow(clippy::too_many_arguments)]
pub fn require_stable_region_supply(
    profile: &ValidatedResourceProfile,
    byte_offset: u64,
    byte_length: u64,
    read: bool,
    write: bool,
    reach: &crate::BoundaryReach,
    description: &str,
) -> Result<(), AccessPlanDiagnostic> {
    let end = byte_offset.checked_add(byte_length).ok_or_else(|| {
        AccessPlanDiagnostic(format!("{description} resource interval overflows"))
    })?;
    let mut covered = byte_offset;
    for region in &profile.regions {
        if covered == end {
            break;
        }
        let region_end = region.offset + region.length;
        if region_end <= covered {
            continue;
        }
        if region.offset > covered {
            return Err(AccessPlanDiagnostic(format!(
                "{description} has no resource supply at byte {covered}"
            )));
        }
        if !region.stable.permits(read, write) {
            let operations = match (read, write) {
                (true, true) => "read/write",
                (true, false) => "read",
                (false, true) => "write",
                (false, false) => "none",
            };
            return Err(AccessPlanDiagnostic(format!(
                "{description} requires Stable {operations} supply at byte {covered}"
            )));
        }
        if !region.reach.contains_all(reach) {
            return Err(AccessPlanDiagnostic(format!(
                "{description} lacks the placement's complete boundary reach at byte {covered}"
            )));
        }
        // Normalized regions have non-overflowing ends bounded by the profile.
        covered = region_end.min(end);
    }
    if covered != end {
        return Err(AccessPlanDiagnostic(format!(
            "{description} has no resource supply at byte {covered}"
        )));
    }
    Ok(())
}

impl BaseCongruence {
    pub const fn modulus(self) -> u64 {
        self.modulus
    }

    pub const fn residue(self) -> u64 {
        self.residue
    }

    pub const fn admits(self, base: u64) -> bool {
        base % self.modulus == self.residue
    }
}

impl EffectiveFieldSupply {
    pub const fn key(&self) -> AccessFieldKey {
        self.key
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn width_bits(&self) -> u16 {
        self.width_bits
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub const fn kind(&self) -> EffectiveSupplyKind {
        self.kind
    }
}

impl PlacementResourceCompatibility {
    pub const fn placement(&self) -> PlacementPlanId {
        self.placement
    }

    pub const fn profile(&self) -> ResourceProfileId {
        self.profile
    }

    pub const fn base_congruence(&self) -> BaseCongruence {
        self.base
    }

    pub fn fields(&self) -> &[EffectiveFieldSupply] {
        &self.fields
    }

    pub fn stable_regions(&self) -> &[StableFieldRegion] {
        &self.stable_regions
    }

    pub(crate) fn field(&self, key: AccessFieldKey) -> Option<&EffectiveFieldSupply> {
        self.fields.iter().find(|field| field.key == key)
    }
}

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
    for region in &plan.access.stable_regions {
        require_stable_region_supply(
            profile,
            region.byte_offset,
            region.byte_length,
            region.read,
            region.write,
            &plan.reach,
            &format!("stable structural field `{}`", region.field),
        )?;
        require_base_congruence(
            &mut congruence,
            &region.field,
            region.byte_offset,
            region.byte_alignment,
        )?;
    }
    Ok(PlacementResourceCompatibility {
        placement: plan.identity,
        profile: profile.identity,
        base: congruence.value,
        fields,
        stable_regions: plan.access.stable_regions.clone(),
    })
}

fn select_effective_supply(
    field: &str,
    access: &FieldAccess,
    region: &ResourceRegion,
) -> Result<(EffectiveSupplyKind, u64), AccessPlanDiagnostic> {
    match access {
        FieldAccess::StableRegion { .. } => Err(AccessPlanDiagnostic(
            "structural Stable supply cannot be selected as one primitive transfer".into(),
        )),
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
