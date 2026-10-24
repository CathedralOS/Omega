//! Identity encodings for the access policy vocabulary.
//!
//! Two identity regimes consume the same accepted policy: the authoritative
//! SHA-256 commitments (`authoritative_access_layout_commitment`,
//! `authoritative_placement_interpretation`) and the compact, explicitly
//! non-authoritative FNV-1a compatibility fingerprints (`AccessPlanId`,
//! `PlacementPlanId`, `ResourceProfileId`). Both feed one canonical byte
//! encoding of one accepted field policy, so a policy change is recognized
//! once and the two regimes cannot disagree about which plans carry the same
//! policy.

use crate::access_plan::AccessLayoutCommitment;
use crate::{
    AccessExposure, AccessPlan, AccessPlanId, AtomicCapability, AtomicPermissions, BoundaryReach,
    ExternalCapability, ExternalRead, ExternalReadBehavior, FieldAccess, PeerWritability,
    PlacementPlanId, ResourceProfileId, ResourceRegion, StableCapability, TransferRule,
};
use extents::{ExtentContentInterpretation, ExtentContentInterpretationId};
use layout_plans::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};

/// The canonical byte encoding of one accepted field policy.
///
/// The byte sequence is the identity protocol — a tag per variant followed by
/// that variant's exact facts — so changing it is an identity migration, not
/// a refactor. Both identity regimes feed these bytes whole rather than
/// restating the vocabulary per hasher.
struct AccessPolicyEncoder(Vec<u8>);

impl AccessPolicyEncoder {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn byte(&mut self, value: u8) {
        self.0.push(value);
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn exposure(&mut self, exposure: AccessExposure) {
        self.byte(match exposure {
            AccessExposure::Exported => 0,
            AccessExposure::BindingPrivate => 1,
        });
    }

    fn atomic_permissions(&mut self, permissions: AtomicPermissions) {
        for enabled in atomic_permission_bits(permissions) {
            self.byte(enabled);
        }
    }
}

/// The twelve atomic permission bits in canonical identity order — the one
/// roster every identity encoder reads, so a permission cannot be recognized
/// by one regime and ignored by the other.
fn atomic_permission_bits(permissions: AtomicPermissions) -> [u8; 12] {
    [
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
    ]
    .map(u8::from)
}

fn canonical_field_access_bytes(access: &FieldAccess) -> Vec<u8> {
    let mut encoder = AccessPolicyEncoder::new();
    match access {
        FieldAccess::Inaccessible => encoder.byte(0),
        FieldAccess::Region {
            byte_length,
            byte_alignment,
            transfer_width_bits,
            observation,
            permissions,
            exposure,
        } => {
            encoder.byte(5);
            encoder.u64(*byte_length);
            encoder.u64(*byte_alignment);
            encoder.u64(u64::from(*transfer_width_bits));
            encoder.byte(*observation as u8);
            encoder.byte(u8::from(permissions.read));
            encoder.byte(u8::from(permissions.take));
            encoder.byte(u8::from(permissions.write));
            encoder.exposure(*exposure);
        }
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            encoder.byte(1);
            encoder.u64(u64::from(*transfer_width_bits));
            encoder.byte(u8::from(*read));
            encoder.byte(u8::from(*write));
            encoder.exposure(*exposure);
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            encoder.byte(2);
            encoder.u64(u64::from(*transfer_width_bits));
            encoder.byte(match read {
                ExternalRead::None => 0,
                ExternalRead::Read => 1,
                ExternalRead::Take => 2,
            });
            encoder.byte(u8::from(*write));
            encoder.exposure(*exposure);
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            exposure,
        } => {
            encoder.byte(3);
            encoder.u64(u64::from(*transfer_width_bits));
            encoder.atomic_permissions(*operations);
            encoder.exposure(*exposure);
        }
    }
    encoder.0
}

pub(crate) fn non_authoritative_placement_compatibility_fingerprint(
    access: AccessPlanId,
    reach: &BoundaryReach,
) -> PlacementPlanId {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.placement-plan.v1");
    hash.u64(access.compatibility_fingerprint());
    hash.u64(reach.services().len() as u64);
    for service in reach.services() {
        hash.u64(service.normalized_identity());
    }
    PlacementPlanId(hash.finish().max(1))
}

/// Collision-resistant semantic owner of compiler-issued access field keys.
/// The compact layout report fingerprint remains useful for diagnostics and
/// caches, but cannot rejoin a key to a different exact layout.
pub(crate) fn authoritative_access_layout_commitment(
    layout: &LayoutPlanReport,
) -> AccessLayoutCommitment {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.access-field-layout.authoritative.v1\0",
    );
    hash_canonical_layout(&mut digest, layout);
    AccessLayoutCommitment(digest.finish().into())
}

/// Collision-resistant identity for the complete canonical placement policy.
///
/// Unlike the compact FNV compatibility fingerprint above, this value may be
/// used to rejoin provider content evidence to its exact interpretation.
pub(crate) fn authoritative_placement_interpretation(
    plan: &crate::placements::placement_plan::ValidatedPlacementPlan,
) -> ExtentContentInterpretation {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.placement-plan.authoritative.v1\0");
    hash_canonical_layout(&mut digest, &plan.layout);
    hash_u64_sha(&mut digest, plan.access.plan.entries.len() as u64);
    for entry in &plan.access.plan.entries {
        hash_u64_sha(&mut digest, u64::from(entry.key.slot));
        digest.raw(&canonical_field_access_bytes(&entry.access));
    }
    hash_u64_sha(&mut digest, plan.reach.services().len() as u64);
    for service in plan.reach.services() {
        hash_u64_sha(&mut digest, service.normalized_identity());
    }
    let commitment: [u8; 32] = digest.finish().into();
    let compatibility_fingerprint = ExtentContentInterpretationId::from_normalized_identity(
        plan.identity.compatibility_fingerprint(),
    )
    .expect("validated placement compatibility fingerprints are nonzero");
    ExtentContentInterpretation::from_sha256_commitment(compatibility_fingerprint, commitment)
}

fn hash_canonical_layout(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    layout: &layout_plans::LayoutPlanReport,
) {
    hash_u64_sha(digest, layout.schema_report_fingerprint);
    match layout.size {
        Some(size) => {
            digest.byte(1);
            hash_u64_sha(digest, size);
        }
        None => digest.byte(0),
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
        digest.raw(&entry);
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

fn hash_u64_sha(digest: &mut crate::digest_encoder::DomainDigestEncoder, value: u64) {
    digest.u64(value);
}

pub(crate) fn non_authoritative_resource_profile_compatibility_fingerprint(
    length: u64,
    regions: &[ResourceRegion],
) -> ResourceProfileId {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.resource-profile.v1");
    hash.u64(length);
    hash.u64(regions.len() as u64);
    for region in regions {
        hash.u64(region.offset);
        hash.u64(region.length);
        hash.byte(match region.stable {
            StableCapability::None => 0,
            StableCapability::Read => 1,
            StableCapability::Write => 2,
            StableCapability::ReadWrite => 3,
        });
        match &region.external {
            ExternalCapability::None => hash.byte(0),
            ExternalCapability::Access {
                read,
                write,
                transfers,
            } => {
                hash.byte(1);
                hash.byte(match read {
                    ExternalReadBehavior::None => 0,
                    ExternalReadBehavior::Repeatable => 1,
                    ExternalReadBehavior::Destructive => 2,
                });
                hash.byte(u8::from(*write));
                hash_transfer_rules(&mut hash, transfers);
            }
        }
        match &region.atomic {
            AtomicCapability::None => hash.byte(0),
            AtomicCapability::Access { transfers } => {
                hash.byte(1);
                hash.u64(transfers.len() as u64);
                for rule in transfers {
                    hash_transfer_rule(&mut hash, rule.transfer);
                    hash.bytes(&atomic_permission_bits(rule.operations));
                }
            }
        }
        hash.byte(match region.peer {
            PeerWritability::Exclusive => 0,
            PeerWritability::HostileShared => 1,
        });
        hash.u64(region.reach.services().len() as u64);
        for service in region.reach.services() {
            hash.u64(service.normalized_identity());
        }
    }
    ResourceProfileId(hash.finish().max(1))
}

fn hash_transfer_rules(hash: &mut semantic_vocabulary::Fnv1a, rules: &[TransferRule]) {
    hash.u64(rules.len() as u64);
    for rule in rules {
        hash_transfer_rule(hash, *rule);
    }
}

fn hash_transfer_rule(hash: &mut semantic_vocabulary::Fnv1a, rule: TransferRule) {
    hash.u64(u64::from(rule.width_bits));
    hash.u64(rule.alignment_bytes);
}

pub(super) fn non_authoritative_access_plan_compatibility_fingerprint(
    plan: &AccessPlan,
    layout_report_fingerprint: u64,
) -> AccessPlanId {
    // FNV-1a is used as a compact deterministic artifact identity here, never
    // as authorization or collision-resistant evidence. The versioned prefix
    // makes any future vocabulary change an explicit identity migration.
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.bytes(b"omega.access-plan.v5");
    hash.u64(layout_report_fingerprint);
    hash.u64(plan.entries.len() as u64);
    for entry in &plan.entries {
        hash.u64(u64::from(entry.key.slot));
        hash.bytes(&canonical_field_access_bytes(&entry.access));
    }
    // Zero is reserved as the inert/no-plan identity throughout the semantic
    // spine. A hash hitting it remains deterministic but is remapped out of
    // the reserved value.
    AccessPlanId(hash.finish().max(1))
}
