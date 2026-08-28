use super::{
    AccessExposure, AccessPlan, AccessPlanId, AtomicCapability, AtomicPermissions, BoundaryReach,
    ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess, PlacementPlanId,
    ResourceProfileId, ResourceRegion, StableCapability, TransferRule,
};

pub(super) fn normalized_placement_plan_identity(
    access: AccessPlanId,
    reach: &BoundaryReach,
) -> PlacementPlanId {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.placement-plan.v1");
    hash_u64(&mut hash, access.normalized_identity());
    hash_u64(&mut hash, reach.services().len() as u64);
    for service in reach.services() {
        hash_u64(&mut hash, service.normalized_identity());
    }
    PlacementPlanId(if hash == 0 { 1 } else { hash })
}

pub(super) fn normalized_resource_profile_identity(
    length: u64,
    regions: &[ResourceRegion],
) -> ResourceProfileId {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.resource-profile.v1");
    hash_u64(&mut hash, length);
    hash_u64(&mut hash, regions.len() as u64);
    for region in regions {
        hash_u64(&mut hash, region.offset);
        hash_u64(&mut hash, region.length);
        hash_byte(
            &mut hash,
            match region.stable {
                StableCapability::None => 0,
                StableCapability::Read => 1,
                StableCapability::Write => 2,
                StableCapability::ReadWrite => 3,
            },
        );
        match &region.external {
            ExternalCapability::None => hash_byte(&mut hash, 0),
            ExternalCapability::Access {
                read,
                write,
                transfers,
            } => {
                hash_byte(&mut hash, 1);
                hash_byte(
                    &mut hash,
                    match read {
                        ExternalReadBehavior::None => 0,
                        ExternalReadBehavior::Repeatable => 1,
                        ExternalReadBehavior::Destructive => 2,
                    },
                );
                hash_byte(&mut hash, u8::from(*write));
                hash_transfer_rules(&mut hash, transfers);
            }
        }
        match &region.atomic {
            AtomicCapability::None => hash_byte(&mut hash, 0),
            AtomicCapability::Access { transfers } => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, transfers.len() as u64);
                for rule in transfers {
                    hash_transfer_rule(&mut hash, rule.transfer);
                    hash_atomic_permissions(&mut hash, rule.operations);
                }
            }
        }
        hash_u64(&mut hash, region.reach.services().len() as u64);
        for service in region.reach.services() {
            hash_u64(&mut hash, service.normalized_identity());
        }
    }
    ResourceProfileId(if hash == 0 { 1 } else { hash })
}

fn hash_transfer_rules(hash: &mut u64, rules: &[TransferRule]) {
    hash_u64(hash, rules.len() as u64);
    for rule in rules {
        hash_transfer_rule(hash, *rule);
    }
}

fn hash_transfer_rule(hash: &mut u64, rule: TransferRule) {
    hash_u64(hash, u64::from(rule.width_bits));
    hash_u64(hash, rule.alignment_bytes);
}

fn hash_atomic_permissions(hash: &mut u64, permissions: AtomicPermissions) {
    for enabled in [
        permissions.load,
        permissions.store,
        permissions.fetch_add,
        permissions.fetch_sub,
        permissions.fetch_xor,
        permissions.fetch_or,
        permissions.fetch_and,
        permissions.swap,
        permissions.compare_exchange,
        permissions.compare_exchange_once,
        permissions.try_exchange,
        permissions.try_exchange_once,
    ] {
        hash_byte(hash, u8::from(enabled));
    }
}

pub(super) fn normalized_access_plan_identity(
    plan: &AccessPlan,
    layout_fingerprint: u64,
) -> AccessPlanId {
    // FNV-1a is used as a compact deterministic artifact identity here, never
    // as authorization or collision-resistant evidence. The versioned prefix
    // makes any future vocabulary change an explicit identity migration.
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.access-plan.v5");
    hash_u64(&mut hash, layout_fingerprint);
    hash_u64(&mut hash, plan.entries.len() as u64);
    for entry in &plan.entries {
        hash_u64(&mut hash, u64::from(entry.key.slot));
        match &entry.access {
            FieldAccess::Inaccessible => hash_byte(&mut hash, 0),
            FieldAccess::Stable {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_byte(&mut hash, u8::from(*read));
                hash_byte(&mut hash, u8::from(*write));
                hash_exposure(&mut hash, *exposure);
            }
            FieldAccess::External {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                hash_byte(&mut hash, 2);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_byte(
                    &mut hash,
                    match read {
                        ExternalRead::None => 0,
                        ExternalRead::Read => 1,
                        ExternalRead::Take => 2,
                    },
                );
                hash_byte(&mut hash, u8::from(*write));
                hash_exposure(&mut hash, *exposure);
            }
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                exposure,
            } => {
                hash_byte(&mut hash, 3);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_atomic_permissions(&mut hash, *operations);
                hash_exposure(&mut hash, *exposure);
            }
        }
    }
    // Zero is reserved as the inert/no-plan identity throughout the semantic
    // spine. A hash hitting it remains deterministic but is remapped out of
    // the reserved value.
    AccessPlanId(if hash == 0 { 1 } else { hash })
}

fn hash_exposure(hash: &mut u64, exposure: AccessExposure) {
    hash_byte(
        hash,
        match exposure {
            AccessExposure::Exported => 0,
            AccessExposure::BindingPrivate => 1,
        },
    );
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_byte(hash, *byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}
