use super::{
    AccessExposure, AccessLayoutCommitment, AccessPlan, AccessPlanId, AtomicCapability,
    AtomicPermissions, BoundaryReach, ExternalCapability, ExternalRead, ExternalReadBehavior,
    FieldAccess, PlacementPlanId, ResourceProfileId, ResourceRegion, StableCapability,
    TransferRule,
};
use psi_extents::{ExtentContentInterpretation, ExtentContentInterpretationId};
use psi_layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
use sha2::{Digest, Sha256};

pub(super) fn non_authoritative_placement_compatibility_fingerprint(
    access: AccessPlanId,
    reach: &BoundaryReach,
) -> PlacementPlanId {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.placement-plan.v1");
    hash_u64(&mut hash, access.compatibility_fingerprint());
    hash_u64(&mut hash, reach.services().len() as u64);
    for service in reach.services() {
        hash_u64(&mut hash, service.normalized_identity());
    }
    PlacementPlanId(if hash == 0 { 1 } else { hash })
}

/// Collision-resistant semantic owner of compiler-issued access field keys.
/// The compact layout report fingerprint remains useful for diagnostics and
/// caches, but cannot rejoin a key to a different exact layout.
pub(super) fn authoritative_access_layout_commitment(
    layout: &LayoutPlanReport,
) -> AccessLayoutCommitment {
    let mut digest = Sha256::new();
    digest.update(b"omega.access-field-layout.authoritative.v1\0");
    hash_canonical_layout(&mut digest, layout);
    AccessLayoutCommitment(digest.finalize().into())
}

/// Collision-resistant identity for the complete canonical placement policy.
///
/// Unlike the compact FNV compatibility fingerprint above, this value may be
/// used to rejoin provider content evidence to its exact interpretation.
pub(super) fn authoritative_placement_interpretation(
    plan: &super::ValidatedPlacementPlan,
) -> ExtentContentInterpretation {
    let mut digest = Sha256::new();
    digest.update(b"omega.placement-plan.authoritative.v1\0");
    hash_canonical_layout(&mut digest, &plan.layout);
    hash_u64_sha(&mut digest, plan.access.plan.entries.len() as u64);
    for entry in &plan.access.plan.entries {
        hash_u64_sha(&mut digest, u64::from(entry.key.slot));
        hash_field_access_sha(&mut digest, &entry.access);
    }
    hash_u64_sha(&mut digest, plan.reach.services().len() as u64);
    for service in plan.reach.services() {
        hash_u64_sha(&mut digest, service.normalized_identity());
    }
    let commitment: [u8; 32] = digest.finalize().into();
    let compatibility_fingerprint = ExtentContentInterpretationId::from_normalized_identity(
        plan.identity.compatibility_fingerprint(),
    )
    .expect("validated placement compatibility fingerprints are nonzero");
    ExtentContentInterpretation::from_sha256_commitment(compatibility_fingerprint, commitment)
}

fn hash_canonical_layout(digest: &mut Sha256, layout: &psi_layout_plans::LayoutPlanReport) {
    hash_u64_sha(digest, layout.schema_identity);
    match layout.size {
        Some(size) => {
            digest.update([1]);
            hash_u64_sha(digest, size);
        }
        None => digest.update([0]),
    }
    hash_u64_sha(digest, layout.align);
    let mut entries = layout
        .entries
        .iter()
        .map(canonical_layout_entry_bytes)
        .collect::<Vec<_>>();
    entries.sort_unstable();
    hash_u64_sha(digest, entries.len() as u64);
    for entry in entries {
        hash_u64_sha(digest, entry.len() as u64);
        digest.update(entry);
    }
}

fn canonical_layout_entry_bytes(entry: &LayoutFieldEntryReport) -> Vec<u8> {
    let mut bytes = Vec::new();
    match entry.member_identity {
        Some(identity) => {
            bytes.push(1);
            bytes.extend(identity.to_le_bytes());
        }
        None => {
            bytes.push(0);
            bytes.extend((entry.field.len() as u64).to_le_bytes());
            bytes.extend(entry.field.as_bytes());
        }
    }
    match entry.placement {
        LayoutPlacementReport::At { offset } => {
            bytes.push(0);
            bytes.extend(offset.to_le_bytes());
        }
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            interpretation,
        } => {
            bytes.push(1);
            bytes.extend(offset.to_le_bytes());
            bytes.extend(stored_width.to_le_bytes());
            bytes.push(match interpretation {
                IntegerInterpretation::Signed => 0,
                IntegerInterpretation::Unsigned => 1,
            });
        }
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => {
            bytes.push(2);
            for value in [
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            ] {
                bytes.extend(value.to_le_bytes());
            }
        }
    }
    bytes
}

fn hash_field_access_sha(digest: &mut Sha256, access: &FieldAccess) {
    match access {
        FieldAccess::Inaccessible => digest.update([0]),
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            digest.update([1]);
            hash_u64_sha(digest, u64::from(*transfer_width_bits));
            digest.update([u8::from(*read), u8::from(*write)]);
            hash_exposure_sha(digest, *exposure);
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            digest.update([2]);
            hash_u64_sha(digest, u64::from(*transfer_width_bits));
            digest.update([
                match read {
                    ExternalRead::None => 0,
                    ExternalRead::Read => 1,
                    ExternalRead::Take => 2,
                },
                u8::from(*write),
            ]);
            hash_exposure_sha(digest, *exposure);
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            exposure,
        } => {
            digest.update([3]);
            hash_u64_sha(digest, u64::from(*transfer_width_bits));
            for enabled in [
                operations.load,
                operations.store,
                operations.fetch_add,
                operations.fetch_sub,
                operations.fetch_xor,
                operations.fetch_or,
                operations.fetch_and,
                operations.swap,
                operations.compare_exchange,
                operations.compare_exchange_once,
                operations.try_exchange,
                operations.try_exchange_once,
            ] {
                digest.update([u8::from(enabled)]);
            }
            hash_exposure_sha(digest, *exposure);
        }
    }
}

fn hash_exposure_sha(digest: &mut Sha256, exposure: AccessExposure) {
    digest.update([match exposure {
        AccessExposure::Exported => 0,
        AccessExposure::BindingPrivate => 1,
    }]);
}

fn hash_u64_sha(digest: &mut Sha256, value: u64) {
    digest.update(value.to_le_bytes());
}

pub(super) fn non_authoritative_resource_profile_compatibility_fingerprint(
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

pub(super) fn non_authoritative_access_plan_compatibility_fingerprint(
    plan: &AccessPlan,
    layout_report_fingerprint: u64,
) -> AccessPlanId {
    // FNV-1a is used as a compact deterministic artifact identity here, never
    // as authorization or collision-resistant evidence. The versioned prefix
    // makes any future vocabulary change an explicit identity migration.
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.access-plan.v5");
    hash_u64(&mut hash, layout_report_fingerprint);
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
