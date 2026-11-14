/*
We validate per-field access policies (`access_fields_validation`), allowed
primitive and atomic operations (`access_operations`), effect-footprint conflict
detection (`authorization`), boundary service reach (`boundary_reach`), and
normalized plan identities (`normalized_identities`).
*/

pub(crate) mod access_fields_validation {
    use layout_plans::{
        LayoutPlacementReport, LayoutPlanReport, layout_plan_reports_match_for_replay,
    };

    use crate::plan_policy::normalized_identities::non_authoritative_access_plan_compatibility_fingerprint;
    use crate::{
        AccessExposure, AccessFieldEntry, AccessFields, AccessPermissions, AccessPlanDiagnostic,
        ExternalRead, FieldAccess, FieldAccessDescriptor, FieldRegion, LogicalFieldExtent,
        LogicalFieldFragment, ObservationModel, RelativeEffectFootprint, ValidatedAccessFields,
    };

    pub fn validate_access_fields(
        plan: AccessFields,
        layout: &LayoutPlanReport,
    ) -> Result<ValidatedAccessFields, AccessPlanDiagnostic> {
        let layout_size = layout.size.ok_or_else(|| {
            AccessPlanDiagnostic("placed access requires a fixed-size layout plan".into())
        })?;
        let expected = AccessFields::inaccessible(layout)?;
        if plan.layout_report_fingerprint != expected.layout_report_fingerprint
            || plan.layout_commitment != expected.layout_commitment
            || !layout_plan_reports_match_for_replay(&plan.retained_layout, layout)
        {
            return Err(AccessPlanDiagnostic(
                "access plan belongs to a different validated layout".into(),
            ));
        }
        if plan.entries.len() != expected.entries.len()
            || plan
                .entries
                .iter()
                .zip(&expected.entries)
                .any(|(actual, expected)| {
                    actual.key != expected.key || actual.field != expected.field
                })
        {
            return Err(AccessPlanDiagnostic(
                "access plan does not contain exactly one canonical decision per schema field"
                    .into(),
            ));
        }

        let mut descriptors = Vec::with_capacity(plan.entries.len());
        let mut field_regions = Vec::new();
        for entry in &plan.entries {
            if let FieldAccess::Region {
                byte_length,
                byte_alignment,
                transfer_width_bits,
                observation,
                permissions,
                exposure,
            } = entry.access
            {
                if !permissions.any()
                    || permissions.atomic.any()
                    || observation == ObservationModel::Atomic
                    || (observation == ObservationModel::Stable && permissions.take)
                    || (permissions.read && permissions.take)
                {
                    return Err(AccessPlanDiagnostic(format!(
                        "structural field `{}` requires nonempty Stable read/write or External read/take/write permissions, with at most one read behavior",
                        entry.field
                    )));
                }
                let transfer_bytes = u64::from(transfer_width_bits / 8);
                if byte_length == 0
                    || !byte_alignment.is_power_of_two()
                    || transfer_width_bits == 0
                    || transfer_width_bits > 128
                    || transfer_width_bits % 8 != 0
                    || !transfer_bytes.is_power_of_two()
                    || byte_length % transfer_bytes != 0
                    || byte_alignment % transfer_bytes != 0
                {
                    return Err(AccessPlanDiagnostic(format!(
                        "structural field `{}` requires a nonempty extent tiled by exact power-of-two scalar transfers and compatible alignment",
                        entry.field
                    )));
                }
                let mut placements = layout.entries.iter().filter(|row| row.field == entry.field);
                let (Some(row), None) = (placements.next(), placements.next()) else {
                    return Err(AccessPlanDiagnostic(format!(
                        "structural field `{}` requires one complete At placement",
                        entry.field
                    )));
                };
                let LayoutPlacementReport::At { offset } = row.placement else {
                    return Err(AccessPlanDiagnostic(format!(
                        "structural field `{}` requires one complete At placement",
                        entry.field
                    )));
                };
                validate_transfer_range(&entry.field, offset, byte_length, layout_size)?;
                field_regions.push(FieldRegion {
                    key: entry.key,
                    field: entry.field.clone(),
                    byte_offset: offset,
                    byte_length,
                    byte_alignment,
                    transfer_width_bits,
                    observation,
                    permissions,
                    exposure,
                });
                continue;
            }
            let Some(policy) = validate_entry_policy(entry)? else {
                continue;
            };
            let (container_byte_offset, logical_extent, effect_footprint) =
                validate_entry_geometry(
                    &entry.field,
                    policy.transfer_width_bits,
                    layout,
                    layout_size,
                )?;
            descriptors.push(FieldAccessDescriptor {
                key: entry.key,
                field: entry.field.clone(),
                container_byte_offset,
                transfer_width_bits: policy.transfer_width_bits,
                logical_extent,
                effect_footprint,
                observation: policy.observation,
                permissions: policy.permissions,
                exposure: policy.exposure,
            });
        }
        validate_external_write_units(&descriptors)?;
        validate_destructive_access_units(&descriptors)?;
        for descriptor in descriptors
            .iter()
            .filter(|descriptor| descriptor.permissions.take)
        {
            if let Some(region) = field_regions.iter().find(|region| {
                descriptor
                    .effect_footprint
                    .overlaps(RelativeEffectFootprint {
                        byte_offset: region.byte_offset,
                        length_bytes: region.byte_length,
                    })
            }) {
                return Err(AccessPlanDiagnostic(format!(
                    "destructive field `{}` overlaps structural field `{}`; one destructive unit derives one whole-snapshot take",
                    descriptor.field, region.field,
                )));
            }
        }
        validate_atomic_transfer_units(&descriptors)?;
        for destructive in field_regions
            .iter()
            .filter(|region| region.permissions.take)
        {
            let footprint = RelativeEffectFootprint {
                byte_offset: destructive.byte_offset,
                length_bytes: destructive.byte_length,
            };
            if descriptors
                .iter()
                .any(|descriptor| descriptor.effect_footprint.overlaps(footprint))
                || field_regions.iter().any(|other| {
                    other.key != destructive.key
                        && footprint.overlaps(RelativeEffectFootprint {
                            byte_offset: other.byte_offset,
                            length_bytes: other.byte_length,
                        })
                })
            {
                return Err(AccessPlanDiagnostic(format!(
                    "destructive structural field `{}` overlaps another accessible field",
                    destructive.field
                )));
            }
        }

        let layout_report_fingerprint = plan.layout_report_fingerprint;
        let layout_commitment = plan.layout_commitment;
        let identity = non_authoritative_access_plan_compatibility_fingerprint(
            &plan,
            layout_report_fingerprint,
        );
        Ok(ValidatedAccessFields {
            identity,
            layout_report_fingerprint,
            layout_commitment,
            plan,
            fields: descriptors,
            field_regions,
            layout_size_bytes: layout_size,
        })
    }

    fn validate_atomic_transfer_units(
        descriptors: &[FieldAccessDescriptor],
    ) -> Result<(), AccessPlanDiagnostic> {
        let atomic = descriptors
            .iter()
            .filter(|descriptor| descriptor.observation == ObservationModel::Atomic)
            .collect::<Vec<_>>();
        for (index, left) in atomic.iter().enumerate() {
            if let Some(right) = atomic[index + 1..].iter().find(|right| {
                left.effect_footprint.overlaps(right.effect_footprint)
                    && left.effect_footprint != right.effect_footprint
            }) {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic fields `{}` and `{}` select overlapping transfer containers {}..{} and {}..{}; one active atomic placement cannot mix widths over the same bytes",
                    left.field,
                    right.field,
                    left.effect_footprint.byte_offset,
                    left.effect_footprint.end(),
                    right.effect_footprint.byte_offset,
                    right.effect_footprint.end(),
                )));
            }
        }
        Ok(())
    }

    fn validate_external_write_units(
        descriptors: &[FieldAccessDescriptor],
    ) -> Result<(), AccessPlanDiagnostic> {
        for descriptor in descriptors.iter().filter(|descriptor| {
            descriptor.observation == ObservationModel::External && descriptor.permissions.write
        }) {
            if !logical_extent_covers_effect(
                &descriptor.logical_extent,
                descriptor.effect_footprint,
            ) {
                return Err(AccessPlanDiagnostic(format!(
                    "external field `{}` names only part of its {}-byte transfer container; a generic External write must cover the complete admitted container",
                    descriptor.field, descriptor.effect_footprint.length_bytes
                )));
            }
        }
        Ok(())
    }

    fn validate_destructive_access_units(
        descriptors: &[FieldAccessDescriptor],
    ) -> Result<(), AccessPlanDiagnostic> {
        for destructive in descriptors
            .iter()
            .filter(|descriptor| descriptor.permissions.take)
        {
            if !logical_extent_covers_effect(
                &destructive.logical_extent,
                destructive.effect_footprint,
            ) {
                return Err(AccessPlanDiagnostic(format!(
                    "destructive field `{}` names only part of its {}-byte transfer container; expose one whole-container snapshot and project fields from the owned result",
                    destructive.field, destructive.effect_footprint.length_bytes
                )));
            }
            if let Some(overlapping) = descriptors.iter().find(|candidate| {
                candidate.key != destructive.key
                    && candidate
                        .effect_footprint
                        .overlaps(destructive.effect_footprint)
            }) {
                return Err(AccessPlanDiagnostic(format!(
                    "destructive field `{}` and field `{}` expose overlapping transfer containers; one destructive unit derives one whole-snapshot take",
                    destructive.field, overlapping.field
                )));
            }
        }
        Ok(())
    }

    fn logical_extent_covers_effect(
        logical: &LogicalFieldExtent,
        effect: RelativeEffectFootprint,
    ) -> bool {
        let Some(effect_start) = effect.byte_offset.checked_mul(8) else {
            return false;
        };
        let Some(effect_end) = effect.end().checked_mul(8) else {
            return false;
        };
        let mut by_layout = logical.fragments.to_vec();
        by_layout.sort_unstable_by_key(|fragment| fragment.layout_bit_offset);
        let mut next_bit = effect_start;
        for fragment in by_layout {
            if fragment.layout_bit_offset != next_bit {
                return false;
            }
            let Some(end) = fragment.layout_bit_offset.checked_add(fragment.width_bits) else {
                return false;
            };
            if end > effect_end {
                return false;
            }
            next_bit = end;
        }
        if next_bit != effect_end {
            return false;
        }

        let mut by_source = logical.fragments.to_vec();
        by_source.sort_unstable_by_key(|fragment| fragment.source_bit_offset);
        let mut next_source_bit = 0;
        for fragment in by_source {
            if fragment.source_bit_offset != next_source_bit {
                return false;
            }
            let Some(end) = fragment.source_bit_offset.checked_add(fragment.width_bits) else {
                return false;
            };
            next_source_bit = end;
        }
        next_source_bit == effect_end - effect_start
    }

    #[derive(Debug, Clone, Copy)]
    struct ValidatedEntryPolicy {
        transfer_width_bits: u16,
        observation: ObservationModel,
        permissions: AccessPermissions,
        exposure: AccessExposure,
    }

    fn validate_entry_policy(
        entry: &AccessFieldEntry,
    ) -> Result<Option<ValidatedEntryPolicy>, AccessPlanDiagnostic> {
        let policy = match entry.access {
            FieldAccess::Inaccessible => return Ok(None),
            FieldAccess::Region { .. } => {
                return Err(AccessPlanDiagnostic(
                    "structural Stable permission is not a primitive transfer policy".into(),
                ));
            }
            FieldAccess::Stable {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                if !read && !write {
                    return Err(AccessPlanDiagnostic(format!(
                        "stable field `{}` exposes no operation; use Inaccessible",
                        entry.field
                    )));
                }
                ValidatedEntryPolicy {
                    transfer_width_bits,
                    observation: ObservationModel::Stable,
                    permissions: AccessPermissions {
                        read,
                        write,
                        ..AccessPermissions::default()
                    },
                    exposure,
                }
            }
            FieldAccess::External {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                if read == ExternalRead::None && !write {
                    return Err(AccessPlanDiagnostic(format!(
                        "external field `{}` exposes no operation; use Inaccessible",
                        entry.field
                    )));
                }
                ValidatedEntryPolicy {
                    transfer_width_bits,
                    observation: ObservationModel::External,
                    permissions: AccessPermissions {
                        read: read == ExternalRead::Read,
                        take: read == ExternalRead::Take,
                        write,
                        ..AccessPermissions::default()
                    },
                    exposure,
                }
            }
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                exposure,
            } => {
                if !operations.any() {
                    return Err(AccessPlanDiagnostic(format!(
                        "atomic field `{}` exposes no operation; use Inaccessible",
                        entry.field
                    )));
                }
                ValidatedEntryPolicy {
                    transfer_width_bits,
                    observation: ObservationModel::Atomic,
                    permissions: AccessPermissions {
                        atomic: operations,
                        ..AccessPermissions::default()
                    },
                    exposure,
                }
            }
        };
        if policy.transfer_width_bits == 0
            || policy.transfer_width_bits > 128
            || !policy.transfer_width_bits.is_multiple_of(8)
        {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` transfer width {} is not a supported whole-byte width in 8..=128",
                entry.field, policy.transfer_width_bits
            )));
        }
        Ok(Some(policy))
    }

    pub(crate) fn validate_entry_geometry(
        field: &str,
        transfer_width_bits: u16,
        layout: &LayoutPlanReport,
        layout_size: u64,
    ) -> Result<(u64, LogicalFieldExtent, RelativeEffectFootprint), AccessPlanDiagnostic> {
        let placements = layout
            .entries
            .iter()
            .filter(|entry| entry.field == field)
            .map(|entry| entry.placement)
            .collect::<Vec<_>>();
        if placements.is_empty() {
            return Err(AccessPlanDiagnostic(format!(
                "access field `{field}` does not exist in the layout plan"
            )));
        }

        let transfer_bytes = u64::from(transfer_width_bits / 8);
        match placements.as_slice() {
            [LayoutPlacementReport::At { offset }] => {
                let offset = *offset;
                validate_transfer_range(field, offset, transfer_bytes, layout_size)?;
                Ok((
                    offset,
                    LogicalFieldExtent {
                        fragments: vec![LogicalFieldFragment {
                            layout_bit_offset: offset * 8,
                            source_bit_offset: 0,
                            width_bits: u64::from(transfer_width_bits),
                        }],
                    },
                    RelativeEffectFootprint {
                        byte_offset: offset,
                        length_bytes: transfer_bytes,
                    },
                ))
            }
            [
                LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    ..
                },
            ] => {
                if *stored_width != u64::from(transfer_width_bits) {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{field}` requests a {transfer_width_bits}-bit transfer over a {stored_width}-bit stored integer"
                    )));
                }
                let offset = *offset;
                validate_transfer_range(field, offset, transfer_bytes, layout_size)?;
                Ok((
                    offset,
                    LogicalFieldExtent {
                        fragments: vec![LogicalFieldFragment {
                            layout_bit_offset: offset * 8,
                            source_bit_offset: 0,
                            width_bits: *stored_width,
                        }],
                    },
                    RelativeEffectFootprint {
                        byte_offset: offset,
                        length_bytes: transfer_bytes,
                    },
                ))
            }
            placements => {
                let mut container = None;
                let mut fragments = Vec::with_capacity(placements.len());
                for placement in placements {
                    let LayoutPlacementReport::Bits {
                        container: candidate,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    } = placement
                    else {
                        return Err(AccessPlanDiagnostic(format!(
                            "access field `{field}` mixes whole and fragmented placement"
                        )));
                    };
                    if *container_width != u64::from(transfer_width_bits) {
                        return Err(AccessPlanDiagnostic(format!(
                            "access field `{}` requests a {}-bit transfer over a {container_width}-bit container",
                            field, transfer_width_bits
                        )));
                    }
                    if container
                        .replace(*candidate)
                        .is_some_and(|prior| prior != *candidate)
                    {
                        return Err(AccessPlanDiagnostic(format!(
                            "fragmented field `{}` spans multiple containers and cannot be projected through one exact access",
                            field
                        )));
                    }
                    fragments.push(LogicalFieldFragment {
                        layout_bit_offset: candidate * 8 + destination_lsb,
                        source_bit_offset: *source_lsb,
                        width_bits: *width,
                    });
                }
                let container = container.expect("nonempty placements");
                validate_transfer_range(field, container, transfer_bytes, layout_size)?;
                fragments.sort_unstable_by_key(|fragment| {
                    (
                        fragment.source_bit_offset,
                        fragment.layout_bit_offset,
                        fragment.width_bits,
                    )
                });
                Ok((
                    container,
                    LogicalFieldExtent { fragments },
                    RelativeEffectFootprint {
                        byte_offset: container,
                        length_bytes: transfer_bytes,
                    },
                ))
            }
        }
    }

    fn validate_transfer_range(
        field: &str,
        offset: u64,
        transfer_bytes: u64,
        layout_size: u64,
    ) -> Result<(), AccessPlanDiagnostic> {
        let end = offset.checked_add(transfer_bytes).ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "access field `{field}` transfer byte range overflows"
            ))
        })?;
        if end > layout_size {
            return Err(AccessPlanDiagnostic(format!(
                "access field `{}` transfer at {offset}..{end} exceeds {layout_size}-byte layout",
                field
            )));
        }
        Ok(())
    }
}

pub(crate) mod access_operations {
    use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum BorrowPolarity {
        Shared,
        Exclusive,
    }

    impl From<extents::LoanPolarity> for BorrowPolarity {
        fn from(polarity: extents::LoanPolarity) -> Self {
            match polarity {
                extents::LoanPolarity::Shared => BorrowPolarity::Shared,
                extents::LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AccessOperation {
        Read,
        Take,
        Write,
        CompoundMutation,
        Atomic(AtomicAccessOperation),
    }

    impl AccessOperation {
        pub const fn receiver_polarity(self) -> BorrowPolarity {
            match self {
                Self::Read => BorrowPolarity::Shared,
                Self::Atomic(operation) => operation.receiver_polarity(),
                Self::Take | Self::Write | Self::CompoundMutation => BorrowPolarity::Exclusive,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AtomicAccessOperation {
        Load(MemoryOrdering),
        Store(MemoryOrdering),
        FetchAdd(MemoryOrdering),
        FetchSub(MemoryOrdering),
        FetchXor(MemoryOrdering),
        FetchOr(MemoryOrdering),
        FetchAnd(MemoryOrdering),
        Swap(MemoryOrdering),
        CompareExchange {
            success: MemoryOrdering,
            failure: MemoryOrdering,
        },
        CompareExchangeOnce {
            success: MemoryOrdering,
            failure: MemoryOrdering,
        },
        TryExchangeOnce {
            success: MemoryOrdering,
            failure: MemoryOrdering,
        },
    }

    impl AtomicAccessOperation {
        pub const fn receiver_polarity(self) -> BorrowPolarity {
            BorrowPolarity::Shared
        }

        pub const fn single_attempt(
            observation: language_core::atomic::AtomicCompareExchangeObservationAxis,
            success: MemoryOrdering,
            failure: MemoryOrdering,
        ) -> Self {
            match observation {
                language_core::atomic::AtomicCompareExchangeObservationAxis::Observing => {
                    Self::CompareExchangeOnce { success, failure }
                }
                language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving => {
                    Self::TryExchangeOnce { success, failure }
                }
            }
        }

        pub const fn single_attempt_custody(
            self,
        ) -> Option<language_core::atomic::AtomicCompareExchangeOnceResultCustody> {
            use language_core::atomic::AtomicCompareExchangeObservationAxis as Observation;
            let observation = match self {
                Self::CompareExchangeOnce { .. } => Observation::Observing,
                Self::TryExchangeOnce { .. } => Observation::NonObserving,
                _ => return None,
            };
            Some(
                language_core::atomic::AtomicCompareExchangeOnceResultCustody::for_observation(
                    observation,
                ),
            )
        }

        pub const fn ordering_plan(self) -> AtomicOrderingPlan {
            match self {
                Self::Load(ordering) => AtomicOrderingPlan::Load(ordering),
                Self::Store(ordering) => AtomicOrderingPlan::Store(ordering),
                Self::FetchAdd(ordering)
                | Self::FetchSub(ordering)
                | Self::FetchXor(ordering)
                | Self::FetchOr(ordering)
                | Self::FetchAnd(ordering) => AtomicOrderingPlan::ReadModifyWrite(ordering),
                Self::Swap(ordering) => AtomicOrderingPlan::Swap(ordering),
                Self::CompareExchange { success, failure } => {
                    AtomicOrderingPlan::CompareExchange { success, failure }
                }
                Self::CompareExchangeOnce { success, failure }
                | Self::TryExchangeOnce { success, failure } => {
                    AtomicOrderingPlan::CompareExchangeOnce { success, failure }
                }
            }
        }
    }
}

pub(crate) mod authorization {
    use language_core::atomic::AtomicOrderingPlan;

    use crate::{
        AccessOperation, AccessPlanDiagnostic, AtomicAccessOperation, BorrowPolarity,
        FieldAccessDescriptor, ObservationModel,
    };

    pub(crate) fn authorize_descriptor(
        descriptor: &FieldAccessDescriptor,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_operation_ordering(operation)?;
        let borrow_permits = match operation.receiver_polarity() {
            BorrowPolarity::Shared => true,
            BorrowPolarity::Exclusive => {
                current_borrow == BorrowPolarity::Exclusive
                    && source_loan == BorrowPolarity::Exclusive
            }
        };
        let permission_admits = match operation {
            AccessOperation::Read => descriptor.permissions.read,
            AccessOperation::Take => descriptor.permissions.take,
            AccessOperation::Write => descriptor.permissions.write,
            AccessOperation::CompoundMutation => {
                descriptor.observation == ObservationModel::Stable
                    && descriptor.permissions.read
                    && descriptor.permissions.write
            }
            AccessOperation::Atomic(AtomicAccessOperation::Load(_)) => {
                descriptor.permissions.atomic.load
            }
            AccessOperation::Atomic(AtomicAccessOperation::Store(_)) => {
                descriptor.permissions.atomic.store
            }
            AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(_)) => {
                descriptor.permissions.atomic.fetch_add
            }
            AccessOperation::Atomic(AtomicAccessOperation::FetchSub(_)) => {
                descriptor.permissions.atomic.fetch_sub
            }
            AccessOperation::Atomic(AtomicAccessOperation::FetchXor(_)) => {
                descriptor.permissions.atomic.fetch_xor
            }
            AccessOperation::Atomic(AtomicAccessOperation::FetchOr(_)) => {
                descriptor.permissions.atomic.fetch_or
            }
            AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(_)) => {
                descriptor.permissions.atomic.fetch_and
            }
            AccessOperation::Atomic(AtomicAccessOperation::Swap(_)) => {
                descriptor.permissions.atomic.swap
            }
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchange { .. }) => {
                descriptor.permissions.atomic.compare_exchange
            }
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce { .. }) => {
                descriptor.permissions.atomic.compare_exchange_once
            }
            AccessOperation::Atomic(AtomicAccessOperation::TryExchangeOnce { .. }) => {
                descriptor.permissions.atomic.try_exchange_once
            }
        };
        if borrow_permits && permission_admits {
            Ok(())
        } else {
            Err(AccessPlanDiagnostic(format!(
                "field `{}` does not permit {operation:?} through a {current_borrow:?} current borrow over a {source_loan:?} source loan",
                descriptor.field,
            )))
        }
    }

    pub(crate) fn validate_operation_ordering(
        operation: AccessOperation,
    ) -> Result<(), AccessPlanDiagnostic> {
        let AccessOperation::Atomic(operation) = operation else {
            return Ok(());
        };
        let ordering = operation.ordering_plan();
        let legal = match ordering {
            AtomicOrderingPlan::Load(ordering) => ordering.valid_for_load(),
            AtomicOrderingPlan::Store(ordering) => ordering.valid_for_store(),
            AtomicOrderingPlan::ReadModifyWrite(_) | AtomicOrderingPlan::Swap(_) => true,
            AtomicOrderingPlan::CompareExchange { success, failure }
            | AtomicOrderingPlan::CompareExchangeOnce { success, failure } => {
                failure.valid_compare_exchange_failure(success)
            }
        };
        if legal {
            Ok(())
        } else {
            Err(AccessPlanDiagnostic(format!(
                "atomic access carries an invalid ordering plan: {ordering:?}"
            )))
        }
    }
}

pub(crate) mod boundary_reach {
    use crate::AccessPlanDiagnostic;
    use crate::normalized_identity;
    use std::collections::BTreeSet;

    normalized_identity!(
        BoundaryServiceReachId,
        "boundary-service reach identity cannot be zero"
    );

    #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct BoundaryReach {
        services: BTreeSet<BoundaryServiceReachId>,
    }

    impl BoundaryReach {
        pub fn from_services(services: impl IntoIterator<Item = BoundaryServiceReachId>) -> Self {
            Self {
                services: services.into_iter().collect(),
            }
        }

        pub fn services(&self) -> impl ExactSizeIterator<Item = BoundaryServiceReachId> + '_ {
            self.services.iter().copied()
        }

        pub fn contains(&self, service: BoundaryServiceReachId) -> bool {
            self.services.contains(&service)
        }

        pub fn contains_all(&self, required: &Self) -> bool {
            required.services.is_subset(&self.services)
        }

        pub fn intersection(&self, other: &Self) -> Self {
            Self {
                services: self
                    .services
                    .intersection(&other.services)
                    .copied()
                    .collect(),
            }
        }
    }
}

pub(crate) mod normalized_identities {
    use crate::access_fields::AccessLayoutCommitment;
    use crate::{
        AccessExposure, AccessFields, AccessFieldsId, AccessPlanId, AtomicCapability,
        AtomicPermissions, BoundaryReach, ExternalCapability, ExternalRead, ExternalReadBehavior,
        FieldAccess, PeerWritability, ResourceProfileId, ResourceRegion, StableCapability,
        TransferRule,
    };
    use extents::{ExtentContentInterpretation, ExtentContentInterpretationId};
    use layout_plans::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    };

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

    pub(crate) fn non_authoritative_access_compatibility_fingerprint(
        access: AccessFieldsId,
        reach: &BoundaryReach,
    ) -> AccessPlanId {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        hash.bytes(b"omega.placement-plan.v1");
        hash.u64(access.compatibility_fingerprint());
        hash.u64(reach.services().len() as u64);
        for service in reach.services() {
            hash.u64(service.normalized_identity());
        }
        AccessPlanId(hash.finish().max(1))
    }

    pub(crate) fn authoritative_access_layout_commitment(
        layout: &LayoutPlanReport,
    ) -> AccessLayoutCommitment {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.access-field-layout.authoritative.v1\0",
        );
        hash_canonical_layout(&mut digest, layout);
        AccessLayoutCommitment(digest.finish())
    }

    pub(crate) fn authoritative_access_interpretation(
        plan: &crate::ValidatedAccessPlan,
    ) -> ExtentContentInterpretation {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.placement-plan.authoritative.v1\0",
        );
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
        let commitment: [u8; 32] = digest.finish();
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
        plan: &AccessFields,
        layout_report_fingerprint: u64,
    ) -> AccessFieldsId {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        hash.bytes(b"omega.access-plan.v5");
        hash.u64(layout_report_fingerprint);
        hash.u64(plan.entries.len() as u64);
        for entry in &plan.entries {
            hash.u64(u64::from(entry.key.slot));
            hash.bytes(&canonical_field_access_bytes(&entry.access));
        }
        AccessFieldsId(hash.finish().max(1))
    }
}
